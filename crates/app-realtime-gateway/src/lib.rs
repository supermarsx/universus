use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::extract::{Extension, Query, State};
use axum::http::{header::AUTHORIZATION, Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

mod websocket;

pub const SERVICE_NAME: &str = "app-realtime-gateway";
pub const DEFAULT_PORT: u16 = 3004;

#[derive(Clone)]
struct AppState {
    inner: Arc<Mutex<RealtimeState>>,
    runtime: websocket::RealtimeRuntime,
    database: Option<platform_db::Database>,
    database_configured: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(RealtimeState::default())),
            runtime: websocket::RealtimeRuntime::local(),
            database: None,
            database_configured: false,
        }
    }
}

impl AppState {
    fn with_dependencies(
        runtime: websocket::RealtimeRuntime,
        database: Option<platform_db::Database>,
        database_configured: bool,
    ) -> Self {
        Self {
            inner: Arc::new(Mutex::new(RealtimeState::default())),
            runtime,
            database,
            database_configured,
        }
    }
}

struct RealtimeState {
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
    format: &'static str,
    authentication: Vec<&'static str>,
    client_frames: Vec<&'static str>,
    active_connections: usize,
    queue_capacity: usize,
    max_subscriptions: usize,
    heartbeat_interval_seconds: u64,
    idle_timeout_seconds: u64,
    cookie_origin_validation: bool,
    transport: &'static str,
    redis_connected: Option<bool>,
}

#[derive(Serialize)]
struct ReadinessStatus {
    status: &'static str,
    service: &'static str,
    transport: &'static str,
    redis_configured: bool,
    redis_connected: Option<bool>,
    database_configured: bool,
    database_connected: Option<bool>,
    active_connections: usize,
    dropped_events: u64,
    backpressure_disconnects: u64,
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
struct PublishRequest {
    channel: String,
    event: String,
}

#[derive(Serialize)]
struct PublishPayload {
    channel: String,
    event: String,
    delivered_to: Option<usize>,
    local_subscribers: usize,
    delivery_scope: &'static str,
    accepted: bool,
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
    app_with_state(AppState::default())
}

fn app_with_state(state: AppState) -> Router {
    let player_router = Router::new()
        .route("/chat/channels", get(rest_chat_channels))
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
            "/chat/messages/:message_id/reactions",
            post(rest_react_chat_message),
        )
        .route("/players/online", get(rest_players_online))
        .route("/trade/offers", get(rest_trade_offers))
        .route("/trade/history", get(rest_trade_history))
        .route("/api/realtime/chat/channels", get(rest_chat_channels))
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
        .route("/api/realtime/subscribe", post(legacy_subscribe))
        .route_layer(middleware::from_fn(require_player_auth));

    let admin_router = Router::new()
        .route(
            "/chat/restrictions",
            get(rest_chat_restrictions)
                .post(rest_upsert_chat_restriction)
                .delete(rest_delete_chat_restriction),
        )
        .route(
            "/chat/messages/:message_id/pin",
            post(rest_pin_chat_message),
        )
        .route(
            "/api/realtime/chat/restrictions",
            get(rest_chat_restrictions)
                .post(rest_upsert_chat_restriction)
                .delete(rest_delete_chat_restriction),
        )
        .route(
            "/api/realtime/chat/messages/:message_id/pin",
            post(rest_pin_chat_message),
        )
        .route("/api/realtime/publish", post(publish))
        .route("/api/realtime/events/recent", get(recent_events))
        .route_layer(middleware::from_fn(require_admin_auth));

    Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/ws", get(websocket::upgrade))
        .route("/ws-info", get(ws_info))
        .merge(player_router)
        .merge(admin_router)
        .with_state(state)
}

async fn require_player_auth(
    request: Request<axum::body::Body>,
    next: Next<axum::body::Body>,
) -> Response {
    require_auth_role(request, next, platform_auth::UserRole::Player).await
}

async fn require_admin_auth(
    request: Request<axum::body::Body>,
    next: Next<axum::body::Body>,
) -> Response {
    require_auth_role(request, next, platform_auth::UserRole::Admin).await
}

async fn require_auth_role(
    mut request: Request<axum::body::Body>,
    next: Next<axum::body::Body>,
    minimum: platform_auth::UserRole,
) -> Response {
    let Some(authorization) = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
    else {
        return auth_failure(StatusCode::UNAUTHORIZED, "Unauthorized");
    };

    let config = platform_auth::AuthConfig::from_env();
    let user = match platform_auth::authenticate_request(&config, authorization) {
        Ok(user) => user,
        Err(_) => return auth_failure(StatusCode::UNAUTHORIZED, "Unauthorized"),
    };
    if platform_auth::require_role(&user, minimum).is_err() {
        return auth_failure(StatusCode::FORBIDDEN, "Forbidden");
    }

    request.extensions_mut().insert(user);
    next.run(request).await
}

