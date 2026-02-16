use std::time::{SystemTime, UNIX_EPOCH};

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
    let realtime_url = std::env::var("REALTIME_GATEWAY_URL")
        .ok()
        .filter(|value| !value.trim().is_empty());

    tracing::info!(
        service = SERVICE_NAME,
        interval_secs,
        run_once,
        has_realtime_url = realtime_url.is_some(),
        "chat cleanup worker started"
    );

    loop {
        let now_unix = now_unix();
        let removed = cleanup_expired_restrictions(now_unix).await;
        tracing::info!(
            service = SERVICE_NAME,
            now_unix,
            removed,
            "chat restriction cleanup cycle completed"
        );
        if removed > 0 {
            if let Some(url) = realtime_url.as_deref() {
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

        if run_once {
            break;
        }
        sleep(Duration::from_secs(interval_secs)).await;
    }
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
