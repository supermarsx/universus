use app_realtime_gateway::build_router;
use axum::body::Body;
use axum::http::{Method, Request as HttpRequest, StatusCode};
use futures_util::{SinkExt, StreamExt};
use hyper::body::to_bytes;
use serde_json::{json, Value};
use tokio::time::{timeout, Duration};
use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Error as WebSocketError, Message};
use tower::ServiceExt;

struct Request;
struct AdminRequest;

fn authenticated_request(role: &str, user_id: &str) -> axum::http::request::Builder {
    let token = auth_token(role, user_id);
    HttpRequest::builder().header("authorization", format!("Bearer {token}"))
}

fn auth_token(role: &str, user_id: &str) -> String {
    let config = platform_auth::AuthConfig::from_env();
    platform_auth::generate_token(&config, user_id, "Route Test", role, Some(7))
        .expect("generate route token")
}

impl Request {
    fn builder() -> axum::http::request::Builder {
        authenticated_request("player", "11")
    }
}

impl AdminRequest {
    fn builder() -> axum::http::request::Builder {
        authenticated_request("admin", "1")
    }
}

struct TestServer {
    http_url: String,
    ws_url: String,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn spawn_gateway() -> TestServer {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind test gateway");
    listener
        .set_nonblocking(true)
        .expect("set listener nonblocking");
    let address = listener.local_addr().expect("test gateway address");
    let server = axum::Server::from_tcp(listener)
        .expect("test server")
        .serve(build_router().into_make_service());
    let task = tokio::spawn(async move {
        let _ = server.await;
    });
    TestServer {
        http_url: format!("http://{address}"),
        ws_url: format!("ws://{address}/ws"),
        task,
    }
}

type TestSocket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn connect_player(server: &TestServer, user_id: &str) -> TestSocket {
    let token = auth_token("player", user_id);
    let mut request = server
        .ws_url
        .as_str()
        .into_client_request()
        .expect("websocket request");
    request.headers_mut().insert(
        "cookie",
        format!("universus_token={token}").parse().unwrap(),
    );
    request
        .headers_mut()
        .insert("origin", server.http_url.parse().unwrap());
    let (mut socket, response) = tokio_tungstenite::connect_async(request)
        .await
        .expect("connect websocket");
    assert_eq!(response.status(), StatusCode::SWITCHING_PROTOCOLS);
    let ready = next_ws_json(&mut socket).await;
    assert_eq!(ready["type"], "ready");
    assert_eq!(ready["user_id"], user_id);
    socket
}

async fn next_ws_json(socket: &mut TestSocket) -> Value {
    let message = timeout(Duration::from_secs(2), socket.next())
        .await
        .expect("websocket response timeout")
        .expect("websocket stream ended")
        .expect("websocket response");
    match message {
        Message::Text(text) => serde_json::from_str(&text).expect("websocket json"),
        other => panic!("expected websocket text frame, got {other:?}"),
    }
}

async fn publish_over_http(server: &TestServer, channel: &str, event: &str) -> (StatusCode, Value) {
    let request = HttpRequest::builder()
        .method(Method::POST)
        .uri(format!("{}/api/realtime/publish", server.http_url))
        .header(
            "authorization",
            format!("Bearer {}", auth_token("admin", "1")),
        )
        .header("content-type", "application/json")
        .body(Body::from(
            json!({ "channel": channel, "event": event }).to_string(),
        ))
        .expect("publish request");
    let response = hyper::Client::new()
        .request(request)
        .await
        .expect("publish response");
    let status = response.status();
    (status, network_json_body(response).await)
}

async fn network_json_body(response: hyper::Response<hyper::Body>) -> Value {
    let bytes = to_bytes(response.into_body())
        .await
        .expect("read network response body");
    serde_json::from_slice(&bytes).expect("parse network response json")
}

async fn json_body(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body())
        .await
        .expect("read response body");
    serde_json::from_slice(&bytes).expect("parse response json")
}

