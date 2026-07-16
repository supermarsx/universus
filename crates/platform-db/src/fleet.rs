//! Authoritative durable fleet persistence and mission processing.
//!
//! Every launch fact is derived server-side and committed in the same
//! transaction as ship, fuel, and cargo deduction. Mission phase effects are
//! protected by persisted leases, generation checks, exact-once markers, and
//! an append-only event timeline.

use std::collections::{BTreeMap, HashMap};
use std::fmt;

use game_combat::{generate_combat_report_with_seed_256, CombatConfig, CombatInput};
use game_fleet::{
    plan_authoritative_mission, recall_return_duration_seconds, Coordinates, FleetComposition,
    FleetMissionType, FleetPlanningConfig, FleetTargetKind, Resources,
};
use game_moon::{calculate_moon_destruction, MoonDestructionInput};
use rand::rngs::OsRng;
use rand::RngCore;
use sha2::{Digest, Sha256};
use tokio_postgres::error::SqlState;
use tokio_postgres::types::Json;
use tokio_postgres::Transaction;

use super::{Database, DbResult};

const MAX_FLEET_PROCESS_BATCH: usize = 1_000;
const MAX_FLEET_LEASE_SECONDS: i64 = 15 * 60;
const MAX_COMMAND_ID_BYTES: usize = 128;
const DEFAULT_PLANET_FLEET_SLOTS: i64 = 16;
const SHIP_TYPES: [&str; 13] = [
    "small_cargo",
    "large_cargo",
    "light_fighter",
    "heavy_fighter",
    "cruiser",
    "battleship",
    "battlecruiser",
    "bomber",
    "destroyer",
    "deathstar",
    "recycler",
    "espionage_probe",
    "colony_ship",
];
const DEFENSE_TYPES: [&str; 8] = [
    "rocket_launcher",
    "light_laser",
    "heavy_laser",
    "gauss_cannon",
    "ion_cannon",
    "plasma_turret",
    "small_shield_dome",
    "large_shield_dome",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FleetSourceKind {
    Planet,
    Moon,
}

impl FleetSourceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Planet => "planet",
            Self::Moon => "moon",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetLaunchInput {
    pub user_id: String,
    pub universe_id: i64,
    pub command_id: String,
    pub mission_type: String,
    pub source_kind: FleetSourceKind,
    /// A moon source still records its parent planet as the durable origin.
    pub origin_planet_id: String,
    pub origin_moon_id: Option<String>,
    pub target_kind: String,
    pub target_galaxy: i32,
    pub target_system: i32,
    pub target_position: i32,
    pub acs_group_id: Option<i32>,
    pub ships: BTreeMap<String, i64>,
    pub cargo_metal: i64,
    pub cargo_crystal: i64,
    pub cargo_deuterium: i64,
    pub speed_percent: i32,
    /// Orbit duration for ACS defense only. Every other mission must use zero.
    pub hold_seconds: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FleetWriteError {
    NotFound,
    IdempotencyConflict,
    Restricted,
    VacationMode,
    Invalid(String),
    TargetProtected,
    FleetSlotsExhausted,
    InsufficientShips,
    InsufficientResources,
    Retryable(String),
    Database(String),
}

impl FleetWriteError {
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Retryable(_))
    }
}

