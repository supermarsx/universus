use std::collections::HashMap;

use backend_core::{core, sim::simulate_combat};
use chrono::{SecondsFormat, Utc};
use napi::Result;
use napi_derive::napi;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
struct SimulateBattleRequest {
    battle_id: String,
    attacker_ships: HashMap<String, i64>,
    defender_ships: HashMap<String, i64>,
    defender_defenses: HashMap<String, i64>,
    attacker_tech: HashMap<String, i64>,
    defender_tech: HashMap<String, i64>,
    planet_metal: i64,
    planet_crystal: i64,
    planet_deuterium: i64,
    seed: Option<String>,
    universe: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RoundResultOut {
    attacker_shots: i32,
    defender_shots: i32,
    attacker_destroyed: i32,
    defender_destroyed: i32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CombatResultOut {
    winner: String,
    rounds: Vec<RoundResultOut>,
    attacker_losses: HashMap<String, i32>,
    defender_losses: HashMap<String, i32>,
    loot: ResourceTripletOut,
    debris: ResourcePairOut,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResourceTripletOut {
    metal: i64,
    crystal: i64,
    deuterium: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResourcePairOut {
    metal: i64,
    crystal: i64,
}

#[derive(Debug, Deserialize)]
struct FleetMovementRequest {
    origin_galaxy: i32,
    origin_system: i32,
    origin_position: i32,
    target_galaxy: i32,
    target_system: i32,
    target_position: i32,
    ships: Vec<ShipMovementSpec>,
}

#[derive(Debug, Deserialize)]
struct ShipMovementSpec {
    count: i32,
    base_speed: f64,
    fuel_consumption: f64,
    cargo: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FleetMovementResult {
    distance: i32,
    fleet_speed: f64,
    travel_time_seconds: i32,
    fuel_needed: f64,
    cargo_capacity: f64,
}

#[derive(Debug, Deserialize)]
struct DefenseLossResolveRequest {
    current: HashMap<String, i64>,
    losses: HashMap<String, i64>,
    rebuild_rate: Option<f64>,
    seed: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DefenseLossResolveResponse {
    updated: HashMap<String, i64>,
}

#[derive(Debug, Deserialize)]
struct PostCombatDistributionRequest {
    participants: Vec<HashMap<String, i64>>,
    total_losses: HashMap<String, i64>,
    loot: PostCombatLoot,
    winner: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct PostCombatLoot {
    metal: i64,
    crystal: i64,
    deuterium: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PostCombatDistributionResponse {
    participants: Vec<PostCombatParticipantResult>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PostCombatParticipantResult {
    survivors: HashMap<String, i64>,
    loot: PostCombatLoot,
}

#[derive(Debug, Deserialize)]
struct MissionCargoTransferRequest {
    #[serde(alias = "transfer_metal", alias = "transferMetal")]
    metal: i64,
    #[serde(alias = "transfer_crystal", alias = "transferCrystal")]
    crystal: i64,
    #[serde(alias = "transfer_deuterium", alias = "transferDeuterium")]
    deuterium: i64,
    #[serde(alias = "clamp_non_negative", alias = "clampNonNegative")]
    clamp_non_negative: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MissionCargoTransferResult {
    transfer_metal: i64,
    transfer_crystal: i64,
    transfer_deuterium: i64,
    remaining_metal: i64,
    remaining_crystal: i64,
    remaining_deuterium: i64,
    total_transfer: i64,
}

#[derive(Debug, Deserialize)]
struct HarvestCollectionRequest {
    #[serde(alias = "debris_metal", alias = "debrisMetal")]
    debris_metal: i64,
    #[serde(alias = "debris_crystal", alias = "debrisCrystal")]
    debris_crystal: i64,
    #[serde(alias = "recycler_count", alias = "recyclerCount")]
    recycler_count: i64,
    #[serde(alias = "recycler_cargo_capacity", alias = "recyclerCargoCapacity")]
    recycler_cargo_capacity: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HarvestCollectionResult {
    collected_metal: i64,
    collected_crystal: i64,
    updated_metal: i64,
    updated_crystal: i64,
    recycler_capacity: i64,
    empty: bool,
}

#[derive(Debug, Deserialize)]
struct CombatReportSummaryRequest {
    report_id: i64,
    mission: String,
    target: CombatTarget,
    attacker_id: i64,
    defender_id: Option<i64>,
    winner: String,
    loot: PostCombatLoot,
    attacker_losses: HashMap<String, i64>,
    defender_losses: HashMap<String, i64>,
    attacker_allies: Vec<CombatReportAlly>,
}

#[derive(Debug, Deserialize, Serialize)]
struct CombatTarget {
    galaxy: i32,
    system: i32,
    position: i32,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CombatReportAlly {
    user_id: i64,
    username: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CombatReportSummaryResponse {
    id: i64,
    mission: String,
    target: CombatTarget,
    attacker_id: i64,
    defender_id: Option<i64>,
    winner: String,
    loot: PostCombatLoot,
    attacker_losses: HashMap<String, i64>,
    defender_losses: HashMap<String, i64>,
    timestamp: String,
    attacker_allies: Vec<CombatReportAlly>,
}

#[napi(object)]
pub struct NapiShipMovementSpec {
    pub count: i32,
    pub base_speed: f64,
    pub fuel_consumption: f64,
    pub cargo: f64,
}

#[napi(object)]
pub struct NapiFleetMovementRequest {
    pub origin_galaxy: i32,
    pub origin_system: i32,
    pub origin_position: i32,
    pub target_galaxy: i32,
    pub target_system: i32,
    pub target_position: i32,
    pub ships: Vec<NapiShipMovementSpec>,
}

#[napi(object)]
pub struct NapiFleetMovementResult {
    pub distance: i32,
    pub fleet_speed: f64,
    pub travel_time_seconds: i32,
    pub fuel_needed: f64,
    pub cargo_capacity: f64,
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
    if h == 0 {
        1
    } else {
        h
    }
}

fn to_i32_map(input: HashMap<String, i64>) -> HashMap<String, i32> {
    input
        .into_iter()
        .map(|(k, v)| {
            let value = if v > i32::MAX as i64 {
                i32::MAX
            } else if v < i32::MIN as i64 {
                i32::MIN
            } else {
                v as i32
            };
            (k, value)
        })
        .collect()
}

#[napi]
pub fn simulate_battle(payload_json: String) -> Result<String> {
    let payload: SimulateBattleRequest = serde_json::from_str(&payload_json)
        .map_err(|e| napi::Error::from_reason(format!("invalid simulate payload: {}", e)))?;

    let request = core::SimulateRequest {
        battle_id: payload.battle_id,
        attacker_ships: to_i32_map(payload.attacker_ships),
        defender_ships: to_i32_map(payload.defender_ships),
        defender_defenses: to_i32_map(payload.defender_defenses),
        attacker_tech: to_i32_map(payload.attacker_tech),
        defender_tech: to_i32_map(payload.defender_tech),
        planet_metal: payload.planet_metal,
        planet_crystal: payload.planet_crystal,
        planet_deuterium: payload.planet_deuterium,
        seed: payload.seed.unwrap_or_default(),
        universe: payload.universe.unwrap_or_else(|| "default".to_string()),
    };

    let result = simulate_combat(&request);
    let loot = result.loot.unwrap_or(core::Loot {
        metal: 0,
        crystal: 0,
        deuterium: 0,
    });
    let debris = result.debris.unwrap_or(core::Debris {
        metal: 0,
        crystal: 0,
    });

    let output = CombatResultOut {
        winner: result.winner,
        rounds: result
            .rounds
            .into_iter()
            .map(|round| RoundResultOut {
                attacker_shots: round.attacker_shots,
                defender_shots: round.defender_shots,
                attacker_destroyed: round.attacker_destroyed,
                defender_destroyed: round.defender_destroyed,
            })
            .collect(),
        attacker_losses: result.attacker_losses,
        defender_losses: result.defender_losses,
        loot: ResourceTripletOut {
            metal: loot.metal,
            crystal: loot.crystal,
            deuterium: loot.deuterium,
        },
        debris: ResourcePairOut {
            metal: debris.metal,
            crystal: debris.crystal,
        },
    };

    serde_json::to_string(&output)
        .map_err(|e| napi::Error::from_reason(format!("serialize combat result failed: {}", e)))
}

#[napi]
pub fn calculate_fleet_movement(payload_json: String) -> Result<String> {
    let payload: FleetMovementRequest = serde_json::from_str(&payload_json)
        .map_err(|e| napi::Error::from_reason(format!("invalid movement payload: {}", e)))?;

    let distance = if payload.origin_galaxy != payload.target_galaxy {
        (payload.origin_galaxy - payload.target_galaxy).abs() * 20000
    } else if payload.origin_system != payload.target_system {
        (payload.origin_system - payload.target_system).abs() * 5 * 19 + 2700
    } else {
        (payload.origin_position - payload.target_position).abs() * 5 + 1000
    };

    let mut min_speed = f64::INFINITY;
    let mut fuel_needed = 0.0f64;
    let mut cargo_capacity = 0.0f64;

    for ship in payload.ships.iter() {
        if ship.count <= 0 {
            continue;
        }
        if ship.base_speed > 0.0 {
            min_speed = min_speed.min(ship.base_speed);
        }
        let count = ship.count as f64;
        fuel_needed += ship.fuel_consumption * count * (distance as f64 / 100.0);
        cargo_capacity += ship.cargo * count;
    }

    let fleet_speed = if min_speed.is_finite() { min_speed } else { 0.0 };
    let travel_time_seconds = if fleet_speed > 0.0 {
        ((distance as f64 / fleet_speed) * 3600.0).ceil() as i32
    } else {
        0
    };

    cargo_capacity -= fuel_needed;

    let output = FleetMovementResult {
        distance,
        fleet_speed,
        travel_time_seconds,
        fuel_needed,
        cargo_capacity,
    };

    serde_json::to_string(&output)
        .map_err(|e| napi::Error::from_reason(format!("serialize movement result failed: {}", e)))
}

fn calculate_movement(
    origin_galaxy: i32,
    origin_system: i32,
    origin_position: i32,
    target_galaxy: i32,
    target_system: i32,
    target_position: i32,
    ships: &[NapiShipMovementSpec],
) -> NapiFleetMovementResult {
    let distance = if origin_galaxy != target_galaxy {
        (origin_galaxy - target_galaxy).abs() * 20000
    } else if origin_system != target_system {
        (origin_system - target_system).abs() * 5 * 19 + 2700
    } else {
        (origin_position - target_position).abs() * 5 + 1000
    };

    let mut min_speed = f64::INFINITY;
    let mut fuel_needed = 0.0f64;
    let mut cargo_capacity = 0.0f64;

    for ship in ships.iter() {
        if ship.count <= 0 {
            continue;
        }
        if ship.base_speed > 0.0 {
            min_speed = min_speed.min(ship.base_speed);
        }
        let count = ship.count as f64;
        fuel_needed += ship.fuel_consumption * count * (distance as f64 / 100.0);
        cargo_capacity += ship.cargo * count;
    }

    let fleet_speed = if min_speed.is_finite() { min_speed } else { 0.0 };
    let travel_time_seconds = if fleet_speed > 0.0 {
        ((distance as f64 / fleet_speed) * 3600.0).ceil() as i32
    } else {
        0
    };

    cargo_capacity -= fuel_needed;

    NapiFleetMovementResult {
        distance,
        fleet_speed,
        travel_time_seconds,
        fuel_needed,
        cargo_capacity,
    }
}

#[napi]
pub fn calculate_fleet_movement_fast(payload: NapiFleetMovementRequest) -> Result<NapiFleetMovementResult> {
    Ok(calculate_movement(
        payload.origin_galaxy,
        payload.origin_system,
        payload.origin_position,
        payload.target_galaxy,
        payload.target_system,
        payload.target_position,
        &payload.ships,
    ))
}

#[napi]
pub fn calculate_fleet_movement_batch(payload: Vec<NapiFleetMovementRequest>) -> Result<Vec<NapiFleetMovementResult>> {
    let mut out = Vec::with_capacity(payload.len());
    for req in payload.iter() {
        out.push(calculate_movement(
            req.origin_galaxy,
            req.origin_system,
            req.origin_position,
            req.target_galaxy,
            req.target_system,
            req.target_position,
            &req.ships,
        ));
    }
    Ok(out)
}

#[napi]
pub fn resolve_defense_losses(payload_json: String) -> Result<String> {
    let payload: DefenseLossResolveRequest = serde_json::from_str(&payload_json)
        .map_err(|e| napi::Error::from_reason(format!("invalid defense-loss payload: {}", e)))?;

    let rebuild_rate = payload.rebuild_rate.unwrap_or(0.7).clamp(0.0, 1.0);
    let seed = payload.seed.unwrap_or_else(|| "defense-loss".to_string());
    let mut rng = Mulberry32::new(calc_seed(&seed));

    let mut updated: HashMap<String, i64> = HashMap::new();

    for (unit, current_value) in payload.current.iter() {
        let current = (*current_value).max(0);
        let loss = payload.losses.get(unit).copied().unwrap_or(0).max(0);
        if loss == 0 {
            updated.insert(unit.clone(), current);
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
        updated.insert(unit.clone(), remaining);
    }

    let response = DefenseLossResolveResponse { updated };
    serde_json::to_string(&response).map_err(|e| {
        napi::Error::from_reason(format!("serialize defense-loss response failed: {}", e))
    })
}

#[napi]
pub fn compute_attacker_post_combat_distribution(payload_json: String) -> Result<String> {
    let payload: PostCombatDistributionRequest = serde_json::from_str(&payload_json).map_err(|e| {
        napi::Error::from_reason(format!(
            "invalid attacker post-combat distribution payload: {}",
            e
        ))
    })?;

    let participant_count = payload.participants.len();
    if participant_count == 0 {
        let response = PostCombatDistributionResponse {
            participants: Vec::new(),
        };
        return serde_json::to_string(&response).map_err(|e| {
            napi::Error::from_reason(format!(
                "serialize attacker post-combat distribution response failed: {}",
                e
            ))
        });
    }

    let mut totals: HashMap<String, i64> = HashMap::new();
    for participant in payload.participants.iter() {
        for (unit, count) in participant.iter() {
            *totals.entry(unit.clone()).or_insert(0) += (*count).max(0);
        }
    }

    let unit_types: Vec<String> = payload.total_losses.keys().cloned().collect();
    let mut allocations: Vec<HashMap<String, i64>> = vec![HashMap::new(); participant_count];
    let mut allocated: HashMap<String, i64> = HashMap::new();

    for (index, participant) in payload.participants.iter().enumerate() {
        for unit in unit_types.iter() {
            let total_loss = payload.total_losses.get(unit).copied().unwrap_or(0).max(0);
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

    let loot_pool = if payload.winner == "attacker" {
        payload.loot
    } else {
        PostCombatLoot {
            metal: 0,
            crystal: 0,
            deuterium: 0,
        }
    };
    let mut loot_shares = vec![
        PostCombatLoot {
            metal: 0,
            crystal: 0,
            deuterium: 0
        };
        participant_count
    ];

    let mut split_resource = |getter: fn(&PostCombatLoot) -> i64, setter: fn(&mut PostCombatLoot, i64)| {
        let mut remaining = getter(&loot_pool).max(0);
        for idx in 0..participant_count {
            let divisor = (participant_count - idx) as i64;
            let value = if divisor > 0 { remaining / divisor } else { 0 };
            setter(&mut loot_shares[idx], value);
            remaining -= value;
        }
    };
    split_resource(|loot| loot.metal, |loot, value| loot.metal = value);
    split_resource(|loot| loot.crystal, |loot, value| loot.crystal = value);
    split_resource(|loot| loot.deuterium, |loot, value| loot.deuterium = value);

    let mut participants = Vec::with_capacity(participant_count);
    for idx in 0..participant_count {
        let participant = &payload.participants[idx];
        let losses = &allocations[idx];
        let mut survivors: HashMap<String, i64> = HashMap::new();
        for (unit, count) in participant.iter() {
            let loss = losses.get(unit).copied().unwrap_or(0).max(0);
            let remaining = (count.max(&0) - loss).max(0);
            if remaining > 0 {
                survivors.insert(unit.clone(), remaining);
            }
        }

        participants.push(PostCombatParticipantResult {
            survivors,
            loot: PostCombatLoot {
                metal: loot_shares[idx].metal,
                crystal: loot_shares[idx].crystal,
                deuterium: loot_shares[idx].deuterium,
            },
        });
    }

    let response = PostCombatDistributionResponse { participants };
    serde_json::to_string(&response).map_err(|e| {
        napi::Error::from_reason(format!(
            "serialize attacker post-combat distribution response failed: {}",
            e
        ))
    })
}

#[napi]
pub fn compute_mission_cargo_transfer(payload_json: String) -> Result<String> {
    let payload: MissionCargoTransferRequest = serde_json::from_str(&payload_json).map_err(|e| {
        napi::Error::from_reason(format!(
            "invalid mission cargo transfer payload: {}",
            e
        ))
    })?;

    let clamp_non_negative = payload.clamp_non_negative.unwrap_or(false);

    let mut transfer_metal = payload.metal;
    let mut transfer_crystal = payload.crystal;
    let mut transfer_deuterium = payload.deuterium;

    if clamp_non_negative {
        transfer_metal = transfer_metal.max(0);
        transfer_crystal = transfer_crystal.max(0);
        transfer_deuterium = transfer_deuterium.max(0);
    }

    let total_transfer = transfer_metal
        .saturating_add(transfer_crystal)
        .saturating_add(transfer_deuterium);

    let response = MissionCargoTransferResult {
        transfer_metal,
        transfer_crystal,
        transfer_deuterium,
        remaining_metal: 0,
        remaining_crystal: 0,
        remaining_deuterium: 0,
        total_transfer,
    };

    serde_json::to_string(&response).map_err(|e| {
        napi::Error::from_reason(format!(
            "serialize mission cargo transfer response failed: {}",
            e
        ))
    })
}

#[napi]
pub fn compute_harvest_collection(payload_json: String) -> Result<String> {
    let payload: HarvestCollectionRequest = serde_json::from_str(&payload_json).map_err(|e| {
        napi::Error::from_reason(format!("invalid harvest collection payload: {}", e))
    })?;

    let debris_metal = payload.debris_metal.max(0);
    let debris_crystal = payload.debris_crystal.max(0);
    let recycler_count = payload.recycler_count.max(0);
    let recycler_cargo_capacity = payload.recycler_cargo_capacity.max(0);
    let recycler_capacity = recycler_count.saturating_mul(recycler_cargo_capacity);

    let collected_metal = debris_metal.min(recycler_capacity);
    let remaining_capacity = recycler_capacity.saturating_sub(collected_metal);
    let collected_crystal = debris_crystal.min(remaining_capacity);

    let updated_metal = debris_metal.saturating_sub(collected_metal);
    let updated_crystal = debris_crystal.saturating_sub(collected_crystal);

    let response = HarvestCollectionResult {
        collected_metal,
        collected_crystal,
        updated_metal,
        updated_crystal,
        recycler_capacity,
        empty: collected_metal == 0 && collected_crystal == 0,
    };

    serde_json::to_string(&response).map_err(|e| {
        napi::Error::from_reason(format!(
            "serialize harvest collection response failed: {}",
            e
        ))
    })
}

#[napi]
pub fn compute_combat_report_summary(payload_json: String) -> Result<String> {
    let payload: CombatReportSummaryRequest = serde_json::from_str(&payload_json).map_err(|e| {
        napi::Error::from_reason(format!(
            "invalid combat report summary payload: {}",
            e
        ))
    })?;

    let response = CombatReportSummaryResponse {
        id: payload.report_id,
        mission: payload.mission,
        target: payload.target,
        attacker_id: payload.attacker_id,
        defender_id: payload.defender_id,
        winner: payload.winner,
        loot: payload.loot,
        attacker_losses: payload.attacker_losses,
        defender_losses: payload.defender_losses,
        timestamp: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        attacker_allies: payload.attacker_allies,
    };

    serde_json::to_string(&response).map_err(|e| {
        napi::Error::from_reason(format!(
            "serialize combat report summary response failed: {}",
            e
        ))
    })
}
