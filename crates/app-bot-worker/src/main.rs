use std::env;
use std::time::Duration;

use tokio::signal;
use tokio::time;
use tracing_subscriber::EnvFilter;

const DEFAULT_INTERVAL_MS: u64 = 60_000;
const DEFAULT_MAX_BOTS_PER_CYCLE: usize = 25;

fn parse_interval_ms(raw: Option<&str>) -> u64 {
    raw.and_then(|value| value.parse::<u64>().ok())
        .filter(|milliseconds| *milliseconds > 0)
        .unwrap_or(DEFAULT_INTERVAL_MS)
}

fn parse_max_bots(raw: Option<&str>) -> usize {
    raw.and_then(|value| value.parse::<usize>().ok())
        .filter(|max| *max > 0)
        .unwrap_or(DEFAULT_MAX_BOTS_PER_CYCLE)
}

fn simulated_pending_bots() -> Vec<u64> {
    vec![101, 102, 103, 104, 105, 106, 107, 108]
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let interval_ms = parse_interval_ms(env::var("BOT_WORKER_INTERVAL_MS").ok().as_deref());
    let max_bots_per_cycle = parse_max_bots(env::var("BOT_WORKER_MAX_BOTS").ok().as_deref());
    let mut interval = time::interval(Duration::from_millis(interval_ms));

    tracing::info!(
        service = "app-bot-worker",
        interval_ms,
        max_bots_per_cycle,
        "worker startup"
    );

    loop {
        tokio::select! {
            _ = interval.tick() => {
                let pending = simulated_pending_bots();
                let batch_size = pending.len().min(max_bots_per_cycle);

                tracing::info!(
                    service = "app-bot-worker",
                    pending_bots = pending.len(),
                    processing_bots = batch_size,
                    "processing cycle started"
                );

                for bot_id in pending.into_iter().take(batch_size) {
                    tracing::info!(service = "app-bot-worker", bot_id, "processing bot think cycle");
                }

                tracing::info!(service = "app-bot-worker", processed_bots = batch_size, "processing cycle completed");
            }
            _ = signal::ctrl_c() => {
                tracing::info!(service = "app-bot-worker", "shutdown signal received");
                break;
            }
        }
    }

    tracing::info!(service = "app-bot-worker", "worker shutdown complete");
}
