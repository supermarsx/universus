//! Notifications cleanup worker.
//!
//! Periodically runs cleanup cycles that:
//! - Remove expired notifications
//! - Archive old notifications past retention period
//! - Publish operational events on cleanup completion
//!
//! Configuration via environment variables:
//! - `NOTIFICATION_CLEANUP_INTERVAL_SECS` — interval between cleanup cycles (default: 3600)
//! - `NOTIFICATION_ARCHIVE_RETENTION_DAYS` — days before archived notifications are purged (default: 30)
//! - `NOTIFICATION_BATCH_SIZE` — max notifications to process per cycle (default: 1000)
//! - `NOTIFICATION_CLEANUP_RUN_ONCE` — run one cycle then exit (default: false)
//! - `NOTIFICATION_WORKER_MAX_INFLIGHT` — max concurrent tasks (default: 8)
//! - `NOTIFICATION_WORKER_TENANT_ID` — tenant ID for worker context
//! - `NOTIFICATION_WORKER_TENANT_NAME` — tenant display name
//! - `REALTIME_GATEWAY_URL` — URL for publishing operational events

use platform_tenancy::{TenantAccessLevel, TenantContext};
use platform_worker_runtime::WorkerRuntime;
use tokio::sync::oneshot;
use tokio::time::{sleep, Duration};

const SERVICE_NAME: &str = "app-notifications-worker";

/// Worker configuration parsed from environment.
struct WorkerConfig {
    interval_secs: u64,
    archive_days: i32,
    batch_size: i64,
    run_once: bool,
    max_inflight: usize,
    tenant_context: TenantContext,
    realtime_url: Option<String>,
}

impl WorkerConfig {
    fn from_env() -> Self {
        Self {
            interval_secs: parse_env("NOTIFICATION_CLEANUP_INTERVAL_SECS", 3600),
            archive_days: parse_env("NOTIFICATION_ARCHIVE_RETENTION_DAYS", 30),
            batch_size: parse_env("NOTIFICATION_BATCH_SIZE", 1000),
            run_once: std::env::var("NOTIFICATION_CLEANUP_RUN_ONCE")
                .map(|v| v.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
            max_inflight: parse_env("NOTIFICATION_WORKER_MAX_INFLIGHT", 8),
            realtime_url: std::env::var("REALTIME_GATEWAY_URL")
                .ok()
                .filter(|v| !v.trim().is_empty()),
            tenant_context: tenant_context_from_env(),
        }
    }
}

/// Cleanup cycle result for reporting and events.
struct CleanupResult {
    expired_removed: i64,
    archived_removed: i64,
}

#[tokio::main]
async fn main() {
    platform_observability::init(SERVICE_NAME);

    let config = WorkerConfig::from_env();
    let runtime = WorkerRuntime::current(config.max_inflight);

    tracing::info!(
        service = SERVICE_NAME,
        interval_secs = config.interval_secs,
        archive_days = config.archive_days,
        batch_size = config.batch_size,
        run_once = config.run_once,
        max_inflight = config.max_inflight,
        tenant_id = %config.tenant_context.tenant_id,
        has_realtime_url = config.realtime_url.is_some(),
        "notifications cleanup worker started"
    );

    let mut cycle_count: u64 = 0;
    loop {
        cycle_count += 1;
        let context = config.tenant_context.clone();
        let (done_tx, done_rx) = oneshot::channel::<CleanupResult>();
        let archive_days = config.archive_days;
        let batch_size = config.batch_size;
        let cycle = cycle_count;

        match runtime.spawn_tenant_task(context, async move {
            let result = run_cleanup_cycle(archive_days, batch_size, cycle).await;
            let _ = done_tx.send(result);
            Ok(())
        }) {
            Ok(_job_id) => {
                match done_rx.await {
                    Ok(result) => {
                        tracing::info!(
                            service = SERVICE_NAME,
                            cycle = cycle_count,
                            expired_removed = result.expired_removed,
                            archived_removed = result.archived_removed,
                            "cleanup cycle completed"
                        );
                        if let Some(url) = &config.realtime_url {
                            publish_ops_event(
                                url,
                                "notifications.cleanup.completed",
                                &serde_json::json!({
                                    "cycle": cycle_count,
                                    "expiredRemoved": result.expired_removed,
                                    "archivedRemoved": result.archived_removed,
                                    "archiveDays": archive_days
                                }),
                            )
                            .await;
                        }
                    }
                    Err(_) => {
                        tracing::warn!(
                            service = SERVICE_NAME,
                            cycle = cycle_count,
                            "cleanup cycle task ended before completion signal"
                        );
                    }
                }
                let stats = runtime.stats().await;
                tracing::info!(
                    service = SERVICE_NAME,
                    total_inflight = stats.total_inflight,
                    total_completed = stats.total_completed,
                    total_failed = stats.total_failed,
                    tenant_count = stats.per_tenant.len(),
                    "notifications runtime stats"
                );
            }
            Err(error) => {
                tracing::warn!(
                    service = SERVICE_NAME,
                    %error,
                    cycle = cycle_count,
                    "failed to schedule notification cleanup cycle"
                );
            }
        }
        if config.run_once {
            break;
        }
        sleep(Duration::from_secs(config.interval_secs)).await;
    }

    runtime.shutdown(Duration::from_secs(5)).await;
    tracing::info!(
        service = SERVICE_NAME,
        total_cycles = cycle_count,
        "worker shutdown complete"
    );
}

async fn run_cleanup_cycle(archive_days: i32, _batch_size: i64, cycle: u64) -> CleanupResult {
    let Some(database) = platform_db::Database::from_env() else {
        tracing::warn!(
            service = SERVICE_NAME,
            cycle,
            "DATABASE_URL not configured; skipping notification cleanup cycle"
        );
        return CleanupResult {
            expired_removed: 0,
            archived_removed: 0,
        };
    };

    let expired_removed = database
        .cleanup_expired_notifications()
        .await
        .unwrap_or_else(|error| {
            tracing::error!(service = SERVICE_NAME, cycle, %error, "failed expired notifications cleanup");
            0
        });

    let archived_removed = database
        .cleanup_archived_notifications_older_than_days(archive_days)
        .await
        .unwrap_or_else(|error| {
            tracing::error!(service = SERVICE_NAME, cycle, %error, "failed archived notifications cleanup");
            0
        });

    CleanupResult {
        expired_removed,
        archived_removed,
    }
}

fn tenant_context_from_env() -> TenantContext {
    let tenant_id = std::env::var("NOTIFICATION_WORKER_TENANT_ID")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "tenant-default".to_string());
    let tenant_name = std::env::var("NOTIFICATION_WORKER_TENANT_NAME")
        .ok()
        .filter(|v| !v.trim().is_empty());

    TenantContext {
        tenant_id,
        tenant_name,
        access_level: TenantAccessLevel::Worker,
    }
}

fn parse_env<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<T>().ok())
        .unwrap_or(default)
}

async fn publish_ops_event(base_url: &str, event_type: &str, payload: &serde_json::Value) {
    let event = platform_events::build_event(event_type, payload);
    if let Err(error) = platform_events::publish_http(base_url, "ops.notifications", &event).await {
        tracing::warn!(
            service = SERVICE_NAME,
            event_type,
            %error,
            "failed to publish ops event"
        );
    }
}
