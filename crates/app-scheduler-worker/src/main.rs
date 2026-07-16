use platform_consensus::LeaseCoordinator;
use platform_db::{
    Database, GameplayCompletion, GameplayCompletionKind, GameplayProcessResult, ScheduledTaskRow,
};
use platform_scheduler::{JobConfig, JobHandler, JobKind, Scheduler};
use platform_sharding::{ShardConfig, ShardingCatalog};
use platform_tenant_routing::{TenantRouteConfig, TenantRouter, TenantRoutingDecision};
use platform_worker_runtime::WorkerRuntime;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::oneshot;
use tokio::time::{sleep, Duration};

const SERVICE_NAME: &str = "app-scheduler-worker";
const GAMEPLAY_QUEUE_LIMIT_PER_KIND: usize = 100;
const FLEET_PROCESS_LIMIT: usize = 100;
const SCHEDULER_JOB_TYPES: [&str; 4] = [
    "scheduler.game_loop",
    "scheduler.fleet",
    "scheduler.moon_destroy",
    "scheduler.shard_health",
];

#[derive(Debug)]
struct ProcessedTask {
    result: serde_json::Value,
    completions: Vec<GameplayCompletion>,
}

#[derive(Clone)]
struct SchedulerProcessingConfig {
    database: Database,
    worker_id: String,
    realtime_url: Option<String>,
    lease_secs: i64,
    retry_delay_secs: i64,
    max_attempts: i32,
}

struct SchedulerRouteSettings {
    tenant_id: String,
    shard_id: String,
    queue_name: String,
    region: String,
}

#[derive(Clone, Copy)]
struct SchedulerCadences {
    game_loop_secs: u64,
    fleet_secs: u64,
    moon_secs: u64,
    shard_health_secs: u64,
}

struct SchedulerBootstrapConfig {
    route: SchedulerRouteSettings,
    processing: SchedulerProcessingConfig,
    cadences: SchedulerCadences,
}

#[derive(Clone, Copy)]
struct SchedulerJobSpec {
    task_type: &'static str,
    cadence_secs: u64,
}

struct TickRoute {
    tenant_id: String,
    shard_id: String,
    queue_name: String,
}

