//! HTTP helper server for fleet/combat deterministic utility endpoints.
//!
//! This binary mirrors the existing Node helper route contracts:
//! - `GET /health`
//! - `POST /api/fleet/helpers/movement`
//! - `POST /api/fleet/helpers/combat/defense-rebuild`
//! - `POST /api/fleet/helpers/combat/attacker-distribution`
//! - `POST /api/fleet/helpers/espionage-outcome`
//! - `POST /api/fleet/helpers/mission-cargo-transfer`
//! - `POST /api/fleet/helpers/harvest-collection`
//!
//! It intentionally keeps request validation and response envelope semantics
//! aligned with the Node routes: successful responses return
//! `{ success: true, data: ... }` and invalid input returns HTTP 400.

use std::collections::HashMap;
use std::net::SocketAddr;

use axum::extract::rejection::JsonRejection;
use axum::extract::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use backend_core::ships::load_ships_for_universe;
use serde::Serialize;
use serde_json::{Map, Value};

#[derive(Serialize)]
struct SuccessResponse<T> {
    success: bool,
    data: T,
}

#[derive(Serialize)]
struct ErrorResponse {
    success: bool,
    error: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MovementData {
    distance: i32,
    fleet_speed: f64,
    travel_time_seconds: i32,
    fuel_needed: f64,
    cargo_capacity: f64,
    engine: &'static str,
}

#[derive(Serialize)]
struct DefenseRebuildData {
    updated: HashMap<String, i64>,
    engine: &'static str,
}

#[derive(Clone, Copy, Serialize, Default)]
struct ResourceTriplet {
    metal: i64,
    crystal: i64,
    deuterium: i64,
}

#[derive(Serialize)]
struct ParticipantDistribution {
    survivors: HashMap<String, i64>,
    loot: ResourceTriplet,
}

#[derive(Serialize)]
struct AttackerDistributionData {
    participants: Vec<ParticipantDistribution>,
    engine: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EspionageOutcomeData {
    intel_level: &'static str,
    detected: bool,
    detection_chance: f64,
    detail_score: f64,
    defense_score: f64,
    engine: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MissionCargoTransferData {
    transfer_metal: i64,
    transfer_crystal: i64,
    transfer_deuterium: i64,
    remaining_metal: i64,
    remaining_crystal: i64,
    remaining_deuterium: i64,
    total_transfer: i64,
    engine: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HarvestCollectionData {
    collected_metal: i64,
    collected_crystal: i64,
    updated_metal: i64,
    updated_crystal: i64,
    recycler_capacity: i64,
    empty: bool,
    engine: &'static str,
}

#[derive(Clone, Copy)]
struct Mulberry32 {
    state: u32,
}

impl Mulberry32 {
    fn new(seed: u32) -> Self {
        Self { state: seed }
    }

    fn next_u32(&mut self) -> u32 {
        let mut t = self.state.wrapping_add(0x6D2B79F5);
        self.state = t;
        t = t.wrapping_mul(t ^ (t >> 15));
        t = t.wrapping_add(t.wrapping_mul(t ^ (t >> 7)));
        (t ^ (t >> 14)) as u32
    }

    fn next_f64(&mut self) -> f64 {
        (self.next_u32() as f64) / (u32::MAX as f64 + 1.0)
    }
}

fn calc_seed(seed: &str) -> u32 {
    let mut h: u32 = 0x811c9dc5;
    for b in seed.as_bytes() {
        h ^= *b as u32;
        h = h.wrapping_mul(16777619);
    }
    if h == 0 { 1 } else { h }
}

fn success<T: Serialize>(data: T) -> Response {
    (
        StatusCode::OK,
        Json(SuccessResponse {
            success: true,
            data,
        }),
    )
        .into_response()
}

fn bad_request(message: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            success: false,
            error: message.to_string(),
        }),
    )
        .into_response()
}

fn internal_error(message: &str) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            success: false,
            error: message.to_string(),
        }),
    )
        .into_response()
}

