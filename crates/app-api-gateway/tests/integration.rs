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
async fn integration_health_works_and_fleet_fails_closed_without_repository() {
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
    assert_eq!(fleets_response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let fleets = json_body(fleets_response).await;
    assert!(!fleets["success"].as_bool().unwrap());
    assert_eq!(fleets["error"], "Fleet repository is unavailable");
}

#[tokio::test]
async fn integration_fleet_detail_fails_closed_without_repository() {
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
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = json_body(response).await;
    assert!(!body["success"].as_bool().unwrap());
    assert_eq!(body["error"], "Fleet repository is unavailable");
}
