#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// Represents quantities of in-game resources.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Resources {
    pub metal: f64,
    pub crystal: f64,
    pub deuterium: f64,
    pub energy: f64,
}

impl Resources {
    pub fn new(metal: f64, crystal: f64, deuterium: f64, energy: f64) -> Self {
        Self {
            metal,
            crystal,
            deuterium,
            energy,
        }
    }

    pub fn zero() -> Self {
        Self::new(0.0, 0.0, 0.0, 0.0)
    }
}

/// Building / research / unit cost expressed in resources.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuildingCost {
    pub metal: f64,
    pub crystal: f64,
    pub deuterium: f64,
    pub energy: f64,
}

impl BuildingCost {
    pub fn new(metal: f64, crystal: f64, deuterium: f64, energy: f64) -> Self {
        Self {
            metal,
            crystal,
            deuterium,
            energy,
        }
    }

    pub fn zero() -> Self {
        Self::new(0.0, 0.0, 0.0, 0.0)
    }
}

/// Snapshot used for lazy (tick-less) resource accumulation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LazyResourceState {
    /// Unix timestamp (seconds) of the last resource snapshot.
    pub last_update: i64,
    /// Current stockpile at `last_update`.
    pub metal: f64,
    pub crystal: f64,
    pub deuterium: f64,
    /// Per-hour production rates at `last_update`.
    pub metal_per_hour: f64,
    pub crystal_per_hour: f64,
    pub deuterium_per_hour: f64,
    /// Storage caps (0 or negative means unlimited).
    pub metal_storage_cap: f64,
    pub crystal_storage_cap: f64,
    pub deuterium_storage_cap: f64,
}

/// Configurable trade ratios (default 3:2:1 metal:crystal:deuterium).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TradeRatios {
    pub metal: f64,
    pub crystal: f64,
    pub deuterium: f64,
}

impl Default for TradeRatios {
    fn default() -> Self {
        Self {
            metal: 3.0,
            crystal: 2.0,
            deuterium: 1.0,
        }
    }
}

impl TradeRatios {
    pub fn new(metal: f64, crystal: f64, deuterium: f64) -> Self {
        Self {
            metal,
            crystal,
            deuterium,
        }
    }

    /// How many units of metal one unit of deuterium is worth.
    pub fn metal_per_deuterium(&self) -> f64 {
        self.metal / self.deuterium
    }

    /// How many units of crystal one unit of deuterium is worth.
    pub fn crystal_per_deuterium(&self) -> f64 {
        self.crystal / self.deuterium
    }

    /// Converts a `Resources` value to a single "deuterium-equivalent" score.
    pub fn to_deuterium_equivalent(&self, res: &Resources) -> f64 {
        res.metal / self.metal_per_deuterium()
            + res.crystal / self.crystal_per_deuterium()
            + res.deuterium
    }
}

// ---------------------------------------------------------------------------
// Resource production formulas (per hour)
// ---------------------------------------------------------------------------

/// Metal mine hourly production.
///
/// Formula: `30 * level * 1.1^level * universe_speed`
pub fn metal_production(mine_level: i32, universe_speed: i32) -> f64 {
    if mine_level <= 0 {
        return 0.0;
    }
    30.0 * mine_level as f64 * 1.1_f64.powi(mine_level) * universe_speed as f64
}

/// Crystal mine hourly production.
///
/// Formula: `20 * level * 1.1^level * universe_speed`
pub fn crystal_production(mine_level: i32, universe_speed: i32) -> f64 {
    if mine_level <= 0 {
        return 0.0;
    }
    20.0 * mine_level as f64 * 1.1_f64.powi(mine_level) * universe_speed as f64
}

/// Deuterium synthesizer hourly production.
///
/// Formula: `10 * level * 1.1^level * (1.28 - 0.002 * max_temp) * universe_speed`
pub fn deuterium_production(synthesizer_level: i32, max_temp: i32, universe_speed: i32) -> f64 {
    if synthesizer_level <= 0 {
        return 0.0;
    }
    10.0 * synthesizer_level as f64
        * 1.1_f64.powi(synthesizer_level)
        * (1.28 - 0.002 * max_temp as f64)
        * universe_speed as f64
}

// ---------------------------------------------------------------------------
// Energy formulas
// ---------------------------------------------------------------------------

/// Solar plant energy output.
///
/// Formula: `20 * level * 1.1^level`
pub fn solar_plant_energy(level: i32) -> f64 {
    if level <= 0 {
        return 0.0;
    }
    20.0 * level as f64 * 1.1_f64.powi(level)
}

