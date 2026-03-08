//! Email dispatch worker.
//!
//! Consumes email jobs from a Redis queue (BLPOP), dispatches them via the
//! configured `EmailProvider`, and routes failures to a dead-letter queue.
//!
//! Configuration via environment variables:
//! - `REDIS_URL` — Redis connection URL (required; worker exits if unset)
//! - `WORKER_POLL_TIMEOUT_SECONDS` — BLPOP timeout (default: 5)
//! - `EMAIL_WORKER_MAX_INFLIGHT` — max concurrent dispatch tasks (default: 16)
//! - `EMAIL_QUEUE_KEY` / `EMAIL_QUEUE_NAME` — Redis key for inbound jobs (default: "email.outbound")
//! - `EMAIL_DEAD_LETTER_KEY` / `EMAIL_DLQ_NAME` — Redis key for failed jobs (default: "email.dead-letter")
//! - `EMAIL_WORKER_TENANT_ID` — tenant ID for worker context
//! - `EMAIL_WORKER_TENANT_NAME` — tenant display name
//! - `REALTIME_GATEWAY_URL` — URL for publishing operational events
//! - `EMAIL_STATS_INTERVAL_JOBS` — log stats every N jobs (default: 50)

use std::env;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use adapter_provider_email::{parse_email_job_payload_bytes, EmailProvider, LoggingEmailProvider};
use platform_tenancy::{TenantAccessLevel, TenantContext};
use platform_worker_runtime::WorkerRuntime;
use redis::aio::MultiplexedConnection;
use redis::{cmd, Client};
use tokio::signal;
use tokio::sync::oneshot;

const SERVICE_NAME: &str = "app-email-worker";
const DEFAULT_POLL_TIMEOUT_SECS: u64 = 5;
const DEFAULT_EMAIL_QUEUE_NAME: &str = "email.outbound";
const DEFAULT_EMAIL_DLQ_NAME: &str = "email.dead-letter";

/// Cumulative counters for monitoring.
struct WorkerMetrics {
    jobs_processed: AtomicU64,
    jobs_failed: AtomicU64,
    jobs_dlq: AtomicU64,
    bytes_processed: AtomicU64,
}

impl WorkerMetrics {
    fn new() -> Self {
        Self {
            jobs_processed: AtomicU64::new(0),
            jobs_failed: AtomicU64::new(0),
            jobs_dlq: AtomicU64::new(0),
            bytes_processed: AtomicU64::new(0),
        }
    }

    fn record_success(&self, payload_size: u64) {
        self.jobs_processed.fetch_add(1, Ordering::Relaxed);
        self.bytes_processed
            .fetch_add(payload_size, Ordering::Relaxed);
    }

    fn record_failure(&self) {
        self.jobs_failed.fetch_add(1, Ordering::Relaxed);
    }

    fn record_dlq(&self) {
        self.jobs_dlq.fetch_add(1, Ordering::Relaxed);
    }

    fn total_jobs(&self) -> u64 {
        self.jobs_processed.load(Ordering::Relaxed) + self.jobs_failed.load(Ordering::Relaxed)
    }

    fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            processed: self.jobs_processed.load(Ordering::Relaxed),
            failed: self.jobs_failed.load(Ordering::Relaxed),
            dlq: self.jobs_dlq.load(Ordering::Relaxed),
            bytes: self.bytes_processed.load(Ordering::Relaxed),
        }
    }
}

struct MetricsSnapshot {
    processed: u64,
    failed: u64,
    dlq: u64,
    bytes: u64,
}

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

