use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

const SERVICE_NAME: &str = "app-sms-api";
const DEFAULT_PORT: u16 = 3003;
const HISTORY_LIMIT: usize = 100;
const DEFAULT_HISTORY_PAGE_SIZE: usize = 20;

#[derive(Serialize)]
struct ServiceStatus {
    status: &'static str,
    service: &'static str,
}

#[derive(Serialize)]
struct ReadinessStatus {
    status: &'static str,
    service: &'static str,
    dependencies: Vec<ComponentStatus>,
}

#[derive(Serialize)]
struct ComponentStatus {
    name: &'static str,
    status: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct SmsMessage {
    to: String,
    from: String,
    body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct EnvelopeMetadata {
    campaign_id: String,
    tags: Vec<String>,
    priority: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct SendEnvelope {
    tenant_id: String,
    channel: String,
    idempotency_key: String,
    message: SmsMessage,
    metadata: EnvelopeMetadata,
}

#[derive(Serialize)]
struct SendAcceptedResponse {
    status: &'static str,
    service: &'static str,
    request_id: String,
    accepted_at_ms: u128,
    envelope: SendEnvelope,
}

#[derive(Clone, Serialize)]
struct SendAcceptedItem {
    request_id: String,
    accepted_at_ms: u128,
    envelope: SendEnvelope,
}

#[derive(Serialize)]
struct BulkSendAcceptedResponse {
    status: &'static str,
    service: &'static str,
    count: usize,
    items: Vec<SendAcceptedItem>,
}

#[derive(Clone, Serialize)]
struct HistoryItem {
    request_id: String,
    accepted_at_ms: u128,
    envelope: SendEnvelope,
}

#[derive(Serialize)]
struct HistoryResponse {
    status: &'static str,
    service: &'static str,
    total: usize,
    items: Vec<HistoryItem>,
}

#[derive(Serialize)]
struct MetricsResponse {
    status: &'static str,
    service: &'static str,
    uptime_seconds: u64,
    total_requests: u64,
    stored_history: usize,
    last_request_id: Option<String>,
}

#[derive(Serialize)]
struct ErrorResponse {
    status: &'static str,
    service: &'static str,
    error: String,
}

#[derive(Deserialize)]
struct HistoryQuery {
    limit: Option<usize>,
}

#[derive(Deserialize)]
struct BulkSendRequest {
    items: Vec<SendEnvelope>,
}

struct SmsApiState {
    started_at: Instant,
    total_requests: u64,
    next_request_sequence: u64,
    history: Vec<HistoryItem>,
}

impl SmsApiState {
    fn new() -> Self {
        Self {
            started_at: Instant::now(),
            total_requests: 0,
            next_request_sequence: 0,
            history: Vec::new(),
        }
    }
}

async fn health() -> Json<ServiceStatus> {
    Json(ServiceStatus {
        status: "ok",
        service: SERVICE_NAME,
    })
}

async fn ready() -> Json<ReadinessStatus> {
    Json(ReadinessStatus {
        status: "ok",
        service: SERVICE_NAME,
        dependencies: vec![
            ComponentStatus {
                name: "sms-gateway",
                status: "ok",
            },
            ComponentStatus {
                name: "delivery-audit-store",
                status: "ok",
            },
        ],
    })
}

fn next_request_id(sequence: u64) -> String {
    format!("sms-req-{sequence:016}")
}

fn now_unix_epoch_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

async fn send_sms_handler(
    State(state): State<Arc<Mutex<SmsApiState>>>,
    Json(envelope): Json<SendEnvelope>,
) -> Json<SendAcceptedResponse> {
    let mut state = state.lock().await;
    let accepted = accept_envelope(&mut state, envelope.clone());

    Json(SendAcceptedResponse {
        status: "accepted",
        service: SERVICE_NAME,
        request_id: accepted.request_id,
        accepted_at_ms: accepted.accepted_at_ms,
        envelope: accepted.envelope,
    })
}

async fn send_bulk_handler(
    State(state): State<Arc<Mutex<SmsApiState>>>,
    Json(request): Json<BulkSendRequest>,
) -> (StatusCode, Json<BulkSendAcceptedResponse>) {
    if request.items.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(BulkSendAcceptedResponse {
                status: "rejected",
                service: SERVICE_NAME,
                count: 0,
                items: Vec::new(),
            }),
        );
    }

    let mut state = state.lock().await;
    let mut items = Vec::with_capacity(request.items.len());
    for envelope in request.items {
        let accepted = accept_envelope(&mut state, envelope);
        items.push(accepted);
    }

    (
        StatusCode::ACCEPTED,
        Json(BulkSendAcceptedResponse {
            status: "accepted",
            service: SERVICE_NAME,
            count: items.len(),
            items,
        }),
    )
}

async fn get_send_request(
    State(state): State<Arc<Mutex<SmsApiState>>>,
    Path(request_id): Path<String>,
) -> impl IntoResponse {
    let state = state.lock().await;
    let Some(item) = state.history.iter().find(|entry| entry.request_id == request_id) else {
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                status: "not_found",
                service: SERVICE_NAME,
                error: "request_id not found".to_string(),
            }),
        )
            .into_response();
    };

    (
        StatusCode::OK,
        Json(SendAcceptedResponse {
            status: "accepted",
            service: SERVICE_NAME,
            request_id: item.request_id.clone(),
            accepted_at_ms: item.accepted_at_ms,
            envelope: item.envelope.clone(),
        }),
    )
        .into_response()
}

