use axum::extract::Path;
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use game_combat::{simulate_combat, CombatInput, CombatResult};
use game_fleet::{calculate_movement, FleetMovementInput, FleetMovementResult};
use serde::{Deserialize, Serialize};

use crate::auth_guard::BearerToken;
use crate::helpers::{
    attacker_distribution_handler, defense_rebuild_handler, espionage_outcome_handler,
    harvest_collection_handler, helper_movement_handler, mission_cargo_transfer_handler,
};
use crate::response::{bad_request, success};
use crate::state::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FleetSummary {
    fleet_id: String,
    mission: &'static str,
    ships: i64,
    origin: &'static str,
    destination: &'static str,
    eta_seconds: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FleetDetail {
    fleet_id: String,
    mission: &'static str,
    status: &'static str,
    ships: Vec<FleetShip>,
    eta_seconds: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FleetShip {
    ship_type: &'static str,
    count: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FleetSendRequest {
    mission: String,
    target: String,
    ships: Vec<FleetSendShip>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FleetSendShip {
    ship_type: String,
    count: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FleetSendResponse {
    command_id: String,
    mission: String,
    target: String,
    total_ships: i64,
    accepted: bool,
}

pub fn router() -> Router {
    Router::new()
        .route("/api/combat/simulate", post(simulate_combat_handler))
        .route("/api/fleet/movement", post(fleet_movement_handler))
        .route("/api/fleet/move", post(fleet_move_handler))
        .route("/api/fleet", get(list_fleets_handler))
        .route("/api/fleet/:fleet_id", get(get_fleet_handler))
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

pub fn protected_router() -> Router {
    Router::new().route("/api/fleet/send", post(send_fleet_handler))
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

async fn list_fleets_handler() -> Response {
    success(vec![
        FleetSummary {
            fleet_id: "f-1001".to_string(),
            mission: "attack",
            ships: 32,
            origin: "[1:120:8]",
            destination: "[1:121:4]",
            eta_seconds: 3580,
        },
        FleetSummary {
            fleet_id: "f-1002".to_string(),
            mission: "transport",
            ships: 18,
            origin: "[1:121:4]",
            destination: "[1:120:8]",
            eta_seconds: 2440,
        },
    ])
}

async fn get_fleet_handler(Path(fleet_id): Path<String>) -> Response {
    let detail = match fleet_id.as_str() {
        "f-1001" => FleetDetail {
            fleet_id,
            mission: "attack",
            status: "en_route",
            ships: vec![
                FleetShip {
                    ship_type: "lightFighter",
                    count: 20,
                },
                FleetShip {
                    ship_type: "cruiser",
                    count: 12,
                },
            ],
            eta_seconds: 3580,
        },
        "f-1002" => FleetDetail {
            fleet_id,
            mission: "transport",
            status: "returning",
            ships: vec![
                FleetShip {
                    ship_type: "smallCargo",
                    count: 10,
                },
                FleetShip {
                    ship_type: "largeCargo",
                    count: 8,
                },
            ],
            eta_seconds: 2440,
        },
        _ => return bad_request("Fleet not found"),
    };
    success(detail)
}

async fn send_fleet_handler(
    BearerToken(token): BearerToken,
    Extension(app_state): Extension<AppState>,
    Json(input): Json<FleetSendRequest>,
) -> Response {
    if input.mission.trim().is_empty() {
        return bad_request("Mission is required");
    }

    if input.target.trim().is_empty() {
        return bad_request("Target is required");
    }

    if input.ships.is_empty() {
        return bad_request("At least one ship entry is required");
    }

    let mut total_ships = 0_i64;
    for ship in &input.ships {
        if ship.ship_type.trim().is_empty() {
            return bad_request("Ship type is required");
        }
        if ship.count <= 0 {
            return bad_request("Ship count must be greater than zero");
        }
        total_ships += ship.count;
    }

    let mission_record = app_state.enqueue_fleet_mission(
        &token,
        input.mission.clone(),
        input.target.clone(),
        total_ships,
    );

    success(FleetSendResponse {
        command_id: mission_record.command_id,
        mission: input.mission,
        target: input.target,
        total_ships,
        accepted: true,
    })
}
