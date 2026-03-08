#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Espionage Technology
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EspionageTech {
    pub level: u32,
}

// ---------------------------------------------------------------------------
// Report Detail Levels
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReportDetail {
    Resources,
    Fleet,
    Defense,
    Buildings,
    Research,
}

/// Probability (0.0..=1.0) that probes are detected/destroyed.
///
/// Base chance = `0.02 * defender_tech * num_probes`, then modified by the
/// technology difference.  When the defender has higher tech the chance
/// increases; when the attacker has higher tech it decreases.
pub fn counter_espionage_chance(attacker_tech: u32, defender_tech: u32, num_probes: u32) -> f64 {
    let base = 0.02 * defender_tech as f64 * num_probes as f64;
    let tech_diff = defender_tech as f64 - attacker_tech as f64;
    let modifier = 1.0 + tech_diff * 0.05;
    let chance = base * modifier;
    chance.clamp(0.0, 1.0)
}

/// Determine the maximum report detail level an attacker will receive.
///
/// `tech_diff = attacker_tech * sqrt(num_probes) - defender_tech`
///
/// | tech_diff | detail      |
/// |-----------|-------------|
/// | >= 0      | Resources   |
/// | >= 2      | Fleet       |
/// | >= 4      | Defense     |
/// | >= 7      | Buildings   |
/// | >= 10     | Research    |
///
/// If `tech_diff < 0` the attacker still gets `Resources` (always visible).
pub fn report_detail_level(
    attacker_tech: u32,
    defender_tech: u32,
    num_probes: u32,
) -> ReportDetail {
    let tech_diff = attacker_tech as f64 * (num_probes as f64).sqrt() - defender_tech as f64;

    if tech_diff >= 10.0 {
        ReportDetail::Research
    } else if tech_diff >= 7.0 {
        ReportDetail::Buildings
    } else if tech_diff >= 4.0 {
        ReportDetail::Defense
    } else if tech_diff >= 2.0 {
        ReportDetail::Fleet
    } else {
        ReportDetail::Resources
    }
}

