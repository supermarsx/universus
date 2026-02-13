use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

const SERVICE_NAME: &str = "sms";
const DEFAULT_PORT: u16 = 3003;
const HISTORY_LIMIT: usize = 1000;
const HISTORY_DEFAULT_LIMIT: usize = 50;
const HISTORY_MAX_LIMIT: usize = 200;

const SUPPORTED_CHANNELS: [&str; 7] = [
    "sms_twilio",
    "sms_http",
    "whatsapp_twilio",
    "whatsapp_baileys",
    "telegram",
    "discord",
    "custom_http",
];

#[derive(Serialize)]
struct ServiceStatus {
    status: &'static str,
    service: &'static str,
}

#[derive(Serialize)]
struct BasicErrorResponse {
    success: bool,
    error: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SendRequest {
    contact: String,
    message: String,
    channels: Option<Vec<String>>,
    metadata: Option<serde_json::Value>,
    idempotency_key: Option<String>,
}

#[derive(Serialize)]
struct SendResponse {
    success: bool,
    channel: String,
    destination: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    idempotent: Option<bool>,
}

#[derive(Clone)]
struct HistoryEntry {
    id: u64,
    request_id: String,
    idempotency_key: Option<String>,
    contact: String,
    destination: String,
    channel: String,
    status: String,
    error: Option<String>,
    metadata: Option<serde_json::Value>,
    created_at_ms: u128,
}

#[derive(Serialize)]
struct HistoryEntryPayload {
    id: u64,
    request_id: String,
    idempotency_key: Option<String>,
    contact: String,
    destination: String,
    channel: String,
    status: String,
    error: Option<String>,
    metadata: Option<serde_json::Value>,
    created_at: String,
}

#[derive(Serialize)]
struct HistoryResponse {
    success: bool,
    entries: Vec<HistoryEntryPayload>,
}

#[derive(Serialize)]
struct MetricsSnapshot {
    requests: u64,
    successes: u64,
    failures: u64,
    #[serde(rename = "perChannelSuccess")]
    per_channel_success: HashMap<String, u64>,
    #[serde(rename = "perChannelFailure")]
    per_channel_failure: HashMap<String, u64>,
    #[serde(rename = "avgResponseMs")]
    avg_response_ms: f64,
}

#[derive(Serialize)]
struct HistoryStatsItem {
    channel: String,
    status: String,
    count: u64,
}

#[derive(Serialize)]
struct MetricsResponse {
    success: bool,
    metrics: MetricsSnapshot,
    history: Vec<HistoryStatsItem>,
}

#[derive(Deserialize)]
struct HistoryQuery {
    limit: Option<usize>,
}

struct SmsApiState {
    started_at: Instant,
    request_count: u64,
    success_count: u64,
    failure_count: u64,
    per_channel_success: HashMap<String, u64>,
    per_channel_failure: HashMap<String, u64>,
    response_times_ms: Vec<u128>,
    history: Vec<HistoryEntry>,
    next_request_sequence: u64,
    next_history_id: u64,
}

impl SmsApiState {
    fn new() -> Self {
        Self {
            started_at: Instant::now(),
            request_count: 0,
            success_count: 0,
            failure_count: 0,
            per_channel_success: HashMap::new(),
            per_channel_failure: HashMap::new(),
            response_times_ms: Vec::new(),
            history: Vec::new(),
            next_request_sequence: 0,
            next_history_id: 0,
        }
    }

    fn next_request_id(&mut self) -> String {
        self.next_request_sequence += 1;
        format!("sms-req-{:016}", self.next_request_sequence)
    }

    fn next_history_id(&mut self) -> u64 {
        self.next_history_id += 1;
        self.next_history_id
    }

    fn record_request(&mut self) {
        self.request_count += 1;
    }

    fn record_success(&mut self, channel: &str, duration_ms: u128) {
        self.success_count += 1;
        *self
            .per_channel_success
            .entry(channel.to_string())
            .or_insert(0) += 1;
        self.response_times_ms.push(duration_ms);
        if self.response_times_ms.len() > 1000 {
            self.response_times_ms.remove(0);
        }
    }

