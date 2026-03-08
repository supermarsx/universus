#![forbid(unsafe_code)]

//! Queue management for the Universus game.
//!
//! Implements OGame-style building, research, and shipyard queues with lazy
//! resource evaluation, cost/time formulas, and completion tracking.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// QueueStatus
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueueStatus {
    Pending,
    InProgress,
    Completed,
    Cancelled,
}

// ---------------------------------------------------------------------------
// QueueError
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueueError {
    InsufficientResources {
        needed_metal: i64,
        needed_crystal: i64,
        needed_deuterium: i64,
    },
    QueueFull,
    AlreadyBuilding,
    AlreadyResearching,
    InvalidType(String),
    NotFound,
    InvalidState(String),
}

impl std::fmt::Display for QueueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InsufficientResources {
                needed_metal,
                needed_crystal,
                needed_deuterium,
            } => write!(
                f,
                "Insufficient resources: need {needed_metal}m {needed_crystal}c {needed_deuterium}d"
            ),
            Self::QueueFull => write!(f, "Queue is full"),
            Self::AlreadyBuilding => {
                write!(f, "Planet already has an active building construction")
            }
            Self::AlreadyResearching => write!(f, "Player already has active research"),
            Self::InvalidType(t) => write!(f, "Invalid type: {t}"),
            Self::NotFound => write!(f, "Queue item not found"),
            Self::InvalidState(s) => write!(f, "Invalid state: {s}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Queue item types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuildingQueueItem {
    pub queue_id: u64,
    pub planet_id: i32,
    pub building_type: String,
    pub current_level: i32,
    pub target_level: i32,
    pub status: QueueStatus,
    pub start_time: String,
    pub finish_time: String,
    pub metal_cost: i64,
    pub crystal_cost: i64,
    pub deuterium_cost: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResearchQueueItem {
    pub queue_id: u64,
    pub player_id: i32,
    pub research_type: String,
    pub current_level: i32,
    pub target_level: i32,
    pub status: QueueStatus,
    pub start_time: String,
    pub finish_time: String,
    pub metal_cost: i64,
    pub crystal_cost: i64,
    pub deuterium_cost: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShipyardQueueItem {
    pub order_id: u64,
    pub planet_id: i32,
    pub unit_type: String,
    pub is_defense: bool,
    pub count: i32,
    pub each_build_time_secs: f64,
    pub status: QueueStatus,
    pub start_time: String,
    pub finish_time: String,
    pub metal_cost: i64,
    pub crystal_cost: i64,
    pub deuterium_cost: i64,
}

// ---------------------------------------------------------------------------
// Completion results
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueueCompletions {
    pub buildings: Vec<BuildingQueueItem>,
    pub research: Vec<ResearchQueueItem>,
    pub shipyard: Vec<ShipyardQueueItem>,
}

// ---------------------------------------------------------------------------
// Lazy resource evaluation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LazyResourceState {
    pub metal: i64,
    pub crystal: i64,
    pub deuterium: i64,
    pub metal_per_hour: f64,
    pub crystal_per_hour: f64,
    pub deuterium_per_hour: f64,
    pub last_updated: String,
    pub storage_metal: i64,
    pub storage_crystal: i64,
    pub storage_deuterium: i64,
}

impl LazyResourceState {
    /// Compute accumulated resources at `now` (ISO 8601), capped by storage.
    /// Returns `(metal, crystal, deuterium)`.
    pub fn evaluate(&self, now: &str) -> (i64, i64, i64) {
        let elapsed_secs = iso_diff_seconds(now, &self.last_updated);
        if elapsed_secs <= 0.0 {
            return (self.metal, self.crystal, self.deuterium);
        }
        let hours = elapsed_secs / 3600.0;

        let metal = (self.metal as f64 + self.metal_per_hour * hours) as i64;
        let crystal = (self.crystal as f64 + self.crystal_per_hour * hours) as i64;
        let deuterium = (self.deuterium as f64 + self.deuterium_per_hour * hours) as i64;

        let metal = if self.storage_metal > 0 {
            metal.min(self.storage_metal)
        } else {
            metal
        };
        let crystal = if self.storage_crystal > 0 {
            crystal.min(self.storage_crystal)
        } else {
            crystal
        };
        let deuterium = if self.storage_deuterium > 0 {
            deuterium.min(self.storage_deuterium)
        } else {
            deuterium
        };

        (metal, crystal, deuterium)
    }

    /// Evaluate resources and deduct `metal`, `crystal`, `deuterium`.
    /// Updates `self.metal/crystal/deuterium` and `self.last_updated` to `now`.
    pub fn spend(
        &mut self,
        metal: i64,
        crystal: i64,
        deuterium: i64,
        now: &str,
    ) -> Result<(), QueueError> {
        let (cur_m, cur_c, cur_d) = self.evaluate(now);
        if cur_m < metal || cur_c < crystal || cur_d < deuterium {
            return Err(QueueError::InsufficientResources {
                needed_metal: metal,
                needed_crystal: crystal,
                needed_deuterium: deuterium,
            });
        }
        self.metal = cur_m - metal;
        self.crystal = cur_c - crystal;
        self.deuterium = cur_d - deuterium;
        self.last_updated = now.to_string();
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Building base costs & cost factor table
// ---------------------------------------------------------------------------

/// Returns `(base_metal, base_crystal, base_deuterium, cost_factor)` for a
/// building type string. Returns `None` for unrecognised types.
fn building_base_cost(building_type: &str) -> Option<(f64, f64, f64, f64)> {
    match building_type {
        "MetalMine" => Some((60.0, 15.0, 0.0, 1.5)),
        "CrystalMine" => Some((48.0, 24.0, 0.0, 1.6)),
        "DeuteriumSynthesizer" => Some((225.0, 75.0, 0.0, 1.5)),
        "SolarPlant" => Some((75.0, 30.0, 0.0, 1.5)),
        "FusionReactor" => Some((900.0, 360.0, 180.0, 1.8)),
        "MetalStorage" => Some((1000.0, 0.0, 0.0, 2.0)),
        "CrystalStorage" => Some((1000.0, 500.0, 0.0, 2.0)),
        "DeuteriumTank" => Some((1000.0, 1000.0, 0.0, 2.0)),
        "RoboticsFactory" => Some((400.0, 120.0, 200.0, 2.0)),
        "Shipyard" => Some((400.0, 200.0, 100.0, 2.0)),
        "ResearchLab" => Some((200.0, 400.0, 200.0, 2.0)),
        "NaniteFactory" => Some((1_000_000.0, 500_000.0, 100_000.0, 2.0)),
        "Terraformer" => Some((0.0, 50_000.0, 100_000.0, 2.0)),
        "MissileSilo" => Some((20_000.0, 20_000.0, 1000.0, 2.0)),
        "AllianceDepot" => Some((20_000.0, 40_000.0, 0.0, 2.0)),
        "SpaceDock" => Some((200.0, 0.0, 50.0, 5.0)),
        _ => None,
    }
}

/// Compute building cost at `level`: base * factor^(level-1), floored.
fn compute_building_cost(building_type: &str, level: i32) -> Result<(i64, i64, i64), QueueError> {
    let (bm, bc, bd, factor) = building_base_cost(building_type)
        .ok_or_else(|| QueueError::InvalidType(building_type.to_string()))?;
    let mult = factor.powi(level - 1);
    Ok((
        (bm * mult).floor() as i64,
        (bc * mult).floor() as i64,
        (bd * mult).floor() as i64,
    ))
}

/// Building construction time in seconds.
///
/// Formula: `base_seconds * factor^(level-1) / (1 + robotics_level) / 2^nanite_level`
///
/// `base_seconds` is derived from `(base_metal + base_crystal) / 2500 * 3600`.
fn compute_building_time(building_type: &str, level: i32, robotics: i32, nanite: i32) -> f64 {
    let (bm, bc, _bd, factor) = match building_base_cost(building_type) {
        Some(v) => v,
        None => return 0.0,
    };
    let base_secs = (bm + bc) / 2500.0 * 3600.0;
    let raw = base_secs * factor.powi(level - 1);
    let time = raw / (1 + robotics) as f64 / 2.0_f64.powi(nanite);
    time.max(1.0) // minimum 1 second
}

// ---------------------------------------------------------------------------
// Research base costs
// ---------------------------------------------------------------------------

fn research_base_cost(research_type: &str) -> Option<(f64, f64, f64, f64)> {
    match research_type {
        "EnergyTechnology" => Some((0.0, 800.0, 400.0, 2.0)),
        "LaserTechnology" => Some((200.0, 100.0, 0.0, 2.0)),
        "IonTechnology" => Some((1000.0, 300.0, 100.0, 2.0)),
        "HyperspaceTechnology" => Some((0.0, 4000.0, 2000.0, 2.0)),
        "PlasmaTechnology" => Some((2000.0, 4000.0, 1000.0, 2.0)),
        "CombustionDrive" => Some((400.0, 0.0, 600.0, 2.0)),
        "ImpulseDrive" => Some((2000.0, 4000.0, 600.0, 2.0)),
        "HyperspaceDrive" => Some((10_000.0, 20_000.0, 6000.0, 2.0)),
        "EspionageTechnology" => Some((200.0, 1000.0, 200.0, 2.0)),
        "ComputerTechnology" => Some((0.0, 400.0, 600.0, 2.0)),
        "Astrophysics" => Some((4000.0, 8000.0, 4000.0, 1.75)),
        "IntergalacticResearchNetwork" => Some((240_000.0, 400_000.0, 160_000.0, 2.0)),
        "GravitonTechnology" => Some((0.0, 0.0, 0.0, 2.0)),
        "WeaponsTechnology" => Some((800.0, 200.0, 0.0, 2.0)),
        "ShieldingTechnology" => Some((200.0, 600.0, 0.0, 2.0)),
        "ArmourTechnology" => Some((1000.0, 0.0, 0.0, 2.0)),
        _ => None,
    }
}

fn compute_research_cost(research_type: &str, level: i32) -> Result<(i64, i64, i64), QueueError> {
    let (bm, bc, bd, factor) = research_base_cost(research_type)
        .ok_or_else(|| QueueError::InvalidType(research_type.to_string()))?;
    let mult = factor.powi(level - 1);
    Ok((
        (bm * mult).floor() as i64,
        (bc * mult).floor() as i64,
        (bd * mult).floor() as i64,
    ))
}

/// Research time in seconds.
///
/// Formula: `(base_metal + base_crystal) / 1000 * 3600 * factor^(level-1) / (1 + lab_level)`
fn compute_research_time(research_type: &str, level: i32, lab_level: i32) -> f64 {
    let (bm, bc, _bd, factor) = match research_base_cost(research_type) {
        Some(v) => v,
        None => return 0.0,
    };
    let base_secs = (bm + bc) / 1000.0 * 3600.0;
    let raw = base_secs * factor.powi(level - 1);
    let time = raw / (1 + lab_level) as f64;
    time.max(1.0)
}

// ---------------------------------------------------------------------------
// Ship / defense base costs
// ---------------------------------------------------------------------------

fn ship_base_cost(ship_type: &str) -> Option<(f64, f64, f64)> {
    match ship_type {
        "SmallCargo" => Some((2000.0, 2000.0, 0.0)),
        "LargeCargo" => Some((6000.0, 6000.0, 0.0)),
        "LightFighter" => Some((3000.0, 1000.0, 0.0)),
        "HeavyFighter" => Some((6000.0, 4000.0, 0.0)),
        "Cruiser" => Some((20_000.0, 7000.0, 2000.0)),
        "Battleship" => Some((45_000.0, 15_000.0, 0.0)),
        "Battlecruiser" => Some((30_000.0, 40_000.0, 15_000.0)),
        "Bomber" => Some((50_000.0, 25_000.0, 15_000.0)),
        "Destroyer" => Some((60_000.0, 50_000.0, 15_000.0)),
        "Deathstar" => Some((5_000_000.0, 4_000_000.0, 1_000_000.0)),
        "Recycler" => Some((10_000.0, 6000.0, 2000.0)),
        "EspionageProbe" => Some((0.0, 1000.0, 0.0)),
        "SolarSatellite" => Some((0.0, 2000.0, 500.0)),
        "ColonyShip" => Some((10_000.0, 20_000.0, 10_000.0)),
        _ => None,
    }
}

fn defense_base_cost(defense_type: &str) -> Option<(f64, f64, f64)> {
    match defense_type {
        "RocketLauncher" => Some((2000.0, 0.0, 0.0)),
        "LightLaser" => Some((1500.0, 500.0, 0.0)),
        "HeavyLaser" => Some((6000.0, 2000.0, 0.0)),
        "GaussCannon" => Some((20_000.0, 15_000.0, 2000.0)),
        "IonCannon" => Some((5000.0, 3000.0, 0.0)),
        "PlasmaTurret" => Some((50_000.0, 50_000.0, 30_000.0)),
        "SmallShieldDome" => Some((10_000.0, 10_000.0, 0.0)),
        "LargeShieldDome" => Some((50_000.0, 50_000.0, 0.0)),
        "AntiBallisticMissile" => Some((8000.0, 0.0, 2000.0)),
        "InterplanetaryMissile" => Some((12_500.0, 2500.0, 10_000.0)),
        _ => None,
    }
}

fn compute_unit_cost(
    unit_type: &str,
    is_defense: bool,
    count: i32,
) -> Result<(i64, i64, i64), QueueError> {
    let (bm, bc, bd) = if is_defense {
        defense_base_cost(unit_type)
    } else {
        ship_base_cost(unit_type)
    }
    .ok_or_else(|| QueueError::InvalidType(unit_type.to_string()))?;
    Ok((
        (bm as i64) * count as i64,
        (bc as i64) * count as i64,
        (bd as i64) * count as i64,
    ))
}

/// Per-unit shipyard construction time in seconds.
///
/// Formula: `(metal + crystal) / (2500 * (1 + shipyard_level) * 2^nanite_level) * 3600`
fn compute_shipyard_unit_time(
    unit_type: &str,
    is_defense: bool,
    shipyard_level: i32,
    nanite_level: i32,
) -> f64 {
    let (bm, bc, _bd) = if is_defense {
        defense_base_cost(unit_type).unwrap_or((0.0, 0.0, 0.0))
    } else {
        ship_base_cost(unit_type).unwrap_or((0.0, 0.0, 0.0))
    };
    let hours = (bm + bc) / (2500.0 * (1 + shipyard_level) as f64 * 2.0_f64.powi(nanite_level));
    (hours * 3600.0).max(1.0)
}

// ---------------------------------------------------------------------------
// ISO 8601 timestamp helpers (minimal, no external deps)
// ---------------------------------------------------------------------------

/// Add `seconds` to an ISO 8601 timestamp string and return a new ISO 8601
/// string.  Supports the format `YYYY-MM-DDTHH:MM:SSZ`.
fn iso_add_seconds(iso: &str, seconds: f64) -> String {
    let unix = iso_to_unix(iso);
    let new_unix = unix + seconds as i64;
    unix_to_iso(new_unix)
}

/// Difference in seconds between two ISO 8601 timestamps: `a - b`.
fn iso_diff_seconds(a: &str, b: &str) -> f64 {
    let ua = iso_to_unix(a);
    let ub = iso_to_unix(b);
    (ua - ub) as f64
}

/// Minimal ISO 8601 (`YYYY-MM-DDTHH:MM:SSZ`) -> unix seconds parser.
fn iso_to_unix(iso: &str) -> i64 {
    // Expect exactly "YYYY-MM-DDTHH:MM:SSZ" (20 chars) or with fractional.
    let s = iso.trim_end_matches('Z');
    let (date_part, time_part) = s.split_once('T').unwrap_or((s, "00:00:00"));

    let date_parts: Vec<&str> = date_part.split('-').collect();
    let year: i64 = date_parts
        .first()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1970);
    let month: i64 = date_parts.get(1).and_then(|v| v.parse().ok()).unwrap_or(1);
    let day: i64 = date_parts.get(2).and_then(|v| v.parse().ok()).unwrap_or(1);

    let time_parts: Vec<&str> = time_part.split(':').collect();
    let hour: i64 = time_parts.first().and_then(|v| v.parse().ok()).unwrap_or(0);
    let minute: i64 = time_parts.get(1).and_then(|v| v.parse().ok()).unwrap_or(0);
    let second: i64 = time_parts
        .get(2)
        .and_then(|v| v.split('.').next())
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    // Days from year
    let mut days: i64 = 0;
    for y in 1970..year {
        days += if is_leap(y) { 366 } else { 365 };
    }

    let month_days = [
        31,
        28 + if is_leap(year) { 1 } else { 0 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    for m in 0..(month - 1) as usize {
        days += month_days[m];
    }
    days += day - 1;

    days * 86400 + hour * 3600 + minute * 60 + second
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn unix_to_iso(unix: i64) -> String {
    let mut remaining = unix;
    let mut year = 1970i64;

    loop {
        let days_in_year = if is_leap(year) { 366 } else { 365 };
        let secs_in_year = days_in_year * 86400;
        if remaining < secs_in_year {
            break;
        }
        remaining -= secs_in_year;
        year += 1;
    }

    let month_days = [
        31,
        28 + if is_leap(year) { 1 } else { 0 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1;
    for &md in &month_days {
        let secs_in_month = md * 86400;
        if remaining < secs_in_month {
            break;
        }
        remaining -= secs_in_month;
        month += 1;
    }

    let day = remaining / 86400 + 1;
    remaining %= 86400;
    let hour = remaining / 3600;
    remaining %= 3600;
    let minute = remaining / 60;
    let second = remaining % 60;

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hour, minute, second
    )
}

// ---------------------------------------------------------------------------
// BuildingQueue
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildingQueue {
    items: HashMap<i32, Vec<BuildingQueueItem>>,
    next_id: u64,
}

impl Default for BuildingQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildingQueue {
    pub fn new() -> Self {
        Self {
            items: HashMap::new(),
            next_id: 1,
        }
    }

    /// Returns the currently active (InProgress) build for a planet, if any.
    pub fn active_build(&self, planet_id: i32) -> Option<BuildingQueueItem> {
        self.items.get(&planet_id).and_then(|queue| {
            queue
                .iter()
                .find(|item| item.status == QueueStatus::InProgress)
                .cloned()
        })
    }

    /// Enqueue a building upgrade. Only ONE building can be under construction
    /// per planet at a time (OGame rule).
    #[allow(clippy::too_many_arguments)]
    pub fn enqueue(
        &mut self,
        planet_id: i32,
        building_type: &str,
        current_level: i32,
        resources_available: (i64, i64, i64),
        robotics_level: i32,
        nanite_level: i32,
        now: &str,
    ) -> Result<BuildingQueueItem, QueueError> {
        // Check no active build on this planet
        if self.active_build(planet_id).is_some() {
            return Err(QueueError::AlreadyBuilding);
        }

        let target_level = current_level + 1;
        let (metal, crystal, deuterium) = compute_building_cost(building_type, target_level)?;

        // Validate resources
        let (avail_m, avail_c, avail_d) = resources_available;
        if avail_m < metal || avail_c < crystal || avail_d < deuterium {
            return Err(QueueError::InsufficientResources {
                needed_metal: metal,
                needed_crystal: crystal,
                needed_deuterium: deuterium,
            });
        }

        let build_secs =
            compute_building_time(building_type, target_level, robotics_level, nanite_level);
        let finish = iso_add_seconds(now, build_secs);

        let id = self.next_id;
        self.next_id += 1;

        let item = BuildingQueueItem {
            queue_id: id,
            planet_id,
            building_type: building_type.to_string(),
            current_level,
            target_level,
            status: QueueStatus::InProgress,
            start_time: now.to_string(),
            finish_time: finish,
            metal_cost: metal,
            crystal_cost: crystal,
            deuterium_cost: deuterium,
        };

        self.items.entry(planet_id).or_default().push(item.clone());

        Ok(item)
    }

    /// Cancel an active build on a planet. Returns the refunded resources
    /// `(metal, crystal, deuterium)`.
    pub fn cancel(&mut self, planet_id: i32, queue_id: u64) -> Result<(i64, i64, i64), QueueError> {
        let queue = self.items.get_mut(&planet_id).ok_or(QueueError::NotFound)?;
        let item = queue
            .iter_mut()
            .find(|i| i.queue_id == queue_id && i.status == QueueStatus::InProgress)
            .ok_or(QueueError::NotFound)?;

        let refund = (item.metal_cost, item.crystal_cost, item.deuterium_cost);
        item.status = QueueStatus::Cancelled;
        Ok(refund)
    }

    /// Check for completed builds on a planet at time `now`.
    /// Marks them as `Completed` and returns them.
    pub fn check_completion(&mut self, planet_id: i32, now: &str) -> Vec<BuildingQueueItem> {
        let mut completed = Vec::new();
        let now_unix = iso_to_unix(now);

        if let Some(queue) = self.items.get_mut(&planet_id) {
            for item in queue.iter_mut() {
                if item.status == QueueStatus::InProgress {
                    let finish_unix = iso_to_unix(&item.finish_time);
                    if now_unix >= finish_unix {
                        item.status = QueueStatus::Completed;
                        completed.push(item.clone());
                    }
                }
            }
        }

        completed
    }
}

// ---------------------------------------------------------------------------
// ResearchQueue
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchQueue {
    items: HashMap<i32, Vec<ResearchQueueItem>>,
    next_id: u64,
}

impl Default for ResearchQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl ResearchQueue {
    pub fn new() -> Self {
        Self {
            items: HashMap::new(),
            next_id: 1,
        }
    }

    /// Returns the currently active research for a player, if any.
    pub fn active_research(&self, player_id: i32) -> Option<ResearchQueueItem> {
        self.items.get(&player_id).and_then(|queue| {
            queue
                .iter()
                .find(|item| item.status == QueueStatus::InProgress)
                .cloned()
        })
    }

    /// Enqueue a research upgrade. Only ONE research globally per player (OGame rule).
    pub fn enqueue(
        &mut self,
        player_id: i32,
        research_type: &str,
        current_level: i32,
        resources_available: (i64, i64, i64),
        lab_level: i32,
        now: &str,
    ) -> Result<ResearchQueueItem, QueueError> {
        if self.active_research(player_id).is_some() {
            return Err(QueueError::AlreadyResearching);
        }

        let target_level = current_level + 1;
        let (metal, crystal, deuterium) = compute_research_cost(research_type, target_level)?;

        let (avail_m, avail_c, avail_d) = resources_available;
        if avail_m < metal || avail_c < crystal || avail_d < deuterium {
            return Err(QueueError::InsufficientResources {
                needed_metal: metal,
                needed_crystal: crystal,
                needed_deuterium: deuterium,
            });
        }

        let research_secs = compute_research_time(research_type, target_level, lab_level);
        let finish = iso_add_seconds(now, research_secs);

        let id = self.next_id;
        self.next_id += 1;

        let item = ResearchQueueItem {
            queue_id: id,
            player_id,
            research_type: research_type.to_string(),
            current_level,
            target_level,
            status: QueueStatus::InProgress,
            start_time: now.to_string(),
            finish_time: finish,
            metal_cost: metal,
            crystal_cost: crystal,
            deuterium_cost: deuterium,
        };

        self.items.entry(player_id).or_default().push(item.clone());

        Ok(item)
    }

    /// Cancel active research for a player. Returns refunded resources.
    pub fn cancel(&mut self, player_id: i32, queue_id: u64) -> Result<(i64, i64, i64), QueueError> {
        let queue = self.items.get_mut(&player_id).ok_or(QueueError::NotFound)?;
        let item = queue
            .iter_mut()
            .find(|i| i.queue_id == queue_id && i.status == QueueStatus::InProgress)
            .ok_or(QueueError::NotFound)?;

        let refund = (item.metal_cost, item.crystal_cost, item.deuterium_cost);
        item.status = QueueStatus::Cancelled;
        Ok(refund)
    }

    /// Check for completed research for a player at time `now`.
    pub fn check_completion(&mut self, player_id: i32, now: &str) -> Vec<ResearchQueueItem> {
        let mut completed = Vec::new();
        let now_unix = iso_to_unix(now);

        if let Some(queue) = self.items.get_mut(&player_id) {
            for item in queue.iter_mut() {
                if item.status == QueueStatus::InProgress {
                    let finish_unix = iso_to_unix(&item.finish_time);
                    if now_unix >= finish_unix {
                        item.status = QueueStatus::Completed;
                        completed.push(item.clone());
                    }
                }
            }
        }

        completed
    }
}

// ---------------------------------------------------------------------------
// ShipyardQueue
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShipyardQueue {
    items: HashMap<i32, Vec<ShipyardQueueItem>>,
    next_id: u64,
}

impl Default for ShipyardQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl ShipyardQueue {
    pub fn new() -> Self {
        Self {
            items: HashMap::new(),
            next_id: 1,
        }
    }

    /// Enqueue a shipyard build order. Multiple items can be queued; they
    /// process sequentially. New items start after the last queued item finishes.
    #[allow(clippy::too_many_arguments)]
    pub fn enqueue(
        &mut self,
        planet_id: i32,
        unit_type: &str,
        is_defense: bool,
        count: i32,
        shipyard_level: i32,
        nanite_level: i32,
        resources_available: (i64, i64, i64),
        now: &str,
    ) -> Result<ShipyardQueueItem, QueueError> {
        if count <= 0 {
            return Err(QueueError::InvalidState(
                "Count must be positive".to_string(),
            ));
        }

        let (metal, crystal, deuterium) = compute_unit_cost(unit_type, is_defense, count)?;

        let (avail_m, avail_c, avail_d) = resources_available;
        if avail_m < metal || avail_c < crystal || avail_d < deuterium {
            return Err(QueueError::InsufficientResources {
                needed_metal: metal,
                needed_crystal: crystal,
                needed_deuterium: deuterium,
            });
        }

        let per_unit_secs =
            compute_shipyard_unit_time(unit_type, is_defense, shipyard_level, nanite_level);
        let total_secs = per_unit_secs * count as f64;

        // Start time is max(now, last item's finish_time) for sequential processing
        let queue = self.items.entry(planet_id).or_default();
        let start = if let Some(last_active) = queue
            .iter()
            .filter(|i| i.status == QueueStatus::InProgress || i.status == QueueStatus::Pending)
            .last()
        {
            let last_finish_unix = iso_to_unix(&last_active.finish_time);
            let now_unix = iso_to_unix(now);
            if last_finish_unix > now_unix {
                last_active.finish_time.clone()
            } else {
                now.to_string()
            }
        } else {
            now.to_string()
        };

        let finish = iso_add_seconds(&start, total_secs);

        let id = self.next_id;
        self.next_id += 1;

        // First item in queue is InProgress, subsequent are Pending
        let status = if queue.iter().any(|i| i.status == QueueStatus::InProgress) {
            QueueStatus::Pending
        } else {
            QueueStatus::InProgress
        };

        let item = ShipyardQueueItem {
            order_id: id,
            planet_id,
            unit_type: unit_type.to_string(),
            is_defense,
            count,
            each_build_time_secs: per_unit_secs,
            status,
            start_time: start,
            finish_time: finish,
            metal_cost: metal,
            crystal_cost: crystal,
            deuterium_cost: deuterium,
        };

        queue.push(item.clone());
        Ok(item)
    }

    /// Cancel the LAST queued item on a planet (OGame rule: can only cancel
    /// the last item). Returns refunded resources.
    pub fn cancel_last(&mut self, planet_id: i32) -> Result<(i64, i64, i64), QueueError> {
        let queue = self.items.get_mut(&planet_id).ok_or(QueueError::NotFound)?;

        // Find last non-completed, non-cancelled item
        let idx = queue
            .iter()
            .rposition(|i| i.status == QueueStatus::Pending || i.status == QueueStatus::InProgress)
            .ok_or(QueueError::NotFound)?;

        let refund = (
            queue[idx].metal_cost,
            queue[idx].crystal_cost,
            queue[idx].deuterium_cost,
        );
        queue[idx].status = QueueStatus::Cancelled;
        Ok(refund)
    }

    /// Check for completed shipyard items on a planet. Promotes pending items
    /// to InProgress as previous items complete.
    pub fn check_completion(&mut self, planet_id: i32, now: &str) -> Vec<ShipyardQueueItem> {
        let mut completed = Vec::new();
        let now_unix = iso_to_unix(now);

        if let Some(queue) = self.items.get_mut(&planet_id) {
            // Complete finished items
            for item in queue.iter_mut() {
                if item.status == QueueStatus::InProgress {
                    let finish_unix = iso_to_unix(&item.finish_time);
                    if now_unix >= finish_unix {
                        item.status = QueueStatus::Completed;
                        completed.push(item.clone());
                    }
                }
            }

            // Promote first pending item to InProgress if nothing is active
            let has_active = queue.iter().any(|i| i.status == QueueStatus::InProgress);
            if !has_active {
                if let Some(pending) = queue.iter_mut().find(|i| i.status == QueueStatus::Pending) {
                    pending.status = QueueStatus::InProgress;
                }
            }
        }

        completed
    }

    /// Number of active + pending items in the queue for a planet.
    pub fn queue_length(&self, planet_id: i32) -> usize {
        self.items
            .get(&planet_id)
            .map(|q| {
                q.iter()
                    .filter(|i| {
                        i.status == QueueStatus::InProgress || i.status == QueueStatus::Pending
                    })
                    .count()
            })
            .unwrap_or(0)
    }

    /// List all active + pending items in the queue for a planet.
    pub fn list_queue(&self, planet_id: i32) -> Vec<ShipyardQueueItem> {
        self.items
            .get(&planet_id)
            .map(|q| {
                q.iter()
                    .filter(|i| {
                        i.status == QueueStatus::InProgress || i.status == QueueStatus::Pending
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// QueueManager
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueManager {
    pub building_queue: BuildingQueue,
    pub research_queue: ResearchQueue,
    pub shipyard_queue: ShipyardQueue,
}

impl Default for QueueManager {
    fn default() -> Self {
        Self::new()
    }
}

impl QueueManager {
    pub fn new() -> Self {
        Self {
            building_queue: BuildingQueue::new(),
            research_queue: ResearchQueue::new(),
            shipyard_queue: ShipyardQueue::new(),
        }
    }

    /// Enqueue a building upgrade.
    #[allow(clippy::too_many_arguments)]
    pub fn enqueue_building(
        &mut self,
        planet_id: i32,
        building_type: &str,
        current_level: i32,
        resources_available: (i64, i64, i64),
        robotics_level: i32,
        nanite_level: i32,
        now: &str,
    ) -> Result<BuildingQueueItem, QueueError> {
        self.building_queue.enqueue(
            planet_id,
            building_type,
            current_level,
            resources_available,
            robotics_level,
            nanite_level,
            now,
        )
    }

    /// Enqueue a research upgrade.
    pub fn enqueue_research(
        &mut self,
        player_id: i32,
        research_type: &str,
        current_level: i32,
        resources_available: (i64, i64, i64),
        lab_level: i32,
        now: &str,
    ) -> Result<ResearchQueueItem, QueueError> {
        self.research_queue.enqueue(
            player_id,
            research_type,
            current_level,
            resources_available,
            lab_level,
            now,
        )
    }

    /// Enqueue a shipyard build order.
    #[allow(clippy::too_many_arguments)]
    pub fn enqueue_shipyard(
        &mut self,
        planet_id: i32,
        unit_type: &str,
        is_defense: bool,
        count: i32,
        shipyard_level: i32,
        nanite_level: i32,
        resources_available: (i64, i64, i64),
        now: &str,
    ) -> Result<ShipyardQueueItem, QueueError> {
        self.shipyard_queue.enqueue(
            planet_id,
            unit_type,
            is_defense,
            count,
            shipyard_level,
            nanite_level,
            resources_available,
            now,
        )
    }

    /// Process all completions across all queues for given planet/player IDs.
    pub fn process_all_completions(
        &mut self,
        planet_ids: &[i32],
        player_ids: &[i32],
        now: &str,
    ) -> QueueCompletions {
        let mut buildings = Vec::new();
        let mut research = Vec::new();
        let mut shipyard = Vec::new();

        for &pid in planet_ids {
            buildings.extend(self.building_queue.check_completion(pid, now));
            shipyard.extend(self.shipyard_queue.check_completion(pid, now));
        }

        for &pid in player_ids {
            research.extend(self.research_queue.check_completion(pid, now));
        }

        QueueCompletions {
            buildings,
            research,
            shipyard,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: &str = "2025-01-01T00:00:00Z";
    const LATER_1H: &str = "2025-01-01T01:00:00Z";
    const LATER_24H: &str = "2025-01-02T00:00:00Z";
    const MUCH_LATER: &str = "2025-06-01T00:00:00Z";
    const HUGE_RES: (i64, i64, i64) = (999_999_999, 999_999_999, 999_999_999);

    // =======================================================================
    // ISO timestamp helpers
    // =======================================================================

    #[test]
    fn test_iso_to_unix_epoch() {
        assert_eq!(iso_to_unix("1970-01-01T00:00:00Z"), 0);
    }

    #[test]
    fn test_iso_to_unix_known_date() {
        // 2025-01-01T00:00:00Z
        // Days from 1970 to 2025: 55 years
        let unix = iso_to_unix("2025-01-01T00:00:00Z");
        assert_eq!(unix, 1_735_689_600);
    }

    #[test]
    fn test_unix_to_iso_roundtrip() {
        let iso = "2025-06-15T12:30:45Z";
        let unix = iso_to_unix(iso);
        let back = unix_to_iso(unix);
        assert_eq!(back, iso);
    }

    #[test]
    fn test_iso_add_seconds() {
        let result = iso_add_seconds("2025-01-01T00:00:00Z", 3600.0);
        assert_eq!(result, "2025-01-01T01:00:00Z");
    }

    #[test]
    fn test_iso_diff_seconds() {
        let diff = iso_diff_seconds("2025-01-01T01:00:00Z", "2025-01-01T00:00:00Z");
        assert_eq!(diff, 3600.0);
    }

    // =======================================================================
    // Building cost formulas
    // =======================================================================

    #[test]
    fn test_building_cost_metal_mine_level_1() {
        let (m, c, d) = compute_building_cost("MetalMine", 1).unwrap();
        // base * 1.5^0 = base
        assert_eq!(m, 60);
        assert_eq!(c, 15);
        assert_eq!(d, 0);
    }

    #[test]
    fn test_building_cost_metal_mine_level_5() {
        let (m, c, d) = compute_building_cost("MetalMine", 5).unwrap();
        // 60 * 1.5^4 = 60 * 5.0625 = 303.75 -> 303
        // 15 * 1.5^4 = 15 * 5.0625 = 75.9375 -> 75
        assert_eq!(m, 303);
        assert_eq!(c, 75);
        assert_eq!(d, 0);
    }

    #[test]
    fn test_building_cost_crystal_mine_level_3() {
        let (m, c, _) = compute_building_cost("CrystalMine", 3).unwrap();
        // 48 * 1.6^2 = 48 * 2.56 = 122.88 -> 122
        // 24 * 1.6^2 = 24 * 2.56 = 61.44 -> 61
        assert_eq!(m, 122);
        assert_eq!(c, 61);
    }

    #[test]
    fn test_building_cost_invalid_type() {
        let result = compute_building_cost("BogusBuilding", 1);
        assert_eq!(
            result,
            Err(QueueError::InvalidType("BogusBuilding".to_string()))
        );
    }

    #[test]
    fn test_building_time_metal_mine_no_bonuses() {
        // base_secs = (60 + 15) / 2500 * 3600 = 108
        // time = 108 * 1.5^0 / (1+0) / 2^0 = 108
        let t = compute_building_time("MetalMine", 1, 0, 0);
        assert!((t - 108.0).abs() < 0.1, "got {t}");
    }

    #[test]
    fn test_building_time_with_robotics() {
        // robotics=10: 108 / 11 = 9.818
        let t = compute_building_time("MetalMine", 1, 10, 0);
        assert!((t - 9.818).abs() < 0.1, "got {t}");
    }

    #[test]
    fn test_building_time_with_nanite() {
        // nanite=2: 108 / 1 / 4 = 27
        let t = compute_building_time("MetalMine", 1, 0, 2);
        assert!((t - 27.0).abs() < 0.1, "got {t}");
    }

    #[test]
    fn test_building_time_with_robotics_and_nanite() {
        // robotics=10, nanite=2: 108 / 11 / 4 = 2.4545
        let t = compute_building_time("MetalMine", 1, 10, 2);
        assert!((t - 2.4545).abs() < 0.01, "got {t}");
    }

    // =======================================================================
    // Research cost formulas
    // =======================================================================

    #[test]
    fn test_research_cost_energy_tech_level_1() {
        let (m, c, d) = compute_research_cost("EnergyTechnology", 1).unwrap();
        assert_eq!(m, 0);
        assert_eq!(c, 800);
        assert_eq!(d, 400);
    }

    #[test]
    fn test_research_cost_weapons_tech_level_4() {
        let (m, c, _) = compute_research_cost("WeaponsTechnology", 4).unwrap();
        // 800 * 2^3 = 6400, 200 * 2^3 = 1600
        assert_eq!(m, 6400);
        assert_eq!(c, 1600);
    }

    #[test]
    fn test_research_cost_astrophysics_level_3() {
        let (m, c, d) = compute_research_cost("Astrophysics", 3).unwrap();
        // 4000 * 1.75^2 = 12250, 8000 * 1.75^2 = 24500
        assert_eq!(m, 12250);
        assert_eq!(c, 24500);
        assert_eq!(d, 12250);
    }

    #[test]
    fn test_research_cost_invalid_type() {
        let result = compute_research_cost("BogusResearch", 1);
        assert!(matches!(result, Err(QueueError::InvalidType(_))));
    }

    #[test]
    fn test_research_time_energy_tech() {
        // base_secs = (0 + 800) / 1000 * 3600 = 2880
        // level 1, lab 1: 2880 * 2^0 / (1+1) = 1440
        let t = compute_research_time("EnergyTechnology", 1, 1);
        assert!((t - 1440.0).abs() < 0.1, "got {t}");
    }

    // =======================================================================
    // Ship/defense cost formulas
    // =======================================================================

    #[test]
    fn test_ship_cost_cruiser() {
        let (m, c, d) = compute_unit_cost("Cruiser", false, 1).unwrap();
        assert_eq!(m, 20_000);
        assert_eq!(c, 7000);
        assert_eq!(d, 2000);
    }

    #[test]
    fn test_ship_cost_batch() {
        let (m, c, d) = compute_unit_cost("LightFighter", false, 10).unwrap();
        assert_eq!(m, 30_000);
        assert_eq!(c, 10_000);
        assert_eq!(d, 0);
    }

    #[test]
    fn test_defense_cost_gauss_cannon() {
        let (m, c, d) = compute_unit_cost("GaussCannon", true, 1).unwrap();
        assert_eq!(m, 20_000);
        assert_eq!(c, 15_000);
        assert_eq!(d, 2000);
    }

    #[test]
    fn test_defense_cost_batch() {
        let (m, c, _) = compute_unit_cost("RocketLauncher", true, 5).unwrap();
        assert_eq!(m, 10_000);
        assert_eq!(c, 0);
    }

    #[test]
    fn test_unit_cost_invalid_type() {
        let result = compute_unit_cost("BogusShip", false, 1);
        assert!(matches!(result, Err(QueueError::InvalidType(_))));
    }

    #[test]
    fn test_shipyard_unit_time() {
        // LightFighter: (3000+1000) / (2500 * (1+1) * 1) * 3600 = 4000/5000 * 3600 = 2880
        let t = compute_shipyard_unit_time("LightFighter", false, 1, 0);
        assert!((t - 2880.0).abs() < 0.1, "got {t}");
    }

    // =======================================================================
    // BuildingQueue
    // =======================================================================

    #[test]
    fn test_building_enqueue_success() {
        let mut q = BuildingQueue::new();
        let item = q.enqueue(1, "MetalMine", 0, HUGE_RES, 0, 0, NOW).unwrap();
        assert_eq!(item.planet_id, 1);
        assert_eq!(item.building_type, "MetalMine");
        assert_eq!(item.current_level, 0);
        assert_eq!(item.target_level, 1);
        assert_eq!(item.status, QueueStatus::InProgress);
        assert_eq!(item.metal_cost, 60);
        assert_eq!(item.crystal_cost, 15);
    }

    #[test]
    fn test_building_enqueue_already_building() {
        let mut q = BuildingQueue::new();
        q.enqueue(1, "MetalMine", 0, HUGE_RES, 0, 0, NOW).unwrap();
        let result = q.enqueue(1, "CrystalMine", 0, HUGE_RES, 0, 0, NOW);
        assert_eq!(result, Err(QueueError::AlreadyBuilding));
    }

    #[test]
    fn test_building_enqueue_different_planets() {
        let mut q = BuildingQueue::new();
        let a = q.enqueue(1, "MetalMine", 0, HUGE_RES, 0, 0, NOW);
        let b = q.enqueue(2, "MetalMine", 0, HUGE_RES, 0, 0, NOW);
        assert!(a.is_ok());
        assert!(b.is_ok());
    }

    #[test]
    fn test_building_enqueue_insufficient_resources() {
        let mut q = BuildingQueue::new();
        let result = q.enqueue(1, "MetalMine", 0, (0, 0, 0), 0, 0, NOW);
        assert!(matches!(
            result,
            Err(QueueError::InsufficientResources { .. })
        ));
    }

    #[test]
    fn test_building_enqueue_invalid_type() {
        let mut q = BuildingQueue::new();
        let result = q.enqueue(1, "Bogus", 0, HUGE_RES, 0, 0, NOW);
        assert!(matches!(result, Err(QueueError::InvalidType(_))));
    }

    #[test]
    fn test_building_active_build() {
        let mut q = BuildingQueue::new();
        assert!(q.active_build(1).is_none());
        q.enqueue(1, "MetalMine", 0, HUGE_RES, 0, 0, NOW).unwrap();
        assert!(q.active_build(1).is_some());
        assert!(q.active_build(2).is_none());
    }

    #[test]
    fn test_building_cancel() {
        let mut q = BuildingQueue::new();
        let item = q.enqueue(1, "MetalMine", 0, HUGE_RES, 0, 0, NOW).unwrap();
        let (m, c, d) = q.cancel(1, item.queue_id).unwrap();
        assert_eq!(m, 60);
        assert_eq!(c, 15);
        assert_eq!(d, 0);
        assert!(q.active_build(1).is_none());
    }

    #[test]
    fn test_building_cancel_not_found() {
        let mut q = BuildingQueue::new();
        assert_eq!(q.cancel(1, 999), Err(QueueError::NotFound));
    }

    #[test]
    fn test_building_check_completion() {
        let mut q = BuildingQueue::new();
        q.enqueue(1, "MetalMine", 0, HUGE_RES, 0, 0, NOW).unwrap();
        // Not completed yet at start
        let completed = q.check_completion(1, NOW);
        assert!(completed.is_empty());
        // Completed much later
        let completed = q.check_completion(1, MUCH_LATER);
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].status, QueueStatus::Completed);
    }

    #[test]
    fn test_building_can_enqueue_after_completion() {
        let mut q = BuildingQueue::new();
        q.enqueue(1, "MetalMine", 0, HUGE_RES, 0, 0, NOW).unwrap();
        q.check_completion(1, MUCH_LATER);
        // Should be able to enqueue again after completion
        let item = q
            .enqueue(1, "MetalMine", 1, HUGE_RES, 0, 0, MUCH_LATER)
            .unwrap();
        assert_eq!(item.target_level, 2);
    }

    #[test]
    fn test_building_can_enqueue_after_cancel() {
        let mut q = BuildingQueue::new();
        let item = q.enqueue(1, "MetalMine", 0, HUGE_RES, 0, 0, NOW).unwrap();
        q.cancel(1, item.queue_id).unwrap();
        let item2 = q.enqueue(1, "CrystalMine", 0, HUGE_RES, 0, 0, NOW).unwrap();
        assert_eq!(item2.building_type, "CrystalMine");
    }

    #[test]
    fn test_building_finish_time_increases_with_level() {
        let mut q = BuildingQueue::new();
        let item1 = q.enqueue(1, "MetalMine", 0, HUGE_RES, 0, 0, NOW).unwrap();
        q.check_completion(1, MUCH_LATER);

        let mut q2 = BuildingQueue::new();
        let item2 = q2.enqueue(1, "MetalMine", 4, HUGE_RES, 0, 0, NOW).unwrap();

        let finish1 = iso_to_unix(&item1.finish_time);
        let finish2 = iso_to_unix(&item2.finish_time);
        assert!(finish2 > finish1, "Higher level should take longer");
    }

    // =======================================================================
    // ResearchQueue
    // =======================================================================

    #[test]
    fn test_research_enqueue_success() {
        let mut q = ResearchQueue::new();
        let item = q
            .enqueue(1, "EnergyTechnology", 0, HUGE_RES, 1, NOW)
            .unwrap();
        assert_eq!(item.player_id, 1);
        assert_eq!(item.research_type, "EnergyTechnology");
        assert_eq!(item.target_level, 1);
        assert_eq!(item.status, QueueStatus::InProgress);
    }

    #[test]
    fn test_research_enqueue_already_researching() {
        let mut q = ResearchQueue::new();
        q.enqueue(1, "EnergyTechnology", 0, HUGE_RES, 1, NOW)
            .unwrap();
        let result = q.enqueue(1, "LaserTechnology", 0, HUGE_RES, 1, NOW);
        assert_eq!(result, Err(QueueError::AlreadyResearching));
    }

    #[test]
    fn test_research_enqueue_different_players() {
        let mut q = ResearchQueue::new();
        assert!(q
            .enqueue(1, "EnergyTechnology", 0, HUGE_RES, 1, NOW)
            .is_ok());
        assert!(q
            .enqueue(2, "EnergyTechnology", 0, HUGE_RES, 1, NOW)
            .is_ok());
    }

    #[test]
    fn test_research_enqueue_insufficient_resources() {
        let mut q = ResearchQueue::new();
        let result = q.enqueue(1, "EnergyTechnology", 0, (0, 0, 0), 1, NOW);
        assert!(matches!(
            result,
            Err(QueueError::InsufficientResources { .. })
        ));
    }

    #[test]
    fn test_research_active_research() {
        let mut q = ResearchQueue::new();
        assert!(q.active_research(1).is_none());
        q.enqueue(1, "EnergyTechnology", 0, HUGE_RES, 1, NOW)
            .unwrap();
        assert!(q.active_research(1).is_some());
    }

    #[test]
    fn test_research_cancel() {
        let mut q = ResearchQueue::new();
        let item = q
            .enqueue(1, "EnergyTechnology", 0, HUGE_RES, 1, NOW)
            .unwrap();
        let (m, c, d) = q.cancel(1, item.queue_id).unwrap();
        assert_eq!(c, 800);
        assert_eq!(d, 400);
        assert_eq!(m, 0);
        assert!(q.active_research(1).is_none());
    }

    #[test]
    fn test_research_check_completion() {
        let mut q = ResearchQueue::new();
        q.enqueue(1, "EnergyTechnology", 0, HUGE_RES, 1, NOW)
            .unwrap();
        let completed = q.check_completion(1, NOW);
        assert!(completed.is_empty());
        let completed = q.check_completion(1, MUCH_LATER);
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].status, QueueStatus::Completed);
    }

    #[test]
    fn test_research_can_enqueue_after_completion() {
        let mut q = ResearchQueue::new();
        q.enqueue(1, "EnergyTechnology", 0, HUGE_RES, 1, NOW)
            .unwrap();
        q.check_completion(1, MUCH_LATER);
        let item = q
            .enqueue(1, "LaserTechnology", 0, HUGE_RES, 1, MUCH_LATER)
            .unwrap();
        assert_eq!(item.research_type, "LaserTechnology");
    }

    // =======================================================================
    // ShipyardQueue
    // =======================================================================

    #[test]
    fn test_shipyard_enqueue_success() {
        let mut q = ShipyardQueue::new();
        let item = q
            .enqueue(1, "LightFighter", false, 5, 1, 0, HUGE_RES, NOW)
            .unwrap();
        assert_eq!(item.planet_id, 1);
        assert_eq!(item.unit_type, "LightFighter");
        assert_eq!(item.count, 5);
        assert!(!item.is_defense);
        assert_eq!(item.status, QueueStatus::InProgress);
        assert_eq!(item.metal_cost, 15_000);
        assert_eq!(item.crystal_cost, 5_000);
    }

    #[test]
    fn test_shipyard_enqueue_defense() {
        let mut q = ShipyardQueue::new();
        let item = q
            .enqueue(1, "RocketLauncher", true, 10, 1, 0, HUGE_RES, NOW)
            .unwrap();
        assert!(item.is_defense);
        assert_eq!(item.metal_cost, 20_000);
    }

    #[test]
    fn test_shipyard_multiple_queue() {
        let mut q = ShipyardQueue::new();
        let a = q
            .enqueue(1, "LightFighter", false, 5, 1, 0, HUGE_RES, NOW)
            .unwrap();
        let b = q
            .enqueue(1, "Cruiser", false, 2, 1, 0, HUGE_RES, NOW)
            .unwrap();
        assert_eq!(a.status, QueueStatus::InProgress);
        assert_eq!(b.status, QueueStatus::Pending);
        assert_eq!(q.queue_length(1), 2);
    }

    #[test]
    fn test_shipyard_sequential_timing() {
        let mut q = ShipyardQueue::new();
        let a = q
            .enqueue(1, "LightFighter", false, 1, 1, 0, HUGE_RES, NOW)
            .unwrap();
        let b = q
            .enqueue(1, "LightFighter", false, 1, 1, 0, HUGE_RES, NOW)
            .unwrap();
        // Second item should start after first finishes
        assert_eq!(b.start_time, a.finish_time);
    }

    #[test]
    fn test_shipyard_insufficient_resources() {
        let mut q = ShipyardQueue::new();
        let result = q.enqueue(1, "Deathstar", false, 1, 1, 0, (0, 0, 0), NOW);
        assert!(matches!(
            result,
            Err(QueueError::InsufficientResources { .. })
        ));
    }

    #[test]
    fn test_shipyard_invalid_count() {
        let mut q = ShipyardQueue::new();
        let result = q.enqueue(1, "LightFighter", false, 0, 1, 0, HUGE_RES, NOW);
        assert!(matches!(result, Err(QueueError::InvalidState(_))));
    }

    #[test]
    fn test_shipyard_cancel_last() {
        let mut q = ShipyardQueue::new();
        q.enqueue(1, "LightFighter", false, 5, 1, 0, HUGE_RES, NOW)
            .unwrap();
        q.enqueue(1, "Cruiser", false, 2, 1, 0, HUGE_RES, NOW)
            .unwrap();
        let (m, c, d) = q.cancel_last(1).unwrap();
        // Cruiser x2: 40000m, 14000c, 4000d
        assert_eq!(m, 40_000);
        assert_eq!(c, 14_000);
        assert_eq!(d, 4_000);
        assert_eq!(q.queue_length(1), 1);
    }

    #[test]
    fn test_shipyard_cancel_last_only_item() {
        let mut q = ShipyardQueue::new();
        q.enqueue(1, "LightFighter", false, 1, 1, 0, HUGE_RES, NOW)
            .unwrap();
        let result = q.cancel_last(1);
        assert!(result.is_ok());
        assert_eq!(q.queue_length(1), 0);
    }

    #[test]
    fn test_shipyard_cancel_last_not_found() {
        let mut q = ShipyardQueue::new();
        assert_eq!(q.cancel_last(1), Err(QueueError::NotFound));
    }

    #[test]
    fn test_shipyard_check_completion() {
        let mut q = ShipyardQueue::new();
        q.enqueue(1, "LightFighter", false, 1, 1, 0, HUGE_RES, NOW)
            .unwrap();
        let completed = q.check_completion(1, NOW);
        assert!(completed.is_empty());
        let completed = q.check_completion(1, MUCH_LATER);
        assert_eq!(completed.len(), 1);
    }

    #[test]
    fn test_shipyard_promotes_pending_after_completion() {
        let mut q = ShipyardQueue::new();
        q.enqueue(1, "LightFighter", false, 1, 1, 0, HUGE_RES, NOW)
            .unwrap();
        q.enqueue(1, "Cruiser", false, 1, 1, 0, HUGE_RES, NOW)
            .unwrap();
        q.check_completion(1, MUCH_LATER);
        let remaining = q.list_queue(1);
        // After completion of both (MUCH_LATER is far enough), nothing pending
        // But if only first completed, second would be promoted
        assert!(remaining.is_empty() || remaining[0].status == QueueStatus::InProgress);
    }

    #[test]
    fn test_shipyard_list_queue() {
        let mut q = ShipyardQueue::new();
        q.enqueue(1, "LightFighter", false, 5, 1, 0, HUGE_RES, NOW)
            .unwrap();
        q.enqueue(1, "Cruiser", false, 2, 1, 0, HUGE_RES, NOW)
            .unwrap();
        let list = q.list_queue(1);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].unit_type, "LightFighter");
        assert_eq!(list[1].unit_type, "Cruiser");
    }

    #[test]
    fn test_shipyard_queue_length_empty() {
        let q = ShipyardQueue::new();
        assert_eq!(q.queue_length(1), 0);
    }

    #[test]
    fn test_shipyard_each_build_time_stored() {
        let mut q = ShipyardQueue::new();
        let item = q
            .enqueue(1, "LightFighter", false, 1, 1, 0, HUGE_RES, NOW)
            .unwrap();
        // Per unit time for LightFighter at shipyard=1, nanite=0:
        // (3000+1000)/(2500*2*1) * 3600 = 2880
        assert!((item.each_build_time_secs - 2880.0).abs() < 0.1);
    }

    // =======================================================================
    // LazyResourceState
    // =======================================================================

    #[test]
    fn test_lazy_evaluate_no_time_passed() {
        let state = LazyResourceState {
            metal: 1000,
            crystal: 500,
            deuterium: 200,
            metal_per_hour: 300.0,
            crystal_per_hour: 200.0,
            deuterium_per_hour: 100.0,
            last_updated: NOW.to_string(),
            storage_metal: 0,
            storage_crystal: 0,
            storage_deuterium: 0,
        };
        let (m, c, d) = state.evaluate(NOW);
        assert_eq!(m, 1000);
        assert_eq!(c, 500);
        assert_eq!(d, 200);
    }

    #[test]
    fn test_lazy_evaluate_one_hour() {
        let state = LazyResourceState {
            metal: 1000,
            crystal: 500,
            deuterium: 200,
            metal_per_hour: 300.0,
            crystal_per_hour: 200.0,
            deuterium_per_hour: 100.0,
            last_updated: NOW.to_string(),
            storage_metal: 0,
            storage_crystal: 0,
            storage_deuterium: 0,
        };
        let (m, c, d) = state.evaluate(LATER_1H);
        assert_eq!(m, 1300);
        assert_eq!(c, 700);
        assert_eq!(d, 300);
    }

    #[test]
    fn test_lazy_evaluate_capped_by_storage() {
        let state = LazyResourceState {
            metal: 9000,
            crystal: 500,
            deuterium: 200,
            metal_per_hour: 3000.0,
            crystal_per_hour: 200.0,
            deuterium_per_hour: 100.0,
            last_updated: NOW.to_string(),
            storage_metal: 10_000,
            storage_crystal: 100_000,
            storage_deuterium: 100_000,
        };
        let (m, _c, _d) = state.evaluate(LATER_1H);
        // 9000 + 3000 = 12000, capped at 10000
        assert_eq!(m, 10_000);
    }

    #[test]
    fn test_lazy_evaluate_past_timestamp() {
        let state = LazyResourceState {
            metal: 1000,
            crystal: 500,
            deuterium: 200,
            metal_per_hour: 300.0,
            crystal_per_hour: 200.0,
            deuterium_per_hour: 100.0,
            last_updated: LATER_1H.to_string(),
            storage_metal: 0,
            storage_crystal: 0,
            storage_deuterium: 0,
        };
        // Evaluate at an earlier time should return current values (no negative accrual)
        let (m, c, d) = state.evaluate(NOW);
        assert_eq!(m, 1000);
        assert_eq!(c, 500);
        assert_eq!(d, 200);
    }

    #[test]
    fn test_lazy_spend_success() {
        let mut state = LazyResourceState {
            metal: 1000,
            crystal: 500,
            deuterium: 200,
            metal_per_hour: 0.0,
            crystal_per_hour: 0.0,
            deuterium_per_hour: 0.0,
            last_updated: NOW.to_string(),
            storage_metal: 0,
            storage_crystal: 0,
            storage_deuterium: 0,
        };
        assert!(state.spend(500, 200, 100, NOW).is_ok());
        assert_eq!(state.metal, 500);
        assert_eq!(state.crystal, 300);
        assert_eq!(state.deuterium, 100);
    }

    #[test]
    fn test_lazy_spend_insufficient() {
        let mut state = LazyResourceState {
            metal: 100,
            crystal: 500,
            deuterium: 200,
            metal_per_hour: 0.0,
            crystal_per_hour: 0.0,
            deuterium_per_hour: 0.0,
            last_updated: NOW.to_string(),
            storage_metal: 0,
            storage_crystal: 0,
            storage_deuterium: 0,
        };
        let result = state.spend(500, 0, 0, NOW);
        assert!(matches!(
            result,
            Err(QueueError::InsufficientResources { .. })
        ));
        // State unchanged on failure
        assert_eq!(state.metal, 100);
    }

    #[test]
    fn test_lazy_spend_updates_last_updated() {
        let mut state = LazyResourceState {
            metal: 1000,
            crystal: 500,
            deuterium: 200,
            metal_per_hour: 300.0,
            crystal_per_hour: 200.0,
            deuterium_per_hour: 100.0,
            last_updated: NOW.to_string(),
            storage_metal: 0,
            storage_crystal: 0,
            storage_deuterium: 0,
        };
        state.spend(100, 100, 100, LATER_1H).unwrap();
        assert_eq!(state.last_updated, LATER_1H);
        // After 1h: metal=1000+300=1300-100=1200
        assert_eq!(state.metal, 1200);
    }

    // =======================================================================
    // QueueManager
    // =======================================================================

    #[test]
    fn test_queue_manager_new() {
        let qm = QueueManager::new();
        assert_eq!(qm.building_queue.next_id, 1);
        assert_eq!(qm.research_queue.next_id, 1);
        assert_eq!(qm.shipyard_queue.next_id, 1);
    }

    #[test]
    fn test_queue_manager_enqueue_building() {
        let mut qm = QueueManager::new();
        let item = qm
            .enqueue_building(1, "MetalMine", 0, HUGE_RES, 0, 0, NOW)
            .unwrap();
        assert_eq!(item.building_type, "MetalMine");
    }

    #[test]
    fn test_queue_manager_enqueue_research() {
        let mut qm = QueueManager::new();
        let item = qm
            .enqueue_research(1, "EnergyTechnology", 0, HUGE_RES, 1, NOW)
            .unwrap();
        assert_eq!(item.research_type, "EnergyTechnology");
    }

    #[test]
    fn test_queue_manager_enqueue_shipyard() {
        let mut qm = QueueManager::new();
        let item = qm
            .enqueue_shipyard(1, "LightFighter", false, 5, 1, 0, HUGE_RES, NOW)
            .unwrap();
        assert_eq!(item.unit_type, "LightFighter");
        assert_eq!(item.count, 5);
    }

    #[test]
    fn test_queue_manager_process_all_completions() {
        let mut qm = QueueManager::new();
        qm.enqueue_building(1, "MetalMine", 0, HUGE_RES, 0, 0, NOW)
            .unwrap();
        qm.enqueue_research(42, "EnergyTechnology", 0, HUGE_RES, 1, NOW)
            .unwrap();
        qm.enqueue_shipyard(1, "LightFighter", false, 1, 1, 0, HUGE_RES, NOW)
            .unwrap();

        let completions = qm.process_all_completions(&[1], &[42], MUCH_LATER);
        assert_eq!(completions.buildings.len(), 1);
        assert_eq!(completions.research.len(), 1);
        assert_eq!(completions.shipyard.len(), 1);
    }

    #[test]
    fn test_queue_manager_process_no_completions_yet() {
        let mut qm = QueueManager::new();
        qm.enqueue_building(1, "MetalMine", 0, HUGE_RES, 0, 0, NOW)
            .unwrap();
        let completions = qm.process_all_completions(&[1], &[], NOW);
        assert!(completions.buildings.is_empty());
    }

    // =======================================================================
    // QueueError Display
    // =======================================================================

    #[test]
    fn test_queue_error_display() {
        let e = QueueError::InsufficientResources {
            needed_metal: 100,
            needed_crystal: 200,
            needed_deuterium: 50,
        };
        let s = format!("{e}");
        assert!(s.contains("100m"));
        assert!(s.contains("200c"));
        assert!(s.contains("50d"));
    }

    #[test]
    fn test_queue_error_display_already_building() {
        let e = QueueError::AlreadyBuilding;
        assert!(format!("{e}").contains("active building"));
    }

    // =======================================================================
    // Serialization round-trips
    // =======================================================================

    #[test]
    fn test_building_queue_item_serde() {
        let item = BuildingQueueItem {
            queue_id: 1,
            planet_id: 42,
            building_type: "MetalMine".to_string(),
            current_level: 5,
            target_level: 6,
            status: QueueStatus::InProgress,
            start_time: NOW.to_string(),
            finish_time: LATER_1H.to_string(),
            metal_cost: 303,
            crystal_cost: 75,
            deuterium_cost: 0,
        };
        let json = serde_json::to_string(&item).unwrap();
        let deser: BuildingQueueItem = serde_json::from_str(&json).unwrap();
        assert_eq!(item, deser);
    }

    #[test]
    fn test_research_queue_item_serde() {
        let item = ResearchQueueItem {
            queue_id: 1,
            player_id: 7,
            research_type: "EnergyTechnology".to_string(),
            current_level: 0,
            target_level: 1,
            status: QueueStatus::InProgress,
            start_time: NOW.to_string(),
            finish_time: LATER_1H.to_string(),
            metal_cost: 0,
            crystal_cost: 800,
            deuterium_cost: 400,
        };
        let json = serde_json::to_string(&item).unwrap();
        let deser: ResearchQueueItem = serde_json::from_str(&json).unwrap();
        assert_eq!(item, deser);
    }

    #[test]
    fn test_shipyard_queue_item_serde() {
        let item = ShipyardQueueItem {
            order_id: 1,
            planet_id: 3,
            unit_type: "LightFighter".to_string(),
            is_defense: false,
            count: 10,
            each_build_time_secs: 2880.0,
            status: QueueStatus::Pending,
            start_time: NOW.to_string(),
            finish_time: LATER_1H.to_string(),
            metal_cost: 30_000,
            crystal_cost: 10_000,
            deuterium_cost: 0,
        };
        let json = serde_json::to_string(&item).unwrap();
        let deser: ShipyardQueueItem = serde_json::from_str(&json).unwrap();
        assert_eq!(item, deser);
    }

    #[test]
    fn test_queue_status_serde() {
        for status in [
            QueueStatus::Pending,
            QueueStatus::InProgress,
            QueueStatus::Completed,
            QueueStatus::Cancelled,
        ] {
            let json = serde_json::to_string(&status).unwrap();
            let deser: QueueStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, deser);
        }
    }

    #[test]
    fn test_lazy_resource_state_serde() {
        let state = LazyResourceState {
            metal: 5000,
            crystal: 3000,
            deuterium: 1000,
            metal_per_hour: 300.0,
            crystal_per_hour: 200.0,
            deuterium_per_hour: 100.0,
            last_updated: NOW.to_string(),
            storage_metal: 100_000,
            storage_crystal: 50_000,
            storage_deuterium: 25_000,
        };
        let json = serde_json::to_string(&state).unwrap();
        let deser: LazyResourceState = serde_json::from_str(&json).unwrap();
        assert_eq!(state, deser);
    }

    // =======================================================================
    // Edge cases
    // =======================================================================

    #[test]
    fn test_building_time_minimum_one_second() {
        // Extremely high robotics + nanite should still give at least 1 second
        let t = compute_building_time("MetalMine", 1, 100, 20);
        assert!((t - 1.0).abs() < 0.01, "got {t}");
    }

    #[test]
    fn test_research_time_minimum_one_second() {
        // GravitonTechnology has 0 metal+crystal base, so time formula yields 0
        // which should be clamped to 1 second minimum.
        let t = compute_research_time("GravitonTechnology", 1, 10);
        assert!((t - 1.0).abs() < 0.01, "got {t}");
    }

    #[test]
    fn test_shipyard_unit_time_minimum_one_second() {
        let t = compute_shipyard_unit_time("EspionageProbe", false, 50, 10);
        assert!(t >= 1.0, "got {t}");
    }

    #[test]
    fn test_building_queue_auto_increment_ids() {
        let mut q = BuildingQueue::new();
        let a = q.enqueue(1, "MetalMine", 0, HUGE_RES, 0, 0, NOW).unwrap();
        q.check_completion(1, MUCH_LATER);
        let b = q
            .enqueue(1, "MetalMine", 1, HUGE_RES, 0, 0, MUCH_LATER)
            .unwrap();
        assert_eq!(a.queue_id, 1);
        assert_eq!(b.queue_id, 2);
    }

    #[test]
    fn test_queue_completions_serde() {
        let completions = QueueCompletions {
            buildings: vec![],
            research: vec![],
            shipyard: vec![],
        };
        let json = serde_json::to_string(&completions).unwrap();
        let deser: QueueCompletions = serde_json::from_str(&json).unwrap();
        assert_eq!(completions, deser);
    }

    #[test]
    fn test_fusion_reactor_high_cost() {
        // FusionReactor level 5: 900 * 1.8^4 = 900 * 10.4976 = 9447 metal
        let (m, c, d) = compute_building_cost("FusionReactor", 5).unwrap();
        assert_eq!(m, 9447);
        assert_eq!(c, (360.0 * 1.8_f64.powi(4)).floor() as i64);
        assert_eq!(d, (180.0 * 1.8_f64.powi(4)).floor() as i64);
    }

    #[test]
    fn test_nanite_factory_enormous_cost() {
        let (m, c, d) = compute_building_cost("NaniteFactory", 1).unwrap();
        assert_eq!(m, 1_000_000);
        assert_eq!(c, 500_000);
        assert_eq!(d, 100_000);
    }

    #[test]
    fn test_deathstar_cost() {
        let (m, c, d) = compute_unit_cost("Deathstar", false, 1).unwrap();
        assert_eq!(m, 5_000_000);
        assert_eq!(c, 4_000_000);
        assert_eq!(d, 1_000_000);
    }

    #[test]
    fn test_24h_resource_accumulation() {
        let state = LazyResourceState {
            metal: 0,
            crystal: 0,
            deuterium: 0,
            metal_per_hour: 100.0,
            crystal_per_hour: 50.0,
            deuterium_per_hour: 25.0,
            last_updated: NOW.to_string(),
            storage_metal: 0,
            storage_crystal: 0,
            storage_deuterium: 0,
        };
        let (m, c, d) = state.evaluate(LATER_24H);
        assert_eq!(m, 2400);
        assert_eq!(c, 1200);
        assert_eq!(d, 600);
    }
}
