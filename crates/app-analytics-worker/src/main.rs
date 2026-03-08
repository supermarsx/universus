//! Analytics event processing worker.
//!
//! Consumes analytics events from a RabbitMQ queue and persists them
//! to the database via `platform_db::Database::track_analytics_event_detailed`.
//!
//! Configuration via environment variables:
//! - `RABBITMQ_URL` — RabbitMQ connection URL (required; worker exits if unset)
//! - `ANALYTICS_QUEUE_NAME` — queue name (default: "analytics_events")
//! - `ANALYTICS_QUEUE_DISABLED` — set to "true" to disable the worker
//! - `ANALYTICS_WORKER_MAX_INFLIGHT` — max concurrent tasks (default: 16)
//! - `ANALYTICS_WORKER_TENANT_ID` — tenant ID for worker context
//! - `ANALYTICS_WORKER_TENANT_NAME` — tenant display name
//! - `ANALYTICS_STATS_INTERVAL` — log stats every N events (default: 100)
//! - `REALTIME_GATEWAY_URL` — URL for publishing operational events

use std::env;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use futures_util::StreamExt;
use lapin::options::{BasicAckOptions, BasicConsumeOptions, BasicNackOptions, QueueDeclareOptions};
use lapin::types::FieldTable;
use lapin::{Channel, Connection, ConnectionProperties};
use platform_db::Database;
use platform_tenancy::{TenantAccessLevel, TenantContext};
use platform_worker_runtime::WorkerRuntime;
use serde::Deserialize;
use serde_json::Value;
use tokio::signal;
use tokio::sync::oneshot;

const SERVICE_NAME: &str = "app-analytics-worker";
const DEFAULT_ANALYTICS_QUEUE_NAME: &str = "analytics_events";

/// Analytics event message schema.
#[derive(Debug, Deserialize)]
struct AnalyticsMessage {
    #[serde(alias = "eventType", alias = "event_type")]
    event_type: String,
    #[serde(default, alias = "userId", alias = "user_id")]
    user_id: Option<i64>,
    #[serde(default, alias = "sessionId", alias = "session_id")]
    session_id: Option<String>,
    #[serde(default)]
    properties: Option<Value>,
    #[serde(default, alias = "userAgent", alias = "user_agent")]
    user_agent: Option<String>,
    #[serde(default, alias = "ipAddress", alias = "ip_address")]
    ip_address: Option<String>,
}

/// Cumulative counters for monitoring.
struct WorkerMetrics {
    events_processed: AtomicU64,
    events_failed: AtomicU64,
    events_nacked: AtomicU64,
    bytes_processed: AtomicU64,
}

impl WorkerMetrics {
    fn new() -> Self {
        Self {
            events_processed: AtomicU64::new(0),
            events_failed: AtomicU64::new(0),
            events_nacked: AtomicU64::new(0),
            bytes_processed: AtomicU64::new(0),
        }
    }

    fn record_success(&self, payload_bytes: u64) {
        self.events_processed.fetch_add(1, Ordering::Relaxed);
        self.bytes_processed
            .fetch_add(payload_bytes, Ordering::Relaxed);
    }

    fn record_failure(&self) {
        self.events_failed.fetch_add(1, Ordering::Relaxed);
        self.events_nacked.fetch_add(1, Ordering::Relaxed);
    }

    fn total_events(&self) -> u64 {
        self.events_processed.load(Ordering::Relaxed) + self.events_failed.load(Ordering::Relaxed)
    }

    fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            processed: self.events_processed.load(Ordering::Relaxed),
            failed: self.events_failed.load(Ordering::Relaxed),
            nacked: self.events_nacked.load(Ordering::Relaxed),
            bytes: self.bytes_processed.load(Ordering::Relaxed),
        }
    }
}

struct MetricsSnapshot {
    processed: u64,
    failed: u64,
    nacked: u64,
    bytes: u64,
}

fn parse_bool(raw: Option<&str>) -> bool {
    matches!(
        raw.map(str::trim).map(str::to_ascii_lowercase).as_deref(),
        Some("1" | "true" | "yes" | "on")
    )
}