#[tokio::test]
async fn health_endpoint_returns_service_status() {
    let app = build_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["status"], "ok");
    assert_eq!(body["service"], "app-realtime-gateway");
}

#[tokio::test]
async fn websocket_upgrade_rejects_missing_and_invalid_authentication() {
    let server = spawn_gateway().await;

    let missing = tokio_tungstenite::connect_async(server.ws_url.as_str())
        .await
        .expect_err("anonymous upgrade must fail");
    match missing {
        WebSocketError::Http(response) => {
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED)
        }
        other => panic!("expected HTTP authentication rejection, got {other:?}"),
    }

    let mut request = server
        .ws_url
        .as_str()
        .into_client_request()
        .expect("invalid websocket request");
    request.headers_mut().insert(
        "cookie",
        "universus_token=not-a-valid-token".parse().unwrap(),
    );
    let invalid = tokio_tungstenite::connect_async(request)
        .await
        .expect_err("invalid token upgrade must fail");
    match invalid {
        WebSocketError::Http(response) => {
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED)
        }
        other => panic!("expected HTTP authentication rejection, got {other:?}"),
    }

    let token = auth_token("player", "origin-test");
    let mut request = server
        .ws_url
        .as_str()
        .into_client_request()
        .expect("cross-origin websocket request");
    request.headers_mut().insert(
        "cookie",
        format!("universus_token={token}").parse().unwrap(),
    );
    request
        .headers_mut()
        .insert("origin", "https://attacker.example".parse().unwrap());
    let cross_origin = tokio_tungstenite::connect_async(request)
        .await
        .expect_err("cross-origin cookie upgrade must fail");
    match cross_origin {
        WebSocketError::Http(response) => assert_eq!(response.status(), StatusCode::FORBIDDEN),
        other => panic!("expected origin rejection, got {other:?}"),
    }

    let mut request = server
        .ws_url
        .as_str()
        .into_client_request()
        .expect("origin-bearing Authorization request");
    request.headers_mut().insert(
        "authorization",
        format!("Bearer {}", auth_token("player", "header-origin-test"))
            .parse()
            .unwrap(),
    );
    request
        .headers_mut()
        .insert("origin", "https://attacker.example".parse().unwrap());
    let cross_origin = tokio_tungstenite::connect_async(request)
        .await
        .expect_err("origin-bearing Authorization upgrade must follow origin policy");
    match cross_origin {
        WebSocketError::Http(response) => assert_eq!(response.status(), StatusCode::FORBIDDEN),
        other => panic!("expected Authorization origin rejection, got {other:?}"),
    }
}

#[tokio::test]
async fn websocket_delivery_is_isolated_and_unsubscribe_stops_fanout() {
    let server = spawn_gateway().await;
    let mut player_one = connect_player(&server, "player-one").await;
    let mut player_two = connect_player(&server, "player-two").await;

    let (status, publish) = publish_over_http(&server, "player:player-one", "fleet_arrived").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(publish["data"]["delivered_to"], 1);
    assert_eq!(publish["data"]["local_subscribers"], 1);
    assert_eq!(publish["data"]["delivery_scope"], "local_process");
    assert_eq!(publish["data"]["accepted"], true);

    let event = next_ws_json(&mut player_one).await;
    assert_eq!(event["type"], "event");
    assert_eq!(event["channel"], "player:player-one");
    assert_eq!(event["event"], "fleet_arrived");
    assert!(timeout(Duration::from_millis(150), player_two.next())
        .await
        .is_err());

    player_one
        .send(Message::Text(
            json!({ "type": "unsubscribe", "channel": "player:player-one" }).to_string(),
        ))
        .await
        .expect("unsubscribe frame");
    let unsubscribed = next_ws_json(&mut player_one).await;
    assert_eq!(unsubscribed["type"], "unsubscribed");

    let (_, publish) = publish_over_http(&server, "player:player-one", "second").await;
    assert_eq!(publish["data"]["delivered_to"], 0);
    assert!(timeout(Duration::from_millis(150), player_one.next())
        .await
        .is_err());
}

