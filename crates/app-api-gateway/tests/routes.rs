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
async fn users_me_without_auth_returns_401() {
    let app = build_router(TEST_SERVICE_NAME);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/users/me")
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
async fn users_me_with_auth_returns_parity_friendly_shape() {
    let app = build_router(TEST_SERVICE_NAME);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/users/me")
                .method("GET")
                .header("authorization", format!("Bearer {DEV_TOKEN}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["id"], 1);
    assert_eq!(body["username"], "Commander");
    assert_eq!(body["data"]["id"], 1);
    assert_eq!(body["user"]["id"], 1);
    assert_eq!(body["research"]["energy_technology"], 12);
}

#[tokio::test]
async fn users_leaderboard_with_auth_returns_sorted_entries() {
    let app = build_router(TEST_SERVICE_NAME);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/users/leaderboard")
                .method("GET")
                .header("authorization", format!("Bearer {DEV_TOKEN}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert!(body.is_array());
    assert_eq!(body[0]["username"], "AdmiralNova");
    assert_eq!(body[0]["total_score"], 8_400_000);
    assert_eq!(body[0]["total_score_value"], 8_400_000);
    assert_eq!(body[1]["username"], "Commander");
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
async fn universe_parity_mutation_routes_return_success_with_auth() {
    let app = build_router(TEST_SERVICE_NAME);

    let create_payload = json!({
        "universeName": "Slice C",
        "speedMultiplier": 4
    });
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/universe/create")
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
    assert_eq!(create_body["success"], true);
    assert_eq!(create_body["data"]["created"], true);

    let seed_payload = json!({
        "generateBots": false
    });
    let seed_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/universe/9/seed")
                .method("POST")
                .header("authorization", format!("Bearer {DEV_TOKEN}"))
                .header("content-type", "application/json")
                .body(Body::from(seed_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(seed_response.status(), StatusCode::OK);
    let seed_body = response_json(seed_response).await;
    assert_eq!(seed_body["data"]["universeId"], 9);
    assert_eq!(seed_body["data"]["seeded"], true);
    assert_eq!(seed_body["data"]["generateBots"], false);

    let place_payload = json!({
        "playerId": 77,
        "customGalaxy": 2,
        "customSystem": 90
    });
    let place_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/universe/9/place-player")
                .method("POST")
                .header("authorization", format!("Bearer {DEV_TOKEN}"))
                .header("content-type", "application/json")
                .body(Body::from(place_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(place_response.status(), StatusCode::OK);
    let place_body = response_json(place_response).await;
    assert_eq!(place_body["data"]["placed"], true);
    assert_eq!(place_body["data"]["placement"]["galaxy"], 2);

    let maintenance_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/universe/9/maintenance/population-balance")
                .method("POST")
                .header("authorization", format!("Bearer {DEV_TOKEN}"))
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(maintenance_response.status(), StatusCode::OK);
    let maintenance_body = response_json(maintenance_response).await;
    assert_eq!(maintenance_body["data"]["operation"], "population-balance");
    assert_eq!(maintenance_body["data"]["balanced"], true);

    let patch_cases = [
        ("/api/universe/9/registration", "registration"),
        ("/api/universe/9/lifecycle", "lifecycle"),
        ("/api/universe/9/speed", "speed"),
        ("/api/universe/9/merge", "merge"),
        ("/api/universe/9/end-event", "end-event"),
        ("/api/universe/9/announcement", "announcement"),
    ];

    for (uri, expected_updated) in patch_cases {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .method("PATCH")
                    .header("authorization", format!("Bearer {DEV_TOKEN}"))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"isActive":true}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["success"], true);
        assert_eq!(body["data"]["updated"], expected_updated);
        assert_eq!(body["data"]["universeId"], 9);
    }
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

#[tokio::test]
async fn shards_routes_require_authentication() {
    let app = build_router(TEST_SERVICE_NAME);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/shards/servers")
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
async fn shards_register_list_health_and_routing_stats_are_stateful() {
    let app = build_router(TEST_SERVICE_NAME);

    let register_1 = json!({
        "serverId": "eu-west-1",
        "serverType": "game",
        "region": "eu-west",
        "endpoint": "http://eu-west-1.internal",
        "status": "online",
        "currentLoad": 240,
        "maxCapacity": 1000,
        "healthScore": 0.92
    });
    let register_1_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/shards/servers/register")
                .method("POST")
                .header("authorization", format!("Bearer {DEV_TOKEN}"))
                .header("content-type", "application/json")
                .body(Body::from(register_1.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(register_1_response.status(), StatusCode::OK);
    let register_1_body = response_json(register_1_response).await;
    assert_eq!(register_1_body["success"], true);
    assert_eq!(register_1_body["data"]["serverId"], "eu-west-1");

    let register_1_update = json!({
        "serverId": "eu-west-1",
        "serverType": "game",
        "region": "eu-west",
        "endpoint": "http://eu-west-1.internal",
        "status": "online",
        "currentLoad": 280,
        "maxCapacity": 1000,
        "healthScore": 0.95
    });
    let register_1_update_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/shards/servers/register")
                .method("POST")
                .header("authorization", format!("Bearer {DEV_TOKEN}"))
                .header("content-type", "application/json")
                .body(Body::from(register_1_update.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(register_1_update_response.status(), StatusCode::OK);

    let register_2 = json!({
        "serverId": "us-east-1",
        "serverType": "game",
        "region": "us-east",
        "endpoint": "http://us-east-1.internal",
        "status": "online",
        "currentLoad": 700,
        "maxCapacity": 1000,
        "healthScore": 0.81
    });
    let register_2_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/shards/servers/register")
                .method("POST")
                .header("authorization", format!("Bearer {DEV_TOKEN}"))
                .header("content-type", "application/json")
                .body(Body::from(register_2.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(register_2_response.status(), StatusCode::OK);

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/shards/servers")
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
    assert_eq!(list_body["data"].as_array().unwrap().len(), 2);

    let health_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/shards/servers/eu-west-1/health")
                .method("GET")
                .header("authorization", format!("Bearer {DEV_TOKEN}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(health_response.status(), StatusCode::OK);
    let health_body = response_json(health_response).await;
    assert_eq!(health_body["success"], true);
    assert_eq!(health_body["data"]["serverId"], "eu-west-1");
    assert_eq!(health_body["data"]["currentLoad"], 280);
    assert_eq!(health_body["data"]["loadPercent"], 28.0);

    let stats_response = app
        .oneshot(
            Request::builder()
                .uri("/api/shards/routing/stats")
                .method("GET")
                .header("authorization", format!("Bearer {DEV_TOKEN}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(stats_response.status(), StatusCode::OK);
    let stats_body = response_json(stats_response).await;
    assert_eq!(stats_body["success"], true);
    assert_eq!(stats_body["data"]["totalServers"], 2);
    assert_eq!(stats_body["data"]["healthyServers"], 2);
    assert_eq!(stats_body["data"]["overloadedServers"], 0);
    assert_eq!(stats_body["data"]["migrationCount"], 1);
    assert_eq!(stats_body["data"]["totalCapacity"], 2000);
    assert_eq!(stats_body["data"]["totalLoad"], 980);
    assert_eq!(stats_body["data"]["averageLoadPercent"], 49.0);
}

#[tokio::test]
async fn acs_routes_require_authentication() {
    let app = build_router(TEST_SERVICE_NAME);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/acs")
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
async fn acs_create_join_and_leave_return_success_envelopes() {
    let app = build_router(TEST_SERVICE_NAME);

    let create_payload = json!({
        "missionType": "attack",
        "targetGalaxy": 2,
        "targetSystem": 155,
        "targetPosition": 8
    });
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/acs")
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
    assert_eq!(create_body["success"], true);
    assert_eq!(create_body["data"]["targetGalaxy"], 2);

    let join_payload = json!({ "planetId": 1 });
    let join_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/acs/101/join")
                .method("POST")
                .header("authorization", format!("Bearer {DEV_TOKEN}"))
                .header("content-type", "application/json")
                .body(Body::from(join_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(join_response.status(), StatusCode::OK);
    let join_body = response_json(join_response).await;
    assert_eq!(join_body["success"], true);
    assert_eq!(join_body["data"]["joined"], true);

    let leave_response = app
        .oneshot(
            Request::builder()
                .uri("/api/acs/101/leave")
                .method("DELETE")
                .header("authorization", format!("Bearer {DEV_TOKEN}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(leave_response.status(), StatusCode::OK);
    let leave_body = response_json(leave_response).await;
    assert_eq!(leave_body["success"], true);
    assert_eq!(leave_body["data"]["left"], true);
}

#[tokio::test]
async fn rips_destroy_moon_validates_and_returns_success_envelope() {
    let app = build_router(TEST_SERVICE_NAME);

    let invalid_payload = json!({
        "sourceMoonId": 101,
        "targetMoonId": 202,
        "numDeathstars": 0
    });
    let invalid_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/rips/destroyMoon")
                .method("POST")
                .header("authorization", format!("Bearer {DEV_TOKEN}"))
                .header("content-type", "application/json")
                .body(Body::from(invalid_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid_response.status(), StatusCode::BAD_REQUEST);
    let invalid_body = response_json(invalid_response).await;
    assert_eq!(invalid_body["success"], false);
    assert_eq!(invalid_body["error"], "Invalid destroy moon request");

    let valid_payload = json!({
        "sourceMoonId": 101,
        "targetMoonId": 202,
        "numDeathstars": 5,
        "speedPercent": 90
    });
    let valid_response = app
        .oneshot(
            Request::builder()
                .uri("/api/rips/destroyMoon")
                .method("POST")
                .header("authorization", format!("Bearer {DEV_TOKEN}"))
                .header("content-type", "application/json")
                .body(Body::from(valid_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(valid_response.status(), StatusCode::OK);
    let valid_body = response_json(valid_response).await;
    assert_eq!(valid_body["success"], true);
    assert_eq!(valid_body["data"]["accepted"], true);
    assert_eq!(valid_body["data"]["numDeathstars"], 5);
}