    fn record_failure(&mut self, channel: &str) {
        self.failure_count += 1;
        *self
            .per_channel_failure
            .entry(channel.to_string())
            .or_insert(0) += 1;
    }

    fn find_history_by_idempotency(&self, key: &str) -> Option<&HistoryEntry> {
        self.history
            .iter()
            .rev()
            .find(|entry| entry.idempotency_key.as_deref() == Some(key))
    }

    fn count_recent_for_contact(&self, contact: &str, window_seconds: u64) -> usize {
        let now = now_unix_epoch_ms();
        let lower_bound = now.saturating_sub((window_seconds as u128) * 1000);
        self.history
            .iter()
            .filter(|entry| entry.contact == contact && entry.created_at_ms >= lower_bound)
            .count()
    }

    fn push_history(&mut self, entry: HistoryEntry) {
        self.history.push(entry);
        if self.history.len() > HISTORY_LIMIT {
            self.history.remove(0);
        }
    }

    fn history_stats(&self) -> Vec<HistoryStatsItem> {
        let mut grouped: HashMap<(String, String), u64> = HashMap::new();
        for entry in &self.history {
            let key = (entry.channel.clone(), entry.status.clone());
            *grouped.entry(key).or_insert(0) += 1;
        }

        let mut stats = grouped
            .into_iter()
            .map(|((channel, status), count)| HistoryStatsItem {
                channel,
                status,
                count,
            })
            .collect::<Vec<_>>();

        stats.sort_by(|a, b| {
            a.channel
                .cmp(&b.channel)
                .then_with(|| a.status.cmp(&b.status))
        });
        stats
    }

    fn metrics_snapshot(&self) -> MetricsSnapshot {
        let avg = if self.response_times_ms.is_empty() {
            0.0
        } else {
            self.response_times_ms.iter().sum::<u128>() as f64 / self.response_times_ms.len() as f64
        };

        MetricsSnapshot {
            requests: self.request_count,
            successes: self.success_count,
            failures: self.failure_count,
            per_channel_success: self.per_channel_success.clone(),
            per_channel_failure: self.per_channel_failure.clone(),
            avg_response_ms: avg,
        }
    }
}

fn now_unix_epoch_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn created_at_iso_ms(unix_ms: u128) -> String {
    let seconds = (unix_ms / 1000) as i64;
    let nanos = ((unix_ms % 1000) * 1_000_000) as u32;
    chrono::DateTime::from_timestamp(seconds, nanos)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| "1970-01-01T00:00:00+00:00".to_string())
}

fn service_api_key() -> Option<String> {
    std::env::var("SMS_SERVICE_API_KEY")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn unauthorized_response() -> (StatusCode, Json<BasicErrorResponse>) {
    (
        StatusCode::UNAUTHORIZED,
        Json(BasicErrorResponse {
            success: false,
            error: "Unauthorized".to_string(),
        }),
    )
}

fn is_authorized(headers: &HeaderMap) -> bool {
    let Some(required_key) = service_api_key() else {
        return true;
    };

    headers
        .get("x-api-key")
        .and_then(|value| value.to_str().ok())
        .map(|value| value == required_key)
        .unwrap_or(false)
}

fn canonicalize_channel(value: &str) -> Result<String, String> {
    let normalized = value.trim().to_lowercase();
    let mapped = match normalized.as_str() {
        "sms" => "sms_twilio",
        "whatsapp" => "whatsapp_twilio",
        "custom" => "custom_http",
        _ => normalized.as_str(),
    };

    if SUPPORTED_CHANNELS.contains(&mapped) {
        Ok(mapped.to_string())
    } else {
        Err(format!("Unsupported SMS verification channel: {value}"))
    }
}

fn build_default_sequence() -> Result<Vec<String>, String> {
    let primary = std::env::var("SMS_DEFAULT_CHANNEL").unwrap_or_else(|_| "sms_twilio".to_string());
    let fallback = std::env::var("SMS_FALLBACK_CHANNELS")
        .unwrap_or_default()
        .split(',')
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();

    let mut sequence = vec![primary];
    sequence.extend(fallback);

    build_sequence(sequence)
}

fn build_sequence(channels: Vec<String>) -> Result<Vec<String>, String> {
    let mut sequence = Vec::new();

    for channel in channels {
        let canonical = canonicalize_channel(&channel)?;
        if !sequence.contains(&canonical) {
            sequence.push(canonical);
        }
    }

    if sequence.is_empty() {
        let default = canonicalize_channel("sms_twilio")?;
        sequence.push(default);
    }

    Ok(sequence)
}

fn normalize_phone_number(phone: &str) -> Result<String, String> {
    let sanitized: String = phone
        .chars()
        .filter(|ch| ch.is_ascii_digit() || *ch == '+')
        .collect();
    if sanitized.is_empty() {
        return Err("Invalid phone number".to_string());
    }

    if sanitized.starts_with('+') {
        return Ok(sanitized);
    }

    let default_country = std::env::var("SMS_DEFAULT_COUNTRY_CODE")
        .ok()
        .map(|value| value.trim().trim_start_matches('+').to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            "SMS_DEFAULT_COUNTRY_CODE is required when sending to bare phone numbers".to_string()
        })?;

    Ok(format!("+{default_country}{sanitized}"))
}

