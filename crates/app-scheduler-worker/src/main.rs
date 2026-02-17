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
        "scheduler worker started"
    );

    let mut game_tick = tokio::time::interval(Duration::from_secs(game_loop_secs));
    let mut fleet_tick = tokio::time::interval(Duration::from_secs(fleet_secs));
    let mut moon_tick = tokio::time::interval(Duration::from_secs(moon_secs));
    let mut shard_tick = tokio::time::interval(Duration::from_secs(shard_health_secs));

    loop {
        tokio::select! {
            _ = game_tick.tick() => {
                enqueue_and_process_tick(
                    "scheduler.game_loop",
                    game_loop_secs,
                    realtime_url.as_deref(),
                    &worker_id,
                    lease_secs,
                    retry_delay_secs,
                    max_attempts,
                ).await;
            }
            _ = fleet_tick.tick() => {
                enqueue_and_process_tick(
                    "scheduler.fleet",
                    fleet_secs,
                    realtime_url.as_deref(),
                    &worker_id,
                    lease_secs,
                    retry_delay_secs,
                    max_attempts,
                ).await;
            }
            _ = moon_tick.tick() => {
                enqueue_and_process_tick(
                    "scheduler.moon_destroy",
                    moon_secs,
                    realtime_url.as_deref(),
                    &worker_id,
                    lease_secs,
                    retry_delay_secs,
                    max_attempts,
                ).await;
            }
            _ = shard_tick.tick() => {
                enqueue_and_process_tick(
                    "scheduler.shard_health",
                    shard_health_secs,
                    realtime_url.as_deref(),
                    &worker_id,
                    lease_secs,
                    retry_delay_secs,
                    max_attempts,
                ).await;
            }
        }

        if run_once {
            break;
        }
    }

    // flush logs in short-lived run_once mode
    sleep(Duration::from_millis(25)).await;
}

async fn enqueue_and_process_tick(
    task_type: &str,
    cadence_secs: u64,
    realtime_url: Option<&str>,
    worker_id: &str,
    lease_secs: i64,
    retry_delay_secs: i64,
    max_attempts: i32,
) {
    tracing::info!(service = SERVICE_NAME, task_type, "tick start");
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
        let event = platform_events::build_event("scheduler.claim", &serde_json::json!({
            "workerId": worker_id,
            "count": claimed.len()
        }));
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