async fn metrics(State(state): State<Arc<Mutex<SmsApiState>>>) -> Json<MetricsResponse> {
    let state = state.lock().await;
    Json(MetricsResponse {
        status: "ok",
        service: SERVICE_NAME,
        uptime_seconds: state.started_at.elapsed().as_secs(),
        total_requests: state.total_requests,
        stored_history: state.history.len(),
        last_request_id: state.history.last().map(|item| item.request_id.clone()),
    })
}

async fn history(
    State(state): State<Arc<Mutex<SmsApiState>>>,
    Query(query): Query<HistoryQuery>,
) -> Json<HistoryResponse> {
    let state = state.lock().await;
    let page_size = query.limit.unwrap_or(DEFAULT_HISTORY_PAGE_SIZE).max(1);
    let items = state
        .history
        .iter()
        .rev()
        .take(page_size)
        .cloned()
        .collect::<Vec<_>>();

    Json(HistoryResponse {
        status: "ok",
        service: SERVICE_NAME,
        total: state.history.len(),
        items,
    })
}

fn listen_port(default_port: u16) -> u16 {
    std::env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(default_port)
}

fn build_router(state: Arc<Mutex<SmsApiState>>) -> Router {
    Router::new()
        .route("/api/send", post(send_sms_handler))
        .route("/api/send/bulk", post(send_bulk_handler))
        .route("/api/send/:request_id", get(get_send_request))
        .route("/metrics", get(metrics))
        .route("/history", get(history))
        .route("/health", get(health))
        .route("/ready", get(ready))
        .with_state(state)
}

