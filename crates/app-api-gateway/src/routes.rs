use axum::routing::{get, post};
use axum::{Json, Router};
use game_combat::{simulate_combat, CombatInput, CombatResult};
use game_fleet::{calculate_movement, FleetMovementInput, FleetMovementResult};
use serde::Serialize;

use crate::helpers::{
    attacker_distribution_handler, defense_rebuild_handler, espionage_outcome_handler,
    harvest_collection_handler, helper_movement_handler, mission_cargo_transfer_handler,
};

#[derive(Serialize)]
struct ServiceStatus {
    status: &'static str,
    service: &'static str,
}

pub fn build_router(service_name: &'static str) -> Router {
    Router::new()
        .route("/health", get(move || health(service_name)))
        .route("/ready", get(move || ready(service_name)))
        .route("/api/combat/simulate", post(simulate_combat_handler))
        .route("/api/fleet/movement", post(fleet_movement_handler))
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