fn normalize_destination(channel: &str, contact: &str) -> Result<String, String> {
    let contact = contact.trim();
    if contact.is_empty() {
        return Err("Contact value is required".to_string());
    }

    let phone_based = matches!(
        channel,
        "sms_twilio" | "sms_http" | "whatsapp_twilio" | "whatsapp_baileys"
    );

    if phone_based {
        normalize_phone_number(contact)
    } else {
        Ok(contact.to_string())
    }
}

fn rate_limit_window_seconds() -> u64 {
    std::env::var("SMS_RATE_LIMIT_WINDOW_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(300)
}

fn rate_limit_max_per_contact() -> u64 {
    std::env::var("SMS_RATE_LIMIT_MAX_PER_CONTACT")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(5)
}

async fn health() -> Json<ServiceStatus> {
    Json(ServiceStatus {
        status: "ok",
        service: SERVICE_NAME,
    })
}

async fn send_sms(
    State(state): State<Arc<Mutex<SmsApiState>>>,
    headers: HeaderMap,
    Json(payload): Json<SendRequest>,
) -> impl IntoResponse {
    if !is_authorized(&headers) {
        return unauthorized_response().into_response();
    }

    if payload.contact.trim().is_empty() || payload.message.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(BasicErrorResponse {
                success: false,
                error: "contact and message are required".to_string(),
            }),
        )
            .into_response();
    }

    let idempotency_header = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let idempotency_key = payload
        .idempotency_key
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or(idempotency_header);

    if idempotency_key
        .as_ref()
        .map(|value| value.len() > 128)
        .unwrap_or(false)
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(BasicErrorResponse {
                success: false,
                error: "Idempotency key too long".to_string(),
            }),
        )
            .into_response();
    }

    let mut state = state.lock().await;
    let contact = payload.contact.clone();

    if let Some(key) = idempotency_key.as_deref() {
        if let Some(existing) = state.find_history_by_idempotency(key) {
            if existing.status == "success" {
                return (
                    StatusCode::OK,
                    Json(SendResponse {
                        success: true,
                        channel: existing.channel.clone(),
                        destination: existing.destination.clone(),
                        idempotent: Some(true),
                    }),
                )
                    .into_response();
            }
        }
    }

    let max_per_contact = rate_limit_max_per_contact();
    if max_per_contact > 0 {
        let recent = state.count_recent_for_contact(&contact, rate_limit_window_seconds());
        if recent >= max_per_contact as usize {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                Json(BasicErrorResponse {
                    success: false,
                    error: "Rate limit exceeded for this contact".to_string(),
                }),
            )
                .into_response();
        }
    }

    state.record_request();
    let request_id = state.next_request_id();
    let started_at_ms = now_unix_epoch_ms();

    let channels_result = if let Some(channels) = payload.channels.clone() {
        if channels.is_empty() {
            build_default_sequence()
        } else {
            build_sequence(channels)
        }
    } else {
        build_default_sequence()
    };

    let channels = match channels_result {
        Ok(channels) => channels,
        Err(error) => {
            state.record_failure("unknown");
            let history_id = state.next_history_id();
            state.push_history(HistoryEntry {
                id: history_id,
                request_id,
                idempotency_key,
                contact,
                destination: "unknown".to_string(),
                channel: "unknown".to_string(),
                status: "failed".to_string(),
                error: Some(error.clone()),
                metadata: payload.metadata,
                created_at_ms: now_unix_epoch_ms(),
            });

            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(BasicErrorResponse {
                    success: false,
                    error,
                }),
            )
                .into_response();
        }
    };

    let channel = channels
        .first()
        .cloned()
        .unwrap_or_else(|| "sms_twilio".to_string());

    match normalize_destination(&channel, &contact) {
        Ok(destination) => {
            let duration_ms = now_unix_epoch_ms().saturating_sub(started_at_ms);
            state.record_success(&channel, duration_ms);
            let history_id = state.next_history_id();
            state.push_history(HistoryEntry {
                id: history_id,
                request_id,
                idempotency_key,
                contact,
                destination: destination.clone(),
                channel: channel.clone(),
                status: "success".to_string(),
                error: None,
                metadata: payload.metadata,
                created_at_ms: now_unix_epoch_ms(),
            });

            (
                StatusCode::OK,
                Json(SendResponse {
                    success: true,
                    channel,
                    destination,
                    idempotent: None,
                }),
            )
                .into_response()
        }
        Err(error) => {
            state.record_failure(&channel);
            let history_id = state.next_history_id();
            state.push_history(HistoryEntry {
                id: history_id,
                request_id,
                idempotency_key,
                contact: contact.clone(),
                destination: contact,
                channel,
                status: "failed".to_string(),
                error: Some(error.clone()),
                metadata: payload.metadata,
                created_at_ms: now_unix_epoch_ms(),
            });

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(BasicErrorResponse {
                    success: false,
                    error,
                }),
            )
                .into_response()
        }
    }
}