#[tokio::main]
async fn main() -> Result<(), String> {
    platform_observability::init(SERVICE_NAME);

    let run_once = bool_env("SCHEDULER_RUN_ONCE");
    let game_loop_secs = bounded_env("GAME_LOOP_INTERVAL_SECS", 5_u64, 1)?;
    let fleet_secs = bounded_env("FLEET_SCHEDULER_INTERVAL_SECS", 10_u64, 1)?;
    let moon_secs = bounded_env("MOON_DESTROY_INTERVAL_SECS", 10_u64, 1)?;
    let shard_health_secs = bounded_env("SHARD_HEALTH_INTERVAL_SECS", 60_u64, 1)?;
    let realtime_url = std::env::var("REALTIME_GATEWAY_URL")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let worker_id = std::env::var("SCHEDULER_WORKER_ID")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "app-scheduler-worker".to_string());
    let lease_secs = bounded_env("SCHED_TASK_LEASE_SECS", 30_i64, 5)?;
    let retry_delay_secs = bounded_env("SCHED_TASK_RETRY_DELAY_SECS", 15_i64, 1)?;
    let max_attempts = bounded_env("SCHED_TASK_MAX_ATTEMPTS", 3_i32, 1)?;
    let runtime_max_inflight = bounded_env("SCHED_RUNTIME_MAX_INFLIGHT", 32_usize, 1)?;
    let tenant_id = std::env::var("SCHED_TENANT_ID")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "tenant-default".to_string());
    let shard_id = std::env::var("SCHED_SHARD_ID")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "shard-default".to_string());
    let queue_name = std::env::var("SCHED_QUEUE_NAME")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "scheduler.default".to_string());
    let region = std::env::var("SCHED_REGION")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "global".to_string());
    let database = match Database::try_from_env() {
        Ok(Some(database)) => database,
        Ok(None) => {
            let error = "DATABASE_URL is required for durable scheduler processing".to_string();
            tracing::error!(service = SERVICE_NAME, %error);
            return Err(error);
        }
        Err(error) => {
            tracing::error!(
                service = SERVICE_NAME,
                %error,
                "failed to initialize durable scheduler repository"
            );
            return Err(format!(
                "failed to initialize durable scheduler repository: {error}"
            ));
        }
    };

    tracing::info!(
        service = SERVICE_NAME,
        run_once,
        game_loop_secs,
        fleet_secs,
        moon_secs,
        shard_health_secs,
        has_realtime_url = realtime_url.is_some(),
        worker_id,
        lease_secs,
        retry_delay_secs,
        max_attempts,
        runtime_max_inflight,
        tenant_id,
        shard_id,
        queue_name,
        region,
        "scheduler worker started"
    );
    let lease_coordinator = Arc::new(LeaseCoordinator::new());
    let runtime = Arc::new(WorkerRuntime::current(runtime_max_inflight));
    let scheduler = match bootstrap_platform_scheduler(
        Arc::clone(&lease_coordinator),
        Arc::clone(&runtime),
        SchedulerBootstrapConfig {
            route: SchedulerRouteSettings {
                tenant_id: tenant_id.clone(),
                shard_id: shard_id.clone(),
                queue_name: queue_name.clone(),
                region: region.clone(),
            },
            processing: SchedulerProcessingConfig {
                database,
                worker_id: worker_id.clone(),
                realtime_url: realtime_url.clone(),
                lease_secs,
                retry_delay_secs,
                max_attempts,
            },
            cadences: SchedulerCadences {
                game_loop_secs,
                fleet_secs,
                moon_secs,
                shard_health_secs,
            },
        },
    )
    .await
    {
        Ok(scheduler) => scheduler,
        Err(error) => {
            tracing::error!(service = SERVICE_NAME, %error, "failed to initialize platform scheduler");
            return Err(format!("failed to initialize platform scheduler: {error}"));
        }
    };

    if run_once {
        let mut failures = Vec::new();
        for job_id in SCHEDULER_JOB_TYPES {
            if let Err(error) = trigger_platform_job(&scheduler, job_id).await {
                tracing::error!(service = SERVICE_NAME, job_id, %error, "run-once job failed");
                failures.push(format!("{job_id}: {error}"));
            }
            platform_observability::emit_consensus_snapshot(
                SERVICE_NAME,
                lease_coordinator.as_ref(),
                SCHEDULER_JOB_TYPES.len(),
            )
            .await;
        }
        runtime.shutdown(Duration::from_secs(5)).await;
        sleep(Duration::from_millis(25)).await;
        return if failures.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "run-once scheduler failures: {}",
                failures.join("; ")
            ))
        };
    }

    let mut game_tick = tokio::time::interval(Duration::from_secs(game_loop_secs));
    let mut fleet_tick = tokio::time::interval(Duration::from_secs(fleet_secs));
    let mut moon_tick = tokio::time::interval(Duration::from_secs(moon_secs));
    let mut shard_tick = tokio::time::interval(Duration::from_secs(shard_health_secs));

    loop {
        tokio::select! {
            _ = game_tick.tick() => {
                trigger_recurring_platform_job(&scheduler, "scheduler.game_loop").await;
                platform_observability::emit_consensus_snapshot(
                    SERVICE_NAME,
                    lease_coordinator.as_ref(),
                    4,
                )
                .await;
            }
            _ = fleet_tick.tick() => {
                trigger_recurring_platform_job(&scheduler, "scheduler.fleet").await;
                platform_observability::emit_consensus_snapshot(
                    SERVICE_NAME,
                    lease_coordinator.as_ref(),
                    4,
                )
                .await;
            }
            _ = moon_tick.tick() => {
                trigger_recurring_platform_job(&scheduler, "scheduler.moon_destroy").await;
                platform_observability::emit_consensus_snapshot(
                    SERVICE_NAME,
                    lease_coordinator.as_ref(),
                    4,
                )
                .await;
            }
            _ = shard_tick.tick() => {
                trigger_recurring_platform_job(&scheduler, "scheduler.shard_health").await;
                platform_observability::emit_consensus_snapshot(
                    SERVICE_NAME,
                    lease_coordinator.as_ref(),
                    4,
                )
                .await;
            }
        }
    }
}

async fn trigger_platform_job(scheduler: &Scheduler, job_id: &str) -> Result<(), String> {
    scheduler
        .trigger_job(job_id)
        .await
        .map_err(|error| error.to_string())
}

async fn trigger_recurring_platform_job(scheduler: &Scheduler, job_id: &str) {
    if let Err(error) = trigger_platform_job(scheduler, job_id).await {
        tracing::warn!(
            service = SERVICE_NAME,
            job_id,
            %error,
            "platform scheduler trigger failed"
        );
    }
}

