use std::time::{SystemTime, UNIX_EPOCH};

use game_chat::ChatRestrictionStore;
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

    let mut store = ChatRestrictionStore::with_seed();

    tracing::info!(
        service = SERVICE_NAME,
        interval_secs,
        run_once,
        "chat cleanup worker started"
    );

    loop {
        let now_unix = now_unix();
        let removed = store.cleanup_expired(now_unix);
        tracing::info!(
            service = SERVICE_NAME,
            now_unix,
            removed,
            remaining = store.list().len(),
            "chat restriction cleanup cycle completed"
        );

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
