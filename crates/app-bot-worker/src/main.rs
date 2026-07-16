//! Bot processing worker.
//!
//! Periodically calls the bot API's `process-all` endpoint to trigger
//! AI bot decision-making cycles across all active bot accounts.
//!
//! Configuration via environment variables:
//! - `BOT_WORKER_INTERVAL_MS` — interval between processing cycles in ms (default: 60000)
//! - `BOT_WORKER_MAX_INFLIGHT` — max concurrent tasks (default: 8)
//! - `BOT_API_URL` — base URL for the bot API (default: "http://localhost:4001")
//! - `JWT_SECRET` — shared signing secret used for service Bearer JWTs
//! - `BOT_WORKER_TENANT_ID` — tenant ID for worker context
//! - `BOT_WORKER_TENANT_NAME` — tenant display name
//! - `BOT_HTTP_TIMEOUT_SECS` — HTTP request timeout in seconds (default: 30)
//! - `REALTIME_GATEWAY_URL` — URL for publishing operational events

use std::env;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use platform_tenancy::{TenantAccessLevel, TenantContext};
use platform_worker_runtime::WorkerRuntime;
use reqwest::{Client, Url};
use serde_json::Value;
use tokio::signal;
use tokio::sync::oneshot;
use tokio::time;

const SERVICE_NAME: &str = "app-bot-worker";
const DEFAULT_INTERVAL_MS: u64 = 60_000;
const DEFAULT_BOT_API_URL: &str = "http://localhost:4001";
const PROCESS_ALL_PATH: &str = "/api/admin/bots/process-all";

/// Cumulative counters for monitoring.
struct WorkerMetrics {
    cycles_total: AtomicU64,
    cycles_success: AtomicU64,
    cycles_failed: AtomicU64,
    total_bots_processed: AtomicU64,
}

impl WorkerMetrics {
    fn new() -> Self {
        Self {
            cycles_total: AtomicU64::new(0),
            cycles_success: AtomicU64::new(0),
            cycles_failed: AtomicU64::new(0),
            total_bots_processed: AtomicU64::new(0),
        }
    }

    fn record_success(&self, bots_processed: u64) {
        self.cycles_total.fetch_add(1, Ordering::Relaxed);
        self.cycles_success.fetch_add(1, Ordering::Relaxed);
        self.total_bots_processed
            .fetch_add(bots_processed, Ordering::Relaxed);
    }

    fn record_failure(&self) {
        self.cycles_total.fetch_add(1, Ordering::Relaxed);
        self.cycles_failed.fetch_add(1, Ordering::Relaxed);
    }

    fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            cycles_total: self.cycles_total.load(Ordering::Relaxed),
            cycles_success: self.cycles_success.load(Ordering::Relaxed),
            cycles_failed: self.cycles_failed.load(Ordering::Relaxed),
            total_bots_processed: self.total_bots_processed.load(Ordering::Relaxed),
        }
    }
}

struct MetricsSnapshot {
    cycles_total: u64,
    cycles_success: u64,
    cycles_failed: u64,
    total_bots_processed: u64,
}

fn parse_interval_ms(raw: Option<&str>) -> u64 {
    raw.and_then(|value| value.parse::<u64>().ok())
        .filter(|milliseconds| *milliseconds > 0)
        .unwrap_or(DEFAULT_INTERVAL_MS)
}

fn parse_bot_api_url(raw: Option<&str>) -> String {
    raw.map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_BOT_API_URL)
        .to_string()
}

fn parse_env<T: std::str::FromStr>(key: &str, default: T) -> T {
    env::var(key)
        .ok()
        .and_then(|v| v.parse::<T>().ok())
        .unwrap_or(default)
}

fn build_process_all_url(base_url: &str) -> Result<Url, String> {
    let mut parsed = Url::parse(base_url)
        .map_err(|error| format!("invalid BOT_API_URL '{base_url}': {error}"))?;
    parsed.set_path(PROCESS_ALL_PATH);
    parsed.set_query(None);
    parsed.set_fragment(None);
    Ok(parsed)
}

fn extract_processed_count(payload: &Value) -> Option<u64> {
    fn count_from_value(value: &Value) -> Option<u64> {
        const CANDIDATE_KEYS: &[&str] = &[
            "processed",
            "processed_count",
            "processedCount",
            "count",
            "total_processed",
            "totalProcessed",
        ];

        CANDIDATE_KEYS
            .iter()
            .filter_map(|key| value.get(*key))
            .find_map(Value::as_u64)
    }

    count_from_value(payload).or_else(|| payload.get("data").and_then(count_from_value))
}

struct ProcessCallOutcome {
    processed_count: Option<u64>,
    message: Option<String>,
}

