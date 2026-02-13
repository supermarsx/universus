use std::env;
use std::time::Duration;
use tokio::signal;
use tokio::time;
use tracing_subscriber::EnvFilter;

const DEFAULT_INTERVAL_SECS: u64 = 10;

fn parse_interval_seconds(raw: Option<&str>) -> u64 {
    raw.and_then(|value| value.parse::<u64>().ok())
        .filter(|seconds| *seconds > 0)
        .unwrap_or(DEFAULT_INTERVAL_SECS)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let raw_interval = env::var("WORKER_INTERVAL_SECONDS").ok();
    let interval_seconds = parse_interval_seconds(raw_interval.as_deref());
    let mut interval = time::interval(Duration::from_secs(interval_seconds));

    tracing::info!(
        service = "app-analytics-worker",
        interval_seconds,
        "worker startup"
    );

    loop {
        tokio::select! {
            _ = interval.tick() => {
                tracing::info!(service = "app-analytics-worker", "heartbeat");
                tracing::info!(service = "app-analytics-worker", "simulated poll action");
            }
            _ = signal::ctrl_c() => {
                tracing::info!(service = "app-analytics-worker", "shutdown signal received");
                break;
            }
        }
    }

    tracing::info!(service = "app-analytics-worker", "worker shutdown complete");
}
