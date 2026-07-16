use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::ws::{close_code, CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::{
    header::{AUTHORIZATION, COOKIE, HOST, ORIGIN},
    HeaderMap, StatusCode, Uri,
};
use axum::response::{IntoResponse, Response};
use futures_util::{SinkExt, StreamExt};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, watch};

use super::{auth_failure, AppState};

const REDIS_BUS_CHANNEL: &str = "universus.realtime.events.v1";
const DEFAULT_QUEUE_CAPACITY: usize = 64;
const DEFAULT_MAX_SUBSCRIPTIONS: usize = 32;
const DEFAULT_HEARTBEAT_INTERVAL_SECONDS: usize = 30;
const DEFAULT_IDLE_TIMEOUT_SECONDS: usize = 90;
const MAX_INBOUND_MESSAGE_BYTES: usize = 16 * 1024;
const MAX_PROTOCOL_ERRORS: u8 = 3;
const REDIS_RECONNECT_DELAY: Duration = Duration::from_secs(1);
const REDIS_CONNECT_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BusEvent {
    pub event_id: String,
    pub channel: String,
    pub event: String,
    pub sequence: u64,
}

#[derive(Clone)]
pub(crate) struct RealtimeRuntime {
    inner: Arc<RuntimeInner>,
}

struct RuntimeInner {
    hub: ConnectionHub,
    backend: FanoutBackend,
    redis_connected: Arc<AtomicBool>,
    queue_capacity: usize,
    max_subscriptions: usize,
    heartbeat_interval: Duration,
    idle_timeout: Duration,
}

pub(crate) struct PublishReceipt {
    pub delivered_to: Option<usize>,
    pub local_subscribers: usize,
    pub delivery_scope: &'static str,
}

#[derive(Clone)]
enum FanoutBackend {
    Local,
    Redis(redis::Client),
    RedisUnavailable,
}

impl RealtimeRuntime {
    pub(crate) fn local() -> Self {
        Self::with_backend(FanoutBackend::Local, true)
    }

    pub(crate) fn redis_unavailable() -> Self {
        Self::with_backend(FanoutBackend::RedisUnavailable, false)
    }

    pub(crate) async fn redis(url: &str, require_initial_connection: bool) -> Result<Self, String> {
        let client = redis::Client::open(url).map_err(|error| error.to_string())?;
        let initial = prepare_pubsub(&client).await;
        if require_initial_connection && initial.is_err() {
            return Err("Redis pub/sub is unavailable".to_string());
        }

        let runtime = Self::with_backend(FanoutBackend::Redis(client.clone()), initial.is_ok());
        let hub = runtime.inner.hub.clone();
        let connected = runtime.inner.redis_connected.clone();
        tokio::spawn(redis_listener_loop(client, initial.ok(), hub, connected));
        Ok(runtime)
    }

    fn with_backend(backend: FanoutBackend, connected: bool) -> Self {
        let queue_capacity =
            parse_positive_env("REALTIME_WS_QUEUE_CAPACITY", DEFAULT_QUEUE_CAPACITY);
        let max_subscriptions =
            parse_positive_env("REALTIME_WS_MAX_SUBSCRIPTIONS", DEFAULT_MAX_SUBSCRIPTIONS);
        let heartbeat_interval = Duration::from_secs(parse_positive_env(
            "REALTIME_WS_HEARTBEAT_INTERVAL_SECS",
            DEFAULT_HEARTBEAT_INTERVAL_SECONDS,
        ) as u64);
        let idle_timeout = Duration::from_secs(parse_positive_env(
            "REALTIME_WS_IDLE_TIMEOUT_SECS",
            DEFAULT_IDLE_TIMEOUT_SECONDS,
        ) as u64);
        Self {
            inner: Arc::new(RuntimeInner {
                hub: ConnectionHub::default(),
                backend,
                redis_connected: Arc::new(AtomicBool::new(connected)),
                queue_capacity,
                max_subscriptions,
                heartbeat_interval,
                idle_timeout,
            }),
        }
    }

