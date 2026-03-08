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

    /// Calculate fuel for a fleet over a distance using OGame-inspired formula:
    /// `total_consumption * distance / 35000 * total_ship_count`
    ///
    /// The speed factor reduces fuel proportionally.
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

/// Process a fleet arrival and produce an outcome.
/// This is a deterministic "stub" processor. Real game logic would consult
/// planet data, combat engine, etc. The function demonstrates the mission
/// lifecycle and returns a placeholder outcome for each mission type.
pub fn process_arrival(mission: &FleetMission) -> MissionOutcome {
    match mission.mission_type {
        FleetMissionType::Attack | FleetMissionType::AcsAttack => {
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
pub struct FleetStore {
    pub fleets: HashMap<u64, FleetMission>,
    next_id: u64,
    dispatcher: FleetDispatcher,
}

impl FleetStore {
    pub fn new(speed_factor: f64) -> Self {
        Self {
            fleets: HashMap::new(),
            next_id: 1,
            dispatcher: FleetDispatcher::new(speed_factor),
        }
    }

    /// Dispatch a fleet and store it. Returns the assigned fleet id.
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
            if fleet_mut.mission_type == FleetMissionType::Deploy {
                fleet_mut.status = MissionStatus::Completed;
            } else if fleet_mut.is_returned(now) {
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
}
