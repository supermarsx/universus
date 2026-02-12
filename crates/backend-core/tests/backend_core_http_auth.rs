#[path = "../src/bin/backend-core-http.rs"]
mod backend_core_http;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::{json, Value};
use tower::ServiceExt;

fn movement_payload() -> Value {
    json!({
        "origin": { "galaxy": 1, "system": 1, "position": 1 },
        "target": { "galaxy": 1, "system": 1, "position": 2 },
        "ships": { "fighter": 1 }
    })
}

#[tokio::test]
async fn health_is_public_when_helper_token_is_configured() {
    let app = backend_core_http::build_app(Some("top-secret".to_string()));

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/health")
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("serve request");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn helper_route_returns_unauthorized_when_token_header_missing() {
    let app = backend_core_http::build_app(Some("top-secret".to_string()));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/fleet/helpers/movement")
                .header("content-type", "application/json")
                .body(Body::from(movement_payload().to_string()))
                .expect("build request"),
        )
        .await
        .expect("serve request");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let body = hyper::body::to_bytes(response.into_body())
        .await
        .expect("read body");
    let json: Value = serde_json::from_slice(&body).expect("parse response json");
    assert_eq!(json, json!({ "success": false, "error": "Unauthorized" }));
}

#[tokio::test]
async fn helper_route_returns_unauthorized_when_token_header_is_invalid() {
    let app = backend_core_http::build_app(Some("top-secret".to_string()));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/fleet/helpers/movement")
                .header("content-type", "application/json")
                .header("x-core-helper-token", "wrong-token")
                .body(Body::from(movement_payload().to_string()))
                .expect("build request"),
        )
        .await
        .expect("serve request");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn helper_route_accepts_exact_matching_token() {
    let app = backend_core_http::build_app(Some("top-secret".to_string()));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/fleet/helpers/movement")
                .header("content-type", "application/json")
                .header("x-core-helper-token", "top-secret")
                .body(Body::from(movement_payload().to_string()))
                .expect("build request"),
        )
        .await
        .expect("serve request");

    assert_eq!(response.status(), StatusCode::OK);

    let body = hyper::body::to_bytes(response.into_body())
        .await
        .expect("read body");
    let json: Value = serde_json::from_slice(&body).expect("parse response json");
    assert_eq!(json.get("success"), Some(&Value::Bool(true)));
}
