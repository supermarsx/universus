#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::fmt;

pub fn crate_name() -> &'static str {
    "game-domain"
}

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Serialize, PartialEq)]
pub struct Resources {
    pub metal: i64,
    pub crystal: i64,
    pub deuterium: i64,
    pub energy: i64,
}

impl Resources {
    pub fn new(metal: i64, crystal: i64, deuterium: i64, energy: i64) -> Self {
        Self { metal, crystal, deuterium, energy }
    }

    pub fn total(&self) -> i64 {
        self.metal + self.crystal + self.deuterium + self.energy
    }

    pub fn can_afford(&self, cost: &Resources) -> bool {
        self.metal >= cost.metal
            && self.crystal >= cost.crystal
            && self.deuterium >= cost.deuterium
            && self.energy >= cost.energy
    }

    pub fn subtract(&mut self, cost: &Resources) -> bool {
        if !self.can_afford(cost) {
            return false;
        }
        self.metal -= cost.metal;
        self.crystal -= cost.crystal;
        self.deuterium -= cost.deuterium;
        self.energy -= cost.energy;
        true
    }

    pub fn add(&mut self, other: &Resources) {
        self.metal += other.metal;
        self.crystal += other.crystal;
        self.deuterium += other.deuterium;
        self.energy += other.energy;
    }
}

// ---------------------------------------------------------------------------
// Coordinates
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Coordinates {
    pub galaxy: i32,
    pub system: i32,
    pub position: i32,
}

impl Coordinates {
    pub fn new(galaxy: i32, system: i32, position: i32) -> Self {
        Self { galaxy, system, position }
    }
}

impl fmt::Display for Coordinates {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}:{}:{}]", self.galaxy, self.system, self.position)
    }
}

// ---------------------------------------------------------------------------
// Planet
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize)]
pub struct Planet {
    pub id: i64,
    pub owner_id: i64,
    pub name: String,
    pub coordinates: Coordinates,
    pub resources: Resources,
    pub temperature: i32,
    pub fields_used: i32,
    pub fields_max: i32,
    pub created_at: String,
}

// ---------------------------------------------------------------------------
// BuildingType
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum BuildingType {
    MetalMine,
    CrystalMine,
    DeuteriumSynthesizer,
    SolarPlant,
    RoboticsFactory,
    Shipyard,
    ResearchLab,
    StorageMetal,
    StorageCrystal,
    StorageDeuterium,
}

impl BuildingType {
    pub fn base_cost(&self) -> Resources {
        match self {
            BuildingType::MetalMine => Resources::new(60, 15, 0, 0),
            BuildingType::CrystalMine => Resources::new(48, 24, 0, 0),
            BuildingType::DeuteriumSynthesizer => Resources::new(225, 75, 0, 0),
            BuildingType::SolarPlant => Resources::new(75, 30, 0, 0),
            BuildingType::RoboticsFactory => Resources::new(400, 120, 200, 0),
            BuildingType::Shipyard => Resources::new(400, 200, 100, 0),
            BuildingType::ResearchLab => Resources::new(200, 400, 200, 0),
            BuildingType::StorageMetal => Resources::new(1000, 0, 0, 0),
            BuildingType::StorageCrystal => Resources::new(1000, 500, 0, 0),
            BuildingType::StorageDeuterium => Resources::new(1000, 1000, 0, 0),
        }
    }

    pub fn cost_at_level(&self, level: i32) -> Resources {
        let base = self.base_cost();
        let multiplier = 1.5_f64.powi(level - 1);
        Resources::new(
            (base.metal as f64 * multiplier) as i64,
            (base.crystal as f64 * multiplier) as i64,
            (base.deuterium as f64 * multiplier) as i64,
            (base.energy as f64 * multiplier) as i64,
        )
    }
}

// ---------------------------------------------------------------------------
// ResearchType
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ResearchType {
    Energy,
    Laser,
    Ion,
    Hyperspace,
    Plasma,
    Espionage,
    Computer,
    Astrophysics,
    Graviton,
    Weapons,
    Shielding,
    Armor,
}

impl ResearchType {
    pub fn base_cost(&self) -> Resources {
        match self {
            ResearchType::Energy => Resources::new(0, 800, 400, 0),
            ResearchType::Laser => Resources::new(200, 100, 0, 0),
            ResearchType::Ion => Resources::new(1000, 300, 100, 0),
            ResearchType::Hyperspace => Resources::new(0, 4000, 2000, 0),
            ResearchType::Plasma => Resources::new(2000, 4000, 1000, 0),
            ResearchType::Espionage => Resources::new(200, 1000, 200, 0),
            ResearchType::Computer => Resources::new(0, 400, 600, 0),
            ResearchType::Astrophysics => Resources::new(4000, 8000, 4000, 0),
            ResearchType::Graviton => Resources::new(0, 0, 0, 300000),
            ResearchType::Weapons => Resources::new(800, 200, 0, 0),
            ResearchType::Shielding => Resources::new(200, 600, 0, 0),
            ResearchType::Armor => Resources::new(1000, 0, 0, 0),
        }
    }

    pub fn cost_at_level(&self, level: i32) -> Resources {
        let base = self.base_cost();
        let multiplier = 1.5_f64.powi(level - 1);
        Resources::new(
            (base.metal as f64 * multiplier) as i64,
            (base.crystal as f64 * multiplier) as i64,
            (base.deuterium as f64 * multiplier) as i64,
            (base.energy as f64 * multiplier) as i64,
        )
    }
}