impl fmt::Display for FleetWriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => formatter.write_str("fleet source or target was not found"),
            Self::IdempotencyConflict => {
                formatter.write_str("command id was already used with a different launch request")
            }
            Self::Restricted => formatter.write_str("account is restricted from fleet actions"),
            Self::VacationMode => formatter.write_str("vacation mode blocks fleet actions"),
            Self::Invalid(message) | Self::Retryable(message) | Self::Database(message) => {
                formatter.write_str(message)
            }
            Self::TargetProtected => formatter.write_str("target is protected from this mission"),
            Self::FleetSlotsExhausted => formatter.write_str("no fleet slots are available"),
            Self::InsufficientShips => formatter.write_str("source has insufficient ships"),
            Self::InsufficientResources => {
                formatter.write_str("source has insufficient resources or fuel")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetMissionRow {
    pub id: String,
    pub universe_id: i64,
    pub user_id: String,
    pub command_id: String,
    pub mission_type: String,
    pub status: String,
    pub origin_kind: String,
    pub origin_planet_id: String,
    pub origin_moon_id: Option<String>,
    pub origin_galaxy: i32,
    pub origin_system: i32,
    pub origin_position: i32,
    pub target_kind: String,
    pub target_planet_id: Option<String>,
    pub target_moon_id: Option<String>,
    pub target_galaxy: i32,
    pub target_system: i32,
    pub target_position: i32,
    pub acs_group_id: Option<i32>,
    pub departed_at_unix: i64,
    pub arrives_at_unix: i64,
    pub returns_at_unix: i64,
    pub phase_due_at_unix: i64,
    pub distance: i32,
    pub fleet_speed: i64,
    pub duration_seconds: i64,
    pub hold_seconds: i64,
    pub movement_fuel_consumed: i64,
    pub holding_fuel_consumed: i64,
    pub fuel_consumed: i64,
    pub cargo_capacity: i64,
    pub applied_universe_speed: i32,
    pub applied_speed_percent: i32,
    pub applied_fuel_multiplier_milli: i32,
    pub applied_cargo_multiplier_milli: i32,
    pub cargo_metal: i64,
    pub cargo_crystal: i64,
    pub cargo_deuterium: i64,
    pub recalled_at_unix: Option<i64>,
    pub arrival_resolved_at_unix: Option<i64>,
    pub hold_resolved_at_unix: Option<i64>,
    pub return_resolved_at_unix: Option<i64>,
    pub terminal_at_unix: Option<i64>,
    pub result: serde_json::Value,
    pub ships: BTreeMap<String, i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetMissionEventRow {
    pub sequence: i32,
    pub event_key: String,
    pub event_type: String,
    pub phase_generation: i32,
    pub actor_user_id: Option<String>,
    pub payload: serde_json::Value,
    pub occurred_at_unix: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetLaunchResult {
    pub mission: FleetMissionRow,
    pub idempotent_replay: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetMissionClaim {
    pub fleet_id: i32,
    pub universe_id: i64,
    pub user_id: i32,
    pub phase: String,
    pub generation: i32,
    pub claim_attempt: i64,
    pub target_kind: String,
    pub target_planet_id: Option<i32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FleetProcessResult {
    pub arrivals: usize,
    pub returns: usize,
    pub skipped: usize,
    pub failed: usize,
    pub fleet_ids: Vec<String>,
}

#[derive(Debug, Clone)]
struct AccountState {
    user_id: i32,
    universe_id: i64,
    alliance_id: Option<i32>,
    score: i64,
    computer_technology: i32,
    astrophysics: i32,
}

#[derive(Debug, Clone)]
struct LocationState {
    kind: FleetSourceKind,
    planet_id: i32,
    moon_id: Option<i32>,
    owner_id: i32,
    alliance_id: Option<i32>,
    coordinates: Coordinates,
    metal: i64,
    crystal: i64,
    deuterium: i64,
    ships: BTreeMap<String, i64>,
    defenses: BTreeMap<String, i64>,
    moon_diameter: Option<i32>,
    owner_score: i64,
    owner_restricted: bool,
    owner_vacation: bool,
}

#[derive(Debug, Clone)]
struct FleetServerConfig {
    planning: FleetPlanningConfig,
    max_active_per_location: i64,
    noob_protection_enabled: bool,
    noob_protection_points: i64,
    noob_protection_multiplier_milli: i64,
}

#[derive(Debug)]
struct ClaimedMission {
    id: i32,
    universe_id: i64,
    user_id: i32,
    mission_type: FleetMissionType,
    status: String,
    origin_kind: String,
    origin_planet_id: i32,
    origin_moon_id: Option<i32>,
    target_kind: FleetTargetKind,
    target_planet_id: Option<i32>,
    target_moon_id: Option<i32>,
    target_galaxy: i32,
    target_system: i32,
    target_position: i32,
    acs_group_id: Option<i32>,
    duration_seconds: i64,
    hold_seconds: i64,
    cargo_capacity: i64,
    cargo_metal: i64,
    cargo_crystal: i64,
    cargo_deuterium: i64,
    returns_at_unix: i64,
    generation: i32,
    claim_attempt: i64,
    resolution_seed: [u8; 32],
    ships: BTreeMap<String, i64>,
}

#[derive(Debug)]
enum ArrivalDisposition {
    Returning(serde_json::Value),
    Holding(serde_json::Value),
    Completed(serde_json::Value),
    Destroyed(serde_json::Value),
}

impl Database {
    pub async fn fleet_repository_ready(&self) -> DbResult<()> {
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let row = client
            .query_one(
                "SELECT
                    to_regclass('public.fleet_mission_ships') IS NOT NULL
                    AND to_regclass('public.fleet_mission_events') IS NOT NULL
                    AND to_regclass('public.idx_fleets_universe_command') IS NOT NULL
                    AND to_regclass('public.idx_fleets_due_phase') IS NOT NULL
                    AND EXISTS (
                        SELECT 1 FROM information_schema.columns
                        WHERE table_schema = 'public' AND table_name = 'fleets'
                          AND column_name = 'request_fingerprint'
                    ) AS ready",
                &[],
            )
            .await
            .map_err(|error| error.to_string())?;
        if row.get::<_, bool>("ready") {
            Ok(())
        } else {
            Err("durable fleet schema is incomplete; run ordered database migrations".to_string())
        }
    }

    pub async fn launch_fleet(
        &self,
        input: FleetLaunchInput,
    ) -> Result<FleetLaunchResult, FleetWriteError> {
        let normalized = normalize_launch_input(input)?;
        // Planet lazy accrual is materialized before the launch transaction;
        // the transaction then locks and rechecks the exact resulting balance.
        if normalized.source_kind == FleetSourceKind::Planet {
            self.gameplay_planet_for_user(&normalized.user_id, &normalized.origin_planet_id)
                .await
                .map_err(FleetWriteError::Database)?
                .ok_or(FleetWriteError::NotFound)?;
        }
        let fingerprint = launch_request_fingerprint(&normalized);
        let mut client = self
            .pool
            .get()
            .await
            .map_err(|error| FleetWriteError::Database(error.to_string()))?;
        let transaction = client.transaction().await.map_err(map_fleet_db_error)?;
        let account = lock_launch_account(&transaction, &normalized).await?;

        if let Some(existing) = find_idempotent_mission(
            &transaction,
            account.universe_id,
            account.user_id,
            &normalized.command_id,
            &fingerprint,
        )
        .await?
        {
            transaction.commit().await.map_err(map_fleet_db_error)?;
            let mission = self
                .fleet_mission_for_user(
                    &account.user_id.to_string(),
                    account.universe_id,
                    &existing.to_string(),
                )
                .await
                .map_err(FleetWriteError::Database)?
                .ok_or(FleetWriteError::NotFound)?;
            return Ok(FleetLaunchResult {
                mission,
                idempotent_replay: true,
            });
        }

        let config = load_fleet_server_config(&transaction, account.universe_id).await?;
        let source = lock_source_location(&transaction, &normalized, &account).await?;
        let mission_type = normalized
            .mission_type
            .parse::<FleetMissionType>()
            .map_err(FleetWriteError::Invalid)?;
        if matches!(
            mission_type,
            FleetMissionType::AcsAttack | FleetMissionType::AcsDefend | FleetMissionType::AcsJoin
        ) {
            return Err(FleetWriteError::Invalid(
                "ACS launch is disabled until group-atomic combat resolution is available"
                    .to_string(),
            ));
        }
        let target_kind = normalized
            .target_kind
            .parse::<FleetTargetKind>()
            .map_err(FleetWriteError::Invalid)?;
        let target = lock_and_validate_target(
            &transaction,
            &normalized,
            &account,
            mission_type,
            target_kind,
            &config,
        )
        .await?;

        ensure_fleet_slots(&transaction, &account, &source, &config).await?;
        ensure_source_inventory(&source, &normalized.ships)?;
        let composition = FleetComposition::from_map(
            normalized
                .ships
                .iter()
                .map(|(ship_type, count)| (ship_type.clone(), *count))
                .collect::<HashMap<_, _>>(),
        );
        let cargo = Resources::new(
            normalized.cargo_metal,
            normalized.cargo_crystal,
            normalized.cargo_deuterium,
        );
        let mut planning = config.planning;
        planning.speed_percent = normalized.speed_percent;
        planning.hold_seconds = normalized.hold_seconds;
        let plan = plan_authoritative_mission(
            mission_type,
            target_kind,
            &source.coordinates,
            &Coordinates::new(
                normalized.target_galaxy,
                normalized.target_system,
                normalized.target_position,
            ),
            &composition,
            &cargo,
            planning,
        )
        .map_err(|error| FleetWriteError::Invalid(error.0))?;
        ensure_source_resources(&source, &cargo, plan.fuel_required)?;

        deduct_launch_inventory(
            &transaction,
            &source,
            &normalized.ships,
            &cargo,
            plan.fuel_required,
        )
        .await?;

        let departure = transaction
            .query_one(
                "SELECT EXTRACT(EPOCH FROM clock_timestamp())::BIGINT AS now",
                &[],
            )
            .await
            .map_err(map_fleet_db_error)?
            .get::<_, i64>("now");
        let unadjusted_arrival = departure
            .checked_add(plan.travel_time_seconds)
            .ok_or_else(|| FleetWriteError::Invalid("fleet arrival overflow".to_string()))?;
        let (arrival, acs_schedule_generation) = reconcile_acs_rendezvous(
            &transaction,
            &normalized,
            &account,
            mission_type,
            unadjusted_arrival,
        )
        .await?;
        let one_way = mission_type == FleetMissionType::Deploy;
        let return_at = if one_way {
            arrival
        } else {
            arrival
                .checked_add(plan.applied_hold_seconds)
                .ok_or_else(|| FleetWriteError::Invalid("fleet hold overflow".to_string()))?
                .checked_add(plan.travel_time_seconds)
                .ok_or_else(|| FleetWriteError::Invalid("fleet return overflow".to_string()))?
        };
        let mut resolution_seed = [0_u8; 32];
        OsRng.fill_bytes(&mut resolution_seed);
        let ships_json = serde_json::to_value(&normalized.ships)
            .map_err(|error| FleetWriteError::Invalid(error.to_string()))?;
        let target_planet_id = target.as_ref().map(|location| location.planet_id);
        let target_moon_id = target.as_ref().and_then(|location| location.moon_id);

        let inserted = transaction
            .query_one(
                "INSERT INTO fleets
                    (user_id, universe_id, command_id, request_fingerprint, resolution_seed,
                     mission_type, origin_kind, origin_planet_id, origin_moon_id,
                     origin_galaxy, origin_system, origin_position,
                     target_kind, target_planet_id, target_moon_id,
                     target_galaxy, target_system, target_position, acs_group_id,
                     departure_time, arrival_time, return_time,
                     departed_at, unadjusted_arrives_at, arrives_at, returns_at, phase_due_at,
                     distance, fleet_speed, duration_seconds, hold_seconds,
                     movement_fuel_consumed, holding_fuel_consumed, fuel_consumed, cargo_capacity,
                     ships, cargo_metal, cargo_crystal, cargo_deuterium,
                     launched_cargo_metal, launched_cargo_crystal, launched_cargo_deuterium,
                     applied_universe_speed, applied_speed_percent,
                     applied_fuel_multiplier_milli, applied_cargo_multiplier_milli,
                     applied_max_galaxies, applied_max_systems, applied_max_positions,
                     acs_schedule_generation, status, result)
                 VALUES
                    ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12,
                     $13, $14, $15, $16, $17, $18, $19,
                     to_timestamp(($20::BIGINT)::DOUBLE PRECISION) AT TIME ZONE 'UTC',
                     to_timestamp(($22::BIGINT)::DOUBLE PRECISION) AT TIME ZONE 'UTC',
                     to_timestamp(($23::BIGINT)::DOUBLE PRECISION) AT TIME ZONE 'UTC',
                     to_timestamp(($20::BIGINT)::DOUBLE PRECISION),
                     to_timestamp(($21::BIGINT)::DOUBLE PRECISION),
                     to_timestamp(($22::BIGINT)::DOUBLE PRECISION),
                     to_timestamp(($23::BIGINT)::DOUBLE PRECISION),
                     to_timestamp(($22::BIGINT)::DOUBLE PRECISION),
                     $24, $25, $26, $27, $28, $29, $30, $31,
                     $32, $33, $34, $35, $33, $34, $35,
                     $36, $37, $38, $39, $40, $41, $42, $43,
                     'outbound', '{}'::jsonb)
                 RETURNING id",
                &[
                    &account.user_id,
                    &account.universe_id,
                    &normalized.command_id,
                    &&fingerprint[..],
                    &&resolution_seed[..],
                    &mission_type.as_str(),
                    &source.kind.as_str(),
                    &source.planet_id,
                    &source.moon_id,
                    &source.coordinates.galaxy,
                    &source.coordinates.system,
                    &source.coordinates.position,
                    &target_kind.as_str(),
                    &target_planet_id,
                    &target_moon_id,
                    &normalized.target_galaxy,
                    &normalized.target_system,
                    &normalized.target_position,
                    &normalized.acs_group_id,
                    &departure,
                    &unadjusted_arrival,
                    &arrival,
                    &return_at,
                    &plan.distance,
                    &plan.fleet_speed,
                    &plan.travel_time_seconds,
                    &plan.applied_hold_seconds,
                    &plan.movement_fuel_required,
                    &plan.holding_fuel_required,
                    &plan.fuel_required,
                    &plan.cargo_capacity,
                    &Json(&ships_json),
                    &cargo.metal,
                    &cargo.crystal,
                    &cargo.deuterium,
                    &plan.applied_universe_speed,
                    &plan.applied_speed_percent,
                    &plan.applied_fuel_multiplier_milli,
                    &plan.applied_cargo_multiplier_milli,
                    &plan.applied_max_galaxies,
                    &plan.applied_max_systems,
                    &plan.applied_max_positions,
                    &acs_schedule_generation,
                ],
            )
            .await
            .map_err(map_fleet_insert_error)?;
        let fleet_id = inserted.get::<_, i32>("id");
        for (ship_type, count) in &normalized.ships {
            transaction
                .execute(
                    "INSERT INTO fleet_mission_ships
                        (fleet_id, ship_type, initial_count, current_count)
                     VALUES ($1, $2, $3, $3)",
                    &[&fleet_id, ship_type, count],
                )
                .await
                .map_err(map_fleet_db_error)?;
        }
        append_fleet_event(
            &transaction,
            account.universe_id,
            fleet_id,
            "launch:dispatched",
            "dispatched",
            0,
            Some(account.user_id),
            serde_json::json!({
                "missionType": mission_type.as_str(),
                "targetKind": target_kind.as_str(),
                "target": {
                    "galaxy": normalized.target_galaxy,
                    "system": normalized.target_system,
                    "position": normalized.target_position
                },
                "distance": plan.distance,
                "durationSeconds": plan.travel_time_seconds,
                "holdSeconds": plan.applied_hold_seconds,
                "movementFuelConsumed": plan.movement_fuel_required,
                "holdingFuelConsumed": plan.holding_fuel_required,
                "fuelConsumed": plan.fuel_required,
                "cargoCapacity": plan.cargo_capacity
            }),
        )
        .await?;
        transaction
            .execute(
                "INSERT INTO player_activity_log
                    (user_id, activity_type, activity_data, planet_id)
                 VALUES ($1, 'fleet_dispatch', $2, $3)",
                &[
                    &account.user_id,
                    &Json(&serde_json::json!({
                        "fleetId": fleet_id,
                        "missionType": mission_type.as_str(),
                        "commandId": normalized.command_id
                    })),
                    &source.planet_id,
                ],
            )
            .await
            .map_err(map_fleet_db_error)?;
        if is_hostile_mission(mission_type) {
            if let Some(target) = &target {
                insert_notification(
                    &transaction,
                    target.owner_id,
                    "under_attack",
                    "Incoming hostile fleet",
                    "A hostile fleet is approaching your position.",
                    5,
                    fleet_id,
                )
                .await?;
            }
        }
        if mission_type == FleetMissionType::AcsJoin {
            let assigned = transaction
                .execute(
                    "UPDATE acs_group_members
                     SET fleet_id = $4
                     WHERE universe_id = $1 AND group_id = $2 AND user_id = $3
                       AND fleet_id IS NULL",
                    &[
                        &account.universe_id,
                        &normalized.acs_group_id,
                        &account.user_id,
                        &fleet_id,
                    ],
                )
                .await
                .map_err(map_fleet_db_error)?;
            if assigned != 1 {
                return Err(FleetWriteError::Invalid(
                    "ACS membership launch slot is no longer available".to_string(),
                ));
            }
        }
        transaction.commit().await.map_err(map_fleet_db_error)?;

        let mission = self
            .fleet_mission_for_user(
                &account.user_id.to_string(),
                account.universe_id,
                &fleet_id.to_string(),
            )
            .await
            .map_err(FleetWriteError::Database)?
            .ok_or(FleetWriteError::NotFound)?;
        Ok(FleetLaunchResult {
            mission,
            idempotent_replay: false,
        })
    }

    pub async fn fleet_missions_for_user(
        &self,
        user_id: &str,
        universe_id: i64,
    ) -> DbResult<Vec<FleetMissionRow>> {
        let Some(user_id) = parse_optional_i32(user_id) else {
            return Ok(Vec::new());
        };
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let rows = client
            .query(
                &format!(
                    "{} WHERE universe_id = $1 AND user_id = $2
                     ORDER BY departed_at DESC, id DESC",
                    fleet_select_sql()
                ),
                &[&universe_id, &user_id],
            )
            .await
            .map_err(|error| error.to_string())?;
        let mut missions = Vec::with_capacity(rows.len());
        for row in &rows {
            missions.push(map_fleet_row(&client, row).await?);
        }
        Ok(missions)
    }

    pub async fn fleet_mission_for_user(
        &self,
        user_id: &str,
        universe_id: i64,
        fleet_id: &str,
    ) -> DbResult<Option<FleetMissionRow>> {
        let (Some(user_id), Some(fleet_id)) =
            (parse_optional_i32(user_id), parse_optional_i32(fleet_id))
        else {
            return Ok(None);
        };
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let row = client
            .query_opt(
                &format!(
                    "{} WHERE universe_id = $1 AND user_id = $2 AND id = $3",
                    fleet_select_sql()
                ),
                &[&universe_id, &user_id, &fleet_id],
            )
            .await
            .map_err(|error| error.to_string())?;
        match row {
            Some(row) => map_fleet_row(&client, &row).await.map(Some),
            None => Ok(None),
        }
    }

    pub async fn fleet_mission_events_for_user(
        &self,
        user_id: &str,
        universe_id: i64,
        fleet_id: &str,
    ) -> DbResult<Vec<FleetMissionEventRow>> {
        let (Some(user_id), Some(fleet_id)) =
            (parse_optional_i32(user_id), parse_optional_i32(fleet_id))
        else {
            return Ok(Vec::new());
        };
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let rows = client
            .query(
                "SELECT event.sequence, event.event_key, event.event_type,
                        event.phase_generation,
                        event.actor_user_id::TEXT AS actor_user_id,
                        event.payload,
                        EXTRACT(EPOCH FROM event.occurred_at)::BIGINT AS occurred_at_unix
                 FROM fleet_mission_events AS event
                 JOIN fleets AS fleet
                   ON fleet.universe_id = event.universe_id AND fleet.id = event.fleet_id
                 WHERE event.universe_id = $1 AND event.fleet_id = $2 AND fleet.user_id = $3
                 ORDER BY event.sequence",
                &[&universe_id, &fleet_id, &user_id],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(rows
            .iter()
            .map(|row| FleetMissionEventRow {
                sequence: row.get("sequence"),
                event_key: row.get("event_key"),
                event_type: row.get("event_type"),
                phase_generation: row.get("phase_generation"),
                actor_user_id: row.get("actor_user_id"),
                payload: row.get("payload"),
                occurred_at_unix: row.get("occurred_at_unix"),
            })
            .collect())
    }

    pub async fn recall_fleet(
        &self,
        user_id: &str,
        universe_id: i64,
        fleet_id: &str,
    ) -> Result<FleetMissionRow, FleetWriteError> {
        let user_id = parse_i32(user_id)?;
        let fleet_id = parse_i32(fleet_id)?;
        let mut client = self
            .pool
            .get()
            .await
            .map_err(|error| FleetWriteError::Database(error.to_string()))?;
        let transaction = client.transaction().await.map_err(map_fleet_db_error)?;
        let row = transaction
            .query_opt(
                "SELECT id, status,
                        EXTRACT(EPOCH FROM departed_at)::BIGINT AS departed_at_unix,
                        EXTRACT(EPOCH FROM arrives_at)::BIGINT AS arrives_at_unix,
                        EXTRACT(EPOCH FROM clock_timestamp())::BIGINT AS now_unix,
                        phase_generation
                 FROM fleets
                 WHERE universe_id = $1 AND user_id = $2 AND id = $3
                 FOR UPDATE",
                &[&universe_id, &user_id, &fleet_id],
            )
            .await
            .map_err(map_fleet_db_error)?
            .ok_or(FleetWriteError::NotFound)?;
        if row.get::<_, String>("status") != "outbound" {
            return Err(FleetWriteError::Invalid(
                "only an outbound fleet can be recalled".to_string(),
            ));
        }
        let departed_at = row.get::<_, i64>("departed_at_unix");
        let arrives_at = row.get::<_, i64>("arrives_at_unix");
        let now = row.get::<_, i64>("now_unix");
        let duration = recall_return_duration_seconds(departed_at, arrives_at, now)
            .map_err(|error| FleetWriteError::Invalid(error.0))?;
        let return_at = now
            .checked_add(duration)
            .ok_or_else(|| FleetWriteError::Invalid("fleet return overflow".to_string()))?;
        let generation = row.get::<_, i32>("phase_generation") + 1;
        transaction
            .execute(
                "UPDATE fleets
                 SET status = 'returning',
                     recalled_at = to_timestamp(($4::BIGINT)::DOUBLE PRECISION),
                     returns_at = to_timestamp(($5::BIGINT)::DOUBLE PRECISION),
                     phase_due_at = to_timestamp(($5::BIGINT)::DOUBLE PRECISION),
                     return_time = to_timestamp(($5::BIGINT)::DOUBLE PRECISION)
                                   AT TIME ZONE 'UTC',
                     phase_generation = $6,
                     resolution_owner = NULL, resolution_expires_at = NULL
                 WHERE universe_id = $1 AND user_id = $2 AND id = $3 AND status = 'outbound'",
                &[
                    &universe_id,
                    &user_id,
                    &fleet_id,
                    &now,
                    &return_at,
                    &generation,
                ],
            )
            .await
            .map_err(map_fleet_db_error)?;
        append_fleet_event(
            &transaction,
            universe_id,
            fleet_id,
            &format!("recall:{generation}"),
            "recalled",
            generation,
            Some(user_id),
            serde_json::json!({"recalledAt": now, "returnsAt": return_at}),
        )
        .await?;
        transaction.commit().await.map_err(map_fleet_db_error)?;
        self.fleet_mission_for_user(&user_id.to_string(), universe_id, &fleet_id.to_string())
            .await
            .map_err(FleetWriteError::Database)?
            .ok_or(FleetWriteError::NotFound)
    }

    pub async fn claim_due_fleet_missions(
        &self,
        worker_id: &str,
        limit: usize,
        lease_seconds: i64,
    ) -> DbResult<Vec<FleetMissionClaim>> {
        let worker_id = worker_id.trim();
        if worker_id.is_empty() || worker_id.len() > 160 {
            return Err("fleet worker id is outside supported bounds".to_string());
        }
        if limit == 0 || limit > MAX_FLEET_PROCESS_BATCH {
            return Err("fleet process limit is outside supported bounds".to_string());
        }
        if !(1..=MAX_FLEET_LEASE_SECONDS).contains(&lease_seconds) {
            return Err("fleet lease duration is outside supported bounds".to_string());
        }
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let rows = client
            .query(
                "WITH due AS (
                    SELECT id
                    FROM fleets
                    WHERE status IN ('outbound', 'holding', 'returning')
                      AND phase_due_at <= clock_timestamp()
                      AND (resolution_expires_at IS NULL
                           OR resolution_expires_at <= clock_timestamp())
                    ORDER BY phase_due_at, id
                    FOR UPDATE SKIP LOCKED
                    LIMIT $1
                 )
                 UPDATE fleets AS fleet
                 SET resolution_owner = $2,
                     resolution_expires_at = clock_timestamp() +
                                             ($3::BIGINT * INTERVAL '1 second'),
                     claim_attempt = fleet.claim_attempt + 1
                 FROM due
                 WHERE fleet.id = due.id
                 RETURNING fleet.id, fleet.universe_id, fleet.user_id,
                           fleet.status, fleet.phase_generation, fleet.claim_attempt,
                           fleet.target_kind,
                           fleet.target_planet_id",
                &[&(limit as i64), &worker_id, &lease_seconds],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(rows
            .iter()
            .map(|row| FleetMissionClaim {
                fleet_id: row.get("id"),
                universe_id: row.get("universe_id"),
                user_id: row.get("user_id"),
                phase: row.get("status"),
                generation: row.get("phase_generation"),
                claim_attempt: row.get("claim_attempt"),
                target_kind: row.get("target_kind"),
                target_planet_id: row.get("target_planet_id"),
            })
            .collect())
    }

    pub async fn process_due_fleet_missions(
        &self,
        worker_id: &str,
        limit: usize,
        lease_seconds: i64,
    ) -> DbResult<FleetProcessResult> {
        let claims = self
            .claim_due_fleet_missions(worker_id, limit, lease_seconds)
            .await?;
        let mut result = FleetProcessResult::default();
        for claim in claims {
            // Bring a planet target's lazy resource balance current before the
            // resolving transaction takes its combat/transfer lock.
            if claim.phase == "outbound" && claim.target_kind == "planet" {
                if let Some(target_planet_id) = claim.target_planet_id {
                    let owner = match self
                        .fleet_target_planet_owner(claim.universe_id, target_planet_id)
                        .await
                    {
                        Ok(Some(owner)) => owner,
                        Ok(None) => {
                            result.failed += 1;
                            let _ = self
                                .release_fleet_claim(
                                    worker_id,
                                    &claim,
                                    "target planet disappeared before resource accrual",
                                )
                                .await;
                            continue;
                        }
                        Err(error) => {
                            result.failed += 1;
                            let _ = self
                                .release_fleet_claim(
                                    worker_id,
                                    &claim,
                                    &format!("target owner lookup failed: {error}"),
                                )
                                .await;
                            continue;
                        }
                    };
                    if let Err(error) = self
                        .gameplay_planet_for_user(&owner.to_string(), &target_planet_id.to_string())
                        .await
                    {
                        result.failed += 1;
                        let _ = self
                            .release_fleet_claim(
                                worker_id,
                                &claim,
                                &format!("target resource accrual failed: {error}"),
                            )
                            .await;
                        continue;
                    }
                }
            }
            match self.process_claimed_fleet_mission(worker_id, &claim).await {
                Ok(Some(phase)) => {
                    if phase == "arrival" {
                        result.arrivals += 1;
                    } else {
                        result.returns += 1;
                    }
                    result.fleet_ids.push(claim.fleet_id.to_string());
                }
                Ok(None) => result.skipped += 1,
                Err(error) => {
                    result.failed += 1;
                    let _ = self.release_fleet_claim(worker_id, &claim, &error).await;
                }
            }
        }
        Ok(result)
    }

    async fn fleet_target_planet_owner(
        &self,
        universe_id: i64,
        planet_id: i32,
    ) -> DbResult<Option<i32>> {
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        client
            .query_opt(
                "SELECT user_id FROM planets WHERE universe_id = $1 AND id = $2",
                &[&universe_id, &planet_id],
            )
            .await
            .map(|row| row.map(|row| row.get("user_id")))
            .map_err(|error| error.to_string())
    }

    /// Resolve one previously leased mission phase. A stale, expired, or
    /// reclaimed lease is a harmless no-op, which lets workers restart safely.
    pub async fn process_claimed_fleet_mission(
        &self,
        worker_id: &str,
        claim: &FleetMissionClaim,
    ) -> DbResult<Option<&'static str>> {
        let mut client = self.pool.get().await.map_err(|error| error.to_string())?;
        let transaction = client
            .transaction()
            .await
            .map_err(|error| error.to_string())?;
        let Some(row) = transaction
            .query_opt(
                "SELECT id, universe_id, user_id, mission_type, status,
                        origin_kind, origin_planet_id, origin_moon_id,
                        target_kind, target_planet_id, target_moon_id,
                        target_galaxy, target_system, target_position, acs_group_id,
                        duration_seconds, hold_seconds, cargo_capacity,
                        cargo_metal, cargo_crystal, cargo_deuterium,
                        EXTRACT(EPOCH FROM returns_at)::BIGINT AS returns_at_unix,
                        phase_generation, claim_attempt, resolution_seed
                 FROM fleets
                 WHERE id = $1 AND universe_id = $2 AND user_id = $3
                   AND status = $4 AND phase_generation = $5
                   AND claim_attempt = $6 AND resolution_owner = $7
                   AND resolution_expires_at > clock_timestamp()
                   AND phase_due_at <= clock_timestamp()
                 FOR UPDATE",
                &[
                    &claim.fleet_id,
                    &claim.universe_id,
                    &claim.user_id,
                    &claim.phase,
                    &claim.generation,
                    &claim.claim_attempt,
                    &worker_id,
                ],
            )
            .await
            .map_err(|error| error.to_string())?
        else {
            transaction
                .commit()
                .await
                .map_err(|error| error.to_string())?;
            return Ok(None);
        };
        let seed = row.get::<_, Vec<u8>>("resolution_seed");
        let resolution_seed: [u8; 32] = seed
            .try_into()
            .map_err(|_| "fleet resolution seed is not exactly 32 bytes".to_string())?;
        let ship_rows = transaction
            .query(
                "SELECT ship_type, current_count FROM fleet_mission_ships
                 WHERE fleet_id = $1 ORDER BY ship_type FOR UPDATE",
                &[&claim.fleet_id],
            )
            .await
            .map_err(|error| error.to_string())?;
        let mission = ClaimedMission {
            id: row.get("id"),
            universe_id: row.get("universe_id"),
            user_id: row.get("user_id"),
            mission_type: row
                .get::<_, String>("mission_type")
                .parse()
                .map_err(|error: String| error)?,
            status: row.get("status"),
            origin_kind: row.get("origin_kind"),
            origin_planet_id: row.get("origin_planet_id"),
            origin_moon_id: row.get("origin_moon_id"),
            target_kind: row
                .get::<_, String>("target_kind")
                .parse()
                .map_err(|error: String| error)?,
            target_planet_id: row.get("target_planet_id"),
            target_moon_id: row.get("target_moon_id"),
            target_galaxy: row.get("target_galaxy"),
            target_system: row.get("target_system"),
            target_position: row.get("target_position"),
            acs_group_id: row.get("acs_group_id"),
            duration_seconds: row.get("duration_seconds"),
            hold_seconds: row.get("hold_seconds"),
            cargo_capacity: row.get("cargo_capacity"),
            cargo_metal: row.get("cargo_metal"),
            cargo_crystal: row.get("cargo_crystal"),
            cargo_deuterium: row.get("cargo_deuterium"),
            returns_at_unix: row.get("returns_at_unix"),
            generation: row.get("phase_generation"),
            claim_attempt: row.get("claim_attempt"),
            resolution_seed,
            ships: ship_rows
                .iter()
                .map(|ship| (ship.get("ship_type"), ship.get("current_count")))
                .collect(),
        };

        let processed_phase = match mission.status.as_str() {
            "outbound" => {
                let disposition = resolve_fleet_arrival(&transaction, &mission).await?;
                persist_arrival_disposition(&transaction, &mission, worker_id, disposition).await?;
                "arrival"
            }
            "holding" => {
                let changed = transaction
                    .execute(
                        "UPDATE fleets
                         SET status = 'returning', hold_resolved_at = clock_timestamp(),
                             phase_due_at = returns_at,
                             phase_generation = phase_generation + 1,
                             resolution_owner = NULL, resolution_expires_at = NULL
                         WHERE id = $1 AND universe_id = $2 AND status = 'holding'
                           AND phase_generation = $3 AND claim_attempt = $4
                           AND resolution_owner = $5
                           AND resolution_expires_at > clock_timestamp()",
                        &[
                            &mission.id,
                            &mission.universe_id,
                            &mission.generation,
                            &mission.claim_attempt,
                            &worker_id,
                        ],
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                if changed != 1 {
                    return Err("fleet hold exact-once transition was lost".to_string());
                }
                append_fleet_event(
                    &transaction,
                    mission.universe_id,
                    mission.id,
                    &format!("hold:{}", mission.generation),
                    "hold_completed",
                    mission.generation,
                    None,
                    serde_json::json!({"holdSeconds": mission.hold_seconds}),
                )
                .await
                .map_err(|error| error.to_string())?;
                "arrival"
            }
            "returning" => {
                restore_returning_fleet(&transaction, &mission, worker_id).await?;
                "return"
            }
            _ => return Err("claimed fleet has a terminal or unknown phase".to_string()),
        };
        transaction
            .commit()
            .await
            .map_err(|error| error.to_string())?;
        Ok(Some(processed_phase))
    }

    async fn release_fleet_claim(
        &self,
        worker_id: &str,
        claim: &FleetMissionClaim,
        error: &str,
    ) -> DbResult<()> {
        let mut client = self
            .pool
            .get()
            .await
            .map_err(|pool_error| pool_error.to_string())?;
        let transaction = client
            .transaction()
            .await
            .map_err(|db_error| db_error.to_string())?;
        let reason_code = fleet_resolution_reason_code(error);
        let released = transaction
            .execute(
                "UPDATE fleets
                 SET resolution_owner = NULL, resolution_expires_at = NULL,
                     result = result || jsonb_build_object('lastResolutionReasonCode', $8::TEXT)
                 WHERE id = $1 AND universe_id = $2 AND user_id = $3
                   AND status = $4 AND phase_generation = $5 AND claim_attempt = $6
                   AND resolution_owner = $7",
                &[
                    &claim.fleet_id,
                    &claim.universe_id,
                    &claim.user_id,
                    &claim.phase,
                    &claim.generation,
                    &claim.claim_attempt,
                    &worker_id,
                    &reason_code,
                ],
            )
            .await
            .map_err(|db_error| db_error.to_string())?;
        if released == 1 {
            append_fleet_event(
                &transaction,
                claim.universe_id,
                claim.fleet_id,
                &format!(
                    "resolution_failed:{}:{}",
                    claim.generation, claim.claim_attempt
                ),
                "resolution_failed",
                claim.generation,
                None,
                serde_json::json!({"reasonCode": reason_code}),
            )
            .await
            .map_err(|event_error| event_error.to_string())?;
        }
        transaction
            .commit()
            .await
            .map_err(|db_error| db_error.to_string())
    }
}

fn fleet_resolution_reason_code(error: &str) -> &'static str {
    if error.contains("resource accrual") {
        "target_accrual_failed"
    } else if error.contains("lease") || error.contains("exact-once") {
        "lease_lost"
    } else if error.contains("target") || error.contains("location") {
        "target_unavailable"
    } else if error.contains("seed") || error.contains("persisted") {
        "persisted_contract_invalid"
    } else {
        "resolution_failed"
    }
}

async fn resolve_fleet_arrival(
    transaction: &Transaction<'_>,
    mission: &ClaimedMission,
) -> DbResult<ArrivalDisposition> {
    match mission.mission_type {
        FleetMissionType::Transport => {
            transfer_cargo_to_target(transaction, mission).await?;
            Ok(ArrivalDisposition::Returning(serde_json::json!({
                "mission": "transport",
                "delivered": {
                    "metal": mission.cargo_metal,
                    "crystal": mission.cargo_crystal,
                    "deuterium": mission.cargo_deuterium
                }
            })))
        }
        FleetMissionType::Deploy => {
            transfer_fleet_to_target(transaction, mission).await?;
            Ok(ArrivalDisposition::Completed(serde_json::json!({
                "mission": "deploy",
                "deployed": true
            })))
        }
        FleetMissionType::Espionage => resolve_espionage(transaction, mission).await,
        FleetMissionType::Colonize => resolve_colonization(transaction, mission).await,
        FleetMissionType::Harvest => resolve_harvest(transaction, mission).await,
        FleetMissionType::Expedition => resolve_expedition(transaction, mission).await,
        FleetMissionType::Attack | FleetMissionType::AcsAttack | FleetMissionType::AcsJoin => {
            resolve_combat_arrival(transaction, mission, false).await
        }
        FleetMissionType::Destroy => resolve_combat_arrival(transaction, mission, true).await,
        FleetMissionType::AcsDefend => Ok(ArrivalDisposition::Holding(serde_json::json!({
            "mission": "acs_defend",
            "holdSeconds": mission.hold_seconds,
            "orbitEndsAt": mission.returns_at_unix - mission.duration_seconds
        }))),
    }
}

async fn persist_arrival_disposition(
    transaction: &Transaction<'_>,
    mission: &ClaimedMission,
    worker_id: &str,
    disposition: ArrivalDisposition,
) -> DbResult<()> {
    let (status, event_type, result) = match disposition {
        ArrivalDisposition::Returning(result) => ("returning", "arrival_resolved", result),
        ArrivalDisposition::Holding(result) => ("holding", "hold_started", result),
        ArrivalDisposition::Completed(result) => ("completed", "mission_completed", result),
        ArrivalDisposition::Destroyed(result) => ("destroyed", "fleet_destroyed", result),
    };
    let changed = match status {
        "returning" => {
            transaction
                .execute(
                    "UPDATE fleets
                     SET status = 'returning', arrival_resolved_at = clock_timestamp(),
                         phase_due_at = returns_at, phase_generation = phase_generation + 1,
                         result = $6,
                         resolution_owner = NULL, resolution_expires_at = NULL
                     WHERE id = $1 AND universe_id = $2 AND status = 'outbound'
                       AND phase_generation = $3 AND claim_attempt = $4
                       AND resolution_owner = $5
                       AND resolution_expires_at > clock_timestamp()
                       AND arrival_resolved_at IS NULL",
                    &[
                        &mission.id,
                        &mission.universe_id,
                        &mission.generation,
                        &mission.claim_attempt,
                        &worker_id,
                        &Json(&result),
                    ],
                )
                .await
        }
        "holding" => {
            transaction
                .execute(
                    "UPDATE fleets
                     SET status = 'holding', arrival_resolved_at = clock_timestamp(),
                         phase_due_at = returns_at -
                             make_interval(secs => duration_seconds::DOUBLE PRECISION),
                         phase_generation = phase_generation + 1, result = $6,
                         resolution_owner = NULL, resolution_expires_at = NULL
                     WHERE id = $1 AND universe_id = $2 AND status = 'outbound'
                       AND phase_generation = $3 AND claim_attempt = $4
                       AND resolution_owner = $5
                       AND resolution_expires_at > clock_timestamp()
                       AND arrival_resolved_at IS NULL",
                    &[
                        &mission.id,
                        &mission.universe_id,
                        &mission.generation,
                        &mission.claim_attempt,
                        &worker_id,
                        &Json(&result),
                    ],
                )
                .await
        }
        "completed" => {
            transaction
                .execute(
                    "UPDATE fleets
                         SET status = 'completed', arrival_resolved_at = clock_timestamp(),
                         return_resolved_at = clock_timestamp(), terminal_at = clock_timestamp(),
                         phase_generation = phase_generation + 1, result = $6,
                         resolution_owner = NULL, resolution_expires_at = NULL
                     WHERE id = $1 AND universe_id = $2 AND status = 'outbound'
                       AND phase_generation = $3 AND claim_attempt = $4
                       AND resolution_owner = $5
                       AND resolution_expires_at > clock_timestamp()
                       AND arrival_resolved_at IS NULL",
                    &[
                        &mission.id,
                        &mission.universe_id,
                        &mission.generation,
                        &mission.claim_attempt,
                        &worker_id,
                        &Json(&result),
                    ],
                )
                .await
        }
        _ => {
            transaction
                .execute(
                    "UPDATE fleets
                     SET status = 'destroyed', arrival_resolved_at = clock_timestamp(),
                         terminal_at = clock_timestamp(), cargo_metal = 0,
                         cargo_crystal = 0, cargo_deuterium = 0, result = $4,
                         phase_generation = phase_generation + 1,
                         resolution_owner = NULL, resolution_expires_at = NULL
                     WHERE id = $1 AND universe_id = $2 AND status = 'outbound'
                       AND phase_generation = $3 AND claim_attempt = $5
                       AND resolution_owner = $6
                       AND resolution_expires_at > clock_timestamp()
                       AND arrival_resolved_at IS NULL",
                    &[
                        &mission.id,
                        &mission.universe_id,
                        &mission.generation,
                        &Json(&result),
                        &mission.claim_attempt,
                        &worker_id,
                    ],
                )
                .await
        }
    }
    .map_err(|error| error.to_string())?;
    if changed != 1 {
        return Err("fleet arrival exact-once transition was lost".to_string());
    }
    append_fleet_event(
        transaction,
        mission.universe_id,
        mission.id,
        &format!("arrival:{}", mission.generation),
        event_type,
        mission.generation,
        None,
        result,
    )
    .await
    .map_err(|error| error.to_string())
}

async fn restore_returning_fleet(
    transaction: &Transaction<'_>,
    mission: &ClaimedMission,
    worker_id: &str,
) -> DbResult<()> {
    let (table, location_id) = mission_origin_table_and_id(mission)?;
    add_resources(
        transaction,
        table,
        location_id,
        mission.cargo_metal,
        mission.cargo_crystal,
        mission.cargo_deuterium,
    )
    .await?;
    add_ships(transaction, table, location_id, &mission.ships).await?;
    transaction
        .execute(
            "UPDATE fleet_mission_ships SET current_count = 0 WHERE fleet_id = $1",
            &[&mission.id],
        )
        .await
        .map_err(|error| error.to_string())?;
    let changed = transaction
        .execute(
            "UPDATE fleets
             SET status = 'completed', return_resolved_at = clock_timestamp(),
                 terminal_at = clock_timestamp(), cargo_metal = 0, cargo_crystal = 0,
                 cargo_deuterium = 0, resolution_owner = NULL, resolution_expires_at = NULL,
                 phase_generation = phase_generation + 1,
                 result = result || jsonb_build_object('returned', TRUE)
             WHERE id = $1 AND universe_id = $2 AND status = 'returning'
               AND phase_generation = $3 AND claim_attempt = $4
               AND resolution_owner = $5
               AND resolution_expires_at > clock_timestamp()
               AND return_resolved_at IS NULL",
            &[
                &mission.id,
                &mission.universe_id,
                &mission.generation,
                &mission.claim_attempt,
                &worker_id,
            ],
        )
        .await
        .map_err(|error| error.to_string())?;
    if changed != 1 {
        return Err("fleet return exact-once transition was lost".to_string());
    }
    append_fleet_event(
        transaction,
        mission.universe_id,
        mission.id,
        &format!("return:{}", mission.generation),
        "returned",
        mission.generation,
        None,
        serde_json::json!({"originKind": mission.origin_kind}),
    )
    .await
    .map_err(|error| error.to_string())
}

fn mission_origin_table_and_id(mission: &ClaimedMission) -> DbResult<(&'static str, i32)> {
    if mission.origin_kind == "planet" {
        Ok(("planets", mission.origin_planet_id))
    } else if mission.origin_kind == "moon" {
        mission
            .origin_moon_id
            .map(|id| ("moons", id))
            .ok_or_else(|| "moon-origin fleet lost its moon identity".to_string())
    } else {
        Err("fleet origin kind is invalid".to_string())
    }
}

fn mission_target_table_and_id(mission: &ClaimedMission) -> DbResult<(&'static str, i32)> {
    match mission.target_kind {
        FleetTargetKind::Planet => mission
            .target_planet_id
            .map(|id| ("planets", id))
            .ok_or_else(|| "planet target identity is missing".to_string()),
        FleetTargetKind::Moon => mission
            .target_moon_id
            .map(|id| ("moons", id))
            .ok_or_else(|| "moon target identity is missing".to_string()),
        _ => Err("mission does not target a resource-bearing location".to_string()),
    }
}

async fn add_resources(
    transaction: &Transaction<'_>,
    table: &str,
    location_id: i32,
    metal: i64,
    crystal: i64,
    deuterium: i64,
) -> DbResult<()> {
    let updated = transaction
        .execute(
            &format!(
                "UPDATE {table} SET metal = metal + $2, crystal = crystal + $3,
                                    deuterium = deuterium + $4 WHERE id = $1"
            ),
            &[&location_id, &metal, &crystal, &deuterium],
        )
        .await
        .map_err(|error| error.to_string())?;
    if updated == 1 {
        Ok(())
    } else {
        Err("fleet location disappeared during resolution".to_string())
    }
}

async fn add_ships(
    transaction: &Transaction<'_>,
    table: &str,
    location_id: i32,
    ships: &BTreeMap<String, i64>,
) -> DbResult<()> {
    for (ship_type, count) in ships {
        if !SHIP_TYPES.contains(&ship_type.as_str()) || *count < 0 {
            return Err("fleet contains an unsupported persisted ship".to_string());
        }
        let updated = transaction
            .execute(
                &format!("UPDATE {table} SET {ship_type} = {ship_type} + $2 WHERE id = $1"),
                &[&location_id, count],
            )
            .await
            .map_err(|error| error.to_string())?;
        if updated != 1 {
            return Err("fleet destination disappeared during ship transfer".to_string());
        }
    }
    Ok(())
}

async fn transfer_cargo_to_target(
    transaction: &Transaction<'_>,
    mission: &ClaimedMission,
) -> DbResult<()> {
    let (table, location_id) = mission_target_table_and_id(mission)?;
    add_resources(
        transaction,
        table,
        location_id,
        mission.cargo_metal,
        mission.cargo_crystal,
        mission.cargo_deuterium,
    )
    .await?;
    transaction
        .execute(
            "UPDATE fleets SET cargo_metal = 0, cargo_crystal = 0, cargo_deuterium = 0
             WHERE id = $1",
            &[&mission.id],
        )
        .await
        .map_err(|error| error.to_string())?;
    Ok(())
}

async fn transfer_fleet_to_target(
    transaction: &Transaction<'_>,
    mission: &ClaimedMission,
) -> DbResult<()> {
    let (table, location_id) = mission_target_table_and_id(mission)?;
    add_resources(
        transaction,
        table,
        location_id,
        mission.cargo_metal,
        mission.cargo_crystal,
        mission.cargo_deuterium,
    )
    .await?;
    add_ships(transaction, table, location_id, &mission.ships).await?;
    transaction
        .execute(
            "UPDATE fleet_mission_ships SET current_count = 0 WHERE fleet_id = $1",
            &[&mission.id],
        )
        .await
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            "UPDATE fleets SET cargo_metal = 0, cargo_crystal = 0, cargo_deuterium = 0
             WHERE id = $1",
            &[&mission.id],
        )
        .await
        .map_err(|error| error.to_string())?;
    Ok(())
}

async fn lock_resolution_target(
    transaction: &Transaction<'_>,
    mission: &ClaimedMission,
) -> DbResult<LocationState> {
    let coordinates = Coordinates::new(
        mission.target_galaxy,
        mission.target_system,
        mission.target_position,
    );
    match mission.target_kind {
        FleetTargetKind::Planet => {
            lock_target_planet(transaction, mission.universe_id, &coordinates)
                .await
                .map_err(|error| error.to_string())
        }
        FleetTargetKind::Moon => lock_target_moon(transaction, mission.universe_id, &coordinates)
            .await
            .map_err(|error| error.to_string()),
        _ => Err("mission target is not a planet or moon".to_string()),
    }
}

async fn resolve_espionage(
    transaction: &Transaction<'_>,
    mission: &ClaimedMission,
) -> DbResult<ArrivalDisposition> {
    let target = lock_resolution_target(transaction, mission).await?;
    let probes = mission.ships.get("espionage_probe").copied().unwrap_or(0);
    let detail = if probes >= 5 {
        "resources_ships_defenses"
    } else if probes >= 2 {
        "resources_ships"
    } else {
        "resources"
    };
    let ships = (probes >= 2).then_some(&target.ships);
    let defenses = (probes >= 5).then_some(&target.defenses);
    Ok(ArrivalDisposition::Returning(serde_json::json!({
        "mission": "espionage",
        "detail": detail,
        "targetOwnerId": target.owner_id,
        "resources": {
            "metal": target.metal,
            "crystal": target.crystal,
            "deuterium": target.deuterium
        },
        "ships": ships,
        "defenses": defenses
    })))
}

async fn resolve_colonization(
    transaction: &Transaction<'_>,
    mission: &ClaimedMission,
) -> DbResult<ArrivalDisposition> {
    let created = transaction
        .query_opt(
            "INSERT INTO planets
                (user_id, name, galaxy, system, position, universe_id,
                 metal, crystal, deuterium)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
             ON CONFLICT (universe_id, galaxy, system, position) DO NOTHING
             RETURNING id",
            &[
                &mission.user_id,
                &format!(
                    "Colony {}:{}:{}",
                    mission.target_galaxy, mission.target_system, mission.target_position
                ),
                &mission.target_galaxy,
                &mission.target_system,
                &mission.target_position,
                &mission.universe_id,
                &mission.cargo_metal,
                &mission.cargo_crystal,
                &mission.cargo_deuterium,
            ],
        )
        .await
        .map_err(|error| error.to_string())?;
    let Some(created) = created else {
        return Ok(ArrivalDisposition::Returning(serde_json::json!({
            "mission": "colonize",
            "colonized": false,
            "reason": "coordinate_occupied_before_arrival"
        })));
    };
    let colony_ship = mission.ships.get("colony_ship").copied().unwrap_or(0);
    if colony_ship < 1 {
        return Err("colonization fleet has no colony ship at resolution".to_string());
    }
    transaction
        .execute(
            "UPDATE fleet_mission_ships SET current_count = current_count - 1
             WHERE fleet_id = $1 AND ship_type = 'colony_ship' AND current_count >= 1",
            &[&mission.id],
        )
        .await
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            "UPDATE fleets SET cargo_metal = 0, cargo_crystal = 0, cargo_deuterium = 0
             WHERE id = $1",
            &[&mission.id],
        )
        .await
        .map_err(|error| error.to_string())?;
    Ok(ArrivalDisposition::Returning(serde_json::json!({
        "mission": "colonize",
        "colonized": true,
        "planetId": created.get::<_, i32>("id")
    })))
}

async fn resolve_harvest(
    transaction: &Transaction<'_>,
    mission: &ClaimedMission,
) -> DbResult<ArrivalDisposition> {
    let row = transaction
        .query_opt(
            "SELECT id, metal, crystal, deuterium FROM debris_fields
             WHERE universe_id = $1 AND galaxy = $2 AND system = $3 AND position = $4
             FOR UPDATE",
            &[
                &mission.universe_id,
                &mission.target_galaxy,
                &mission.target_system,
                &mission.target_position,
            ],
        )
        .await
        .map_err(|error| error.to_string())?;
    let Some(row) = row else {
        return Ok(ArrivalDisposition::Returning(serde_json::json!({
            "mission": "harvest", "collected": {"metal": 0, "crystal": 0, "deuterium": 0}
        })));
    };
    let occupied = mission
        .cargo_metal
        .checked_add(mission.cargo_crystal)
        .and_then(|value| value.checked_add(mission.cargo_deuterium))
        .ok_or_else(|| "fleet cargo overflow".to_string())?;
    let mut free = mission.cargo_capacity.saturating_sub(occupied);
    let take_metal = row.get::<_, i64>("metal").min(free).max(0);
    free -= take_metal;
    let take_crystal = row.get::<_, i64>("crystal").min(free).max(0);
    free -= take_crystal;
    let take_deuterium = row.get::<_, i64>("deuterium").min(free).max(0);
    transaction
        .execute(
            "UPDATE debris_fields
             SET metal = metal - $2, crystal = crystal - $3,
                 deuterium = deuterium - $4, updated_at = clock_timestamp()
             WHERE id = $1",
            &[
                &row.get::<_, i32>("id"),
                &take_metal,
                &take_crystal,
                &take_deuterium,
            ],
        )
        .await
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            "UPDATE fleets SET cargo_metal = cargo_metal + $2,
                               cargo_crystal = cargo_crystal + $3,
                               cargo_deuterium = cargo_deuterium + $4
             WHERE id = $1",
            &[&mission.id, &take_metal, &take_crystal, &take_deuterium],
        )
        .await
        .map_err(|error| error.to_string())?;
    Ok(ArrivalDisposition::Returning(serde_json::json!({
        "mission": "harvest",
        "collected": {"metal": take_metal, "crystal": take_crystal, "deuterium": take_deuterium}
    })))
}

async fn resolve_expedition(
    transaction: &Transaction<'_>,
    mission: &ClaimedMission,
) -> DbResult<ArrivalDisposition> {
    let seed = derive_phase_seed(mission, b"expedition");
    let roll = seed[0] % 100;
    let mut result = serde_json::json!({"mission": "expedition", "outcome": "empty"});
    if (45..75).contains(&roll) {
        let occupied = mission.cargo_metal + mission.cargo_crystal + mission.cargo_deuterium;
        let free = mission.cargo_capacity.saturating_sub(occupied);
        let found = (i64::from(u16::from_le_bytes([seed[1], seed[2]])) * 10)
            .min(free)
            .max(0);
        transaction
            .execute(
                "UPDATE fleets SET cargo_metal = cargo_metal + $2 WHERE id = $1",
                &[&mission.id, &found],
            )
            .await
            .map_err(|error| error.to_string())?;
        result =
            serde_json::json!({"mission": "expedition", "outcome": "resources", "metal": found});
    } else if (75..90).contains(&roll) {
        let dark_matter = i32::from(seed[1]).max(1);
        transaction
            .execute(
                "UPDATE users SET dark_matter = dark_matter + $2
                 WHERE universe_id = $1 AND id = $3",
                &[&mission.universe_id, &dark_matter, &mission.user_id],
            )
            .await
            .map_err(|error| error.to_string())?;
        result = serde_json::json!({"mission": "expedition", "outcome": "dark_matter", "amount": dark_matter});
    } else if roll >= 90 {
        if let Some((ship_type, count)) = mission.ships.iter().find(|(_, count)| **count > 0) {
            let lost = (*count / 4).max(1);
            transaction
                .execute(
                    "UPDATE fleet_mission_ships SET current_count = current_count - $3
                     WHERE fleet_id = $1 AND ship_type = $2 AND current_count >= $3",
                    &[&mission.id, ship_type, &lost],
                )
                .await
                .map_err(|error| error.to_string())?;
            result = serde_json::json!({"mission": "expedition", "outcome": "damage", "shipType": ship_type, "lost": lost});
        }
    }
    Ok(ArrivalDisposition::Returning(result))
}

fn derive_phase_seed(mission: &ClaimedMission, domain: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"universus-fleet-resolution-v1\0");
    fingerprint_field(&mut hasher, &mission.resolution_seed);
    fingerprint_field(&mut hasher, &mission.id.to_be_bytes());
    fingerprint_field(&mut hasher, &mission.generation.to_be_bytes());
    fingerprint_field(&mut hasher, domain);
    hasher.finalize().into()
}

