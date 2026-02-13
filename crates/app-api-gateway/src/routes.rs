use axum::routing::{get, post};
use axum::{Json, Router};
use axum::extract::Path;
use axum::extract::rejection::JsonRejection;
use axum::response::Response;
use game_combat::{simulate_combat, CombatInput, CombatResult};
use game_fleet::{calculate_movement, FleetMovementInput, FleetMovementResult};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::helpers::{
    attacker_distribution_handler, defense_rebuild_handler, espionage_outcome_handler,
    harvest_collection_handler, helper_movement_handler, mission_cargo_transfer_handler,
};
use crate::response::{bad_request, success};

#[derive(Serialize)]
struct ServiceStatus {
    status: &'static str,
    service: &'static str,
}

pub fn build_router(service_name: &'static str) -> Router {
    Router::new()
        .route("/health", get(move || health(service_name)))
        .route("/ready", get(move || ready(service_name)))
        .route("/api/auth/login", post(login_handler))
        .route("/api/auth/register", post(register_handler))
        .route("/api/planets", get(list_planets_handler))
        .route("/api/planets/:planet_id", get(get_planet_handler))
        .route("/api/combat/simulate", post(simulate_combat_handler))
        .route("/api/fleet/movement", post(fleet_movement_handler))
        .route("/api/fleet/move", post(fleet_move_handler))
        .route("/api/fleet/helpers/movement", post(helper_movement_handler))
        .route(
            "/api/fleet/helpers/combat/defense-rebuild",
            post(defense_rebuild_handler),
        )
        .route(
            "/api/fleet/helpers/combat/attacker-distribution",
            post(attacker_distribution_handler),
        )
        .route(
            "/api/fleet/helpers/espionage-outcome",
            post(espionage_outcome_handler),
        )
        .route(
            "/api/fleet/helpers/mission-cargo-transfer",
            post(mission_cargo_transfer_handler),
        )
        .route(
            "/api/fleet/helpers/harvest-collection",
            post(harvest_collection_handler),
        )
}

async fn health(service_name: &'static str) -> Json<ServiceStatus> {
    Json(ServiceStatus {
        status: "ok",
        service: service_name,
    })
}

async fn ready(service_name: &'static str) -> Json<ServiceStatus> {
    Json(ServiceStatus {
        status: "ok",
        service: service_name,
    })
}

async fn simulate_combat_handler(Json(input): Json<CombatInput>) -> Json<CombatResult> {
    Json(simulate_combat(&input))
}

async fn fleet_movement_handler(
    Json(input): Json<FleetMovementInput>,
) -> Json<FleetMovementResult> {
    Json(calculate_movement(&input))
}

async fn fleet_move_handler(Json(input): Json<FleetMovementInput>) -> Json<FleetMovementResult> {
    Json(calculate_movement(&input))
}

#[derive(Debug, Deserialize)]
struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Debug, Deserialize)]
struct RegisterRequest {
    username: String,
    email: String,
    password: String,
}

#[derive(Debug, Serialize)]
struct AuthUser {
    id: String,
    username: String,
    email: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthPayload {
    token: String,
    user: AuthUser,
    expires_in_seconds: i64,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PlanetPayload {
    id: String,
    name: String,
    galaxy: i32,
    system: i32,
    position: i32,
    metal: i64,
    crystal: i64,
    deuterium: i64,
}

async fn login_handler(payload: Result<Json<LoginRequest>, JsonRejection>) -> Response {
    let Json(input) = match payload {
        Ok(value) => value,
        Err(_) => return bad_request("Invalid login payload"),
    };

    if input.email.trim().is_empty() || input.password.trim().is_empty() {
        return bad_request("Email and password are required");
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    success(AuthPayload {
        token: format!("rust-gateway-token-{}", now),
        user: AuthUser {
            id: "u-rust-1".to_string(),
            username: "Commander".to_string(),
            email: input.email,
        },
        expires_in_seconds: 7 * 24 * 3600,
    })
}

async fn register_handler(payload: Result<Json<RegisterRequest>, JsonRejection>) -> Response {
    let Json(input) = match payload {
        Ok(value) => value,
        Err(_) => return bad_request("Invalid register payload"),
    };

    if input.email.trim().is_empty()
        || input.password.trim().is_empty()
        || input.username.trim().is_empty()
    {
        return bad_request("Username, email and password are required");
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    success(AuthPayload {
        token: format!("rust-gateway-token-{}", now),
        user: AuthUser {
            id: "u-rust-new".to_string(),
            username: input.username,
            email: input.email,
        },
        expires_in_seconds: 7 * 24 * 3600,
    })
}

async fn list_planets_handler() -> Response {
    success(default_planets())
}

async fn get_planet_handler(Path(planet_id): Path<String>) -> Response {
    let planets = default_planets();
    if let Some(planet) = planets.into_iter().find(|planet| planet.id == planet_id) {
        success(planet)
    } else {
        bad_request("Planet not found")
    }
}

fn default_planets() -> Vec<PlanetPayload> {
    vec![
        PlanetPayload {
            id: "p-001".to_string(),
            name: "New Terra".to_string(),
            galaxy: 1,
            system: 120,
            position: 8,
            metal: 12_000,
            crystal: 8_500,
            deuterium: 2_300,
        },
        PlanetPayload {
            id: "p-002".to_string(),
            name: "Helios".to_string(),
            galaxy: 1,
            system: 121,
            position: 4,
            metal: 9_400,
            crystal: 7_100,
            deuterium: 1_800,
        },
    ]
}