    pub(crate) fn transport_name(&self) -> &'static str {
        match self.inner.backend {
            FanoutBackend::Local => "local",
            FanoutBackend::Redis(_) | FanoutBackend::RedisUnavailable => "redis",
        }
    }

    pub(crate) fn redis_configured(&self) -> bool {
        !matches!(self.inner.backend, FanoutBackend::Local)
    }

    pub(crate) fn redis_connected(&self) -> bool {
        self.inner.redis_connected.load(Ordering::Acquire)
    }

    pub(crate) fn is_ready(&self) -> bool {
        !self.redis_configured() || self.redis_connected()
    }

    pub(crate) fn active_connections(&self) -> usize {
        self.inner.hub.active_connections()
    }

    pub(crate) fn dropped_events(&self) -> u64 {
        self.inner.hub.dropped_events.load(Ordering::Relaxed)
    }

    pub(crate) fn backpressure_disconnects(&self) -> u64 {
        self.inner
            .hub
            .backpressure_disconnects
            .load(Ordering::Relaxed)
    }

    pub(crate) fn queue_capacity(&self) -> usize {
        self.inner.queue_capacity
    }

    pub(crate) fn max_subscriptions(&self) -> usize {
        self.inner.max_subscriptions
    }

    pub(crate) fn heartbeat_interval_seconds(&self) -> u64 {
        self.inner.heartbeat_interval.as_secs()
    }

    pub(crate) fn idle_timeout_seconds(&self) -> u64 {
        self.inner.idle_timeout.as_secs()
    }

    pub(crate) fn channel_snapshot(&self) -> Vec<(String, usize)> {
        self.inner.hub.channel_snapshot()
    }

    pub(crate) async fn publish(&self, event: BusEvent) -> Result<PublishReceipt, String> {
        match &self.inner.backend {
            FanoutBackend::Local => {
                let local_subscribers = self.inner.hub.subscriber_count(&event.channel);
                let delivered_to = self.inner.hub.dispatch(event);
                Ok(PublishReceipt {
                    delivered_to: Some(delivered_to),
                    local_subscribers,
                    delivery_scope: "local_process",
                })
            }
            FanoutBackend::RedisUnavailable => Err("Redis pub/sub is unavailable".to_string()),
            FanoutBackend::Redis(client) => {
                if !self.redis_connected() {
                    return Err("Redis pub/sub is unavailable".to_string());
                }
                let payload = serde_json::to_string(&event).map_err(|error| error.to_string())?;
                let mut connection = tokio::time::timeout(
                    REDIS_CONNECT_TIMEOUT,
                    client.get_multiplexed_async_connection(),
                )
                .await
                .map_err(|_| "Redis publish connection timed out".to_string())?
                .map_err(|error| error.to_string())?;
                let _: usize = connection
                    .publish(REDIS_BUS_CHANNEL, payload)
                    .await
                    .map_err(|error| error.to_string())?;
                Ok(PublishReceipt {
                    delivered_to: None,
                    local_subscribers: self.inner.hub.subscriber_count(&event.channel),
                    delivery_scope: "redis_cluster_accepted",
                })
            }
        }
    }
}

