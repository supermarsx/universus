use std::time::{SystemTime, UNIX_EPOCH};

use platform_tenancy::{TenantAccessLevel, TenantContext};
use platform_worker_runtime::WorkerRuntime;
use tokio::sync::oneshot;
use platform_db::Database;
use tokio::time::{sleep, Duration};

const SERVICE_NAME: &str = "app-chat-worker";

#[tokio::main]
async fn main() {
    platform_observability::init(SERVICE_NAME);

    let interval_secs = std::env::var("CHAT_CLEANUP_INTERVAL_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(3600);
    let run_once = std::env::var("CHAT_CLEANUP_RUN_ONCE")
        .map(|value| value.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let max_inflight = std::env::var("CHAT_WORKER_MAX_INFLIGHT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(8);
    let realtime_url = std::env::var("REALTIME_GATEWAY_URL")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let tenant_context = tenant_context_from_env();
    let runtime = WorkerRuntime::current(max_inflight);

    tracing::info!(
        service = SERVICE_NAME,
        interval_secs,
        run_once,
        max_inflight,
        tenant_id = %tenant_context.tenant_id,
        has_realtime_url = realtime_url.is_some(),
        "chat cleanup worker started"
    );

    loop {
        let context = tenant_context.clone();
        let realtime_owned = realtime_url.clone();
        let (done_tx, done_rx) = oneshot::channel::<()>();
        match runtime.spawn_tenant_task(context, async move {
            let now_unix = now_unix();
            let removed = cleanup_expired_restrictions(now_unix).await;
            tracing::info!(
                service = SERVICE_NAME,
                now_unix,
                removed,
                "chat restriction cleanup cycle completed"
            );
            if removed > 0 {
                if let Some(url) = realtime_owned.as_deref() {
                    publish_ops_event(
                        url,
                        "chat.restrictions_cleanup",
                        &serde_json::json!({
                            "removed": removed,
                            "nowUnix": now_unix
                        }),
                    )
                    .await;
                }
            }
            let _ = done_tx.send(());
            Ok(())
        }) {
            Ok(()) => {
                if done_rx.await.is_err() {
                    tracing::warn!(
                        service = SERVICE_NAME,
                        "chat cleanup task ended before completion signal"
                    );
                }
                let stats = runtime.stats().await;
                tracing::info!(
                    service = SERVICE_NAME,
                    total_inflight = stats.total_inflight,
                    tenant_count = stats.per_tenant.len(),
                    "chat worker runtime stats"
                );
            }
            Err(error) => {
                tracing::warn!(
                    service = SERVICE_NAME,
                    %error,
                    "failed to schedule chat cleanup cycle"
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

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs() as i64)
        .unwrap_or(0)
}

async fn cleanup_expired_restrictions(now_unix: i64) -> i64 {
    let Some(database) = Database::from_env() else {
        tracing::warn!(
            service = SERVICE_NAME,
            "DATABASE_URL not configured; skipping chat restriction cleanup"
        );
        return 0;
    };

    database
        .cleanup_expired_chat_restrictions(now_unix)
        .await
        .unwrap_or_else(|error| {
            tracing::error!(
                service = SERVICE_NAME,
                %error,
                "failed cleanup of expired chat restrictions"
            );
            0
        })
}

async fn publish_ops_event(base_url: &str, event_type: &str, payload: &serde_json::Value) {
    let event = platform_events::build_event(event_type, payload);
    let _ = platform_events::publish_http(base_url, "ops.chat", &event).await;
}

fn tenant_context_from_env() -> TenantContext {
    let tenant_id = std::env::var("CHAT_WORKER_TENANT_ID")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "tenant-default".to_string());
    let tenant_name = std::env::var("CHAT_WORKER_TENANT_NAME")
        .ok()
        .filter(|value| !value.trim().is_empty());

    TenantContext {
        tenant_id,
        tenant_name,
        access_level: TenantAccessLevel::Worker,
    }
}