async fn call_process_all_bots(
    client: &Client,
    endpoint: &Url,
    auth_config: &platform_auth::AuthConfig,
) -> Result<ProcessCallOutcome, String> {
    let token = platform_auth::generate_token(
        auth_config,
        "service:app-bot-worker",
        SERVICE_NAME,
        "admin",
        None,
    )
    .map_err(|error| format!("failed to issue service token: {error}"))?;

    let response = client
        .post(endpoint.clone())
        .bearer_auth(token)
        .send()
        .await
        .map_err(|error| format!("request failed: {error}"))?;

    let status = response.status();
    let payload = response.json::<Value>().await.ok();
    let message = payload.as_ref().and_then(|json| {
        json.get("message")
            .or_else(|| json.get("error"))
            .and_then(Value::as_str)
            .map(str::to_string)
    });

    if !status.is_success() {
        return Err(match message {
            Some(message) => format!("status {status}: {message}"),
            None => format!("status {status}"),
        });
    }

    if let Some(success) = payload
        .as_ref()
        .and_then(|json| json.get("success"))
        .and_then(Value::as_bool)
    {
        if !success {
            return Err(match message {
                Some(message) => format!("API reported failure: {message}"),
                None => "API reported failure".to_string(),
            });
        }
    }

    Ok(ProcessCallOutcome {
        processed_count: payload.as_ref().and_then(extract_processed_count),
        message,
    })
}

