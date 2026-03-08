use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use hyper::body::to_bytes;
use serde_json::{json, Value};
use tower::ServiceExt;

use app_api_gateway::routes::build_router;

async fn json_body(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body())
        .await
        .expect("failed to read response body");
    serde_json::from_slice(&bytes).expect("response is valid json")
}

fn json_request_with_body(path: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("build request")
}

fn dev_token() -> String {
    let config = platform_auth::AuthConfig {
        jwt_secret: "default-secret".to_string(),
        jwt_expiry_seconds: 86_400,
        ..platform_auth::AuthConfig::default()
    };
    platform_auth::generate_token(&config, "u-rust-1", "Commander", "player", Some(1)).unwrap()
}

#[tokio::test]
async fn simulated_player_flow_happy_path() {
    let app = build_router("simulation-flow");

    let health = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(health.status(), StatusCode::OK);
    let health_body = json_body(health).await;
    assert_eq!(health_body["service"], "simulation-flow");

    let planets = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/planets")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(planets.status(), StatusCode::OK);
    let planets_body = json_body(planets).await;
    let planets_array = planets_body["data"]
        .as_array()
        .expect("returned planets array");
    assert_eq!(planets_array.len(), 2);

    let fleet = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/fleet")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(fleet.status(), StatusCode::OK);
    let fleet_body = json_body(fleet).await;
    let fleet_array = fleet_body["data"].as_array().expect("fleet data array");
    assert!(!fleet_array.is_empty());

    let unauthorized_build = app
        .clone()
        .oneshot(json_request_with_body(
            "/api/planets/p-001/build",
            json!({ "buildingType": "metal_mine" }),
        ))
        .await
        .unwrap();
    assert_eq!(unauthorized_build.status(), StatusCode::UNAUTHORIZED);

    let build_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/planets/p-001/build")
                .header(header::AUTHORIZATION, format!("Bearer {}", dev_token()))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "buildingType": "metal_mine" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(build_response.status(), StatusCode::OK);
    let build_body = json_body(build_response).await;
    assert!(build_body["success"].as_bool().unwrap());
    assert_eq!(build_body["data"]["planetId"], "p-001");

    let movement_payload = json!({
        "origin": { "galaxy": 1, "system": 1, "position": 1 },
        "target": { "galaxy": 1, "system": 2, "position": 1 },
        "ships": { "lightFighter": 5 }
    });

    let movement_response = app
        .clone()
        .oneshot(json_request_with_body(
            "/api/fleet/helpers/movement",
            movement_payload,
        ))
        .await
        .unwrap();
    assert_eq!(movement_response.status(), StatusCode::OK);
    let movement_body = json_body(movement_response).await;
    assert_eq!(movement_body["data"]["distance"], 2795);

    let send_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/fleet/send")
                .header(header::AUTHORIZATION, format!("Bearer {}", dev_token()))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "mission": "deploy",
                        "target": "[1:121:4]",
                        "ships": [
                            { "shipType": "lightFighter", "count": 3 }
                        ]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(send_response.status(), StatusCode::OK);
    let send_body = json_body(send_response).await;
    let command = send_body["data"]["commandId"]
        .as_str()
        .expect("command id present");
    assert!(command.starts_with("cmd-fleet-"));
    assert_eq!(send_body["data"]["accepted"], true);
}