fn js_truthy(value: Option<&Value>) -> bool {
    match value {
        None => false,
        Some(Value::Null) => false,
        Some(Value::Bool(v)) => *v,
        Some(Value::Number(v)) => v
            .as_f64()
            .map(|n| n.is_finite() && n != 0.0)
            .unwrap_or(false),
        Some(Value::String(v)) => !v.is_empty(),
        Some(Value::Array(_)) | Some(Value::Object(_)) => true,
    }
}

fn js_number(value: Option<&Value>) -> Option<f64> {
    match value {
        None => None,
        Some(Value::Null) => Some(0.0),
        Some(Value::Bool(v)) => Some(if *v { 1.0 } else { 0.0 }),
        Some(Value::Number(v)) => v.as_f64(),
        Some(Value::String(v)) => v.parse::<f64>().ok(),
        Some(Value::Array(_)) | Some(Value::Object(_)) => None,
    }
}

fn trunc_f64_to_i64(value: f64) -> i64 {
    if value.is_nan() {
        0
    } else if value >= i64::MAX as f64 {
        i64::MAX
    } else if value <= i64::MIN as f64 {
        i64::MIN
    } else {
        value.trunc() as i64
    }
}

fn to_js_int(value: Option<&Value>) -> Option<i64> {
    let number = js_number(value)?;
    if number.is_finite() {
        Some(trunc_f64_to_i64(number))
    } else {
        None
    }
}

fn normalize_non_negative_int(value: Option<&Value>) -> i64 {
    let number = js_number(value).unwrap_or(0.0);
    if !number.is_finite() {
        0
    } else {
        trunc_f64_to_i64(number).max(0)
    }
}

fn normalize_fleet_map(value: Option<&Value>) -> HashMap<String, i64> {
    let mut result = HashMap::new();
    let Some(Value::Object(map)) = value else {
        return result;
    };
    for (unit, raw_count) in map {
        let count = normalize_non_negative_int(Some(raw_count));
        if count > 0 {
            result.insert(unit.clone(), count);
        }
    }
    result
}

fn normalize_coords(value: &Value) -> Option<(i32, i32, i32)> {
    let obj = value.as_object()?;
    let galaxy = to_js_int(obj.get("galaxy"))?;
    let system = to_js_int(obj.get("system"))?;
    let position = to_js_int(obj.get("position"))?;
    Some((
        galaxy.clamp(i32::MIN as i64, i32::MAX as i64) as i32,
        system.clamp(i32::MIN as i64, i32::MAX as i64) as i32,
        position.clamp(i32::MIN as i64, i32::MAX as i64) as i32,
    ))
}

fn coalesce_object(value: Option<&Value>) -> Value {
    if js_truthy(value) {
        value.cloned().unwrap_or_else(|| Value::Object(Map::new()))
    } else {
        Value::Object(Map::new())
    }
}

fn pick<'a>(root: Option<&'a Map<String, Value>>, keys: &[&str]) -> Option<&'a Value> {
    let root = root?;
    keys.iter().find_map(|key| root.get(*key))
}

fn calculate_distance(
    origin_galaxy: i32,
    origin_system: i32,
    origin_position: i32,
    target_galaxy: i32,
    target_system: i32,
    target_position: i32,
) -> i32 {
    if origin_galaxy != target_galaxy {
        (origin_galaxy - target_galaxy).abs() * 20000
    } else if origin_system != target_system {
        (origin_system - target_system).abs() * 5 * 19 + 2700
    } else {
        (origin_position - target_position).abs() * 5 + 1000
    }
}

fn derive_movement_ship_stats(
    ship_type: &str,
    universe_ship_defs: &HashMap<String, backend_core::ships::ShipDef>,
) -> (f64, f64, f64) {
    universe_ship_defs
        .get(ship_type)
        .map(|ship| {
            (
                ship.weapon.unwrap_or(0.0),
                ship.deuterium_cost.unwrap_or(0) as f64,
                ship.cargo.unwrap_or(0) as f64,
            )
        })
        .unwrap_or((0.0, 0.0, 0.0))
}