fn auth_failure(status: StatusCode, error: &'static str) -> Response {
    (
        status,
        Json(ErrorEnvelope {
            status: "error",
            error: error.to_string(),
        }),
    )
        .into_response()
}

pub fn listen_port(default_port: u16) -> u16 {
    std::env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(default_port)
}

fn production_environment() -> bool {
    ["UNIVERSUS_ENV", "APP_ENV", "ENVIRONMENT", "RUST_ENV"]
        .into_iter()
        .find_map(|name| std::env::var(name).ok())
        .is_some_and(|environment| {
            matches!(
                environment.trim().to_ascii_lowercase().as_str(),
                "production" | "prod" | "staging" | "stage"
            )
        })
}

async fn app_state_from_env() -> Result<AppState, String> {
    let production = production_environment();
    let database_configured = std::env::var("DATABASE_URL")
        .ok()
        .is_some_and(|value| !value.trim().is_empty());
    let database = match platform_db::Database::try_from_env() {
        Ok(database) => database,
        Err(error) if production => return Err(error),
        Err(_) => {
            tracing::warn!(
                service = SERVICE_NAME,
                "database configuration is invalid; readiness will remain unavailable"
            );
            None
        }
    };
    if production && database.is_none() {
        return Err("DATABASE_URL is required for realtime persistence in production".to_string());
    }
    if production {
        let ping = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            database
                .as_ref()
                .expect("production database configured")
                .ping(),
        )
        .await;
        if !matches!(ping, Ok(Ok(()))) {
            return Err("Postgres is unavailable for realtime persistence".to_string());
        }
    }

    let redis_url = std::env::var("REDIS_URL")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let runtime = match redis_url {
        Some(url) => match websocket::RealtimeRuntime::redis(&url, production).await {
            Ok(runtime) => runtime,
            Err(error) if production => return Err(error),
            Err(_) => {
                tracing::warn!(
                    service = SERVICE_NAME,
                    "Redis configuration is invalid; readiness will remain unavailable"
                );
                websocket::RealtimeRuntime::redis_unavailable()
            }
        },
        None if production => {
            return Err("REDIS_URL is required for realtime fanout in production".to_string())
        }
        None => {
            tracing::warn!(
                service = SERVICE_NAME,
                "using local in-process realtime fanout for development"
            );
            websocket::RealtimeRuntime::local()
        }
    };
    Ok(AppState::with_dependencies(
        runtime,
        database,
        database_configured,
    ))
}

pub async fn serve() {
    tracing_subscriber::fmt::init();

    platform_auth::AuthConfig::from_env()
        .validate_runtime()
        .expect("invalid authentication configuration");

    let state = app_state_from_env()
        .await
        .expect("invalid realtime transport configuration");
    let app = app_with_state(state);
    let addr = SocketAddr::from(([0, 0, 0, 0], listen_port(DEFAULT_PORT)));
    tracing::info!(service = SERVICE_NAME, %addr, "startup");

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .expect("server failed");
}

async fn health() -> Json<ServiceStatus> {
    Json(ServiceStatus {
        status: "ok",
        service: SERVICE_NAME,
    })
}

async fn ready(State(state): State<AppState>) -> (StatusCode, Json<ReadinessStatus>) {
    let database_connected = match state.database.as_ref() {
        Some(database) => Some(matches!(
            tokio::time::timeout(std::time::Duration::from_secs(2), database.ping()).await,
            Ok(Ok(()))
        )),
        None if state.database_configured => Some(false),
        None => None,
    };
    let ready = state.runtime.is_ready() && database_connected.unwrap_or(true);
    (
        if ready {
            StatusCode::OK
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        },
        Json(ReadinessStatus {
            status: if ready { "ok" } else { "unavailable" },
            service: SERVICE_NAME,
            transport: state.runtime.transport_name(),
            redis_configured: state.runtime.redis_configured(),
            redis_connected: state
                .runtime
                .redis_configured()
                .then(|| state.runtime.redis_connected()),
            database_configured: state.database_configured,
            database_connected,
            active_connections: state.runtime.active_connections(),
            dropped_events: state.runtime.dropped_events(),
            backpressure_disconnects: state.runtime.backpressure_disconnects(),
        }),
    )
}

