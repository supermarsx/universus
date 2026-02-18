use axum::body::Body;
use axum::http::{Request, StatusCode};
use hyper::body::to_bytes;
use serde_json::Value;
use tower::ServiceExt;

use app_api_gateway::routes::build_router;

async fn json_body(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body())
        .await
        .expect("read body bytes");
    serde_json::from_slice(&bytes).expect("parse json")
}

#[tokio::test]
async fn integration_health_and_game_routes_work() {
    let app = build_router("integration");

    let health_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(health_response.status(), StatusCode::OK);
    let health_body = json_body(health_response).await;
    assert_eq!(health_body["service"], "integration");

    let fleets_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/fleet")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(fleets_response.status(), StatusCode::OK);
    let fleets = json_body(fleets_response).await;
    assert!(fleets["success"].as_bool().unwrap());
    let data = fleets["data"].as_array().expect("data array");
    assert!(!data.is_empty());
    assert!(data[0]["fleetId"].as_str().is_some());

    let detail_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/fleet/f-1001")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(detail_response.status(), StatusCode::OK);
    let detail = json_body(detail_response).await;
    assert_eq!(detail["data"]["fleetId"], "f-1001");
}

#[tokio::test]
async fn integration_handles_missing_fleet() {
    let app = build_router("integration");

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/fleet/unknown")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = json_body(response).await;
    assert!(!body["success"].as_bool().unwrap());
    assert!(body["error"].as_str().unwrap().contains("Fleet not found"));
}
