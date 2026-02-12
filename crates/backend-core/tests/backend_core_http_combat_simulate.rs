#[path = "../src/bin/backend-core-http.rs"]
mod backend_core_http;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::{json, Value};
use tower::ServiceExt;

fn simulate_payload() -> Value {
    json!({
        "battle_id": "battle-1",
        "attacker_ships": { "fighter": 8 },
        "defender_ships": { "fighter": 6 },
        "defender_defenses": { "rocket_launcher": 2 },
        "attacker_tech": { "weapons_technology": 2 },
        "defender_tech": { "shielding_technology": 1 },
        "planet_metal": 10000,
        "planet_crystal": 5000,
        "planet_deuterium": 2000,
        "seed": "fixed-seed",
        "universe": "default",
        "max_rounds": 2
    })
}

#[tokio::test]
async fn combat_simulate_requires_helper_token() {
    let app = backend_core_http::build_app(Some("top-secret".to_string()));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/combat/simulate")
                .header("content-type", "application/json")
                .body(Body::from(simulate_payload().to_string()))
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
async fn combat_simulate_returns_success_envelope_with_camel_case_data() {
    let app = backend_core_http::build_app(Some("top-secret".to_string()));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/combat/simulate")
                .header("content-type", "application/json")
                .header("x-core-helper-token", "top-secret")
                .body(Body::from(simulate_payload().to_string()))
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

    let data = json
        .get("data")
        .and_then(Value::as_object)
        .expect("data object");
    assert!(data.contains_key("winner"));
    assert!(data.contains_key("rounds"));
    assert!(data.contains_key("attackerLosses"));
    assert!(data.contains_key("defenderLosses"));
    assert!(data.contains_key("loot"));
    assert!(data.contains_key("debris"));
    assert!(!data.contains_key("attacker_losses"));
    assert!(!data.contains_key("defender_losses"));

    let rounds = data
        .get("rounds")
        .and_then(Value::as_array)
        .expect("rounds array");
    assert!(!rounds.is_empty(), "expected at least one round");
    let first_round = rounds[0].as_object().expect("first round object");
    assert!(first_round.contains_key("attackerShots"));
    assert!(first_round.contains_key("defenderShots"));
    assert!(first_round.contains_key("attackerDestroyed"));
    assert!(first_round.contains_key("defenderDestroyed"));
}
