use app_api_gateway::routes::build_router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use hyper::body::to_bytes;
use serde_json::{json, Value};
use tower::ServiceExt;

const TEST_SERVICE_NAME: &str = "app-api-gateway";
const DEV_TOKEN: &str = "dev-token";

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

#[tokio::test]
async fn account_profile_without_auth_returns_401() {
    let app = build_router(TEST_SERVICE_NAME);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/account/profile")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let body = response_json(response).await;
    assert_eq!(body["success"], false);
    assert_eq!(body["error"], "Unauthorized");
}

#[tokio::test]
async fn account_profile_with_valid_bearer_token_returns_200() {
    let app = build_router(TEST_SERVICE_NAME);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/account/profile")
                .method("GET")
                .header("authorization", format!("Bearer {DEV_TOKEN}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["username"], "Commander");
}

#[tokio::test]
async fn account_resources_with_invalid_token_returns_401() {
    let app = build_router(TEST_SERVICE_NAME);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/account/resources")
                .method("GET")
                .header("authorization", "Bearer wrong-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let body = response_json(response).await;
    assert_eq!(body["success"], false);
    assert_eq!(body["error"], "Unauthorized");
}

#[tokio::test]
async fn fleet_send_without_auth_returns_401() {
    let app = build_router(TEST_SERVICE_NAME);
    let payload = json!({
        "mission": "attack",
        "target": "[1:123:7]",
        "ships": [
            { "shipType": "lightFighter", "count": 20 }
        ]
    });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/fleet/send")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let body = response_json(response).await;
    assert_eq!(body["success"], false);
    assert_eq!(body["error"], "Unauthorized");
}

#[tokio::test]
async fn fleet_send_with_auth_returns_success_envelope() {
    let app = build_router(TEST_SERVICE_NAME);
    let payload = json!({
        "mission": "attack",
        "target": "[1:123:7]",
        "ships": [
            { "shipType": "lightFighter", "count": 20 },
            { "shipType": "cruiser", "count": 5 }
        ]
    });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/fleet/send")
                .method("POST")
                .header("authorization", format!("Bearer {DEV_TOKEN}"))
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["accepted"], true);
    assert_eq!(body["data"]["totalShips"], 25);
}

#[tokio::test]
async fn account_resources_drop_after_research_start() {
    let app = build_router(TEST_SERVICE_NAME);

    let initial_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/account/resources")
                .method("GET")
                .header("authorization", format!("Bearer {DEV_TOKEN}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(initial_response.status(), StatusCode::OK);
    let initial_body = response_json(initial_response).await;

    let start_payload = json!({
        "planetId": "p-001",
        "technologyType": "energy_technology"
    });
    let start_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/research/start")
                .method("POST")
                .header("authorization", format!("Bearer {DEV_TOKEN}"))
                .header("content-type", "application/json")
                .body(Body::from(start_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(start_response.status(), StatusCode::OK);
    let start_body = response_json(start_response).await;
    assert_eq!(start_body["success"], true);
    assert_eq!(start_body["data"]["queued"], true);

    let final_response = app
        .oneshot(
            Request::builder()
                .uri("/api/account/resources")
                .method("GET")
                .header("authorization", format!("Bearer {DEV_TOKEN}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(final_response.status(), StatusCode::OK);
    let final_body = response_json(final_response).await;

    assert_eq!(
        final_body["data"]["metal"].as_i64().unwrap(),
        initial_body["data"]["metal"].as_i64().unwrap() - 24_000
    );
    assert_eq!(
        final_body["data"]["crystal"].as_i64().unwrap(),
        initial_body["data"]["crystal"].as_i64().unwrap() - 12_000
    );
    assert_eq!(
        final_body["data"]["deuterium"].as_i64().unwrap(),
        initial_body["data"]["deuterium"].as_i64().unwrap() - 5_000
    );
}

#[tokio::test]
async fn research_start_rejects_unknown_technology() {
    let app = build_router(TEST_SERVICE_NAME);
    let payload = json!({
        "planetId": "p-001",
        "technologyType": "invalid_tech"
    });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/research/start")
                .method("POST")
                .header("authorization", format!("Bearer {DEV_TOKEN}"))
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response_json(response).await;
    assert_eq!(body["success"], false);
    assert_eq!(body["error"], "Research technology not found");
}

#[tokio::test]
async fn fleet_send_records_mission_sequence() {
    let app = build_router(TEST_SERVICE_NAME);
    let payload = json!({
        "mission": "attack",
        "target": "[1:123:7]",
        "ships": [
            { "shipType": "lightFighter", "count": 10 }
        ]
    });

    let first_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/fleet/send")
                .method("POST")
                .header("authorization", format!("Bearer {DEV_TOKEN}"))
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first_response.status(), StatusCode::OK);
    let first_body = response_json(first_response).await;
    assert_eq!(first_body["data"]["commandId"], "cmd-fleet-001");

    let second_response = app
        .oneshot(
            Request::builder()
                .uri("/api/fleet/send")
                .method("POST")
                .header("authorization", format!("Bearer {DEV_TOKEN}"))
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second_response.status(), StatusCode::OK);
    let second_body = response_json(second_response).await;
    assert_eq!(second_body["data"]["commandId"], "cmd-fleet-002");
}

#[tokio::test]
async fn shipyard_build_queues_and_decreases_resources() {
    let app = build_router(TEST_SERVICE_NAME);

    let initial_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/account/resources")
                .method("GET")
                .header("authorization", format!("Bearer {DEV_TOKEN}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(initial_response.status(), StatusCode::OK);
    let initial_body = response_json(initial_response).await;

    let payload = json!({
        "planetId": "p-001",
        "shipType": "small_cargo",
        "quantity": 2
    });
    let build_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/shipyard/build")
                .method("POST")
                .header("authorization", format!("Bearer {DEV_TOKEN}"))
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(build_response.status(), StatusCode::OK);
    let build_body = response_json(build_response).await;
    assert_eq!(build_body["success"], true);
    assert_eq!(build_body["data"]["orderId"], "o-p001-001");

    let final_response = app
        .oneshot(
            Request::builder()
                .uri("/api/account/resources")
                .method("GET")
                .header("authorization", format!("Bearer {DEV_TOKEN}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(final_response.status(), StatusCode::OK);
    let final_body = response_json(final_response).await;
    assert_eq!(
        final_body["data"]["metal"].as_i64().unwrap(),
        initial_body["data"]["metal"].as_i64().unwrap() - 4_000
    );
    assert_eq!(
        final_body["data"]["crystal"].as_i64().unwrap(),
        initial_body["data"]["crystal"].as_i64().unwrap() - 4_000
    );
}

#[tokio::test]
async fn shipyard_build_rejects_non_positive_quantity() {
    let app = build_router(TEST_SERVICE_NAME);
    let payload = json!({
        "planetId": "p-001",
        "shipType": "small_cargo",
        "quantity": 0
    });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/shipyard/build")
                .method("POST")
                .header("authorization", format!("Bearer {DEV_TOKEN}"))
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response_json(response).await;
    assert_eq!(body["success"], false);
    assert_eq!(body["error"], "Quantity must be greater than zero");
}

#[tokio::test]
async fn planet_build_queues_and_increments_level_target() {
    let app = build_router(TEST_SERVICE_NAME);
    let payload = json!({
        "buildingType": "metal_mine"
    });

    let first_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/planets/p-001/build")
                .method("POST")
                .header("authorization", format!("Bearer {DEV_TOKEN}"))
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first_response.status(), StatusCode::OK);
    let first_body = response_json(first_response).await;
    assert_eq!(first_body["data"]["levelTarget"], 1);

    let second_response = app
        .oneshot(
            Request::builder()
                .uri("/api/planets/p-001/build")
                .method("POST")
                .header("authorization", format!("Bearer {DEV_TOKEN}"))
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second_response.status(), StatusCode::OK);
    let second_body = response_json(second_response).await;
    assert_eq!(second_body["data"]["levelTarget"], 2);
}

#[tokio::test]
async fn planet_build_requires_building_type() {
    let app = build_router(TEST_SERVICE_NAME);
    let payload = json!({
        "buildingType": " "
    });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/planets/p-001/build")
                .method("POST")
                .header("authorization", format!("Bearer {DEV_TOKEN}"))
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response_json(response).await;
    assert_eq!(body["success"], false);
    assert_eq!(body["error"], "Building type is required");
}

#[tokio::test]
async fn debris_routes_require_authentication() {
    let app = build_router(TEST_SERVICE_NAME);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/debris")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn debris_location_with_auth_returns_scoped_field() {
    let app = build_router(TEST_SERVICE_NAME);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/debris/location/2/222/9")
                .method("GET")
                .header("authorization", format!("Bearer {DEV_TOKEN}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["success"], true);
    assert_eq!(body["data"][0]["galaxy"], 2);
    assert_eq!(body["data"][0]["system"], 222);
    assert_eq!(body["data"][0]["position"], 9);
}

#[tokio::test]
async fn moon_jump_gate_rejects_invalid_payload() {
    let app = build_router(TEST_SERVICE_NAME);
    let payload = json!({
        "toMoonId": 0,
        "fleetIds": []
    });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/moons/101/jump-gate")
                .method("POST")
                .header("authorization", format!("Bearer {DEV_TOKEN}"))
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response_json(response).await;
    assert_eq!(body["success"], false);
    assert_eq!(body["error"], "Invalid request");
}

#[tokio::test]
async fn universe_routes_with_auth_return_expected_contracts() {
    let app = build_router(TEST_SERVICE_NAME);

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/universe")
                .method("GET")
                .header("authorization", format!("Bearer {DEV_TOKEN}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);
    let list_body = response_json(list_response).await;
    assert_eq!(list_body["success"], true);
    assert!(list_body["data"].is_array());

    let stats_response = app
        .oneshot(
            Request::builder()
                .uri("/api/universe/7/stats")
                .method("GET")
                .header("authorization", format!("Bearer {DEV_TOKEN}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(stats_response.status(), StatusCode::OK);
    let stats_body = response_json(stats_response).await;
    assert_eq!(stats_body["data"]["universeId"], 7);
}

#[tokio::test]
async fn player_blocks_create_list_delete_flow_is_stateful() {
    let app = build_router(TEST_SERVICE_NAME);
    let create_payload = json!({
        "blockedUserId": 9001,
        "username": "RaidBoss",
        "scope": "chat",
        "reason": "spam"
    });

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/player-blocks")
                .method("POST")
                .header("authorization", format!("Bearer {DEV_TOKEN}"))
                .header("content-type", "application/json")
                .body(Body::from(create_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::OK);
    let create_body = response_json(create_response).await;
    assert_eq!(create_body["data"]["blockedUserId"], 9001);

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/player-blocks")
                .method("GET")
                .header("authorization", format!("Bearer {DEV_TOKEN}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);
    let list_body = response_json(list_response).await;
    assert_eq!(list_body["data"][0]["blockedUserId"], 9001);
    assert_eq!(list_body["data"][0]["scope"], "chat");

    let delete_response = app
        .oneshot(
            Request::builder()
                .uri("/api/player-blocks/9001")
                .method("DELETE")
                .header("authorization", format!("Bearer {DEV_TOKEN}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_response.status(), StatusCode::OK);
    let delete_body = response_json(delete_response).await;
    assert_eq!(delete_body["data"]["message"], "Player unblocked");
}

#[tokio::test]
async fn config_update_persists_and_adds_history_entry() {
    let app = build_router(TEST_SERVICE_NAME);
    let update_payload = json!({
        "value": 2,
        "reason": "test-adjustment"
    });

    let update_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/config/parameters/economy.resource_multiplier")
                .method("PUT")
                .header("authorization", format!("Bearer {DEV_TOKEN}"))
                .header("content-type", "application/json")
                .body(Body::from(update_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(update_response.status(), StatusCode::OK);
    let update_body = response_json(update_response).await;
    assert_eq!(update_body["data"]["value"], "2");

    let history_response = app
        .oneshot(
            Request::builder()
                .uri("/api/config/history?limit=1")
                .method("GET")
                .header("authorization", format!("Bearer {DEV_TOKEN}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(history_response.status(), StatusCode::OK);
    let history_body = response_json(history_response).await;
    assert_eq!(
        history_body["data"][0]["parameterKey"],
        "economy.resource_multiplier"
    );
    assert_eq!(history_body["data"][0]["newValue"], "2");
}

#[tokio::test]
async fn themes_public_and_user_preference_routes_work() {
    let app = build_router(TEST_SERVICE_NAME);

    let public_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/themes/current")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(public_response.status(), StatusCode::OK);
    let public_body = response_json(public_response).await;
    assert_eq!(public_body["success"], true);
    assert_eq!(public_body["data"]["theme"]["key"], "default");

    let update_payload = json!({
        "themeKey": "solstice",
        "reduceMotion": true
    });
    let update_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/themes/user/preferences")
                .method("PUT")
                .header("authorization", format!("Bearer {DEV_TOKEN}"))
                .header("content-type", "application/json")
                .body(Body::from(update_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(update_response.status(), StatusCode::OK);
    let update_body = response_json(update_response).await;
    assert_eq!(update_body["data"]["themeKey"], "solstice");
    assert_eq!(update_body["data"]["reduceMotion"], true);

    let get_response = app
        .oneshot(
            Request::builder()
                .uri("/api/themes/user/preferences")
                .method("GET")
                .header("authorization", format!("Bearer {DEV_TOKEN}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_response.status(), StatusCode::OK);
    let get_body = response_json(get_response).await;
    assert_eq!(get_body["data"]["themeKey"], "solstice");
}