fn parse_queue_name(raw: Option<&str>, default_name: &'static str) -> String {
    raw.map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default_name)
        .to_string()
}

fn parse_env<T: std::str::FromStr>(key: &str, default: T) -> T {
    env::var(key)
        .ok()
        .and_then(|v| v.parse::<T>().ok())
        .unwrap_or(default)
}

async fn nack_without_requeue(channel: &Channel, delivery_tag: u64) {
    if let Err(error) = channel
        .basic_nack(
            delivery_tag,
            BasicNackOptions {
                multiple: false,
                requeue: false,
            },
        )
        .await
    {
        tracing::error!(
            service = SERVICE_NAME,
            delivery_tag,
            error = %error,
            "failed to nack message"
        );
    }
}

async fn process_analytics_message(
    database: Option<Database>,
    payload: Vec<u8>,
) -> Result<(), String> {
    let message = serde_json::from_slice::<AnalyticsMessage>(&payload)
        .map_err(|error| format!("invalid analytics payload: {error}"))?;

    let event_type = message.event_type.trim();
    if event_type.is_empty() {
        return Err("analytics payload missing event type".to_string());
    }

    let database = database
        .ok_or_else(|| "database is not configured; cannot persist analytics event".to_string())?;
    database
        .track_analytics_event_detailed(
            event_type,
            message.session_id.as_deref(),
            message.properties,
            message.user_id,
            message.user_agent.as_deref(),
            message.ip_address.as_deref(),
        )
        .await
        .map_err(|error| format!("failed to persist analytics event: {error}"))?;
    Ok(())
}

fn tenant_context_from_env() -> TenantContext {
    let tenant_id = env::var("ANALYTICS_WORKER_TENANT_ID")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "tenant-default".to_string());
    let tenant_name = env::var("ANALYTICS_WORKER_TENANT_NAME")
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
    if let Err(error) = platform_events::publish_http(base_url, "ops.analytics", &event).await {
        tracing::warn!(
            service = SERVICE_NAME,
            event_type,
            %error,
            "failed to publish ops event"
        );
    }
}