#[tokio::test]
async fn canonical_notification_channel_delivers_only_to_its_signed_user() {
    let server = spawn_gateway().await;
    let mut owner = connect_player(&server, "notification-owner").await;
    let mut other = connect_player(&server, "notification-other").await;
    let channel = platform_events::user_notification_channel("notification-owner");

    other
        .send(Message::Text(
            json!({ "type": "subscribe", "channel": channel }).to_string(),
        ))
        .await
        .expect("other user subscribe frame");
    let forbidden = next_ws_json(&mut other).await;
    assert_eq!(forbidden["type"], "error");
    assert_eq!(forbidden["code"], "forbidden_channel");

    let envelope =
        platform_events::build_event("notification.created", &json!({ "notificationId": 42 }));
    let status = platform_events::publish_http(&server.http_url, &channel, &envelope)
        .await
        .expect("canonical notification publish");
    assert_eq!(status, StatusCode::OK.as_u16());

    let event = next_ws_json(&mut owner).await;
    assert_eq!(event["type"], "event");
    assert_eq!(event["channel"], channel);
    let delivered: platform_events::EventEnvelope =
        serde_json::from_str(event["event"].as_str().expect("event envelope string"))
            .expect("event envelope JSON");
    assert_eq!(delivered.event_type, "notification.created");
    assert!(timeout(Duration::from_millis(150), other.next())
        .await
        .is_err());
}

#[tokio::test]
async fn websocket_rejects_forbidden_and_malformed_frames_and_answers_ping() {
    let server = spawn_gateway().await;
    let mut socket = connect_player(&server, "player-frame-test").await;

    socket
        .send(Message::Text(
            json!({ "type": "subscribe", "channel": "ops.scheduler" }).to_string(),
        ))
        .await
        .expect("forbidden subscribe frame");
    let forbidden = next_ws_json(&mut socket).await;
    assert_eq!(forbidden["type"], "error");
    assert_eq!(forbidden["code"], "forbidden_channel");

    socket
        .send(Message::Text("{".to_string()))
        .await
        .expect("malformed frame");
    let malformed = next_ws_json(&mut socket).await;
    assert_eq!(malformed["type"], "error");
    assert_eq!(malformed["code"], "malformed_frame");

    socket
        .send(Message::Text(
            json!({ "type": "ping", "nonce": "roundtrip-1" }).to_string(),
        ))
        .await
        .expect("ping frame");
    let pong = next_ws_json(&mut socket).await;
    assert_eq!(pong["type"], "pong");
    assert_eq!(pong["nonce"], "roundtrip-1");
}

#[tokio::test]
async fn repeated_binary_protocol_errors_close_the_websocket() {
    let server = spawn_gateway().await;
    let mut socket = connect_player(&server, "binary-protocol-test").await;

    for _ in 0..3 {
        socket
            .send(Message::Binary(vec![1, 2, 3]))
            .await
            .expect("binary frame");
        let error = next_ws_json(&mut socket).await;
        assert_eq!(error["code"], "binary_not_supported");
    }

    let closed = timeout(Duration::from_secs(2), socket.next())
        .await
        .expect("close timeout")
        .expect("close frame missing")
        .expect("websocket close response");
    assert!(matches!(closed, Message::Close(_)));
}

