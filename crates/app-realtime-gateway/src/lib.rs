use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::extract::{Query, State};
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

struct RealtimeState {
    subscriptions: HashMap<String, HashSet<String>>,
    publish_sequence: u64,
    notifications: Vec<NotificationItem>,
    online_players: Vec<OnlinePlayer>,
    trade_offers: Vec<TradeOfferItem>,
}

impl Default for RealtimeState {
    fn default() -> Self {
        Self {
            subscriptions: HashMap::new(),
            publish_sequence: 0,
            notifications: vec![
                NotificationItem {
                    id: 1,
                    category: "trade".to_string(),
                    message: "Your offer was accepted".to_string(),
                    read: false,
                    created_at: "2026-02-13T12:00:00Z".to_string(),
                },
                NotificationItem {
                    id: 2,
                    category: "chat".to_string(),
                    message: "New alliance announcement".to_string(),
                    read: true,
                    created_at: "2026-02-12T08:00:00Z".to_string(),
                },
            ],
            online_players: vec![
                OnlinePlayer {
                    user_id: 7,
                    username: "Commander".to_string(),
                    status: "online".to_string(),
                    alliance_id: Some(10),
                    alliance_tag: Some("UNI".to_string()),
                },
                OnlinePlayer {
                    user_id: 11,
                    username: "Scout".to_string(),
                    status: "online".to_string(),
                    alliance_id: Some(15),
                    alliance_tag: Some("RIM".to_string()),
                },
            ],
            trade_offers: vec![
                TradeOfferItem {
                    id: 100,
                    status: "active".to_string(),
                    resource_offered: "metal".to_string(),
                    amount_offered: 10000,
                    resource_wanted: "crystal".to_string(),
                    amount_wanted: 5000,
                    seller_username: "Commander".to_string(),
                },
                TradeOfferItem {
                    id: 101,
                    status: "completed".to_string(),
                    resource_offered: "deuterium".to_string(),
                    amount_offered: 2000,
                    resource_wanted: "metal".to_string(),
                    amount_wanted: 8000,
                    seller_username: "Trader".to_string(),
                },
            ],
        }
    }
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

#[derive(Serialize)]
struct ChatChannelsResponse {
    channels: Vec<ChannelInfo>,
}

#[derive(Clone, Serialize)]
struct NotificationItem {
    id: u64,
    category: String,
    message: String,
    read: bool,
    created_at: String,
}

#[derive(Serialize)]
struct NotificationsResponse {
    notifications: Vec<NotificationItem>,
    total: usize,
}

#[derive(Default, Deserialize)]
struct NotificationsQuery {
    #[serde(rename = "unreadOnly")]
    unread_only: Option<bool>,
    category: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
}

#[derive(Clone, Serialize)]
struct OnlinePlayer {
    user_id: u64,
    username: String,
    status: String,
    alliance_id: Option<u64>,
    alliance_tag: Option<String>,
}

#[derive(Serialize)]
struct OnlinePlayersResponse {
    players: Vec<OnlinePlayer>,
    count: usize,
}

#[derive(Default, Deserialize)]
struct OnlinePlayersQuery {
    limit: Option<usize>,
    #[serde(rename = "allianceId")]
    alliance_id: Option<u64>,
}

#[derive(Clone, Serialize)]
struct TradeOfferItem {
    id: u64,
    status: String,
    resource_offered: String,
    amount_offered: u64,
    resource_wanted: String,
    amount_wanted: u64,
    seller_username: String,
}

#[derive(Serialize)]
struct TradeOffersResponse {
    offers: Vec<TradeOfferItem>,
    total: usize,
}

#[derive(Default, Deserialize)]
struct TradeOffersQuery {
    status: Option<String>,
    #[serde(rename = "resourceOffered")]
    resource_offered: Option<String>,
    #[serde(rename = "resourceWanted")]
    resource_wanted: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
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
        .route("/chat/channels", get(rest_chat_channels))
        .route("/notifications", get(rest_notifications))
        .route("/players/online", get(rest_players_online))
        .route("/trade/offers", get(rest_trade_offers))
        .route("/api/realtime/chat/channels", get(rest_chat_channels))
        .route("/api/realtime/notifications", get(rest_notifications))
        .route("/api/realtime/players/online", get(rest_players_online))
        .route("/api/realtime/trade/offers", get(rest_trade_offers))
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

async fn rest_chat_channels(State(state): State<AppState>) -> Json<ChatChannelsResponse> {
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

    Json(ChatChannelsResponse { channels })
}

async fn rest_notifications(
    State(state): State<AppState>,
    Query(query): Query<NotificationsQuery>,
) -> Json<NotificationsResponse> {
    let store = state.inner.lock().expect("state lock poisoned");
    let mut notifications = store.notifications.clone();

    if query.unread_only == Some(true) {
        notifications.retain(|item| !item.read);
    }

    if let Some(category) = query.category {
        notifications.retain(|item| item.category == category);
    }

    let total = notifications.len();
    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(50);
    let paged = notifications.into_iter().skip(offset).take(limit).collect();

    Json(NotificationsResponse {
        notifications: paged,
        total,
    })
}

async fn rest_players_online(
    State(state): State<AppState>,
    Query(query): Query<OnlinePlayersQuery>,
) -> Json<OnlinePlayersResponse> {
    let store = state.inner.lock().expect("state lock poisoned");
    let mut players = store.online_players.clone();

    if let Some(alliance_id) = query.alliance_id {
        players.retain(|player| player.alliance_id == Some(alliance_id));
    }

    let limit = query.limit.unwrap_or(100);
    let players: Vec<OnlinePlayer> = players.into_iter().take(limit).collect();
    let count = players.len();

    Json(OnlinePlayersResponse { players, count })
}

async fn rest_trade_offers(
    State(state): State<AppState>,
    Query(query): Query<TradeOffersQuery>,
) -> Json<TradeOffersResponse> {
    let store = state.inner.lock().expect("state lock poisoned");
    let mut offers = store.trade_offers.clone();
    let status = query.status.unwrap_or_else(|| "active".to_string());

    offers.retain(|offer| offer.status == status);

    if let Some(resource_offered) = query.resource_offered {
        offers.retain(|offer| offer.resource_offered == resource_offered);
    }

    if let Some(resource_wanted) = query.resource_wanted {
        offers.retain(|offer| offer.resource_wanted == resource_wanted);
    }

    let total = offers.len();
    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(50);
    let offers = offers.into_iter().skip(offset).take(limit).collect();

    Json(TradeOffersResponse { offers, total })
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
