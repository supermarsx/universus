use app_realtime_gateway::build_router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use hyper::body::to_bytes;
use serde_json::{json, Value};
use tower::ServiceExt;

async fn json_body(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body()).await.expect("read response body");
    serde_json::from_slice(&bytes).expect("parse response json")
}

#[tokio::test]
async fn health_endpoint_returns_service_status() {
    let app = build_router();

    let response = app
        .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
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
    assert_eq!(channels_body["data"]["channels"][0]["name"], "alliance-updates");
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
                .body(Body::from(json!({ "channel": "", "event": "" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(publish_response.status(), StatusCode::BAD_REQUEST);
    let body = json_body(publish_response).await;
    assert_eq!(body["status"], "error");
    assert_eq!(body["error"], "channel and event are required");
}
