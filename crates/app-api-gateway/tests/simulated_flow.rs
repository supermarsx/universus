use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use hyper::body::to_bytes;
use serde_json::{json, Value};
use tower::ServiceExt;

use app_api_gateway::accounts::AccountRepository;
use app_api_gateway::routes::build_router_with_dependencies;

fn build_router(service_name: &'static str) -> axum::Router {
    build_router_with_dependencies(service_name, None, AccountRepository::in_memory())
}

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
async fn simulated_player_flow_respects_durable_gameplay_boundary() {
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
                .header(header::AUTHORIZATION, format!("Bearer {}", dev_token()))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(planets.status(), StatusCode::SERVICE_UNAVAILABLE);
    let planets_body = json_body(planets).await;
    assert_eq!(planets_body["success"], false);
    assert_eq!(planets_body["error"], "Gameplay repository is unavailable");

    let fleet = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/fleet")
                .header(header::AUTHORIZATION, format!("Bearer {}", dev_token()))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(fleet.status(), StatusCode::SERVICE_UNAVAILABLE);
    let fleet_body = json_body(fleet).await;
    assert_eq!(fleet_body["success"], false);
    assert_eq!(fleet_body["error"], "Fleet repository is unavailable");

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
    assert_eq!(build_response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let build_body = json_body(build_response).await;
    assert_eq!(build_body["success"], false);
    assert_eq!(build_body["error"], "Gameplay repository is unavailable");

    let movement_payload = json!({
        "origin": { "galaxy": 1, "system": 1, "position": 1 },
        "target": { "galaxy": 1, "system": 2, "position": 1 },
        "ships": { "lightFighter": 5 }
    });

    let movement_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/fleet/helpers/movement")
                .header(header::AUTHORIZATION, format!("Bearer {}", dev_token()))
                .header("content-type", "application/json")
                .body(Body::from(movement_payload.to_string()))
                .unwrap(),
        )
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
                        "commandId": "simulated-flow-fleet-0001",
                        "mission": "deploy",
                        "sourceKind": "planet",
                        "originPlanetId": "1",
                        "targetKind": "planet",
                        "targetGalaxy": 1,
                        "targetSystem": 121,
                        "targetPosition": 4,
                        "ships": [
                            { "shipType": "lightFighter", "count": 3 }
                        ],
                        "cargo": { "metal": 0, "crystal": 0, "deuterium": 0 },
                        "speedPercent": 100,
                        "holdSeconds": 0
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(send_response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let send_body = json_body(send_response).await;
    assert_eq!(send_body["success"], false);
    assert_eq!(send_body["error"], "Fleet repository is unavailable");
}
