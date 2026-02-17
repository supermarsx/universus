use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, post};
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
    notification_preferences: HashMap<String, bool>,
    online_players: Vec<OnlinePlayer>,
    trade_offers: Vec<TradeOfferItem>,
    trade_history: Vec<TradeHistoryItem>,
    chat_conversations: Vec<ChatConversationItem>,
    chat_messages: HashMap<String, ChatRealtimeMessage>,
    recent_events: Vec<RecentEventItem>,
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
            notification_preferences: HashMap::from([
                ("trade".to_string(), true),
                ("chat".to_string(), true),
                ("combat".to_string(), true),
            ]),
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
            trade_history: vec![TradeHistoryItem {
                id: 501,
                offer_id: 101,
                seller_username: "Trader".to_string(),
                buyer_username: "Commander".to_string(),
                resource_offered: "deuterium".to_string(),
                amount_offered: 2000,
                resource_wanted: "metal".to_string(),
                amount_wanted: 8000,
                completed_at: "2026-02-13T09:00:00Z".to_string(),
            }],
            chat_conversations: vec![
                ChatConversationItem {
                    id: "conv-1".to_string(),
                    participant: "Scout".to_string(),
                    unread_count: 2,
                },
                ChatConversationItem {
                    id: "conv-2".to_string(),
                    participant: "AdmiralNova".to_string(),
                    unread_count: 0,
                },
            ],
            chat_messages: HashMap::from([
                (
                    "msg-1".to_string(),
                    ChatRealtimeMessage {
                        id: "msg-1".to_string(),
                        channel_id: 1,
                        user_id: 11,
                        message: "Ping from frontier".to_string(),
                        edited: false,
                        deleted: false,
                        pinned: false,
                        announcement: false,
                        flags: 0,
                        reactions: HashMap::new(),
                    },
                ),
                (
                    "msg-2".to_string(),
                    ChatRealtimeMessage {
                        id: "msg-2".to_string(),
                        channel_id: 1,
                        user_id: 7,
                        message: "Acknowledged".to_string(),
                        edited: false,
                        deleted: false,
                        pinned: false,
                        announcement: false,
                        flags: 0,
                        reactions: HashMap::new(),
                    },
                ),
            ]),
            recent_events: Vec::new(),
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

#[derive(Clone, Serialize)]
struct TradeHistoryItem {
    id: u64,
    offer_id: u64,
    seller_username: String,
    buyer_username: String,
    resource_offered: String,
    amount_offered: u64,
    resource_wanted: String,
    amount_wanted: u64,
    completed_at: String,
}

#[derive(Serialize)]
struct TradeHistoryResponse {
    entries: Vec<TradeHistoryItem>,
    total: usize,
}

#[derive(Default, Deserialize)]
struct TradeHistoryQuery {
    limit: Option<usize>,
    offset: Option<usize>,
}

#[derive(Clone, Serialize)]
struct ChatConversationItem {
    id: String,
    participant: String,
    unread_count: u64,
}

#[derive(Serialize)]
struct ConversationsResponse {
    conversations: Vec<ChatConversationItem>,
    total: usize,
}

