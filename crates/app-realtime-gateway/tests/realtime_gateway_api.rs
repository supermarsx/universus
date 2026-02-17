use app_realtime_gateway::build_router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use hyper::body::to_bytes;
use serde_json::{json, Value};
use tower::ServiceExt;

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
async fn subscribe_creates_channel_and_updates_channel_listing() {
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

    assert_eq!(subscribe_response.status(), StatusCode::OK);
    let subscribe_body = json_body(subscribe_response).await;
    assert_eq!(subscribe_body["status"], "ok");
    assert_eq!(subscribe_body["data"]["channel"], "alliance-updates");
    assert_eq!(subscribe_body["data"]["subscriber_id"], "player-7");
    assert_eq!(subscribe_body["data"]["subscriber_count"], 1);

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
    assert_eq!(
        channels_body["data"]["channels"][0]["name"],
        "alliance-updates"
    );
    assert_eq!(channels_body["data"]["channels"][0]["subscriber_count"], 1);
}

#[tokio::test]
async fn publish_reports_delivered_subscribers_and_sequence() {
    let app = build_router();

    for subscriber_id in ["player-1", "player-2"] {
        let payload = json!({
            "channel": "battle-feed",
            "subscriber_id": subscriber_id
        });

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/realtime/subscribe")
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    let publish_payload = json!({
        "channel": "battle-feed",
        "event": "fleet_arrived"
    });

    let publish_response = app
        .clone()
        .oneshot(
            Request::builder()
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
    assert_eq!(publish_body["data"]["delivered_to"], 2);
    assert_eq!(publish_body["data"]["publish_sequence"], 1);
}

#[tokio::test]
async fn publish_validates_required_fields() {
    let app = build_router();

    let publish_response = app
        .oneshot(
            Request::builder()
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
            Request::builder()
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
            Request::builder()
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
    assert!(events_body["events"][0]["event"].as_str().unwrap().contains("scheduler.tick"));
}

#[tokio::test]
async fn chat_restrictions_endpoint_returns_empty_list_without_database() {
    let app = build_router();

    let response = app
        .oneshot(
            Request::builder()
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
            Request::builder()
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
            Request::builder()
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

    let delete_forbidden = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/realtime/chat/messages/msg-1")
                .method(Method::DELETE)
                .header("content-type", "application/json")
                .body(Body::from(json!({ "userId": 99, "isAdmin": false }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_forbidden.status(), StatusCode::FORBIDDEN);

    let delete_admin = app
        .oneshot(
            Request::builder()
                .uri("/api/realtime/chat/messages/msg-1")
                .method(Method::DELETE)
                .header("content-type", "application/json")
                .body(Body::from(json!({ "userId": 99, "isAdmin": true }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_admin.status(), StatusCode::OK);
    let delete_body = json_body(delete_admin).await;
    assert_eq!(delete_body["success"], true);
}