/// Fusion reactor energy output.
///
/// Formula: `30 * level * (1.05 + 0.01 * energy_tech)^level`
pub fn fusion_reactor_energy(level: i32, energy_tech: i32) -> f64 {
    if level <= 0 {
        return 0.0;
    }
    30.0 * level as f64 * (1.05 + 0.01 * energy_tech as f64).powi(level)
}

/// Solar satellite energy output.
///
/// Formula: `floor((max_temp + 160) / 6) * count`
pub fn solar_satellite_energy(count: i32, max_temp: i32) -> f64 {
    if count <= 0 {
        return 0.0;
    }
    let per_sat = ((max_temp as f64 + 160.0) / 6.0).floor();
    per_sat * count as f64
}

/// Metal mine energy consumption.
///
/// Formula: `10 * level * 1.1^level`
pub fn metal_mine_energy_consumption(level: i32) -> f64 {
    if level <= 0 {
        return 0.0;
    }
    10.0 * level as f64 * 1.1_f64.powi(level)
}

/// Crystal mine energy consumption.
///
/// Formula: `10 * level * 1.1^level`
pub fn crystal_mine_energy_consumption(level: i32) -> f64 {
    if level <= 0 {
        return 0.0;
    }
    10.0 * level as f64 * 1.1_f64.powi(level)
}

/// Deuterium synthesizer energy consumption.
///
/// Formula: `20 * level * 1.1^level`
pub fn deuterium_synthesizer_energy_consumption(level: i32) -> f64 {
    if level <= 0 {
        return 0.0;
    }
    20.0 * level as f64 * 1.1_f64.powi(level)
}

// ---------------------------------------------------------------------------
// Building costs
// ---------------------------------------------------------------------------

/// Returns the cost to build `building_type` at the given `level`.
///
/// Cost = base * factor^(level - 1)  (per-resource).
///
/// Recognised building types (case-insensitive):
/// `MetalMine`, `CrystalMine`, `DeuteriumSynthesizer`, `SolarPlant`,
/// `FusionReactor`, `RoboticsFactory`, `NaniteFactory`, `Shipyard`,
/// `MetalStorage`, `CrystalStorage`, `DeuteriumTank`,
/// `ResearchLab`, `AllianceDepot`, `MissileSilo`, `Terraformer`,
/// `SpaceDock`, `LunarBase`, `SensorPhalanx`, `JumpGate`.
pub fn building_cost(building_type: &str, level: i32) -> BuildingCost {
    if level <= 0 {
        return BuildingCost::zero();
    }

    let (base_m, base_c, base_d, base_e, factor): (f64, f64, f64, f64, f64) =
        match building_type.to_ascii_lowercase().replace(' ', "").as_str() {
            "metalmine" => (60.0, 15.0, 0.0, 0.0, 1.5),
            "crystalmine" => (48.0, 24.0, 0.0, 0.0, 1.6),
            "deuteriumsynthesizer" => (225.0, 75.0, 0.0, 0.0, 1.5),
            "solarplant" => (75.0, 30.0, 0.0, 0.0, 1.5),
            "fusionreactor" => (900.0, 360.0, 180.0, 0.0, 1.8),
            "roboticsfactory" => (400.0, 120.0, 200.0, 0.0, 2.0),
            "nanitefactory" => (1_000_000.0, 500_000.0, 100_000.0, 0.0, 2.0),
            "shipyard" => (400.0, 200.0, 100.0, 0.0, 2.0),
            "metalstorage" => (1000.0, 0.0, 0.0, 0.0, 2.0),
            "crystalstorage" => (1000.0, 500.0, 0.0, 0.0, 2.0),
            "deuteriumtank" => (1000.0, 1000.0, 0.0, 0.0, 2.0),
            "researchlab" => (200.0, 400.0, 200.0, 0.0, 2.0),
            "alliancedepot" => (20_000.0, 40_000.0, 0.0, 0.0, 2.0),
            "missilesilo" => (20_000.0, 20_000.0, 1000.0, 0.0, 2.0),
            "terraformer" => (0.0, 50_000.0, 100_000.0, 1000.0, 2.0),
            "spacedock" => (200.0, 0.0, 50.0, 50.0, 5.0),
            "lunarbase" => (20_000.0, 40_000.0, 20_000.0, 0.0, 2.0),
            "sensorphalanx" => (20_000.0, 40_000.0, 20_000.0, 0.0, 2.0),
            "jumpgate" => (2_000_000.0, 4_000_000.0, 2_000_000.0, 0.0, 2.0),
            _ => return BuildingCost::zero(),
        };

    let multiplier = factor.powi(level - 1);
    BuildingCost::new(
        (base_m * multiplier).floor(),
        (base_c * multiplier).floor(),
        (base_d * multiplier).floor(),
        (base_e * multiplier).floor(),
    )
}