fn accept_envelope(state: &mut SmsApiState, envelope: SendEnvelope) -> SendAcceptedItem {
    state.total_requests += 1;
    state.next_request_sequence += 1;

    let request_id = next_request_id(state.next_request_sequence);
    let accepted_at_ms = now_unix_epoch_ms();

    state.history.push(HistoryItem {
        request_id: request_id.clone(),
        accepted_at_ms,
        envelope: envelope.clone(),
    });

    if state.history.len() > HISTORY_LIMIT {
        state.history.remove(0);
    }

    SendAcceptedItem {
        request_id,
        accepted_at_ms,
        envelope,
    }
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
    use axum::http::{Method, Request, StatusCode};
    use hyper::body::to_bytes;
    use serde_json::{json, Value};
    use tower::ServiceExt;

    fn test_app() -> Router {
        build_router(Arc::new(Mutex::new(SmsApiState::new())))
    }

    fn sample_envelope(suffix: &str) -> Value {
        json!({
            "tenant_id": "tenant-us-east-1",
            "channel": "sms",
            "idempotency_key": format!("idem-{suffix}"),
            "message": {
                "to": "+12065550123",
                "from": "+12065550000",
                "body": format!("Campaign ping {suffix}")
            },
            "metadata": {
                "campaign_id": "cmp-winter-2026",
                "tags": ["promo", "vip"],
                "priority": "high"
            }
        })
    }

    async fn json_response(
        app: &Router,
        method: Method,
        uri: &str,
        body: Value,
    ) -> (StatusCode, Value) {
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
        let payload = serde_json::from_slice::<Value>(&bytes).expect("json response should parse");
        (status, payload)
    }

    async fn get_json(app: &Router, uri: &str) -> (StatusCode, Value) {
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
        let payload = serde_json::from_slice::<Value>(&bytes).expect("json response should parse");
        (status, payload)
    }

    #[tokio::test]
    async fn send_assigns_deterministic_request_ids() {
        let app = test_app();

        let (_, first) =
            json_response(&app, Method::POST, "/api/send", sample_envelope("001")).await;
        let (_, second) =
            json_response(&app, Method::POST, "/api/send", sample_envelope("002")).await;

        assert_eq!(first["status"], "accepted");
        assert_eq!(first["request_id"], "sms-req-0000000000000001");
        assert_eq!(second["request_id"], "sms-req-0000000000000002");
    }

    #[tokio::test]
    async fn metrics_reflects_request_counts() {
        let app = test_app();

        let _ = json_response(&app, Method::POST, "/api/send", sample_envelope("001")).await;
        let _ = json_response(&app, Method::POST, "/api/send", sample_envelope("002")).await;
        let (status, payload) = get_json(&app, "/metrics").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(payload["total_requests"], 2);
        assert_eq!(payload["stored_history"], 2);
        assert_eq!(payload["last_request_id"], "sms-req-0000000000000002");
    }

    #[tokio::test]
    async fn history_supports_limit_parameter() {
        let app = test_app();

        let _ = json_response(&app, Method::POST, "/api/send", sample_envelope("001")).await;
        let _ = json_response(&app, Method::POST, "/api/send", sample_envelope("002")).await;
        let _ = json_response(&app, Method::POST, "/api/send", sample_envelope("003")).await;
        let (status, payload) = get_json(&app, "/history?limit=2").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(payload["total"], 3);
        assert_eq!(
            payload["items"]
                .as_array()
                .expect("items should be an array")
                .len(),
            2
        );
        assert_eq!(
            payload["items"][0]["request_id"],
            "sms-req-0000000000000003"
        );
        assert_eq!(
            payload["items"][1]["request_id"],
            "sms-req-0000000000000002"
        );
    }

    #[tokio::test]
    async fn health_and_ready_are_available() {
        let app = test_app();

        let (health_status, health_payload) = get_json(&app, "/health").await;
        let (ready_status, ready_payload) = get_json(&app, "/ready").await;

        assert_eq!(health_status, StatusCode::OK);
        assert_eq!(health_payload["service"], SERVICE_NAME);
        assert_eq!(ready_status, StatusCode::OK);
        assert_eq!(
            ready_payload["dependencies"]
                .as_array()
                .expect("dependencies should be an array")
                .len(),
            2
        );
    }

    #[tokio::test]
    async fn bulk_send_assigns_deterministic_ids_in_order() {
        let app = test_app();
        let payload = json!({
            "items": [
                sample_envelope("010"),
                sample_envelope("011"),
                sample_envelope("012")
            ]
        });

        let (status, response) = json_response(&app, Method::POST, "/api/send/bulk", payload).await;
        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(response["status"], "accepted");
        assert_eq!(response["count"], 3);
        assert_eq!(response["items"][0]["request_id"], "sms-req-0000000000000001");
        assert_eq!(response["items"][1]["request_id"], "sms-req-0000000000000002");
        assert_eq!(response["items"][2]["request_id"], "sms-req-0000000000000003");
    }

    #[tokio::test]
    async fn send_lookup_returns_existing_request() {
        let app = test_app();
        let (_, sent) = json_response(&app, Method::POST, "/api/send", sample_envelope("020")).await;
        let request_id = sent["request_id"].as_str().expect("request id should exist");

        let (status, lookup) = get_json(&app, &format!("/api/send/{request_id}")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(lookup["request_id"], request_id);
        assert_eq!(lookup["envelope"]["idempotency_key"], "idem-020");
    }

    #[tokio::test]
    async fn send_lookup_returns_not_found_for_missing_request() {
        let app = test_app();

        let (status, payload) = get_json(&app, "/api/send/sms-req-0000000000009999").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(payload["status"], "not_found");
        assert_eq!(payload["error"], "request_id not found");
    }

    #[tokio::test]
    async fn bulk_send_updates_metrics_and_history() {
        let app = test_app();
        let _ = json_response(&app, Method::POST, "/api/send", sample_envelope("030")).await;
        let bulk_payload = json!({
            "items": [
                sample_envelope("031"),
                sample_envelope("032")
            ]
        });
        let _ = json_response(&app, Method::POST, "/api/send/bulk", bulk_payload).await;

        let (metrics_status, metrics_payload) = get_json(&app, "/metrics").await;
        assert_eq!(metrics_status, StatusCode::OK);
        assert_eq!(metrics_payload["total_requests"], 3);
        assert_eq!(metrics_payload["stored_history"], 3);
        assert_eq!(metrics_payload["last_request_id"], "sms-req-0000000000000003");

        let (history_status, history_payload) = get_json(&app, "/history?limit=3").await;
        assert_eq!(history_status, StatusCode::OK);
        assert_eq!(history_payload["total"], 3);
        assert_eq!(history_payload["items"][0]["request_id"], "sms-req-0000000000000003");
        assert_eq!(history_payload["items"][2]["request_id"], "sms-req-0000000000000001");
    }
}