async fn resolve_combat_arrival(
    transaction: &Transaction<'_>,
    mission: &ClaimedMission,
    destroy_moon: bool,
) -> DbResult<ArrivalDisposition> {
    let target = lock_resolution_target(transaction, mission).await?;
    let attacker_ships = counts_as_i32(&mission.ships)?;
    let defender_ships = counts_as_i32(&target.ships)?;
    let defender_defenses = counts_as_i32(&target.defenses)?;
    let attacker_tech = load_combat_tech(transaction, mission.user_id).await?;
    let defender_tech = load_combat_tech(transaction, target.owner_id).await?;
    let input = CombatInput {
        attacker_ships: attacker_ships.clone(),
        defender_ships: defender_ships.clone(),
        defender_defenses: defender_defenses.clone(),
        attacker_tech,
        defender_tech,
        planet_metal: target.metal,
        planet_crystal: target.crystal,
        planet_deuterium: target.deuterium,
        // The authoritative entry point ignores this compatibility field; the
        // full seed remains out-of-band and is never serialized.
        seed: String::new(),
        universe: mission.universe_id.to_string(),
        max_rounds: None,
    };
    let phase_seed = derive_phase_seed(mission, b"combat");
    let mut report =
        generate_combat_report_with_seed_256(&input, &CombatConfig::default(), &phase_seed);
    let occupied = mission
        .cargo_metal
        .checked_add(mission.cargo_crystal)
        .and_then(|value| value.checked_add(mission.cargo_deuterium))
        .ok_or_else(|| "fleet cargo overflow".to_string())?;
    let mut free = mission.cargo_capacity.saturating_sub(occupied);
    report.result.loot.metal = report.result.loot.metal.min(free).max(0);
    free -= report.result.loot.metal;
    report.result.loot.crystal = report.result.loot.crystal.min(free).max(0);
    free -= report.result.loot.crystal;
    report.result.loot.deuterium = report.result.loot.deuterium.min(free).max(0);

    let attacker_survivors = survivor_counts(&attacker_ships, &report.result.attacker_losses);
    let defender_survivors = survivor_counts(&defender_ships, &report.result.defender_losses);
    let mut defense_survivors = survivor_counts(&defender_defenses, &report.result.defender_losses);
    for (defense, rebuilt) in &report.defense_rebuilt {
        *defense_survivors.entry(defense.clone()).or_insert(0) += i64::from(*rebuilt);
    }
    set_fleet_counts(transaction, mission.id, &attacker_survivors).await?;
    set_location_counts(
        transaction,
        &target,
        &defender_survivors,
        &defense_survivors,
    )
    .await?;

    let remaining_metal = target.metal.saturating_sub(report.result.loot.metal);
    let remaining_crystal = target.crystal.saturating_sub(report.result.loot.crystal);
    let remaining_deuterium = target
        .deuterium
        .saturating_sub(report.result.loot.deuterium);
    set_location_resources(
        transaction,
        &target,
        remaining_metal,
        remaining_crystal,
        remaining_deuterium,
    )
    .await?;
    transaction
        .execute(
            "UPDATE fleets
             SET cargo_metal = cargo_metal + $2,
                 cargo_crystal = cargo_crystal + $3,
                 cargo_deuterium = cargo_deuterium + $4
             WHERE id = $1",
            &[
                &mission.id,
                &report.result.loot.metal,
                &report.result.loot.crystal,
                &report.result.loot.deuterium,
            ],
        )
        .await
        .map_err(|error| error.to_string())?;

    let (debris_metal, debris_crystal) = canonical_combat_debris(
        &report.result.attacker_losses,
        &report.result.defender_losses,
    );
    report.result.debris.metal = debris_metal;
    report.result.debris.crystal = debris_crystal;
    transaction
        .execute(
            "INSERT INTO debris_fields
                (universe_id, galaxy, system, position, metal, crystal, deuterium)
             VALUES ($1, $2, $3, $4, $5, $6, 0)
             ON CONFLICT (universe_id, galaxy, system, position)
             DO UPDATE SET metal = debris_fields.metal + EXCLUDED.metal,
                           crystal = debris_fields.crystal + EXCLUDED.crystal,
                           updated_at = clock_timestamp()",
            &[
                &mission.universe_id,
                &mission.target_galaxy,
                &mission.target_system,
                &mission.target_position,
                &debris_metal,
                &debris_crystal,
            ],
        )
        .await
        .map_err(|error| error.to_string())?;
    let report_json = serde_json::to_value(&report).map_err(|error| error.to_string())?;
    let combat_report_id = transaction
        .query_one(
            "INSERT INTO combat_reports
                (attacker_id, defender_id, planet_galaxy, planet_system, planet_position,
                 rounds, winner, attacker_losses, defender_losses,
                 loot_metal, loot_crystal, loot_deuterium, debris_metal, debris_crystal,
                 universe_id, fleet_id, target_kind, target_planet_id, target_moon_id)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9,
                     $10, $11, $12, $13, $14, $15, $16, $17, $18, $19)
             RETURNING id",
            &[
                &mission.user_id,
                &target.owner_id,
                &mission.target_galaxy,
                &mission.target_system,
                &mission.target_position,
                &Json(&report.rounds),
                &report.result.winner,
                &Json(&report.result.attacker_losses),
                &Json(&report.result.defender_losses),
                &report.result.loot.metal,
                &report.result.loot.crystal,
                &report.result.loot.deuterium,
                &debris_metal,
                &debris_crystal,
                &mission.universe_id,
                &mission.id,
                &mission.target_kind.as_str(),
                &mission.target_planet_id,
                &mission.target_moon_id,
            ],
        )
        .await
        .map_err(|error| error.to_string())?
        .get::<_, i32>("id");
    insert_notification(
        transaction,
        target.owner_id,
        "fleet_arrived",
        "Combat resolved",
        "A combat at your position has been resolved.",
        4,
        mission.id,
    )
    .await
    .map_err(|error| error.to_string())?;

    let mut fleet_destroyed = attacker_survivors.values().all(|count| *count <= 0);
    let mut moon_result = None;
    if destroy_moon && !fleet_destroyed {
        let moon_id = mission
            .target_moon_id
            .ok_or_else(|| "moon-destruction mission lost its target moon".to_string())?;
        let rip_count = attacker_survivors.get("deathstar").copied().unwrap_or(0);
        let rip_count = i32::try_from(rip_count)
            .map_err(|_| "deathstar survivor count exceeds moon-destruction bounds".to_string())?;
        let moon_seed = u64::from_le_bytes(
            derive_phase_seed(mission, b"moon-destruction")[..8]
                .try_into()
                .map_err(|_| "moon seed derivation failed".to_string())?,
        );
        let outcome = calculate_moon_destruction(
            &MoonDestructionInput {
                attacker_id: i64::from(mission.user_id),
                defender_id: i64::from(target.owner_id),
                moon_id: i64::from(moon_id),
                rip_count,
                moon_diameter: target.moon_diameter.unwrap_or(0),
            },
            moon_seed,
        );
        if outcome.moon_destroyed {
            mark_moon_destroyed(transaction, moon_id).await?;
        }
        if outcome.fleet_destroyed {
            fleet_destroyed = true;
            transaction
                .execute(
                    "UPDATE fleet_mission_ships SET current_count = 0 WHERE fleet_id = $1",
                    &[&mission.id],
                )
                .await
                .map_err(|error| error.to_string())?;
        }
        moon_result = Some(outcome);
    }
    if let Some(group_id) = mission.acs_group_id {
        transaction
            .execute(
                "UPDATE acs_groups SET status = 'arrived'
                 WHERE universe_id = $1 AND id = $2 AND status IN ('forming', 'launched')",
                &[&mission.universe_id, &group_id],
            )
            .await
            .map_err(|error| error.to_string())?;
    }
    let result = serde_json::json!({
        "mission": mission.mission_type.as_str(),
        "combatReportId": combat_report_id,
        "combat": report_json,
        "moonDestruction": moon_result
    });
    if fleet_destroyed {
        Ok(ArrivalDisposition::Destroyed(result))
    } else {
        Ok(ArrivalDisposition::Returning(result))
    }
}