async fn bootstrap_platform_scheduler(
    lease_coordinator: Arc<LeaseCoordinator>,
    runtime: Arc<WorkerRuntime>,
    config: SchedulerBootstrapConfig,
) -> Result<Arc<Scheduler>, String> {
    let SchedulerBootstrapConfig {
        route,
        processing,
        cadences,
    } = config;
    let tenant_router = Arc::new(TenantRouter::with_leases(Arc::clone(&lease_coordinator)));
    let catalog = Arc::new(ShardingCatalog::new());
    catalog
        .register_shard(ShardConfig {
            shard_id: route.shard_id.clone(),
            region: route.region.clone(),
            allowed_tenants: HashSet::from_iter(vec![route.tenant_id.clone()]),
            consensus_resource: Some(format!("scheduler-shard:{}", route.shard_id)),
        })
        .await;
    tenant_router
        .register(TenantRouteConfig {
            tenant_id: route.tenant_id.clone(),
            shard_id: route.shard_id.clone(),
            queue_name: route.queue_name.clone(),
            region: route.region.clone(),
            max_inflight: 8,
            max_per_second: 0,
            consensus_resource: Some(format!("scheduler-tenant:{}", route.tenant_id)),
            lease_ttl: Duration::from_secs(processing.lease_secs.max(1) as u64),
        })
        .await;

    let scheduler = Arc::new(Scheduler::new(tenant_router, catalog));
    for job in scheduler_job_specs(cadences) {
        register_scheduler_job(
            Arc::clone(&scheduler),
            Arc::clone(&runtime),
            job,
            &route,
            processing.clone(),
        )
        .await?;
    }

    Ok(scheduler)
}

fn scheduler_job_specs(cadences: SchedulerCadences) -> [SchedulerJobSpec; 4] {
    [
        SchedulerJobSpec {
            task_type: SCHEDULER_JOB_TYPES[0],
            cadence_secs: cadences.game_loop_secs,
        },
        SchedulerJobSpec {
            task_type: SCHEDULER_JOB_TYPES[1],
            cadence_secs: cadences.fleet_secs,
        },
        SchedulerJobSpec {
            task_type: SCHEDULER_JOB_TYPES[2],
            cadence_secs: cadences.moon_secs,
        },
        SchedulerJobSpec {
            task_type: SCHEDULER_JOB_TYPES[3],
            cadence_secs: cadences.shard_health_secs,
        },
    ]
}

async fn register_scheduler_job(
    scheduler: Arc<Scheduler>,
    runtime: Arc<WorkerRuntime>,
    job: SchedulerJobSpec,
    route: &SchedulerRouteSettings,
    processing: SchedulerProcessingConfig,
) -> Result<(), String> {
    let task_type_owned = job.task_type.to_string();
    let task_type_for_handler = task_type_owned.clone();
    let handler: JobHandler = Arc::new(move |decision: TenantRoutingDecision| {
        let task_type = task_type_for_handler.clone();
        let runtime = Arc::clone(&runtime);
        let processing = processing.clone();
        Box::pin(async move {
            let tenant_id = decision.tenant_id().to_string();
            let shard_id = decision.route.shard_id.clone();
            let queue_name = decision.route.queue_name.clone();
            let context = decision.guard.context().clone();
            let tick_route = TickRoute {
                tenant_id,
                shard_id,
                queue_name,
            };
            let (done_tx, done_rx) = oneshot::channel::<Result<(), String>>();
            runtime
                .spawn_tenant_task(context, async move {
                    let result = enqueue_and_process_tick(
                        &processing,
                        &task_type,
                        job.cadence_secs,
                        &tick_route,
                    )
                    .await;
                    let runtime_result = result
                        .as_ref()
                        .map(|_| ())
                        .map_err(|error| anyhow::anyhow!(error.clone()));
                    let _ = done_tx.send(result);
                    runtime_result
                })
                .map_err(|error| anyhow::anyhow!("runtime schedule failure: {error}"))?;
            done_rx
                .await
                .map_err(|error| anyhow::anyhow!("runtime task ended without result: {error}"))?
                .map_err(anyhow::Error::msg)
        })
    });
    let config = JobConfig {
        job_id: task_type_owned.clone(),
        tenant_id: route.tenant_id.clone(),
        description: format!("{task_type_owned} scheduler job"),
        shard_id: route.shard_id.clone(),
        interval: Duration::from_secs(job.cadence_secs.max(1)),
        kind: JobKind::Recurring,
        priority: 100,
        max_failures: 0,
    };
    scheduler
        .register_job(config, handler)
        .await
        .map_err(|error| format!("register {task_type_owned}: {error}"))
}