// ---------------------------------------------------------------------------
// PlayerProfile
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize)]
pub struct PlayerProfile {
    pub id: i64,
    pub username: String,
    pub alliance_id: Option<i64>,
    pub score: i64,
    pub rank: i32,
    pub created_at: String,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crate_name() {
        assert_eq!(crate_name(), "game-domain");
    }

    #[test]
    fn test_resources_new_and_total() {
        let r = Resources::new(100, 200, 300, 50);
        assert_eq!(r.total(), 650);
    }

    #[test]
    fn test_resources_default() {
        let r = Resources::default();
        assert_eq!(r.total(), 0);
        assert_eq!(r, Resources::new(0, 0, 0, 0));
    }

    #[test]
    fn test_resources_can_afford() {
        let r = Resources::new(100, 50, 30, 10);
        assert!(r.can_afford(&Resources::new(100, 50, 30, 10)));
        assert!(r.can_afford(&Resources::new(50, 25, 15, 5)));
        assert!(!r.can_afford(&Resources::new(101, 0, 0, 0)));
        assert!(!r.can_afford(&Resources::new(0, 51, 0, 0)));
    }

    #[test]
    fn test_resources_subtract_success() {
        let mut r = Resources::new(200, 100, 50, 10);
        let cost = Resources::new(60, 15, 0, 0);
        assert!(r.subtract(&cost));
        assert_eq!(r, Resources::new(140, 85, 50, 10));
    }

    #[test]
    fn test_resources_subtract_failure() {
        let mut r = Resources::new(10, 5, 0, 0);
        let cost = Resources::new(60, 15, 0, 0);
        assert!(!r.subtract(&cost));
        // Resources unchanged on failure
        assert_eq!(r, Resources::new(10, 5, 0, 0));
    }

    #[test]
    fn test_resources_add() {
        let mut r = Resources::new(100, 50, 25, 10);
        r.add(&Resources::new(10, 20, 30, 5));
        assert_eq!(r, Resources::new(110, 70, 55, 15));
    }

    #[test]
    fn test_coordinates_display() {
        let c = Coordinates::new(1, 42, 8);
        assert_eq!(format!("{}", c), "[1:42:8]");
    }

    #[test]
    fn test_coordinates_equality_and_hash() {
        use std::collections::HashSet;
        let a = Coordinates::new(1, 2, 3);
        let b = Coordinates::new(1, 2, 3);
        let c = Coordinates::new(1, 2, 4);
        assert_eq!(a, b);
        assert_ne!(a, c);
        let mut set = HashSet::new();
        set.insert(a.clone());
        assert!(set.contains(&b));
        assert!(!set.contains(&c));
    }

    #[test]
    fn test_building_base_cost() {
        let cost = BuildingType::MetalMine.base_cost();
        assert_eq!(cost, Resources::new(60, 15, 0, 0));

        let cost = BuildingType::RoboticsFactory.base_cost();
        assert_eq!(cost, Resources::new(400, 120, 200, 0));
    }

    #[test]
    fn test_building_cost_at_level_one() {
        // Level 1 should equal base cost (1.5^0 = 1)
        let base = BuildingType::CrystalMine.base_cost();
        let level1 = BuildingType::CrystalMine.cost_at_level(1);
        assert_eq!(level1, base);
    }

    #[test]
    fn test_building_cost_at_level_scales() {
        let level1 = BuildingType::MetalMine.cost_at_level(1);
        let level2 = BuildingType::MetalMine.cost_at_level(2);
        let level3 = BuildingType::MetalMine.cost_at_level(3);
        // Level 2: 60 * 1.5 = 90 metal, 15 * 1.5 = 22 crystal
        assert_eq!(level2.metal, 90);
        assert_eq!(level2.crystal, 22);
        // Each level should cost more
        assert!(level3.metal > level2.metal);
        assert!(level2.metal > level1.metal);
    }

    #[test]
    fn test_research_base_cost() {
        let cost = ResearchType::Graviton.base_cost();
        assert_eq!(cost, Resources::new(0, 0, 0, 300000));
    }

    #[test]
    fn test_research_cost_at_level_scales() {
        let level1 = ResearchType::Laser.cost_at_level(1);
        let level3 = ResearchType::Laser.cost_at_level(3);
        assert!(level3.metal > level1.metal);
    }

    #[test]
    fn test_planet_serialization() {
        let planet = Planet {
            id: 1,
            owner_id: 42,
            name: "Homeworld".to_string(),
            coordinates: Coordinates::new(1, 1, 1),
            resources: Resources::new(500, 300, 100, 50),
            temperature: 25,
            fields_used: 3,
            fields_max: 163,
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&planet).unwrap();
        assert!(json.contains("Homeworld"));
        assert!(json.contains("\"galaxy\":1"));
    }

    #[test]
    fn test_player_profile_clone() {
        let p = PlayerProfile {
            id: 1,
            username: "TestPlayer".to_string(),
            alliance_id: Some(5),
            score: 1000,
            rank: 1,
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };
        let p2 = p.clone();
        assert_eq!(p2.username, "TestPlayer");
        assert_eq!(p2.alliance_id, Some(5));
    }
}

