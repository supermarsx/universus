use std::env;

use adapter_provider_email::EmailProviderAdapter;
use redis::aio::MultiplexedConnection;
use redis::{cmd, Client};
use tokio::signal;
use tracing_subscriber::EnvFilter;

const DEFAULT_POLL_TIMEOUT_SECS: u64 = 5;
const DEFAULT_EMAIL_QUEUE_NAME: &str = "email.outbound";
const DEFAULT_EMAIL_DLQ_NAME: &str = "email.dead-letter";

struct EmailDispatcher {
    provider: EmailProviderAdapter,
}

impl EmailDispatcher {
    fn new(provider: EmailProviderAdapter) -> Self {
        Self { provider }
    }

    fn dispatch(&self, payload: &[u8]) -> Result<(), &'static str> {
        let _provider = &self.provider;
        if payload.is_empty() {
            return Err("empty job payload");
        }

        Ok(())
    }
}

fn parse_poll_timeout_seconds(raw: Option<&str>) -> u64 {
    raw.and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(DEFAULT_POLL_TIMEOUT_SECS)
}

fn parse_key_name(raw: Option<&str>, default_name: &str) -> String {
    raw.map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default_name)
        .to_string()
}

fn read_redis_key(primary_env: &str, fallback_env: &str, default_name: &str) -> String {
    let fallback = parse_key_name(env::var(fallback_env).ok().as_deref(), default_name);
    parse_key_name(env::var(primary_env).ok().as_deref(), fallback.as_str())
}

async fn pop_job(
    conn: &mut MultiplexedConnection,
    queue_key: &str,
    timeout_seconds: u64,
) -> redis::RedisResult<Option<Vec<u8>>> {
    let popped: Option<(String, Vec<u8>)> = cmd("BLPOP")
        .arg(queue_key)
        .arg(timeout_seconds)
        .query_async(conn)
        .await?;

    Ok(popped.map(|(_, payload)| payload))
}

fn process_job(dispatcher: &EmailDispatcher, payload: &[u8]) -> Result<(), &'static str> {
    dispatcher.dispatch(payload)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let redis_url = env::var("REDIS_URL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let Some(redis_url) = redis_url else {
        tracing::info!(service = "app-email-worker", "REDIS_URL not set; worker disabled");
        return;
    };

    let poll_timeout_seconds =
        parse_poll_timeout_seconds(env::var("WORKER_POLL_TIMEOUT_SECONDS").ok().as_deref());
    let email_queue_key =
        read_redis_key("EMAIL_QUEUE_KEY", "EMAIL_QUEUE_NAME", DEFAULT_EMAIL_QUEUE_NAME);
    let email_dlq_key =
        read_redis_key("EMAIL_DEAD_LETTER_KEY", "EMAIL_DLQ_NAME", DEFAULT_EMAIL_DLQ_NAME);

    let client = match Client::open(redis_url.as_str()) {
        Ok(client) => client,
        Err(error) => {
            tracing::error!(
                service = "app-email-worker",
                error = %error,
                "failed to create Redis client"
            );
            return;
        }
    };

    let mut conn = match client.get_multiplexed_async_connection().await {
        Ok(conn) => conn,
        Err(error) => {
            tracing::error!(
                service = "app-email-worker",
                error = %error,
                "failed to connect to Redis"
            );
            return;
        }
    };

    tracing::info!(
        service = "app-email-worker",
        poll_timeout_seconds,
        email_queue_key = %email_queue_key,
        email_dlq_key = %email_dlq_key,
        "worker startup"
    );

    let dispatcher = EmailDispatcher::new(EmailProviderAdapter);

    loop {
        tokio::select! {
            _ = signal::ctrl_c() => {
                tracing::info!(service = "app-email-worker", "shutdown signal received");
                break;
            }
            pop_result = pop_job(&mut conn, &email_queue_key, poll_timeout_seconds) => {
                match pop_result {
                    Ok(Some(payload)) => {
                        match process_job(&dispatcher, &payload) {
                            Ok(()) => {
                                tracing::info!(
                                    service = "app-email-worker",
                                    email_queue_key = %email_queue_key,
                                    payload_size = payload.len(),
                                    "processed email job"
                                );
                            }
                            Err(error) => {
                                let dlq_push_result: redis::RedisResult<usize> = cmd("RPUSH")
                                    .arg(&email_dlq_key)
                                    .arg(&payload)
                                    .query_async(&mut conn)
                                    .await;

                                match dlq_push_result {
                                    Ok(dlq_size) => {
                                        tracing::warn!(
                                            service = "app-email-worker",
                                            email_queue_key = %email_queue_key,
                                            email_dlq_key = %email_dlq_key,
                                            payload_size = payload.len(),
                                            dlq_size,
                                            error,
                                            "job failed and moved to dead letter queue"
                                        );
                                    }
                                    Err(dlq_error) => {
                                        tracing::error!(
                                            service = "app-email-worker",
                                            email_queue_key = %email_queue_key,
                                            email_dlq_key = %email_dlq_key,
                                            payload_size = payload.len(),
                                            error,
                                            dlq_error = %dlq_error,
                                            "job failed and dead letter enqueue failed"
                                        );
                                    }
                                }
                            }
                        }
                    }
                    Ok(None) => {
                        tracing::debug!(
                            service = "app-email-worker",
                            email_queue_key = %email_queue_key,
                            poll_timeout_seconds,
                            "poll timed out with no job"
                        );
                    }
                    Err(error) => {
                        tracing::error!(
                            service = "app-email-worker",
                            email_queue_key = %email_queue_key,
                            error = %error,
                            "redis pop failed; worker shutting down"
                        );
                        break;
                    }
                }
            }
        }
    }

    tracing::info!(service = "app-email-worker", "worker shutdown complete");
}