async fn health_handler() -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({ "status": "ok" })))
}

async fn movement_handler(payload: Result<Json<Value>, JsonRejection>) -> Response {
    let Json(body) = match payload {
        Ok(body) => body,
        Err(_) => return bad_request("Invalid fleet helper movement request"),
    };

    let root = body.as_object();
    let origin_value = coalesce_object(root.and_then(|m| m.get("origin")));
    let target_value = coalesce_object(root.and_then(|m| m.get("target")));
    let ships_value = coalesce_object(root.and_then(|m| m.get("ships")));

    let Some((origin_galaxy, origin_system, origin_position)) = normalize_coords(&origin_value)
    else {
        return bad_request("Invalid fleet helper movement request");
    };
    let Some((target_galaxy, target_system, target_position)) = normalize_coords(&target_value)
    else {
        return bad_request("Invalid fleet helper movement request");
    };

    if !ships_value.is_object() || ships_value.is_array() {
        return bad_request("Invalid fleet helper movement request");
    }
    let ships = normalize_fleet_map(Some(&ships_value));

    let distance = calculate_distance(
        origin_galaxy,
        origin_system,
        origin_position,
        target_galaxy,
        target_system,
        target_position,
    );
    let ship_defs = load_ships_for_universe("default");

    let mut min_speed = f64::INFINITY;
    let mut fuel_needed = 0.0f64;
    let mut cargo_capacity = 0.0f64;
    for (ship_type, count_raw) in ships {
        let count = count_raw.max(0);
        if count <= 0 {
            continue;
        }
        let (base_speed, fuel_consumption, cargo) = derive_movement_ship_stats(&ship_type, &ship_defs);
        if base_speed > 0.0 {
            min_speed = min_speed.min(base_speed);
        }
        let count = count as f64;
        fuel_needed += fuel_consumption * count * (distance as f64 / 100.0);
        cargo_capacity += cargo * count;
    }

    let fleet_speed = if min_speed.is_finite() { min_speed } else { 0.0 };
    let travel_time_seconds = if fleet_speed > 0.0 {
        ((distance as f64 / fleet_speed) * 3600.0).ceil() as i32
    } else {
        0
    };
    cargo_capacity -= fuel_needed;

    success(MovementData {
        distance,
        fleet_speed,
        travel_time_seconds,
        fuel_needed,
        cargo_capacity,
        engine: "rust-napi",
    })
}

async fn defense_rebuild_handler(payload: Result<Json<Value>, JsonRejection>) -> Response {
    let Json(body) = match payload {
        Ok(body) => body,
        Err(_) => return bad_request("Invalid defense rebuild request"),
    };

    let root = body.as_object();
    let current_value = coalesce_object(root.and_then(|m| m.get("current")));
    let losses_value = coalesce_object(root.and_then(|m| m.get("losses")));

    if !current_value.is_object()
        || current_value.is_array()
        || !losses_value.is_object()
        || losses_value.is_array()
    {
        return bad_request("Invalid defense rebuild request");
    }

    let rebuild_rate = root
        .and_then(|m| m.get("rebuildRate"))
        .and_then(|v| js_number(Some(v)))
        .filter(|v| v.is_finite())
        .map(|v| v.clamp(0.0, 1.0))
        .unwrap_or(0.7);
    let seed = root
        .and_then(|m| m.get("seed"))
        .and_then(|v| v.as_str())
        .unwrap_or("defense-loss");
    let mut rng = Mulberry32::new(calc_seed(seed));

    let current_map = current_value.as_object().cloned().unwrap_or_default();
    let losses_map = losses_value.as_object().cloned().unwrap_or_default();

    let mut updated = HashMap::new();
    for (unit, current_raw) in current_map {
        let current = normalize_non_negative_int(Some(&current_raw));
        let loss = normalize_non_negative_int(losses_map.get(&unit));
        if loss == 0 {
            updated.insert(unit, current);
            continue;
        }
        let mut rebuilt = 0i64;
        for _ in 0..loss {
            if rng.next_f64() < rebuild_rate {
                rebuilt += 1;
            }
        }
        let effective_loss = (loss - rebuilt).max(0);
        let remaining = (current - effective_loss).max(0);
        updated.insert(unit, remaining);
    }

    success(DefenseRebuildData {
        updated,
        engine: "rust-napi",
    })
}

