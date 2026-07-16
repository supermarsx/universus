#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

// ---------------------------------------------------------------------------
// ships module (preserved from original)
// ---------------------------------------------------------------------------

pub mod ships {
    use serde::Deserialize;
    use std::collections::HashMap;

    #[derive(Debug, Deserialize, Clone)]
    pub struct ShipDef {
        pub name: String,
        pub weapon: Option<f64>,
        pub shield: Option<f64>,
        pub hull: Option<f64>,
        pub cargo: Option<i64>,
        pub metal_cost: Option<i64>,
        pub crystal_cost: Option<i64>,
        pub deuterium_cost: Option<i64>,
        pub rapid_fire: Option<HashMap<String, i32>>,
    }

    pub fn load_ships_for_universe(universe: &str) -> HashMap<String, ShipDef> {
        let assets_path = format!(
            "{}/assets/{}/ships.json",
            env!("CARGO_MANIFEST_DIR"),
            universe
        );
        if let Ok(s) = std::fs::read_to_string(&assets_path) {
            if let Ok(m) = serde_json::from_str(&s) {
                return m;
            }
        }

        let json = r#"
        {
            "fighter": { "name": "fighter", "weapon": 100.0, "shield": 50.0, "hull": 200.0, "cargo": 5, "metal_cost": 300, "crystal_cost": 100, "deuterium_cost": 0 },
            "bomber": { "name": "bomber", "weapon": 400.0, "shield": 150.0, "hull": 600.0, "cargo": 20, "metal_cost": 1200, "crystal_cost": 800, "deuterium_cost": 0, "rapid_fire": {"defender": 2} },
            "defender": { "name": "defender", "weapon": 150.0, "shield": 80.0, "hull": 400.0, "cargo": 0, "metal_cost": 800, "crystal_cost": 300, "deuterium_cost": 0 },
            "turret": { "name": "turret", "weapon": 300.0, "shield": 200.0, "hull": 900.0, "cargo": 0, "metal_cost": 2000, "crystal_cost": 1200, "deuterium_cost": 0, "rapid_fire": {"fighter": 3} }
        }
        "#;
        serde_json::from_str(json).expect("ships json parse")
    }

    pub fn load_default_ships() -> HashMap<String, ShipDef> {
        load_ships_for_universe("default")
    }
}

// ---------------------------------------------------------------------------
// Original movement types (preserved)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FleetShipInput {
    pub count: i32,
    pub base_speed: f64,
    pub fuel_consumption: f64,
    pub cargo: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FleetMovementInput {
    pub origin_galaxy: i32,
    pub origin_system: i32,
    pub origin_position: i32,
    pub target_galaxy: i32,
    pub target_system: i32,
    pub target_position: i32,
    pub ships: Vec<FleetShipInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FleetMovementResult {
    pub distance: i32,
    pub fleet_speed: f64,
    pub travel_time_seconds: i32,
    pub fuel_needed: f64,
    pub cargo_capacity: f64,
}

pub fn calculate_distance(
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

pub fn calculate_movement(input: &FleetMovementInput) -> FleetMovementResult {
    let distance = calculate_distance(
        input.origin_galaxy,
        input.origin_system,
        input.origin_position,
        input.target_galaxy,
        input.target_system,
        input.target_position,
    );

    let mut min_speed = f64::INFINITY;
    let mut fuel_needed = 0.0f64;
    let mut cargo_capacity = 0.0f64;

    for ship in &input.ships {
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

    let fleet_speed = if min_speed.is_finite() {
        min_speed
    } else {
        0.0
    };
    let travel_time_seconds = if fleet_speed > 0.0 {
        ((distance as f64 / fleet_speed) * 3600.0).ceil() as i32
    } else {
        0
    };

    cargo_capacity -= fuel_needed;

    FleetMovementResult {
        distance,
        fleet_speed,
        travel_time_seconds,
        fuel_needed,
        cargo_capacity,
    }
}

// ---------------------------------------------------------------------------
// Coordinates
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Coordinates {
    pub galaxy: i32,
    pub system: i32,
    pub position: i32,
}

impl Coordinates {
    pub fn new(galaxy: i32, system: i32, position: i32) -> Self {
        Self {
            galaxy,
            system,
            position,
        }
    }
}

impl fmt::Display for Coordinates {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}:{}:{}]", self.galaxy, self.system, self.position)
    }
}

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Resources {
    pub metal: i64,
    pub crystal: i64,
    pub deuterium: i64,
}

/// The authoritative kind of a fleet destination. Coordinates alone are not
/// enough to distinguish a planet from its moon, an unoccupied colonization
/// slot, a debris field, or the expedition position.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum FleetTargetKind {
    Planet,
    Moon,
    Debris,
    EmptyCoordinate,
    ExpeditionSlot,
}

impl FleetTargetKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Planet => "planet",
            Self::Moon => "moon",
            Self::Debris => "debris",
            Self::EmptyCoordinate => "empty_coordinate",
            Self::ExpeditionSlot => "expedition_slot",
        }
    }
}

impl fmt::Display for FleetTargetKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for FleetTargetKind {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "planet" => Ok(Self::Planet),
            "moon" => Ok(Self::Moon),
            "debris" | "debris_field" => Ok(Self::Debris),
            "empty" | "empty_coordinate" => Ok(Self::EmptyCoordinate),
            "expedition" | "expedition_slot" => Ok(Self::ExpeditionSlot),
            other => Err(format!("unknown fleet target kind: {other}")),
        }
    }
}

impl Resources {
    pub fn new(metal: i64, crystal: i64, deuterium: i64) -> Self {
        Self {
            metal,
            crystal,
            deuterium,
        }
    }

    pub fn zero() -> Self {
        Self {
            metal: 0,
            crystal: 0,
            deuterium: 0,
        }
    }

    pub fn total(&self) -> i64 {
        self.metal + self.crystal + self.deuterium
    }
}

// ---------------------------------------------------------------------------
// Fleet Mission Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum FleetMissionType {
    Attack,
    Transport,
    Deploy,
    Espionage,
    Colonize,
    Harvest,
    Expedition,
    Destroy,
    AcsAttack,
    AcsDefend,
    AcsJoin,
}

impl FleetMissionType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Attack => "attack",
            Self::Transport => "transport",
            Self::Deploy => "deploy",
            Self::Espionage => "espionage",
            Self::Colonize => "colonize",
            Self::Harvest => "harvest",
            Self::Expedition => "expedition",
            Self::Destroy => "destroy",
            Self::AcsAttack => "acs_attack",
            Self::AcsDefend => "acs_defend",
            Self::AcsJoin => "acs_join",
        }
    }

    pub const fn target_kind_allowed(self, kind: FleetTargetKind) -> bool {
        match self {
            Self::Attack | Self::Espionage | Self::AcsAttack | Self::AcsDefend | Self::AcsJoin => {
                matches!(kind, FleetTargetKind::Planet | FleetTargetKind::Moon)
            }
            Self::Transport | Self::Deploy => {
                matches!(kind, FleetTargetKind::Planet | FleetTargetKind::Moon)
            }
            Self::Colonize => matches!(kind, FleetTargetKind::EmptyCoordinate),
            Self::Harvest => matches!(kind, FleetTargetKind::Debris),
            Self::Expedition => matches!(kind, FleetTargetKind::ExpeditionSlot),
            Self::Destroy => matches!(kind, FleetTargetKind::Moon),
        }
    }
}

impl fmt::Display for FleetMissionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            FleetMissionType::Attack => "Attack",
            FleetMissionType::Transport => "Transport",
            FleetMissionType::Deploy => "Deploy",
            FleetMissionType::Espionage => "Espionage",
            FleetMissionType::Colonize => "Colonize",
            FleetMissionType::Harvest => "Harvest",
            FleetMissionType::Expedition => "Expedition",
            FleetMissionType::Destroy => "Destroy",
            FleetMissionType::AcsAttack => "AcsAttack",
            FleetMissionType::AcsDefend => "AcsDefend",
            FleetMissionType::AcsJoin => "AcsJoin",
        };
        write!(f, "{}", s)
    }
}

impl FromStr for FleetMissionType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "attack" => Ok(FleetMissionType::Attack),
            "transport" => Ok(FleetMissionType::Transport),
            "deploy" => Ok(FleetMissionType::Deploy),
            "espionage" | "spy" => Ok(FleetMissionType::Espionage),
            "colonize" | "colonise" => Ok(FleetMissionType::Colonize),
            "harvest" | "recycle" => Ok(FleetMissionType::Harvest),
            "expedition" | "explore" => Ok(FleetMissionType::Expedition),
            "destroy" | "moon_destroy" | "moondestroy" => Ok(FleetMissionType::Destroy),
            "acsattack" | "acs_attack" => Ok(FleetMissionType::AcsAttack),
            "acsdefend" | "acs_defend" => Ok(FleetMissionType::AcsDefend),
            "acsjoin" | "acs_join" => Ok(FleetMissionType::AcsJoin),
            _ => Err(format!("unknown mission type: {}", s)),
        }
    }
}

// ---------------------------------------------------------------------------
// Mission Status
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MissionStatus {
    Outbound,
    Arrived,
    Returning,
    Completed,
    Recalled,
}

impl fmt::Display for MissionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            MissionStatus::Outbound => "Outbound",
            MissionStatus::Arrived => "Arrived",
            MissionStatus::Returning => "Returning",
            MissionStatus::Completed => "Completed",
            MissionStatus::Recalled => "Recalled",
        };
        write!(f, "{}", s)
    }
}

// ---------------------------------------------------------------------------
// Ship Stats (OGame ship database)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShipStats {
    pub weapon: f64,
    pub shield: f64,
    pub hull: f64,
    pub speed: f64,
    pub cargo: i64,
    pub fuel_consumption: f64,
    pub rapid_fire: HashMap<String, i32>,
}