async fn ws_info(State(state): State<AppState>) -> Json<WebSocketInfo> {
    Json(WebSocketInfo {
        service: SERVICE_NAME,
        websocket: true,
        endpoint: "/ws",
        format: "json",
        authentication: vec!["authorization_bearer", "universus_token_cookie"],
        client_frames: vec!["subscribe", "unsubscribe", "ping"],
        active_connections: state.runtime.active_connections(),
        queue_capacity: state.runtime.queue_capacity(),
        max_subscriptions: state.runtime.max_subscriptions(),
        heartbeat_interval_seconds: state.runtime.heartbeat_interval_seconds(),
        idle_timeout_seconds: state.runtime.idle_timeout_seconds(),
        cookie_origin_validation: true,
        transport: state.runtime.transport_name(),
        redis_connected: state
            .runtime
            .redis_configured()
            .then(|| state.runtime.redis_connected()),
    })
}

async fn rest_chat_channels(
    State(state): State<AppState>,
    Extension(auth_user): Extension<platform_auth::AuthUser>,
) -> Json<ChatChannelsResponse> {
    let channels = state
        .runtime
        .channel_snapshot()
        .into_iter()
        .filter(|(name, _)| websocket::authorize_channel(&auth_user, name).is_ok())
        .map(|(name, subscriber_count)| ChannelInfo {
            name,
            subscriber_count,
        })
        .collect();

    Json(ChatChannelsResponse { channels })
}

