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

async fn create_test_bot(app: &axum::Router, username: &str) -> u64 {
    let payload = json!({
        "username": username,
        "email": format!("{username}@example.com"),
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

    let create_body = response_json(create_response).await;
    create_body["data"]["id"].as_u64().unwrap()
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

#[tokio::test]
async fn disable_then_enable_updates_active_state() {
    let app = build_router();
    let bot_id = create_test_bot(&app, "bot_toggle").await;

    let disable_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/admin/bots/{bot_id}/disable"))
                .method("POST")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(disable_response.status(), StatusCode::OK);
    let disable_body = response_json(disable_response).await;
    assert_eq!(disable_body["data"]["is_active"], false);

    let enable_response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/admin/bots/{bot_id}/enable"))
                .method("POST")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(enable_response.status(), StatusCode::OK);
    let enable_body = response_json(enable_response).await;
    assert_eq!(enable_body["data"]["is_active"], true);
}

#[tokio::test]
async fn actions_endpoint_returns_action_log_entries() {
    let app = build_router();
    let bot_id = create_test_bot(&app, "bot_actions").await;

    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/admin/bots/{bot_id}/disable"))
                .method("POST")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/admin/bots/{bot_id}/enable"))
                .method("POST")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/admin/bots/{bot_id}/actions"))
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    let actions = body["data"].as_array().unwrap();
    assert!(actions.len() >= 3);
    assert_eq!(actions[0]["action"], "enabled");
    assert_eq!(actions[1]["action"], "disabled");
    assert_eq!(actions[2]["action"], "created");
}

#[tokio::test]
async fn statistics_endpoint_counts_operations() {
    let app = build_router();
    let bot_id = create_test_bot(&app, "bot_stats").await;

    let update_payload = json!({
        "difficulty_level": 8,
        "think_interval_minutes": 15
    });
    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/admin/bots/{bot_id}"))
                .method("PUT")
                .header("content-type", "application/json")
                .body(Body::from(update_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/admin/bots/{bot_id}/disable"))
                .method("POST")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/admin/bots/{bot_id}/think"))
                .method("POST")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/admin/bots/{bot_id}/statistics"))
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["data"]["created_count"], 1);
    assert_eq!(body["data"]["update_count"], 1);
    assert_eq!(body["data"]["disable_count"], 1);
    assert_eq!(body["data"]["think_count"], 1);
    assert_eq!(body["data"]["total_actions"], 4);
}

#[tokio::test]
async fn actions_for_missing_bot_returns_not_found() {
    let app = build_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/admin/bots/404/actions")
                .method("GET")
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
async fn actions_endpoint_supports_limit_and_action_type_filters() {
    let app = build_router();
    let bot_id = create_test_bot(&app, "bot_filtered_actions").await;

    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/admin/bots/{bot_id}/disable"))
                .method("POST")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/admin/bots/{bot_id}/enable"))
                .method("POST")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let filtered_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/admin/bots/{bot_id}/actions?limit=1&action_type=enabled"
                ))
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(filtered_response.status(), StatusCode::OK);
    let filtered_body = response_json(filtered_response).await;
    let filtered_actions = filtered_body["data"].as_array().unwrap();
    assert_eq!(filtered_actions.len(), 1);
    assert_eq!(filtered_actions[0]["action"], "enabled");
}

#[tokio::test]
async fn bulk_create_bots_returns_requested_and_created_counts() {
    let app = build_router();
    let payload = json!({
        "count": 3,
        "personality_type": "tech_enthusiast",
        "difficulty_level": 6
    });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/admin/bots/bulk")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body = response_json(response).await;
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["requested"], 3);
    assert_eq!(body["data"]["created"], 3);
}

#[tokio::test]
async fn universe_generate_route_returns_success_shape() {
    let app = build_router();
    let payload = json!({
        "botCount": 12,
        "personalities": ["aggressive_conqueror"],
        "skillLevels": ["medium"],
        "distributeEvenly": true
    });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/admin/bots/universe/7/generate")
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
    assert_eq!(body["botsGenerated"], 12);
}