/// Returns hardcoded OGame-style ship stats for the given ship type.
/// Ship types: light_fighter, heavy_fighter, cruiser, battleship,
/// battlecruiser, bomber, destroyer, deathstar, small_cargo, large_cargo,
/// colony_ship, recycler, espionage_probe, solar_satellite, pathfinder.
pub fn get_ship_stats(ship_type: &str) -> Option<ShipStats> {
    let rf = |pairs: &[(&str, i32)]| -> HashMap<String, i32> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    };

    let stats = match ship_type {
        "light_fighter" => ShipStats {
            weapon: 50.0,
            shield: 10.0,
            hull: 400.0,
            speed: 12500.0,
            cargo: 50,
            fuel_consumption: 20.0,
            rapid_fire: rf(&[("espionage_probe", 5), ("solar_satellite", 5)]),
        },
        "heavy_fighter" => ShipStats {
            weapon: 150.0,
            shield: 25.0,
            hull: 1000.0,
            speed: 10000.0,
            cargo: 100,
            fuel_consumption: 75.0,
            rapid_fire: rf(&[
                ("small_cargo", 3),
                ("espionage_probe", 5),
                ("solar_satellite", 5),
            ]),
        },
        "cruiser" => ShipStats {
            weapon: 400.0,
            shield: 50.0,
            hull: 2700.0,
            speed: 15000.0,
            cargo: 800,
            fuel_consumption: 300.0,
            rapid_fire: rf(&[
                ("light_fighter", 6),
                ("espionage_probe", 5),
                ("solar_satellite", 5),
            ]),
        },
        "battleship" => ShipStats {
            weapon: 1000.0,
            shield: 200.0,
            hull: 6000.0,
            speed: 10000.0,
            cargo: 1500,
            fuel_consumption: 500.0,
            rapid_fire: rf(&[("espionage_probe", 5), ("solar_satellite", 5)]),
        },
        "battlecruiser" => ShipStats {
            weapon: 700.0,
            shield: 400.0,
            hull: 7000.0,
            speed: 10000.0,
            cargo: 750,
            fuel_consumption: 250.0,
            rapid_fire: rf(&[
                ("small_cargo", 3),
                ("large_cargo", 3),
                ("heavy_fighter", 4),
                ("cruiser", 4),
                ("battleship", 7),
                ("espionage_probe", 5),
                ("solar_satellite", 5),
            ]),
        },
        "bomber" => ShipStats {
            weapon: 1000.0,
            shield: 500.0,
            hull: 7500.0,
            speed: 4000.0,
            cargo: 500,
            fuel_consumption: 700.0,
            rapid_fire: rf(&[("espionage_probe", 5), ("solar_satellite", 5)]),
        },
        "destroyer" => ShipStats {
            weapon: 2000.0,
            shield: 500.0,
            hull: 11000.0,
            speed: 5000.0,
            cargo: 2000,
            fuel_consumption: 1000.0,
            rapid_fire: rf(&[
                ("battlecruiser", 2),
                ("espionage_probe", 5),
                ("solar_satellite", 5),
            ]),
        },
        "deathstar" => ShipStats {
            weapon: 200000.0,
            shield: 50000.0,
            hull: 900000.0,
            speed: 100.0,
            cargo: 1000000,
            fuel_consumption: 1.0,
            rapid_fire: rf(&[
                ("small_cargo", 250),
                ("large_cargo", 250),
                ("light_fighter", 200),
                ("heavy_fighter", 100),
                ("cruiser", 33),
                ("battleship", 30),
                ("bomber", 25),
                ("destroyer", 5),
                ("espionage_probe", 1250),
                ("solar_satellite", 1250),
                ("battlecruiser", 15),
                ("recycler", 250),
                ("colony_ship", 250),
            ]),
        },
        "small_cargo" => ShipStats {
            weapon: 5.0,
            shield: 10.0,
            hull: 400.0,
            speed: 5000.0,
            cargo: 5000,
            fuel_consumption: 10.0,
            rapid_fire: rf(&[("espionage_probe", 5), ("solar_satellite", 5)]),
        },
        "large_cargo" => ShipStats {
            weapon: 5.0,
            shield: 25.0,
            hull: 1200.0,
            speed: 7500.0,
            cargo: 25000,
            fuel_consumption: 50.0,
            rapid_fire: rf(&[("espionage_probe", 5), ("solar_satellite", 5)]),
        },
        "colony_ship" => ShipStats {
            weapon: 50.0,
            shield: 100.0,
            hull: 3000.0,
            speed: 2500.0,
            cargo: 7500,
            fuel_consumption: 1000.0,
            rapid_fire: rf(&[("espionage_probe", 5), ("solar_satellite", 5)]),
        },
        "recycler" => ShipStats {
            weapon: 1.0,
            shield: 10.0,
            hull: 1600.0,
            speed: 2000.0,
            cargo: 20000,
            fuel_consumption: 300.0,
            rapid_fire: rf(&[("espionage_probe", 5), ("solar_satellite", 5)]),
        },
        "espionage_probe" => ShipStats {
            weapon: 0.01,
            shield: 0.01,
            hull: 100.0,
            speed: 100000000.0,
            cargo: 0,
            fuel_consumption: 1.0,
            rapid_fire: HashMap::new(),
        },
        "solar_satellite" => ShipStats {
            weapon: 1.0,
            shield: 1.0,
            hull: 200.0,
            speed: 0.0,
            cargo: 0,
            fuel_consumption: 0.0,
            rapid_fire: HashMap::new(),
        },
        "pathfinder" => ShipStats {
            weapon: 200.0,
            shield: 100.0,
            hull: 2300.0,
            speed: 12000.0,
            cargo: 10000,
            fuel_consumption: 300.0,
            rapid_fire: rf(&[
                ("espionage_probe", 5),
                ("solar_satellite", 5),
                ("cruiser", 3),
                ("light_fighter", 3),
            ]),
        },
        _ => return None,
    };
    Some(stats)
}

/// All known ship type identifiers.
pub const ALL_SHIP_TYPES: &[&str] = &[
    "light_fighter",
    "heavy_fighter",
    "cruiser",
    "battleship",
    "battlecruiser",
    "bomber",
    "destroyer",
    "deathstar",
    "small_cargo",
    "large_cargo",
    "colony_ship",
    "recycler",
    "espionage_probe",
    "solar_satellite",
    "pathfinder",
];

// ---------------------------------------------------------------------------
// Fleet Composition
// ---------------------------------------------------------------------------

/// Maps ship type names to their counts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FleetComposition {
    pub ships: HashMap<String, i64>,
}

impl FleetComposition {
    pub fn new() -> Self {
        Self {
            ships: HashMap::new(),
        }
    }

    pub fn from_map(ships: HashMap<String, i64>) -> Self {
        Self { ships }
    }

    /// Add ships of a given type.
    pub fn add(&mut self, ship_type: &str, count: i64) {
        let entry = self.ships.entry(ship_type.to_string()).or_insert(0);
        *entry += count;
    }

    /// Total number of ships in the fleet.
    pub fn total_ships(&self) -> i64 {
        self.ships.values().sum()
    }

    /// Total cargo capacity across all ships.
    pub fn cargo_capacity(&self) -> i64 {
        self.ships
            .iter()
            .map(|(ship_type, &count)| {
                get_ship_stats(ship_type)
                    .map(|s| s.cargo * count)
                    .unwrap_or(0)
            })
            .sum()
    }

    /// Minimum speed in the fleet (fleet travels at slowest ship speed).
    /// Returns 0.0 if the fleet is empty or all ships have zero speed.
    pub fn min_speed(&self) -> f64 {
        let mut min = f64::INFINITY;
        for (ship_type, &count) in &self.ships {
            if count <= 0 {
                continue;
            }
            if let Some(stats) = get_ship_stats(ship_type) {
                if stats.speed > 0.0 {
                    min = min.min(stats.speed);
                }
            }
        }
        if min.is_finite() {
            min
        } else {
            0.0
        }
    }

    /// Sum of base fuel consumption rates across all ships.
    pub fn fuel_consumption(&self) -> f64 {
        self.ships
            .iter()
            .map(|(ship_type, &count)| {
                get_ship_stats(ship_type)
                    .map(|s| s.fuel_consumption * count as f64)
                    .unwrap_or(0.0)
            })
            .sum()
    }

    /// Aggregate combat power (sum of weapon * count for all ships).
    pub fn combat_power(&self) -> f64 {
        self.ships
            .iter()
            .map(|(ship_type, &count)| {
                get_ship_stats(ship_type)
                    .map(|s| s.weapon * count as f64)
                    .unwrap_or(0.0)
            })
            .sum()
    }

    /// Returns true if the composition has no ships.
    pub fn is_empty(&self) -> bool {
        self.total_ships() <= 0
    }
}

/// Hard launch bound shared by the planner, persistence layer, and combat
/// adapter. The combat engine currently represents per-type counts as `i32`,
/// so an accepted fleet must remain comfortably below that boundary.
pub const MAX_AUTHORITATIVE_SHIPS_PER_TYPE: i64 = 1_000_000_000;
pub const MAX_AUTHORITATIVE_TRAVEL_SECONDS: i64 = 10 * 365 * 24 * 60 * 60;

/// Server-owned movement configuration. Multipliers use thousandths so the
/// persisted plan can be reproduced without floating-point configuration
/// drift. A value of 1_000 means 1.0x.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FleetPlanningConfig {
    pub universe_speed: i32,
    pub speed_percent: i32,
    pub fuel_multiplier_milli: i32,
    pub cargo_multiplier_milli: i32,
    pub max_galaxies: i32,
    pub max_systems: i32,
    pub max_positions: i32,
    /// Requested orbit duration for ACS defense/join missions. All other
    /// missions require zero. The repository derives this from validated
    /// server/API input and persists it as an immutable launch fact.
    pub hold_seconds: i64,
}