#[derive(Clone, Serialize)]
struct ChatMessageItem {
    id: String,
    conversation_id: String,
    sender: String,
    text: String,
    sent_at: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatRealtimeMessage {
    id: String,
    channel_id: i64,
    user_id: u64,
    message: String,
    edited: bool,
    deleted: bool,
    pinned: bool,
    announcement: bool,
    flags: u64,
    reactions: HashMap<String, u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EditChatMessageRequest {
    user_id: Option<u64>,
    message: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteChatMessageRequest {
    user_id: Option<u64>,
    is_admin: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FlagChatMessageRequest {
    user_id: Option<u64>,
    reason: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PinChatMessageRequest {
    user_id: Option<u64>,
    is_pinned: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReactionChatMessageRequest {
    user_id: Option<u64>,
    reaction_type: Option<String>,
}

#[derive(Serialize)]
struct ConversationMessagesResponse {
    conversation_id: String,
    messages: Vec<ChatMessageItem>,
    total: usize,
}

#[derive(Serialize)]
struct UnreadCountResponse {
    unread_count: u64,
}

#[derive(Serialize)]
struct NotificationPreferencesResponse {
    preferences: HashMap<String, bool>,
}

#[derive(Deserialize)]
struct UpdatePreferenceRequest {
    enabled: bool,
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

#[derive(Clone, Serialize)]
struct RecentEventItem {
    channel: String,
    event: String,
    publish_sequence: u64,
}

#[derive(Serialize)]
struct RecentEventsResponse {
    events: Vec<RecentEventItem>,
    total: usize,
}

#[derive(Default, Deserialize)]
struct RecentEventsQuery {
    limit: Option<usize>,
}

#[derive(Clone, Serialize)]
struct ChatRestrictionItem {
    id: i64,
    user_id: i64,
    channel_id: Option<i64>,
    restriction_type: String,
    reason: String,
    restricted_by: i64,
    expires_at_unix: Option<i64>,
    created_at_unix: i64,
}

#[derive(Serialize)]
struct ChatRestrictionsResponse {
    restrictions: Vec<ChatRestrictionItem>,
    total: usize,
}

#[derive(Default, Deserialize)]
struct ChatRestrictionsQuery {
    #[serde(rename = "userId")]
    user_id: Option<i64>,
    #[serde(rename = "channelId")]
    channel_id: Option<i64>,
    limit: Option<i64>,
}

#[derive(Deserialize)]
struct UpsertChatRestrictionRequest {
    #[serde(rename = "userId")]
    user_id: i64,
    #[serde(rename = "channelId")]
    channel_id: Option<i64>,
    #[serde(rename = "restrictionType")]
    restriction_type: String,
    reason: String,
    #[serde(rename = "restrictedBy")]
    restricted_by: i64,
    #[serde(rename = "expiresAtUnix")]
    expires_at_unix: Option<i64>,
}

#[derive(Deserialize)]
struct DeleteChatRestrictionRequest {
    #[serde(rename = "userId")]
    user_id: i64,
    #[serde(rename = "channelId")]
    channel_id: Option<i64>,
    #[serde(rename = "restrictionType")]
    restriction_type: String,
}

pub fn build_router() -> Router {
    let state = AppState::default();

    Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/ws-info", get(ws_info))
        .route("/chat/channels", get(rest_chat_channels))
        .route("/chat/restrictions", get(rest_chat_restrictions))
        .route("/chat/restrictions", post(rest_upsert_chat_restriction))
        .route("/chat/restrictions", delete(rest_delete_chat_restriction))
        .route("/notifications", get(rest_notifications))
        .route(
            "/notifications/unread/count",
            get(rest_notifications_unread_count),
        )
        .route(
            "/notifications/preferences",
            get(rest_notifications_preferences),
        )
        .route(
            "/notifications/preferences/:type_id",
            axum::routing::put(rest_update_notification_preference),
        )
        .route("/chat/conversations", get(rest_chat_conversations))
        .route(
            "/chat/conversations/:conversation_id/messages",
            get(rest_chat_conversation_messages),
        )
        .route(
            "/chat/messages/:message_id",
            axum::routing::put(rest_edit_chat_message),
        )
        .route(
            "/chat/messages/:message_id",
            axum::routing::delete(rest_delete_chat_message),
        )
        .route(
            "/chat/messages/:message_id/flag",
            post(rest_flag_chat_message),
        )
        .route(
            "/chat/messages/:message_id/pin",
            post(rest_pin_chat_message),
        )
        .route(
            "/chat/messages/:message_id/reactions",
            post(rest_react_chat_message),
        )
        .route("/players/online", get(rest_players_online))
        .route("/trade/offers", get(rest_trade_offers))
        .route("/trade/history", get(rest_trade_history))
        .route("/api/realtime/chat/channels", get(rest_chat_channels))
        .route(
            "/api/realtime/chat/restrictions",
            get(rest_chat_restrictions),
        )
        .route(
            "/api/realtime/chat/restrictions",
            post(rest_upsert_chat_restriction),
        )
        .route(
            "/api/realtime/chat/restrictions",
            delete(rest_delete_chat_restriction),
        )
        .route(
            "/api/realtime/chat/conversations",
            get(rest_chat_conversations),
        )
        .route(
            "/api/realtime/chat/conversations/:conversation_id/messages",
            get(rest_chat_conversation_messages),
        )
        .route(
            "/api/realtime/chat/messages/:message_id",
            axum::routing::put(rest_edit_chat_message),
        )
        .route(
            "/api/realtime/chat/messages/:message_id",
            axum::routing::delete(rest_delete_chat_message),
        )
        .route(
            "/api/realtime/chat/messages/:message_id/flag",
            post(rest_flag_chat_message),
        )
        .route(
            "/api/realtime/chat/messages/:message_id/pin",
            post(rest_pin_chat_message),
        )
        .route(
            "/api/realtime/chat/messages/:message_id/reactions",
            post(rest_react_chat_message),
        )
        .route("/api/realtime/notifications", get(rest_notifications))
        .route(
            "/api/realtime/notifications/unread/count",
            get(rest_notifications_unread_count),
        )
        .route(
            "/api/realtime/notifications/preferences",
            get(rest_notifications_preferences),
        )
        .route(
            "/api/realtime/notifications/preferences/:type_id",
            axum::routing::put(rest_update_notification_preference),
        )
        .route("/api/realtime/players/online", get(rest_players_online))
        .route("/api/realtime/trade/offers", get(rest_trade_offers))
        .route("/api/realtime/trade/history", get(rest_trade_history))
        .route("/api/realtime/channels", get(list_channels))
        .route("/api/realtime/subscribe", post(subscribe))
        .route("/api/realtime/publish", post(publish))
        .route("/api/realtime/events/recent", get(recent_events))
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

async fn rest_chat_restrictions(
    Query(query): Query<ChatRestrictionsQuery>,
) -> (StatusCode, Json<serde_json::Value>) {
    let Some(database) = platform_db::Database::from_env() else {
        return (
            StatusCode::OK,
            Json(serde_json::json!(ChatRestrictionsResponse {
                restrictions: Vec::new(),
                total: 0
            })),
        );
    };

    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    match database
        .list_chat_restrictions(query.user_id, query.channel_id, limit)
        .await
    {
        Ok(rows) => {
            let restrictions = rows
                .into_iter()
                .map(map_chat_restriction)
                .collect::<Vec<_>>();
            (
                StatusCode::OK,
                Json(serde_json::json!(ChatRestrictionsResponse {
                    total: restrictions.len(),
                    restrictions
                })),
            )
        }
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!(ErrorEnvelope {
                status: "error",
                error: format!("failed to load chat restrictions: {error}"),
            })),
        ),
    }
}

async fn rest_upsert_chat_restriction(
    Json(payload): Json<UpsertChatRestrictionRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    if payload.user_id <= 0
        || payload.restricted_by <= 0
        || payload.restriction_type.trim().is_empty()
        || payload.reason.trim().is_empty()
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!(ErrorEnvelope {
                status: "error",
                error: "userId, restrictedBy, restrictionType, and reason are required".to_string(),
            })),
        );
    }

    let Some(database) = platform_db::Database::from_env() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!(ErrorEnvelope {
                status: "error",
                error: "DATABASE_URL not configured".to_string(),
            })),
        );
    };