// ---------------------------------------------------------------------------
// Research costs
// ---------------------------------------------------------------------------

/// Returns the cost of researching `research_type` at the given `level`.
///
/// Cost = base * 2^(level - 1)  unless stated otherwise.
pub fn research_cost(research_type: &str, level: i32) -> BuildingCost {
    if level <= 0 {
        return BuildingCost::zero();
    }

    let (base_m, base_c, base_d, factor): (f64, f64, f64, f64) =
        match research_type.to_ascii_lowercase().replace(' ', "").as_str() {
            "espionagetechnology" | "espionagetech" => (200.0, 1000.0, 200.0, 2.0),
            "computertechnology" | "computertech" => (0.0, 400.0, 600.0, 2.0),
            "weaponstechnology" | "weaponstech" => (800.0, 200.0, 0.0, 2.0),
            "shieldingtechnology" | "shieldingtech" => (200.0, 600.0, 0.0, 2.0),
            "armourtechnology" | "armourtech" => (1000.0, 0.0, 0.0, 2.0),
            "energytechnology" | "energytech" => (0.0, 800.0, 400.0, 2.0),
            "hyperspacetechnology" | "hyperspacetech" => (0.0, 4000.0, 2000.0, 2.0),
            "combustiondrive" => (400.0, 0.0, 600.0, 2.0),
            "impulsedrive" => (2000.0, 4000.0, 600.0, 2.0),
            "hyperspacedrive" => (10_000.0, 20_000.0, 6000.0, 2.0),
            "lasertechnology" | "lasertech" => (200.0, 100.0, 0.0, 2.0),
            "iontechnology" | "iontech" => (1000.0, 300.0, 100.0, 2.0),
            "plasmatechnology" | "plasmatech" => (2000.0, 4000.0, 1000.0, 2.0),
            "intergalacticresearchnetwork" | "igrn" => (240_000.0, 400_000.0, 160_000.0, 2.0),
            "astrophysics" => (4000.0, 8000.0, 4000.0, 1.75),
            "gravitontechnology" | "gravitontech" => (0.0, 0.0, 0.0, 2.0), // energy-only (300k)
            _ => return BuildingCost::zero(),
        };

    let multiplier = factor.powi(level - 1);
    BuildingCost::new(
        (base_m * multiplier).floor(),
        (base_c * multiplier).floor(),
        (base_d * multiplier).floor(),
        0.0,
    )
}

// ---------------------------------------------------------------------------
// Ship costs (flat per unit)
// ---------------------------------------------------------------------------

/// Returns the cost to build **one** ship of the given type.
pub fn ship_cost(ship_type: &str) -> BuildingCost {
    match ship_type.to_ascii_lowercase().replace(' ', "").as_str() {
        "smallcargo" | "smallcargovessel" => BuildingCost::new(2000.0, 2000.0, 0.0, 0.0),
        "largecargo" | "largecargovessel" => BuildingCost::new(6000.0, 6000.0, 0.0, 0.0),
        "lightfighter" => BuildingCost::new(3000.0, 1000.0, 0.0, 0.0),
        "heavyfighter" => BuildingCost::new(6000.0, 4000.0, 0.0, 0.0),
        "cruiser" => BuildingCost::new(20_000.0, 7000.0, 2000.0, 0.0),
        "battleship" => BuildingCost::new(45_000.0, 15_000.0, 0.0, 0.0),
        "battlecruiser" => BuildingCost::new(30_000.0, 40_000.0, 15_000.0, 0.0),
        "bomber" => BuildingCost::new(50_000.0, 25_000.0, 15_000.0, 0.0),
        "destroyer" => BuildingCost::new(60_000.0, 50_000.0, 15_000.0, 0.0),
        "deathstar" => BuildingCost::new(5_000_000.0, 4_000_000.0, 1_000_000.0, 0.0),
        "recycler" => BuildingCost::new(10_000.0, 6000.0, 2000.0, 0.0),
        "espionageprobe" => BuildingCost::new(0.0, 1000.0, 0.0, 0.0),
        "solarsatellite" => BuildingCost::new(0.0, 2000.0, 500.0, 0.0),
        "colonyship" => BuildingCost::new(10_000.0, 20_000.0, 10_000.0, 0.0),
        _ => BuildingCost::zero(),
    }
}

