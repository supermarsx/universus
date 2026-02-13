use std::env;
use std::time::Duration;
use tokio::signal;
use tokio::time;
use tracing_subscriber::EnvFilter;

const DEFAULT_INTERVAL_SECS: u64 = 10;
const DEFAULT_BATCH_SIZE: u64 = 100;

fn parse_interval_seconds(raw: Option<&str>) -> u64 {
    raw.and_then(|value| value.parse::<u64>().ok())
        .filter(|seconds| *seconds > 0)
        .unwrap_or(DEFAULT_INTERVAL_SECS)
}

fn parse_batch_size(raw: Option<&str>) -> u64 {
    raw.and_then(|value| value.parse::<u64>().ok())
        .filter(|size| *size > 0)
        .unwrap_or(DEFAULT_BATCH_SIZE)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let raw_interval = env::var("WORKER_INTERVAL_SECONDS").ok();
    let interval_seconds = parse_interval_seconds(raw_interval.as_deref());
    let raw_batch_size = env::var("ANALYTICS_BATCH_SIZE").ok();
    let batch_size = parse_batch_size(raw_batch_size.as_deref());
    let mut interval = time::interval(Duration::from_secs(interval_seconds));
    let mut cycle: u64 = 0;
    let mut pending_events: u64 = 0;

    tracing::info!(
        service = "app-analytics-worker",
        interval_seconds,
        batch_size,
        "worker startup"
    );

    loop {
        tokio::select! {
            _ = interval.tick() => {
                cycle += 1;
                let generated_events = 20 + ((cycle * 11) % 55);
                pending_events += generated_events;

                let flush_batches = pending_events / batch_size;
                let flushed_events = flush_batches * batch_size;
                pending_events -= flushed_events;

                tracing::info!(
                    service = "app-analytics-worker",
                    cycle,
                    generated_events,
                    batch_size,
                    flush_batches,
                    flushed_events,
                    pending_events,
                    "analytics ingestion cycle"
                );
            }
            _ = signal::ctrl_c() => {
                tracing::info!(service = "app-analytics-worker", "shutdown signal received");
                break;
            }
        }
    }

    tracing::info!(service = "app-analytics-worker", "worker shutdown complete");
}