fn parse_positive_env(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

#[derive(Clone, Default)]
struct ConnectionHub {
    inner: Arc<Mutex<HubState>>,
    dropped_events: Arc<AtomicU64>,
    backpressure_disconnects: Arc<AtomicU64>,
}

#[derive(Default)]
struct HubState {
    next_connection_id: u64,
    connections: HashMap<u64, ConnectionEntry>,
}

struct ConnectionEntry {
    user: platform_auth::AuthUser,
    subscriptions: HashSet<String>,
    outbound: mpsc::Sender<ServerFrame>,
    disconnect: watch::Sender<bool>,
}

struct ConnectionHandle {
    id: u64,
    outbound: mpsc::Receiver<ServerFrame>,
    disconnect: watch::Receiver<bool>,
    initial_subscriptions: Vec<String>,
}

impl ConnectionHub {
    fn register(&self, user: platform_auth::AuthUser, queue_capacity: usize) -> ConnectionHandle {
        let (outbound_tx, outbound_rx) = mpsc::channel(queue_capacity);
        let (disconnect_tx, disconnect_rx) = watch::channel(false);
        let mut subscriptions = HashSet::from([
            format!("player:{}", user.user_id),
            platform_events::user_notification_channel(&user.user_id),
        ]);
        if let Some(universe_id) = user.universe_id {
            subscriptions.insert(format!("universe:{universe_id}"));
        }
        subscriptions.insert("global".to_string());

        let mut state = self.inner.lock().expect("realtime hub lock poisoned");
        state.next_connection_id = state.next_connection_id.saturating_add(1);
        let id = state.next_connection_id;
        let mut initial_subscriptions = subscriptions.iter().cloned().collect::<Vec<_>>();
        initial_subscriptions.sort();
        state.connections.insert(
            id,
            ConnectionEntry {
                user,
                subscriptions,
                outbound: outbound_tx,
                disconnect: disconnect_tx,
            },
        );

        ConnectionHandle {
            id,
            outbound: outbound_rx,
            disconnect: disconnect_rx,
            initial_subscriptions,
        }
    }

    fn remove(&self, connection_id: u64) {
        self.inner
            .lock()
            .expect("realtime hub lock poisoned")
            .connections
            .remove(&connection_id);
    }

    fn subscribe(
        &self,
        connection_id: u64,
        channel: &str,
        max_subscriptions: usize,
    ) -> Result<String, ChannelError> {
        let mut state = self.inner.lock().expect("realtime hub lock poisoned");
        let entry = state
            .connections
            .get_mut(&connection_id)
            .ok_or(ChannelError::Disconnected)?;
        let channel = authorize_channel_inner(&entry.user, channel)?;
        if entry.subscriptions.len() >= max_subscriptions && !entry.subscriptions.contains(&channel)
        {
            return Err(ChannelError::TooManySubscriptions);
        }
        entry.subscriptions.insert(channel.clone());
        Ok(channel)
    }

    fn unsubscribe(&self, connection_id: u64, channel: &str) -> Result<String, ChannelError> {
        let mut state = self.inner.lock().expect("realtime hub lock poisoned");
        let entry = state
            .connections
            .get_mut(&connection_id)
            .ok_or(ChannelError::Disconnected)?;
        let channel = authorize_channel_inner(&entry.user, channel)?;
        entry.subscriptions.remove(&channel);
        Ok(channel)
    }

    fn dispatch(&self, event: BusEvent) -> usize {
        let mut state = self.inner.lock().expect("realtime hub lock poisoned");
        let mut delivered = 0;
        let mut disconnect = Vec::new();

        for (connection_id, entry) in &state.connections {
            if !entry.subscriptions.contains(&event.channel) {
                continue;
            }
            match entry.outbound.try_send(ServerFrame::Event {
                event_id: event.event_id.clone(),
                channel: event.channel.clone(),
                event: event.event.clone(),
                sequence: event.sequence,
            }) {
                Ok(()) => delivered += 1,
                Err(mpsc::error::TrySendError::Full(_)) => {
                    self.dropped_events.fetch_add(1, Ordering::Relaxed);
                    self.backpressure_disconnects
                        .fetch_add(1, Ordering::Relaxed);
                    let _ = entry.disconnect.send(true);
                    disconnect.push(*connection_id);
                }
                Err(mpsc::error::TrySendError::Closed(_)) => disconnect.push(*connection_id),
            }
        }

        for connection_id in disconnect {
            state.connections.remove(&connection_id);
        }
        delivered
    }

    fn subscriber_count(&self, channel: &str) -> usize {
        self.inner
            .lock()
            .expect("realtime hub lock poisoned")
            .connections
            .values()
            .filter(|entry| entry.subscriptions.contains(channel))
            .count()
    }

    fn active_connections(&self) -> usize {
        self.inner
            .lock()
            .expect("realtime hub lock poisoned")
            .connections
            .len()
    }

    fn channel_snapshot(&self) -> Vec<(String, usize)> {
        let state = self.inner.lock().expect("realtime hub lock poisoned");
        let mut channels = HashMap::<String, usize>::new();
        for entry in state.connections.values() {
            for channel in &entry.subscriptions {
                *channels.entry(channel.clone()).or_default() += 1;
            }
        }
        let mut channels = channels.into_iter().collect::<Vec<_>>();
        channels.sort_by(|left, right| left.0.cmp(&right.0));
        channels
    }
}

#[derive(Debug)]
enum ChannelError {
    Forbidden,
    Invalid,
    TooManySubscriptions,
    Disconnected,
}

impl ChannelError {
    fn code(&self) -> &'static str {
        match self {
            Self::Forbidden => "forbidden_channel",
            Self::Invalid => "invalid_channel",
            Self::TooManySubscriptions => "subscription_limit",
            Self::Disconnected => "disconnected",
        }
    }
}

