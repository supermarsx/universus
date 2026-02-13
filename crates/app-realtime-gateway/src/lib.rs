use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

pub const SERVICE_NAME: &str = "app-realtime-gateway";
pub const DEFAULT_PORT: u16 = 3004;

#[derive(Clone)]
struct AppState {
    inner: Arc<Mutex<RealtimeState>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(RealtimeState::default())),
        }
    }
}

#[derive(Default)]
struct RealtimeState {
    subscriptions: HashMap<String, HashSet<String>>,
    publish_sequence: u64,
}

#[derive(Serialize)]
struct ServiceStatus {
    status: &'static str,
    service: &'static str,
}

#[derive(Serialize)]
struct WebSocketInfo {
    service: &'static str,
    websocket: bool,
    endpoint: &'static str,
    formats: Vec<&'static str>,
}

#[derive(Serialize)]
struct Envelope<T> {
    status: &'static str,
    data: T,
}

#[derive(Serialize)]
struct ErrorEnvelope {
    status: &'static str,
    error: String,
}

#[derive(Serialize)]
struct ChannelsPayload {
    channels: Vec<ChannelInfo>,
}

#[derive(Serialize)]
struct ChannelInfo {
    name: String,
    subscriber_count: usize,
}

#[derive(Deserialize)]
struct SubscribeRequest {
    channel: String,
    subscriber_id: String,
}

#[derive(Serialize)]
struct SubscribePayload {
    channel: String,
    subscriber_id: String,
    subscriber_count: usize,
}

#[derive(Deserialize)]
struct PublishRequest {
    channel: String,
    event: String,
}

#[derive(Serialize)]
struct PublishPayload {
    channel: String,
    event: String,
    delivered_to: usize,
    publish_sequence: u64,
}

pub fn build_router() -> Router {
    let state = AppState::default();

    Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/ws-info", get(ws_info))
        .route("/api/realtime/channels", get(list_channels))
        .route("/api/realtime/subscribe", post(subscribe))
        .route("/api/realtime/publish", post(publish))
        .with_state(state)
}

pub fn listen_port(default_port: u16) -> u16 {
    std::env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(default_port)
}

pub async fn serve() {
    tracing_subscriber::fmt::init();

    let addr = SocketAddr::from(([0, 0, 0, 0], listen_port(DEFAULT_PORT)));
    tracing::info!(service = SERVICE_NAME, %addr, "startup");

    axum::Server::bind(&addr)
        .serve(build_router().into_make_service())
        .await
        .expect("server failed");
}

async fn health() -> Json<ServiceStatus> {
    Json(ServiceStatus {
        status: "ok",
        service: SERVICE_NAME,
    })
}

async fn ready() -> Json<ServiceStatus> {
    Json(ServiceStatus {
        status: "ok",
        service: SERVICE_NAME,
    })
}

async fn ws_info() -> Json<WebSocketInfo> {
    Json(WebSocketInfo {
        service: SERVICE_NAME,
        websocket: true,
        endpoint: "/ws",
        formats: vec!["json"],
    })
}

async fn list_channels(State(state): State<AppState>) -> Json<Envelope<ChannelsPayload>> {
    let store = state.inner.lock().expect("state lock poisoned");

    let mut channels: Vec<ChannelInfo> = store
        .subscriptions
        .iter()
        .map(|(name, subscribers)| ChannelInfo {
            name: name.clone(),
            subscriber_count: subscribers.len(),
        })
        .collect();

    channels.sort_by(|left, right| left.name.cmp(&right.name));

    Json(Envelope {
        status: "ok",
        data: ChannelsPayload { channels },
    })
}

async fn subscribe(
    State(state): State<AppState>,
    Json(request): Json<SubscribeRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    if request.channel.trim().is_empty() || request.subscriber_id.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!(ErrorEnvelope {
                status: "error",
                error: "channel and subscriber_id are required".to_string(),
            })),
        );
    }

    let mut store = state.inner.lock().expect("state lock poisoned");
    let subscribers = store
        .subscriptions
        .entry(request.channel.clone())
        .or_default();
    subscribers.insert(request.subscriber_id.clone());

    (
        StatusCode::OK,
        Json(serde_json::json!(Envelope {
            status: "ok",
            data: SubscribePayload {
                channel: request.channel,
                subscriber_id: request.subscriber_id,
                subscriber_count: subscribers.len(),
            },
        })),
    )
}

async fn publish(
    State(state): State<AppState>,
    Json(request): Json<PublishRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    if request.channel.trim().is_empty() || request.event.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!(ErrorEnvelope {
                status: "error",
                error: "channel and event are required".to_string(),
            })),
        );
    }

    let mut store = state.inner.lock().expect("state lock poisoned");
    let delivered_to = store
        .subscriptions
        .get(&request.channel)
        .map_or(0, HashSet::len);

    store.publish_sequence += 1;

    (
        StatusCode::OK,
        Json(serde_json::json!(Envelope {
            status: "ok",
            data: PublishPayload {
                channel: request.channel,
                event: request.event,
                delivered_to,
                publish_sequence: store.publish_sequence,
            },
        })),
    )
}