fn split_loot(loot: ResourceTriplet, parts: usize) -> Vec<ResourceTriplet> {
    if parts == 0 {
        return Vec::new();
    }
    let mut shares = vec![ResourceTriplet::default(); parts];
    for resource in ["metal", "crystal", "deuterium"] {
        let mut remaining = match resource {
            "metal" => loot.metal.max(0),
            "crystal" => loot.crystal.max(0),
            _ => loot.deuterium.max(0),
        };
        for (idx, share) in shares.iter_mut().enumerate() {
            let divisor = (parts - idx) as i64;
            let value = if divisor > 0 { remaining / divisor } else { 0 };
            match resource {
                "metal" => share.metal = value,
                "crystal" => share.crystal = value,
                _ => share.deuterium = value,
            }
            remaining -= value;
        }
    }
    shares
}

async fn attacker_distribution_handler(payload: Result<Json<Value>, JsonRejection>) -> Response {
    let Json(body) = match payload {
        Ok(body) => body,
        Err(_) => return bad_request("Invalid attacker distribution request"),
    };

    let root = body.as_object();
    let participants_value = root.and_then(|m| m.get("participants"));
    let total_losses_value = coalesce_object(root.and_then(|m| m.get("totalLosses")));
    let loot_value = coalesce_object(root.and_then(|m| m.get("loot")));
    let winner = root
        .and_then(|m| m.get("winner"))
        .map(|v| match v {
            Value::String(s) => s.clone(),
            _ => v.to_string(),
        })
        .unwrap_or_default();

    let Some(participants_array) = participants_value.and_then(Value::as_array) else {
        return bad_request("Invalid attacker distribution request");
    };
    if !total_losses_value.is_object()
        || total_losses_value.is_array()
        || !loot_value.is_object()
        || loot_value.is_array()
        || !matches!(winner.as_str(), "attacker" | "defender" | "draw")
    {
        return bad_request("Invalid attacker distribution request");
    }

    let participants: Vec<HashMap<String, i64>> = participants_array
        .iter()
        .map(|participant| normalize_fleet_map(Some(participant)))
        .collect();
    let total_losses = normalize_fleet_map(Some(&total_losses_value));
    let loot_object = loot_value.as_object().cloned().unwrap_or_default();
    let loot = ResourceTriplet {
        metal: normalize_non_negative_int(loot_object.get("metal")),
        crystal: normalize_non_negative_int(loot_object.get("crystal")),
        deuterium: normalize_non_negative_int(loot_object.get("deuterium")),
    };

    let participant_count = participants.len();
    if participant_count == 0 {
        return success(AttackerDistributionData {
            participants: Vec::new(),
            engine: "rust-napi",
        });
    }

    let mut totals: HashMap<String, i64> = HashMap::new();
    for participant in &participants {
        for (unit, count) in participant {
            *totals.entry(unit.clone()).or_insert(0) += (*count).max(0);
        }
    }

    let unit_types: Vec<String> = total_losses.keys().cloned().collect();
    let mut allocations: Vec<HashMap<String, i64>> = vec![HashMap::new(); participant_count];
    let mut allocated: HashMap<String, i64> = HashMap::new();

    for (index, participant) in participants.iter().enumerate() {
        for unit in &unit_types {
            let total_loss = total_losses.get(unit).copied().unwrap_or(0).max(0);
            if total_loss == 0 {
                allocations[index].insert(unit.clone(), 0);
                continue;
            }

            let fleet_count = participant.get(unit).copied().unwrap_or(0).max(0);
            let total_count = totals.get(unit).copied().unwrap_or(0).max(0);
            if fleet_count == 0 || total_count == 0 {
                allocations[index].insert(unit.clone(), 0);
                continue;
            }

            let loss = if index == participant_count - 1 {
                let already = allocated.get(unit).copied().unwrap_or(0).max(0);
                let remaining = (total_loss - already).max(0);
                remaining.min(fleet_count)
            } else {
                let proportional = ((total_loss as f64 * fleet_count as f64) / total_count as f64).round() as i64;
                let clamped = proportional.min(fleet_count).max(0);
                *allocated.entry(unit.clone()).or_insert(0) += clamped;
                clamped
            };

            allocations[index].insert(unit.clone(), loss);
        }
    }

    let loot_pool = if winner == "attacker" {
        loot
    } else {
        ResourceTriplet::default()
    };
    let loot_shares = split_loot(loot_pool, participant_count);

    let mut participant_results = Vec::with_capacity(participant_count);
    for idx in 0..participant_count {
        let participant = &participants[idx];
        let losses = &allocations[idx];
        let mut survivors = HashMap::new();

        for (unit, count) in participant {
            let loss = losses.get(unit).copied().unwrap_or(0).max(0);
            let remaining = (count.max(&0) - loss).max(0);
            if remaining > 0 {
                survivors.insert(unit.clone(), remaining);
            }
        }

        participant_results.push(ParticipantDistribution {
            survivors,
            loot: loot_shares.get(idx).copied().unwrap_or_default(),
        });
    }

    success(AttackerDistributionData {
        participants: participant_results,
        engine: "rust-napi",
    })
}