async fn enqueue_and_process_tick(
    processing: &SchedulerProcessingConfig,
    task_type: &str,
    cadence_secs: u64,
    route: &TickRoute,
) -> Result<(), String> {
    tracing::info!(service = SERVICE_NAME, task_type, "tick start");
    tracing::debug!(
        service = SERVICE_NAME,
        task_type,
        tenant_id = route.tenant_id,
        shard_id = route.shard_id,
        queue_name = route.queue_name,
        "platform scheduler decision acquired"
    );

    let run_at_unix = unix_timestamp();
    let task_key = scheduler_task_key(task_type, run_at_unix, cadence_secs);
    processing
        .database
        .enqueue_scheduled_task(platform_db::ScheduledTaskCreateInput {
            task_type: task_type.to_string(),
            payload: serde_json::json!({
                "taskType": task_type,
                "scheduledAtUnix": run_at_unix,
                "cadenceSecs": cadence_secs,
                "cadenceBucket": run_at_unix / cadence_secs.max(1) as i64
            }),
            run_at_unix,
            task_key: Some(task_key),
        })
        .await
        .map_err(|error| format!("failed enqueue scheduled task {task_type}: {error}"))?;

    let claimed = processing
        .database
        .claim_due_scheduled_tasks(&processing.worker_id, 16, processing.lease_secs)
        .await
        .map_err(|error| format!("failed claim due scheduled tasks: {error}"))?;

    if claimed.is_empty() {
        return Ok(());
    }

    if let Some(url) = processing.realtime_url.as_deref() {
        let event = platform_events::build_event(
            "scheduler.claim",
            &serde_json::json!({
                "workerId": processing.worker_id,
                "count": claimed.len()
            }),
        );
        if let Err(error) = platform_events::publish_http(url, "ops.scheduler", &event).await {
            tracing::warn!(
                service = SERVICE_NAME,
                event_type = "scheduler.claim",
                %error,
                "failed to publish scheduler operations event"
            );
        }
    }

    let mut failures = Vec::new();
    for task in claimed {
        if let Err(error) = process_claimed_task(
            &processing.database,
            &task,
            &processing.worker_id,
            processing.realtime_url.as_deref(),
            processing.lease_secs,
            processing.retry_delay_secs,
            processing.max_attempts,
        )
        .await
        {
            failures.push(format!("task {} ({}): {error}", task.id, task.task_type));
        }
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
    }
}

async fn process_task(
    database: &Database,
    task_type: &str,
    payload: &serde_json::Value,
    fleet_worker_id: &str,
    fleet_lease_seconds: i64,
) -> Result<ProcessedTask, String> {
    match task_type {
        "scheduler.game_loop" => {
            // The repository processor is global today: tenant routing guards
            // this worker invocation, but the queue query itself is not scoped
            // by tenant or universe. A scoped repository API remains follow-up
            // work rather than being approximated in the scheduler.
            let result = database
                .process_due_gameplay_queues(GAMEPLAY_QUEUE_LIMIT_PER_KIND)
                .await
                .map_err(|error| format!("authoritative gameplay processing failed: {error}"))?;
            Ok(ProcessedTask {
                result: gameplay_process_payload(&result, payload),
                completions: result.completions,
            })
        }
        "scheduler.fleet" | "scheduler.moon_destroy" => {
            // Moon-destruction missions are ordinary durable fleet missions.
            // Both cadences may therefore wake the same exact-once resolver;
            // row leases and phase CAS make concurrent ticks safe.
            let result = database
                .process_due_fleet_missions(
                    fleet_worker_id,
                    FLEET_PROCESS_LIMIT,
                    fleet_lease_seconds,
                )
                .await
                .map_err(|error| format!("authoritative fleet processing failed: {error}"))?;
            Ok(ProcessedTask {
                result: serde_json::json!({
                    "kind": if task_type == "scheduler.fleet" {
                        "fleet_tick"
                    } else {
                        "moon_destroy_tick"
                    },
                    "applied": true,
                    "counts": {
                        "arrivals": result.arrivals,
                        "returns": result.returns,
                        "skipped": result.skipped,
                        "failed": result.failed
                    },
                    "fleetIds": result.fleet_ids,
                    "payload": payload
                }),
                completions: Vec::new(),
            })
        }
        "scheduler.shard_health" => Ok(ProcessedTask {
            result: serde_json::json!({
                "kind": "shard_health_tick",
                "applied": true,
                "payload": payload
            }),
            completions: Vec::new(),
        }),
        other => Err(format!("unsupported task type: {other}")),
    }
}

async fn process_claimed_task(
    database: &Database,
    task: &ScheduledTaskRow,
    lease_owner: &str,
    realtime_url: Option<&str>,
    fleet_lease_seconds: i64,
    retry_delay_secs: i64,
    max_attempts: i32,
) -> Result<serde_json::Value, String> {
    let processed = match process_task(
        database,
        &task.task_type,
        &task.payload,
        lease_owner,
        fleet_lease_seconds,
    )
    .await
    {
        Ok(processed) => processed,
        Err(process_error) => {
            let lifecycle_error = record_task_failure(
                database,
                task,
                lease_owner,
                &process_error,
                retry_delay_secs,
                max_attempts,
            )
            .await
            .err();
            tracing::warn!(
                service = SERVICE_NAME,
                task_id = task.id,
                task_type = %task.task_type,
                error = %process_error,
                "scheduled task side effects failed"
            );
            if let Some(url) = realtime_url {
                publish_ops_event(
                    url,
                    "scheduler.task_failed",
                    &serde_json::json!({
                        "taskId": task.id,
                        "taskType": task.task_type,
                        "error": process_error
                    }),
                )
                .await;
            }
            return Err(match lifecycle_error {
                Some(error) => format!("{process_error}; {error}"),
                None => process_error,
            });
        }
    };

    if let Some(url) = realtime_url {
        publish_completion_notifications(url, task, &processed.completions).await;
    }

    match database
        .complete_scheduled_task_for_owner(task.id, lease_owner)
        .await
    {
        Ok(true) => {}
        Ok(false) => {
            return Err(format!(
                "scheduled task {} is no longer owned by {lease_owner} at completion",
                task.id,
            ));
        }
        Err(error) => {
            let completion_error = format!(
                "failed to persist completion for scheduled task {}: {error}",
                task.id
            );
            let lifecycle_error = record_task_failure(
                database,
                task,
                lease_owner,
                &completion_error,
                retry_delay_secs,
                max_attempts,
            )
            .await
            .err();
            return Err(match lifecycle_error {
                Some(error) => format!("{completion_error}; {error}"),
                None => completion_error,
            });
        }
    }

    tracing::info!(
        service = SERVICE_NAME,
        task_id = task.id,
        task_type = %task.task_type,
        result = %processed.result,
        "scheduled task completed"
    );
    if let Some(url) = realtime_url {
        publish_ops_event(
            url,
            "scheduler.task_completed",
            &serde_json::json!({
                "taskId": task.id,
                "taskType": task.task_type,
                "result": processed.result
            }),
        )
        .await;
    }
    Ok(processed.result)
}