impl Default for FleetPlanningConfig {
    fn default() -> Self {
        Self {
            universe_speed: 1,
            speed_percent: 100,
            fuel_multiplier_milli: 1_000,
            cargo_multiplier_milli: 1_000,
            max_galaxies: 9,
            max_systems: 499,
            max_positions: 15,
            hold_seconds: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuthoritativeMissionPlan {
    pub distance: i32,
    pub fleet_speed: i64,
    pub travel_time_seconds: i64,
    pub fuel_required: i64,
    pub movement_fuel_required: i64,
    pub holding_fuel_required: i64,
    pub cargo_capacity: i64,
    pub usable_cargo_capacity: i64,
    pub applied_max_galaxies: i32,
    pub applied_max_systems: i32,
    pub applied_max_positions: i32,
    pub applied_universe_speed: i32,
    pub applied_speed_percent: i32,
    pub applied_fuel_multiplier_milli: i32,
    pub applied_cargo_multiplier_milli: i32,
    pub applied_hold_seconds: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissionPlanningError(pub String);

impl fmt::Display for MissionPlanningError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Produce the complete movement plan from server-owned configuration and a
/// canonical ship composition. Clients never supply speed, fuel, distance,
/// duration, or cargo capacity.
pub fn plan_authoritative_mission(
    mission_type: FleetMissionType,
    target_kind: FleetTargetKind,
    origin: &Coordinates,
    target: &Coordinates,
    composition: &FleetComposition,
    resources: &Resources,
    config: FleetPlanningConfig,
) -> Result<AuthoritativeMissionPlan, MissionPlanningError> {
    if !mission_type.target_kind_allowed(target_kind) {
        return Err(MissionPlanningError(format!(
            "{} missions cannot target {}",
            mission_type.as_str(),
            target_kind.as_str()
        )));
    }
    if !(1..=1_000).contains(&config.universe_speed)
        || !(10..=100).contains(&config.speed_percent)
        || !(1..=100_000).contains(&config.fuel_multiplier_milli)
        || !(1..=100_000).contains(&config.cargo_multiplier_milli)
        || !(1..=1_000).contains(&config.max_galaxies)
        || !(1..=100_000).contains(&config.max_systems)
        || !(1..=10_000).contains(&config.max_positions)
    {
        return Err(MissionPlanningError(
            "fleet planning configuration is outside supported bounds".to_string(),
        ));
    }
    match mission_type {
        FleetMissionType::AcsDefend if !(60..=48 * 60 * 60).contains(&config.hold_seconds) => {
            return Err(MissionPlanningError(
                "ACS defense hold time must be between 60 seconds and 48 hours".to_string(),
            ));
        }
        FleetMissionType::AcsDefend => {}
        _ if config.hold_seconds != 0 => {
            return Err(MissionPlanningError(
                "hold time is only valid for ACS defense missions".to_string(),
            ));
        }
        _ => {}
    }
    validate_authoritative_coordinates(origin, false, config)?;
    validate_authoritative_coordinates(
        target,
        matches!(target_kind, FleetTargetKind::ExpeditionSlot),
        config,
    )?;
    if matches!(target_kind, FleetTargetKind::ExpeditionSlot)
        && target.position != config.max_positions + 1
    {
        return Err(MissionPlanningError(
            "expedition missions must target the configured expedition position".to_string(),
        ));
    }
    if !matches!(target_kind, FleetTargetKind::ExpeditionSlot)
        && target.position > config.max_positions
    {
        return Err(MissionPlanningError(
            "only expedition missions may target the expedition position".to_string(),
        ));
    }
    if resources.metal < 0 || resources.crystal < 0 || resources.deuterium < 0 {
        return Err(MissionPlanningError(
            "fleet cargo resources cannot be negative".to_string(),
        ));
    }
    let mut total_ships = 0_i128;
    let mut base_fuel = 0_i128;
    let mut base_cargo = 0_i128;
    let mut fleet_speed = i64::MAX;
    for (ship_type, count) in &composition.ships {
        if *count <= 0 || *count > MAX_AUTHORITATIVE_SHIPS_PER_TYPE {
            return Err(MissionPlanningError(format!(
                "ship count for {ship_type} is outside supported bounds"
            )));
        }
        let stats = get_ship_stats(ship_type)
            .ok_or_else(|| MissionPlanningError(format!("unknown ship type: {ship_type}")))?;
        if !stats.speed.is_finite() || stats.speed <= 0.0 || stats.speed.fract() != 0.0 {
            return Err(MissionPlanningError(format!(
                "{ship_type} cannot participate in a moving fleet"
            )));
        }
        if !stats.fuel_consumption.is_finite()
            || stats.fuel_consumption < 0.0
            || stats.fuel_consumption.fract() != 0.0
        {
            return Err(MissionPlanningError(format!(
                "{ship_type} has unsupported fuel statistics"
            )));
        }
        let count = i128::from(*count);
        total_ships = total_ships
            .checked_add(count)
            .ok_or_else(|| MissionPlanningError("fleet size overflow".to_string()))?;
        base_fuel = base_fuel
            .checked_add(
                (stats.fuel_consumption as i128)
                    .checked_mul(count)
                    .ok_or_else(|| MissionPlanningError("fleet fuel overflow".to_string()))?,
            )
            .ok_or_else(|| MissionPlanningError("fleet fuel overflow".to_string()))?;
        base_cargo =
            base_cargo
                .checked_add(i128::from(stats.cargo).checked_mul(count).ok_or_else(|| {
                    MissionPlanningError("fleet cargo capacity overflow".to_string())
                })?)
                .ok_or_else(|| MissionPlanningError("fleet cargo capacity overflow".to_string()))?;
        fleet_speed = fleet_speed.min(stats.speed as i64);
    }
    if total_ships == 0 || fleet_speed == i64::MAX {
        return Err(MissionPlanningError("fleet has no ships".to_string()));
    }

    require_mission_ship(mission_type, composition)?;
    let distance = calculate_distance(
        origin.galaxy,
        origin.system,
        origin.position,
        target.galaxy,
        target.system,
        target.position,
    );
    // OGame-style speed-dependent consumption. With speed selection expressed
    // as 10..100 percent, the exact factor is `(speed_percent + 100)^2 / 10000`:
    // 1.21x at 10%, 2.25x at 50%, and 4.00x at 100%.
    let speed_fuel_factor = i128::from(config.speed_percent + 100)
        .checked_mul(i128::from(config.speed_percent + 100))
        .ok_or_else(|| MissionPlanningError("fleet fuel overflow".to_string()))?;
    let fuel_numerator = base_fuel
        .checked_mul(i128::from(distance))
        .and_then(|value| value.checked_mul(i128::from(config.fuel_multiplier_milli)))
        .and_then(|value| value.checked_mul(speed_fuel_factor))
        .ok_or_else(|| MissionPlanningError("fleet fuel overflow".to_string()))?;
    let movement_fuel_required =
        checked_i128_to_i64(ceil_div(fuel_numerator, 35_000 * 1_000 * 10_000))?;
    // Orbit support consumes 10% of the fleet's configured base consumption
    // per hour, rounded up. It is zero for non-holding missions.
    let holding_fuel_required = checked_i128_to_i64(ceil_div(
        base_fuel
            .checked_mul(i128::from(config.fuel_multiplier_milli))
            .and_then(|value| value.checked_mul(i128::from(config.hold_seconds)))
            .ok_or_else(|| MissionPlanningError("fleet holding fuel overflow".to_string()))?,
        1_000 * 10 * 3_600,
    ))?;
    let fuel_required = movement_fuel_required
        .checked_add(holding_fuel_required)
        .ok_or_else(|| MissionPlanningError("fleet fuel overflow".to_string()))?;
    let cargo_capacity = checked_i128_to_i64(
        base_cargo
            .checked_mul(i128::from(config.cargo_multiplier_milli))
            .ok_or_else(|| MissionPlanningError("fleet cargo capacity overflow".to_string()))?
            / 1_000,
    )?;
    let usable_cargo_capacity = cargo_capacity.saturating_sub(fuel_required);
    let cargo_total = resources
        .metal
        .checked_add(resources.crystal)
        .and_then(|value| value.checked_add(resources.deuterium))
        .ok_or_else(|| MissionPlanningError("fleet cargo overflow".to_string()))?;
    if cargo_total > usable_cargo_capacity {
        return Err(MissionPlanningError(format!(
            "fleet cargo exceeds usable capacity of {usable_cargo_capacity}"
        )));
    }

    // Fixed-point form of:
    // (10 + 3_500_000/speed_percent * sqrt(distance*10/fleet_speed))
    // / universe_speed. The square root is rounded upward at one-millionth
    // precision so the server never schedules an arrival earlier than the
    // mathematical result and every platform produces the same second.
    const SQRT_SCALE: i128 = 1_000_000;
    let scaled_radicand = i128::from(distance)
        .checked_mul(10)
        .and_then(|value| value.checked_mul(SQRT_SCALE))
        .and_then(|value| value.checked_mul(SQRT_SCALE))
        .ok_or_else(|| MissionPlanningError("fleet duration overflow".to_string()))?;
    let scaled_radicand = ceil_div(scaled_radicand, i128::from(fleet_speed));
    let sqrt_scaled = integer_sqrt_ceil(
        u128::try_from(scaled_radicand)
            .map_err(|_| MissionPlanningError("fleet duration overflow".to_string()))?,
    );
    let duration_denominator = i128::from(config.speed_percent)
        .checked_mul(SQRT_SCALE)
        .ok_or_else(|| MissionPlanningError("fleet duration overflow".to_string()))?;
    let duration_numerator = i128::from(10)
        .checked_mul(duration_denominator)
        .and_then(|value| {
            i128::try_from(sqrt_scaled)
                .ok()
                .and_then(|root| root.checked_mul(3_500_000))
                .and_then(|term| value.checked_add(term))
        })
        .ok_or_else(|| MissionPlanningError("fleet duration overflow".to_string()))?;
    let travel_time_seconds = checked_i128_to_i64(ceil_div(
        duration_numerator,
        duration_denominator
            .checked_mul(i128::from(config.universe_speed))
            .ok_or_else(|| MissionPlanningError("fleet duration overflow".to_string()))?,
    ))?;
    if travel_time_seconds <= 0 || travel_time_seconds > MAX_AUTHORITATIVE_TRAVEL_SECONDS {
        return Err(MissionPlanningError(
            "fleet travel duration is outside supported bounds".to_string(),
        ));
    }

    Ok(AuthoritativeMissionPlan {
        distance,
        fleet_speed,
        travel_time_seconds,
        fuel_required,
        movement_fuel_required,
        holding_fuel_required,
        cargo_capacity,
        usable_cargo_capacity,
        applied_max_galaxies: config.max_galaxies,
        applied_max_systems: config.max_systems,
        applied_max_positions: config.max_positions,
        applied_universe_speed: config.universe_speed,
        applied_speed_percent: config.speed_percent,
        applied_fuel_multiplier_milli: config.fuel_multiplier_milli,
        applied_cargo_multiplier_milli: config.cargo_multiplier_milli,
        applied_hold_seconds: config.hold_seconds,
    })
}

/// Return-flight duration for a recall. The fleet returns for exactly the time
/// it has already travelled. A recall at or after arrival is rejected because
/// arrival resolution and recall must serialize on the persisted mission.
pub fn recall_return_duration_seconds(
    departed_at_unix: i64,
    arrives_at_unix: i64,
    recalled_at_unix: i64,
) -> Result<i64, MissionPlanningError> {
    if arrives_at_unix <= departed_at_unix
        || recalled_at_unix < departed_at_unix
        || recalled_at_unix >= arrives_at_unix
    {
        return Err(MissionPlanningError(
            "invalid mission timestamps for recall".to_string(),
        ));
    }
    Ok(recalled_at_unix - departed_at_unix)
}

fn validate_authoritative_coordinates(
    coordinates: &Coordinates,
    expedition: bool,
    config: FleetPlanningConfig,
) -> Result<(), MissionPlanningError> {
    let maximum_position = config.max_positions + i32::from(expedition);
    if !(1..=config.max_galaxies).contains(&coordinates.galaxy)
        || !(1..=config.max_systems).contains(&coordinates.system)
        || !(1..=maximum_position).contains(&coordinates.position)
    {
        return Err(MissionPlanningError(
            "fleet coordinates are outside universe bounds".to_string(),
        ));
    }
    Ok(())
}

fn require_mission_ship(
    mission_type: FleetMissionType,
    composition: &FleetComposition,
) -> Result<(), MissionPlanningError> {
    let required = match mission_type {
        FleetMissionType::Espionage => Some(("espionage_probe", "espionage probe")),
        FleetMissionType::Colonize => Some(("colony_ship", "colony ship")),
        FleetMissionType::Harvest => Some(("recycler", "recycler")),
        FleetMissionType::Destroy => Some(("deathstar", "deathstar")),
        _ => None,
    };
    if let Some((ship_type, description)) = required {
        if composition.ships.get(ship_type).copied().unwrap_or(0) <= 0 {
            return Err(MissionPlanningError(format!(
                "{} missions require at least one {description}",
                mission_type.as_str()
            )));
        }
    }
    Ok(())
}

fn ceil_div(value: i128, divisor: i128) -> i128 {
    if value == 0 {
        0
    } else {
        (value - 1) / divisor + 1
    }
}

fn integer_sqrt_ceil(value: u128) -> u128 {
    if value <= 1 {
        return value;
    }
    let mut low = 1_u128;
    let mut high = value.min(u128::from(u64::MAX));
    while low < high {
        let middle = low + (high - low) / 2;
        if middle > value / middle {
            high = middle;
        } else if middle * middle == value {
            return middle;
        } else {
            low = middle + 1;
        }
    }
    low
}

fn checked_i128_to_i64(value: i128) -> Result<i64, MissionPlanningError> {
    i64::try_from(value)
        .map_err(|_| MissionPlanningError("fleet calculation exceeds i64".to_string()))
}

impl Default for FleetComposition {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Expedition Variants
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExpeditionVariant {
    ResourceFind {
        metal: i64,
        crystal: i64,
        deuterium: i64,
    },
    FleetFind {
        ship_type: String,
        count: i64,
    },
    DarkMatterFind {
        amount: i64,
    },
    Nothing,
    Pirates,
    Aliens,
    BlackHole,
    Delay {
        extra_seconds: i64,
    },
}

// ---------------------------------------------------------------------------
// Mission Outcome
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MissionOutcome {
    AttackResult {
        loot: Resources,
        debris_metal: i64,
        debris_crystal: i64,
    },
    TransportDelivered {
        resources: Resources,
    },
    ColonyEstablished {
        planet_position: Coordinates,
    },
    EspionageComplete {
        report_level: i32,
    },
    HarvestCollected {
        metal: i64,
        crystal: i64,
    },
    ExpeditionResult {
        variant: ExpeditionVariant,
    },
    DeployComplete,
    Recalled,
}

// ---------------------------------------------------------------------------
// Fleet Mission
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FleetMission {
    pub id: u64,
    pub owner_id: String,
    pub mission_type: FleetMissionType,
    pub origin: Coordinates,
    pub target: Coordinates,
    pub composition: FleetComposition,
    pub resources_carried: Resources,
    pub departure_time: String,
    pub arrival_time: String,
    pub return_time: String,
    pub status: MissionStatus,
    pub fuel_consumed: f64,
}

impl FleetMission {
    /// Check whether the fleet has arrived at its target given `now` in epoch
    /// seconds.
    pub fn is_arrived(&self, now: i64) -> bool {
        let arrival = parse_iso_epoch(&self.arrival_time);
        now >= arrival
    }

    /// Check whether the fleet has returned to origin given `now` in epoch
    /// seconds.
    pub fn is_returned(&self, now: i64) -> bool {
        let ret = parse_iso_epoch(&self.return_time);
        now >= ret
    }

    /// Seconds remaining until arrival (0 if already arrived).
    pub fn time_remaining(&self, now: i64) -> i64 {
        let arrival = parse_iso_epoch(&self.arrival_time);
        if now >= arrival {
            0
        } else {
            arrival - now
        }
    }

    /// Recall the fleet. Sets status to `Recalled` and adjusts return time to
    /// be symmetric around `now` based on how far the fleet has traveled.
    pub fn recall(&mut self, now: i64) {
        let departure = parse_iso_epoch(&self.departure_time);
        let arrival = parse_iso_epoch(&self.arrival_time);

        // Time elapsed since departure (clamped to full trip duration)
        let elapsed = (now - departure).max(0).min(arrival - departure);
        let new_return = now + elapsed;

        self.status = MissionStatus::Recalled;
        self.return_time = epoch_to_iso(new_return);
    }
}

// ---------------------------------------------------------------------------
// ISO 8601 timestamp helpers (no chrono dependency)
// ---------------------------------------------------------------------------

/// Parse a subset of ISO 8601 into Unix epoch seconds.
/// Accepts `YYYY-MM-DDTHH:MM:SSZ` or plain epoch-second strings.
pub fn parse_iso_epoch(s: &str) -> i64 {
    // Fast path: if it looks like a plain integer, parse directly.
    if let Ok(epoch) = s.parse::<i64>() {
        return epoch;
    }

    // Manual parse of YYYY-MM-DDTHH:MM:SSZ
    let s = s.trim().trim_end_matches('Z');
    let parts: Vec<&str> = s.split('T').collect();
    if parts.len() != 2 {
        return 0;
    }
    let date_parts: Vec<i64> = parts[0].split('-').filter_map(|p| p.parse().ok()).collect();
    let time_parts: Vec<i64> = parts[1].split(':').filter_map(|p| p.parse().ok()).collect();
    if date_parts.len() != 3 || time_parts.len() < 3 {
        return 0;
    }

    let year = date_parts[0];
    let month = date_parts[1];
    let day = date_parts[2];
    let hour = time_parts[0];
    let minute = time_parts[1];
    let second = time_parts[2];

    // Days from year 1970 to start of `year`
    let mut total_days: i64 = 0;
    // Approximate: accumulate year by year from 1970
    for y in 1970..year {
        total_days += if is_leap_year(y) { 366 } else { 365 };
    }

    let days_in_months = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    for m in 1..month {
        total_days += days_in_months[(m - 1) as usize] as i64;
        if m == 2 && is_leap_year(year) {
            total_days += 1;
        }
    }
    total_days += day - 1;

    total_days * 86400 + hour * 3600 + minute * 60 + second
}

fn is_leap_year(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}

/// Convert epoch seconds to ISO 8601 `YYYY-MM-DDTHH:MM:SSZ`.
pub fn epoch_to_iso(epoch: i64) -> String {
    let mut remaining = epoch;
    let second = remaining % 60;
    remaining /= 60;
    let minute = remaining % 60;
    remaining /= 60;
    let hour = remaining % 24;
    remaining /= 24;

    // remaining is now days since 1970-01-01
    let mut year: i64 = 1970;
    loop {
        let days_in_year: i64 = if is_leap_year(year) { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        year += 1;
    }

    let days_in_months: [i64; 12] = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month: i64 = 1;
    for &dim in &days_in_months {
        if remaining < dim {
            break;
        }
        remaining -= dim;
        month += 1;
    }
    let day = remaining + 1;

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hour, minute, second
    )
}

// ---------------------------------------------------------------------------
// Fleet Dispatcher
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct FleetDispatcher {
    /// Universe speed multiplier (default 1).
    pub speed_factor: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DispatchError(pub String);

impl fmt::Display for DispatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FleetDispatcher {
    pub fn new(speed_factor: f64) -> Self {
        Self { speed_factor }
    }

    /// Validate a mission before dispatch.
    pub fn validate_mission(
        &self,
        mission_type: FleetMissionType,
        composition: &FleetComposition,
    ) -> Result<(), DispatchError> {
        if composition.is_empty() {
            return Err(DispatchError("fleet has no ships".to_string()));
        }

        // Espionage requires at least one espionage probe
        if mission_type == FleetMissionType::Espionage {
            let probe_count = composition
                .ships
                .get("espionage_probe")
                .copied()
                .unwrap_or(0);
            if probe_count <= 0 {
                return Err(DispatchError(
                    "espionage missions require at least one espionage probe".to_string(),
                ));
            }
        }

        // Colonize requires a colony ship
        if mission_type == FleetMissionType::Colonize {
            let colony_count = composition.ships.get("colony_ship").copied().unwrap_or(0);
            if colony_count <= 0 {
                return Err(DispatchError(
                    "colonize missions require at least one colony ship".to_string(),
                ));
            }
        }

        // Harvest requires a recycler
        if mission_type == FleetMissionType::Harvest {
            let recycler_count = composition.ships.get("recycler").copied().unwrap_or(0);
            if recycler_count <= 0 {
                return Err(DispatchError(
                    "harvest missions require at least one recycler".to_string(),
                ));
            }
        }

        // Destroy (moon destruction) requires a deathstar
        if mission_type == FleetMissionType::Destroy {
            let ds_count = composition.ships.get("deathstar").copied().unwrap_or(0);
            if ds_count <= 0 {
                return Err(DispatchError(
                    "destroy missions require at least one deathstar".to_string(),
                ));
            }
        }

        // Speed check: fleet must be able to move
        if composition.min_speed() <= 0.0 {
            return Err(DispatchError(
                "fleet has no speed (all ships have zero speed)".to_string(),
            ));
        }

        Ok(())
    }

    /// Legacy compatibility estimate. Authoritative launches use
    /// `plan_authoritative_mission`, whose checked fixed-point formula includes
    /// selected speed, configured multipliers, cargo occupancy, and hold fuel.
    pub fn calculate_fuel(&self, composition: &FleetComposition, distance: i32) -> f64 {
        let base_consumption = composition.fuel_consumption();
        // OGame-inspired: fuel = base_consumption * distance / 35000
        // Speed factor affects travel time, not fuel directly in basic model.
        base_consumption * (distance as f64) / 35000.0
    }

    /// Travel time in seconds considering the speed factor.
    pub fn calculate_travel_time(&self, composition: &FleetComposition, distance: i32) -> i64 {
        let speed = composition.min_speed();
        if speed <= 0.0 {
            return 0;
        }
        let factor = if self.speed_factor > 0.0 {
            self.speed_factor
        } else {
            1.0
        };
        ((10.0 + (35000.0 / factor) * ((distance as f64 * 10.0) / speed).sqrt()) / factor).ceil()
            as i64
    }

    /// Dispatch a fleet mission. Returns a `FleetMission` with computed times.
    // Compatibility entry point retained for existing callers. New durable
    // launches use `plan_authoritative_mission` plus repository-owned input.
    #[allow(clippy::too_many_arguments)]
    pub fn dispatch(
        &self,
        id: u64,
        owner_id: &str,
        mission_type: FleetMissionType,
        origin: Coordinates,
        target: Coordinates,
        composition: FleetComposition,
        resources_carried: Resources,
        departure_epoch: i64,
    ) -> Result<FleetMission, DispatchError> {
        self.validate_mission(mission_type, &composition)?;

        let distance = calculate_distance(
            origin.galaxy,
            origin.system,
            origin.position,
            target.galaxy,
            target.system,
            target.position,
        );

        let travel_seconds = self.calculate_travel_time(&composition, distance);
        let fuel = self.calculate_fuel(&composition, distance);

        let arrival_epoch = departure_epoch + travel_seconds;
        // Deploy missions are one-way
        let return_epoch = if mission_type == FleetMissionType::Deploy {
            arrival_epoch
        } else {
            arrival_epoch + travel_seconds
        };

        Ok(FleetMission {
            id,
            owner_id: owner_id.to_string(),
            mission_type,
            origin,
            target,
            composition,
            resources_carried,
            departure_time: epoch_to_iso(departure_epoch),
            arrival_time: epoch_to_iso(arrival_epoch),
            return_time: epoch_to_iso(return_epoch),
            status: MissionStatus::Outbound,
            fuel_consumed: fuel,
        })
    }
}

impl Default for FleetDispatcher {
    fn default() -> Self {
        Self::new(1.0)
    }
}

// ---------------------------------------------------------------------------
// Mission Processing
// ---------------------------------------------------------------------------

/// Legacy deterministic benchmark fixture.
///
/// Production mission resolution is exclusively implemented by the durable
/// `platform-db` fleet repository. This helper is compiled only for this
/// crate's unit tests and the explicitly opted-in benchmark harness.
#[cfg(any(test, feature = "legacy-benchmark-fixture"))]
#[doc(hidden)]
pub fn process_arrival(mission: &FleetMission) -> MissionOutcome {
    match mission.mission_type {
        FleetMissionType::Attack | FleetMissionType::AcsAttack | FleetMissionType::AcsJoin => {
            // Simplified: loot proportional to combat power, small debris
            let power = mission.composition.combat_power();
            let loot_factor = (power / 10.0).ceil() as i64;
            MissionOutcome::AttackResult {
                loot: Resources::new(loot_factor, loot_factor / 2, loot_factor / 4),
                debris_metal: loot_factor / 3,
                debris_crystal: loot_factor / 6,
            }
        }
        FleetMissionType::Transport => MissionOutcome::TransportDelivered {
            resources: mission.resources_carried.clone(),
        },
        FleetMissionType::Deploy | FleetMissionType::AcsDefend => MissionOutcome::DeployComplete,
        FleetMissionType::Espionage => {
            // Report level based on number of probes
            let probes = mission
                .composition
                .ships
                .get("espionage_probe")
                .copied()
                .unwrap_or(1) as i32;
            let level = probes.min(8);
            MissionOutcome::EspionageComplete {
                report_level: level,
            }
        }
        FleetMissionType::Colonize => MissionOutcome::ColonyEstablished {
            planet_position: mission.target.clone(),
        },
        FleetMissionType::Harvest => {
            let cargo = mission.composition.cargo_capacity();
            // Split harvest 60/40 metal/crystal
            MissionOutcome::HarvestCollected {
                metal: (cargo as f64 * 0.6) as i64,
                crystal: (cargo as f64 * 0.4) as i64,
            }
        }
        FleetMissionType::Expedition => {
            // Deterministic placeholder: resource find
            let ships = mission.composition.total_ships();
            MissionOutcome::ExpeditionResult {
                variant: ExpeditionVariant::ResourceFind {
                    metal: ships * 100,
                    crystal: ships * 50,
                    deuterium: ships * 25,
                },
            }
        }
        FleetMissionType::Destroy => {
            // Moon destruction simplified outcome
            let power = mission.composition.combat_power();
            MissionOutcome::AttackResult {
                loot: Resources::zero(),
                debris_metal: (power / 5.0) as i64,
                debris_crystal: (power / 10.0) as i64,
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Fleet Store (in-memory)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
#[cfg(any(test, feature = "legacy-benchmark-fixture"))]
#[doc(hidden)]
pub struct FleetStore {
    pub fleets: HashMap<u64, FleetMission>,
    next_id: u64,
    dispatcher: FleetDispatcher,
}

#[cfg(any(test, feature = "legacy-benchmark-fixture"))]
impl FleetStore {
    pub fn new(speed_factor: f64) -> Self {
        Self {
            fleets: HashMap::new(),
            next_id: 1,
            dispatcher: FleetDispatcher::new(speed_factor),
        }
    }

    /// Dispatch a fleet and store it. Returns the assigned fleet id.
    // Compatibility wrapper around the legacy in-memory store.
    #[allow(clippy::too_many_arguments)]
    pub fn dispatch_fleet(
        &mut self,
        owner_id: &str,
        mission_type: FleetMissionType,
        origin: Coordinates,
        target: Coordinates,
        composition: FleetComposition,
        resources_carried: Resources,
        departure_epoch: i64,
    ) -> Result<u64, DispatchError> {
        let id = self.next_id;
        let mission = self.dispatcher.dispatch(
            id,
            owner_id,
            mission_type,
            origin,
            target,
            composition,
            resources_carried,
            departure_epoch,
        )?;
        self.fleets.insert(id, mission);
        self.next_id += 1;
        Ok(id)
    }

    /// Get a fleet by id.
    pub fn get_fleet(&self, id: u64) -> Option<&FleetMission> {
        self.fleets.get(&id)
    }

    /// Get a mutable reference to a fleet by id.
    pub fn get_fleet_mut(&mut self, id: u64) -> Option<&mut FleetMission> {
        self.fleets.get_mut(&id)
    }

    /// List all fleets owned by a player.
    pub fn list_player_fleets(&self, owner_id: &str) -> Vec<&FleetMission> {
        self.fleets
            .values()
            .filter(|f| f.owner_id == owner_id)
            .collect()
    }

    /// List fleets heading toward the given coordinates that haven't completed.
    pub fn list_incoming_fleets(&self, target: &Coordinates) -> Vec<&FleetMission> {
        self.fleets
            .values()
            .filter(|f| {
                f.target == *target
                    && (f.status == MissionStatus::Outbound || f.status == MissionStatus::Arrived)
            })
            .collect()
    }

    /// Process all fleets whose arrival time is <= `now` and that are still
    /// outbound. Returns a list of (fleet_id, outcome) pairs.
    pub fn process_due_arrivals(&mut self, now: i64) -> Vec<(u64, MissionOutcome)> {
        let mut results = Vec::new();

        // Collect ids of due fleets first to avoid borrow issues
        let due_ids: Vec<u64> = self
            .fleets
            .iter()
            .filter(|(_, f)| f.status == MissionStatus::Outbound && f.is_arrived(now))
            .map(|(&id, _)| id)
            .collect();

        for id in due_ids {
            let fleet = self.fleets.get(&id).unwrap();
            let outcome = process_arrival(fleet);

            // Update status
            let fleet_mut = self.fleets.get_mut(&id).unwrap();
            if fleet_mut.mission_type == FleetMissionType::Deploy || fleet_mut.is_returned(now) {
                fleet_mut.status = MissionStatus::Completed;
            } else {
                fleet_mut.status = MissionStatus::Returning;
            }

            results.push((id, outcome));
        }

        // Also complete any returning fleets that have arrived back
        let returning_done: Vec<u64> = self
            .fleets
            .iter()
            .filter(|(_, f)| f.status == MissionStatus::Returning && f.is_returned(now))
            .map(|(&id, _)| id)
            .collect();

        for id in returning_done {
            let fleet_mut = self.fleets.get_mut(&id).unwrap();
            fleet_mut.status = MissionStatus::Completed;
        }

        results
    }

    /// Recall a fleet. Returns error if fleet is not outbound.
    pub fn recall_fleet(&mut self, id: u64, now: i64) -> Result<(), DispatchError> {
        let fleet = self
            .fleets
            .get_mut(&id)
            .ok_or_else(|| DispatchError(format!("fleet {} not found", id)))?;

        if fleet.status != MissionStatus::Outbound {
            return Err(DispatchError(format!(
                "fleet {} cannot be recalled (status: {})",
                id, fleet.status
            )));
        }

        fleet.recall(now);
        Ok(())
    }

    /// Count active (non-completed, non-recalled) fleets for a player.
    pub fn count_active_fleets(&self, owner_id: &str) -> usize {
        self.fleets
            .values()
            .filter(|f| {
                f.owner_id == owner_id
                    && f.status != MissionStatus::Completed
                    && f.status != MissionStatus::Recalled
            })
            .count()
    }
}

#[cfg(any(test, feature = "legacy-benchmark-fixture"))]
impl Default for FleetStore {
    fn default() -> Self {
        Self::new(1.0)
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // --- Original tests (preserved) ---

    #[test]
    fn distance_uses_expected_tiers() {
        assert_eq!(calculate_distance(1, 1, 1, 2, 1, 1), 20000);
        assert_eq!(calculate_distance(1, 1, 1, 1, 2, 1), 2795);
        assert_eq!(calculate_distance(1, 1, 1, 1, 1, 2), 1005);
    }

    #[test]
    fn movement_sanity_matches_backend_formula() {
        let input = FleetMovementInput {
            origin_galaxy: 1,
            origin_system: 1,
            origin_position: 1,
            target_galaxy: 1,
            target_system: 2,
            target_position: 1,
            ships: vec![
                FleetShipInput {
                    count: 10,
                    base_speed: 1000.0,
                    fuel_consumption: 2.0,
                    cargo: 50.0,
                },
                FleetShipInput {
                    count: 1,
                    base_speed: 500.0,
                    fuel_consumption: 5.0,
                    cargo: 100.0,
                },
            ],
        };

        let result = calculate_movement(&input);

        assert_eq!(result.distance, 2795);
        assert_eq!(result.fleet_speed, 500.0);
        assert_eq!(result.travel_time_seconds, 20124);
        assert!((result.fuel_needed - 698.75).abs() < 1e-9);
        assert!((result.cargo_capacity - (-98.75)).abs() < 1e-9);
    }

    // --- Coordinates ---

    #[test]
    fn coordinates_display() {
        let c = Coordinates::new(1, 200, 8);
        assert_eq!(format!("{}", c), "[1:200:8]");
    }

    #[test]
    fn coordinates_equality() {
        let a = Coordinates::new(1, 2, 3);
        let b = Coordinates::new(1, 2, 3);
        let c = Coordinates::new(1, 2, 4);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    // --- Resources ---

    #[test]
    fn resources_total() {
        let r = Resources::new(100, 200, 50);
        assert_eq!(r.total(), 350);
    }

    #[test]
    fn resources_zero() {
        let r = Resources::zero();
        assert_eq!(r.total(), 0);
    }

    // --- FleetMissionType ---

    #[test]
    fn mission_type_display_roundtrip() {
        let types = vec![
            FleetMissionType::Attack,
            FleetMissionType::Transport,
            FleetMissionType::Deploy,
            FleetMissionType::Espionage,
            FleetMissionType::Colonize,
            FleetMissionType::Harvest,
            FleetMissionType::Expedition,
            FleetMissionType::Destroy,
            FleetMissionType::AcsAttack,
            FleetMissionType::AcsDefend,
        ];
        for mt in types {
            let s = mt.to_string();
            let parsed: FleetMissionType = s.parse().unwrap();
            assert_eq!(parsed, mt);
        }
    }

    #[test]
    fn mission_type_from_str_aliases() {
        assert_eq!(
            "spy".parse::<FleetMissionType>().unwrap(),
            FleetMissionType::Espionage
        );
        assert_eq!(
            "recycle".parse::<FleetMissionType>().unwrap(),
            FleetMissionType::Harvest
        );
        assert_eq!(
            "explore".parse::<FleetMissionType>().unwrap(),
            FleetMissionType::Expedition
        );
        assert_eq!(
            "moon_destroy".parse::<FleetMissionType>().unwrap(),
            FleetMissionType::Destroy
        );
        assert_eq!(
            "acs_attack".parse::<FleetMissionType>().unwrap(),
            FleetMissionType::AcsAttack
        );
        assert_eq!(
            "colonise".parse::<FleetMissionType>().unwrap(),
            FleetMissionType::Colonize
        );
    }

    #[test]
    fn mission_type_from_str_invalid() {
        assert!("nonsense".parse::<FleetMissionType>().is_err());
    }

    // --- MissionStatus ---

    #[test]
    fn mission_status_display() {
        assert_eq!(MissionStatus::Outbound.to_string(), "Outbound");
        assert_eq!(MissionStatus::Completed.to_string(), "Completed");
    }

    // --- ShipStats ---

    #[test]
    fn ship_stats_known_types() {
        for &ship_type in ALL_SHIP_TYPES {
            assert!(
                get_ship_stats(ship_type).is_some(),
                "missing stats for {}",
                ship_type
            );
        }
    }

    #[test]
    fn ship_stats_unknown_returns_none() {
        assert!(get_ship_stats("nonexistent_ship").is_none());
    }

    #[test]
    fn deathstar_has_high_stats() {
        let ds = get_ship_stats("deathstar").unwrap();
        assert!(ds.weapon > 100000.0);
        assert!(ds.hull > 500000.0);
        assert!(ds.cargo >= 1000000);
    }

    #[test]
    fn espionage_probe_is_fast_and_fragile() {
        let ep = get_ship_stats("espionage_probe").unwrap();
        assert!(ep.speed > 1_000_000.0);
        assert!(ep.hull < 200.0);
        assert_eq!(ep.cargo, 0);
    }

    // --- FleetComposition ---

    #[test]
    fn composition_total_ships() {
        let mut comp = FleetComposition::new();
        comp.add("light_fighter", 10);
        comp.add("cruiser", 5);
        assert_eq!(comp.total_ships(), 15);
    }

    #[test]
    fn composition_is_empty() {
        let comp = FleetComposition::new();
        assert!(comp.is_empty());
    }

    #[test]
    fn composition_cargo_capacity() {
        let mut comp = FleetComposition::new();
        comp.add("small_cargo", 2);
        // small_cargo has 5000 cargo each
        assert_eq!(comp.cargo_capacity(), 10000);
    }

    #[test]
    fn composition_min_speed() {
        let mut comp = FleetComposition::new();
        comp.add("light_fighter", 5); // speed 12500
        comp.add("recycler", 1); // speed 2000
        assert!((comp.min_speed() - 2000.0).abs() < 1e-9);
    }

    #[test]
    fn composition_empty_min_speed_is_zero() {
        let comp = FleetComposition::new();
        assert!((comp.min_speed() - 0.0).abs() < 1e-9);
    }

    #[test]
    fn composition_combat_power() {
        let mut comp = FleetComposition::new();
        comp.add("light_fighter", 10); // weapon = 50 each
                                       // 10 * 50 = 500
        assert!((comp.combat_power() - 500.0).abs() < 1e-9);
    }

    #[test]
    fn composition_fuel_consumption() {
        let mut comp = FleetComposition::new();
        comp.add("light_fighter", 3); // 20 each
                                      // 3 * 20 = 60
        assert!((comp.fuel_consumption() - 60.0).abs() < 1e-9);
    }

    // --- ISO 8601 helpers ---

    #[test]
    fn iso_roundtrip() {
        let epoch = 1_700_000_000i64; // 2023-11-14T22:13:20Z
        let iso = epoch_to_iso(epoch);
        let parsed = parse_iso_epoch(&iso);
        assert_eq!(parsed, epoch);
    }

    #[test]
    fn parse_iso_plain_integer() {
        assert_eq!(parse_iso_epoch("1000"), 1000);
    }

    #[test]
    fn epoch_to_iso_known_value() {
        // 2000-01-01T00:00:00Z = 946684800
        let iso = epoch_to_iso(946_684_800);
        assert_eq!(iso, "2000-01-01T00:00:00Z");
    }

    // --- FleetDispatcher ---

    #[test]
    fn dispatcher_validate_empty_fleet_fails() {
        let disp = FleetDispatcher::default();
        let comp = FleetComposition::new();
        let result = disp.validate_mission(FleetMissionType::Attack, &comp);
        assert!(result.is_err());
    }

    #[test]
    fn dispatcher_validate_espionage_without_probe_fails() {
        let disp = FleetDispatcher::default();
        let mut comp = FleetComposition::new();
        comp.add("light_fighter", 5);
        let result = disp.validate_mission(FleetMissionType::Espionage, &comp);
        assert!(result.is_err());
        assert!(result.unwrap_err().0.contains("espionage probe"));
    }

    #[test]
    fn dispatcher_validate_colonize_without_colony_ship_fails() {
        let disp = FleetDispatcher::default();
        let mut comp = FleetComposition::new();
        comp.add("light_fighter", 5);
        let result = disp.validate_mission(FleetMissionType::Colonize, &comp);
        assert!(result.is_err());
    }

    #[test]
    fn dispatcher_validate_harvest_without_recycler_fails() {
        let disp = FleetDispatcher::default();
        let mut comp = FleetComposition::new();
        comp.add("light_fighter", 5);
        let result = disp.validate_mission(FleetMissionType::Harvest, &comp);
        assert!(result.is_err());
    }

    #[test]
    fn dispatcher_validate_destroy_without_deathstar_fails() {
        let disp = FleetDispatcher::default();
        let mut comp = FleetComposition::new();
        comp.add("battleship", 5);
        let result = disp.validate_mission(FleetMissionType::Destroy, &comp);
        assert!(result.is_err());
    }

    #[test]
    fn dispatcher_validate_attack_succeeds() {
        let disp = FleetDispatcher::default();
        let mut comp = FleetComposition::new();
        comp.add("light_fighter", 5);
        assert!(disp
            .validate_mission(FleetMissionType::Attack, &comp)
            .is_ok());
    }

    #[test]
    fn dispatcher_dispatch_creates_mission() {
        let disp = FleetDispatcher::new(1.0);
        let mut comp = FleetComposition::new();
        comp.add("light_fighter", 10);
        let origin = Coordinates::new(1, 1, 1);
        let target = Coordinates::new(1, 2, 1);
        let resources = Resources::zero();

        let mission = disp
            .dispatch(
                1,
                "player1",
                FleetMissionType::Attack,
                origin,
                target,
                comp,
                resources,
                1_000_000,
            )
            .unwrap();

        assert_eq!(mission.id, 1);
        assert_eq!(mission.owner_id, "player1");
        assert_eq!(mission.mission_type, FleetMissionType::Attack);
        assert_eq!(mission.status, MissionStatus::Outbound);
        assert!(mission.fuel_consumed > 0.0);
    }

    #[test]
    fn dispatcher_deploy_has_same_arrival_and_return() {
        let disp = FleetDispatcher::default();
        let mut comp = FleetComposition::new();
        comp.add("light_fighter", 5);
        let mission = disp
            .dispatch(
                1,
                "p1",
                FleetMissionType::Deploy,
                Coordinates::new(1, 1, 1),
                Coordinates::new(1, 1, 3),
                comp,
                Resources::zero(),
                1_000_000,
            )
            .unwrap();
        assert_eq!(mission.arrival_time, mission.return_time);
    }

    // --- FleetMission lifecycle ---

    #[test]
    fn fleet_mission_is_arrived() {
        let disp = FleetDispatcher::default();
        let mut comp = FleetComposition::new();
        comp.add("cruiser", 1);
        let mission = disp
            .dispatch(
                1,
                "p",
                FleetMissionType::Attack,
                Coordinates::new(1, 1, 1),
                Coordinates::new(1, 2, 1),
                comp,
                Resources::zero(),
                0,
            )
            .unwrap();
        let arrival = parse_iso_epoch(&mission.arrival_time);
        assert!(!mission.is_arrived(0));
        assert!(mission.is_arrived(arrival));
        assert!(mission.is_arrived(arrival + 1));
    }

    #[test]
    fn fleet_mission_recall() {
        let disp = FleetDispatcher::default();
        let mut comp = FleetComposition::new();
        comp.add("cruiser", 1);
        let mut mission = disp
            .dispatch(
                1,
                "p",
                FleetMissionType::Attack,
                Coordinates::new(1, 1, 1),
                Coordinates::new(2, 1, 1),
                comp,
                Resources::zero(),
                0,
            )
            .unwrap();

        let arrival = parse_iso_epoch(&mission.arrival_time);
        let recall_time = arrival / 2; // halfway
        mission.recall(recall_time);

        assert_eq!(mission.status, MissionStatus::Recalled);
        let return_epoch = parse_iso_epoch(&mission.return_time);
        // Return should be recall_time + elapsed = recall_time + recall_time = 2*recall_time
        assert_eq!(return_epoch, recall_time * 2);
    }

    #[test]
    fn fleet_mission_time_remaining() {
        let disp = FleetDispatcher::default();
        let mut comp = FleetComposition::new();
        comp.add("light_fighter", 1);
        let mission = disp
            .dispatch(
                1,
                "p",
                FleetMissionType::Transport,
                Coordinates::new(1, 1, 1),
                Coordinates::new(1, 1, 5),
                comp,
                Resources::new(100, 0, 0),
                1000,
            )
            .unwrap();
        let arrival = parse_iso_epoch(&mission.arrival_time);
        assert!(mission.time_remaining(1000) > 0);
        assert_eq!(mission.time_remaining(arrival), 0);
        assert_eq!(mission.time_remaining(arrival + 100), 0);
    }

    // --- process_arrival ---

    #[test]
    fn process_arrival_attack() {
        let disp = FleetDispatcher::default();
        let mut comp = FleetComposition::new();
        comp.add("battleship", 5);
        let mission = disp
            .dispatch(
                1,
                "p",
                FleetMissionType::Attack,
                Coordinates::new(1, 1, 1),
                Coordinates::new(1, 1, 3),
                comp,
                Resources::zero(),
                0,
            )
            .unwrap();
        let outcome = process_arrival(&mission);
        match outcome {
            MissionOutcome::AttackResult {
                loot,
                debris_metal,
                debris_crystal,
            } => {
                assert!(loot.metal > 0);
                assert!(debris_metal > 0);
                assert!(debris_crystal > 0);
            }
            _ => panic!("expected AttackResult"),
        }
    }

    #[test]
    fn process_arrival_transport() {
        let disp = FleetDispatcher::default();
        let mut comp = FleetComposition::new();
        comp.add("small_cargo", 2);
        let res = Resources::new(1000, 500, 100);
        let mission = disp
            .dispatch(
                1,
                "p",
                FleetMissionType::Transport,
                Coordinates::new(1, 1, 1),
                Coordinates::new(1, 1, 2),
                comp,
                res.clone(),
                0,
            )
            .unwrap();
        let outcome = process_arrival(&mission);
        match outcome {
            MissionOutcome::TransportDelivered { resources } => {
                assert_eq!(resources, res);
            }
            _ => panic!("expected TransportDelivered"),
        }
    }

    #[test]
    fn process_arrival_espionage() {
        let disp = FleetDispatcher::default();
        let mut comp = FleetComposition::new();
        comp.add("espionage_probe", 3);
        let mission = disp
            .dispatch(
                1,
                "p",
                FleetMissionType::Espionage,
                Coordinates::new(1, 1, 1),
                Coordinates::new(1, 1, 5),
                comp,
                Resources::zero(),
                0,
            )
            .unwrap();
        let outcome = process_arrival(&mission);
        match outcome {
            MissionOutcome::EspionageComplete { report_level } => {
                assert_eq!(report_level, 3);
            }
            _ => panic!("expected EspionageComplete"),
        }
    }

    #[test]
    fn process_arrival_colonize() {
        let disp = FleetDispatcher::default();
        let mut comp = FleetComposition::new();
        comp.add("colony_ship", 1);
        let target = Coordinates::new(2, 100, 8);
        let mission = disp
            .dispatch(
                1,
                "p",
                FleetMissionType::Colonize,
                Coordinates::new(1, 1, 1),
                target.clone(),
                comp,
                Resources::zero(),
                0,
            )
            .unwrap();
        let outcome = process_arrival(&mission);
        match outcome {
            MissionOutcome::ColonyEstablished { planet_position } => {
                assert_eq!(planet_position, target);
            }
            _ => panic!("expected ColonyEstablished"),
        }
    }

    #[test]
    fn process_arrival_harvest() {
        let disp = FleetDispatcher::default();
        let mut comp = FleetComposition::new();
        comp.add("recycler", 2);
        let mission = disp
            .dispatch(
                1,
                "p",
                FleetMissionType::Harvest,
                Coordinates::new(1, 1, 1),
                Coordinates::new(1, 1, 2),
                comp,
                Resources::zero(),
                0,
            )
            .unwrap();
        let outcome = process_arrival(&mission);
        match outcome {
            MissionOutcome::HarvestCollected { metal, crystal } => {
                assert!(metal > 0);
                assert!(crystal > 0);
            }
            _ => panic!("expected HarvestCollected"),
        }
    }

    #[test]
    fn process_arrival_expedition() {
        let disp = FleetDispatcher::default();
        let mut comp = FleetComposition::new();
        comp.add("light_fighter", 5);
        comp.add("large_cargo", 1);
        let mission = disp
            .dispatch(
                1,
                "p",
                FleetMissionType::Expedition,
                Coordinates::new(1, 1, 1),
                Coordinates::new(1, 1, 2),
                comp,
                Resources::zero(),
                0,
            )
            .unwrap();
        let outcome = process_arrival(&mission);
        match outcome {
            MissionOutcome::ExpeditionResult { variant } => match variant {
                ExpeditionVariant::ResourceFind {
                    metal,
                    crystal,
                    deuterium,
                } => {
                    assert!(metal > 0);
                    assert!(crystal > 0);
                    assert!(deuterium > 0);
                }
                _ => panic!("expected ResourceFind variant"),
            },
            _ => panic!("expected ExpeditionResult"),
        }
    }

    #[test]
    fn process_arrival_deploy() {
        let disp = FleetDispatcher::default();
        let mut comp = FleetComposition::new();
        comp.add("light_fighter", 3);
        let mission = disp
            .dispatch(
                1,
                "p",
                FleetMissionType::Deploy,
                Coordinates::new(1, 1, 1),
                Coordinates::new(1, 1, 2),
                comp,
                Resources::zero(),
                0,
            )
            .unwrap();
        let outcome = process_arrival(&mission);
        assert_eq!(outcome, MissionOutcome::DeployComplete);
    }

    // --- FleetStore ---

    #[test]
    fn store_dispatch_and_get() {
        let mut store = FleetStore::new(1.0);
        let mut comp = FleetComposition::new();
        comp.add("cruiser", 5);
        let id = store
            .dispatch_fleet(
                "player1",
                FleetMissionType::Attack,
                Coordinates::new(1, 1, 1),
                Coordinates::new(1, 2, 1),
                comp,
                Resources::zero(),
                0,
            )
            .unwrap();
        assert_eq!(id, 1);
        let fleet = store.get_fleet(id).unwrap();
        assert_eq!(fleet.owner_id, "player1");
    }

    #[test]
    fn store_auto_increment_ids() {
        let mut store = FleetStore::new(1.0);
        let mut comp = FleetComposition::new();
        comp.add("light_fighter", 1);
        let id1 = store
            .dispatch_fleet(
                "p",
                FleetMissionType::Attack,
                Coordinates::new(1, 1, 1),
                Coordinates::new(1, 1, 2),
                comp.clone(),
                Resources::zero(),
                0,
            )
            .unwrap();
        let id2 = store
            .dispatch_fleet(
                "p",
                FleetMissionType::Attack,
                Coordinates::new(1, 1, 1),
                Coordinates::new(1, 1, 3),
                comp,
                Resources::zero(),
                0,
            )
            .unwrap();
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
    }

    #[test]
    fn store_list_player_fleets() {
        let mut store = FleetStore::new(1.0);
        let mut comp = FleetComposition::new();
        comp.add("light_fighter", 1);

        store
            .dispatch_fleet(
                "alice",
                FleetMissionType::Attack,
                Coordinates::new(1, 1, 1),
                Coordinates::new(1, 1, 2),
                comp.clone(),
                Resources::zero(),
                0,
            )
            .unwrap();
        store
            .dispatch_fleet(
                "bob",
                FleetMissionType::Attack,
                Coordinates::new(1, 1, 1),
                Coordinates::new(1, 1, 3),
                comp.clone(),
                Resources::zero(),
                0,
            )
            .unwrap();
        store
            .dispatch_fleet(
                "alice",
                FleetMissionType::Transport,
                Coordinates::new(1, 1, 1),
                Coordinates::new(1, 1, 4),
                comp,
                Resources::new(100, 0, 0),
                0,
            )
            .unwrap();

        assert_eq!(store.list_player_fleets("alice").len(), 2);
        assert_eq!(store.list_player_fleets("bob").len(), 1);
        assert_eq!(store.list_player_fleets("charlie").len(), 0);
    }

    #[test]
    fn store_list_incoming_fleets() {
        let mut store = FleetStore::new(1.0);
        let mut comp = FleetComposition::new();
        comp.add("light_fighter", 1);
        let target = Coordinates::new(1, 1, 5);

        store
            .dispatch_fleet(
                "a",
                FleetMissionType::Attack,
                Coordinates::new(1, 1, 1),
                target.clone(),
                comp.clone(),
                Resources::zero(),
                0,
            )
            .unwrap();
        store
            .dispatch_fleet(
                "b",
                FleetMissionType::Attack,
                Coordinates::new(1, 1, 2),
                target.clone(),
                comp.clone(),
                Resources::zero(),
                0,
            )
            .unwrap();
        // Different target
        store
            .dispatch_fleet(
                "c",
                FleetMissionType::Attack,
                Coordinates::new(1, 1, 1),
                Coordinates::new(1, 1, 6),
                comp,
                Resources::zero(),
                0,
            )
            .unwrap();

        assert_eq!(store.list_incoming_fleets(&target).len(), 2);
    }

    #[test]
    fn store_count_active_fleets() {
        let mut store = FleetStore::new(1.0);
        let mut comp = FleetComposition::new();
        comp.add("light_fighter", 1);
        store
            .dispatch_fleet(
                "p",
                FleetMissionType::Attack,
                Coordinates::new(1, 1, 1),
                Coordinates::new(1, 1, 2),
                comp.clone(),
                Resources::zero(),
                0,
            )
            .unwrap();
        store
            .dispatch_fleet(
                "p",
                FleetMissionType::Attack,
                Coordinates::new(1, 1, 1),
                Coordinates::new(1, 1, 3),
                comp,
                Resources::zero(),
                0,
            )
            .unwrap();
        assert_eq!(store.count_active_fleets("p"), 2);
        assert_eq!(store.count_active_fleets("other"), 0);
    }

    #[test]
    fn store_recall_fleet() {
        let mut store = FleetStore::new(1.0);
        let mut comp = FleetComposition::new();
        comp.add("cruiser", 1);
        let id = store
            .dispatch_fleet(
                "p",
                FleetMissionType::Attack,
                Coordinates::new(1, 1, 1),
                Coordinates::new(2, 1, 1),
                comp,
                Resources::zero(),
                0,
            )
            .unwrap();

        let arrival = parse_iso_epoch(&store.get_fleet(id).unwrap().arrival_time);
        store.recall_fleet(id, arrival / 3).unwrap();

        let fleet = store.get_fleet(id).unwrap();
        assert_eq!(fleet.status, MissionStatus::Recalled);
        // No longer counted as active
        assert_eq!(store.count_active_fleets("p"), 0);
    }

    #[test]
    fn store_recall_non_outbound_fails() {
        let mut store = FleetStore::new(1.0);
        let mut comp = FleetComposition::new();
        comp.add("light_fighter", 1);
        let id = store
            .dispatch_fleet(
                "p",
                FleetMissionType::Deploy,
                Coordinates::new(1, 1, 1),
                Coordinates::new(1, 1, 2),
                comp,
                Resources::zero(),
                0,
            )
            .unwrap();

        // Force completion by processing far in the future
        store.process_due_arrivals(999_999_999);

        let result = store.recall_fleet(id, 999_999_999);
        assert!(result.is_err());
    }

    #[test]
    fn store_process_due_arrivals() {
        let mut store = FleetStore::new(1.0);
        let mut comp = FleetComposition::new();
        comp.add("small_cargo", 2);
        let id = store
            .dispatch_fleet(
                "p",
                FleetMissionType::Transport,
                Coordinates::new(1, 1, 1),
                Coordinates::new(1, 1, 2),
                comp,
                Resources::new(500, 250, 100),
                0,
            )
            .unwrap();

        // Not yet arrived
        let results = store.process_due_arrivals(1);
        assert!(results.is_empty());

        // Far future - should arrive
        let results = store.process_due_arrivals(999_999_999);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, id);
        match &results[0].1 {
            MissionOutcome::TransportDelivered { resources } => {
                assert_eq!(resources.metal, 500);
            }
            _ => panic!("expected TransportDelivered"),
        }

        // Fleet should now be completed (both arrival and return are past)
        let fleet = store.get_fleet(id).unwrap();
        assert_eq!(fleet.status, MissionStatus::Completed);
    }

    #[test]
    fn store_get_nonexistent_returns_none() {
        let store = FleetStore::new(1.0);
        assert!(store.get_fleet(42).is_none());
    }

    #[test]
    fn store_recall_nonexistent_fails() {
        let mut store = FleetStore::new(1.0);
        assert!(store.recall_fleet(42, 0).is_err());
    }

    // --- Serialization ---

    #[test]
    fn fleet_composition_serde_roundtrip() {
        let mut comp = FleetComposition::new();
        comp.add("cruiser", 5);
        comp.add("small_cargo", 10);
        let json = serde_json::to_string(&comp).unwrap();
        let deserialized: FleetComposition = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, comp);
    }

    #[test]
    fn fleet_mission_serde_roundtrip() {
        let disp = FleetDispatcher::default();
        let mut comp = FleetComposition::new();
        comp.add("battleship", 3);
        let mission = disp
            .dispatch(
                1,
                "test_player",
                FleetMissionType::Attack,
                Coordinates::new(1, 1, 1),
                Coordinates::new(1, 2, 1),
                comp,
                Resources::new(100, 50, 25),
                1_000_000,
            )
            .unwrap();
        let json = serde_json::to_string(&mission).unwrap();
        let deserialized: FleetMission = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, mission.id);
        assert_eq!(deserialized.mission_type, mission.mission_type);
        assert_eq!(deserialized.status, mission.status);
    }

    #[test]
    fn mission_outcome_serde_roundtrip() {
        let outcome = MissionOutcome::AttackResult {
            loot: Resources::new(1000, 500, 200),
            debris_metal: 300,
            debris_crystal: 150,
        };
        let json = serde_json::to_string(&outcome).unwrap();
        let deserialized: MissionOutcome = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, outcome);
    }

    // --- ExpeditionVariant ---

    #[test]
    fn expedition_variant_all_types_serialize() {
        let variants = vec![
            ExpeditionVariant::ResourceFind {
                metal: 1,
                crystal: 2,
                deuterium: 3,
            },
            ExpeditionVariant::FleetFind {
                ship_type: "cruiser".to_string(),
                count: 2,
            },
            ExpeditionVariant::DarkMatterFind { amount: 500 },
            ExpeditionVariant::Nothing,
            ExpeditionVariant::Pirates,
            ExpeditionVariant::Aliens,
            ExpeditionVariant::BlackHole,
            ExpeditionVariant::Delay {
                extra_seconds: 3600,
            },
        ];
        for v in &variants {
            let json = serde_json::to_string(v).unwrap();
            let parsed: ExpeditionVariant = serde_json::from_str(&json).unwrap();
            assert_eq!(&parsed, v);
        }
    }

    // --- FleetDispatcher fuel and travel ---

    #[test]
    fn dispatcher_calculate_fuel_positive() {
        let disp = FleetDispatcher::default();
        let mut comp = FleetComposition::new();
        comp.add("cruiser", 10);
        let fuel = disp.calculate_fuel(&comp, 2795);
        assert!(fuel > 0.0);
    }

    #[test]
    fn dispatcher_travel_time_increases_with_distance() {
        let disp = FleetDispatcher::default();
        let mut comp = FleetComposition::new();
        comp.add("cruiser", 1);
        let t1 = disp.calculate_travel_time(&comp, 1000);
        let t2 = disp.calculate_travel_time(&comp, 20000);
        assert!(t2 > t1, "farther distance should take longer");
    }

    #[test]
    fn dispatcher_speed_factor_reduces_travel_time() {
        let slow = FleetDispatcher::new(1.0);
        let fast = FleetDispatcher::new(4.0);
        let mut comp = FleetComposition::new();
        comp.add("cruiser", 1);
        let t_slow = slow.calculate_travel_time(&comp, 10000);
        let t_fast = fast.calculate_travel_time(&comp, 10000);
        assert!(
            t_fast < t_slow,
            "higher speed factor should reduce travel time"
        );
    }

    #[test]
    fn authoritative_plan_derives_integer_fuel_cargo_and_duration() {
        let mut composition = FleetComposition::new();
        composition.add("small_cargo", 10);
        let plan = plan_authoritative_mission(
            FleetMissionType::Transport,
            FleetTargetKind::Planet,
            &Coordinates::new(1, 1, 1),
            &Coordinates::new(1, 1, 2),
            &composition,
            &Resources::new(20_000, 10_000, 5_000),
            FleetPlanningConfig::default(),
        )
        .unwrap();

        assert_eq!(plan.distance, 1005);
        assert_eq!(plan.fleet_speed, 5_000);
        assert_eq!(plan.fuel_required, 12);
        assert_eq!(plan.movement_fuel_required, 12);
        assert_eq!(plan.holding_fuel_required, 0);
        assert_eq!(plan.cargo_capacity, 50_000);
        assert_eq!(plan.usable_cargo_capacity, 49_988);
        assert_eq!(plan.travel_time_seconds, 49_632);
        assert_eq!(plan.applied_max_galaxies, 9);
        assert_eq!(plan.applied_max_systems, 499);
        assert_eq!(plan.applied_max_positions, 15);
        assert_eq!(plan.applied_speed_percent, 100);
        assert_eq!(plan.applied_fuel_multiplier_milli, 1_000);
    }

    #[test]
    fn authoritative_plan_enforces_target_and_required_ship_contracts() {
        let mut cargo = FleetComposition::new();
        cargo.add("small_cargo", 1);
        let invalid_target = plan_authoritative_mission(
            FleetMissionType::Harvest,
            FleetTargetKind::Planet,
            &Coordinates::new(1, 1, 1),
            &Coordinates::new(1, 1, 2),
            &cargo,
            &Resources::zero(),
            FleetPlanningConfig::default(),
        )
        .unwrap_err();
        assert!(invalid_target.0.contains("cannot target"));

        let missing_recycler = plan_authoritative_mission(
            FleetMissionType::Harvest,
            FleetTargetKind::Debris,
            &Coordinates::new(1, 1, 1),
            &Coordinates::new(1, 1, 2),
            &cargo,
            &Resources::zero(),
            FleetPlanningConfig::default(),
        )
        .unwrap_err();
        assert!(missing_recycler.0.contains("recycler"));
    }

    #[test]
    fn authoritative_plan_rejects_client_impossible_compositions_and_cargo() {
        let mut immobile = FleetComposition::new();
        immobile.add("solar_satellite", 1);
        assert!(plan_authoritative_mission(
            FleetMissionType::Transport,
            FleetTargetKind::Planet,
            &Coordinates::new(1, 1, 1),
            &Coordinates::new(1, 1, 2),
            &immobile,
            &Resources::zero(),
            FleetPlanningConfig::default(),
        )
        .unwrap_err()
        .0
        .contains("cannot participate"));

        let mut cargo = FleetComposition::new();
        cargo.add("small_cargo", 1);
        assert!(plan_authoritative_mission(
            FleetMissionType::Transport,
            FleetTargetKind::Planet,
            &Coordinates::new(1, 1, 1),
            &Coordinates::new(1, 1, 2),
            &cargo,
            &Resources::new(5_000, 0, 0),
            FleetPlanningConfig::default(),
        )
        .unwrap_err()
        .0
        .contains("usable capacity"));
    }

    #[test]
    fn authoritative_plan_requires_expedition_position_sixteen() {
        let mut fleet = FleetComposition::new();
        fleet.add("small_cargo", 1);
        assert!(plan_authoritative_mission(
            FleetMissionType::Expedition,
            FleetTargetKind::ExpeditionSlot,
            &Coordinates::new(1, 1, 1),
            &Coordinates::new(1, 1, 15),
            &fleet,
            &Resources::zero(),
            FleetPlanningConfig::default(),
        )
        .is_err());
        assert!(plan_authoritative_mission(
            FleetMissionType::Expedition,
            FleetTargetKind::ExpeditionSlot,
            &Coordinates::new(1, 1, 1),
            &Coordinates::new(1, 1, 16),
            &fleet,
            &Resources::zero(),
            FleetPlanningConfig::default(),
        )
        .is_ok());
    }

    #[test]
    fn recall_duration_is_symmetric_and_rejects_arrived_missions() {
        assert_eq!(recall_return_duration_seconds(100, 200, 140).unwrap(), 40);
        assert!(recall_return_duration_seconds(100, 200, 200).is_err());
        assert!(recall_return_duration_seconds(100, 200, 250).is_err());
        assert!(recall_return_duration_seconds(200, 100, 150).is_err());
    }

    #[test]
    fn authoritative_bounds_come_from_server_configuration() {
        let mut fleet = FleetComposition::new();
        fleet.add("small_cargo", 1);
        let config = FleetPlanningConfig {
            max_galaxies: 2,
            max_systems: 20,
            max_positions: 12,
            ..FleetPlanningConfig::default()
        };
        assert!(plan_authoritative_mission(
            FleetMissionType::Transport,
            FleetTargetKind::Planet,
            &Coordinates::new(2, 20, 12),
            &Coordinates::new(1, 1, 1),
            &fleet,
            &Resources::zero(),
            config,
        )
        .is_ok());
        assert!(plan_authoritative_mission(
            FleetMissionType::Transport,
            FleetTargetKind::Planet,
            &Coordinates::new(3, 20, 12),
            &Coordinates::new(1, 1, 1),
            &fleet,
            &Resources::zero(),
            config,
        )
        .is_err());
        assert!(plan_authoritative_mission(
            FleetMissionType::Expedition,
            FleetTargetKind::ExpeditionSlot,
            &Coordinates::new(2, 20, 12),
            &Coordinates::new(2, 20, 13),
            &fleet,
            &Resources::zero(),
            config,
        )
        .is_ok());
    }

    #[test]
    fn integer_square_root_rounds_up_at_boundaries() {
        assert_eq!(integer_sqrt_ceil(0), 0);
        assert_eq!(integer_sqrt_ceil(1), 1);
        assert_eq!(integer_sqrt_ceil(2), 2);
        assert_eq!(integer_sqrt_ceil(4), 2);
        assert_eq!(integer_sqrt_ceil(15), 4);
        assert_eq!(integer_sqrt_ceil(16), 4);
        assert_eq!(integer_sqrt_ceil(17), 5);
        assert_eq!(
            integer_sqrt_ceil(u128::from(u64::MAX).pow(2)),
            u128::from(u64::MAX)
        );
    }

    #[test]
    fn selected_speed_has_golden_integer_fuel_costs() {
        let mut fleet = FleetComposition::new();
        fleet.add("small_cargo", 10);
        let fuel_at = |speed_percent| {
            plan_authoritative_mission(
                FleetMissionType::Transport,
                FleetTargetKind::Planet,
                &Coordinates::new(1, 1, 1),
                &Coordinates::new(1, 1, 2),
                &fleet,
                &Resources::zero(),
                FleetPlanningConfig {
                    speed_percent,
                    ..FleetPlanningConfig::default()
                },
            )
            .unwrap()
            .fuel_required
        };
        assert_eq!(fuel_at(10), 4);
        assert_eq!(fuel_at(50), 7);
        assert_eq!(fuel_at(100), 12);
    }

    #[test]
    fn cargo_boundary_accounts_for_exact_speed_dependent_fuel() {
        let mut fleet = FleetComposition::new();
        fleet.add("small_cargo", 1);
        let config = FleetPlanningConfig::default();
        let accepted = plan_authoritative_mission(
            FleetMissionType::Transport,
            FleetTargetKind::Planet,
            &Coordinates::new(1, 1, 1),
            &Coordinates::new(1, 1, 2),
            &fleet,
            &Resources::new(4_998, 0, 0),
            config,
        )
        .unwrap();
        assert_eq!(accepted.fuel_required, 2);
        assert_eq!(accepted.usable_cargo_capacity, 4_998);
        assert!(plan_authoritative_mission(
            FleetMissionType::Transport,
            FleetTargetKind::Planet,
            &Coordinates::new(1, 1, 1),
            &Coordinates::new(1, 1, 2),
            &fleet,
            &Resources::new(4_999, 0, 0),
            config,
        )
        .is_err());
    }

    #[test]
    fn acs_defense_plan_persists_bounded_hold_and_fuel() {
        let mut fleet = FleetComposition::new();
        fleet.add("small_cargo", 10);
        let plan = plan_authoritative_mission(
            FleetMissionType::AcsDefend,
            FleetTargetKind::Planet,
            &Coordinates::new(1, 1, 1),
            &Coordinates::new(1, 1, 2),
            &fleet,
            &Resources::zero(),
            FleetPlanningConfig {
                hold_seconds: 3_600,
                ..FleetPlanningConfig::default()
            },
        )
        .unwrap();
        assert_eq!(plan.movement_fuel_required, 12);
        assert_eq!(plan.holding_fuel_required, 10);
        assert_eq!(plan.fuel_required, 22);
        assert_eq!(plan.applied_hold_seconds, 3_600);

        assert!(plan_authoritative_mission(
            FleetMissionType::Attack,
            FleetTargetKind::Planet,
            &Coordinates::new(1, 1, 1),
            &Coordinates::new(1, 1, 2),
            &fleet,
            &Resources::zero(),
            FleetPlanningConfig {
                hold_seconds: 60,
                ..FleetPlanningConfig::default()
            },
        )
        .is_err());
        assert!(plan_authoritative_mission(
            FleetMissionType::AcsJoin,
            FleetTargetKind::Planet,
            &Coordinates::new(1, 1, 1),
            &Coordinates::new(1, 1, 2),
            &fleet,
            &Resources::zero(),
            FleetPlanningConfig {
                hold_seconds: 60,
                ..FleetPlanningConfig::default()
            },
        )
        .is_err());
    }
}