#[tokio::test]
async fn websocket_close_cleans_up_connection_metrics() {
    let server = spawn_gateway().await;
    let mut socket = connect_player(&server, "player-disconnect").await;

    socket.close(None).await.expect("close websocket");
    let mut active_connections = usize::MAX;
    for _ in 0..50 {
        let request = HttpRequest::builder()
            .uri(format!("{}/ws-info", server.http_url))
            .body(Body::empty())
            .expect("ws-info request");
        let response = hyper::Client::new()
            .request(request)
            .await
            .expect("ws-info response");
        let body = network_json_body(response).await;
        active_connections = body["active_connections"].as_u64().unwrap_or(u64::MAX) as usize;
        if active_connections == 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(active_connections, 0);
}

#[tokio::test]
async fn realtime_auth_matrix_keeps_health_public_and_enforces_player_and_admin_roles() {
    let app = build_router();

    let public = app
        .clone()
        .oneshot(
            HttpRequest::builder()
                .uri("/ready")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(public.status(), StatusCode::OK);

    let anonymous = app
        .clone()
        .oneshot(
            HttpRequest::builder()
                .uri("/api/realtime/channels")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);

    let player = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/realtime/channels")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(player.status(), StatusCode::OK);

    let player_admin_operation = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/realtime/publish")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "channel": "ops.test", "event": "test" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(player_admin_operation.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn string_jwt_subjects_use_a_server_derived_legacy_numeric_id() {
    let app = build_router();
    let subject = "user:durable-account-id";

    let response = app
        .oneshot(
            authenticated_request("player", subject)
                .method(Method::PUT)
                .uri("/chat/messages/string-subject-message")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "userId": 999, "message": "Authenticated owner" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(
        body["message"]["userId"],
        platform_auth::stable_numeric_subject_id(subject)
    );
    assert_ne!(body["message"]["userId"], 999);
}

#[tokio::test]
async fn channels_endpoint_returns_empty_list_initially() {
    let app = build_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/realtime/channels")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["status"], "ok");
    assert_eq!(body["data"]["channels"], json!([]));
}

#[tokio::test]
async fn legacy_rest_subscribe_requires_websocket_upgrade() {
    let app = build_router();

    let subscribe_payload = json!({
        "channel": "alliance-updates",
        "subscriber_id": "player-7"
    });

    let subscribe_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/realtime/subscribe")
                .header("content-type", "application/json")
                .body(Body::from(subscribe_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(subscribe_response.status(), StatusCode::UPGRADE_REQUIRED);
    let subscribe_body = json_body(subscribe_response).await;
    assert_eq!(subscribe_body["status"], "error");
    assert!(subscribe_body["error"].as_str().unwrap().contains("/ws"));

    let channels_response = app
        .oneshot(
            Request::builder()
                .uri("/api/realtime/channels")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(channels_response.status(), StatusCode::OK);
    let channels_body = json_body(channels_response).await;
    assert_eq!(channels_body["status"], "ok");
    assert_eq!(channels_body["data"]["channels"], json!([]));
}

#[tokio::test]
async fn publish_reports_delivered_subscribers_and_sequence() {
    let app = build_router();

    let publish_payload = json!({
        "channel": "battle-feed",
        "event": "fleet_arrived"
    });

    let publish_response = app
        .clone()
        .oneshot(
            AdminRequest::builder()
                .method(Method::POST)
                .uri("/api/realtime/publish")
                .header("content-type", "application/json")
                .body(Body::from(publish_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(publish_response.status(), StatusCode::OK);
    let publish_body = json_body(publish_response).await;
    assert_eq!(publish_body["status"], "ok");
    assert_eq!(publish_body["data"]["channel"], "battle-feed");
    assert_eq!(publish_body["data"]["event"], "fleet_arrived");
    assert_eq!(publish_body["data"]["delivered_to"], 0);
    assert_eq!(publish_body["data"]["delivery_scope"], "local_process");
    assert_eq!(publish_body["data"]["publish_sequence"], 1);
}

#[tokio::test]
async fn publish_validates_required_fields() {
    let app = build_router();

    let publish_response = app
        .oneshot(
            AdminRequest::builder()
                .method(Method::POST)
                .uri("/api/realtime/publish")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "channel": "", "event": "" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(publish_response.status(), StatusCode::BAD_REQUEST);
    let body = json_body(publish_response).await;
    assert_eq!(body["status"], "error");
    assert_eq!(body["error"], "channel and event are required");
}

#[tokio::test]
async fn chat_channels_parity_endpoint_returns_list_shape() {
    let app = build_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/chat/channels")
                .method(Method::GET)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert!(body["channels"].is_array());
}

#[tokio::test]
async fn notifications_parity_endpoint_supports_unread_filter_and_paging() {
    let app = build_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/notifications?unreadOnly=true&limit=1&offset=0")
                .method(Method::GET)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert!(body["notifications"].is_array());
    assert!(body["total"].is_u64());
    assert_eq!(body["notifications"].as_array().unwrap().len(), 1);
    assert_eq!(body["notifications"][0]["read"], false);
}

#[tokio::test]
async fn players_online_parity_endpoint_supports_alliance_filter() {
    let app = build_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/players/online?allianceId=10")
                .method(Method::GET)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["count"], 1);
    assert_eq!(body["players"][0]["alliance_id"], 10);
}

#[tokio::test]
async fn trade_offers_parity_endpoint_defaults_to_active_status() {
    let app = build_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/trade/offers")
                .method(Method::GET)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert!(body["offers"].is_array());
    assert_eq!(body["total"], 1);
    assert_eq!(body["offers"][0]["status"], "active");
}

#[tokio::test]
async fn realtime_prefixed_parity_aliases_match_family_shapes() {
    let app = build_router();

    let chat = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/realtime/chat/channels")
                .method(Method::GET)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(chat.status(), StatusCode::OK);
    let chat_body = json_body(chat).await;
    assert!(chat_body["channels"].is_array());

    let notifications = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/realtime/notifications")
                .method(Method::GET)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(notifications.status(), StatusCode::OK);
    let notifications_body = json_body(notifications).await;
    assert!(notifications_body["notifications"].is_array());

    let players = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/realtime/players/online")
                .method(Method::GET)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(players.status(), StatusCode::OK);
    let players_body = json_body(players).await;
    assert!(players_body["players"].is_array());

    let trade = app
        .oneshot(
            Request::builder()
                .uri("/api/realtime/trade/offers")
                .method(Method::GET)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(trade.status(), StatusCode::OK);
    let trade_body = json_body(trade).await;
    assert!(trade_body["offers"].is_array());
}

#[tokio::test]
async fn notifications_unread_count_and_preferences_update_work() {
    let app = build_router();

    let unread = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/notifications/unread/count")
                .method(Method::GET)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unread.status(), StatusCode::OK);
    let unread_body = json_body(unread).await;
    assert_eq!(unread_body["unread_count"], 1);

    let update = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/notifications/preferences/trade")
                .method(Method::PUT)
                .header("content-type", "application/json")
                .body(Body::from(json!({ "enabled": false }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(update.status(), StatusCode::OK);
    let update_body = json_body(update).await;
    assert_eq!(update_body["preferences"]["trade"], false);

    let get_prefs = app
        .oneshot(
            Request::builder()
                .uri("/notifications/preferences")
                .method(Method::GET)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_prefs.status(), StatusCode::OK);
    let get_prefs_body = json_body(get_prefs).await;
    assert_eq!(get_prefs_body["preferences"]["trade"], false);
}

#[tokio::test]
async fn conversations_and_trade_history_endpoints_return_expected_shapes() {
    let app = build_router();

    let conv = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/chat/conversations")
                .method(Method::GET)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(conv.status(), StatusCode::OK);
    let conv_body = json_body(conv).await;
    assert!(conv_body["conversations"].is_array());
    assert!(conv_body["total"].is_u64());

    let messages = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/chat/conversations/conv-1/messages")
                .method(Method::GET)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(messages.status(), StatusCode::OK);
    let messages_body = json_body(messages).await;
    assert_eq!(messages_body["conversation_id"], "conv-1");
    assert!(messages_body["messages"].is_array());

    let history = app
        .oneshot(
            Request::builder()
                .uri("/trade/history")
                .method(Method::GET)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(history.status(), StatusCode::OK);
    let history_body = json_body(history).await;
    assert!(history_body["entries"].is_array());
    assert!(history_body["total"].is_u64());
}

#[tokio::test]
async fn recent_events_endpoint_tracks_published_events() {
    let app = build_router();

    let publish_payload = json!({
        "channel": "ops.scheduler",
        "event": "{\"eventType\":\"scheduler.tick\",\"payload\":{\"job\":\"game_loop\"}}"
    });
    let publish_response = app
        .clone()
        .oneshot(
            AdminRequest::builder()
                .method(Method::POST)
                .uri("/api/realtime/publish")
                .header("content-type", "application/json")
                .body(Body::from(publish_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(publish_response.status(), StatusCode::OK);

    let events_response = app
        .oneshot(
            AdminRequest::builder()
                .uri("/api/realtime/events/recent?limit=10")
                .method(Method::GET)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(events_response.status(), StatusCode::OK);
    let events_body = json_body(events_response).await;
    assert!(events_body["events"].is_array());
    assert!(events_body["total"].as_u64().unwrap() >= 1);
    assert_eq!(events_body["events"][0]["channel"], "ops.scheduler");
    assert!(events_body["events"][0]["event"]
        .as_str()
        .unwrap()
        .contains("scheduler.tick"));
}

#[tokio::test]
async fn chat_restrictions_endpoint_returns_empty_list_without_database() {
    let app = build_router();

    let response = app
        .oneshot(
            AdminRequest::builder()
                .uri("/api/realtime/chat/restrictions")
                .method(Method::GET)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert!(body["restrictions"].is_array());
    assert_eq!(body["total"], 0);
}

#[tokio::test]
async fn chat_restriction_upsert_requires_database_url() {
    let app = build_router();

    let response = app
        .oneshot(
            AdminRequest::builder()
                .uri("/api/realtime/chat/restrictions")
                .method(Method::POST)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "userId": 42,
                        "restrictionType": "mute",
                        "reason": "spam",
                        "restrictedBy": 1
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = json_body(response).await;
    assert_eq!(body["status"], "error");
    assert_eq!(body["error"], "DATABASE_URL not configured");
}

#[tokio::test]
async fn chat_message_moderation_endpoints_update_state() {
    let app = build_router();

    let edit = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/realtime/chat/messages/msg-1")
                .method(Method::PUT)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "userId": 11,
                        "message": "Updated message"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(edit.status(), StatusCode::OK);
    let edit_body = json_body(edit).await;
    assert_eq!(edit_body["message"]["edited"], true);
    assert_eq!(edit_body["message"]["message"], "Updated message");

    let pin = app
        .clone()
        .oneshot(
            AdminRequest::builder()
                .uri("/chat/messages/msg-1/pin")
                .method(Method::POST)
                .header("content-type", "application/json")
                .body(Body::from(json!({ "isPinned": true }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(pin.status(), StatusCode::OK);
    let pin_body = json_body(pin).await;
    assert_eq!(pin_body["message"]["pinned"], true);

    let reaction = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/realtime/chat/messages/msg-1/reactions")
                .method(Method::POST)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "reactionType": "clap",
                        "userId": 11
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reaction.status(), StatusCode::OK);
    let reaction_body = json_body(reaction).await;
    assert_eq!(reaction_body["reactionType"], "clap");
    assert_eq!(reaction_body["count"], 1);

    let forged_admin_claim = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/realtime/chat/messages/msg-1")
                .method(Method::DELETE)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "userId": 99, "isAdmin": true }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(forged_admin_claim.status(), StatusCode::FORBIDDEN);

    let delete_admin = app
        .oneshot(
            AdminRequest::builder()
                .uri("/api/realtime/chat/messages/msg-1")
                .method(Method::DELETE)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "userId": 99, "isAdmin": false }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_admin.status(), StatusCode::OK);
    let delete_body = json_body(delete_admin).await;
    assert_eq!(delete_body["success"], true);
}
