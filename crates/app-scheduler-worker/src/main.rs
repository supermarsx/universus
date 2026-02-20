use platform_consensus::LeaseCoordinator;
use platform_scheduler::{JobConfig, JobHandler, Scheduler};
use platform_sharding::{ShardConfig, ShardingCatalog};
use platform_tenant_routing::{TenantRouteConfig, TenantRouter, TenantRoutingDecision};
use platform_worker_runtime::WorkerRuntime;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::oneshot;
use tokio::time::{sleep, Duration};

const SERVICE_NAME: &str = "app-scheduler-worker";

#[tokio::main]
async fn main() {
    platform_observability::init(SERVICE_NAME);

    let run_once = bool_env("SCHEDULER_RUN_ONCE");
    let game_loop_secs = u64_env("GAME_LOOP_INTERVAL_SECS", 5);
    let fleet_secs = u64_env("FLEET_SCHEDULER_INTERVAL_SECS", 10);
    let moon_secs = u64_env("MOON_DESTROY_INTERVAL_SECS", 10);
    let shard_health_secs = u64_env("SHARD_HEALTH_INTERVAL_SECS", 60);
    let realtime_url = std::env::var("REALTIME_GATEWAY_URL")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let worker_id = std::env::var("SCHEDULER_WORKER_ID")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "app-scheduler-worker".to_string());
    let lease_secs = i64_env("SCHED_TASK_LEASE_SECS", 30);
    let retry_delay_secs = i64_env("SCHED_TASK_RETRY_DELAY_SECS", 15);
    let max_attempts = i32_env("SCHED_TASK_MAX_ATTEMPTS", 3);
    let runtime_max_inflight = std::env::var("SCHED_RUNTIME_MAX_INFLIGHT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(32);
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
        tenant_id.clone(),
        shard_id.clone(),
        queue_name.clone(),
        region.clone(),
        worker_id.clone(),
        lease_secs,
        retry_delay_secs,
        max_attempts,
        realtime_url.clone(),
        game_loop_secs,
        fleet_secs,
        moon_secs,
        shard_health_secs,
    )
    .await
    {
        Ok(scheduler) => scheduler,
        Err(error) => {
            tracing::error!(service = SERVICE_NAME, %error, "failed to initialize platform scheduler");
            return;
        }
    };

    let mut game_tick = tokio::time::interval(Duration::from_secs(game_loop_secs));
    let mut fleet_tick = tokio::time::interval(Duration::from_secs(fleet_secs));
    let mut moon_tick = tokio::time::interval(Duration::from_secs(moon_secs));
    let mut shard_tick = tokio::time::interval(Duration::from_secs(shard_health_secs));

    loop {
        tokio::select! {
            _ = game_tick.tick() => {
                trigger_platform_job(&scheduler, "scheduler.game_loop").await;
                platform_observability::emit_consensus_snapshot(
                    SERVICE_NAME,
                    lease_coordinator.as_ref(),
                    4,
                )
                .await;
            }
            _ = fleet_tick.tick() => {
                trigger_platform_job(&scheduler, "scheduler.fleet").await;
                platform_observability::emit_consensus_snapshot(
                    SERVICE_NAME,
                    lease_coordinator.as_ref(),
                    4,
                )
                .await;
            }
            _ = moon_tick.tick() => {
                trigger_platform_job(&scheduler, "scheduler.moon_destroy").await;
                platform_observability::emit_consensus_snapshot(
                    SERVICE_NAME,
                    lease_coordinator.as_ref(),
                    4,
                )
                .await;
            }
            _ = shard_tick.tick() => {
                trigger_platform_job(&scheduler, "scheduler.shard_health").await;
                platform_observability::emit_consensus_snapshot(
                    SERVICE_NAME,
                    lease_coordinator.as_ref(),
                    4,
                )
                .await;
            }
        }

        if run_once {
            break;
        }
    }

    // flush logs in short-lived run_once mode
    runtime.shutdown(Duration::from_secs(5)).await;
    sleep(Duration::from_millis(25)).await;
}