fn parse_env<T: std::str::FromStr>(key: &str, default: T) -> T {
    env::var(key)
        .ok()
        .and_then(|v| v.parse::<T>().ok())
        .unwrap_or(default)
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
    platform_observability::init(SERVICE_NAME);

    let redis_url = env::var("REDIS_URL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let Some(redis_url) = redis_url else {
        tracing::info!(service = SERVICE_NAME, "REDIS_URL not set; worker disabled");
        return;
    };

    let poll_timeout_seconds =
        parse_poll_timeout_seconds(env::var("WORKER_POLL_TIMEOUT_SECONDS").ok().as_deref());
    let max_inflight: usize = parse_env("EMAIL_WORKER_MAX_INFLIGHT", 16);
    let stats_interval: u64 = parse_env("EMAIL_STATS_INTERVAL_JOBS", 50);
    let realtime_url: Option<String> = env::var("REALTIME_GATEWAY_URL")
        .ok()
        .filter(|v| !v.trim().is_empty());

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
                service = SERVICE_NAME,
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
                service = SERVICE_NAME,
                error = %error,
                "failed to connect to Redis"
            );
            return;
        }
    };

    tracing::info!(
        service = SERVICE_NAME,
        poll_timeout_seconds,
        max_inflight,
        stats_interval,
        email_queue_key = %email_queue_key,
        email_dlq_key = %email_dlq_key,
        has_realtime_url = realtime_url.is_some(),
        "worker startup"
    );

    let dispatcher = Arc::new(EmailDispatcher::new(LoggingEmailProvider::default()));
    let tenant_context = tenant_context_from_env();
    let runtime = WorkerRuntime::current(max_inflight);
    let metrics = Arc::new(WorkerMetrics::new());

    loop {
        tokio::select! {
            _ = signal::ctrl_c() => {
                tracing::info!(service = SERVICE_NAME, "shutdown signal received");
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
                            Ok(_job_id) => match done_rx.await {
                                Ok(result) => result,
                                Err(_) => Err("email runtime task ended before reporting result".to_string()),
                            },
                            Err(error) => Err(format!("failed to schedule email job: {error}")),
                        };

                        match process_result {
                            Ok(()) => {
                                metrics.record_success(payload.len() as u64);
                                tracing::info!(
                                    service = SERVICE_NAME,
                                    email_queue_key = %email_queue_key,
                                    payload_size = payload.len(),
                                    "processed email job"
                                );
                            }
                            Err(error) => {
                                metrics.record_failure();

                                let dlq_push_result: redis::RedisResult<usize> = cmd("RPUSH")
                                    .arg(&email_dlq_key)
                                    .arg(&payload)
                                    .query_async(&mut conn)
                                    .await;

                                match dlq_push_result {
                                    Ok(dlq_size) => {
                                        metrics.record_dlq();
                                        tracing::warn!(
                                            service = SERVICE_NAME,
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
                                            service = SERVICE_NAME,
                                            email_queue_key = %email_queue_key,
                                            email_dlq_key = %email_dlq_key,
                                            payload_size = payload.len(),
                                            error,
                                            dlq_error = %dlq_error,
                                            "job failed and dead letter enqueue failed"
                                        );
                                    }
                                }

                                // Publish failure event for alerting
                                if let Some(url) = &realtime_url {
                                    publish_ops_event(
                                        url,
                                        "email.dispatch.failed",
                                        &serde_json::json!({
                                            "error": error,
                                            "payloadSize": payload.len(),
                                            "queueKey": email_queue_key
                                        }),
                                    )
                                    .await;
                                }
                            }
                        }

                        // Periodic stats logging
                        let total = metrics.total_jobs();
                        if stats_interval > 0 && total > 0 && total % stats_interval == 0 {
                            let snap = metrics.snapshot();
                            let rt_stats = runtime.stats().await;
                            tracing::info!(
                                service = SERVICE_NAME,
                                jobs_processed = snap.processed,
                                jobs_failed = snap.failed,
                                jobs_dlq = snap.dlq,
                                bytes_processed = snap.bytes,
                                runtime_inflight = rt_stats.total_inflight,
                                runtime_completed = rt_stats.total_completed,
                                runtime_failed = rt_stats.total_failed,
                                "periodic stats"
                            );
                        }
                    }
                    Ok(None) => {
                        tracing::debug!(
                            service = SERVICE_NAME,
                            email_queue_key = %email_queue_key,
                            poll_timeout_seconds,
                            "poll timed out with no job"
                        );
                    }
                    Err(error) => {
                        tracing::error!(
                            service = SERVICE_NAME,
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

    // Log final metrics on shutdown
    let snap = metrics.snapshot();
    tracing::info!(
        service = SERVICE_NAME,
        jobs_processed = snap.processed,
        jobs_failed = snap.failed,
        jobs_dlq = snap.dlq,
        bytes_processed = snap.bytes,
        "final metrics at shutdown"
    );

    runtime.shutdown(std::time::Duration::from_secs(5)).await;
    tracing::info!(service = SERVICE_NAME, "worker shutdown complete");
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

async fn publish_ops_event(base_url: &str, event_type: &str, payload: &serde_json::Value) {
    let event = platform_events::build_event(event_type, payload);
    if let Err(error) = platform_events::publish_http(base_url, "ops.email", &event).await {
        tracing::warn!(
            service = SERVICE_NAME,
            event_type,
            %error,
            "failed to publish ops event"
        );
    }
}
