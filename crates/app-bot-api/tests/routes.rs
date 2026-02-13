use app_bot_api::build_router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use hyper::body::to_bytes;
use serde_json::{json, Value};
use tower::ServiceExt;

async fn response_json(response: axum::response::Response) -> Value {
    let body = to_bytes(response.into_body()).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn list_bots_returns_success_with_data_array() {
    let app = build_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/admin/bots")
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
async fn create_then_get_bot_returns_expected_shape() {
    let app = build_router();
    let payload = json!({
        "username": "bot_alpha",
        "email": "bot_alpha@example.com",
        "personality_type": "aggressive_conqueror",
        "difficulty_level": 7
    });

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/admin/bots")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(create_response.status(), StatusCode::CREATED);
    let create_body = response_json(create_response).await;
    assert_eq!(create_body["success"], true);
    let bot_id = create_body["data"]["id"].as_u64().unwrap();

    let get_response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/admin/bots/{bot_id}"))
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(get_response.status(), StatusCode::OK);
    let get_body = response_json(get_response).await;
    assert_eq!(get_body["success"], true);
    assert_eq!(get_body["data"]["bot"]["username"], "bot_alpha");
    assert!(get_body["data"]["recentActions"].is_array());
}

#[tokio::test]
async fn personalities_list_returns_eight_items() {
    let app = build_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/admin/bots/personalities/list")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["success"], true);
    assert_eq!(body["data"].as_array().unwrap().len(), 8);
}

#[tokio::test]
async fn think_missing_bot_returns_404() {
    let app = build_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/admin/bots/think/999")
                .method("POST")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = response_json(response).await;
    assert_eq!(body["success"], false);
    assert_eq!(body["error"], "Bot not found");
}

#[tokio::test]
async fn process_all_returns_trigger_message() {
    let app = build_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/admin/bots/process/all")
                .method("POST")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["success"], true);
    assert_eq!(body["message"], "Bot processing triggered");
}