async fn record_task_failure(
    database: &Database,
    task: &ScheduledTaskRow,
    lease_owner: &str,
    error_message: &str,
    retry_delay_secs: i64,
    max_attempts: i32,
) -> Result<(), String> {
    match database
        .fail_scheduled_task_for_owner(
            task.id,
            lease_owner,
            error_message,
            retry_delay_secs,
            max_attempts,
        )
        .await
    {
        Ok(true) => Ok(()),
        Ok(false) => Err(format!(
            "scheduled task {} is no longer owned by {lease_owner}; failure was not recorded",
            task.id,
        )),
        Err(error) => Err(format!(
            "failed to persist retry state for scheduled task {}: {error}",
            task.id
        )),
    }
}

fn gameplay_process_payload(
    result: &GameplayProcessResult,
    scheduled_payload: &serde_json::Value,
) -> serde_json::Value {
    let completions = result
        .completions
        .iter()
        .map(gameplay_completion_payload)
        .collect::<Vec<_>>();
    serde_json::json!({
        "kind": "gameplay_queue_tick",
        "counts": {
            "buildings": result.buildings,
            "research": result.research,
            "ships": result.ships,
            "failed": result.failed,
            "completed": completions.len()
        },
        "completions": completions,
        "scheduled": scheduled_payload
    })
}

fn gameplay_completion_payload(completion: &GameplayCompletion) -> serde_json::Value {
    serde_json::json!({
        "kind": gameplay_completion_kind(completion.kind),
        "queueId": completion.queue_id,
        "userId": completion.user_id,
        "planetId": completion.planet_id,
        "itemType": completion.item_type,
        "targetLevel": completion.target_level,
        "quantity": completion.quantity,
        "scoreDelta": completion.score_delta
    })
}

fn gameplay_completion_kind(kind: GameplayCompletionKind) -> &'static str {
    match kind {
        GameplayCompletionKind::Building => "building",
        GameplayCompletionKind::Research => "research",
        GameplayCompletionKind::Shipyard => "shipyard",
    }
}

fn completion_notification_event(
    completion: &GameplayCompletion,
) -> platform_events::EventEnvelope {
    let kind = gameplay_completion_kind(completion.kind);
    let title = match completion.kind {
        GameplayCompletionKind::Building => "Construction completed",
        GameplayCompletionKind::Research => "Research completed",
        GameplayCompletionKind::Shipyard => "Ship production completed",
    };
    let outcome = match (completion.target_level, completion.quantity) {
        (Some(level), _) => format!("{} reached level {level}", completion.item_type),
        (_, Some(quantity)) => format!("{quantity} × {} completed", completion.item_type),
        _ => format!("{} completed", completion.item_type),
    };
    platform_events::build_event(
        "notification.created",
        &serde_json::json!({
            "idempotencyKey": format!("gameplay:{kind}:{}", completion.queue_id),
            "category": "gameplay",
            "priority": "normal",
            "title": title,
            "message": outcome,
            "completion": gameplay_completion_payload(completion)
        }),
    )
}

async fn publish_completion_notifications(
    base_url: &str,
    task: &ScheduledTaskRow,
    completions: &[GameplayCompletion],
) {
    for completion in completions {
        let channel = platform_events::user_notification_channel(&completion.user_id);
        let event = completion_notification_event(completion);
        if let Err(error) = platform_events::publish_http(base_url, &channel, &event).await {
            tracing::warn!(
                service = SERVICE_NAME,
                task_id = task.id,
                queue_id = %completion.queue_id,
                user_id = %completion.user_id,
                %error,
                "failed to publish gameplay completion notification"
            );
        }
    }
}