async fn metrics(
    State(state): State<Arc<Mutex<SmsApiState>>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if !is_authorized(&headers) {
        return unauthorized_response().into_response();
    }

    let state = state.lock().await;
    let _ = state.started_at.elapsed();

    (
        StatusCode::OK,
        Json(MetricsResponse {
            success: true,
            metrics: state.metrics_snapshot(),
            history: state.history_stats(),
        }),
    )
        .into_response()
}

async fn history(
    State(state): State<Arc<Mutex<SmsApiState>>>,
    headers: HeaderMap,
    Query(query): Query<HistoryQuery>,
) -> impl IntoResponse {
    if !is_authorized(&headers) {
        return unauthorized_response().into_response();
    }

    let state = state.lock().await;
    let limit = query
        .limit
        .unwrap_or(HISTORY_DEFAULT_LIMIT)
        .min(HISTORY_MAX_LIMIT)
        .max(1);

    let entries = state
        .history
        .iter()
        .rev()
        .take(limit)
        .map(|entry| HistoryEntryPayload {
            id: entry.id,
            request_id: entry.request_id.clone(),
            idempotency_key: entry.idempotency_key.clone(),
            contact: entry.contact.clone(),
            destination: entry.destination.clone(),
            channel: entry.channel.clone(),
            status: entry.status.clone(),
            error: entry.error.clone(),
            metadata: entry.metadata.clone(),
            created_at: created_at_iso_ms(entry.created_at_ms),
        })
        .collect::<Vec<_>>();

    (
        StatusCode::OK,
        Json(HistoryResponse {
            success: true,
            entries,
        }),
    )
        .into_response()
}

fn listen_port(default_port: u16) -> u16 {
    std::env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(default_port)
}

