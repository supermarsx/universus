use app_api_gateway::routes::build_router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use hyper::body::to_bytes;
use serde_json::{json, Value};
use tower::ServiceExt;

const TEST_SERVICE_NAME: &str = "app-api-gateway";

async fn response_json(response: axum::response::Response) -> Value {
    let body = to_bytes(response.into_body()).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn health_returns_200_and_service_name() {
    let app = build_router(TEST_SERVICE_NAME);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["service"], TEST_SERVICE_NAME);
}

#[tokio::test]
async fn helper_movement_invalid_payload_returns_400_and_error_envelope() {
    let app = build_router(TEST_SERVICE_NAME);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/fleet/helpers/movement")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"origin":{}}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response_json(response).await;
    assert_eq!(body["success"], false);
    assert!(body["error"].is_string());
}

#[tokio::test]
async fn helper_movement_valid_payload_returns_200_and_distance_data() {
    let app = build_router(TEST_SERVICE_NAME);
    let payload = json!({
        "origin": { "galaxy": 1, "system": 1, "position": 1 },
        "target": { "galaxy": 1, "system": 2, "position": 1 },
        "ships": { "lightFighter": 10 }
    });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/fleet/helpers/movement")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["success"], true);
    assert!(body["data"]["distance"].is_number());
    assert_eq!(body["data"]["distance"], 2795);
}

#[tokio::test]
async fn auth_login_returns_success_envelope_and_token() {
    let app = build_router(TEST_SERVICE_NAME);
    let payload = json!({
        "email": "commander@example.com",
        "password": "secret"
    });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/auth/login")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["success"], true);
    assert!(body["data"]["token"].is_string());
    assert_eq!(body["data"]["user"]["email"], "commander@example.com");
}

#[tokio::test]
async fn fleet_move_alias_matches_movement_response_shape() {
    let app = build_router(TEST_SERVICE_NAME);
    let payload = json!({
        "origin_galaxy": 1,
        "origin_system": 1,
        "origin_position": 1,
        "target_galaxy": 1,
        "target_system": 2,
        "target_position": 1,
        "ships": [
          { "count": 10, "base_speed": 1000.0, "fuel_consumption": 2.0, "cargo": 50.0 }
        ]
    });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/fleet/move")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["distance"], 2795);
}

#[tokio::test]
async fn planets_list_returns_success_envelope() {
    let app = build_router(TEST_SERVICE_NAME);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/planets")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["success"], true);
    assert!(body["data"].is_array());
}

#[tokio::test]
async fn alliance_members_returns_success_array() {
    let app = build_router(TEST_SERVICE_NAME);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/alliance/members")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["success"], true);
    assert!(body["data"].is_array());
    assert_eq!(body["data"][0]["username"], "Commander");
}

#[tokio::test]
async fn messages_unread_count_returns_success_envelope() {
    let app = build_router(TEST_SERVICE_NAME);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/messages/unread-count")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["unreadCount"], 1);
}

#[tokio::test]
async fn leaderboard_global_returns_ranked_entries() {
    let app = build_router(TEST_SERVICE_NAME);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/leaderboard")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["scope"], "global");
    assert_eq!(body["data"]["entries"][0]["rank"], 1);
}

#[tokio::test]
async fn galaxy_system_returns_requested_coordinates() {
    let app = build_router(TEST_SERVICE_NAME);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/galaxy/1/120")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["galaxy"], 1);
    assert_eq!(body["data"]["system"], 120);
}

#[tokio::test]
async fn shop_purchase_preview_returns_calculated_total() {
    let app = build_router(TEST_SERVICE_NAME);
    let payload = json!({
        "package_id": "pkg-small",
        "quantity": 3
    });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/shop/purchase-preview")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["totalDarkMatter"], 2700);
}

#[tokio::test]
async fn research_cost_for_known_tech_returns_payload() {
    let app = build_router(TEST_SERVICE_NAME);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/research/energy_tech/cost")
                .method("POST")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["nextLevel"], 12);
}

#[tokio::test]
async fn shipyard_build_preview_requires_positive_count() {
    let app = build_router(TEST_SERVICE_NAME);
    let payload = json!({
        "ship_type": "lightFighter",
        "count": 0
    });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/shipyard/p-001/build-preview")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response_json(response).await;
    assert_eq!(body["success"], false);
    assert_eq!(body["error"], "Count must be greater than zero");
}