pub(crate) fn authorize_channel(
    user: &platform_auth::AuthUser,
    requested: &str,
) -> Result<String, &'static str> {
    authorize_channel_inner(user, requested).map_err(|error| error.code())
}

fn authorize_channel_inner(
    user: &platform_auth::AuthUser,
    requested: &str,
) -> Result<String, ChannelError> {
    let channel = requested.trim();
    if channel.is_empty()
        || channel.len() > 128
        || !channel
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b':' | b'_' | b'-'))
    {
        return Err(ChannelError::Invalid);
    }

    let admin = platform_auth::require_role(user, platform_auth::UserRole::Admin).is_ok();
    if matches!(channel, "global" | "announcements" | "battle-feed") {
        return Ok(channel.to_string());
    }
    if let Some(target) = channel
        .strip_prefix("player:")
        .or_else(|| channel.strip_prefix(platform_events::USER_NOTIFICATION_CHANNEL_PREFIX))
    {
        return if admin || target == user.user_id {
            Ok(channel.to_string())
        } else {
            Err(ChannelError::Forbidden)
        };
    }
    if let Some(target) = channel.strip_prefix("universe:") {
        return if admin || target.parse::<i64>().ok() == user.universe_id {
            Ok(channel.to_string())
        } else {
            Err(ChannelError::Forbidden)
        };
    }
    if channel.starts_with("ops.")
        || channel.starts_with("alliance:")
        || channel.starts_with("planet:")
        || channel.starts_with("combat:")
        || channel.starts_with("chat:")
    {
        return if admin {
            Ok(channel.to_string())
        } else {
            Err(ChannelError::Forbidden)
        };
    }
    Err(ChannelError::Invalid)
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientFrame {
    Subscribe { channel: String },
    Unsubscribe { channel: String },
    Ping { nonce: Option<String> },
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ServerFrame {
    Ready {
        connection_id: u64,
        user_id: String,
        transport: &'static str,
        subscriptions: Vec<String>,
    },
    Subscribed {
        channel: String,
    },
    Unsubscribed {
        channel: String,
    },
    Pong {
        nonce: Option<String>,
    },
    Event {
        event_id: String,
        channel: String,
        event: String,
        sequence: u64,
    },
    Error {
        code: &'static str,
        message: &'static str,
    },
}

pub(crate) async fn upgrade(
    State(state): State<AppState>,
    websocket: WebSocketUpgrade,
    headers: HeaderMap,
) -> Response {
    let (user, source) = match authenticate_upgrade(&headers) {
        Ok(authenticated) => authenticated,
        Err(()) => return auth_failure(StatusCode::UNAUTHORIZED, "Unauthorized"),
    };
    if !upgrade_origin_allowed(&headers, source) {
        return auth_failure(StatusCode::FORBIDDEN, "Forbidden origin");
    }
    let runtime = state.runtime.clone();
    websocket
        .max_message_size(MAX_INBOUND_MESSAGE_BYTES)
        .max_frame_size(MAX_INBOUND_MESSAGE_BYTES)
        .on_upgrade(move |socket| connection_loop(socket, runtime, user))
        .into_response()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AuthSource {
    Authorization,
    Cookie,
}

fn authenticate_upgrade(headers: &HeaderMap) -> Result<(platform_auth::AuthUser, AuthSource), ()> {
    let header_authorization = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let (authorization, source) = match header_authorization {
        Some(authorization) => (authorization, AuthSource::Authorization),
        None => {
            let authorization = headers
                .get(COOKIE)
                .and_then(|value| value.to_str().ok())
                .and_then(|cookies| {
                    cookies.split(';').find_map(|cookie| {
                        let (name, token) = cookie.trim().split_once('=')?;
                        (name == "universus_token" && !token.is_empty())
                            .then(|| format!("Bearer {token}"))
                    })
                })
                .ok_or(())?;
            (authorization, AuthSource::Cookie)
        }
    };
    let config = platform_auth::AuthConfig::from_env();
    let user = platform_auth::authenticate_request(&config, &authorization).map_err(|_| ())?;
    platform_auth::require_role(&user, platform_auth::UserRole::Player).map_err(|_| ())?;
    Ok((user, source))
}

fn upgrade_origin_allowed(headers: &HeaderMap, source: AuthSource) -> bool {
    let origin = headers
        .get(ORIGIN)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.trim().trim_end_matches('/'));
    let Some(origin) = origin else {
        return source == AuthSource::Authorization;
    };
    if origin.eq_ignore_ascii_case("null") {
        return false;
    }

    let explicitly_allowed = std::env::var("REALTIME_ALLOWED_ORIGINS")
        .ok()
        .into_iter()
        .flat_map(|origins| {
            origins
                .split(',')
                .map(str::trim)
                .filter(|allowed| !allowed.is_empty())
                .map(|allowed| allowed.trim_end_matches('/').to_string())
                .collect::<Vec<_>>()
        })
        .any(|allowed| allowed.eq_ignore_ascii_case(origin));
    if explicitly_allowed {
        return true;
    }

    let Ok(uri) = origin.parse::<Uri>() else {
        return false;
    };
    let origin_host = uri.authority().map(|authority| authority.as_str());
    let request_host = headers.get(HOST).and_then(|value| value.to_str().ok());
    matches!(uri.scheme_str(), Some("http" | "https")) && origin_host == request_host
}

async fn connection_loop(
    socket: WebSocket,
    runtime: RealtimeRuntime,
    user: platform_auth::AuthUser,
) {
    let mut handle = runtime
        .inner
        .hub
        .register(user.clone(), runtime.queue_capacity());
    let connection_id = handle.id;
    let (mut sink, mut stream) = socket.split();
    if send_server_frame(
        &mut sink,
        ServerFrame::Ready {
            connection_id,
            user_id: user.user_id,
            transport: runtime.transport_name(),
            subscriptions: handle.initial_subscriptions,
        },
    )
    .await
    .is_err()
    {
        runtime.inner.hub.remove(connection_id);
        return;
    }

    let mut protocol_errors = 0_u8;
    let mut last_activity = tokio::time::Instant::now();
    let heartbeat_period = runtime
        .inner
        .heartbeat_interval
        .min(runtime.inner.idle_timeout);
    let mut heartbeat = tokio::time::interval(heartbeat_period);
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    heartbeat.tick().await;
    loop {
        tokio::select! {
            _ = heartbeat.tick() => {
                if idle_expired(last_activity, tokio::time::Instant::now(), runtime.inner.idle_timeout) {
                    let _ = sink.send(Message::Close(Some(CloseFrame {
                        code: close_code::AWAY,
                        reason: Cow::Borrowed("idle timeout"),
                    }))).await;
                    break;
                }
                if sink.send(Message::Ping(Vec::new())).await.is_err() { break; }
            }
            outbound = handle.outbound.recv() => {
                let Some(frame) = outbound else { break; };
                if send_server_frame(&mut sink, frame).await.is_err() { break; }
            }
            changed = handle.disconnect.changed() => {
                if changed.is_err() || *handle.disconnect.borrow() {
                    let _ = sink.send(Message::Close(Some(CloseFrame {
                        code: close_code::AGAIN,
                        reason: Cow::Borrowed("outbound backpressure"),
                    }))).await;
                    break;
                }
            }
            incoming = stream.next() => {
                let Some(incoming) = incoming else { break; };
                let Ok(message) = incoming else { break; };
                last_activity = tokio::time::Instant::now();
                match message {
                    Message::Text(text) => {
                        match serde_json::from_str::<ClientFrame>(&text) {
                            Ok(ClientFrame::Subscribe { channel }) => {
                                match runtime.inner.hub.subscribe(connection_id, &channel, runtime.max_subscriptions()) {
                                    Ok(channel) => {
                                        if send_server_frame(&mut sink, ServerFrame::Subscribed { channel }).await.is_err() { break; }
                                    }
                                    Err(error) => {
                                        if send_protocol_error(&mut sink, error.code()).await.is_err() { break; }
                                    }
                                }
                            }
                            Ok(ClientFrame::Unsubscribe { channel }) => {
                                match runtime.inner.hub.unsubscribe(connection_id, &channel) {
                                    Ok(channel) => {
                                        if send_server_frame(&mut sink, ServerFrame::Unsubscribed { channel }).await.is_err() { break; }
                                    }
                                    Err(error) => {
                                        if send_protocol_error(&mut sink, error.code()).await.is_err() { break; }
                                    }
                                }
                            }
                            Ok(ClientFrame::Ping { nonce }) => {
                                if send_server_frame(&mut sink, ServerFrame::Pong { nonce }).await.is_err() { break; }
                            }
                            Err(_) => {
                                protocol_errors = protocol_errors.saturating_add(1);
                                if send_protocol_error(&mut sink, "malformed_frame").await.is_err() { break; }
                                if protocol_errors >= MAX_PROTOCOL_ERRORS {
                                    let _ = sink.send(Message::Close(Some(CloseFrame {
                                        code: close_code::POLICY,
                                        reason: Cow::Borrowed("too many malformed frames"),
                                    }))).await;
                                    break;
                                }
                            }
                        }
                    }
                    Message::Ping(payload) => {
                        if sink.send(Message::Pong(payload)).await.is_err() { break; }
                    }
                    Message::Pong(_) => {}
                    Message::Close(_) => break,
                    Message::Binary(_) => {
                        protocol_errors = protocol_errors.saturating_add(1);
                        if send_protocol_error(&mut sink, "binary_not_supported").await.is_err() { break; }
                        if protocol_errors >= MAX_PROTOCOL_ERRORS {
                            let _ = sink.send(Message::Close(Some(CloseFrame {
                                code: close_code::POLICY,
                                reason: Cow::Borrowed("too many malformed frames"),
                            }))).await;
                            break;
                        }
                    }
                }
            }
        }
    }
    runtime.inner.hub.remove(connection_id);
}

fn idle_expired(
    last_activity: tokio::time::Instant,
    now: tokio::time::Instant,
    idle_timeout: Duration,
) -> bool {
    now.saturating_duration_since(last_activity) >= idle_timeout
}

async fn send_server_frame<S>(sink: &mut S, frame: ServerFrame) -> Result<(), ()>
where
    S: futures_util::Sink<Message> + Unpin,
{
    let text = serde_json::to_string(&frame).map_err(|_| ())?;
    sink.send(Message::Text(text)).await.map_err(|_| ())
}

async fn send_protocol_error<S>(sink: &mut S, code: &'static str) -> Result<(), ()>
where
    S: futures_util::Sink<Message> + Unpin,
{
    send_server_frame(
        sink,
        ServerFrame::Error {
            code,
            message: "WebSocket frame rejected",
        },
    )
    .await
}

async fn prepare_pubsub(client: &redis::Client) -> Result<redis::aio::PubSub, redis::RedisError> {
    let mut pubsub = tokio::time::timeout(REDIS_CONNECT_TIMEOUT, client.get_async_pubsub())
        .await
        .map_err(|_| redis_timeout_error())??;
    tokio::time::timeout(REDIS_CONNECT_TIMEOUT, pubsub.subscribe(REDIS_BUS_CHANNEL))
        .await
        .map_err(|_| redis_timeout_error())??;
    Ok(pubsub)
}

fn redis_timeout_error() -> redis::RedisError {
    redis::RedisError::from((redis::ErrorKind::IoError, "Redis connection timed out"))
}

async fn redis_listener_loop(
    client: redis::Client,
    mut pubsub: Option<redis::aio::PubSub>,
    hub: ConnectionHub,
    connected: Arc<AtomicBool>,
) {
    loop {
        if pubsub.is_none() {
            match prepare_pubsub(&client).await {
                Ok(new_pubsub) => pubsub = Some(new_pubsub),
                Err(error) => {
                    connected.store(false, Ordering::Release);
                    tracing::warn!(service = super::SERVICE_NAME, error_kind = ?error.kind(), "Redis pub/sub unavailable");
                    tokio::time::sleep(REDIS_RECONNECT_DELAY).await;
                    continue;
                }
            }
        }

        connected.store(true, Ordering::Release);
        let mut active = pubsub.take().expect("pubsub initialized");
        {
            let mut messages = active.on_message();
            while let Some(message) = messages.next().await {
                let payload = match message.get_payload::<String>() {
                    Ok(payload) => payload,
                    Err(_) => continue,
                };
                if let Ok(event) = serde_json::from_str::<BusEvent>(&payload) {
                    hub.dispatch(event);
                }
            }
        }
        connected.store(false, Ordering::Release);
        tokio::time::sleep(REDIS_RECONNECT_DELAY).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player(user_id: &str) -> platform_auth::AuthUser {
        platform_auth::AuthUser {
            user_id: user_id.to_string(),
            username: user_id.to_string(),
            email: None,
            role: "player".to_string(),
            universe_id: Some(7),
        }
    }

    #[test]
    fn channel_policy_is_identity_and_role_aware() {
        let player = player("player-1");
        assert!(authorize_channel_inner(&player, "player:player-1").is_ok());
        assert!(matches!(
            authorize_channel_inner(&player, "player:player-2"),
            Err(ChannelError::Forbidden)
        ));
        assert!(authorize_channel_inner(&player, "universe:7").is_ok());
        assert!(matches!(
            authorize_channel_inner(&player, "ops.scheduler"),
            Err(ChannelError::Forbidden)
        ));
    }

    #[tokio::test]
    async fn bounded_queue_disconnects_slow_consumers() {
        let hub = ConnectionHub::default();
        let handle = hub.register(player("slow"), 1);
        assert_eq!(hub.active_connections(), 1);
        assert_eq!(
            hub.dispatch(BusEvent {
                event_id: "event-1".into(),
                channel: "global".into(),
                event: "one".into(),
                sequence: 1,
            }),
            1
        );
        assert_eq!(
            hub.dispatch(BusEvent {
                event_id: "event-2".into(),
                channel: "global".into(),
                event: "two".into(),
                sequence: 2,
            }),
            0
        );
        assert_eq!(hub.active_connections(), 0);
        assert_eq!(hub.dropped_events.load(Ordering::Relaxed), 1);
        assert!(*handle.disconnect.borrow());
    }

    #[test]
    fn heartbeat_idle_policy_expires_only_at_timeout() {
        let start = tokio::time::Instant::now();
        let timeout = Duration::from_secs(90);
        assert!(!idle_expired(
            start,
            start + Duration::from_secs(89),
            timeout
        ));
        assert!(idle_expired(start, start + timeout, timeout));
    }
}