async fn rest_chat_restrictions(
    State(state): State<AppState>,
    Query(query): Query<ChatRestrictionsQuery>,
) -> (StatusCode, Json<serde_json::Value>) {
    let Some(database) = state.database else {
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
    State(state): State<AppState>,
    Extension(auth_user): Extension<platform_auth::AuthUser>,
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

    let restricted_by = match authenticated_numeric_user_id(&auth_user, None) {
        Ok(user_id) => user_id as i64,
        Err(response) => return response,
    };

    let Some(database) = state.database else {
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
        restricted_by,
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
    State(state): State<AppState>,
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

    let Some(database) = state.database else {
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
    Extension(auth_user): Extension<platform_auth::AuthUser>,
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

    let actor_id = match authenticated_numeric_user_id(&auth_user, payload.user_id) {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };

    let mut store = state.inner.lock().expect("state lock poisoned");
    if let Some(message) = store.chat_messages.get(&message_id) {
        if message.user_id != actor_id && !is_admin(&auth_user) {
            return forbidden_operation();
        }
    }
    let message = upsert_realtime_message(&mut store, &message_id, actor_id);
    message.message = payload.message;
    message.edited = true;

    (
        StatusCode::OK,
        Json(serde_json::json!({ "message": message })),
    )
}

async fn rest_delete_chat_message(
    State(state): State<AppState>,
    Extension(auth_user): Extension<platform_auth::AuthUser>,
    axum::extract::Path(message_id): axum::extract::Path<String>,
    Json(payload): Json<DeleteChatMessageRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    // The legacy `isAdmin` field remains accepted for wire compatibility but
    // authorization is derived exclusively from the signed JWT role.
    let _ = payload.is_admin;
    let actor_id = match authenticated_numeric_user_id(&auth_user, payload.user_id) {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };
    let mut store = state.inner.lock().expect("state lock poisoned");
    if let Some(message) = store.chat_messages.get(&message_id) {
        if message.user_id != actor_id && !is_admin(&auth_user) {
            return forbidden_operation();
        }
    }
    let message = upsert_realtime_message(&mut store, &message_id, actor_id);
    message.deleted = true;
    (StatusCode::OK, Json(serde_json::json!({ "success": true })))
}

async fn rest_flag_chat_message(
    State(state): State<AppState>,
    Extension(auth_user): Extension<platform_auth::AuthUser>,
    axum::extract::Path(message_id): axum::extract::Path<String>,
    Json(payload): Json<FlagChatMessageRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let actor_id = match authenticated_numeric_user_id(&auth_user, payload.user_id) {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };
    let mut store = state.inner.lock().expect("state lock poisoned");
    let message = upsert_realtime_message(&mut store, &message_id, actor_id);
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
    Extension(auth_user): Extension<platform_auth::AuthUser>,
    axum::extract::Path(message_id): axum::extract::Path<String>,
    Json(payload): Json<PinChatMessageRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let actor_id = match authenticated_numeric_user_id(&auth_user, None) {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };
    let _ = payload.user_id;
    let mut store = state.inner.lock().expect("state lock poisoned");
    let message = upsert_realtime_message(&mut store, &message_id, actor_id);
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
    Extension(auth_user): Extension<platform_auth::AuthUser>,
    axum::extract::Path(message_id): axum::extract::Path<String>,
    Json(payload): Json<ReactionChatMessageRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let actor_id = match authenticated_numeric_user_id(&auth_user, payload.user_id) {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };
    let reaction = payload
        .reaction_type
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "thumbs_up".to_string());
    let mut store = state.inner.lock().expect("state lock poisoned");
    let message = upsert_realtime_message(&mut store, &message_id, actor_id);
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

async fn list_channels(
    State(state): State<AppState>,
    Extension(auth_user): Extension<platform_auth::AuthUser>,
) -> Json<Envelope<ChannelsPayload>> {
    let channels = state
        .runtime
        .channel_snapshot()
        .into_iter()
        .filter(|(name, _)| websocket::authorize_channel(&auth_user, name).is_ok())
        .map(|(name, subscriber_count)| ChannelInfo {
            name,
            subscriber_count,
        })
        .collect();

    Json(Envelope {
        status: "ok",
        data: ChannelsPayload { channels },
    })
}

async fn legacy_subscribe() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::UPGRADE_REQUIRED,
        Json(serde_json::json!(ErrorEnvelope {
            status: "error",
            error: "subscriptions require the /ws WebSocket transport".to_string(),
        })),
    )
}

async fn publish(
    State(state): State<AppState>,
    Extension(auth_user): Extension<platform_auth::AuthUser>,
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
    let channel = match websocket::authorize_channel(&auth_user, &request.channel) {
        Ok(channel) => channel,
        Err(code) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!(ErrorEnvelope {
                    status: "error",
                    error: code.to_string(),
                })),
            )
        }
    };
    if request.event.len() > 64 * 1024 {
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(serde_json::json!(ErrorEnvelope {
                status: "error",
                error: "event exceeds 65536 bytes".to_string(),
            })),
        );
    }

    let publish_sequence = {
        let mut store = state.inner.lock().expect("state lock poisoned");
        store.publish_sequence = store.publish_sequence.saturating_add(1);
        store.publish_sequence
    };
    let instance = std::env::var("HOSTNAME").unwrap_or_else(|_| SERVICE_NAME.to_string());
    let bus_event = websocket::BusEvent {
        event_id: format!("{instance}:{}:{publish_sequence}", std::process::id()),
        channel: channel.clone(),
        event: request.event.clone(),
        sequence: publish_sequence,
    };
    let receipt = match state.runtime.publish(bus_event).await {
        Ok(receipt) => receipt,
        Err(_) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!(ErrorEnvelope {
                    status: "error",
                    error: "realtime fanout unavailable".to_string(),
                })),
            )
        }
    };

    {
        let mut store = state.inner.lock().expect("state lock poisoned");
        store.recent_events.push(RecentEventItem {
            channel: channel.clone(),
            event: request.event.clone(),
            publish_sequence,
        });
        if store.recent_events.len() > 200 {
            let drop_count = store.recent_events.len() - 200;
            store.recent_events.drain(0..drop_count);
        }
    }

    (
        StatusCode::OK,
        Json(serde_json::json!(Envelope {
            status: "ok",
            data: PublishPayload {
                channel,
                event: request.event,
                delivered_to: receipt.delivered_to,
                local_subscribers: receipt.local_subscribers,
                delivery_scope: receipt.delivery_scope,
                accepted: true,
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

fn authenticated_numeric_user_id(
    auth_user: &platform_auth::AuthUser,
    claimed_user_id: Option<u64>,
) -> Result<u64, (StatusCode, Json<serde_json::Value>)> {
    let parsed_id = auth_user
        .user_id
        .parse::<u64>()
        .ok()
        .filter(|user_id| (1..=i64::MAX as u64).contains(user_id));
    let authenticated_id =
        parsed_id.unwrap_or_else(|| platform_auth::stable_numeric_subject_id(&auth_user.user_id));
    if !is_admin(auth_user)
        && parsed_id.is_some()
        && claimed_user_id.is_some_and(|user_id| user_id != authenticated_id)
    {
        return Err(forbidden_operation());
    }
    Ok(authenticated_id)
}

fn is_admin(auth_user: &platform_auth::AuthUser) -> bool {
    platform_auth::require_role(auth_user, platform_auth::UserRole::Admin).is_ok()
}

fn forbidden_operation() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::FORBIDDEN,
        Json(serde_json::json!(ErrorEnvelope {
            status: "error",
            error: "forbidden".to_string(),
        })),
    )
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

#[cfg(test)]
mod readiness_tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    use super::*;

    #[tokio::test]
    async fn configured_but_unavailable_database_fails_readiness() {
        let state = AppState::with_dependencies(websocket::RealtimeRuntime::local(), None, true);
        let response = app_with_state(state)
            .oneshot(
                Request::builder()
                    .uri("/ready")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn unavailable_redis_fails_readiness_without_local_fallback() {
        let state = AppState::with_dependencies(
            websocket::RealtimeRuntime::redis_unavailable(),
            None,
            false,
        );
        let response = app_with_state(state)
            .oneshot(
                Request::builder()
                    .uri("/ready")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}