fn build_router(state: Arc<Mutex<SmsApiState>>) -> Router {
    Router::new()
        .route("/api/send", post(send_sms))
        .route("/metrics", get(metrics))
        .route("/history", get(history))
        .route("/health", get(health))
        .with_state(state)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let app = build_router(Arc::new(Mutex::new(SmsApiState::new())));

    let addr = SocketAddr::from(([0, 0, 0, 0], listen_port(DEFAULT_PORT)));
    tracing::info!(service = SERVICE_NAME, %addr, "startup");

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .expect("server failed");
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Method, Request};
    use hyper::body::to_bytes;
    use serde_json::{json, Value};
    use serial_test::serial;
    use tower::ServiceExt;

    fn test_app() -> Router {
        build_router(Arc::new(Mutex::new(SmsApiState::new())))
    }

    async fn request(app: &Router, method: Method, uri: &str, body: Value) -> (StatusCode, Value) {
        let request = Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .expect("request should build");

        let response = app
            .clone()
            .oneshot(request)
            .await
            .expect("request should execute");
        let status = response.status();
        let bytes = to_bytes(response.into_body())
            .await
            .expect("body should read");
        let payload = serde_json::from_slice::<Value>(&bytes).expect("json should parse");
        (status, payload)
    }

    async fn get(app: &Router, uri: &str) -> (StatusCode, Value) {
        let request = Request::builder()
            .method(Method::GET)
            .uri(uri)
            .body(Body::empty())
            .expect("request should build");

        let response = app
            .clone()
            .oneshot(request)
            .await
            .expect("request should execute");
        let status = response.status();
        let bytes = to_bytes(response.into_body())
            .await
            .expect("body should read");
        let payload = serde_json::from_slice::<Value>(&bytes).expect("json should parse");
        (status, payload)
    }

    fn sample_payload(contact: &str, idempotency_key: Option<&str>) -> Value {
        json!({
            "contact": contact,
            "message": "Your Universus code is 123456",
            "channels": ["sms", "telegram"],
            "metadata": { "userId": 42 },
            "idempotencyKey": idempotency_key
        })
    }

    #[tokio::test]
    #[serial]
    async fn send_accepts_legacy_payload_shape() {
        std::env::set_var("SMS_DEFAULT_COUNTRY_CODE", "1");
        let app = test_app();

        let (status, payload) = request(
            &app,
            Method::POST,
            "/api/send",
            sample_payload("+12065550123", None),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(payload["success"], true);
        assert_eq!(payload["channel"], "sms_twilio");
        assert_eq!(payload["destination"], "+12065550123");
    }

    #[tokio::test]
    #[serial]
    async fn send_replays_idempotent_success() {
        std::env::set_var("SMS_DEFAULT_COUNTRY_CODE", "1");
        let app = test_app();

        let _ = request(
            &app,
            Method::POST,
            "/api/send",
            sample_payload("+12065550123", Some("idem-1")),
        )
        .await;

        let (status, replay) = request(
            &app,
            Method::POST,
            "/api/send",
            sample_payload("+12065550123", Some("idem-1")),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(replay["success"], true);
        assert_eq!(replay["idempotent"], true);
    }

    #[tokio::test]
    #[serial]
    async fn metrics_and_history_match_legacy_shape() {
        std::env::set_var("SMS_DEFAULT_COUNTRY_CODE", "1");
        let app = test_app();

        let _ = request(
            &app,
            Method::POST,
            "/api/send",
            sample_payload("+12065550123", None),
        )
        .await;

        let (metrics_status, metrics_payload) = get(&app, "/metrics").await;
        assert_eq!(metrics_status, StatusCode::OK);
        assert_eq!(metrics_payload["success"], true);
        assert_eq!(metrics_payload["metrics"]["requests"], 1);
        assert!(metrics_payload["history"].is_array());

        let (history_status, history_payload) = get(&app, "/history?limit=1").await;
        assert_eq!(history_status, StatusCode::OK);
        assert_eq!(history_payload["success"], true);
        assert_eq!(history_payload["entries"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    #[serial]
    async fn health_reports_legacy_service_name() {
        let app = test_app();

        let (status, payload) = get(&app, "/health").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(payload["status"], "ok");
        assert_eq!(payload["service"], "sms");
    }

    #[tokio::test]
    #[serial]
    async fn send_requires_api_key_when_configured() {
        std::env::set_var("SMS_SERVICE_API_KEY", "secret");
        std::env::set_var("SMS_DEFAULT_COUNTRY_CODE", "1");
        let app = test_app();

        let (status, payload) = request(
            &app,
            Method::POST,
            "/api/send",
            sample_payload("+12065550123", None),
        )
        .await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(payload["success"], false);
        assert_eq!(payload["error"], "Unauthorized");

        std::env::remove_var("SMS_SERVICE_API_KEY");
    }
}
