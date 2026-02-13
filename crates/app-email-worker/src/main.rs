use std::env;
use std::time::Duration;
use tokio::signal;
use tokio::time;
use tracing_subscriber::EnvFilter;

const DEFAULT_INTERVAL_SECS: u64 = 10;
const DEFAULT_EMAIL_QUEUE_NAME: &str = "email.outbound";
const DEFAULT_EMAIL_DLQ_NAME: &str = "email.dead-letter";

fn parse_interval_seconds(raw: Option<&str>) -> u64 {
    raw.and_then(|value| value.parse::<u64>().ok())
        .filter(|seconds| *seconds > 0)
        .unwrap_or(DEFAULT_INTERVAL_SECS)
}

fn parse_queue_name(raw: Option<&str>, default_name: &'static str) -> String {
    raw.map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default_name)
        .to_string()
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let raw_interval = env::var("WORKER_INTERVAL_SECONDS").ok();
    let interval_seconds = parse_interval_seconds(raw_interval.as_deref());
    let email_queue_name = parse_queue_name(
        env::var("EMAIL_QUEUE_NAME").ok().as_deref(),
        DEFAULT_EMAIL_QUEUE_NAME,
    );
    let email_dlq_name = parse_queue_name(
        env::var("EMAIL_DLQ_NAME").ok().as_deref(),
        DEFAULT_EMAIL_DLQ_NAME,
    );
    let mut interval = time::interval(Duration::from_secs(interval_seconds));
    let mut cycle: u64 = 0;

    tracing::info!(
        service = "app-email-worker",
        interval_seconds,
        email_queue_name = %email_queue_name,
        email_dlq_name = %email_dlq_name,
        "worker startup"
    );

    loop {
        tokio::select! {
            _ = interval.tick() => {
                cycle += 1;
                let queue_count = (cycle * 7) % 41;
                let dead_letter_count = (cycle * 3) % 9;
                let polled = queue_count.min(5);
                let sent = polled.saturating_sub(dead_letter_count.min(polled));

                tracing::info!(
                    service = "app-email-worker",
                    cycle,
                    email_queue_name = %email_queue_name,
                    email_dlq_name = %email_dlq_name,
                    queue_count,
                    dead_letter_count,
                    polled,
                    sent,
                    "worker poll cycle"
                );
            }
            _ = signal::ctrl_c() => {
                tracing::info!(service = "app-email-worker", "shutdown signal received");
                break;
            }
        }
    }

    tracing::info!(service = "app-email-worker", "worker shutdown complete");
}