// ---------------------------------------------------------------------------
// Defense costs (flat per unit)
// ---------------------------------------------------------------------------

/// Returns the cost to build **one** defense unit of the given type.
pub fn defense_cost(defense_type: &str) -> BuildingCost {
    match defense_type.to_ascii_lowercase().replace(' ', "").as_str() {
        "rocketlauncher" => BuildingCost::new(2000.0, 0.0, 0.0, 0.0),
        "lightlaser" => BuildingCost::new(1500.0, 500.0, 0.0, 0.0),
        "heavylaser" => BuildingCost::new(6000.0, 2000.0, 0.0, 0.0),
        "gausscannon" => BuildingCost::new(20_000.0, 15_000.0, 2000.0, 0.0),
        "ioncannon" => BuildingCost::new(5000.0, 3000.0, 0.0, 0.0),
        "plasmaturret" => BuildingCost::new(50_000.0, 50_000.0, 30_000.0, 0.0),
        "smallshielddome" => BuildingCost::new(10_000.0, 10_000.0, 0.0, 0.0),
        "largeshielddome" => BuildingCost::new(50_000.0, 50_000.0, 0.0, 0.0),
        "antiballisticmissile" | "abm" => BuildingCost::new(8000.0, 0.0, 2000.0, 0.0),
        "interplanetarymissile" | "ipm" => BuildingCost::new(12_500.0, 2500.0, 10_000.0, 0.0),
        _ => BuildingCost::zero(),
    }
}

// ---------------------------------------------------------------------------
// Construction / research / shipyard time
// ---------------------------------------------------------------------------

/// Building construction time in **seconds**.
///
/// Formula: `(metal + crystal) / (2500 * (1 + robotics_level) * 2^nanite_level * universe_speed) * 3600`
pub fn building_construction_time(
    metal_cost: f64,
    crystal_cost: f64,
    robotics_level: i32,
    nanite_level: i32,
    universe_speed: i32,
) -> f64 {
    let speed = universe_speed.max(1) as f64;
    let hours = (metal_cost + crystal_cost)
        / (2500.0 * (1 + robotics_level) as f64 * 2.0_f64.powi(nanite_level) * speed);
    (hours * 3600.0).max(1.0) // at least 1 second
}

/// Research time in **seconds**.
///
/// Formula: `(metal + crystal) / (1000 * (1 + lab_level) * universe_speed) * 3600`
pub fn research_time(
    metal_cost: f64,
    crystal_cost: f64,
    lab_level: i32,
    universe_speed: i32,
) -> f64 {
    let speed = universe_speed.max(1) as f64;
    let hours = (metal_cost + crystal_cost) / (1000.0 * (1 + lab_level) as f64 * speed);
    (hours * 3600.0).max(1.0)
}

/// Shipyard / defense construction time in **seconds**.
///
/// Formula: `(metal + crystal) / (2500 * (1 + shipyard_level) * 2^nanite_level * universe_speed) * 3600`
pub fn shipyard_construction_time(
    metal_cost: f64,
    crystal_cost: f64,
    shipyard_level: i32,
    nanite_level: i32,
    universe_speed: i32,
) -> f64 {
    let speed = universe_speed.max(1) as f64;
    let hours = (metal_cost + crystal_cost)
        / (2500.0 * (1 + shipyard_level) as f64 * 2.0_f64.powi(nanite_level) * speed);
    (hours * 3600.0).max(1.0)
}

// ---------------------------------------------------------------------------
// Storage capacity
// ---------------------------------------------------------------------------

/// Storage capacity for metal/crystal/deuterium storages.
///
/// Formula: `5000 * floor(2.5 * e^(20 * level / 33))`
pub fn storage_capacity(storage_level: i32) -> i64 {
    if storage_level < 0 {
        return 0;
    }
    let inner = 2.5 * (20.0 * storage_level as f64 / 33.0).exp();
    5000 * inner.floor() as i64
}

// ---------------------------------------------------------------------------
// Lazy resource accumulation
// ---------------------------------------------------------------------------