    let input = platform_db::ChatRestrictionUpsert {
        user_id: payload.user_id,
        channel_id: payload.channel_id,
        restriction_type: payload.restriction_type,
        reason: payload.reason,
        restricted_by: payload.restricted_by,
        expires_at_unix: payload.expires_at_unix,
    };

    match database.upsert_chat_restriction(input).await {
        Ok(row) => (
            StatusCode::OK,
            Json(serde_json::json!(map_chat_restriction(row))),
        ),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!(ErrorEnvelope {
                status: "error",
                error: format!("failed to upsert chat restriction: {error}"),
            })),
        ),
    }
}

async fn rest_delete_chat_restriction(
    Json(payload): Json<DeleteChatRestrictionRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    if payload.user_id <= 0 || payload.restriction_type.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!(ErrorEnvelope {
                status: "error",
                error: "userId and restrictionType are required".to_string(),
            })),
        );
    }

    let Some(database) = platform_db::Database::from_env() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!(ErrorEnvelope {
                status: "error",
                error: "DATABASE_URL not configured".to_string(),
            })),
        );
    };

    match database
        .remove_chat_restriction(
            payload.user_id,
            payload.channel_id,
            payload.restriction_type.as_str(),
        )
        .await
    {
        Ok(removed) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "removed": removed
            })),
        ),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!(ErrorEnvelope {
                status: "error",
                error: format!("failed to remove chat restriction: {error}"),
            })),
        ),
    }
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