async fn espionage_outcome_handler(payload: Result<Json<Value>, JsonRejection>) -> Response {
    let Json(body) = match payload {
        Ok(body) => body,
        Err(_) => return bad_request("Invalid espionage outcome request"),
    };

    let root = body.as_object();
    let probes = match to_js_int(pick(root, &["probes"])) {
        Some(value) => value.max(0) as f64,
        None => return bad_request("Invalid espionage outcome request"),
    };
    let attacker_espionage = match js_number(pick(root, &["attackerEspionage", "attacker_espionage"])) {
        Some(value) if value.is_finite() => value,
        _ => return bad_request("Invalid espionage outcome request"),
    };
    let defender_espionage = match js_number(pick(root, &["defenderEspionage", "defender_espionage"])) {
        Some(value) if value.is_finite() => value,
        _ => return bad_request("Invalid espionage outcome request"),
    };
    let seed = pick(root, &["seed"])
        .and_then(|value| value.as_str())
        .unwrap_or("espionage");

    let detail_score = attacker_espionage + (probes + 1.0).log2();
    let defense_score = defender_espionage;
    let detail_delta = detail_score - defense_score;
    let intel_level = if detail_delta >= 3.0 {
        "full"
    } else if detail_delta >= 0.0 {
        "standard"
    } else {
        "minimal"
    };

    let detection_chance = (0.5 + (defense_score - detail_score) * 0.05).clamp(0.05, 0.95);
    let mut rng = Mulberry32::new(calc_seed(seed));
    let detected = rng.next_f64() < detection_chance;

    success(EspionageOutcomeData {
        intel_level,
        detected,
        detection_chance,
        detail_score,
        defense_score,
        engine: "rust-napi",
    })
}