/// Calculates accumulated resources between `state.last_update` and `now`
/// (both unix timestamps in seconds).  Production rates are per-hour.
///
/// Each resource is capped at its storage capacity (if > 0).
pub fn calculate_accumulated_resources(state: &LazyResourceState, now: i64) -> Resources {
    let dt_seconds = (now - state.last_update).max(0) as f64;
    let dt_hours = dt_seconds / 3600.0;

    let mut metal = state.metal + state.metal_per_hour * dt_hours;
    let mut crystal = state.crystal + state.crystal_per_hour * dt_hours;
    let mut deuterium = state.deuterium + state.deuterium_per_hour * dt_hours;

    // Clamp to storage caps when they are positive.
    if state.metal_storage_cap > 0.0 {
        metal = metal.min(state.metal_storage_cap);
    }
    if state.crystal_storage_cap > 0.0 {
        crystal = crystal.min(state.crystal_storage_cap);
    }
    if state.deuterium_storage_cap > 0.0 {
        deuterium = deuterium.min(state.deuterium_storage_cap);
    }

    Resources {
        metal,
        crystal,
        deuterium,
        energy: 0.0,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: compare floats within a tolerance.
    fn approx(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() < tol
    }

    // -----------------------------------------------------------------------
    // Production tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_metal_production_level_1_speed_1() {
        // 30 * 1 * 1.1^1 * 1 = 33.0
        let prod = metal_production(1, 1);
        assert!(approx(prod, 33.0, 0.01), "got {prod}");
    }

    #[test]
    fn test_metal_production_level_5_speed_1() {
        // 30 * 5 * 1.1^5 = 150 * 1.61051 = 241.577
        let prod = metal_production(5, 1);
        assert!(approx(prod, 241.577, 0.1), "got {prod}");
    }

    #[test]
    fn test_metal_production_level_10_speed_4() {
        // 30 * 10 * 1.1^10 * 4 = 300 * 2.59374 * 4 = 3112.49
        let prod = metal_production(10, 4);
        assert!(approx(prod, 3112.49, 1.0), "got {prod}");
    }

    #[test]
    fn test_crystal_production_level_1_speed_1() {
        // 20 * 1 * 1.1^1 * 1 = 22.0
        let prod = crystal_production(1, 1);
        assert!(approx(prod, 22.0, 0.01), "got {prod}");
    }

    #[test]
    fn test_crystal_production_level_7_speed_1() {
        // 20 * 7 * 1.1^7 = 140 * 1.9487 = 272.82
        let prod = crystal_production(7, 1);
        assert!(approx(prod, 272.82, 0.1), "got {prod}");
    }

    #[test]
    fn test_deuterium_production_level_5_temp50_speed1() {
        // 10 * 5 * 1.1^5 * (1.28 - 0.002*50) * 1
        // = 50 * 1.61051 * 1.18 = 94.82
        let prod = deuterium_production(5, 50, 1);
        assert!(approx(prod, 95.02, 0.2), "got {prod}");
    }

    #[test]
    fn test_deuterium_production_level_0() {
        assert_eq!(deuterium_production(0, 50, 1), 0.0);
    }

    // -----------------------------------------------------------------------
    // Energy tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_solar_plant_energy_level_1() {
        // 20 * 1 * 1.1^1 = 22.0
        let e = solar_plant_energy(1);
        assert!(approx(e, 22.0, 0.01), "got {e}");
    }

    #[test]
    fn test_solar_plant_energy_level_10() {
        // 20 * 10 * 1.1^10 = 200 * 2.59374 = 518.75
        let e = solar_plant_energy(10);
        assert!(approx(e, 518.75, 0.1), "got {e}");
    }

    #[test]
    fn test_fusion_reactor_energy_level_5_tech3() {
        // 30 * 5 * (1.05 + 0.03)^5 = 150 * 1.08^5 = 150 * 1.46933 = 220.40
        let e = fusion_reactor_energy(5, 3);
        assert!(approx(e, 220.40, 0.1), "got {e}");
    }

    #[test]
    fn test_solar_satellite_energy_temp50() {
        // floor((50+160)/6) = floor(35.0) = 35  -> 35 * 10 = 350
        let e = solar_satellite_energy(10, 50);
        assert!(approx(e, 350.0, 0.01), "got {e}");
    }

    #[test]
    fn test_energy_consumption_metal_mine_level_5() {
        // 10 * 5 * 1.1^5 = 50 * 1.61051 = 80.53
        let c = metal_mine_energy_consumption(5);
        assert!(approx(c, 80.53, 0.1), "got {c}");
    }

    #[test]
    fn test_energy_consumption_deuterium_level_3() {
        // 20 * 3 * 1.1^3 = 60 * 1.331 = 79.86
        let c = deuterium_synthesizer_energy_consumption(3);
        assert!(approx(c, 79.86, 0.1), "got {c}");
    }

    // -----------------------------------------------------------------------
    // Building cost tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_metal_mine_cost_level_1() {
        let c = building_cost("MetalMine", 1);
        // base * 1.5^0 = base
        assert!(approx(c.metal, 60.0, 0.01), "metal: {}", c.metal);
        assert!(approx(c.crystal, 15.0, 0.01), "crystal: {}", c.crystal);
    }

    #[test]
    fn test_metal_mine_cost_level_5() {
        // 60 * 1.5^4 = 60 * 5.0625 = 303.75 -> floor = 303
        // 15 * 1.5^4 = 15 * 5.0625 = 75.9375 -> floor = 75
        let c = building_cost("MetalMine", 5);
        assert!(approx(c.metal, 303.0, 1.0), "metal: {}", c.metal);
        assert!(approx(c.crystal, 75.0, 1.0), "crystal: {}", c.crystal);
    }

    #[test]
    fn test_crystal_mine_cost_level_3() {
        // 48 * 1.6^2 = 48 * 2.56 = 122.88 -> 122
        // 24 * 1.6^2 = 24 * 2.56 = 61.44 -> 61
        let c = building_cost("CrystalMine", 3);
        assert!(approx(c.metal, 122.0, 1.0), "metal: {}", c.metal);
        assert!(approx(c.crystal, 61.0, 1.0), "crystal: {}", c.crystal);
    }

    #[test]
    fn test_building_cost_unknown_returns_zero() {
        let c = building_cost("NonexistentBuilding", 5);
        assert_eq!(c.metal, 0.0);
        assert_eq!(c.crystal, 0.0);
        assert_eq!(c.deuterium, 0.0);
    }

    // -----------------------------------------------------------------------
    // Research cost tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_energy_tech_cost_level_1() {
        let c = research_cost("EnergyTechnology", 1);
        assert!(approx(c.metal, 0.0, 0.01));
        assert!(approx(c.crystal, 800.0, 0.01), "crystal: {}", c.crystal);
        assert!(approx(c.deuterium, 400.0, 0.01), "deut: {}", c.deuterium);
    }

    #[test]
    fn test_weapons_tech_cost_level_4() {
        // 800 * 2^3 = 6400 metal,  200 * 2^3 = 1600 crystal
        let c = research_cost("WeaponsTechnology", 4);
        assert!(approx(c.metal, 6400.0, 1.0), "metal: {}", c.metal);
        assert!(approx(c.crystal, 1600.0, 1.0), "crystal: {}", c.crystal);
    }

    #[test]
    fn test_astrophysics_cost_level_3() {
        // 4000 * 1.75^2 = 4000 * 3.0625 = 12250
        // 8000 * 1.75^2 = 8000 * 3.0625 = 24500
        // 4000 * 1.75^2 = 12250
        let c = research_cost("Astrophysics", 3);
        assert!(approx(c.metal, 12250.0, 1.0), "metal: {}", c.metal);
        assert!(approx(c.crystal, 24500.0, 1.0), "crystal: {}", c.crystal);
        assert!(approx(c.deuterium, 12250.0, 1.0), "deut: {}", c.deuterium);
    }

    // -----------------------------------------------------------------------
    // Ship & defense cost tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_ship_cost_cruiser() {
        let c = ship_cost("Cruiser");
        assert_eq!(c.metal, 20_000.0);
        assert_eq!(c.crystal, 7000.0);
        assert_eq!(c.deuterium, 2000.0);
    }

    #[test]
    fn test_defense_cost_gauss_cannon() {
        let c = defense_cost("GaussCannon");
        assert_eq!(c.metal, 20_000.0);
        assert_eq!(c.crystal, 15_000.0);
        assert_eq!(c.deuterium, 2000.0);
    }

    // -----------------------------------------------------------------------
    // Construction time tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_building_construction_time_basic() {
        // metal_mine level 1: metal=60, crystal=15, robotics=0, nanite=0, speed=1
        // hours = 75 / (2500 * 1 * 1 * 1) = 0.03
        // seconds = 0.03 * 3600 = 108
        let t = building_construction_time(60.0, 15.0, 0, 0, 1);
        assert!(approx(t, 108.0, 0.1), "got {t}");
    }

    #[test]
    fn test_building_construction_time_with_robotics_and_nanite() {
        // robotics=10, nanite=2, speed=1
        // hours = 75 / (2500 * 11 * 4 * 1) = 75 / 110000 = 0.000681818
        // seconds = 2.4545
        let t = building_construction_time(60.0, 15.0, 10, 2, 1);
        assert!(approx(t, 2.4545, 0.01), "got {t}");
    }

    #[test]
    fn test_research_time_basic() {
        // energy tech level 1: metal=0, crystal=800, lab=1, speed=1
        // hours = 800 / (1000 * 2 * 1) = 0.4
        // seconds = 1440
        let t = research_time(0.0, 800.0, 1, 1);
        assert!(approx(t, 1440.0, 0.1), "got {t}");
    }

    #[test]
    fn test_shipyard_construction_time_light_fighter() {
        // light fighter: 3000 + 1000 = 4000, shipyard=1, nanite=0, speed=1
        // hours = 4000 / (2500 * 2 * 1 * 1) = 0.8
        // seconds = 2880
        let t = shipyard_construction_time(3000.0, 1000.0, 1, 0, 1);
        assert!(approx(t, 2880.0, 0.1), "got {t}");
    }

    // -----------------------------------------------------------------------
    // Storage capacity tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_storage_capacity_level_0() {
        // 5000 * floor(2.5 * e^0) = 5000 * floor(2.5) = 5000 * 2 = 10000
        assert_eq!(storage_capacity(0), 10_000);
    }

    #[test]
    fn test_storage_capacity_level_1() {
        // 5000 * floor(2.5 * e^(20/33))
        // e^0.6060 ≈ 1.8332
        // 2.5 * 1.8332 = 4.583 -> floor = 4
        // 5000 * 4 = 20000
        let cap = storage_capacity(1);
        assert_eq!(cap, 20_000, "got {cap}");
    }

    #[test]
    fn test_storage_capacity_level_5() {
        // 5000 * floor(2.5 * e^(100/33))
        // e^3.0303 ≈ 20.7016
        // 2.5 * 20.7016 = 51.754 -> floor = 51
        // 5000 * 51 = 255000
        let cap = storage_capacity(5);
        assert_eq!(cap, 255_000, "got {cap}");
    }

    #[test]
    fn test_storage_capacity_level_10() {
        // 5000 * floor(2.5 * e^(200/33))
        // e^6.0606 ≈ 428.76
        // 2.5 * 428.76 = 1071.9 -> floor = 1071
        // 5000 * 1071 = 5_355_000
        let cap = storage_capacity(10);
        assert_eq!(cap, 5_355_000, "got {cap}");
    }

    // -----------------------------------------------------------------------
    // Lazy resource accumulation tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_lazy_accumulation_one_hour() {
        let state = LazyResourceState {
            last_update: 0,
            metal: 1000.0,
            crystal: 500.0,
            deuterium: 200.0,
            metal_per_hour: 300.0,
            crystal_per_hour: 200.0,
            deuterium_per_hour: 100.0,
            metal_storage_cap: 0.0,   // unlimited
            crystal_storage_cap: 0.0, // unlimited
            deuterium_storage_cap: 0.0,
        };

        let res = calculate_accumulated_resources(&state, 3600); // 1 hour later
        assert!(approx(res.metal, 1300.0, 0.01), "metal: {}", res.metal);
        assert!(approx(res.crystal, 700.0, 0.01), "crystal: {}", res.crystal);
        assert!(
            approx(res.deuterium, 300.0, 0.01),
            "deut: {}",
            res.deuterium
        );
    }

    #[test]
    fn test_lazy_accumulation_capped() {
        let state = LazyResourceState {
            last_update: 0,
            metal: 9000.0,
            crystal: 0.0,
            deuterium: 0.0,
            metal_per_hour: 3000.0,
            crystal_per_hour: 0.0,
            deuterium_per_hour: 0.0,
            metal_storage_cap: 10_000.0,
            crystal_storage_cap: 0.0,
            deuterium_storage_cap: 0.0,
        };

        // After 1 hour, raw metal = 9000 + 3000 = 12000, capped to 10000
        let res = calculate_accumulated_resources(&state, 3600);
        assert!(approx(res.metal, 10_000.0, 0.01), "metal: {}", res.metal);
    }

    #[test]
    fn test_lazy_accumulation_zero_elapsed() {
        let state = LazyResourceState {
            last_update: 1000,
            metal: 500.0,
            crystal: 300.0,
            deuterium: 100.0,
            metal_per_hour: 100.0,
            crystal_per_hour: 50.0,
            deuterium_per_hour: 25.0,
            metal_storage_cap: 0.0,
            crystal_storage_cap: 0.0,
            deuterium_storage_cap: 0.0,
        };

        let res = calculate_accumulated_resources(&state, 1000); // same instant
        assert!(approx(res.metal, 500.0, 0.01));
        assert!(approx(res.crystal, 300.0, 0.01));
        assert!(approx(res.deuterium, 100.0, 0.01));
    }

    #[test]
    fn test_lazy_accumulation_past_timestamp() {
        // If `now` is before last_update, treat elapsed as 0 (no negative accrual).
        let state = LazyResourceState {
            last_update: 5000,
            metal: 100.0,
            crystal: 100.0,
            deuterium: 100.0,
            metal_per_hour: 1000.0,
            crystal_per_hour: 1000.0,
            deuterium_per_hour: 1000.0,
            metal_storage_cap: 0.0,
            crystal_storage_cap: 0.0,
            deuterium_storage_cap: 0.0,
        };

        let res = calculate_accumulated_resources(&state, 1000);
        assert!(approx(res.metal, 100.0, 0.01));
    }

    // -----------------------------------------------------------------------
    // Trade ratio tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_trade_ratios_default() {
        let ratios = TradeRatios::default();
        assert_eq!(ratios.metal, 3.0);
        assert_eq!(ratios.crystal, 2.0);
        assert_eq!(ratios.deuterium, 1.0);
    }

    #[test]
    fn test_trade_ratio_deuterium_equivalent() {
        let ratios = TradeRatios::default();
        // 300 metal + 200 crystal + 100 deut
        // metal->deut: 300 / 3 = 100, crystal->deut: 200 / 2 = 100, deut: 100
        // total = 300
        let res = Resources::new(300.0, 200.0, 100.0, 0.0);
        let equiv = ratios.to_deuterium_equivalent(&res);
        assert!(approx(equiv, 300.0, 0.01), "got {equiv}");
    }

    // -----------------------------------------------------------------------
    // Zero / negative level guards
    // -----------------------------------------------------------------------

    #[test]
    fn test_production_zero_level() {
        assert_eq!(metal_production(0, 1), 0.0);
        assert_eq!(crystal_production(-1, 1), 0.0);
        assert_eq!(solar_plant_energy(0), 0.0);
        assert_eq!(fusion_reactor_energy(-1, 5), 0.0);
        assert_eq!(solar_satellite_energy(0, 50), 0.0);
    }

    #[test]
    fn test_building_cost_zero_level() {
        let c = building_cost("MetalMine", 0);
        assert_eq!(c.metal, 0.0);
    }

    // -----------------------------------------------------------------------
    // Serialization round-trip
    // -----------------------------------------------------------------------

    #[test]
    fn test_resources_serde_roundtrip() {
        let res = Resources::new(123.45, 678.9, 12.0, 50.0);
        let json = serde_json::to_string(&res).unwrap();
        let deser: Resources = serde_json::from_str(&json).unwrap();
        assert_eq!(res, deser);
    }

    #[test]
    fn test_building_cost_serde_roundtrip() {
        let cost = BuildingCost::new(1000.0, 500.0, 250.0, 0.0);
        let json = serde_json::to_string(&cost).unwrap();
        let deser: BuildingCost = serde_json::from_str(&json).unwrap();
        assert_eq!(cost, deser);
    }

    #[test]
    fn test_lazy_state_serde_roundtrip() {
        let state = LazyResourceState {
            last_update: 1_700_000_000,
            metal: 5000.0,
            crystal: 3000.0,
            deuterium: 1000.0,
            metal_per_hour: 300.0,
            crystal_per_hour: 200.0,
            deuterium_per_hour: 100.0,
            metal_storage_cap: 100_000.0,
            crystal_storage_cap: 50_000.0,
            deuterium_storage_cap: 25_000.0,
        };
        let json = serde_json::to_string(&state).unwrap();
        let deser: LazyResourceState = serde_json::from_str(&json).unwrap();
        assert_eq!(state, deser);
    }
}
