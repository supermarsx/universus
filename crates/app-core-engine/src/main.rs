use std::net::SocketAddr;

use axum::routing::{get, post};
use axum::{Json, Router};
use game_combat::{simulate_combat, CombatInput, CombatResult};
use game_fleet::{calculate_movement, FleetMovementInput, FleetMovementResult};
use serde::{Deserialize, Serialize};

const SERVICE_NAME: &str = "app-core-engine";
const DEFAULT_PORT: u16 = 3007;

#[derive(Debug, Serialize)]
struct Envelope<T> {
    status: &'static str,
    data: T,
}

#[derive(Debug, Serialize)]
struct ServiceStatus {
    status: &'static str,
    service: &'static str,
}

#[derive(Debug, Deserialize)]
struct CombatSimulateRequest {
    input: CombatInput,
}

#[derive(Debug, Deserialize)]
struct FleetMovementRequest {
    input: FleetMovementInput,
}

fn app_router() -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/engine/combat/simulate", post(combat_simulate))
        .route("/engine/fleet/movement", post(fleet_movement))
}

fn listen_port(default_port: u16) -> u16 {
    std::env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(default_port)
}

async fn health() -> Json<ServiceStatus> {
    Json(ServiceStatus {
        status: "ok",
        service: SERVICE_NAME,
    })
}

async fn ready() -> Json<ServiceStatus> {
    Json(ServiceStatus {
        status: "ok",
        service: SERVICE_NAME,
    })
}

async fn combat_simulate(
    Json(request): Json<CombatSimulateRequest>,
) -> Json<Envelope<CombatResult>> {
    Json(Envelope {
        status: "ok",
        data: simulate_combat(&request.input),
    })
}

async fn fleet_movement(
    Json(request): Json<FleetMovementRequest>,
) -> Json<Envelope<FleetMovementResult>> {
    Json(Envelope {
        status: "ok",
        data: calculate_movement(&request.input),
    })
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let app = app_router();
    let addr = SocketAddr::from(([0, 0, 0, 0], listen_port(DEFAULT_PORT)));
    tracing::info!(service = SERVICE_NAME, %addr, "startup");

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .expect("server failed");
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode};
    use hyper::body::to_bytes;
    use serde_json::{json, Value};
    use tower::ServiceExt;

    use super::app_router;

    async fn json_body(response: axum::response::Response) -> Value {
        let bytes = to_bytes(response.into_body()).await.expect("read body");
        serde_json::from_slice(&bytes).expect("parse json")
    }

    fn combat_payload(seed: &str) -> Value {
        json!({
            "input": {
                "attacker_ships": {
                    "fighter": 100,
                    "bomber": 10
                },
                "defender_ships": {
                    "defender": 50
                },
                "defender_defenses": {
                    "turret": 5
                },
                "attacker_tech": {
                    "weapons_technology": 2
                },
                "defender_tech": {
                    "shielding_technology": 1
                },
                "planet_metal": 10000,
                "planet_crystal": 5000,
                "planet_deuterium": 1000,
                "seed": seed,
                "universe": "default",
                "max_rounds": 4
            }
        })
    }

    fn movement_payload() -> Value {
        json!({
            "input": {
                "origin_galaxy": 1,
                "origin_system": 1,
                "origin_position": 1,
                "target_galaxy": 1,
                "target_system": 2,
                "target_position": 1,
                "ships": [
                    {
                        "count": 10,
                        "base_speed": 1000.0,
                        "fuel_consumption": 2.0,
                        "cargo": 50.0
                    },
                    {
                        "count": 1,
                        "base_speed": 500.0,
                        "fuel_consumption": 5.0,
                        "cargo": 100.0
                    }
                ]
            }
        })
    }

    #[tokio::test]
    async fn health_endpoint_returns_expected_contract() {
        let app = app_router();

        let response = app
            .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert_eq!(body["status"], "ok");
        assert_eq!(body["service"], "app-core-engine");
    }

    #[tokio::test]
    async fn combat_endpoint_returns_deterministic_envelope_and_payload() {
        let app = app_router();
        let payload = combat_payload("seed-combat-1");

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/engine/combat/simulate")
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert_eq!(body["status"], "ok");
        assert!(body["data"]["winner"].is_string());
        assert!(body["data"]["rounds"].is_array());
        assert!(body["data"]["loot"].is_object());
    }

    #[tokio::test]
    async fn fleet_movement_endpoint_returns_deterministic_envelope_and_payload() {
        let app = app_router();
        let payload = movement_payload();

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/engine/fleet/movement")
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert_eq!(body["status"], "ok");
        assert_eq!(body["data"]["distance"], 2795);
        assert_eq!(body["data"]["fleet_speed"], 500.0);
        assert_eq!(body["data"]["travel_time_seconds"], 20124);
    }

    #[tokio::test]
    async fn combat_endpoint_is_deterministic_for_same_input() {
        let app = app_router();
        let payload = combat_payload("same-seed");

        let first = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/engine/combat/simulate")
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let first_body = json_body(first).await;

        let second = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/engine/combat/simulate")
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let second_body = json_body(second).await;

        assert_eq!(first_body, second_body);
    }
}
