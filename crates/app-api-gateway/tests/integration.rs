use axum::body::Body;
use axum::http::{Request, StatusCode};
use hyper::body::to_bytes;
use serde_json::Value;
use tower::ServiceExt;

use app_api_gateway::accounts::AccountRepository;
use app_api_gateway::routes::build_router_with_dependencies;

fn build_router(service_name: &'static str) -> axum::Router {
    build_router_with_dependencies(service_name, None, AccountRepository::in_memory())
}

fn dev_token() -> String {
    let config = platform_auth::AuthConfig {
        jwt_secret: "default-secret".to_string(),
        jwt_expiry_seconds: 86_400,
        ..platform_auth::AuthConfig::default()
    };
    platform_auth::generate_token(&config, "u-rust-1", "Commander", "player", Some(1)).unwrap()
}

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
                .header("authorization", format!("Bearer {}", dev_token()))
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
                .header("authorization", format!("Bearer {}", dev_token()))
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
                .header("authorization", format!("Bearer {}", dev_token()))
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