async fn mission_cargo_transfer_handler(payload: Result<Json<Value>, JsonRejection>) -> Response {
    let Json(body) = match payload {
        Ok(body) => body,
        Err(_) => return bad_request("Invalid mission cargo transfer request"),
    };

    let root = body.as_object();
    let metal = match to_js_int(pick(root, &["metal", "transferMetal", "transfer_metal"])) {
        Some(value) => value,
        None => return bad_request("Invalid mission cargo transfer request"),
    };
    let crystal = match to_js_int(pick(root, &["crystal", "transferCrystal", "transfer_crystal"])) {
        Some(value) => value,
        None => return bad_request("Invalid mission cargo transfer request"),
    };
    let deuterium = match to_js_int(pick(root, &["deuterium", "transferDeuterium", "transfer_deuterium"])) {
        Some(value) => value,
        None => return bad_request("Invalid mission cargo transfer request"),
    };

    let clamp_non_negative = pick(root, &["clampNonNegative", "clamp_non_negative"])
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let transfer_metal = if clamp_non_negative { metal.max(0) } else { metal };
    let transfer_crystal = if clamp_non_negative { crystal.max(0) } else { crystal };
    let transfer_deuterium = if clamp_non_negative {
        deuterium.max(0)
    } else {
        deuterium
    };
    let total_transfer = transfer_metal
        .saturating_add(transfer_crystal)
        .saturating_add(transfer_deuterium);

    success(MissionCargoTransferData {
        transfer_metal,
        transfer_crystal,
        transfer_deuterium,
        remaining_metal: 0,
        remaining_crystal: 0,
        remaining_deuterium: 0,
        total_transfer,
        engine: "rust-napi",
    })
}

async fn harvest_collection_handler(payload: Result<Json<Value>, JsonRejection>) -> Response {
    let Json(body) = match payload {
        Ok(body) => body,
        Err(_) => return bad_request("Invalid harvest collection request"),
    };

    let root = body.as_object();
    let debris_metal = match to_js_int(pick(root, &["debrisMetal", "debris_metal"])) {
        Some(value) => value.max(0),
        None => return bad_request("Invalid harvest collection request"),
    };
    let debris_crystal = match to_js_int(pick(root, &["debrisCrystal", "debris_crystal"])) {
        Some(value) => value.max(0),
        None => return bad_request("Invalid harvest collection request"),
    };
    let recycler_count = match to_js_int(pick(root, &["recyclerCount", "recycler_count"])) {
        Some(value) => value.max(0),
        None => return bad_request("Invalid harvest collection request"),
    };
    let recycler_cargo_capacity =
        match to_js_int(pick(root, &["recyclerCargoCapacity", "recycler_cargo_capacity"])) {
            Some(value) => value.max(0),
            None => return bad_request("Invalid harvest collection request"),
        };

    let recycler_capacity = recycler_count.saturating_mul(recycler_cargo_capacity);
    let collected_metal = debris_metal.min(recycler_capacity);
    let remaining_capacity = recycler_capacity.saturating_sub(collected_metal);
    let collected_crystal = debris_crystal.min(remaining_capacity);
    let updated_metal = debris_metal.saturating_sub(collected_metal);
    let updated_crystal = debris_crystal.saturating_sub(collected_crystal);

    success(HarvestCollectionData {
        collected_metal,
        collected_crystal,
        updated_metal,
        updated_crystal,
        recycler_capacity,
        empty: collected_metal == 0 && collected_crystal == 0,
        engine: "rust-napi",
    })
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/api/fleet/helpers/movement", post(movement_handler))
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
        );

    let bind_addr = std::env::var("CORE_HTTP_BIND_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:50052".to_string());
    let addr: SocketAddr = match bind_addr.parse() {
        Ok(value) => value,
        Err(err) => {
            let _ = internal_error(&format!("invalid CORE_HTTP_BIND_ADDR: {}", err));
            eprintln!("invalid CORE_HTTP_BIND_ADDR '{}': {}", bind_addr, err);
            std::process::exit(1);
        }
    };

    println!("backend-core-http listening on {}", addr);
    if let Err(err) = axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
    {
        eprintln!("backend-core-http server error: {}", err);
        std::process::exit(1);
    }
}