fn tenant_context_from_env() -> TenantContext {
    let tenant_id = env::var("BOT_WORKER_TENANT_ID")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "tenant-default".to_string());
    let tenant_name = env::var("BOT_WORKER_TENANT_NAME")
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
    if let Err(error) = platform_events::publish_http(base_url, "ops.bot", &event).await {
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

    let interval_ms = parse_interval_ms(env::var("BOT_WORKER_INTERVAL_MS").ok().as_deref());
    let max_inflight: usize = parse_env("BOT_WORKER_MAX_INFLIGHT", 8);
    let http_timeout_secs: u64 = parse_env("BOT_HTTP_TIMEOUT_SECS", 30);
    let bot_api_url = parse_bot_api_url(env::var("BOT_API_URL").ok().as_deref());
    let auth_config = platform_auth::AuthConfig::from_env();
    if let Err(error) = auth_config.validate_runtime() {
        tracing::error!(service = SERVICE_NAME, %error, "worker startup failed");
        return;
    }
    let realtime_url: Option<String> = env::var("REALTIME_GATEWAY_URL")
        .ok()
        .filter(|v| !v.trim().is_empty());
    let tenant_context = tenant_context_from_env();
    let runtime = WorkerRuntime::current(max_inflight);
    let metrics = Arc::new(WorkerMetrics::new());

    let endpoint = match build_process_all_url(&bot_api_url) {
        Ok(endpoint) => endpoint,
        Err(error) => {
            tracing::error!(service = SERVICE_NAME, error = %error, "worker startup failed");
            return;
        }
    };
    let client = match Client::builder()
        .timeout(Duration::from_secs(http_timeout_secs))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            tracing::error!(
                service = SERVICE_NAME,
                error = %error,
                "failed to initialize HTTP client"
            );
            return;
        }
    };
    let mut interval = time::interval(Duration::from_millis(interval_ms));

    tracing::info!(
        service = SERVICE_NAME,
        interval_ms,
        max_inflight,
        http_timeout_secs,
        tenant_id = %tenant_context.tenant_id,
        endpoint = %endpoint,
        has_realtime_url = realtime_url.is_some(),
        "worker startup"
    );

    loop {
        tokio::select! {
            _ = interval.tick() => {
                let cycle = metrics.snapshot().cycles_total + 1;
                tracing::info!(
                    service = SERVICE_NAME,
                    cycle,
                    endpoint = %endpoint,
                    "processing cycle started"
                );

                let client_ref = client.clone();
                let endpoint_ref = endpoint.clone();
                let auth_config_ref = auth_config.clone();
                let context = tenant_context.clone();
                let (done_tx, done_rx) = oneshot::channel::<Result<ProcessCallOutcome, String>>();
                let spawned = runtime.spawn_tenant_task(context, async move {
                    let result = call_process_all_bots(&client_ref, &endpoint_ref, &auth_config_ref).await;
                    let _ = done_tx.send(result);
                    Ok(())
                });
                let cycle_result = match spawned {
                    Ok(_job_id) => match done_rx.await {
                        Ok(result) => result,
                        Err(_) => Err("bot runtime task ended before reporting result".to_string()),
                    },
                    Err(error) => Err(format!("failed to schedule bot cycle: {error}")),
                };

                match cycle_result {
                    Ok(outcome) => {
                        let bots = outcome.processed_count.unwrap_or(0);
                        metrics.record_success(bots);

                        tracing::info!(
                            service = SERVICE_NAME,
                            cycle,
                            processed_bots = bots,
                            message = ?outcome.message,
                            "processing cycle completed"
                        );

                        if let Some(url) = &realtime_url {
                            publish_ops_event(
                                url,
                                "bot.processing.completed",
                                &serde_json::json!({
                                    "cycle": cycle,
                                    "botsProcessed": bots,
                                    "message": outcome.message
                                }),
                            )
                            .await;
                        }
                    }
                    Err(error) => {
                        metrics.record_failure();

                        tracing::error!(
                            service = SERVICE_NAME,
                            cycle,
                            endpoint = %endpoint,
                            error = %error,
                            "processing cycle failed"
                        );

                        if let Some(url) = &realtime_url {
                            publish_ops_event(
                                url,
                                "bot.processing.failed",
                                &serde_json::json!({
                                    "cycle": cycle,
                                    "error": error,
                                    "endpoint": endpoint.as_str()
                                }),
                            )
                            .await;
                        }
                    }
                }

                // Log runtime stats after each cycle
                let rt_stats = runtime.stats().await;
                let snap = metrics.snapshot();
                tracing::info!(
                    service = SERVICE_NAME,
                    cycles_total = snap.cycles_total,
                    cycles_success = snap.cycles_success,
                    cycles_failed = snap.cycles_failed,
                    total_bots_processed = snap.total_bots_processed,
                    runtime_inflight = rt_stats.total_inflight,
                    runtime_completed = rt_stats.total_completed,
                    "bot worker stats"
                );
            }
            _ = signal::ctrl_c() => {
                tracing::info!(service = SERVICE_NAME, "shutdown signal received");
                break;
            }
        }
    }

    let snap = metrics.snapshot();
    tracing::info!(
        service = SERVICE_NAME,
        cycles_total = snap.cycles_total,
        cycles_success = snap.cycles_success,
        cycles_failed = snap.cycles_failed,
        total_bots_processed = snap.total_bots_processed,
        "final metrics at shutdown"
    );

    runtime.shutdown(Duration::from_secs(5)).await;
    tracing::info!(service = SERVICE_NAME, "worker shutdown complete");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_interval_uses_default_for_invalid_values() {
        assert_eq!(parse_interval_ms(None), DEFAULT_INTERVAL_MS);
        assert_eq!(parse_interval_ms(Some("0")), DEFAULT_INTERVAL_MS);
        assert_eq!(parse_interval_ms(Some("abc")), DEFAULT_INTERVAL_MS);
    }

    #[test]
    fn parse_interval_accepts_valid_values() {
        assert_eq!(parse_interval_ms(Some("5000")), 5000);
        assert_eq!(parse_interval_ms(Some("1")), 1);
        assert_eq!(parse_interval_ms(Some("120000")), 120000);
    }

    #[test]
    fn build_process_all_url_sets_expected_path() {
        let url = build_process_all_url("http://localhost:4001/base/path").unwrap();
        assert_eq!(
            url.as_str(),
            "http://localhost:4001/api/admin/bots/process-all"
        );
    }

    #[test]
    fn build_process_all_url_strips_query_and_fragment() {
        let url = build_process_all_url("http://example.com?foo=bar#baz").unwrap();
        assert_eq!(
            url.as_str(),
            "http://example.com/api/admin/bots/process-all"
        );
    }

    #[test]
    fn build_process_all_url_rejects_invalid_url() {
        let result = build_process_all_url("not-a-url");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid BOT_API_URL"));
    }

    #[test]
    fn extract_processed_count_supports_common_shapes() {
        let top_level = serde_json::json!({ "processedCount": 17 });
        assert_eq!(extract_processed_count(&top_level), Some(17));

        let nested = serde_json::json!({ "data": { "processed": 9 } });
        assert_eq!(extract_processed_count(&nested), Some(9));

        let missing = serde_json::json!({ "success": true });
        assert_eq!(extract_processed_count(&missing), None);
    }

    #[test]
    fn extract_processed_count_all_key_variants() {
        for key in &[
            "processed",
            "processed_count",
            "processedCount",
            "count",
            "total_processed",
            "totalProcessed",
        ] {
            let json = serde_json::json!({ (*key): 42 });
            assert_eq!(
                extract_processed_count(&json),
                Some(42),
                "failed for key: {key}"
            );
        }
    }

    #[test]
    fn parse_bot_api_url_defaults() {
        assert_eq!(parse_bot_api_url(None), DEFAULT_BOT_API_URL);
        assert_eq!(parse_bot_api_url(Some("")), DEFAULT_BOT_API_URL);
        assert_eq!(parse_bot_api_url(Some("  ")), DEFAULT_BOT_API_URL);
    }

    #[test]
    fn parse_bot_api_url_custom() {
        assert_eq!(
            parse_bot_api_url(Some("http://bots.local:9000")),
            "http://bots.local:9000"
        );
    }

    #[test]
    fn metrics_snapshot_tracking() {
        let m = WorkerMetrics::new();
        m.record_success(10);
        m.record_success(5);
        m.record_failure();

        let snap = m.snapshot();
        assert_eq!(snap.cycles_total, 3);
        assert_eq!(snap.cycles_success, 2);
        assert_eq!(snap.cycles_failed, 1);
        assert_eq!(snap.total_bots_processed, 15);
    }
}
