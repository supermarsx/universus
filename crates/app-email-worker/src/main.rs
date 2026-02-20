use std::env;
use std::sync::Arc;

use adapter_provider_email::{parse_email_job_payload_bytes, EmailProvider, LoggingEmailProvider};
use platform_tenancy::{TenantAccessLevel, TenantContext};
use platform_worker_runtime::WorkerRuntime;
use redis::aio::MultiplexedConnection;
use redis::{cmd, Client};
use tokio::signal;
use tokio::sync::oneshot;
use tracing_subscriber::EnvFilter;

const DEFAULT_POLL_TIMEOUT_SECS: u64 = 5;
const DEFAULT_EMAIL_QUEUE_NAME: &str = "email.outbound";
const DEFAULT_EMAIL_DLQ_NAME: &str = "email.dead-letter";

struct EmailDispatcher {
    provider: Box<dyn EmailProvider>,
}

impl EmailDispatcher {
    fn new(provider: impl EmailProvider + 'static) -> Self {
        Self {
            provider: Box::new(provider),
        }
    }

    fn dispatch(&self, payload: &[u8]) -> Result<(), String> {
        let job = parse_email_job_payload_bytes(payload).map_err(|error| error.to_string())?;
        self.provider
            .dispatch(&job)
            .map(|_| ())
            .map_err(|error| error.to_string())
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

fn process_job(dispatcher: &EmailDispatcher, payload: &[u8]) -> Result<(), String> {
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
        tracing::info!(
            service = "app-email-worker",
            "REDIS_URL not set; worker disabled"
        );
        return;
    };

    let poll_timeout_seconds =
        parse_poll_timeout_seconds(env::var("WORKER_POLL_TIMEOUT_SECONDS").ok().as_deref());
    let max_inflight = env::var("EMAIL_WORKER_MAX_INFLIGHT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(16);
    let email_queue_key = read_redis_key(
        "EMAIL_QUEUE_KEY",
        "EMAIL_QUEUE_NAME",
        DEFAULT_EMAIL_QUEUE_NAME,
    );
    let email_dlq_key = read_redis_key(
        "EMAIL_DEAD_LETTER_KEY",
        "EMAIL_DLQ_NAME",
        DEFAULT_EMAIL_DLQ_NAME,
    );

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
        max_inflight,
        email_queue_key = %email_queue_key,
        email_dlq_key = %email_dlq_key,
        "worker startup"
    );

    let dispatcher = Arc::new(EmailDispatcher::new(LoggingEmailProvider::default()));
    let tenant_context = tenant_context_from_env();
    let runtime = WorkerRuntime::current(max_inflight);

    loop {
        tokio::select! {
            _ = signal::ctrl_c() => {
                tracing::info!(service = "app-email-worker", "shutdown signal received");
                break;
            }
            pop_result = pop_job(&mut conn, &email_queue_key, poll_timeout_seconds) => {
                match pop_result {
                    Ok(Some(payload)) => {
                        let dispatcher_ref = Arc::clone(&dispatcher);
                        let context = tenant_context.clone();
                        let payload_for_task = payload.clone();
                        let (done_tx, done_rx) = oneshot::channel::<Result<(), String>>();
                        let spawn_result = runtime.spawn_tenant_task(context, async move {
                            let result = process_job(dispatcher_ref.as_ref(), &payload_for_task);
                            let _ = done_tx.send(result);
                            Ok(())
                        });
                        let process_result = match spawn_result {
                            Ok(()) => match done_rx.await {
                                Ok(result) => result,
                                Err(_) => Err("email runtime task ended before reporting result".to_string()),
                            },
                            Err(error) => Err(format!("failed to schedule email job: {error}")),
                        };

                        match process_result {
                            Ok(()) => {
                                tracing::info!(
                                    service = "app-email-worker",
                                    email_queue_key = %email_queue_key,
                                    payload_size = payload.len(),
                                    "processed email job"
                                );
                                let stats = runtime.stats().await;
                                tracing::debug!(
                                    service = "app-email-worker",
                                    total_inflight = stats.total_inflight,
                                    tenant_count = stats.per_tenant.len(),
                                    "email worker runtime stats"
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

    runtime.shutdown(std::time::Duration::from_secs(5)).await;
    tracing::info!(service = "app-email-worker", "worker shutdown complete");
}

fn tenant_context_from_env() -> TenantContext {
    let tenant_id = env::var("EMAIL_WORKER_TENANT_ID")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "tenant-default".to_string());
    let tenant_name = env::var("EMAIL_WORKER_TENANT_NAME")
        .ok()
        .filter(|value| !value.trim().is_empty());

    TenantContext {
        tenant_id,
        tenant_name,
        access_level: TenantAccessLevel::Worker,
    }
}