// ---------------------------------------------------------------------------
// Snapshot / Intel structs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Coordinates {
    pub galaxy: u16,
    pub system: u16,
    pub position: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResourceSnapshot {
    pub metal: i64,
    pub crystal: i64,
    pub deuterium: i64,
    pub energy: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShipCount {
    pub ship_type: String,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FleetSnapshot {
    pub ships: Vec<ShipCount>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DefenseCount {
    pub defense_type: String,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DefenseSnapshot {
    pub defenses: Vec<DefenseCount>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BuildingLevel {
    pub building_type: String,
    pub level: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BuildingSnapshot {
    pub buildings: Vec<BuildingLevel>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TechLevel {
    pub tech_type: String,
    pub level: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResearchSnapshot {
    pub technologies: Vec<TechLevel>,
}

// ---------------------------------------------------------------------------
// Espionage Report
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EspionageReport {
    pub id: u64,
    pub attacker_id: String,
    pub defender_id: String,
    pub target_coordinates: Coordinates,
    pub detail_level: ReportDetail,
    pub timestamp: String,
    pub resources: Option<ResourceSnapshot>,
    pub fleet: Option<FleetSnapshot>,
    pub defenses: Option<DefenseSnapshot>,
    pub buildings: Option<BuildingSnapshot>,
    pub research: Option<ResearchSnapshot>,
    pub probes_sent: u32,
    pub probes_lost: u32,
}

// ---------------------------------------------------------------------------
// Spy Mission
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SpyMissionStatus {
    InTransit,
    Completed,
    ProbesDestroyed,
    Aborted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpyMission {
    pub id: u64,
    pub attacker_id: String,
    pub defender_id: String,
    pub target: Coordinates,
    pub probes_sent: u32,
    pub status: SpyMissionStatus,
    pub launched_at: String,
    pub arrival_at: String,
    pub report_id: Option<u64>,
}

// ---------------------------------------------------------------------------
// Planet Intel — full state used as input to mission resolution
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlanetIntel {
    pub resources: ResourceSnapshot,
    pub fleet: FleetSnapshot,
    pub defenses: DefenseSnapshot,
    pub buildings: BuildingSnapshot,
    pub research: ResearchSnapshot,
}

// ---------------------------------------------------------------------------
// Spy Mission Result
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpyMissionResult {
    pub report: Option<EspionageReport>,
    pub probes_lost: u32,
    pub probes_survived: u32,
    pub detected: bool,
}

// ---------------------------------------------------------------------------
// Counter-Espionage
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CounterEspionageResult {
    pub probes_destroyed: u32,
    pub probes_escaped: u32,
    pub detected: bool,
    pub detection_message: Option<String>,
}

// ---------------------------------------------------------------------------
// Deterministic hash-based PRNG for counter-espionage
// ---------------------------------------------------------------------------

/// Simple deterministic pseudo-random number in [0.0, 1.0) derived from a
/// seed.  Uses a splitmix-style hash so results are reproducible across runs.
fn deterministic_random(seed: u64) -> f64 {
    let mut z = seed.wrapping_add(0x9E3779B97F4A7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z = z ^ (z >> 31);
    // Map to [0.0, 1.0)
    (z as f64) / (u64::MAX as f64)
}

/// Resolve counter-espionage: for each probe, deterministically decide if it
/// is destroyed using a hash-based pseudo-random derived from `probe_index`.
///
/// The `mission_seed` is combined with each probe index to produce a unique
/// but reproducible random value.
fn resolve_counter_espionage_seeded(
    defender_tech: u32,
    attacker_tech: u32,
    num_probes: u32,
    mission_seed: u64,
) -> CounterEspionageResult {
    let chance = counter_espionage_chance(attacker_tech, defender_tech, num_probes);
    let mut destroyed: u32 = 0;

    for i in 0..num_probes {
        let seed = mission_seed
            .wrapping_mul(31)
            .wrapping_add(i as u64)
            .wrapping_add(defender_tech as u64 * 1000)
            .wrapping_add(attacker_tech as u64 * 7);
        let roll = deterministic_random(seed);
        if roll < chance {
            destroyed += 1;
        }
    }

    let escaped = num_probes - destroyed;
    let detected = destroyed > 0;
    let detection_message = if detected {
        Some(format!(
            "Counter-espionage detected! {} of {} probes were destroyed.",
            destroyed, num_probes
        ))
    } else {
        None
    };

    CounterEspionageResult {
        probes_destroyed: destroyed,
        probes_escaped: escaped,
        detected,
        detection_message,
    }
}

/// Public counter-espionage resolution using the mission ID as seed (0 if not
/// available).
pub fn resolve_counter_espionage(
    defender_tech: u32,
    attacker_tech: u32,
    num_probes: u32,
) -> CounterEspionageResult {
    resolve_counter_espionage_seeded(defender_tech, attacker_tech, num_probes, 0)
}

// ---------------------------------------------------------------------------
// Mission Resolution
// ---------------------------------------------------------------------------

/// Build an `EspionageReport` by filtering `PlanetIntel` according to the
/// attacker's maximum detail level.
fn build_report(
    mission: &SpyMission,
    detail: ReportDetail,
    planet: &PlanetIntel,
    probes_lost: u32,
) -> EspionageReport {
    let resources = Some(planet.resources.clone());

    let fleet = if detail >= ReportDetail::Fleet {
        Some(planet.fleet.clone())
    } else {
        None
    };

    let defenses = if detail >= ReportDetail::Defense {
        Some(planet.defenses.clone())
    } else {
        None
    };

    let buildings = if detail >= ReportDetail::Buildings {
        Some(planet.buildings.clone())
    } else {
        None
    };

    let research = if detail >= ReportDetail::Research {
        Some(planet.research.clone())
    } else {
        None
    };

    EspionageReport {
        id: 0, // Assigned by store
        attacker_id: mission.attacker_id.clone(),
        defender_id: mission.defender_id.clone(),
        target_coordinates: mission.target.clone(),
        detail_level: detail,
        timestamp: mission.arrival_at.clone(),
        resources,
        fleet,
        defenses,
        buildings,
        research,
        probes_sent: mission.probes_sent,
        probes_lost,
    }
}

/// Resolve a spy mission: run counter-espionage, determine detail level, and
/// produce an espionage report (if any probes survive).
pub fn resolve_spy_mission(
    mission: &SpyMission,
    attacker_tech: u32,
    defender_tech: u32,
    defender_planet: &PlanetIntel,
) -> SpyMissionResult {
    let counter = resolve_counter_espionage_seeded(
        defender_tech,
        attacker_tech,
        mission.probes_sent,
        mission.id,
    );

    let probes_lost = counter.probes_destroyed;
    let probes_survived = counter.probes_escaped;
    let detected = counter.detected;

    if probes_survived == 0 {
        return SpyMissionResult {
            report: None,
            probes_lost,
            probes_survived,
            detected,
        };
    }

    // Surviving probes determine intel quality
    let detail = report_detail_level(attacker_tech, defender_tech, probes_survived);
    let report = build_report(mission, detail, defender_planet, probes_lost);

    SpyMissionResult {
        report: Some(report),
        probes_lost,
        probes_survived,
        detected,
    }
}

// ---------------------------------------------------------------------------
// Utility
// ---------------------------------------------------------------------------

/// Calculate the flight time (in seconds) for spy probes to reach a target.
///
/// `flight_time = distance / (probe_speed * universe_speed)`
pub fn espionage_flight_time(distance: f64, probe_speed: f64, universe_speed: u32) -> f64 {
    if probe_speed <= 0.0 || universe_speed == 0 {
        return f64::INFINITY;
    }
    distance / (probe_speed * universe_speed as f64)
}

// ---------------------------------------------------------------------------
// Espionage Store
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct EspionageStore {
    missions: HashMap<u64, SpyMission>,
    reports: HashMap<u64, EspionageReport>,
    next_mission_id: u64,
    next_report_id: u64,
}

impl Default for EspionageStore {
    fn default() -> Self {
        Self::new()
    }
}

impl EspionageStore {
    pub fn new() -> Self {
        Self {
            missions: HashMap::new(),
            reports: HashMap::new(),
            next_mission_id: 1,
            next_report_id: 1,
        }
    }

    pub fn create_mission(
        &mut self,
        attacker_id: &str,
        defender_id: &str,
        target: Coordinates,
        probes: u32,
    ) -> SpyMission {
        let id = self.next_mission_id;
        self.next_mission_id += 1;

        let mission = SpyMission {
            id,
            attacker_id: attacker_id.to_string(),
            defender_id: defender_id.to_string(),
            target,
            probes_sent: probes,
            status: SpyMissionStatus::InTransit,
            launched_at: String::new(),
            arrival_at: String::new(),
            report_id: None,
        };

        self.missions.insert(id, mission.clone());
        mission
    }

    pub fn complete_mission(
        &mut self,
        mission_id: u64,
        result: SpyMissionResult,
    ) -> Option<&SpyMission> {
        if !self.missions.contains_key(&mission_id) {
            return None;
        }

        // Store report first (if any) to avoid double mutable borrow
        let report_id = if let Some(mut report) = result.report {
            Some(self.store_report_inner(&mut report))
        } else {
            None
        };

        let mission = self.missions.get_mut(&mission_id).unwrap();

        if result.probes_survived == 0 {
            mission.status = SpyMissionStatus::ProbesDestroyed;
        } else {
            mission.status = SpyMissionStatus::Completed;
        }

        mission.report_id = report_id;

        self.missions.get(&mission_id)
    }

    pub fn get_mission(&self, id: u64) -> Option<&SpyMission> {
        self.missions.get(&id)
    }

    pub fn list_missions_by_attacker(&self, attacker_id: &str) -> Vec<&SpyMission> {
        self.missions
            .values()
            .filter(|m| m.attacker_id == attacker_id)
            .collect()
    }

    pub fn list_missions_by_defender(&self, defender_id: &str) -> Vec<&SpyMission> {
        self.missions
            .values()
            .filter(|m| m.defender_id == defender_id)
            .collect()
    }

    pub fn store_report(&mut self, mut report: EspionageReport) -> u64 {
        self.store_report_inner(&mut report)
    }

    fn store_report_inner(&mut self, report: &mut EspionageReport) -> u64 {
        let id = self.next_report_id;
        self.next_report_id += 1;
        report.id = id;
        self.reports.insert(id, report.clone());
        id
    }

    pub fn get_report(&self, id: u64) -> Option<&EspionageReport> {
        self.reports.get(&id)
    }

    pub fn list_reports_for_player(&self, player_id: &str, limit: usize) -> Vec<&EspionageReport> {
        let mut reports: Vec<&EspionageReport> = self
            .reports
            .values()
            .filter(|r| r.attacker_id == player_id || r.defender_id == player_id)
            .collect();
        // Sort by id descending (most recent first)
        reports.sort_by(|a, b| b.id.cmp(&a.id));
        reports.truncate(limit);
        reports
    }

    pub fn delete_report(&mut self, id: u64) -> bool {
        self.reports.remove(&id).is_some()
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn sample_coords() -> Coordinates {
        Coordinates {
            galaxy: 1,
            system: 200,
            position: 8,
        }
    }

    fn sample_planet_intel() -> PlanetIntel {
        PlanetIntel {
            resources: ResourceSnapshot {
                metal: 100_000,
                crystal: 50_000,
                deuterium: 25_000,
                energy: 500,
            },
            fleet: FleetSnapshot {
                ships: vec![
                    ShipCount {
                        ship_type: "LightFighter".into(),
                        count: 100,
                    },
                    ShipCount {
                        ship_type: "HeavyFighter".into(),
                        count: 50,
                    },
                ],
            },
            defenses: DefenseSnapshot {
                defenses: vec![
                    DefenseCount {
                        defense_type: "RocketLauncher".into(),
                        count: 200,
                    },
                    DefenseCount {
                        defense_type: "LightLaser".into(),
                        count: 100,
                    },
                ],
            },
            buildings: BuildingSnapshot {
                buildings: vec![
                    BuildingLevel {
                        building_type: "MetalMine".into(),
                        level: 20,
                    },
                    BuildingLevel {
                        building_type: "CrystalMine".into(),
                        level: 18,
                    },
                ],
            },
            research: ResearchSnapshot {
                technologies: vec![
                    TechLevel {
                        tech_type: "EspionageTech".into(),
                        level: 8,
                    },
                    TechLevel {
                        tech_type: "WeaponsTech".into(),
                        level: 10,
                    },
                ],
            },
        }
    }

    // -----------------------------------------------------------------------
    // EspionageTech struct
    // -----------------------------------------------------------------------

    #[test]
    fn test_espionage_tech_creation() {
        let tech = EspionageTech { level: 5 };
        assert_eq!(tech.level, 5);
    }

    #[test]
    fn test_espionage_tech_serde() {
        let tech = EspionageTech { level: 12 };
        let json = serde_json::to_string(&tech).unwrap();
        let deserialized: EspionageTech = serde_json::from_str(&json).unwrap();
        assert_eq!(tech, deserialized);
    }

    // -----------------------------------------------------------------------
    // counter_espionage_chance
    // -----------------------------------------------------------------------

    #[test]
    fn test_counter_espionage_chance_equal_tech() {
        // defender_tech=5, attacker_tech=5, probes=1
        // base = 0.02 * 5 * 1 = 0.10
        // tech_diff = 0, modifier = 1.0
        // chance = 0.10
        let chance = counter_espionage_chance(5, 5, 1);
        assert!((chance - 0.10).abs() < 1e-9);
    }

    #[test]
    fn test_counter_espionage_chance_attacker_higher() {
        // attacker=10, defender=5, probes=1
        // base = 0.02 * 5 * 1 = 0.10
        // tech_diff = 5 - 10 = -5, modifier = 1 + (-5 * 0.05) = 0.75
        // chance = 0.10 * 0.75 = 0.075
        let chance = counter_espionage_chance(10, 5, 1);
        assert!((chance - 0.075).abs() < 1e-9);
    }

    #[test]
    fn test_counter_espionage_chance_defender_higher() {
        // attacker=3, defender=8, probes=2
        // base = 0.02 * 8 * 2 = 0.32
        // tech_diff = 8 - 3 = 5, modifier = 1 + 5 * 0.05 = 1.25
        // chance = 0.32 * 1.25 = 0.40
        let chance = counter_espionage_chance(3, 8, 2);
        assert!((chance - 0.40).abs() < 1e-9);
    }

    #[test]
    fn test_counter_espionage_chance_clamped_at_one() {
        // Very high defender tech, many probes -> should clamp to 1.0
        let chance = counter_espionage_chance(0, 50, 50);
        assert!((chance - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_counter_espionage_chance_zero_defender_tech() {
        // defender_tech=0 -> base = 0
        let chance = counter_espionage_chance(10, 0, 5);
        assert!((chance - 0.0).abs() < 1e-9);
    }

    #[test]
    fn test_counter_espionage_chance_zero_probes() {
        let chance = counter_espionage_chance(5, 5, 0);
        assert!((chance - 0.0).abs() < 1e-9);
    }

    #[test]
    fn test_counter_espionage_chance_clamped_at_zero() {
        // Extreme attacker advantage, low defender tech -> modifier could go
        // negative, but base * negative -> clamp to 0
        let chance = counter_espionage_chance(100, 1, 1);
        assert!(chance >= 0.0);
    }

    // -----------------------------------------------------------------------
    // report_detail_level
    // -----------------------------------------------------------------------

    #[test]
    fn test_report_detail_resources_basic() {
        // attacker=5, defender=5, probes=1 -> diff = 5*1 - 5 = 0 -> Resources
        assert_eq!(report_detail_level(5, 5, 1), ReportDetail::Resources);
    }

    #[test]
    fn test_report_detail_resources_negative() {
        // attacker=1, defender=10, probes=1 -> diff = 1 - 10 = -9 -> Resources
        assert_eq!(report_detail_level(1, 10, 1), ReportDetail::Resources);
    }

    #[test]
    fn test_report_detail_fleet() {
        // attacker=7, defender=5, probes=1 -> diff = 7 - 5 = 2 -> Fleet
        assert_eq!(report_detail_level(7, 5, 1), ReportDetail::Fleet);
    }

    #[test]
    fn test_report_detail_defense() {
        // attacker=9, defender=5, probes=1 -> diff = 9 - 5 = 4 -> Defense
        assert_eq!(report_detail_level(9, 5, 1), ReportDetail::Defense);
    }

    #[test]
    fn test_report_detail_buildings() {
        // attacker=12, defender=5, probes=1 -> diff = 12 - 5 = 7 -> Buildings
        assert_eq!(report_detail_level(12, 5, 1), ReportDetail::Buildings);
    }

    #[test]
    fn test_report_detail_research() {
        // attacker=15, defender=5, probes=1 -> diff = 15 - 5 = 10 -> Research
        assert_eq!(report_detail_level(15, 5, 1), ReportDetail::Research);
    }

    #[test]
    fn test_report_detail_probes_increase_detail() {
        // attacker=5, defender=5, probes=4 -> diff = 5*2 - 5 = 5 -> Defense (>= 4)
        assert_eq!(report_detail_level(5, 5, 4), ReportDetail::Defense);
    }

    #[test]
    fn test_report_detail_many_probes_research() {
        // attacker=5, defender=5, probes=9 -> diff = 5*3 - 5 = 10 -> Research
        assert_eq!(report_detail_level(5, 5, 9), ReportDetail::Research);
    }

    #[test]
    fn test_report_detail_edge_fleet_boundary() {
        // Need diff exactly 2: attacker=7, defender=5, probes=1 -> 7-5=2
        assert_eq!(report_detail_level(7, 5, 1), ReportDetail::Fleet);
    }

    // -----------------------------------------------------------------------
    // Coordinates / snapshot structs
    // -----------------------------------------------------------------------

    #[test]
    fn test_coordinates_serde() {
        let c = Coordinates {
            galaxy: 3,
            system: 150,
            position: 12,
        };
        let json = serde_json::to_string(&c).unwrap();
        let c2: Coordinates = serde_json::from_str(&json).unwrap();
        assert_eq!(c, c2);
    }

    #[test]
    fn test_resource_snapshot_serde() {
        let r = ResourceSnapshot {
            metal: 1_000_000,
            crystal: 500_000,
            deuterium: 250_000,
            energy: 1200,
        };
        let json = serde_json::to_string(&r).unwrap();
        let r2: ResourceSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(r, r2);
    }

    #[test]
    fn test_fleet_snapshot_serde() {
        let f = FleetSnapshot {
            ships: vec![ShipCount {
                ship_type: "Battleship".into(),
                count: 42,
            }],
        };
        let json = serde_json::to_string(&f).unwrap();
        let f2: FleetSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(f, f2);
    }

    #[test]
    fn test_defense_snapshot_serde() {
        let d = DefenseSnapshot {
            defenses: vec![DefenseCount {
                defense_type: "GaussCannon".into(),
                count: 10,
            }],
        };
        let json = serde_json::to_string(&d).unwrap();
        let d2: DefenseSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(d, d2);
    }

    #[test]
    fn test_building_snapshot_serde() {
        let b = BuildingSnapshot {
            buildings: vec![BuildingLevel {
                building_type: "SolarPlant".into(),
                level: 25,
            }],
        };
        let json = serde_json::to_string(&b).unwrap();
        let b2: BuildingSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(b, b2);
    }

    #[test]
    fn test_research_snapshot_serde() {
        let r = ResearchSnapshot {
            technologies: vec![TechLevel {
                tech_type: "Hyperspace".into(),
                level: 7,
            }],
        };
        let json = serde_json::to_string(&r).unwrap();
        let r2: ResearchSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(r, r2);
    }

    // -----------------------------------------------------------------------
    // EspionageReport
    // -----------------------------------------------------------------------

    #[test]
    fn test_espionage_report_serde() {
        let report = EspionageReport {
            id: 1,
            attacker_id: "player_a".into(),
            defender_id: "player_b".into(),
            target_coordinates: sample_coords(),
            detail_level: ReportDetail::Fleet,
            timestamp: "2026-03-08T12:00:00Z".into(),
            resources: Some(ResourceSnapshot {
                metal: 100,
                crystal: 200,
                deuterium: 300,
                energy: 50,
            }),
            fleet: Some(FleetSnapshot {
                ships: vec![ShipCount {
                    ship_type: "Cruiser".into(),
                    count: 5,
                }],
            }),
            defenses: None,
            buildings: None,
            research: None,
            probes_sent: 3,
            probes_lost: 1,
        };
        let json = serde_json::to_string(&report).unwrap();
        let r2: EspionageReport = serde_json::from_str(&json).unwrap();
        assert_eq!(report, r2);
    }

    // -----------------------------------------------------------------------
    // SpyMission & SpyMissionStatus
    // -----------------------------------------------------------------------

    #[test]
    fn test_spy_mission_status_serde() {
        let statuses = vec![
            SpyMissionStatus::InTransit,
            SpyMissionStatus::Completed,
            SpyMissionStatus::ProbesDestroyed,
            SpyMissionStatus::Aborted,
        ];
        for s in &statuses {
            let json = serde_json::to_string(s).unwrap();
            let s2: SpyMissionStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(*s, s2);
        }
    }

    #[test]
    fn test_spy_mission_serde() {
        let m = SpyMission {
            id: 42,
            attacker_id: "a".into(),
            defender_id: "d".into(),
            target: sample_coords(),
            probes_sent: 5,
            status: SpyMissionStatus::InTransit,
            launched_at: "2026-03-08T10:00:00Z".into(),
            arrival_at: "2026-03-08T10:05:00Z".into(),
            report_id: None,
        };
        let json = serde_json::to_string(&m).unwrap();
        let m2: SpyMission = serde_json::from_str(&json).unwrap();
        assert_eq!(m, m2);
    }

    // -----------------------------------------------------------------------
    // resolve_counter_espionage
    // -----------------------------------------------------------------------

    #[test]
    fn test_resolve_counter_espionage_deterministic() {
        let r1 = resolve_counter_espionage(5, 5, 10);
        let r2 = resolve_counter_espionage(5, 5, 10);
        assert_eq!(r1, r2, "Counter-espionage should be deterministic");
    }

    #[test]
    fn test_resolve_counter_espionage_totals() {
        let r = resolve_counter_espionage(5, 5, 10);
        assert_eq!(r.probes_destroyed + r.probes_escaped, 10);
    }

    #[test]
    fn test_resolve_counter_espionage_zero_probes() {
        let r = resolve_counter_espionage(5, 5, 0);
        assert_eq!(r.probes_destroyed, 0);
        assert_eq!(r.probes_escaped, 0);
        assert!(!r.detected);
        assert!(r.detection_message.is_none());
    }

    #[test]
    fn test_resolve_counter_espionage_zero_defender_tech() {
        // chance=0, no probes destroyed
        let r = resolve_counter_espionage(0, 10, 5);
        assert_eq!(r.probes_destroyed, 0);
        assert_eq!(r.probes_escaped, 5);
        assert!(!r.detected);
    }

    #[test]
    fn test_resolve_counter_espionage_high_defender_tech() {
        // Very high defender tech, high chance of detection
        let r = resolve_counter_espionage(50, 1, 20);
        // With chance clamped to 1.0, all probes should be destroyed
        assert_eq!(r.probes_destroyed, 20);
        assert_eq!(r.probes_escaped, 0);
        assert!(r.detected);
        assert!(r.detection_message.is_some());
    }

    #[test]
    fn test_resolve_counter_espionage_detection_message_format() {
        let r = resolve_counter_espionage(50, 1, 5);
        if let Some(msg) = &r.detection_message {
            assert!(msg.contains("Counter-espionage detected"));
        }
    }

    // -----------------------------------------------------------------------
    // resolve_spy_mission
    // -----------------------------------------------------------------------

    #[test]
    fn test_resolve_spy_mission_returns_report_with_resources() {
        let mission = SpyMission {
            id: 1,
            attacker_id: "attacker".into(),
            defender_id: "defender".into(),
            target: sample_coords(),
            probes_sent: 1,
            status: SpyMissionStatus::InTransit,
            launched_at: "2026-03-08T10:00:00Z".into(),
            arrival_at: "2026-03-08T10:05:00Z".into(),
            report_id: None,
        };
        let planet = sample_planet_intel();
        // attacker_tech = defender_tech = 0 -> no counter-espionage -> report
        let result = resolve_spy_mission(&mission, 0, 0, &planet);
        assert!(result.report.is_some());
        let report = result.report.unwrap();
        assert!(report.resources.is_some());
    }

    #[test]
    fn test_resolve_spy_mission_high_tech_full_report() {
        let mission = SpyMission {
            id: 100,
            attacker_id: "attacker".into(),
            defender_id: "defender".into(),
            target: sample_coords(),
            probes_sent: 10,
            status: SpyMissionStatus::InTransit,
            launched_at: "2026-03-08T10:00:00Z".into(),
            arrival_at: "2026-03-08T10:05:00Z".into(),
            report_id: None,
        };
        let planet = sample_planet_intel();
        // Very high attacker tech, 0 defender -> full detail, no probes lost
        let result = resolve_spy_mission(&mission, 20, 0, &planet);
        assert!(result.report.is_some());
        let report = result.report.unwrap();
        assert_eq!(report.detail_level, ReportDetail::Research);
        assert!(report.resources.is_some());
        assert!(report.fleet.is_some());
        assert!(report.defenses.is_some());
        assert!(report.buildings.is_some());
        assert!(report.research.is_some());
    }

    #[test]
    fn test_resolve_spy_mission_all_probes_destroyed() {
        let mission = SpyMission {
            id: 2,
            attacker_id: "attacker".into(),
            defender_id: "defender".into(),
            target: sample_coords(),
            probes_sent: 5,
            status: SpyMissionStatus::InTransit,
            launched_at: "2026-03-08T10:00:00Z".into(),
            arrival_at: "2026-03-08T10:05:00Z".into(),
            report_id: None,
        };
        let planet = sample_planet_intel();
        // Very high defender tech -> all probes destroyed -> no report
        let result = resolve_spy_mission(&mission, 1, 50, &planet);
        assert!(result.report.is_none());
        assert_eq!(result.probes_survived, 0);
        assert!(result.detected);
    }

    #[test]
    fn test_resolve_spy_mission_deterministic() {
        let mission = SpyMission {
            id: 7,
            attacker_id: "a".into(),
            defender_id: "d".into(),
            target: sample_coords(),
            probes_sent: 5,
            status: SpyMissionStatus::InTransit,
            launched_at: "2026-03-08T10:00:00Z".into(),
            arrival_at: "2026-03-08T10:05:00Z".into(),
            report_id: None,
        };
        let planet = sample_planet_intel();
        let r1 = resolve_spy_mission(&mission, 5, 5, &planet);
        let r2 = resolve_spy_mission(&mission, 5, 5, &planet);
        assert_eq!(r1, r2);
    }

    #[test]
    fn test_resolve_spy_mission_probes_count() {
        let mission = SpyMission {
            id: 3,
            attacker_id: "a".into(),
            defender_id: "d".into(),
            target: sample_coords(),
            probes_sent: 8,
            status: SpyMissionStatus::InTransit,
            launched_at: "2026-03-08T10:00:00Z".into(),
            arrival_at: "2026-03-08T10:05:00Z".into(),
            report_id: None,
        };
        let planet = sample_planet_intel();
        let result = resolve_spy_mission(&mission, 5, 5, &planet);
        assert_eq!(result.probes_lost + result.probes_survived, 8);
    }

    // -----------------------------------------------------------------------
    // espionage_flight_time
    // -----------------------------------------------------------------------

    #[test]
    fn test_flight_time_basic() {
        // distance=1000, speed=100, universe_speed=1 -> 10.0
        let t = espionage_flight_time(1000.0, 100.0, 1);
        assert!((t - 10.0).abs() < 1e-9);
    }

    #[test]
    fn test_flight_time_universe_speed() {
        // distance=1000, speed=100, universe_speed=2 -> 5.0
        let t = espionage_flight_time(1000.0, 100.0, 2);
        assert!((t - 5.0).abs() < 1e-9);
    }

    #[test]
    fn test_flight_time_zero_speed() {
        let t = espionage_flight_time(1000.0, 0.0, 1);
        assert!(t.is_infinite());
    }

    #[test]
    fn test_flight_time_zero_universe_speed() {
        let t = espionage_flight_time(1000.0, 100.0, 0);
        assert!(t.is_infinite());
    }

    #[test]
    fn test_flight_time_zero_distance() {
        let t = espionage_flight_time(0.0, 100.0, 1);
        assert!((t - 0.0).abs() < 1e-9);
    }

    // -----------------------------------------------------------------------
    // EspionageStore
    // -----------------------------------------------------------------------

    #[test]
    fn test_store_create_mission() {
        let mut store = EspionageStore::new();
        let m = store.create_mission("a", "d", sample_coords(), 5);
        assert_eq!(m.id, 1);
        assert_eq!(m.attacker_id, "a");
        assert_eq!(m.probes_sent, 5);
        assert_eq!(m.status, SpyMissionStatus::InTransit);
    }

    #[test]
    fn test_store_auto_increment_ids() {
        let mut store = EspionageStore::new();
        let m1 = store.create_mission("a", "d", sample_coords(), 1);
        let m2 = store.create_mission("a", "d", sample_coords(), 2);
        let m3 = store.create_mission("b", "d", sample_coords(), 3);
        assert_eq!(m1.id, 1);
        assert_eq!(m2.id, 2);
        assert_eq!(m3.id, 3);
    }

    #[test]
    fn test_store_get_mission() {
        let mut store = EspionageStore::new();
        let m = store.create_mission("a", "d", sample_coords(), 5);
        let fetched = store.get_mission(m.id);
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().id, m.id);
    }

    #[test]
    fn test_store_get_mission_not_found() {
        let store = EspionageStore::new();
        assert!(store.get_mission(999).is_none());
    }

    #[test]
    fn test_store_list_missions_by_attacker() {
        let mut store = EspionageStore::new();
        store.create_mission("a", "d1", sample_coords(), 1);
        store.create_mission("a", "d2", sample_coords(), 2);
        store.create_mission("b", "d1", sample_coords(), 3);
        let missions = store.list_missions_by_attacker("a");
        assert_eq!(missions.len(), 2);
    }

    #[test]
    fn test_store_list_missions_by_defender() {
        let mut store = EspionageStore::new();
        store.create_mission("a1", "d", sample_coords(), 1);
        store.create_mission("a2", "d", sample_coords(), 2);
        store.create_mission("a3", "x", sample_coords(), 3);
        let missions = store.list_missions_by_defender("d");
        assert_eq!(missions.len(), 2);
    }

    #[test]
    fn test_store_complete_mission_completed() {
        let mut store = EspionageStore::new();
        let m = store.create_mission("a", "d", sample_coords(), 5);
        let result = SpyMissionResult {
            report: Some(EspionageReport {
                id: 0,
                attacker_id: "a".into(),
                defender_id: "d".into(),
                target_coordinates: sample_coords(),
                detail_level: ReportDetail::Resources,
                timestamp: "2026-03-08T12:00:00Z".into(),
                resources: Some(ResourceSnapshot {
                    metal: 100,
                    crystal: 200,
                    deuterium: 300,
                    energy: 50,
                }),
                fleet: None,
                defenses: None,
                buildings: None,
                research: None,
                probes_sent: 5,
                probes_lost: 0,
            }),
            probes_lost: 0,
            probes_survived: 5,
            detected: false,
        };
        let completed = store.complete_mission(m.id, result);
        assert!(completed.is_some());
        let completed = completed.unwrap();
        assert_eq!(completed.status, SpyMissionStatus::Completed);
        assert!(completed.report_id.is_some());
    }

    #[test]
    fn test_store_complete_mission_probes_destroyed() {
        let mut store = EspionageStore::new();
        let m = store.create_mission("a", "d", sample_coords(), 5);
        let result = SpyMissionResult {
            report: None,
            probes_lost: 5,
            probes_survived: 0,
            detected: true,
        };
        let completed = store.complete_mission(m.id, result);
        assert!(completed.is_some());
        assert_eq!(completed.unwrap().status, SpyMissionStatus::ProbesDestroyed);
    }

    #[test]
    fn test_store_complete_mission_not_found() {
        let mut store = EspionageStore::new();
        let result = SpyMissionResult {
            report: None,
            probes_lost: 0,
            probes_survived: 0,
            detected: false,
        };
        assert!(store.complete_mission(999, result).is_none());
    }

    #[test]
    fn test_store_report() {
        let mut store = EspionageStore::new();
        let report = EspionageReport {
            id: 0,
            attacker_id: "a".into(),
            defender_id: "d".into(),
            target_coordinates: sample_coords(),
            detail_level: ReportDetail::Fleet,
            timestamp: "2026-03-08T12:00:00Z".into(),
            resources: None,
            fleet: None,
            defenses: None,
            buildings: None,
            research: None,
            probes_sent: 3,
            probes_lost: 1,
        };
        let id = store.store_report(report);
        assert_eq!(id, 1);
        let fetched = store.get_report(id).unwrap();
        assert_eq!(fetched.id, 1);
        assert_eq!(fetched.attacker_id, "a");
    }

    #[test]
    fn test_store_get_report_not_found() {
        let store = EspionageStore::new();
        assert!(store.get_report(999).is_none());
    }

    #[test]
    fn test_store_list_reports_for_player() {
        let mut store = EspionageStore::new();
        for i in 0..5 {
            let report = EspionageReport {
                id: 0,
                attacker_id: "player1".into(),
                defender_id: format!("target_{}", i),
                target_coordinates: sample_coords(),
                detail_level: ReportDetail::Resources,
                timestamp: format!("2026-03-08T12:{:02}:00Z", i),
                resources: None,
                fleet: None,
                defenses: None,
                buildings: None,
                research: None,
                probes_sent: 1,
                probes_lost: 0,
            };
            store.store_report(report);
        }
        let reports = store.list_reports_for_player("player1", 3);
        assert_eq!(reports.len(), 3);
        // Most recent first (highest id)
        assert!(reports[0].id > reports[1].id);
        assert!(reports[1].id > reports[2].id);
    }

    #[test]
    fn test_store_list_reports_for_player_as_defender() {
        let mut store = EspionageStore::new();
        let report = EspionageReport {
            id: 0,
            attacker_id: "other".into(),
            defender_id: "me".into(),
            target_coordinates: sample_coords(),
            detail_level: ReportDetail::Resources,
            timestamp: "2026-03-08T12:00:00Z".into(),
            resources: None,
            fleet: None,
            defenses: None,
            buildings: None,
            research: None,
            probes_sent: 1,
            probes_lost: 0,
        };
        store.store_report(report);
        let reports = store.list_reports_for_player("me", 10);
        assert_eq!(reports.len(), 1);
    }

    #[test]
    fn test_store_list_reports_limit() {
        let mut store = EspionageStore::new();
        for i in 0..10 {
            let report = EspionageReport {
                id: 0,
                attacker_id: "p".into(),
                defender_id: format!("t{}", i),
                target_coordinates: sample_coords(),
                detail_level: ReportDetail::Resources,
                timestamp: "2026-03-08T12:00:00Z".into(),
                resources: None,
                fleet: None,
                defenses: None,
                buildings: None,
                research: None,
                probes_sent: 1,
                probes_lost: 0,
            };
            store.store_report(report);
        }
        let reports = store.list_reports_for_player("p", 100);
        assert_eq!(reports.len(), 10);
    }

    #[test]
    fn test_store_delete_report() {
        let mut store = EspionageStore::new();
        let report = EspionageReport {
            id: 0,
            attacker_id: "a".into(),
            defender_id: "d".into(),
            target_coordinates: sample_coords(),
            detail_level: ReportDetail::Resources,
            timestamp: "2026-03-08T12:00:00Z".into(),
            resources: None,
            fleet: None,
            defenses: None,
            buildings: None,
            research: None,
            probes_sent: 1,
            probes_lost: 0,
        };
        let id = store.store_report(report);
        assert!(store.delete_report(id));
        assert!(store.get_report(id).is_none());
    }

    #[test]
    fn test_store_delete_report_not_found() {
        let mut store = EspionageStore::new();
        assert!(!store.delete_report(999));
    }

    #[test]
    fn test_store_default() {
        let store = EspionageStore::default();
        assert!(store.get_mission(1).is_none());
    }

    // -----------------------------------------------------------------------
    // ReportDetail ordering
    // -----------------------------------------------------------------------

    #[test]
    fn test_report_detail_ordering() {
        assert!(ReportDetail::Resources < ReportDetail::Fleet);
        assert!(ReportDetail::Fleet < ReportDetail::Defense);
        assert!(ReportDetail::Defense < ReportDetail::Buildings);
        assert!(ReportDetail::Buildings < ReportDetail::Research);
    }

    // -----------------------------------------------------------------------
    // PlanetIntel
    // -----------------------------------------------------------------------

    #[test]
    fn test_planet_intel_serde() {
        let pi = sample_planet_intel();
        let json = serde_json::to_string(&pi).unwrap();
        let pi2: PlanetIntel = serde_json::from_str(&json).unwrap();
        assert_eq!(pi, pi2);
    }

    // -----------------------------------------------------------------------
    // SpyMissionResult
    // -----------------------------------------------------------------------

    #[test]
    fn test_spy_mission_result_serde() {
        let r = SpyMissionResult {
            report: None,
            probes_lost: 3,
            probes_survived: 2,
            detected: true,
        };
        let json = serde_json::to_string(&r).unwrap();
        let r2: SpyMissionResult = serde_json::from_str(&json).unwrap();
        assert_eq!(r, r2);
    }

    // -----------------------------------------------------------------------
    // CounterEspionageResult
    // -----------------------------------------------------------------------

    #[test]
    fn test_counter_espionage_result_serde() {
        let r = CounterEspionageResult {
            probes_destroyed: 2,
            probes_escaped: 3,
            detected: true,
            detection_message: Some("Detected!".into()),
        };
        let json = serde_json::to_string(&r).unwrap();
        let r2: CounterEspionageResult = serde_json::from_str(&json).unwrap();
        assert_eq!(r, r2);
    }

    // -----------------------------------------------------------------------
    // Integration: full mission lifecycle through store
    // -----------------------------------------------------------------------

    #[test]
    fn test_full_mission_lifecycle() {
        let mut store = EspionageStore::new();
        let planet = sample_planet_intel();

        // Create mission
        let mission = store.create_mission("alice", "bob", sample_coords(), 5);
        assert_eq!(mission.status, SpyMissionStatus::InTransit);

        // Resolve mission (attacker_tech=10, defender_tech=0 -> guaranteed report)
        let result = resolve_spy_mission(&mission, 10, 0, &planet);
        assert!(result.report.is_some());
        assert_eq!(result.probes_lost, 0);
        assert_eq!(result.probes_survived, 5);

        // Complete mission
        let completed = store.complete_mission(mission.id, result).unwrap();
        assert_eq!(completed.status, SpyMissionStatus::Completed);
        assert!(completed.report_id.is_some());
        let report_id = completed.report_id.unwrap();

        // Retrieve report from store
        let report = store.get_report(report_id).unwrap();
        assert_eq!(report.attacker_id, "alice");
        assert_eq!(report.defender_id, "bob");
        assert!(report.resources.is_some());
        let report_id = report.id;

        // List reports
        let alice_reports = store.list_reports_for_player("alice", 10);
        assert_eq!(alice_reports.len(), 1);

        // Delete report
        assert!(store.delete_report(report_id));
        assert!(store.get_report(report_id).is_none());
    }

    #[test]
    fn test_full_mission_lifecycle_probes_destroyed() {
        let mut store = EspionageStore::new();
        let planet = sample_planet_intel();

        let mission = store.create_mission("alice", "bob", sample_coords(), 3);
        // Very high defender tech -> all probes destroyed
        let result = resolve_spy_mission(&mission, 1, 50, &planet);
        assert!(result.report.is_none());
        assert_eq!(result.probes_survived, 0);

        let completed = store.complete_mission(mission.id, result).unwrap();
        assert_eq!(completed.status, SpyMissionStatus::ProbesDestroyed);
        assert!(completed.report_id.is_none());
    }
}
