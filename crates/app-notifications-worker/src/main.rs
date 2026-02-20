use platform_tenancy::{TenantAccessLevel, TenantContext};
use platform_worker_runtime::WorkerRuntime;
use tokio::sync::oneshot;
use tokio::time::{sleep, Duration};

const SERVICE_NAME: &str = "app-notifications-worker";

#[tokio::main]
async fn main() {
    platform_observability::init(SERVICE_NAME);

    let interval_secs = std::env::var("NOTIFICATION_CLEANUP_INTERVAL_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(3600);
    let archive_days = std::env::var("NOTIFICATION_ARCHIVE_RETENTION_DAYS")
        .ok()
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or(30);
    let run_once = std::env::var("NOTIFICATION_CLEANUP_RUN_ONCE")
        .map(|value| value.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let max_inflight = std::env::var("NOTIFICATION_WORKER_MAX_INFLIGHT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(8);
    let tenant_context = tenant_context_from_env();
    let runtime = WorkerRuntime::current(max_inflight);

    tracing::info!(
        service = SERVICE_NAME,
        interval_secs,
        archive_days,
        run_once,
        max_inflight,
        tenant_id = %tenant_context.tenant_id,
        "notifications cleanup worker started"
    );

    loop {
        let context = tenant_context.clone();
        let (done_tx, done_rx) = oneshot::channel::<()>();
        match runtime.spawn_tenant_task(context, async move {
            run_cleanup_cycle(archive_days).await;
            let _ = done_tx.send(());
            Ok(())
        }) {
            Ok(()) => {
                if done_rx.await.is_err() {
                    tracing::warn!(
                        service = SERVICE_NAME,
                        "cleanup cycle task ended before completion signal"
                    );
                }
                let stats = runtime.stats().await;
                tracing::info!(
                    service = SERVICE_NAME,
                    total_inflight = stats.total_inflight,
                    tenant_count = stats.per_tenant.len(),
                    "notifications runtime stats"
                );
            }
            Err(error) => {
                tracing::warn!(
                    service = SERVICE_NAME,
                    %error,
                    "failed to schedule notification cleanup cycle"
                );
            }
        }
        if run_once {
            break;
        }
        sleep(Duration::from_secs(interval_secs)).await;
    }

    runtime.shutdown(Duration::from_secs(5)).await;
}

async fn run_cleanup_cycle(archive_days: i32) {
    let Some(database) = platform_db::Database::from_env() else {
        tracing::warn!(
            service = SERVICE_NAME,
            "DATABASE_URL not configured; skipping notification cleanup cycle"
        );
        return;
    };

    let expired_removed = database
        .cleanup_expired_notifications()
        .await
        .unwrap_or_else(|error| {
            tracing::error!(service = SERVICE_NAME, %error, "failed expired notifications cleanup");
            0
        });

    let archived_removed = database
        .cleanup_archived_notifications_older_than_days(archive_days)
        .await
        .unwrap_or_else(|error| {
            tracing::error!(service = SERVICE_NAME, %error, "failed archived notifications cleanup");
            0
        });

    tracing::info!(
        service = SERVICE_NAME,
        expired_removed,
        archived_removed,
        archive_days,
        "notifications cleanup cycle completed"
    );

    publish_ops_event(
        "notifications.cleanup.completed",
        &serde_json::json!({
            "expiredRemoved": expired_removed,
            "archivedRemoved": archived_removed,
            "archiveDays": archive_days
        }),
    )
    .await;
}

fn tenant_context_from_env() -> TenantContext {
    let tenant_id = std::env::var("NOTIFICATION_WORKER_TENANT_ID")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "tenant-default".to_string());
    let tenant_name = std::env::var("NOTIFICATION_WORKER_TENANT_NAME")
        .ok()
        .filter(|value| !value.trim().is_empty());

    TenantContext {
        tenant_id,
        tenant_name,
        access_level: TenantAccessLevel::Worker,
    }
}

async fn publish_ops_event(event_type: &str, payload: &serde_json::Value) {
    let Some(base_url) = std::env::var("REALTIME_GATEWAY_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
    else {
        return;
    };
    let event = platform_events::build_event(event_type, payload);
    let _ = platform_events::publish_http(&base_url, "ops.notifications", &event).await;
}