async fn rest_notifications_unread_count(
    State(state): State<AppState>,
) -> Json<UnreadCountResponse> {
    let store = state.inner.lock().expect("state lock poisoned");
    let unread_count = store.notifications.iter().filter(|item| !item.read).count() as u64;
    Json(UnreadCountResponse { unread_count })
}

async fn rest_notifications_preferences(
    State(state): State<AppState>,
) -> Json<NotificationPreferencesResponse> {
    let store = state.inner.lock().expect("state lock poisoned");
    Json(NotificationPreferencesResponse {
        preferences: store.notification_preferences.clone(),
    })
}

async fn rest_update_notification_preference(
    State(state): State<AppState>,
    axum::extract::Path(type_id): axum::extract::Path<String>,
    Json(payload): Json<UpdatePreferenceRequest>,
) -> Json<NotificationPreferencesResponse> {
    let mut store = state.inner.lock().expect("state lock poisoned");
    store
        .notification_preferences
        .insert(type_id, payload.enabled);
    Json(NotificationPreferencesResponse {
        preferences: store.notification_preferences.clone(),
    })
}

async fn rest_chat_conversations(State(state): State<AppState>) -> Json<ConversationsResponse> {
    let store = state.inner.lock().expect("state lock poisoned");
    Json(ConversationsResponse {
        conversations: store.chat_conversations.clone(),
        total: store.chat_conversations.len(),
    })
}

async fn rest_chat_conversation_messages(
    State(_state): State<AppState>,
    axum::extract::Path(conversation_id): axum::extract::Path<String>,
) -> Json<ConversationMessagesResponse> {
    let messages = vec![
        ChatMessageItem {
            id: "msg-1".to_string(),
            conversation_id: conversation_id.clone(),
            sender: "Scout".to_string(),
            text: "Ping from frontier".to_string(),
            sent_at: "2026-02-13T10:00:00Z".to_string(),
        },
        ChatMessageItem {
            id: "msg-2".to_string(),
            conversation_id: conversation_id.clone(),
            sender: "Commander".to_string(),
            text: "Acknowledged".to_string(),
            sent_at: "2026-02-13T10:01:00Z".to_string(),
        },
    ];

    Json(ConversationMessagesResponse {
        conversation_id,
        total: messages.len(),
        messages,
    })
}

async fn rest_edit_chat_message(
    State(state): State<AppState>,
    axum::extract::Path(message_id): axum::extract::Path<String>,
    Json(payload): Json<EditChatMessageRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    if payload.message.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!(ErrorEnvelope {
                status: "error",
                error: "message is required".to_string(),
            })),
        );
    }

    let mut store = state.inner.lock().expect("state lock poisoned");
    let message = upsert_realtime_message(&mut store, &message_id, payload.user_id.unwrap_or(0));
    message.message = payload.message;
    message.edited = true;

    (
        StatusCode::OK,
        Json(serde_json::json!({ "message": message })),
    )
}

async fn rest_delete_chat_message(
    State(state): State<AppState>,
    axum::extract::Path(message_id): axum::extract::Path<String>,
    Json(payload): Json<DeleteChatMessageRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let mut store = state.inner.lock().expect("state lock poisoned");
    let user_id = payload.user_id.unwrap_or(0);
    let is_admin = payload.is_admin.unwrap_or(false);
    let message = upsert_realtime_message(&mut store, &message_id, user_id);
    if !is_admin && user_id > 0 && message.user_id != user_id {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!(ErrorEnvelope {
                status: "error",
                error: "forbidden".to_string(),
            })),
        );
    }
    message.deleted = true;
    (StatusCode::OK, Json(serde_json::json!({ "success": true })))
}