fn counts_as_i32(counts: &BTreeMap<String, i64>) -> DbResult<HashMap<String, i32>> {
    counts
        .iter()
        .map(|(key, count)| {
            i32::try_from(*count)
                .map(|count| (key.clone(), count))
                .map_err(|_| format!("persisted unit count for {key} exceeds combat bounds"))
        })
        .collect()
}

async fn load_combat_tech(
    transaction: &Transaction<'_>,
    user_id: i32,
) -> DbResult<HashMap<String, i32>> {
    let row = transaction
        .query_opt(
            "SELECT weapons_technology, shielding_technology, armor_technology
             FROM research WHERE user_id = $1",
            &[&user_id],
        )
        .await
        .map_err(|error| error.to_string())?;
    let mut tech = HashMap::new();
    if let Some(row) = row {
        tech.insert(
            "weapons_technology".to_string(),
            row.get("weapons_technology"),
        );
        tech.insert(
            "shielding_technology".to_string(),
            row.get("shielding_technology"),
        );
        tech.insert("armor_technology".to_string(), row.get("armor_technology"));
    }
    Ok(tech)
}

fn survivor_counts(
    initial: &HashMap<String, i32>,
    losses: &HashMap<String, i32>,
) -> BTreeMap<String, i64> {
    initial
        .iter()
        .map(|(kind, count)| {
            (
                kind.clone(),
                i64::from((*count - losses.get(kind).copied().unwrap_or(0)).max(0)),
            )
        })
        .collect()
}