#[tokio::main]
async fn main() {
    platform_observability::init(SERVICE_NAME);

    let queue_disabled = parse_bool(env::var("ANALYTICS_QUEUE_DISABLED").ok().as_deref());
    let queue_name = parse_queue_name(
        env::var("ANALYTICS_QUEUE_NAME").ok().as_deref(),
        DEFAULT_ANALYTICS_QUEUE_NAME,
    );
    let max_inflight: usize = parse_env("ANALYTICS_WORKER_MAX_INFLIGHT", 16);
    let stats_interval: u64 = parse_env("ANALYTICS_STATS_INTERVAL", 100);
    let realtime_url: Option<String> = env::var("REALTIME_GATEWAY_URL")
        .ok()
        .filter(|v| !v.trim().is_empty());
    let tenant_context = tenant_context_from_env();
    let runtime = WorkerRuntime::current(max_inflight);
    let metrics = Arc::new(WorkerMetrics::new());
    let rabbitmq_url = env::var("RABBITMQ_URL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    if queue_disabled {
        tracing::info!(
            service = SERVICE_NAME,
            queue_name = %queue_name,
            "analytics queue is disabled, exiting"
        );
        return;
    }

    let Some(rabbitmq_url) = rabbitmq_url else {
        tracing::info!(
            service = SERVICE_NAME,
            queue_name = %queue_name,
            "RABBITMQ_URL is not configured, analytics queue consumer is disabled"
        );
        return;
    };

    let database = Database::from_env();

    let connection = match Connection::connect(&rabbitmq_url, ConnectionProperties::default()).await
    {
        Ok(connection) => connection,
        Err(error) => {
            tracing::error!(
                service = SERVICE_NAME,
                queue_name = %queue_name,
                error = %error,
                "failed to connect to rabbitmq"
            );
            return;
        }
    };

    let channel = match connection.create_channel().await {
        Ok(channel) => channel,
        Err(error) => {
            tracing::error!(
                service = SERVICE_NAME,
                queue_name = %queue_name,
                error = %error,
                "failed to create rabbitmq channel"
            );
            return;
        }
    };

    if let Err(error) = channel
        .queue_declare(
            &queue_name,
            QueueDeclareOptions {
                durable: true,
                ..QueueDeclareOptions::default()
            },
            FieldTable::default(),
        )
        .await
    {
        tracing::error!(
            service = SERVICE_NAME,
            queue_name = %queue_name,
            error = %error,
            "failed to declare analytics queue"
        );
        return;
    }

    let mut consumer = match channel
        .basic_consume(
            &queue_name,
            "app-analytics-worker",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await
    {
        Ok(consumer) => consumer,
        Err(error) => {
            tracing::error!(
                service = SERVICE_NAME,
                queue_name = %queue_name,
                error = %error,
                "failed to start analytics queue consumer"
            );
            return;
        }
    };

    tracing::info!(
        service = SERVICE_NAME,
        queue_name = %queue_name,
        max_inflight,
        stats_interval,
        has_database = database.is_some(),
        has_realtime_url = realtime_url.is_some(),
        tenant_id = %tenant_context.tenant_id,
        "worker startup"
    );

    loop {
        tokio::select! {
            _ = signal::ctrl_c() => {
                tracing::info!(service = SERVICE_NAME, "shutdown signal received");
                break;
            }
            delivery = consumer.next() => {
                let Some(delivery) = delivery else {
                    tracing::warn!(
                        service = SERVICE_NAME,
                        "analytics consumer stream ended"
                    );
                    break;
                };

                let delivery = match delivery {
                    Ok(delivery) => delivery,
                    Err(error) => {
                        tracing::error!(
                            service = SERVICE_NAME,
                            error = %error,
                            "rabbitmq delivery error"
                        );
                        continue;
                    }
                };

                let payload_size = delivery.data.len();
                let payload = delivery.data.clone();
                let database_ref = database.clone();
                let context = tenant_context.clone();
                let (done_tx, done_rx) = oneshot::channel::<Result<(), String>>();
                let spawned = runtime.spawn_tenant_task(context, async move {
                    let result = process_analytics_message(database_ref, payload).await;
                    let _ = done_tx.send(result);
                    Ok(())
                });
                let process_result = match spawned {
                    Ok(_job_id) => match done_rx.await {
                        Ok(result) => result,
                        Err(_) => Err("analytics runtime task ended before reporting result".to_string()),
                    },
                    Err(error) => Err(format!("failed to schedule analytics message: {error}")),
                };

                match process_result {
                    Ok(_) => {
                        if let Err(error) = channel
                            .basic_ack(delivery.delivery_tag, BasicAckOptions::default())
                            .await
                        {
                            tracing::error!(
                                service = SERVICE_NAME,
                                delivery_tag = delivery.delivery_tag,
                                error = %error,
                                "failed to ack analytics event"
                            );
                        }
                        metrics.record_success(payload_size as u64);
                    }
                    Err(error) => {
                        tracing::error!(
                            service = SERVICE_NAME,
                            delivery_tag = delivery.delivery_tag,
                            error = %error,
                            "analytics message processing failed"
                        );
                        nack_without_requeue(&channel, delivery.delivery_tag).await;
                        metrics.record_failure();

                        // Publish failure event for alerting
                        if let Some(url) = &realtime_url {
                            publish_ops_event(
                                url,
                                "analytics.processing.failed",
                                &serde_json::json!({
                                    "error": error,
                                    "deliveryTag": delivery.delivery_tag,
                                    "payloadSize": payload_size
                                }),
                            )
                            .await;
                        }
                    }
                }

                // Periodic stats logging
                let total = metrics.total_events();
                if stats_interval > 0 && total > 0 && total % stats_interval == 0 {
                    let snap = metrics.snapshot();
                    let rt_stats = runtime.stats().await;
                    tracing::info!(
                        service = SERVICE_NAME,
                        events_processed = snap.processed,
                        events_failed = snap.failed,
                        events_nacked = snap.nacked,
                        bytes_processed = snap.bytes,
                        runtime_inflight = rt_stats.total_inflight,
                        runtime_completed = rt_stats.total_completed,
                        runtime_failed = rt_stats.total_failed,
                        "periodic stats"
                    );
                }
            }
        }
    }

    // Log final metrics on shutdown
    let snap = metrics.snapshot();
    tracing::info!(
        service = SERVICE_NAME,
        events_processed = snap.processed,
        events_failed = snap.failed,
        events_nacked = snap.nacked,
        bytes_processed = snap.bytes,
        "final metrics at shutdown"
    );

    runtime.shutdown(std::time::Duration::from_secs(5)).await;
    tracing::info!(service = SERVICE_NAME, "worker shutdown complete");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bool_truthy_values() {
        assert!(parse_bool(Some("true")));
        assert!(parse_bool(Some("TRUE")));
        assert!(parse_bool(Some("True")));
        assert!(parse_bool(Some("1")));
        assert!(parse_bool(Some("yes")));
        assert!(parse_bool(Some("on")));
        assert!(parse_bool(Some("  true  ")));
    }

    #[test]
    fn parse_bool_falsy_values() {
        assert!(!parse_bool(None));
        assert!(!parse_bool(Some("")));
        assert!(!parse_bool(Some("false")));
        assert!(!parse_bool(Some("0")));
        assert!(!parse_bool(Some("no")));
        assert!(!parse_bool(Some("off")));
        assert!(!parse_bool(Some("random")));
    }

    #[test]
    fn parse_queue_name_defaults() {
        assert_eq!(
            parse_queue_name(None, DEFAULT_ANALYTICS_QUEUE_NAME),
            "analytics_events"
        );
        assert_eq!(
            parse_queue_name(Some(""), DEFAULT_ANALYTICS_QUEUE_NAME),
            "analytics_events"
        );
        assert_eq!(
            parse_queue_name(Some("  "), DEFAULT_ANALYTICS_QUEUE_NAME),
            "analytics_events"
        );
    }

    #[test]
    fn parse_queue_name_custom() {
        assert_eq!(
            parse_queue_name(Some("my_queue"), DEFAULT_ANALYTICS_QUEUE_NAME),
            "my_queue"
        );
    }

    #[test]
    fn analytics_message_deserialize_snake_case() {
        let json = r#"{"event_type":"page_view","user_id":42,"session_id":"abc"}"#;
        let msg: AnalyticsMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.event_type, "page_view");
        assert_eq!(msg.user_id, Some(42));
        assert_eq!(msg.session_id.as_deref(), Some("abc"));
    }

    #[test]
    fn analytics_message_deserialize_camel_case() {
        let json = r#"{"eventType":"click","userId":7,"sessionId":"xyz","userAgent":"Mozilla","ipAddress":"1.2.3.4"}"#;
        let msg: AnalyticsMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.event_type, "click");
        assert_eq!(msg.user_id, Some(7));
        assert_eq!(msg.session_id.as_deref(), Some("xyz"));
        assert_eq!(msg.user_agent.as_deref(), Some("Mozilla"));
        assert_eq!(msg.ip_address.as_deref(), Some("1.2.3.4"));
    }

    #[test]
    fn analytics_message_minimal() {
        let json = r#"{"event_type":"login"}"#;
        let msg: AnalyticsMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.event_type, "login");
        assert_eq!(msg.user_id, None);
        assert_eq!(msg.session_id, None);
        assert_eq!(msg.properties, None);
        assert_eq!(msg.user_agent, None);
        assert_eq!(msg.ip_address, None);
    }

    #[test]
    fn analytics_message_with_properties() {
        let json = r#"{"event_type":"purchase","properties":{"item":"sword","price":100}}"#;
        let msg: AnalyticsMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.event_type, "purchase");
        let props = msg.properties.unwrap();
        assert_eq!(props["item"], "sword");
        assert_eq!(props["price"], 100);
    }

    #[test]
    fn metrics_tracking() {
        let m = WorkerMetrics::new();
        m.record_success(100);
        m.record_success(200);
        m.record_failure();

        let snap = m.snapshot();
        assert_eq!(snap.processed, 2);
        assert_eq!(snap.failed, 1);
        assert_eq!(snap.nacked, 1);
        assert_eq!(snap.bytes, 300);
        assert_eq!(m.total_events(), 3);
    }
}