async fn publish_ops_event(base_url: &str, event_type: &str, payload: &serde_json::Value) {
    let event = platform_events::build_event(event_type, payload);
    if let Err(error) = platform_events::publish_http(base_url, "ops.scheduler", &event).await {
        tracing::warn!(
            service = SERVICE_NAME,
            event_type,
            %error,
            "failed to publish scheduler operations event"
        );
    }
}

fn bool_env(key: &str) -> bool {
    std::env::var(key)
        .map(|value| value.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn bounded_env<T>(key: &str, default: T, minimum: T) -> Result<T, String>
where
    T: std::str::FromStr + PartialOrd + std::fmt::Display,
    T::Err: std::fmt::Display,
{
    debug_assert!(default >= minimum);
    match std::env::var(key) {
        Ok(raw) => parse_bounded_value(key, &raw, minimum),
        Err(std::env::VarError::NotPresent) => Ok(default),
        Err(std::env::VarError::NotUnicode(_)) => {
            Err(format!("{key} must contain a valid numeric value"))
        }
    }
}

fn parse_bounded_value<T>(key: &str, raw: &str, minimum: T) -> Result<T, String>
where
    T: std::str::FromStr + PartialOrd + std::fmt::Display,
    T::Err: std::fmt::Display,
{
    let value = raw
        .parse::<T>()
        .map_err(|error| format!("{key} has invalid value {raw:?}: {error}"))?;
    if value < minimum {
        return Err(format!(
            "{key} must be at least {minimum}; received {value}"
        ));
    }
    Ok(value)
}

fn unix_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn scheduler_task_key(task_type: &str, run_at_unix: i64, cadence_secs: u64) -> String {
    let cadence_bucket = run_at_unix / cadence_secs.max(1) as i64;
    format!("{task_type}:{cadence_bucket}")
}

#[cfg(test)]
mod tests {
    use super::{
        completion_notification_event, gameplay_process_payload, parse_bounded_value,
        process_claimed_task, process_task, scheduler_job_specs, scheduler_task_key,
        SchedulerCadences, SCHEDULER_JOB_TYPES,
    };
    use platform_db::{
        AccountCreateInput, Database, GameplayCompletion, GameplayCompletionKind,
        GameplayProcessResult, GameplayQueueInput, ScheduledTaskCreateInput,
    };

    fn disconnected_database() -> Database {
        Database::from_database_url(
            "postgres://scheduler:scheduler@127.0.0.1:1/scheduler?connect_timeout=1",
        )
        .expect("syntactically valid disconnected database")
    }

    fn completion(kind: GameplayCompletionKind) -> GameplayCompletion {
        GameplayCompletion {
            kind,
            queue_id: "queue-7".to_string(),
            user_id: "user-3".to_string(),
            planet_id: "planet-9".to_string(),
            item_type: "metal_mine".to_string(),
            target_level: Some(4),
            quantity: None,
            score_delta: 12,
        }
    }

    fn building_input(user_id: &str, planet_id: &str, target_level: i32) -> GameplayQueueInput {
        GameplayQueueInput {
            user_id: user_id.to_string(),
            planet_id: planet_id.to_string(),
            item_type: "metal_mine".to_string(),
            target_level: Some(target_level),
            quantity: None,
            metal_cost: 0,
            crystal_cost: 0,
            deuterium_cost: 0,
            energy_required: 0,
            duration_seconds: 1,
        }
    }

    #[test]
    fn scheduler_task_key_is_stable_for_same_bucket() {
        let first = scheduler_task_key("scheduler.game_loop", 1700000010, 5);
        let second = scheduler_task_key("scheduler.game_loop", 1700000014, 5);
        assert_eq!(first, second);
    }

    #[test]
    fn scheduler_task_key_changes_for_next_bucket() {
        let first = scheduler_task_key("scheduler.game_loop", 1700000014, 5);
        let second = scheduler_task_key("scheduler.game_loop", 1700000015, 5);
        assert_ne!(first, second);
    }

    #[test]
    fn run_once_job_order_matches_every_registered_scheduler_job() {
        let specs = scheduler_job_specs(SchedulerCadences {
            game_loop_secs: 1,
            fleet_secs: 2,
            moon_secs: 3,
            shard_health_secs: 4,
        });
        assert_eq!(
            specs.map(|spec| spec.task_type),
            SCHEDULER_JOB_TYPES,
            "run-once and recurring registration must share one deterministic job order"
        );
        assert_eq!(specs.map(|spec| spec.cadence_secs), [1, 2, 3, 4]);
    }

    #[test]
    fn scheduler_numeric_configuration_rejects_invalid_and_unsafe_bounds() {
        assert_eq!(
            parse_bounded_value::<u64>("GAME_LOOP_INTERVAL_SECS", "1", 1).unwrap(),
            1
        );
        assert!(
            parse_bounded_value::<u64>("GAME_LOOP_INTERVAL_SECS", "0", 1)
                .unwrap_err()
                .contains("must be at least 1")
        );
        assert!(parse_bounded_value::<i64>("SCHED_TASK_LEASE_SECS", "4", 5)
            .unwrap_err()
            .contains("must be at least 5"));
        assert!(
            parse_bounded_value::<usize>("SCHED_RUNTIME_MAX_INFLIGHT", "invalid", 1)
                .unwrap_err()
                .contains("has invalid value")
        );
    }

    #[test]
    fn gameplay_payload_preserves_exact_counts_and_completion_facts() {
        let completion = completion(GameplayCompletionKind::Building);
        let result = GameplayProcessResult {
            buildings: 1,
            research: 0,
            ships: 0,
            failed: 4,
            completions: vec![completion],
        };
        let payload = gameplay_process_payload(&result, &serde_json::json!({"cadenceBucket": 11}));

        assert_eq!(payload["kind"], "gameplay_queue_tick");
        assert_eq!(payload["counts"]["buildings"], 1);
        assert_eq!(payload["counts"]["research"], 0);
        assert_eq!(payload["counts"]["ships"], 0);
        assert_eq!(payload["counts"]["failed"], 4);
        assert_eq!(payload["counts"]["completed"], 1);
        assert_eq!(payload["completions"][0]["kind"], "building");
        assert_eq!(payload["completions"][0]["queueId"], "queue-7");
        assert_eq!(payload["completions"][0]["userId"], "user-3");
        assert_eq!(payload["completions"][0]["planetId"], "planet-9");
        assert_eq!(payload["completions"][0]["targetLevel"], 4);
        assert_eq!(payload["completions"][0]["scoreDelta"], 12);
        assert_eq!(payload["scheduled"]["cadenceBucket"], 11);
    }

    #[test]
    fn gameplay_completion_notification_uses_canonical_user_contract() {
        let completion = completion(GameplayCompletionKind::Research);
        let channel = platform_events::user_notification_channel(&completion.user_id);
        let event = completion_notification_event(&completion);

        assert_eq!(channel, "notifications:user-3");
        assert_eq!(event.event_type, "notification.created");
        assert_eq!(event.payload["category"], "gameplay");
        assert_eq!(event.payload["title"], "Research completed");
        assert_eq!(event.payload["idempotencyKey"], "gameplay:research:queue-7");
        assert_eq!(event.payload["completion"]["queueId"], "queue-7");
    }

    #[tokio::test]
    async fn authoritative_fleet_and_moon_processing_propagate_repository_failure() {
        let database = disconnected_database();
        for task_type in ["scheduler.fleet", "scheduler.moon_destroy"] {
            let error = process_task(
                &database,
                task_type,
                &serde_json::json!({"tick": 1}),
                "scheduler-unit-worker",
                30,
            )
            .await
            .expect_err("unreachable repository must fail processing");
            assert!(
                error.contains("authoritative fleet processing failed"),
                "{task_type}: {error}"
            );
        }
    }

    #[tokio::test]
    async fn authoritative_game_loop_propagates_repository_failure() {
        let error = process_task(
            &disconnected_database(),
            "scheduler.game_loop",
            &serde_json::json!({}),
            "scheduler-unit-worker",
            30,
        )
        .await
        .expect_err("unreachable repository must fail processing");
        assert!(error.contains("authoritative gameplay processing failed"));
    }

    /// Requires a pre-migrated disposable PostgreSQL database. The test owns
    /// all data it creates but intentionally does not mutate shared schemas.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "requires pre-migrated disposable PostgreSQL in UNIVERSUS_TEST_DATABASE_URL"]
    async fn scheduler_completes_after_effects_and_retries_failures_exactly_once() {
        let database_url = std::env::var("UNIVERSUS_TEST_DATABASE_URL")
            .expect("UNIVERSUS_TEST_DATABASE_URL must name a disposable migrated database");
        let database = Database::from_database_url(&database_url).expect("scheduler test pool");
        database
            .gameplay_repository_ready()
            .await
            .expect("authoritative gameplay repository");
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let account = database
            .register_account_with_starting_state(AccountCreateInput {
                username: format!("Scheduler{nonce}"),
                email: format!("scheduler-{nonce}@example.com"),
                password_hash: "$argon2id$v=19$m=19456,t=2,p=1$c2FsdA$aGFzaA".to_string(),
            })
            .await
            .expect("scheduler test account");
        let planet = database
            .gameplay_planets_for_user(&account.id)
            .await
            .expect("scheduler test planet")
            .remove(0);
        database
            .gameplay_enqueue_building(&building_input(&account.id, &planet.id, 1))
            .await
            .expect("due building queue");
        tokio::time::sleep(std::time::Duration::from_millis(1_200)).await;

        let success_key = format!("scheduler-pg-success-{nonce}");
        let scheduled = database
            .enqueue_scheduled_task(ScheduledTaskCreateInput {
                task_type: "scheduler.game_loop".to_string(),
                payload: serde_json::json!({"test": "effect-before-completion"}),
                run_at_unix: super::unix_timestamp(),
                task_key: Some(success_key.clone()),
            })
            .await
            .expect("scheduled game-loop task");
        let task = database
            .claim_due_scheduled_tasks("scheduler-pg-worker", 16, 5)
            .await
            .expect("claim scheduled game-loop task")
            .into_iter()
            .find(|task| task.id == scheduled.id)
            .expect("claimed exact scheduled game-loop task");
        let result = process_claimed_task(&database, &task, "scheduler-pg-worker", None, 30, 1, 3)
            .await
            .expect("authoritative scheduler completion");
        assert_eq!(result["counts"]["buildings"], 1);
        assert_eq!(result["counts"]["completed"], 1);
        assert_eq!(result["completions"][0]["targetLevel"], 1);
        assert_eq!(
            database
                .gameplay_planet_for_user(&account.id, &planet.id)
                .await
                .unwrap()
                .unwrap()
                .buildings["metal_mine"],
            1,
            "scheduled task may complete only after the authoritative effect"
        );
        assert!(database
            .claim_due_scheduled_tasks("scheduler-pg-reclaim", 16, 5)
            .await
            .unwrap()
            .into_iter()
            .all(|task| task.id != scheduled.id));
        let persisted = database
            .enqueue_scheduled_task(ScheduledTaskCreateInput {
                task_type: "scheduler.game_loop".to_string(),
                payload: serde_json::json!({"test": "effect-before-completion"}),
                run_at_unix: super::unix_timestamp(),
                task_key: Some(success_key),
            })
            .await
            .expect("read completed task through idempotent enqueue");
        assert_eq!(persisted.id, scheduled.id);
        assert_eq!(persisted.status, "completed");

        database
            .gameplay_enqueue_building(&building_input(&account.id, &planet.id, 2))
            .await
            .expect("second due building queue");
        tokio::time::sleep(std::time::Duration::from_millis(1_200)).await;
        let worker_a_payload = serde_json::json!({"worker": "a"});
        let concurrent_a = process_task(
            &database,
            "scheduler.game_loop",
            &worker_a_payload,
            "scheduler-pg-worker-a",
            30,
        );
        let restarted = Database::from_database_url(&database_url).expect("restarted pool");
        let worker_b_payload = serde_json::json!({"worker": "b"});
        let concurrent_b = process_task(
            &restarted,
            "scheduler.game_loop",
            &worker_b_payload,
            "scheduler-pg-worker-b",
            30,
        );
        let (first, second) = tokio::join!(concurrent_a, concurrent_b);
        let first = first.expect("first concurrent processing pass");
        let second = second.expect("second concurrent processing pass");
        assert_eq!(
            first.result["counts"]["buildings"].as_u64().unwrap()
                + second.result["counts"]["buildings"].as_u64().unwrap(),
            1,
            "FOR UPDATE SKIP LOCKED must apply a due row exactly once"
        );
        let second_restart =
            Database::from_database_url(&database_url).expect("second restarted pool");
        let restart_payload = serde_json::json!({"worker": "restart"});
        let replay = process_task(
            &second_restart,
            "scheduler.game_loop",
            &restart_payload,
            "scheduler-pg-worker-restart",
            30,
        )
        .await
        .expect("restart replay");
        assert_eq!(replay.result["counts"]["completed"], 0);
        assert!(replay.completions.is_empty());

        let unsupported = database
            .enqueue_scheduled_task(ScheduledTaskCreateInput {
                task_type: "scheduler.unsupported".to_string(),
                payload: serde_json::json!({}),
                run_at_unix: super::unix_timestamp(),
                task_key: Some(format!("scheduler-pg-failure-{nonce}")),
            })
            .await
            .expect("unsupported scheduled task");
        let failed = database
            .claim_due_scheduled_tasks("scheduler-pg-failure", 16, 5)
            .await
            .expect("claim unsupported task")
            .into_iter()
            .find(|task| task.id == unsupported.id)
            .expect("claimed unsupported task");
        let error =
            process_claimed_task(&database, &failed, "scheduler-pg-failure", None, 30, 1, 3)
                .await
                .expect_err("processing failure must propagate");
        assert!(error.contains("unsupported task type"));
        tokio::time::sleep(std::time::Duration::from_millis(1_100)).await;
        let retry = database
            .claim_due_scheduled_tasks("scheduler-pg-retry", 16, 5)
            .await
            .expect("claim retry task")
            .into_iter()
            .find(|task| task.id == unsupported.id)
            .expect("failed task must become retryable");
        assert_eq!(retry.attempt_count, 1);
        assert_eq!(retry.status, "running");
    }
}