async fn rest_flag_chat_message(
    State(state): State<AppState>,
    axum::extract::Path(message_id): axum::extract::Path<String>,
    Json(payload): Json<FlagChatMessageRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let mut store = state.inner.lock().expect("state lock poisoned");
    let message = upsert_realtime_message(&mut store, &message_id, payload.user_id.unwrap_or(0));
    message.flags = message.flags.saturating_add(1);
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "flags": message.flags,
            "reason": payload.reason
        })),
    )
}

async fn rest_pin_chat_message(
    State(state): State<AppState>,
    axum::extract::Path(message_id): axum::extract::Path<String>,
    Json(payload): Json<PinChatMessageRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let mut store = state.inner.lock().expect("state lock poisoned");
    let message = upsert_realtime_message(&mut store, &message_id, payload.user_id.unwrap_or(0));
    message.pinned = payload.is_pinned.unwrap_or(true);
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "message": message
        })),
    )
}

async fn rest_react_chat_message(
    State(state): State<AppState>,
    axum::extract::Path(message_id): axum::extract::Path<String>,
    Json(payload): Json<ReactionChatMessageRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let reaction = payload
        .reaction_type
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "thumbs_up".to_string());
    let mut store = state.inner.lock().expect("state lock poisoned");
    let message = upsert_realtime_message(&mut store, &message_id, payload.user_id.unwrap_or(0));
    let counter = message.reactions.entry(reaction.clone()).or_insert(0);
    *counter = counter.saturating_add(1);
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "messageId": message_id,
            "reactionType": reaction,
            "count": *counter,
            "reactions": message.reactions
        })),
    )
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

async fn rest_trade_history(
    State(state): State<AppState>,
    Query(query): Query<TradeHistoryQuery>,
) -> Json<TradeHistoryResponse> {
    let store = state.inner.lock().expect("state lock poisoned");
    let total = store.trade_history.len();
    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(50);
    let entries = store
        .trade_history
        .iter()
        .skip(offset)
        .take(limit)
        .cloned()
        .collect::<Vec<_>>();

    Json(TradeHistoryResponse { entries, total })
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
    let publish_sequence = store.publish_sequence;
    store.recent_events.push(RecentEventItem {
        channel: request.channel.clone(),
        event: request.event.clone(),
        publish_sequence,
    });
    if store.recent_events.len() > 200 {
        let drop_count = store.recent_events.len() - 200;
        store.recent_events.drain(0..drop_count);
    }

    (
        StatusCode::OK,
        Json(serde_json::json!(Envelope {
            status: "ok",
            data: PublishPayload {
                channel: request.channel,
                event: request.event,
                delivered_to,
                publish_sequence,
            },
        })),
    )
}

async fn recent_events(
    State(state): State<AppState>,
    Query(query): Query<RecentEventsQuery>,
) -> Json<RecentEventsResponse> {
    let store = state.inner.lock().expect("state lock poisoned");
    let limit = query.limit.unwrap_or(50).max(1);
    let total = store.recent_events.len();
    let events = store
        .recent_events
        .iter()
        .rev()
        .take(limit)
        .cloned()
        .collect::<Vec<_>>();
    Json(RecentEventsResponse { events, total })
}

fn upsert_realtime_message<'a>(
    store: &'a mut RealtimeState,
    message_id: &str,
    user_id: u64,
) -> &'a mut ChatRealtimeMessage {
    store
        .chat_messages
        .entry(message_id.to_string())
        .or_insert_with(|| ChatRealtimeMessage {
            id: message_id.to_string(),
            channel_id: 1,
            user_id,
            message: "".to_string(),
            edited: false,
            deleted: false,
            pinned: false,
            announcement: false,
            flags: 0,
            reactions: HashMap::new(),
        })
}

fn map_chat_restriction(row: platform_db::ChatRestrictionRow) -> ChatRestrictionItem {
    ChatRestrictionItem {
        id: row.id,
        user_id: row.user_id,
        channel_id: row.channel_id,
        restriction_type: row.restriction_type,
        reason: row.reason,
        restricted_by: row.restricted_by,
        expires_at_unix: row.expires_at_unix,
        created_at_unix: row.created_at_unix,
    }
}