async fn trigger_platform_job(scheduler: &Scheduler, job_id: &str) {
    if let Err(error) = scheduler.trigger_job(job_id).await {
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
    tenant_id: String,
    shard_id: String,
    queue_name: String,
    region: String,
    worker_id: String,
    lease_secs: i64,
    retry_delay_secs: i64,
    max_attempts: i32,
    realtime_url: Option<String>,
    game_loop_secs: u64,
    fleet_secs: u64,
    moon_secs: u64,
    shard_health_secs: u64,
) -> Result<Arc<Scheduler>, String> {
    let tenant_router = Arc::new(TenantRouter::with_leases(Arc::clone(&lease_coordinator)));
    let catalog = Arc::new(ShardingCatalog::new());
    catalog
        .register_shard(ShardConfig {
            shard_id: shard_id.clone(),
            region: region.clone(),
            allowed_tenants: HashSet::from_iter(vec![tenant_id.clone()]),
            consensus_resource: Some(format!("scheduler-shard:{shard_id}")),
        })
        .await;
    tenant_router
        .register(TenantRouteConfig {
            tenant_id: tenant_id.clone(),
            shard_id: shard_id.clone(),
            queue_name,
            region,
            max_inflight: 8,
            max_per_second: 0,
            consensus_resource: Some(format!("scheduler-tenant:{tenant_id}")),
            lease_ttl: Duration::from_secs(lease_secs.max(1) as u64),
        })
        .await;

    let scheduler = Arc::new(Scheduler::new(tenant_router, catalog));
    register_scheduler_job(
        Arc::clone(&scheduler),
        Arc::clone(&runtime),
        "scheduler.game_loop",
        &tenant_id,
        &shard_id,
        game_loop_secs,
        worker_id.clone(),
        realtime_url.clone(),
        lease_secs,
        retry_delay_secs,
        max_attempts,
    )
    .await?;
    register_scheduler_job(
        Arc::clone(&scheduler),
        Arc::clone(&runtime),
        "scheduler.fleet",
        &tenant_id,
        &shard_id,
        fleet_secs,
        worker_id.clone(),
        realtime_url.clone(),
        lease_secs,
        retry_delay_secs,
        max_attempts,
    )
    .await?;
    register_scheduler_job(
        Arc::clone(&scheduler),
        Arc::clone(&runtime),
        "scheduler.moon_destroy",
        &tenant_id,
        &shard_id,
        moon_secs,
        worker_id.clone(),
        realtime_url.clone(),
        lease_secs,
        retry_delay_secs,
        max_attempts,
    )
    .await?;
    register_scheduler_job(
        Arc::clone(&scheduler),
        Arc::clone(&runtime),
        "scheduler.shard_health",
        &tenant_id,
        &shard_id,
        shard_health_secs,
        worker_id,
        realtime_url,
        lease_secs,
        retry_delay_secs,
        max_attempts,
    )
    .await?;

    Ok(scheduler)
}

async fn register_scheduler_job(
    scheduler: Arc<Scheduler>,
    runtime: Arc<WorkerRuntime>,
    task_type: &str,
    tenant_id: &str,
    shard_id: &str,
    cadence_secs: u64,
    worker_id: String,
    realtime_url: Option<String>,
    lease_secs: i64,
    retry_delay_secs: i64,
    max_attempts: i32,
    ) -> Result<(), String> {
    let task_type_owned = task_type.to_string();
    let task_type_for_handler = task_type_owned.clone();
    let handler: JobHandler = Arc::new(move |decision: TenantRoutingDecision| {
        let task_type = task_type_for_handler.clone();
        let worker_id = worker_id.clone();
        let realtime_url = realtime_url.clone();
        let runtime = Arc::clone(&runtime);
        Box::pin(async move {
            let tenant_id = decision.tenant_id().to_string();
            let shard_id = decision.route.shard_id.clone();
            let queue_name = decision.route.queue_name.clone();
            let context = decision.guard.context().clone();
            let (done_tx, done_rx) = oneshot::channel::<()>();
            runtime.spawn_tenant_task(context, async move {
                enqueue_and_process_tick(
                    &task_type,
                    cadence_secs,
                    realtime_url.as_deref(),
                    &worker_id,
                    lease_secs,
                    retry_delay_secs,
                    max_attempts,
                    &tenant_id,
                    &shard_id,
                    &queue_name,
                )
                .await;
                let _ = done_tx.send(());
                Ok(())
            }).map_err(|error| anyhow::anyhow!("runtime schedule failure: {error}"))?;
            let _ = done_rx.await;
            Ok(())
        })
    });
    let config = JobConfig {
        job_id: task_type_owned.clone(),
        tenant_id: tenant_id.to_string(),
        description: format!("{task_type_owned} scheduler job"),
        shard_id: shard_id.to_string(),
        interval: Duration::from_secs(cadence_secs.max(1)),
    };
    scheduler
        .register_job(config, handler)
        .await
        .map_err(|error| format!("register {task_type_owned}: {error}"))
}

async fn enqueue_and_process_tick(
    task_type: &str,
    cadence_secs: u64,
    realtime_url: Option<&str>,
    worker_id: &str,
    lease_secs: i64,
    retry_delay_secs: i64,
    max_attempts: i32,
    tenant_id: &str,
    shard_id: &str,
    queue_name: &str,
) {
    tracing::info!(service = SERVICE_NAME, task_type, "tick start");
    tracing::debug!(
        service = SERVICE_NAME,
        task_type,
        tenant_id,
        shard_id,
        queue_name,
        "platform scheduler decision acquired"
    );

    let Some(database) = platform_db::Database::from_env() else {
        tracing::warn!(
            service = SERVICE_NAME,
            task_type,
            "DATABASE_URL not configured; skipping enqueue/process cycle"
        );
        return;
    };

    let run_at_unix = unix_timestamp();
    let task_key = scheduler_task_key(task_type, run_at_unix, cadence_secs);
    let enqueue_result = database
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
        .await;
    if let Err(error) = enqueue_result {
        tracing::error!(service = SERVICE_NAME, task_type, %error, "failed enqueue scheduled task");
        return;
    }

    let claimed = match database
        .claim_due_scheduled_tasks(worker_id, 16, lease_secs)
        .await
    {
        Ok(tasks) => tasks,
        Err(error) => {
            tracing::error!(service = SERVICE_NAME, task_type, %error, "failed claim due tasks");
            return;
        }
    };

    if claimed.is_empty() {
        return;
    }

    if let Some(url) = realtime_url {
        let event = platform_events::build_event(
            "scheduler.claim",
            &serde_json::json!({
                "workerId": worker_id,
                "count": claimed.len()
            }),
        );
        let _ = platform_events::publish_http(url, "ops.scheduler", &event).await;
    }

    for task in claimed {
        let process_result = process_task(&task.task_type, &task.payload).await;
        match process_result {
            Ok(result_payload) => {
                let _ = database.complete_scheduled_task(task.id).await;
                tracing::info!(
                    service = SERVICE_NAME,
                    task_id = task.id,
                    task_type = %task.task_type,
                    "scheduled task completed"
                );
                if let Some(url) = realtime_url {
                    let event = platform_events::build_event(
                        "scheduler.task_completed",
                        &serde_json::json!({
                            "taskId": task.id,
                            "taskType": task.task_type,
                            "result": result_payload
                        }),
                    );
                    let _ = platform_events::publish_http(url, "ops.scheduler", &event).await;
                }
            }
            Err(error_message) => {
                let _ = database
                    .fail_scheduled_task(task.id, &error_message, retry_delay_secs, max_attempts)
                    .await;
                tracing::warn!(
                    service = SERVICE_NAME,
                    task_id = task.id,
                    task_type = %task.task_type,
                    error = %error_message,
                    "scheduled task failed"
                );
                if let Some(url) = realtime_url {
                    let event = platform_events::build_event(
                        "scheduler.task_failed",
                        &serde_json::json!({
                            "taskId": task.id,
                            "taskType": task.task_type,
                            "error": error_message
                        }),
                    );
                    let _ = platform_events::publish_http(url, "ops.scheduler", &event).await;
                }
            }
        }
    }
}

async fn process_task(
    task_type: &str,
    payload: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    match task_type {
        "scheduler.game_loop" => Ok(serde_json::json!({
            "kind": "game_loop_tick",
            "applied": true,
            "payload": payload
        })),
        "scheduler.fleet" => Ok(serde_json::json!({
            "kind": "fleet_tick",
            "applied": true,
            "payload": payload
        })),
        "scheduler.moon_destroy" => Ok(serde_json::json!({
            "kind": "moon_destroy_tick",
            "applied": true,
            "payload": payload
        })),
        "scheduler.shard_health" => Ok(serde_json::json!({
            "kind": "shard_health_tick",
            "applied": true,
            "payload": payload
        })),
        other => Err(format!("unsupported task type: {other}")),
    }
}

fn bool_env(key: &str) -> bool {
    std::env::var(key)
        .map(|value| value.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn u64_env(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default)
}

fn i64_env(key: &str, default: i64) -> i64 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(default)
}

fn i32_env(key: &str, default: i32) -> i32 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or(default)
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
    use super::scheduler_task_key;

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
}