async fn set_fleet_counts(
    transaction: &Transaction<'_>,
    fleet_id: i32,
    counts: &BTreeMap<String, i64>,
) -> DbResult<()> {
    for (ship_type, count) in counts {
        transaction
            .execute(
                "UPDATE fleet_mission_ships SET current_count = $3
                 WHERE fleet_id = $1 AND ship_type = $2 AND current_count >= $3",
                &[&fleet_id, ship_type, count],
            )
            .await
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

async fn set_location_counts(
    transaction: &Transaction<'_>,
    target: &LocationState,
    ships: &BTreeMap<String, i64>,
    defenses: &BTreeMap<String, i64>,
) -> DbResult<()> {
    let (table, location_id) = location_table_and_id(target).map_err(|error| error.to_string())?;
    for (kind, count) in ships.iter().chain(defenses.iter()) {
        if !SHIP_TYPES.contains(&kind.as_str()) && !DEFENSE_TYPES.contains(&kind.as_str()) {
            return Err("combat returned an unsupported target unit".to_string());
        }
        transaction
            .execute(
                &format!("UPDATE {table} SET {kind} = $2 WHERE id = $1"),
                &[&location_id, count],
            )
            .await
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

async fn set_location_resources(
    transaction: &Transaction<'_>,
    target: &LocationState,
    metal: i64,
    crystal: i64,
    deuterium: i64,
) -> DbResult<()> {
    let (table, location_id) = location_table_and_id(target).map_err(|error| error.to_string())?;
    transaction
        .execute(
            &format!("UPDATE {table} SET metal = $2, crystal = $3, deuterium = $4 WHERE id = $1"),
            &[&location_id, &metal, &crystal, &deuterium],
        )
        .await
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn canonical_combat_debris(
    attacker_losses: &HashMap<String, i32>,
    defender_losses: &HashMap<String, i32>,
) -> (i64, i64) {
    attacker_losses.iter().chain(defender_losses.iter()).fold(
        (0_i64, 0_i64),
        |(metal, crystal), (kind, count)| {
            let (unit_metal, unit_crystal) = canonical_unit_cost(kind);
            let count = i64::from((*count).max(0));
            (
                metal.saturating_add(unit_metal.saturating_mul(count) * 3 / 10),
                crystal.saturating_add(unit_crystal.saturating_mul(count) * 3 / 10),
            )
        },
    )
}

fn canonical_unit_cost(kind: &str) -> (i64, i64) {
    match kind {
        "small_cargo" => (2_000, 2_000),
        "large_cargo" => (6_000, 6_000),
        "light_fighter" => (3_000, 1_000),
        "heavy_fighter" => (6_000, 4_000),
        "cruiser" => (20_000, 7_000),
        "battleship" => (45_000, 15_000),
        "battlecruiser" => (30_000, 40_000),
        "bomber" => (50_000, 25_000),
        "destroyer" => (60_000, 50_000),
        "deathstar" => (5_000_000, 4_000_000),
        "recycler" => (10_000, 6_000),
        "espionage_probe" => (0, 1_000),
        "colony_ship" => (10_000, 20_000),
        "rocket_launcher" => (2_000, 0),
        "light_laser" => (1_500, 500),
        "heavy_laser" => (6_000, 2_000),
        "gauss_cannon" => (20_000, 15_000),
        "ion_cannon" => (2_000, 6_000),
        "plasma_turret" => (50_000, 50_000),
        "small_shield_dome" => (10_000, 10_000),
        "large_shield_dome" => (50_000, 50_000),
        _ => (0, 0),
    }
}

async fn mark_moon_destroyed(transaction: &Transaction<'_>, moon_id: i32) -> DbResult<()> {
    let zero_units = SHIP_TYPES
        .iter()
        .chain(DEFENSE_TYPES.iter())
        .map(|unit| format!("{unit} = 0"))
        .collect::<Vec<_>>()
        .join(", ");
    transaction
        .execute(
            &format!(
                "UPDATE moons SET destroyed_at = clock_timestamp(), metal = 0, crystal = 0,
                                  deuterium = 0, {zero_units}
                 WHERE id = $1 AND destroyed_at IS NULL"
            ),
            &[&moon_id],
        )
        .await
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn normalize_launch_input(
    mut input: FleetLaunchInput,
) -> Result<FleetLaunchInput, FleetWriteError> {
    input.user_id = input.user_id.trim().to_string();
    input.origin_planet_id = input.origin_planet_id.trim().to_string();
    input.origin_moon_id = input
        .origin_moon_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    input.command_id = input.command_id.trim().to_string();
    input.mission_type = input.mission_type.trim().to_ascii_lowercase();
    input.target_kind = input.target_kind.trim().to_ascii_lowercase();
    if input.command_id.is_empty() || input.command_id.len() > MAX_COMMAND_ID_BYTES {
        return Err(FleetWriteError::Invalid(
            "command id is outside supported bounds".to_string(),
        ));
    }
    if input.universe_id <= 0 {
        return Err(FleetWriteError::Invalid(
            "universe id must be positive".to_string(),
        ));
    }
    if input.source_kind == FleetSourceKind::Moon && input.origin_moon_id.is_none() {
        return Err(FleetWriteError::Invalid(
            "moon source requires origin moon id".to_string(),
        ));
    }
    if input.source_kind == FleetSourceKind::Planet && input.origin_moon_id.is_some() {
        return Err(FleetWriteError::Invalid(
            "planet source cannot include origin moon id".to_string(),
        ));
    }
    if input.ships.is_empty() {
        return Err(FleetWriteError::Invalid(
            "fleet requires at least one ship type".to_string(),
        ));
    }
    let mut normalized_ships = BTreeMap::new();
    for (ship_type, count) in input.ships {
        let ship_type = ship_type.trim().to_ascii_lowercase();
        if !SHIP_TYPES.contains(&ship_type.as_str()) {
            return Err(FleetWriteError::Invalid(format!(
                "unsupported fleet ship type: {ship_type}"
            )));
        }
        if normalized_ships.insert(ship_type.clone(), count).is_some() {
            return Err(FleetWriteError::Invalid(format!(
                "duplicate fleet ship type: {ship_type}"
            )));
        }
    }
    input.ships = normalized_ships;
    Ok(input)
}

fn launch_request_fingerprint(input: &FleetLaunchInput) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"universus-fleet-launch-v1\0");
    fingerprint_field(&mut hasher, input.user_id.as_bytes());
    fingerprint_field(&mut hasher, &input.universe_id.to_be_bytes());
    fingerprint_field(&mut hasher, input.command_id.as_bytes());
    fingerprint_field(&mut hasher, input.mission_type.as_bytes());
    fingerprint_field(&mut hasher, input.source_kind.as_str().as_bytes());
    fingerprint_field(&mut hasher, input.origin_planet_id.as_bytes());
    fingerprint_optional_field(&mut hasher, input.origin_moon_id.as_deref());
    fingerprint_field(&mut hasher, input.target_kind.as_bytes());
    fingerprint_field(&mut hasher, &input.target_galaxy.to_be_bytes());
    fingerprint_field(&mut hasher, &input.target_system.to_be_bytes());
    fingerprint_field(&mut hasher, &input.target_position.to_be_bytes());
    match input.acs_group_id {
        Some(value) => {
            hasher.update([1]);
            fingerprint_field(&mut hasher, &value.to_be_bytes());
        }
        None => hasher.update([0]),
    }
    fingerprint_field(&mut hasher, &input.cargo_metal.to_be_bytes());
    fingerprint_field(&mut hasher, &input.cargo_crystal.to_be_bytes());
    fingerprint_field(&mut hasher, &input.cargo_deuterium.to_be_bytes());
    fingerprint_field(&mut hasher, &input.speed_percent.to_be_bytes());
    fingerprint_field(&mut hasher, &input.hold_seconds.to_be_bytes());
    for (ship_type, count) in &input.ships {
        fingerprint_field(&mut hasher, ship_type.as_bytes());
        fingerprint_field(&mut hasher, &count.to_be_bytes());
    }
    hasher.finalize().into()
}

fn fingerprint_field(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}

fn fingerprint_optional_field(hasher: &mut Sha256, value: Option<&str>) {
    match value {
        Some(value) => {
            hasher.update([1]);
            fingerprint_field(hasher, value.as_bytes());
        }
        None => hasher.update([0]),
    }
}

async fn lock_launch_account(
    transaction: &Transaction<'_>,
    input: &FleetLaunchInput,
) -> Result<AccountState, FleetWriteError> {
    let user_id = parse_i32(&input.user_id)?;
    let row = transaction
        .query_opt(
            "SELECT users.id, users.universe_id, users.alliance_id,
                    COALESCE(scores.total_score, 0)::BIGINT AS score,
                    COALESCE(research.computer_technology, 0) AS computer_technology,
                    COALESCE(research.astrophysics, 0) AS astrophysics,
                    users.is_banned,
                    users.vacation_mode,
                    users.privacy_restriction_active,
                    users.privacy_erasure_pending,
                    COALESCE(users.account_status, 'active') AS account_status,
                    COALESCE(users.is_locked, FALSE) AS is_locked,
                    EXISTS (
                        SELECT 1 FROM account_suspensions AS suspension
                        WHERE suspension.user_id = users.id
                          AND suspension.is_active
                          AND suspension.lifted_at IS NULL
                          AND (suspension.expires_at IS NULL OR suspension.expires_at > now())
                    ) AS suspended
             FROM users
             LEFT JOIN player_scores AS scores ON scores.user_id = users.id
             LEFT JOIN research ON research.user_id = users.id
             WHERE users.id = $1 AND users.universe_id = $2
             FOR UPDATE OF users",
            &[&user_id, &input.universe_id],
        )
        .await
        .map_err(map_fleet_db_error)?
        .ok_or(FleetWriteError::NotFound)?;
    if row.get::<_, bool>("vacation_mode") {
        return Err(FleetWriteError::VacationMode);
    }
    if row.get::<_, bool>("is_banned")
        || row.get::<_, bool>("privacy_restriction_active")
        || row.get::<_, bool>("privacy_erasure_pending")
        || row.get::<_, bool>("is_locked")
        || row.get::<_, bool>("suspended")
        || row.get::<_, String>("account_status") != "active"
    {
        return Err(FleetWriteError::Restricted);
    }
    Ok(AccountState {
        user_id,
        universe_id: row.get("universe_id"),
        alliance_id: row.get("alliance_id"),
        score: row.get("score"),
        computer_technology: row.get("computer_technology"),
        astrophysics: row.get("astrophysics"),
    })
}

async fn find_idempotent_mission(
    transaction: &Transaction<'_>,
    universe_id: i64,
    user_id: i32,
    command_id: &str,
    fingerprint: &[u8; 32],
) -> Result<Option<i32>, FleetWriteError> {
    let row = transaction
        .query_opt(
            "SELECT id, request_fingerprint
             FROM fleets
             WHERE universe_id = $1 AND user_id = $2 AND command_id = $3",
            &[&universe_id, &user_id, &command_id],
        )
        .await
        .map_err(map_fleet_db_error)?;
    let Some(row) = row else {
        return Ok(None);
    };
    if row.get::<_, Vec<u8>>("request_fingerprint").as_slice() != fingerprint {
        return Err(FleetWriteError::IdempotencyConflict);
    }
    Ok(Some(row.get("id")))
}

async fn load_fleet_server_config(
    transaction: &Transaction<'_>,
    universe_id: i64,
) -> Result<FleetServerConfig, FleetWriteError> {
    let universe_speed = transaction
        .query_opt(
            "SELECT speed FROM universes WHERE id = $1 FOR SHARE",
            &[&universe_id],
        )
        .await
        .map_err(map_fleet_db_error)?
        .ok_or(FleetWriteError::NotFound)?
        .get::<_, i32>("speed");
    let rows = transaction
        .query(
            "SELECT parameter_key, current_value
             FROM config_parameters
             WHERE parameter_key = ANY($1)",
            &[&&[
                "fleet.fuel_consumption_multiplier",
                "fleet.cargo_multiplier",
                "fleet.max_active_per_planet",
                "universe.max_galaxies",
                "universe.max_systems",
                "universe.max_planets",
                "combat.noob_protection_enabled",
                "combat.noob_protection_points",
                "combat.noob_protection_multiplier",
            ][..]],
        )
        .await
        .map_err(map_fleet_db_error)?;
    let values = rows
        .iter()
        .map(|row| {
            (
                row.get::<_, String>("parameter_key"),
                row.get::<_, String>("current_value"),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let int = |key: &str, fallback: i64| -> Result<i64, FleetWriteError> {
        values
            .get(key)
            .map(|value| {
                value.parse::<i64>().map_err(|_| {
                    FleetWriteError::Invalid(format!("invalid persisted fleet config: {key}"))
                })
            })
            .transpose()
            .map(|value| value.unwrap_or(fallback))
    };
    let milli = |key: &str, fallback: i64| -> Result<i64, FleetWriteError> {
        values
            .get(key)
            .map(|value| parse_positive_decimal_milli(value, key))
            .transpose()
            .map(|value| value.unwrap_or(fallback))
    };
    let bool_value = |key: &str, fallback: bool| -> Result<bool, FleetWriteError> {
        values
            .get(key)
            .map(|value| match value.trim().to_ascii_lowercase().as_str() {
                "true" | "1" => Ok(true),
                "false" | "0" => Ok(false),
                _ => Err(FleetWriteError::Invalid(format!(
                    "invalid persisted fleet config: {key}"
                ))),
            })
            .transpose()
            .map(|value| value.unwrap_or(fallback))
    };
    let fuel_multiplier_milli = milli("fleet.fuel_consumption_multiplier", 1_000)?;
    let cargo_multiplier_milli = milli("fleet.cargo_multiplier", 1_000)?;
    let noob_multiplier = milli("combat.noob_protection_multiplier", 5_000)?;
    Ok(FleetServerConfig {
        planning: FleetPlanningConfig {
            universe_speed,
            speed_percent: 100,
            fuel_multiplier_milli: i32::try_from(fuel_multiplier_milli).map_err(|_| {
                FleetWriteError::Invalid("fleet fuel multiplier is too large".to_string())
            })?,
            cargo_multiplier_milli: i32::try_from(cargo_multiplier_milli).map_err(|_| {
                FleetWriteError::Invalid("fleet cargo multiplier is too large".to_string())
            })?,
            max_galaxies: i32::try_from(int("universe.max_galaxies", 9)?).map_err(|_| {
                FleetWriteError::Invalid("universe galaxy bound is too large".to_string())
            })?,
            max_systems: i32::try_from(int("universe.max_systems", 499)?).map_err(|_| {
                FleetWriteError::Invalid("universe system bound is too large".to_string())
            })?,
            max_positions: i32::try_from(int("universe.max_planets", 15)?).map_err(|_| {
                FleetWriteError::Invalid("universe position bound is too large".to_string())
            })?,
            hold_seconds: 0,
        },
        max_active_per_location: int("fleet.max_active_per_planet", DEFAULT_PLANET_FLEET_SLOTS)?,
        noob_protection_enabled: bool_value("combat.noob_protection_enabled", true)?,
        noob_protection_points: int("combat.noob_protection_points", 5_000)?,
        noob_protection_multiplier_milli: noob_multiplier,
    })
}

fn parse_positive_decimal_milli(value: &str, key: &str) -> Result<i64, FleetWriteError> {
    let value = value.trim();
    let mut parts = value.split('.');
    let whole = parts
        .next()
        .and_then(|part| part.parse::<i64>().ok())
        .ok_or_else(|| {
            FleetWriteError::Invalid(format!("invalid persisted fleet config: {key}"))
        })?;
    let fraction = parts.next().unwrap_or("");
    if parts.next().is_some()
        || whole < 0
        || fraction.len() > 3
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(FleetWriteError::Invalid(format!(
            "invalid persisted fleet config: {key}"
        )));
    }
    let padded = format!("{fraction:0<3}");
    whole
        .checked_mul(1_000)
        .and_then(|result| {
            padded
                .parse::<i64>()
                .ok()
                .and_then(|part| result.checked_add(part))
        })
        .filter(|result| *result > 0)
        .ok_or_else(|| FleetWriteError::Invalid(format!("invalid persisted fleet config: {key}")))
}

async fn lock_source_location(
    transaction: &Transaction<'_>,
    input: &FleetLaunchInput,
    account: &AccountState,
) -> Result<LocationState, FleetWriteError> {
    let planet_id = parse_i32(&input.origin_planet_id)?;
    match input.source_kind {
        FleetSourceKind::Planet => {
            let row = transaction
                .query_opt(
                    &format!(
                        "SELECT p.id AS planet_id, NULL::INTEGER AS moon_id, p.user_id,
                                p.universe_id, u.alliance_id, p.galaxy, p.system, p.position,
                                COALESCE(p.metal, 0)::BIGINT AS metal,
                                COALESCE(p.crystal, 0)::BIGINT AS crystal,
                                COALESCE(p.deuterium, 0)::BIGINT AS deuterium,
                                NULL::INTEGER AS moon_diameter,
                                COALESCE(score.total_score, 0)::BIGINT AS owner_score,
                                FALSE AS owner_restricted, FALSE AS owner_vacation,
                                {}, {}
                         FROM planets AS p
                         JOIN users AS u ON u.id = p.user_id AND u.universe_id = p.universe_id
                         LEFT JOIN player_scores AS score ON score.user_id = u.id
                         WHERE p.id = $1 AND p.user_id = $2 AND p.universe_id = $3
                         FOR UPDATE OF p",
                        location_ship_select("p"),
                        location_defense_select("p")
                    ),
                    &[&planet_id, &account.user_id, &account.universe_id],
                )
                .await
                .map_err(map_fleet_db_error)?
                .ok_or(FleetWriteError::NotFound)?;
            Ok(map_location_row(FleetSourceKind::Planet, &row))
        }
        FleetSourceKind::Moon => {
            let moon_id = parse_i32(input.origin_moon_id.as_deref().unwrap_or_default())?;
            let row = transaction
                .query_opt(
                    &format!(
                        "SELECT p.id AS planet_id, m.id AS moon_id, m.user_id,
                                m.universe_id, u.alliance_id, p.galaxy, p.system, p.position,
                                m.metal, m.crystal, m.deuterium, m.diameter AS moon_diameter,
                                COALESCE(score.total_score, 0)::BIGINT AS owner_score,
                                FALSE AS owner_restricted, FALSE AS owner_vacation,
                                {}, {}
                         FROM moons AS m
                         JOIN planets AS p
                           ON p.universe_id = m.universe_id AND p.id = m.planet_id
                         JOIN users AS u ON u.id = m.user_id AND u.universe_id = m.universe_id
                         LEFT JOIN player_scores AS score ON score.user_id = u.id
                         WHERE m.id = $1 AND m.planet_id = $2
                           AND m.user_id = $3 AND m.universe_id = $4
                           AND m.destroyed_at IS NULL
                         FOR UPDATE OF m",
                        location_ship_select("m"),
                        location_defense_select("m")
                    ),
                    &[&moon_id, &planet_id, &account.user_id, &account.universe_id],
                )
                .await
                .map_err(map_fleet_db_error)?
                .ok_or(FleetWriteError::NotFound)?;
            Ok(map_location_row(FleetSourceKind::Moon, &row))
        }
    }
}

async fn lock_and_validate_target(
    transaction: &Transaction<'_>,
    input: &FleetLaunchInput,
    account: &AccountState,
    mission_type: FleetMissionType,
    target_kind: FleetTargetKind,
    config: &FleetServerConfig,
) -> Result<Option<LocationState>, FleetWriteError> {
    if matches!(
        mission_type,
        FleetMissionType::AcsAttack | FleetMissionType::AcsDefend | FleetMissionType::AcsJoin
    ) {
        validate_acs_launch(transaction, input, account, mission_type, target_kind).await?;
    } else if input.acs_group_id.is_some() {
        return Err(FleetWriteError::Invalid(
            "ACS group is only valid for ACS missions".to_string(),
        ));
    }
    let coordinates = Coordinates::new(
        input.target_galaxy,
        input.target_system,
        input.target_position,
    );
    let location = match target_kind {
        FleetTargetKind::Planet => {
            Some(lock_target_planet(transaction, account.universe_id, &coordinates).await?)
        }
        FleetTargetKind::Moon => {
            Some(lock_target_moon(transaction, account.universe_id, &coordinates).await?)
        }
        FleetTargetKind::Debris => {
            let exists = transaction
                .query_opt(
                    "SELECT id FROM debris_fields
                     WHERE universe_id = $1 AND galaxy = $2 AND system = $3 AND position = $4
                     FOR UPDATE",
                    &[
                        &account.universe_id,
                        &coordinates.galaxy,
                        &coordinates.system,
                        &coordinates.position,
                    ],
                )
                .await
                .map_err(map_fleet_db_error)?;
            if exists.is_none() {
                return Err(FleetWriteError::NotFound);
            }
            None
        }
        FleetTargetKind::EmptyCoordinate => {
            let occupied = transaction
                .query_opt(
                    "SELECT id FROM planets
                     WHERE universe_id = $1 AND galaxy = $2 AND system = $3 AND position = $4
                     FOR SHARE",
                    &[
                        &account.universe_id,
                        &coordinates.galaxy,
                        &coordinates.system,
                        &coordinates.position,
                    ],
                )
                .await
                .map_err(map_fleet_db_error)?;
            if occupied.is_some() {
                return Err(FleetWriteError::Invalid(
                    "colonization target is occupied".to_string(),
                ));
            }
            let planet_count = transaction
                .query_one(
                    "SELECT COUNT(*)::BIGINT AS count FROM planets
                     WHERE universe_id = $1 AND user_id = $2",
                    &[&account.universe_id, &account.user_id],
                )
                .await
                .map_err(map_fleet_db_error)?
                .get::<_, i64>("count");
            let maximum_planets = 1_i64 + i64::from((account.astrophysics + 1) / 2);
            if planet_count >= maximum_planets {
                return Err(FleetWriteError::Invalid(
                    "astrophysics level does not allow another colony".to_string(),
                ));
            }
            None
        }
        FleetTargetKind::ExpeditionSlot => None,
    };

    if let Some(target) = &location {
        if target.owner_restricted || target.owner_vacation {
            return Err(FleetWriteError::TargetProtected);
        }
        if is_hostile_mission(mission_type) {
            if target.owner_id == account.user_id
                || (account.alliance_id.is_some() && account.alliance_id == target.alliance_id)
            {
                return Err(FleetWriteError::Invalid(
                    "hostile mission cannot target self or an ally".to_string(),
                ));
            }
            if is_score_protected(account.score, target.owner_score, config) {
                return Err(FleetWriteError::TargetProtected);
            }
        }
        if matches!(mission_type, FleetMissionType::Deploy) && target.owner_id != account.user_id {
            return Err(FleetWriteError::Invalid(
                "deploy missions require a self-owned destination".to_string(),
            ));
        }
        if matches!(mission_type, FleetMissionType::AcsDefend)
            && target.owner_id != account.user_id
            && (account.alliance_id.is_none() || account.alliance_id != target.alliance_id)
        {
            return Err(FleetWriteError::Invalid(
                "ACS defend destination must belong to self or an ally".to_string(),
            ));
        }
    }
    Ok(location)
}

async fn validate_acs_launch(
    transaction: &Transaction<'_>,
    input: &FleetLaunchInput,
    account: &AccountState,
    mission_type: FleetMissionType,
    target_kind: FleetTargetKind,
) -> Result<(), FleetWriteError> {
    let group_id = input
        .acs_group_id
        .ok_or_else(|| FleetWriteError::Invalid("ACS mission requires an ACS group".to_string()))?;
    let row = transaction
        .query_opt(
            "SELECT acs.mission_type, acs.target_kind, acs.target_galaxy,
                    acs.target_system, acs.target_position, acs.status,
                    acs.departure_window_start <= now()
                        AND acs.departure_window_end >= now() AS window_open,
                    member.user_id IS NOT NULL AS is_member,
                    member.fleet_id IS NULL AS launch_slot_available
             FROM acs_groups AS acs
             LEFT JOIN acs_group_members AS member
               ON member.universe_id = acs.universe_id
              AND member.group_id = acs.id AND member.user_id = $3
             WHERE acs.universe_id = $1 AND acs.id = $2
             FOR UPDATE OF acs",
            &[&account.universe_id, &group_id, &account.user_id],
        )
        .await
        .map_err(map_fleet_db_error)?
        .ok_or(FleetWriteError::NotFound)?;
    let expected_type = if mission_type == FleetMissionType::AcsDefend {
        "defend"
    } else {
        "attack"
    };
    if row.get::<_, String>("mission_type") != expected_type
        || row.get::<_, String>("target_kind") != target_kind.as_str()
        || row.get::<_, i32>("target_galaxy") != input.target_galaxy
        || row.get::<_, i32>("target_system") != input.target_system
        || row.get::<_, i32>("target_position") != input.target_position
        || row.get::<_, String>("status") != "forming"
        || !row.get::<_, bool>("window_open")
        || !row.get::<_, bool>("is_member")
        || (mission_type == FleetMissionType::AcsJoin
            && !row.get::<_, bool>("launch_slot_available"))
    {
        return Err(FleetWriteError::Invalid(
            "ACS group does not authorize this launch".to_string(),
        ));
    }
    Ok(())
}

async fn reconcile_acs_rendezvous(
    transaction: &Transaction<'_>,
    input: &FleetLaunchInput,
    account: &AccountState,
    mission_type: FleetMissionType,
    candidate_arrival: i64,
) -> Result<(i64, i32), FleetWriteError> {
    if !matches!(
        mission_type,
        FleetMissionType::AcsAttack | FleetMissionType::AcsJoin
    ) {
        return Ok((candidate_arrival, 0));
    }
    let group_id = input
        .acs_group_id
        .ok_or_else(|| FleetWriteError::Invalid("ACS attack requires an ACS group".to_string()))?;
    let schedule = transaction
        .query_opt(
            "UPDATE acs_groups
             SET schedule_generation = schedule_generation +
                    CASE WHEN rendezvous_at IS NULL
                                   OR rendezvous_at <
                                      to_timestamp(($3::BIGINT)::DOUBLE PRECISION)
                         THEN 1 ELSE 0 END,
                 rendezvous_at = GREATEST(
                    COALESCE(rendezvous_at,
                             to_timestamp(($3::BIGINT)::DOUBLE PRECISION)),
                    to_timestamp(($3::BIGINT)::DOUBLE PRECISION))
             WHERE universe_id = $1 AND id = $2 AND mission_type = 'attack'
               AND status = 'forming'
               AND departure_window_start <= clock_timestamp()
               AND departure_window_end >= clock_timestamp()
             RETURNING EXTRACT(EPOCH FROM rendezvous_at)::BIGINT AS rendezvous_unix,
                       schedule_generation",
            &[&account.universe_id, &group_id, &candidate_arrival],
        )
        .await
        .map_err(map_fleet_db_error)?
        .ok_or_else(|| {
            FleetWriteError::Invalid("ACS group launch window is no longer open".to_string())
        })?;
    let rendezvous = schedule.get::<_, i64>("rendezvous_unix");
    let generation = schedule.get::<_, i32>("schedule_generation");

    // A later, slower participant moves every still-forming attack forward in
    // the same transaction. The migration trigger permits only this monotonic
    // update to the locked group's exact rendezvous timestamp/generation.
    transaction
        .execute(
            "UPDATE fleets
             SET arrives_at = to_timestamp(($3::BIGINT)::DOUBLE PRECISION),
                 arrival_time = to_timestamp(($3::BIGINT)::DOUBLE PRECISION)
                                AT TIME ZONE 'UTC',
                 returns_at = to_timestamp(($3::BIGINT)::DOUBLE PRECISION) +
                              make_interval(secs => duration_seconds::DOUBLE PRECISION),
                 return_time = (to_timestamp(($3::BIGINT)::DOUBLE PRECISION) +
                                make_interval(secs => duration_seconds::DOUBLE PRECISION))
                               AT TIME ZONE 'UTC',
                 phase_due_at = to_timestamp(($3::BIGINT)::DOUBLE PRECISION),
                 acs_schedule_generation = $4
             WHERE universe_id = $1 AND acs_group_id = $2
               AND mission_type IN ('acs_attack', 'acs_join')
               AND status = 'outbound' AND arrival_resolved_at IS NULL
               AND recalled_at IS NULL
               AND arrives_at < to_timestamp(($3::BIGINT)::DOUBLE PRECISION)",
            &[&account.universe_id, &group_id, &rendezvous, &generation],
        )
        .await
        .map_err(map_fleet_db_error)?;
    Ok((rendezvous, generation))
}

async fn lock_target_planet(
    transaction: &Transaction<'_>,
    universe_id: i64,
    coordinates: &Coordinates,
) -> Result<LocationState, FleetWriteError> {
    let row = transaction
        .query_opt(
            &format!(
                "SELECT p.id AS planet_id, NULL::INTEGER AS moon_id, p.user_id,
                        p.universe_id, u.alliance_id, p.galaxy, p.system, p.position,
                        COALESCE(p.metal, 0)::BIGINT AS metal,
                        COALESCE(p.crystal, 0)::BIGINT AS crystal,
                        COALESCE(p.deuterium, 0)::BIGINT AS deuterium,
                        NULL::INTEGER AS moon_diameter,
                        COALESCE(score.total_score, 0)::BIGINT AS owner_score,
                        (u.is_banned OR u.privacy_restriction_active
                         OR u.privacy_erasure_pending OR COALESCE(u.is_locked, FALSE)
                         OR COALESCE(u.account_status, 'active') <> 'active') AS owner_restricted,
                        u.vacation_mode AS owner_vacation,
                        {}, {}
                 FROM planets AS p
                 JOIN users AS u ON u.id = p.user_id AND u.universe_id = p.universe_id
                 LEFT JOIN player_scores AS score ON score.user_id = u.id
                 WHERE p.universe_id = $1 AND p.galaxy = $2
                   AND p.system = $3 AND p.position = $4
                 FOR UPDATE OF p, u",
                location_ship_select("p"),
                location_defense_select("p")
            ),
            &[
                &universe_id,
                &coordinates.galaxy,
                &coordinates.system,
                &coordinates.position,
            ],
        )
        .await
        .map_err(map_fleet_db_error)?
        .ok_or(FleetWriteError::NotFound)?;
    Ok(map_location_row(FleetSourceKind::Planet, &row))
}

async fn lock_target_moon(
    transaction: &Transaction<'_>,
    universe_id: i64,
    coordinates: &Coordinates,
) -> Result<LocationState, FleetWriteError> {
    let row = transaction
        .query_opt(
            &format!(
                "SELECT p.id AS planet_id, m.id AS moon_id, m.user_id,
                        m.universe_id, u.alliance_id, p.galaxy, p.system, p.position,
                        m.metal, m.crystal, m.deuterium, m.diameter AS moon_diameter,
                        COALESCE(score.total_score, 0)::BIGINT AS owner_score,
                        (u.is_banned OR u.privacy_restriction_active
                         OR u.privacy_erasure_pending OR COALESCE(u.is_locked, FALSE)
                         OR COALESCE(u.account_status, 'active') <> 'active') AS owner_restricted,
                        u.vacation_mode AS owner_vacation,
                        {}, {}
                 FROM moons AS m
                 JOIN planets AS p
                   ON p.universe_id = m.universe_id AND p.id = m.planet_id
                 JOIN users AS u ON u.id = m.user_id AND u.universe_id = m.universe_id
                 LEFT JOIN player_scores AS score ON score.user_id = u.id
                 WHERE m.universe_id = $1 AND p.galaxy = $2
                   AND p.system = $3 AND p.position = $4
                   AND m.destroyed_at IS NULL
                 FOR UPDATE OF m, u",
                location_ship_select("m"),
                location_defense_select("m")
            ),
            &[
                &universe_id,
                &coordinates.galaxy,
                &coordinates.system,
                &coordinates.position,
            ],
        )
        .await
        .map_err(map_fleet_db_error)?
        .ok_or(FleetWriteError::NotFound)?;
    Ok(map_location_row(FleetSourceKind::Moon, &row))
}

fn location_ship_select(alias: &str) -> String {
    SHIP_TYPES
        .iter()
        .map(|ship_type| format!("COALESCE({alias}.{ship_type}, 0)::BIGINT AS {ship_type}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn location_defense_select(alias: &str) -> String {
    DEFENSE_TYPES
        .iter()
        .map(|defense_type| {
            format!("COALESCE({alias}.{defense_type}, 0)::BIGINT AS {defense_type}")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn map_location_row(kind: FleetSourceKind, row: &tokio_postgres::Row) -> LocationState {
    LocationState {
        kind,
        planet_id: row.get("planet_id"),
        moon_id: row.get("moon_id"),
        owner_id: row.get("user_id"),
        alliance_id: row.get("alliance_id"),
        coordinates: Coordinates::new(row.get("galaxy"), row.get("system"), row.get("position")),
        metal: row.get("metal"),
        crystal: row.get("crystal"),
        deuterium: row.get("deuterium"),
        ships: SHIP_TYPES
            .iter()
            .map(|ship_type| (ship_type.to_string(), row.get::<_, i64>(*ship_type)))
            .collect(),
        defenses: DEFENSE_TYPES
            .iter()
            .map(|defense_type| (defense_type.to_string(), row.get::<_, i64>(*defense_type)))
            .collect(),
        moon_diameter: row.get("moon_diameter"),
        owner_score: row.get("owner_score"),
        owner_restricted: row.get("owner_restricted"),
        owner_vacation: row.get("owner_vacation"),
    }
}

fn is_hostile_mission(mission_type: FleetMissionType) -> bool {
    matches!(
        mission_type,
        FleetMissionType::Attack
            | FleetMissionType::Espionage
            | FleetMissionType::Destroy
            | FleetMissionType::AcsAttack
            | FleetMissionType::AcsJoin
    )
}

fn is_score_protected(
    attacker_score: i64,
    defender_score: i64,
    config: &FleetServerConfig,
) -> bool {
    if !config.noob_protection_enabled {
        return false;
    }
    let multiplier = i128::from(config.noob_protection_multiplier_milli);
    let attacker = i128::from(attacker_score.max(0)) * 1_000;
    let defender = i128::from(defender_score.max(0)) * 1_000;
    (defender_score < config.noob_protection_points
        && attacker > multiplier * i128::from(defender_score.max(0)))
        || (attacker_score < config.noob_protection_points
            && defender > multiplier * i128::from(attacker_score.max(0)))
}

async fn ensure_fleet_slots(
    transaction: &Transaction<'_>,
    account: &AccountState,
    source: &LocationState,
    config: &FleetServerConfig,
) -> Result<(), FleetWriteError> {
    let counts = transaction
        .query_one(
            "SELECT COUNT(*) FILTER (
                        WHERE status IN ('outbound', 'returning')
                    )::BIGINT AS user_active,
                    COUNT(*) FILTER (
                        WHERE status IN ('outbound', 'returning')
                          AND origin_kind = $3
                          AND origin_planet_id = $4
                          AND origin_moon_id IS NOT DISTINCT FROM $5
                    )::BIGINT AS location_active
             FROM fleets
             WHERE universe_id = $1 AND user_id = $2",
            &[
                &account.universe_id,
                &account.user_id,
                &source.kind.as_str(),
                &source.planet_id,
                &source.moon_id,
            ],
        )
        .await
        .map_err(map_fleet_db_error)?;
    let user_limit = 1_i64 + i64::from(account.computer_technology.max(0));
    if counts.get::<_, i64>("user_active") >= user_limit
        || counts.get::<_, i64>("location_active") >= config.max_active_per_location
    {
        return Err(FleetWriteError::FleetSlotsExhausted);
    }
    Ok(())
}

fn ensure_source_inventory(
    source: &LocationState,
    requested: &BTreeMap<String, i64>,
) -> Result<(), FleetWriteError> {
    if requested.iter().any(|(ship_type, requested)| {
        *requested <= 0 || source.ships.get(ship_type).copied().unwrap_or(0) < *requested
    }) {
        Err(FleetWriteError::InsufficientShips)
    } else {
        Ok(())
    }
}

fn ensure_source_resources(
    source: &LocationState,
    cargo: &Resources,
    fuel: i64,
) -> Result<(), FleetWriteError> {
    let total_deuterium = cargo
        .deuterium
        .checked_add(fuel)
        .ok_or_else(|| FleetWriteError::Invalid("deuterium deduction overflow".to_string()))?;
    if source.metal < cargo.metal
        || source.crystal < cargo.crystal
        || source.deuterium < total_deuterium
    {
        Err(FleetWriteError::InsufficientResources)
    } else {
        Ok(())
    }
}

async fn deduct_launch_inventory(
    transaction: &Transaction<'_>,
    source: &LocationState,
    ships: &BTreeMap<String, i64>,
    cargo: &Resources,
    fuel: i64,
) -> Result<(), FleetWriteError> {
    let (table, location_id) = location_table_and_id(source)?;
    let deuterium = cargo
        .deuterium
        .checked_add(fuel)
        .ok_or_else(|| FleetWriteError::Invalid("deuterium deduction overflow".to_string()))?;
    let resource_update = transaction
        .execute(
            &format!(
                "UPDATE {table}
                 SET metal = metal - $2, crystal = crystal - $3, deuterium = deuterium - $4
                 WHERE id = $1 AND metal >= $2 AND crystal >= $3 AND deuterium >= $4"
            ),
            &[&location_id, &cargo.metal, &cargo.crystal, &deuterium],
        )
        .await
        .map_err(map_fleet_db_error)?;
    if resource_update != 1 {
        return Err(FleetWriteError::InsufficientResources);
    }
    for (ship_type, count) in ships {
        if !SHIP_TYPES.contains(&ship_type.as_str()) {
            return Err(FleetWriteError::Invalid(
                "unsupported ship type".to_string(),
            ));
        }
        let updated = transaction
            .execute(
                &format!(
                    "UPDATE {table}
                     SET {ship_type} = {ship_type} - $2
                     WHERE id = $1 AND {ship_type} >= $2"
                ),
                &[&location_id, count],
            )
            .await
            .map_err(map_fleet_db_error)?;
        if updated != 1 {
            return Err(FleetWriteError::InsufficientShips);
        }
    }
    Ok(())
}

fn location_table_and_id(source: &LocationState) -> Result<(&'static str, i32), FleetWriteError> {
    match source.kind {
        FleetSourceKind::Planet => Ok(("planets", source.planet_id)),
        FleetSourceKind::Moon => source
            .moon_id
            .map(|moon_id| ("moons", moon_id))
            .ok_or(FleetWriteError::NotFound),
    }
}

fn fleet_select_sql() -> &'static str {
    "SELECT id::TEXT AS id, universe_id, user_id::TEXT AS user_id, command_id,
            mission_type, status, origin_kind, origin_planet_id::TEXT AS origin_planet_id,
            origin_moon_id::TEXT AS origin_moon_id,
            origin_galaxy, origin_system, origin_position,
            target_kind, target_planet_id::TEXT AS target_planet_id,
            target_moon_id::TEXT AS target_moon_id,
            target_galaxy, target_system, target_position, acs_group_id,
            EXTRACT(EPOCH FROM departed_at)::BIGINT AS departed_at_unix,
            EXTRACT(EPOCH FROM arrives_at)::BIGINT AS arrives_at_unix,
            EXTRACT(EPOCH FROM returns_at)::BIGINT AS returns_at_unix,
            EXTRACT(EPOCH FROM phase_due_at)::BIGINT AS phase_due_at_unix,
            distance, fleet_speed, duration_seconds, hold_seconds,
            movement_fuel_consumed, holding_fuel_consumed, fuel_consumed, cargo_capacity,
            applied_universe_speed, applied_speed_percent,
            applied_fuel_multiplier_milli, applied_cargo_multiplier_milli,
            cargo_metal, cargo_crystal, cargo_deuterium,
            CASE WHEN recalled_at IS NULL THEN NULL
                 ELSE EXTRACT(EPOCH FROM recalled_at)::BIGINT END AS recalled_at_unix,
            CASE WHEN arrival_resolved_at IS NULL THEN NULL
                 ELSE EXTRACT(EPOCH FROM arrival_resolved_at)::BIGINT END AS arrival_resolved_at_unix,
            CASE WHEN hold_resolved_at IS NULL THEN NULL
                 ELSE EXTRACT(EPOCH FROM hold_resolved_at)::BIGINT END AS hold_resolved_at_unix,
            CASE WHEN return_resolved_at IS NULL THEN NULL
                 ELSE EXTRACT(EPOCH FROM return_resolved_at)::BIGINT END AS return_resolved_at_unix,
            CASE WHEN terminal_at IS NULL THEN NULL
                 ELSE EXTRACT(EPOCH FROM terminal_at)::BIGINT END AS terminal_at_unix,
            result
     FROM fleets"
}

async fn map_fleet_row(
    client: &deadpool_postgres::Client,
    row: &tokio_postgres::Row,
) -> DbResult<FleetMissionRow> {
    let fleet_id = row
        .get::<_, String>("id")
        .parse::<i32>()
        .map_err(|_| "invalid fleet id".to_string())?;
    let ship_rows = client
        .query(
            "SELECT ship_type, current_count
             FROM fleet_mission_ships WHERE fleet_id = $1 ORDER BY ship_type",
            &[&fleet_id],
        )
        .await
        .map_err(|error| error.to_string())?;
    Ok(FleetMissionRow {
        id: row.get("id"),
        universe_id: row.get("universe_id"),
        user_id: row.get("user_id"),
        command_id: row.get("command_id"),
        mission_type: row.get("mission_type"),
        status: row.get("status"),
        origin_kind: row.get("origin_kind"),
        origin_planet_id: row.get("origin_planet_id"),
        origin_moon_id: row.get("origin_moon_id"),
        origin_galaxy: row.get("origin_galaxy"),
        origin_system: row.get("origin_system"),
        origin_position: row.get("origin_position"),
        target_kind: row.get("target_kind"),
        target_planet_id: row.get("target_planet_id"),
        target_moon_id: row.get("target_moon_id"),
        target_galaxy: row.get("target_galaxy"),
        target_system: row.get("target_system"),
        target_position: row.get("target_position"),
        acs_group_id: row.get("acs_group_id"),
        departed_at_unix: row.get("departed_at_unix"),
        arrives_at_unix: row.get("arrives_at_unix"),
        returns_at_unix: row.get("returns_at_unix"),
        phase_due_at_unix: row.get("phase_due_at_unix"),
        distance: row.get("distance"),
        fleet_speed: row.get("fleet_speed"),
        duration_seconds: row.get("duration_seconds"),
        hold_seconds: row.get("hold_seconds"),
        movement_fuel_consumed: row.get("movement_fuel_consumed"),
        holding_fuel_consumed: row.get("holding_fuel_consumed"),
        fuel_consumed: row.get("fuel_consumed"),
        cargo_capacity: row.get("cargo_capacity"),
        applied_universe_speed: row.get("applied_universe_speed"),
        applied_speed_percent: row.get("applied_speed_percent"),
        applied_fuel_multiplier_milli: row.get("applied_fuel_multiplier_milli"),
        applied_cargo_multiplier_milli: row.get("applied_cargo_multiplier_milli"),
        cargo_metal: row.get("cargo_metal"),
        cargo_crystal: row.get("cargo_crystal"),
        cargo_deuterium: row.get("cargo_deuterium"),
        recalled_at_unix: row.get("recalled_at_unix"),
        arrival_resolved_at_unix: row.get("arrival_resolved_at_unix"),
        hold_resolved_at_unix: row.get("hold_resolved_at_unix"),
        return_resolved_at_unix: row.get("return_resolved_at_unix"),
        terminal_at_unix: row.get("terminal_at_unix"),
        result: row.get("result"),
        ships: ship_rows
            .iter()
            .map(|ship| (ship.get("ship_type"), ship.get("current_count")))
            .collect(),
    })
}

#[allow(clippy::too_many_arguments)]
async fn append_fleet_event(
    transaction: &Transaction<'_>,
    universe_id: i64,
    fleet_id: i32,
    event_key: &str,
    event_type: &str,
    phase_generation: i32,
    actor_user_id: Option<i32>,
    payload: serde_json::Value,
) -> Result<(), FleetWriteError> {
    let inserted = transaction
        .execute(
            "INSERT INTO fleet_mission_events
                (universe_id, fleet_id, sequence, event_key, event_type,
                 phase_generation, actor_user_id, payload)
             SELECT $1, $2, COALESCE(MAX(sequence), 0) + 1, $3, $4, $5, $6, $7
             FROM fleet_mission_events
             WHERE fleet_id = $2
             ON CONFLICT (fleet_id, event_key) DO NOTHING",
            &[
                &universe_id,
                &fleet_id,
                &event_key,
                &event_type,
                &phase_generation,
                &actor_user_id,
                &Json(&payload),
            ],
        )
        .await
        .map_err(map_fleet_db_error)?;
    if inserted == 0 {
        return Err(FleetWriteError::Invalid(format!(
            "fleet event key already exists: {event_key}"
        )));
    }
    Ok(())
}

async fn insert_notification(
    transaction: &Transaction<'_>,
    user_id: i32,
    notification_type: &str,
    title: &str,
    message: &str,
    priority: i32,
    fleet_id: i32,
) -> Result<(), FleetWriteError> {
    transaction
        .execute(
            "INSERT INTO notifications
                (user_id, notification_type_id, title, message, priority,
                 reference_type, reference_id, metadata)
             SELECT $1, id, $3, $4, $5, 'fleet', $6,
                    jsonb_build_object('fleetId', $6)
             FROM notification_types WHERE type_name = $2",
            &[
                &user_id,
                &notification_type,
                &title,
                &message,
                &priority,
                &fleet_id,
            ],
        )
        .await
        .map_err(map_fleet_db_error)?;
    Ok(())
}

fn map_fleet_insert_error(error: tokio_postgres::Error) -> FleetWriteError {
    if error.code() == Some(&SqlState::UNIQUE_VIOLATION) {
        FleetWriteError::IdempotencyConflict
    } else {
        map_fleet_db_error(error)
    }
}

fn map_fleet_db_error(error: tokio_postgres::Error) -> FleetWriteError {
    if matches!(
        error.code(),
        Some(&SqlState::T_R_DEADLOCK_DETECTED | &SqlState::T_R_SERIALIZATION_FAILURE)
    ) {
        FleetWriteError::Retryable(error.to_string())
    } else {
        FleetWriteError::Database(error.to_string())
    }
}

fn parse_i32(value: &str) -> Result<i32, FleetWriteError> {
    value.parse::<i32>().map_err(|_| FleetWriteError::NotFound)
}

fn parse_optional_i32(value: &str) -> Option<i32> {
    value.parse::<i32>().ok()
}
