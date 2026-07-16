use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::Path;
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use game_combat::{simulate_combat, CombatInput, CombatResult};
use game_fleet::{calculate_movement, FleetMovementInput, FleetMovementResult};
use platform_db::{
    Database, FleetLaunchInput, FleetMissionEventRow, FleetMissionRow, FleetSourceKind,
    FleetWriteError,
};
use serde::Deserialize;

use crate::auth_guard::AuthUser;
use crate::helpers::{
    attacker_distribution_handler, defense_rebuild_handler, espionage_outcome_handler,
    harvest_collection_handler, helper_movement_handler, mission_cargo_transfer_handler,
};
use crate::response::{bad_request, conflict, not_found, service_unavailable, success};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FleetSendRequest {
    command_id: String,
    mission: String,
    #[serde(default = "default_source_kind")]
    source_kind: String,
    origin_planet_id: String,
    #[serde(default)]
    origin_moon_id: Option<String>,
    target_kind: String,
    target_galaxy: i32,
    target_system: i32,
    target_position: i32,
    ships: Vec<FleetSendShip>,
    #[serde(default)]
    cargo: FleetCargoRequest,
    #[serde(default = "default_speed_percent")]
    speed_percent: i32,
    #[serde(default)]
    hold_seconds: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FleetSendShip {
    ship_type: String,
    count: i64,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FleetCargoRequest {
    #[serde(default)]
    metal: i64,
    #[serde(default)]
    crystal: i64,
    #[serde(default)]
    deuterium: i64,
}

fn default_source_kind() -> String {
    "planet".to_string()
}

fn default_speed_percent() -> i32 {
    100
}

pub fn router() -> Router {
    Router::new()
        .route("/api/combat/simulate", post(simulate_combat_handler))
        .route("/api/fleet/movement", post(fleet_movement_handler))
        .route("/api/fleet/move", post(fleet_move_handler))
        .route("/api/fleet", get(list_fleets_handler))
        .route("/api/fleet/:fleet_id", get(get_fleet_handler))
        .route("/api/fleet/:fleet_id/events", get(get_fleet_events_handler))
        .route("/api/fleet/:fleet_id/recall", post(recall_fleet_handler))
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

async fn list_fleets_handler(
    AuthUser(user): AuthUser,
    Extension(database): Extension<Option<Database>>,
) -> Response {
    let Some(database) = database else {
        return repository_unavailable();
    };
    let Some(universe_id) = user.universe_id else {
        return bad_request("Fleet actions require a universe-scoped session");
    };
    match database
        .fleet_missions_for_user(&user.user_id, universe_id)
        .await
    {
        Ok(fleets) => {
            let now = unix_now();
            success(
                fleets
                    .into_iter()
                    .map(|fleet| fleet_payload(fleet, now, false))
                    .collect::<Vec<_>>(),
            )
        }
        Err(_) => repository_unavailable(),
    }
}

async fn get_fleet_handler(
    Path(fleet_id): Path<String>,
    AuthUser(user): AuthUser,
    Extension(database): Extension<Option<Database>>,
) -> Response {
    let Some(database) = database else {
        return repository_unavailable();
    };
    let Some(universe_id) = user.universe_id else {
        return bad_request("Fleet actions require a universe-scoped session");
    };
    match database
        .fleet_mission_for_user(&user.user_id, universe_id, &fleet_id)
        .await
    {
        Ok(Some(fleet)) => success(fleet_payload(fleet, unix_now(), true)),
        Ok(None) => not_found("Fleet not found"),
        Err(_) => repository_unavailable(),
    }
}

async fn get_fleet_events_handler(
    Path(fleet_id): Path<String>,
    AuthUser(user): AuthUser,
    Extension(database): Extension<Option<Database>>,
) -> Response {
    let Some(database) = database else {
        return repository_unavailable();
    };
    let Some(universe_id) = user.universe_id else {
        return bad_request("Fleet actions require a universe-scoped session");
    };
    match database
        .fleet_mission_events_for_user(&user.user_id, universe_id, &fleet_id)
        .await
    {
        Ok(events) => success(events.into_iter().map(event_payload).collect::<Vec<_>>()),
        Err(_) => repository_unavailable(),
    }
}

async fn recall_fleet_handler(
    Path(fleet_id): Path<String>,
    AuthUser(user): AuthUser,
    Extension(database): Extension<Option<Database>>,
) -> Response {
    let Some(database) = database else {
        return repository_unavailable();
    };
    let Some(universe_id) = user.universe_id else {
        return bad_request("Fleet actions require a universe-scoped session");
    };
    match database
        .recall_fleet(&user.user_id, universe_id, &fleet_id)
        .await
    {
        Ok(fleet) => success(fleet_payload(fleet, unix_now(), true)),
        Err(error) => fleet_error_response(error),
    }
}

async fn send_fleet_handler(
    AuthUser(user): AuthUser,
    Extension(database): Extension<Option<Database>>,
    Json(input): Json<FleetSendRequest>,
) -> Response {
    let Some(database) = database else {
        return repository_unavailable();
    };
    let Some(universe_id) = user.universe_id else {
        return bad_request("Fleet actions require a universe-scoped session");
    };
    let source_kind = match input.source_kind.trim().to_ascii_lowercase().as_str() {
        "planet" => FleetSourceKind::Planet,
        "moon" => FleetSourceKind::Moon,
        _ => return bad_request("Source kind must be planet or moon"),
    };
    let mut ships = BTreeMap::new();
    for ship in input.ships {
        let Some(ship_type) = canonical_ship_type(&ship.ship_type) else {
            return bad_request("Unsupported ship type");
        };
        if ship.count <= 0 || ships.insert(ship_type.to_string(), ship.count).is_some() {
            return bad_request("Fleet ship counts must be positive and unique by type");
        }
    }
    let launch = FleetLaunchInput {
        user_id: user.user_id,
        universe_id,
        command_id: input.command_id,
        mission_type: input.mission,
        source_kind,
        origin_planet_id: input.origin_planet_id,
        origin_moon_id: input.origin_moon_id,
        target_kind: input.target_kind,
        target_galaxy: input.target_galaxy,
        target_system: input.target_system,
        target_position: input.target_position,
        acs_group_id: None,
        ships,
        cargo_metal: input.cargo.metal,
        cargo_crystal: input.cargo.crystal,
        cargo_deuterium: input.cargo.deuterium,
        speed_percent: input.speed_percent,
        hold_seconds: input.hold_seconds,
    };
    match database.launch_fleet(launch).await {
        Ok(result) => success(serde_json::json!({
            "accepted": true,
            "idempotentReplay": result.idempotent_replay,
            "fleet": fleet_payload(result.mission, unix_now(), true)
        })),
        Err(error) => fleet_error_response(error),
    }
}

fn fleet_payload(fleet: FleetMissionRow, now: i64, detailed: bool) -> serde_json::Value {
    let total_ships = fleet.ships.values().copied().sum::<i64>();
    let ships = fleet
        .ships
        .iter()
        .map(|(ship_type, count)| serde_json::json!({"shipType": ship_type, "count": count}))
        .collect::<Vec<_>>();
    let mut payload = serde_json::json!({
        "fleetId": fleet.id,
        "commandId": fleet.command_id,
        "mission": fleet.mission_type,
        "status": fleet.status,
        "ships": if detailed { serde_json::Value::Array(ships) } else { serde_json::Value::Null },
        "totalShips": total_ships,
        "origin": format!("[{}:{}:{}]", fleet.origin_galaxy, fleet.origin_system, fleet.origin_position),
        "destination": format!("[{}:{}:{}]", fleet.target_galaxy, fleet.target_system, fleet.target_position),
        "originKind": fleet.origin_kind,
        "targetKind": fleet.target_kind,
        "departedAt": fleet.departed_at_unix,
        "arrivesAt": fleet.arrives_at_unix,
        "returnsAt": fleet.returns_at_unix,
        "phaseDueAt": fleet.phase_due_at_unix,
        "etaSeconds": fleet.phase_due_at_unix.saturating_sub(now).max(0),
        "speedPercent": fleet.applied_speed_percent,
        "holdSeconds": fleet.hold_seconds,
        "fuelConsumed": fleet.fuel_consumed,
        "cargo": {"metal": fleet.cargo_metal, "crystal": fleet.cargo_crystal, "deuterium": fleet.cargo_deuterium},
        "result": if detailed { fleet.result } else { serde_json::Value::Null }
    });
    if !detailed {
        payload
            .as_object_mut()
            .expect("fleet payload object")
            .remove("ships");
        payload
            .as_object_mut()
            .expect("fleet payload object")
            .remove("result");
    }
    payload
}

fn event_payload(event: FleetMissionEventRow) -> serde_json::Value {
    serde_json::json!({
        "sequence": event.sequence,
        "eventKey": event.event_key,
        "eventType": event.event_type,
        "phaseGeneration": event.phase_generation,
        "actorUserId": event.actor_user_id,
        "payload": event.payload,
        "occurredAt": event.occurred_at_unix
    })
}

fn canonical_ship_type(value: &str) -> Option<&'static str> {
    match value
        .trim()
        .to_ascii_lowercase()
        .replace(['-', '_', ' '], "")
        .as_str()
    {
        "smallcargo" => Some("small_cargo"),
        "largecargo" => Some("large_cargo"),
        "lightfighter" => Some("light_fighter"),
        "heavyfighter" => Some("heavy_fighter"),
        "cruiser" => Some("cruiser"),
        "battleship" => Some("battleship"),
        "battlecruiser" => Some("battlecruiser"),
        "bomber" => Some("bomber"),
        "destroyer" => Some("destroyer"),
        "deathstar" => Some("deathstar"),
        "recycler" => Some("recycler"),
        "espionageprobe" => Some("espionage_probe"),
        "colonyship" => Some("colony_ship"),
        _ => None,
    }
}

fn fleet_error_response(error: FleetWriteError) -> Response {
    match error {
        FleetWriteError::NotFound => not_found("Fleet source or target not found"),
        FleetWriteError::IdempotencyConflict => conflict(&error.to_string()),
        FleetWriteError::Retryable(_) | FleetWriteError::Database(_) => repository_unavailable(),
        FleetWriteError::Restricted
        | FleetWriteError::VacationMode
        | FleetWriteError::Invalid(_)
        | FleetWriteError::TargetProtected
        | FleetWriteError::FleetSlotsExhausted
        | FleetWriteError::InsufficientShips
        | FleetWriteError::InsufficientResources => bad_request(&error.to_string()),
    }
}

fn repository_unavailable() -> Response {
    service_unavailable("Fleet repository is unavailable")
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_secs()).unwrap_or(i64::MAX)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_ship_names_accept_ui_and_database_forms() {
        assert_eq!(canonical_ship_type("smallCargo"), Some("small_cargo"));
        assert_eq!(canonical_ship_type("battle_cruiser"), Some("battlecruiser"));
        assert_eq!(canonical_ship_type("solarSatellite"), None);
    }

    #[test]
    fn launch_contract_rejects_unknown_or_client_derived_fields() {
        let base = serde_json::json!({
            "commandId": "command-1",
            "mission": "transport",
            "originPlanetId": "1",
            "targetKind": "planet",
            "targetGalaxy": 1,
            "targetSystem": 2,
            "targetPosition": 3,
            "ships": [{"shipType": "smallCargo", "count": 1}],
            "cargo": {"metal": 0, "crystal": 0, "deuterium": 0},
            "speedPercent": 100,
            "fuelConsumed": 1
        });
        assert!(serde_json::from_value::<FleetSendRequest>(base).is_err());
        let bad_ship = serde_json::json!({
            "commandId": "command-2",
            "mission": "transport",
            "originPlanetId": "1",
            "targetKind": "planet",
            "targetGalaxy": 1,
            "targetSystem": 2,
            "targetPosition": 3,
            "ships": [{"shipType": "smallCargo", "count": 1, "combatPower": 4}]
        });
        assert!(serde_json::from_value::<FleetSendRequest>(bad_ship).is_err());
    }
}
