#![forbid(unsafe_code)]

//! Core domain models for the Universus game.
//!
//! This crate defines all shared types used across the game engine: resources,
//! coordinates, players, planets, buildings, ships, defenses, research, fleets,
//! queues, battle reports, messages, and universe configuration.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resources {
    pub metal: i64,
    pub crystal: i64,
    pub deuterium: i64,
    pub energy: i64,
    pub dark_matter: i64,
}

impl Default for Resources {
    fn default() -> Self {
        Self::new()
    }
}

impl Resources {
    pub fn new() -> Self {
        Self {
            metal: 0,
            crystal: 0,
            deuterium: 0,
            energy: 0,
            dark_matter: 0,
        }
    }

    /// Add another `Resources` to this one (component-wise).
    pub fn add(&mut self, other: &Resources) {
        self.metal += other.metal;
        self.crystal += other.crystal;
        self.deuterium += other.deuterium;
        self.energy += other.energy;
        self.dark_matter += other.dark_matter;
    }

    /// Subtract `other` from `self`. Returns `true` if all resulting values are
    /// non-negative (i.e. the subtraction was affordable). Returns `false` and
    /// leaves `self` unchanged otherwise.
    pub fn subtract(&mut self, other: &Resources) -> bool {
        if !self.can_afford(other) {
            return false;
        }
        self.metal -= other.metal;
        self.crystal -= other.crystal;
        self.deuterium -= other.deuterium;
        self.energy -= other.energy;
        self.dark_matter -= other.dark_matter;
        true
    }

    /// Whether `self` has enough of every resource to cover `cost`.
    pub fn can_afford(&self, cost: &Resources) -> bool {
        self.metal >= cost.metal
            && self.crystal >= cost.crystal
            && self.deuterium >= cost.deuterium
            && self.energy >= cost.energy
            && self.dark_matter >= cost.dark_matter
    }

    /// Multiply every resource by `factor`.
    pub fn multiply(&mut self, factor: i64) {
        self.metal *= factor;
        self.crystal *= factor;
        self.deuterium *= factor;
        self.energy *= factor;
        self.dark_matter *= factor;
    }

    /// Returns `true` when all resource values are zero.
    pub fn is_empty(&self) -> bool {
        self.metal == 0
            && self.crystal == 0
            && self.deuterium == 0
            && self.energy == 0
            && self.dark_matter == 0
    }
}

// ---------------------------------------------------------------------------
// Coordinates
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Coordinates {
    pub galaxy: i32,
    pub system: i32,
    pub position: i32,
}

impl fmt::Display for Coordinates {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}:{}:{}]", self.galaxy, self.system, self.position)
    }
}

impl Coordinates {
    pub fn new(galaxy: i32, system: i32, position: i32) -> Self {
        Self {
            galaxy,
            system,
            position,
        }
    }

    /// Compute the distance between two coordinate sets using the OGame-style
    /// tiered formula that matches the fleet movement system.
    ///
    /// * Different galaxies: `|g1 - g2| * 20_000`
    /// * Same galaxy, different systems: `|s1 - s2| * 5 * 19 + 2700`
    /// * Same system, different positions: `|p1 - p2| * 5 + 1000`
    /// * Identical coordinates: `5`
    pub fn distance_to(&self, other: &Coordinates) -> i32 {
        if self.galaxy != other.galaxy {
            (self.galaxy - other.galaxy).abs() * 20_000
        } else if self.system != other.system {
            (self.system - other.system).abs() * 5 * 19 + 2700
        } else if self.position != other.position {
            (self.position - other.position).abs() * 5 + 1000
        } else {
            5
        }
    }
}

// ---------------------------------------------------------------------------
// Player status
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayerStatus {
    Active,
    Vacation,
    Banned,
    Inactive,
}

impl fmt::Display for PlayerStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active => write!(f, "Active"),
            Self::Vacation => write!(f, "Vacation"),
            Self::Banned => write!(f, "Banned"),
            Self::Inactive => write!(f, "Inactive"),
        }
    }
}

impl FromStr for PlayerStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "active" => Ok(Self::Active),
            "vacation" => Ok(Self::Vacation),
            "banned" => Ok(Self::Banned),
            "inactive" => Ok(Self::Inactive),
            _ => Err(format!("Unknown player status: {s}")),
        }
    }
}

// ---------------------------------------------------------------------------
// Player
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Player {
    pub id: i32,
    pub username: String,
    pub email: String,
    pub alliance_id: Option<i32>,
    pub score: i64,
    pub rank: i32,
    pub planets: Vec<i32>,
    pub status: PlayerStatus,
    pub created_at: String,
    pub last_login: Option<String>,
}

// ---------------------------------------------------------------------------
// Planet
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanetVisuals {
    pub visual_seed: Option<i64>,
    pub visual_version: Option<String>,
    pub icon_url: Option<String>,
    pub banner_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Planet {
    pub id: i32,
    pub owner_id: i32,
    pub name: String,
    pub coordinates: Coordinates,
    pub resources: Resources,
    pub buildings: HashMap<BuildingType, i32>,
    pub ships: HashMap<ShipType, i32>,
    pub defenses: HashMap<DefenseType, i32>,
    pub diameter: i32,
    pub temperature_min: i32,
    pub temperature_max: i32,
    pub moon_id: Option<i32>,
    pub is_homeworld: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visuals: Option<PlanetVisuals>,
    pub created_at: String,
}

// ---------------------------------------------------------------------------
// BuildingType
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BuildingType {
    MetalMine,
    CrystalMine,
    DeuteriumSynthesizer,
    SolarPlant,
    FusionReactor,
    MetalStorage,
    CrystalStorage,
    DeuteriumTank,
    RoboticsFactory,
    Shipyard,
    ResearchLab,
    NaniteFactory,
    Terraformer,
    MissileSilo,
    AllianceDepot,
    SpaceDock,
}

impl fmt::Display for BuildingType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MetalMine => write!(f, "MetalMine"),
            Self::CrystalMine => write!(f, "CrystalMine"),
            Self::DeuteriumSynthesizer => write!(f, "DeuteriumSynthesizer"),
            Self::SolarPlant => write!(f, "SolarPlant"),
            Self::FusionReactor => write!(f, "FusionReactor"),
            Self::MetalStorage => write!(f, "MetalStorage"),
            Self::CrystalStorage => write!(f, "CrystalStorage"),
            Self::DeuteriumTank => write!(f, "DeuteriumTank"),
            Self::RoboticsFactory => write!(f, "RoboticsFactory"),
            Self::Shipyard => write!(f, "Shipyard"),
            Self::ResearchLab => write!(f, "ResearchLab"),
            Self::NaniteFactory => write!(f, "NaniteFactory"),
            Self::Terraformer => write!(f, "Terraformer"),
            Self::MissileSilo => write!(f, "MissileSilo"),
            Self::AllianceDepot => write!(f, "AllianceDepot"),
            Self::SpaceDock => write!(f, "SpaceDock"),
        }
    }
}

impl FromStr for BuildingType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "MetalMine" | "metal_mine" => Ok(Self::MetalMine),
            "CrystalMine" | "crystal_mine" => Ok(Self::CrystalMine),
            "DeuteriumSynthesizer" | "deuterium_synthesizer" => Ok(Self::DeuteriumSynthesizer),
            "SolarPlant" | "solar_plant" => Ok(Self::SolarPlant),
            "FusionReactor" | "fusion_reactor" => Ok(Self::FusionReactor),
            "MetalStorage" | "metal_storage" => Ok(Self::MetalStorage),
            "CrystalStorage" | "crystal_storage" => Ok(Self::CrystalStorage),
            "DeuteriumTank" | "deuterium_tank" => Ok(Self::DeuteriumTank),
            "RoboticsFactory" | "robotics_factory" => Ok(Self::RoboticsFactory),
            "Shipyard" | "shipyard" => Ok(Self::Shipyard),
            "ResearchLab" | "research_lab" => Ok(Self::ResearchLab),
            "NaniteFactory" | "nanite_factory" => Ok(Self::NaniteFactory),
            "Terraformer" | "terraformer" => Ok(Self::Terraformer),
            "MissileSilo" | "missile_silo" => Ok(Self::MissileSilo),
            "AllianceDepot" | "alliance_depot" => Ok(Self::AllianceDepot),
            "SpaceDock" | "space_dock" => Ok(Self::SpaceDock),
            _ => Err(format!("Unknown building type: {s}")),
        }
    }
}

// ---------------------------------------------------------------------------
// ShipType
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ShipType {
    SmallCargo,
    LargeCargo,
    LightFighter,
    HeavyFighter,
    Cruiser,
    Battleship,
    Battlecruiser,
    Bomber,
    Destroyer,
    Deathstar,
    Recycler,
    EspionageProbe,
    SolarSatellite,
    ColonyShip,
}

impl fmt::Display for ShipType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SmallCargo => write!(f, "SmallCargo"),
            Self::LargeCargo => write!(f, "LargeCargo"),
            Self::LightFighter => write!(f, "LightFighter"),
            Self::HeavyFighter => write!(f, "HeavyFighter"),
            Self::Cruiser => write!(f, "Cruiser"),
            Self::Battleship => write!(f, "Battleship"),
            Self::Battlecruiser => write!(f, "Battlecruiser"),
            Self::Bomber => write!(f, "Bomber"),
            Self::Destroyer => write!(f, "Destroyer"),
            Self::Deathstar => write!(f, "Deathstar"),
            Self::Recycler => write!(f, "Recycler"),
            Self::EspionageProbe => write!(f, "EspionageProbe"),
            Self::SolarSatellite => write!(f, "SolarSatellite"),
            Self::ColonyShip => write!(f, "ColonyShip"),
        }
    }
}

impl FromStr for ShipType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "SmallCargo" | "small_cargo" => Ok(Self::SmallCargo),
            "LargeCargo" | "large_cargo" => Ok(Self::LargeCargo),
            "LightFighter" | "light_fighter" => Ok(Self::LightFighter),
            "HeavyFighter" | "heavy_fighter" => Ok(Self::HeavyFighter),
            "Cruiser" | "cruiser" => Ok(Self::Cruiser),
            "Battleship" | "battleship" => Ok(Self::Battleship),
            "Battlecruiser" | "battlecruiser" => Ok(Self::Battlecruiser),
            "Bomber" | "bomber" => Ok(Self::Bomber),
            "Destroyer" | "destroyer" => Ok(Self::Destroyer),
            "Deathstar" | "deathstar" => Ok(Self::Deathstar),
            "Recycler" | "recycler" => Ok(Self::Recycler),
            "EspionageProbe" | "espionage_probe" => Ok(Self::EspionageProbe),
            "SolarSatellite" | "solar_satellite" => Ok(Self::SolarSatellite),
            "ColonyShip" | "colony_ship" => Ok(Self::ColonyShip),
            _ => Err(format!("Unknown ship type: {s}")),
        }
    }
}

// ---------------------------------------------------------------------------
// DefenseType
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DefenseType {
    RocketLauncher,
    LightLaser,
    HeavyLaser,
    GaussCannon,
    IonCannon,
    PlasmaTurret,
    SmallShieldDome,
    LargeShieldDome,
    AntiBallisticMissile,
    InterplanetaryMissile,
}

impl fmt::Display for DefenseType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RocketLauncher => write!(f, "RocketLauncher"),
            Self::LightLaser => write!(f, "LightLaser"),
            Self::HeavyLaser => write!(f, "HeavyLaser"),
            Self::GaussCannon => write!(f, "GaussCannon"),
            Self::IonCannon => write!(f, "IonCannon"),
            Self::PlasmaTurret => write!(f, "PlasmaTurret"),
            Self::SmallShieldDome => write!(f, "SmallShieldDome"),
            Self::LargeShieldDome => write!(f, "LargeShieldDome"),
            Self::AntiBallisticMissile => write!(f, "AntiBallisticMissile"),
            Self::InterplanetaryMissile => write!(f, "InterplanetaryMissile"),
        }
    }
}

impl FromStr for DefenseType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "RocketLauncher" | "rocket_launcher" => Ok(Self::RocketLauncher),
            "LightLaser" | "light_laser" => Ok(Self::LightLaser),
            "HeavyLaser" | "heavy_laser" => Ok(Self::HeavyLaser),
            "GaussCannon" | "gauss_cannon" => Ok(Self::GaussCannon),
            "IonCannon" | "ion_cannon" => Ok(Self::IonCannon),
            "PlasmaTurret" | "plasma_turret" => Ok(Self::PlasmaTurret),
            "SmallShieldDome" | "small_shield_dome" => Ok(Self::SmallShieldDome),
            "LargeShieldDome" | "large_shield_dome" => Ok(Self::LargeShieldDome),
            "AntiBallisticMissile" | "anti_ballistic_missile" => Ok(Self::AntiBallisticMissile),
            "InterplanetaryMissile" | "interplanetary_missile" => Ok(Self::InterplanetaryMissile),
            _ => Err(format!("Unknown defense type: {s}")),
        }
    }
}

// ---------------------------------------------------------------------------
// ResearchType
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResearchType {
    EnergyTechnology,
    LaserTechnology,
    IonTechnology,
    HyperspaceTechnology,
    PlasmaTechnology,
    CombustionDrive,
    ImpulseDrive,
    HyperspaceDrive,
    EspionageTechnology,
    ComputerTechnology,
    Astrophysics,
    IntergalacticResearchNetwork,
    GravitonTechnology,
    WeaponsTechnology,
    ShieldingTechnology,
    ArmourTechnology,
}

impl fmt::Display for ResearchType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EnergyTechnology => write!(f, "EnergyTechnology"),
            Self::LaserTechnology => write!(f, "LaserTechnology"),
            Self::IonTechnology => write!(f, "IonTechnology"),
            Self::HyperspaceTechnology => write!(f, "HyperspaceTechnology"),
            Self::PlasmaTechnology => write!(f, "PlasmaTechnology"),
            Self::CombustionDrive => write!(f, "CombustionDrive"),
            Self::ImpulseDrive => write!(f, "ImpulseDrive"),
            Self::HyperspaceDrive => write!(f, "HyperspaceDrive"),
            Self::EspionageTechnology => write!(f, "EspionageTechnology"),
            Self::ComputerTechnology => write!(f, "ComputerTechnology"),
            Self::Astrophysics => write!(f, "Astrophysics"),
            Self::IntergalacticResearchNetwork => write!(f, "IntergalacticResearchNetwork"),
            Self::GravitonTechnology => write!(f, "GravitonTechnology"),
            Self::WeaponsTechnology => write!(f, "WeaponsTechnology"),
            Self::ShieldingTechnology => write!(f, "ShieldingTechnology"),
            Self::ArmourTechnology => write!(f, "ArmourTechnology"),
        }
    }
}

impl FromStr for ResearchType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "EnergyTechnology" | "energy_technology" => Ok(Self::EnergyTechnology),
            "LaserTechnology" | "laser_technology" => Ok(Self::LaserTechnology),
            "IonTechnology" | "ion_technology" => Ok(Self::IonTechnology),
            "HyperspaceTechnology" | "hyperspace_technology" => Ok(Self::HyperspaceTechnology),
            "PlasmaTechnology" | "plasma_technology" => Ok(Self::PlasmaTechnology),
            "CombustionDrive" | "combustion_drive" => Ok(Self::CombustionDrive),
            "ImpulseDrive" | "impulse_drive" => Ok(Self::ImpulseDrive),
            "HyperspaceDrive" | "hyperspace_drive" => Ok(Self::HyperspaceDrive),
            "EspionageTechnology" | "espionage_technology" => Ok(Self::EspionageTechnology),
            "ComputerTechnology" | "computer_technology" => Ok(Self::ComputerTechnology),
            "Astrophysics" | "astrophysics" => Ok(Self::Astrophysics),
            "IntergalacticResearchNetwork" | "intergalactic_research_network" => {
                Ok(Self::IntergalacticResearchNetwork)
            }
            "GravitonTechnology" | "graviton_technology" => Ok(Self::GravitonTechnology),
            "WeaponsTechnology" | "weapons_technology" => Ok(Self::WeaponsTechnology),
            "ShieldingTechnology" | "shielding_technology" => Ok(Self::ShieldingTechnology),
            "ArmourTechnology" | "armour_technology" | "armor_technology" | "ArmorTechnology" => {
                Ok(Self::ArmourTechnology)
            }
            _ => Err(format!("Unknown research type: {s}")),
        }
    }
}

// ---------------------------------------------------------------------------
// FleetMission
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FleetMission {
    Attack,
    Transport,
    Deploy,
    Espionage,
    Colonize,
    HarvestDebris,
    Expedition,
    ACSAttack,
    ACSDefend,
    MoonDestruction,
}

impl fmt::Display for FleetMission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Attack => write!(f, "Attack"),
            Self::Transport => write!(f, "Transport"),
            Self::Deploy => write!(f, "Deploy"),
            Self::Espionage => write!(f, "Espionage"),
            Self::Colonize => write!(f, "Colonize"),
            Self::HarvestDebris => write!(f, "HarvestDebris"),
            Self::Expedition => write!(f, "Expedition"),
            Self::ACSAttack => write!(f, "ACSAttack"),
            Self::ACSDefend => write!(f, "ACSDefend"),
            Self::MoonDestruction => write!(f, "MoonDestruction"),
        }
    }
}

impl FromStr for FleetMission {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Attack" | "attack" => Ok(Self::Attack),
            "Transport" | "transport" => Ok(Self::Transport),
            "Deploy" | "deploy" => Ok(Self::Deploy),
            "Espionage" | "espionage" => Ok(Self::Espionage),
            "Colonize" | "colonize" => Ok(Self::Colonize),
            "HarvestDebris" | "harvest_debris" | "harvest" => Ok(Self::HarvestDebris),
            "Expedition" | "expedition" => Ok(Self::Expedition),
            "ACSAttack" | "acs_attack" => Ok(Self::ACSAttack),
            "ACSDefend" | "acs_defend" => Ok(Self::ACSDefend),
            "MoonDestruction" | "moon_destruction" => Ok(Self::MoonDestruction),
            _ => Err(format!("Unknown fleet mission: {s}")),
        }
    }
}

// ---------------------------------------------------------------------------
// Queue items
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildQueueItem {
    pub building_type: BuildingType,
    pub target_level: i32,
    pub planet_id: i32,
    pub start_time: String,
    pub finish_time: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchQueueItem {
    pub research_type: ResearchType,
    pub target_level: i32,
    pub player_id: i32,
    pub start_time: String,
    pub finish_time: String,
}

/// Discriminates whether a shipyard queue item is producing a ship or a
/// defense unit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShipyardItemKind {
    Ship(ShipType),
    Defense(DefenseType),
}

impl fmt::Display for ShipyardItemKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ship(s) => write!(f, "Ship:{s}"),
            Self::Defense(d) => write!(f, "Defense:{d}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShipyardQueueItem {
    pub kind: ShipyardItemKind,
    pub count: i32,
    pub planet_id: i32,
    pub start_time: String,
    pub finish_time: String,
}

// ---------------------------------------------------------------------------
// FleetMovement
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetMovement {
    pub id: i32,
    pub player_id: i32,
    pub mission: FleetMission,
    pub ships: HashMap<ShipType, i32>,
    pub cargo: Resources,
    pub origin: Coordinates,
    pub destination: Coordinates,
    pub departure_time: String,
    pub arrival_time: String,
    pub return_time: Option<String>,
    pub is_returning: bool,
}

// ---------------------------------------------------------------------------
// DebrisField
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebrisField {
    pub coordinates: Coordinates,
    pub metal: i64,
    pub crystal: i64,
}

// ---------------------------------------------------------------------------
// BattleReport
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BattleReport {
    pub id: i32,
    pub attacker_id: i32,
    pub defender_id: i32,
    pub attacker_losses: Resources,
    pub defender_losses: Resources,
    pub loot: Resources,
    pub debris_created: DebrisField,
    pub rounds: Vec<BattleRound>,
    pub timestamp: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BattleRound {
    pub attacker_shots: i32,
    pub defender_shots: i32,
    pub attacker_damage: i64,
    pub defender_damage: i64,
    pub attacker_ships_destroyed: i32,
    pub defender_ships_destroyed: i32,
}

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MessageType {
    Private,
    Alliance,
    Combat,
    Espionage,
    Expedition,
    System,
}

impl fmt::Display for MessageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Private => write!(f, "Private"),
            Self::Alliance => write!(f, "Alliance"),
            Self::Combat => write!(f, "Combat"),
            Self::Espionage => write!(f, "Espionage"),
            Self::Expedition => write!(f, "Expedition"),
            Self::System => write!(f, "System"),
        }
    }
}

impl FromStr for MessageType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Private" | "private" | "player" => Ok(Self::Private),
            "Alliance" | "alliance" => Ok(Self::Alliance),
            "Combat" | "combat" => Ok(Self::Combat),
            "Espionage" | "espionage" => Ok(Self::Espionage),
            "Expedition" | "expedition" => Ok(Self::Expedition),
            "System" | "system" => Ok(Self::System),
            _ => Err(format!("Unknown message type: {s}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub id: i32,
    pub sender_id: Option<i32>,
    pub recipient_id: i32,
    pub subject: String,
    pub content: String,
    pub message_type: MessageType,
    pub is_read: bool,
    pub combat_report_id: Option<i32>,
    pub sent_at: String,
}

// ---------------------------------------------------------------------------
// UniverseSettings
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UniverseSettings {
    pub name: String,
    pub speed_factor: f64,
    pub fleet_speed_factor: f64,
    pub resource_multiplier: f64,
    pub max_galaxies: i32,
    pub max_systems: i32,
    pub max_positions: i32,
}

impl Default for UniverseSettings {
    fn default() -> Self {
        Self {
            name: String::from("Universe 1"),
            speed_factor: 1.0,
            fleet_speed_factor: 1.0,
            resource_multiplier: 1.0,
            max_galaxies: 9,
            max_systems: 499,
            max_positions: 15,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Resources ----------------------------------------------------------

    #[test]
    fn resources_new_is_zero() {
        let r = Resources::new();
        assert!(r.is_empty());
        assert_eq!(r.metal, 0);
        assert_eq!(r.crystal, 0);
        assert_eq!(r.deuterium, 0);
        assert_eq!(r.energy, 0);
        assert_eq!(r.dark_matter, 0);
    }

    #[test]
    fn resources_default_equals_new() {
        assert_eq!(Resources::default(), Resources::new());
    }

    #[test]
    fn resources_add() {
        let mut a = Resources {
            metal: 100,
            crystal: 200,
            deuterium: 50,
            energy: 10,
            dark_matter: 5,
        };
        let b = Resources {
            metal: 50,
            crystal: 100,
            deuterium: 25,
            energy: 5,
            dark_matter: 1,
        };
        a.add(&b);
        assert_eq!(a.metal, 150);
        assert_eq!(a.crystal, 300);
        assert_eq!(a.deuterium, 75);
        assert_eq!(a.energy, 15);
        assert_eq!(a.dark_matter, 6);
    }

    #[test]
    fn resources_subtract_success() {
        let mut a = Resources {
            metal: 100,
            crystal: 200,
            deuterium: 50,
            energy: 10,
            dark_matter: 5,
        };
        let cost = Resources {
            metal: 60,
            crystal: 100,
            deuterium: 50,
            energy: 0,
            dark_matter: 0,
        };
        assert!(a.subtract(&cost));
        assert_eq!(a.metal, 40);
        assert_eq!(a.crystal, 100);
        assert_eq!(a.deuterium, 0);
    }

    #[test]
    fn resources_subtract_failure_leaves_unchanged() {
        let mut a = Resources {
            metal: 100,
            crystal: 200,
            deuterium: 50,
            energy: 10,
            dark_matter: 5,
        };
        let cost = Resources {
            metal: 200,
            crystal: 0,
            deuterium: 0,
            energy: 0,
            dark_matter: 0,
        };
        assert!(!a.subtract(&cost));
        assert_eq!(a.metal, 100);
    }

    #[test]
    fn resources_can_afford() {
        let a = Resources {
            metal: 100,
            crystal: 200,
            deuterium: 50,
            energy: 10,
            dark_matter: 5,
        };
        let affordable = Resources {
            metal: 100,
            crystal: 200,
            deuterium: 50,
            energy: 10,
            dark_matter: 5,
        };
        let too_expensive = Resources {
            metal: 101,
            crystal: 0,
            deuterium: 0,
            energy: 0,
            dark_matter: 0,
        };
        assert!(a.can_afford(&affordable));
        assert!(!a.can_afford(&too_expensive));
    }

    #[test]
    fn resources_can_afford_zero_cost() {
        let a = Resources::new();
        assert!(a.can_afford(&Resources::new()));
    }

    #[test]
    fn resources_multiply() {
        let mut r = Resources {
            metal: 10,
            crystal: 20,
            deuterium: 5,
            energy: 3,
            dark_matter: 1,
        };
        r.multiply(3);
        assert_eq!(r.metal, 30);
        assert_eq!(r.crystal, 60);
        assert_eq!(r.deuterium, 15);
        assert_eq!(r.energy, 9);
        assert_eq!(r.dark_matter, 3);
    }

    #[test]
    fn resources_multiply_by_zero() {
        let mut r = Resources {
            metal: 100,
            crystal: 200,
            deuterium: 50,
            energy: 10,
            dark_matter: 5,
        };
        r.multiply(0);
        assert!(r.is_empty());
    }

    #[test]
    fn resources_is_empty_false() {
        let r = Resources {
            metal: 1,
            crystal: 0,
            deuterium: 0,
            energy: 0,
            dark_matter: 0,
        };
        assert!(!r.is_empty());
    }

    #[test]
    fn resources_serde_roundtrip() {
        let r = Resources {
            metal: 500,
            crystal: 300,
            deuterium: 100,
            energy: 0,
            dark_matter: 0,
        };
        let json = serde_json::to_string(&r).unwrap();
        let r2: Resources = serde_json::from_str(&json).unwrap();
        assert_eq!(r, r2);
    }

    // -- Coordinates --------------------------------------------------------

    #[test]
    fn coordinates_display() {
        let c = Coordinates::new(1, 42, 7);
        assert_eq!(format!("{c}"), "[1:42:7]");
    }

    #[test]
    fn coordinates_distance_different_galaxy() {
        let a = Coordinates::new(1, 1, 1);
        let b = Coordinates::new(3, 1, 1);
        assert_eq!(a.distance_to(&b), 40_000);
    }

    #[test]
    fn coordinates_distance_different_system() {
        let a = Coordinates::new(1, 1, 1);
        let b = Coordinates::new(1, 2, 1);
        assert_eq!(a.distance_to(&b), 2795);
    }

    #[test]
    fn coordinates_distance_different_position() {
        let a = Coordinates::new(1, 1, 1);
        let b = Coordinates::new(1, 1, 2);
        assert_eq!(a.distance_to(&b), 1005);
    }

    #[test]
    fn coordinates_distance_same() {
        let a = Coordinates::new(1, 1, 1);
        assert_eq!(a.distance_to(&a), 5);
    }

    #[test]
    fn coordinates_distance_symmetric() {
        let a = Coordinates::new(1, 50, 8);
        let b = Coordinates::new(3, 120, 12);
        assert_eq!(a.distance_to(&b), b.distance_to(&a));
    }

    #[test]
    fn coordinates_distance_matches_fleet_crate() {
        // These values come directly from game-fleet's test suite.
        assert_eq!(
            Coordinates::new(1, 1, 1).distance_to(&Coordinates::new(2, 1, 1)),
            20_000
        );
        assert_eq!(
            Coordinates::new(1, 1, 1).distance_to(&Coordinates::new(1, 2, 1)),
            2795
        );
        assert_eq!(
            Coordinates::new(1, 1, 1).distance_to(&Coordinates::new(1, 1, 2)),
            1005
        );
    }

    #[test]
    fn coordinates_serde_roundtrip() {
        let c = Coordinates::new(4, 200, 10);
        let json = serde_json::to_string(&c).unwrap();
        let c2: Coordinates = serde_json::from_str(&json).unwrap();
        assert_eq!(c, c2);
    }

    // -- PlayerStatus -------------------------------------------------------

    #[test]
    fn player_status_display_and_parse() {
        for status in [
            PlayerStatus::Active,
            PlayerStatus::Vacation,
            PlayerStatus::Banned,
            PlayerStatus::Inactive,
        ] {
            let s = status.to_string();
            let parsed: PlayerStatus = s.parse().unwrap();
            assert_eq!(parsed, status);
        }
    }

    #[test]
    fn player_status_parse_case_insensitive() {
        assert_eq!(
            "active".parse::<PlayerStatus>().unwrap(),
            PlayerStatus::Active
        );
        assert_eq!(
            "BANNED".parse::<PlayerStatus>().unwrap(),
            PlayerStatus::Banned
        );
    }

    #[test]
    fn player_status_parse_unknown() {
        assert!("sleeping".parse::<PlayerStatus>().is_err());
    }

    // -- BuildingType -------------------------------------------------------

    #[test]
    fn building_type_display_roundtrip() {
        let all = [
            BuildingType::MetalMine,
            BuildingType::CrystalMine,
            BuildingType::DeuteriumSynthesizer,
            BuildingType::SolarPlant,
            BuildingType::FusionReactor,
            BuildingType::MetalStorage,
            BuildingType::CrystalStorage,
            BuildingType::DeuteriumTank,
            BuildingType::RoboticsFactory,
            BuildingType::Shipyard,
            BuildingType::ResearchLab,
            BuildingType::NaniteFactory,
            BuildingType::Terraformer,
            BuildingType::MissileSilo,
            BuildingType::AllianceDepot,
            BuildingType::SpaceDock,
        ];
        for b in all {
            let s = b.to_string();
            let parsed: BuildingType = s.parse().unwrap();
            assert_eq!(parsed, b);
        }
    }

    #[test]
    fn building_type_parse_snake_case() {
        assert_eq!(
            "metal_mine".parse::<BuildingType>().unwrap(),
            BuildingType::MetalMine
        );
        assert_eq!(
            "deuterium_synthesizer".parse::<BuildingType>().unwrap(),
            BuildingType::DeuteriumSynthesizer
        );
    }

    #[test]
    fn building_type_serde_roundtrip() {
        let b = BuildingType::NaniteFactory;
        let json = serde_json::to_string(&b).unwrap();
        let b2: BuildingType = serde_json::from_str(&json).unwrap();
        assert_eq!(b, b2);
    }

    #[test]
    fn building_type_as_hashmap_key() {
        let mut map = HashMap::new();
        map.insert(BuildingType::MetalMine, 5);
        map.insert(BuildingType::CrystalMine, 3);
        assert_eq!(map.get(&BuildingType::MetalMine), Some(&5));
    }

    // -- ShipType -----------------------------------------------------------

    #[test]
    fn ship_type_display_roundtrip() {
        let all = [
            ShipType::SmallCargo,
            ShipType::LargeCargo,
            ShipType::LightFighter,
            ShipType::HeavyFighter,
            ShipType::Cruiser,
            ShipType::Battleship,
            ShipType::Battlecruiser,
            ShipType::Bomber,
            ShipType::Destroyer,
            ShipType::Deathstar,
            ShipType::Recycler,
            ShipType::EspionageProbe,
            ShipType::SolarSatellite,
            ShipType::ColonyShip,
        ];
        for s in all {
            let display = s.to_string();
            let parsed: ShipType = display.parse().unwrap();
            assert_eq!(parsed, s);
        }
    }

    #[test]
    fn ship_type_parse_snake_case() {
        assert_eq!(
            "small_cargo".parse::<ShipType>().unwrap(),
            ShipType::SmallCargo
        );
        assert_eq!(
            "espionage_probe".parse::<ShipType>().unwrap(),
            ShipType::EspionageProbe
        );
    }

    // -- DefenseType --------------------------------------------------------

    #[test]
    fn defense_type_display_roundtrip() {
        let all = [
            DefenseType::RocketLauncher,
            DefenseType::LightLaser,
            DefenseType::HeavyLaser,
            DefenseType::GaussCannon,
            DefenseType::IonCannon,
            DefenseType::PlasmaTurret,
            DefenseType::SmallShieldDome,
            DefenseType::LargeShieldDome,
            DefenseType::AntiBallisticMissile,
            DefenseType::InterplanetaryMissile,
        ];
        for d in all {
            let display = d.to_string();
            let parsed: DefenseType = display.parse().unwrap();
            assert_eq!(parsed, d);
        }
    }

    #[test]
    fn defense_type_parse_snake_case() {
        assert_eq!(
            "plasma_turret".parse::<DefenseType>().unwrap(),
            DefenseType::PlasmaTurret
        );
        assert_eq!(
            "anti_ballistic_missile".parse::<DefenseType>().unwrap(),
            DefenseType::AntiBallisticMissile
        );
    }

    // -- ResearchType -------------------------------------------------------

    #[test]
    fn research_type_display_roundtrip() {
        let all = [
            ResearchType::EnergyTechnology,
            ResearchType::LaserTechnology,
            ResearchType::IonTechnology,
            ResearchType::HyperspaceTechnology,
            ResearchType::PlasmaTechnology,
            ResearchType::CombustionDrive,
            ResearchType::ImpulseDrive,
            ResearchType::HyperspaceDrive,
            ResearchType::EspionageTechnology,
            ResearchType::ComputerTechnology,
            ResearchType::Astrophysics,
            ResearchType::IntergalacticResearchNetwork,
            ResearchType::GravitonTechnology,
            ResearchType::WeaponsTechnology,
            ResearchType::ShieldingTechnology,
            ResearchType::ArmourTechnology,
        ];
        for r in all {
            let display = r.to_string();
            let parsed: ResearchType = display.parse().unwrap();
            assert_eq!(parsed, r);
        }
    }

    #[test]
    fn research_type_parse_armor_aliases() {
        assert_eq!(
            "armor_technology".parse::<ResearchType>().unwrap(),
            ResearchType::ArmourTechnology
        );
        assert_eq!(
            "ArmorTechnology".parse::<ResearchType>().unwrap(),
            ResearchType::ArmourTechnology
        );
        assert_eq!(
            "armour_technology".parse::<ResearchType>().unwrap(),
            ResearchType::ArmourTechnology
        );
    }

    // -- FleetMission -------------------------------------------------------

    #[test]
    fn fleet_mission_display_roundtrip() {
        let all = [
            FleetMission::Attack,
            FleetMission::Transport,
            FleetMission::Deploy,
            FleetMission::Espionage,
            FleetMission::Colonize,
            FleetMission::HarvestDebris,
            FleetMission::Expedition,
            FleetMission::ACSAttack,
            FleetMission::ACSDefend,
            FleetMission::MoonDestruction,
        ];
        for m in all {
            let display = m.to_string();
            let parsed: FleetMission = display.parse().unwrap();
            assert_eq!(parsed, m);
        }
    }

    #[test]
    fn fleet_mission_parse_db_values() {
        // Values from the DB CHECK constraint in 01_core_schema.sql
        assert_eq!(
            "attack".parse::<FleetMission>().unwrap(),
            FleetMission::Attack
        );
        assert_eq!(
            "transport".parse::<FleetMission>().unwrap(),
            FleetMission::Transport
        );
        assert_eq!(
            "deploy".parse::<FleetMission>().unwrap(),
            FleetMission::Deploy
        );
        assert_eq!(
            "espionage".parse::<FleetMission>().unwrap(),
            FleetMission::Espionage
        );
        assert_eq!(
            "colonize".parse::<FleetMission>().unwrap(),
            FleetMission::Colonize
        );
        assert_eq!(
            "harvest".parse::<FleetMission>().unwrap(),
            FleetMission::HarvestDebris
        );
        assert_eq!(
            "acs_attack".parse::<FleetMission>().unwrap(),
            FleetMission::ACSAttack
        );
        assert_eq!(
            "acs_defend".parse::<FleetMission>().unwrap(),
            FleetMission::ACSDefend
        );
    }

    // -- MessageType --------------------------------------------------------

    #[test]
    fn message_type_display_roundtrip() {
        let all = [
            MessageType::Private,
            MessageType::Alliance,
            MessageType::Combat,
            MessageType::Espionage,
            MessageType::Expedition,
            MessageType::System,
        ];
        for mt in all {
            let display = mt.to_string();
            let parsed: MessageType = display.parse().unwrap();
            assert_eq!(parsed, mt);
        }
    }

    #[test]
    fn message_type_parse_db_player_alias() {
        // DB stores "player" for private messages
        assert_eq!(
            "player".parse::<MessageType>().unwrap(),
            MessageType::Private
        );
    }

    // -- ShipyardItemKind ---------------------------------------------------

    #[test]
    fn shipyard_item_kind_display() {
        let ship = ShipyardItemKind::Ship(ShipType::Cruiser);
        assert_eq!(format!("{ship}"), "Ship:Cruiser");

        let defense = ShipyardItemKind::Defense(DefenseType::GaussCannon);
        assert_eq!(format!("{defense}"), "Defense:GaussCannon");
    }

    // -- Queue items --------------------------------------------------------

    #[test]
    fn build_queue_item_serde_roundtrip() {
        let item = BuildQueueItem {
            building_type: BuildingType::MetalMine,
            target_level: 10,
            planet_id: 1,
            start_time: "2025-01-01T00:00:00Z".to_string(),
            finish_time: "2025-01-01T01:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&item).unwrap();
        let item2: BuildQueueItem = serde_json::from_str(&json).unwrap();
        assert_eq!(item, item2);
    }

    #[test]
    fn research_queue_item_serde_roundtrip() {
        let item = ResearchQueueItem {
            research_type: ResearchType::EspionageTechnology,
            target_level: 5,
            player_id: 42,
            start_time: "2025-06-15T12:00:00Z".to_string(),
            finish_time: "2025-06-15T14:30:00Z".to_string(),
        };
        let json = serde_json::to_string(&item).unwrap();
        let item2: ResearchQueueItem = serde_json::from_str(&json).unwrap();
        assert_eq!(item, item2);
    }

    #[test]
    fn shipyard_queue_item_ship_serde_roundtrip() {
        let item = ShipyardQueueItem {
            kind: ShipyardItemKind::Ship(ShipType::Battleship),
            count: 50,
            planet_id: 7,
            start_time: "2025-03-01T08:00:00Z".to_string(),
            finish_time: "2025-03-01T10:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&item).unwrap();
        let item2: ShipyardQueueItem = serde_json::from_str(&json).unwrap();
        assert_eq!(item, item2);
    }

    #[test]
    fn shipyard_queue_item_defense_serde_roundtrip() {
        let item = ShipyardQueueItem {
            kind: ShipyardItemKind::Defense(DefenseType::PlasmaTurret),
            count: 20,
            planet_id: 3,
            start_time: "2025-04-01T00:00:00Z".to_string(),
            finish_time: "2025-04-01T02:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&item).unwrap();
        let item2: ShipyardQueueItem = serde_json::from_str(&json).unwrap();
        assert_eq!(item, item2);
    }

    // -- FleetMovement ------------------------------------------------------

    #[test]
    fn fleet_movement_serde_roundtrip() {
        let mut ships = HashMap::new();
        ships.insert(ShipType::SmallCargo, 100);
        ships.insert(ShipType::LightFighter, 50);

        let fm = FleetMovement {
            id: 1,
            player_id: 42,
            mission: FleetMission::Attack,
            ships,
            cargo: Resources {
                metal: 1000,
                crystal: 500,
                deuterium: 200,
                energy: 0,
                dark_matter: 0,
            },
            origin: Coordinates::new(1, 50, 8),
            destination: Coordinates::new(1, 100, 3),
            departure_time: "2025-01-01T00:00:00Z".to_string(),
            arrival_time: "2025-01-01T01:00:00Z".to_string(),
            return_time: Some("2025-01-01T02:00:00Z".to_string()),
            is_returning: false,
        };

        let json = serde_json::to_string(&fm).unwrap();
        let fm2: FleetMovement = serde_json::from_str(&json).unwrap();
        assert_eq!(fm.id, fm2.id);
        assert_eq!(fm.player_id, fm2.player_id);
        assert_eq!(fm.mission, fm2.mission);
        assert_eq!(fm.ships, fm2.ships);
        assert_eq!(fm.origin, fm2.origin);
        assert_eq!(fm.destination, fm2.destination);
        assert_eq!(fm.is_returning, fm2.is_returning);
    }

    // -- DebrisField --------------------------------------------------------

    #[test]
    fn debris_field_serde_roundtrip() {
        let df = DebrisField {
            coordinates: Coordinates::new(3, 200, 5),
            metal: 50_000,
            crystal: 25_000,
        };
        let json = serde_json::to_string(&df).unwrap();
        let df2: DebrisField = serde_json::from_str(&json).unwrap();
        assert_eq!(df, df2);
    }

    // -- BattleReport -------------------------------------------------------

    #[test]
    fn battle_report_serde_roundtrip() {
        let report = BattleReport {
            id: 1,
            attacker_id: 10,
            defender_id: 20,
            attacker_losses: Resources {
                metal: 5000,
                crystal: 3000,
                deuterium: 0,
                energy: 0,
                dark_matter: 0,
            },
            defender_losses: Resources {
                metal: 8000,
                crystal: 4000,
                deuterium: 1000,
                energy: 0,
                dark_matter: 0,
            },
            loot: Resources {
                metal: 2000,
                crystal: 1000,
                deuterium: 500,
                energy: 0,
                dark_matter: 0,
            },
            debris_created: DebrisField {
                coordinates: Coordinates::new(1, 50, 8),
                metal: 3000,
                crystal: 1500,
            },
            rounds: vec![
                BattleRound {
                    attacker_shots: 100,
                    defender_shots: 80,
                    attacker_damage: 15000,
                    defender_damage: 12000,
                    attacker_ships_destroyed: 3,
                    defender_ships_destroyed: 5,
                },
                BattleRound {
                    attacker_shots: 97,
                    defender_shots: 75,
                    attacker_damage: 14500,
                    defender_damage: 10000,
                    attacker_ships_destroyed: 2,
                    defender_ships_destroyed: 8,
                },
            ],
            timestamp: "2025-07-01T15:30:00Z".to_string(),
        };
        let json = serde_json::to_string(&report).unwrap();
        let report2: BattleReport = serde_json::from_str(&json).unwrap();
        assert_eq!(report, report2);
    }

    // -- Message ------------------------------------------------------------

    #[test]
    fn message_serde_roundtrip() {
        let msg = Message {
            id: 42,
            sender_id: Some(1),
            recipient_id: 2,
            subject: "Test".to_string(),
            content: "Hello".to_string(),
            message_type: MessageType::Private,
            is_read: false,
            combat_report_id: None,
            sent_at: "2025-06-01T12:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let msg2: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, msg2);
    }

    #[test]
    fn message_system_no_sender() {
        let msg = Message {
            id: 1,
            sender_id: None,
            recipient_id: 42,
            subject: "Welcome".to_string(),
            content: "Welcome to Universus!".to_string(),
            message_type: MessageType::System,
            is_read: false,
            combat_report_id: None,
            sent_at: "2025-01-01T00:00:00Z".to_string(),
        };
        assert!(msg.sender_id.is_none());
        assert_eq!(msg.message_type, MessageType::System);
    }

    // -- Player -------------------------------------------------------------

    #[test]
    fn player_serde_roundtrip() {
        let player = Player {
            id: 1,
            username: "Commander".to_string(),
            email: "cmd@universus.io".to_string(),
            alliance_id: Some(5),
            score: 100_000,
            rank: 1,
            planets: vec![1, 2, 3],
            status: PlayerStatus::Active,
            created_at: "2025-01-01T00:00:00Z".to_string(),
            last_login: Some("2025-06-01T12:00:00Z".to_string()),
        };
        let json = serde_json::to_string(&player).unwrap();
        let player2: Player = serde_json::from_str(&json).unwrap();
        assert_eq!(player, player2);
    }

    // -- Planet -------------------------------------------------------------

    #[test]
    fn planet_serde_roundtrip() {
        let mut buildings = HashMap::new();
        buildings.insert(BuildingType::MetalMine, 10);
        buildings.insert(BuildingType::CrystalMine, 8);
        buildings.insert(BuildingType::SolarPlant, 12);

        let mut ships = HashMap::new();
        ships.insert(ShipType::SmallCargo, 50);

        let mut defenses = HashMap::new();
        defenses.insert(DefenseType::RocketLauncher, 100);

        let planet = Planet {
            id: 1,
            owner_id: 42,
            name: "Homeworld".to_string(),
            coordinates: Coordinates::new(1, 50, 8),
            resources: Resources {
                metal: 5000,
                crystal: 3000,
                deuterium: 1000,
                energy: 50,
                dark_matter: 0,
            },
            buildings,
            ships,
            defenses,
            diameter: 12800,
            temperature_min: -20,
            temperature_max: 40,
            moon_id: None,
            is_homeworld: true,
            visuals: Some(PlanetVisuals {
                visual_seed: Some(7_654_321),
                visual_version: Some("v1".to_string()),
                icon_url: Some("/assets/planets/homeworld-icon.webp".to_string()),
                banner_url: Some("/assets/planets/homeworld-banner.webp".to_string()),
            }),
            created_at: "2025-01-01T00:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&planet).unwrap();
        let planet2: Planet = serde_json::from_str(&json).unwrap();
        assert_eq!(planet.id, planet2.id);
        assert_eq!(planet.name, planet2.name);
        assert_eq!(planet.coordinates, planet2.coordinates);
        assert_eq!(planet.buildings, planet2.buildings);
        assert_eq!(planet.ships, planet2.ships);
        assert_eq!(planet.defenses, planet2.defenses);
        assert_eq!(planet.is_homeworld, planet2.is_homeworld);
        assert_eq!(planet.visuals, planet2.visuals);
    }

    #[test]
    fn planet_serde_accepts_missing_visuals() {
        let json = r#"{
            "id": 1,
            "owner_id": 42,
            "name": "Homeworld",
            "coordinates": { "galaxy": 1, "system": 50, "position": 8 },
            "resources": {
                "metal": 5000,
                "crystal": 3000,
                "deuterium": 1000,
                "energy": 50,
                "dark_matter": 0
            },
            "buildings": {},
            "ships": {},
            "defenses": {},
            "diameter": 12800,
            "temperature_min": -20,
            "temperature_max": 40,
            "moon_id": null,
            "is_homeworld": true,
            "created_at": "2025-01-01T00:00:00Z"
        }"#;

        let planet: Planet = serde_json::from_str(json).unwrap();
        assert_eq!(planet.visuals, None);
    }

    // -- UniverseSettings ---------------------------------------------------

    #[test]
    fn universe_settings_defaults() {
        let s = UniverseSettings::default();
        assert_eq!(s.max_galaxies, 9);
        assert_eq!(s.max_systems, 499);
        assert_eq!(s.max_positions, 15);
        assert!((s.speed_factor - 1.0).abs() < f64::EPSILON);
        assert!((s.fleet_speed_factor - 1.0).abs() < f64::EPSILON);
        assert!((s.resource_multiplier - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn universe_settings_serde_roundtrip() {
        let s = UniverseSettings {
            name: "Speed Universe".to_string(),
            speed_factor: 5.0,
            fleet_speed_factor: 3.0,
            resource_multiplier: 2.0,
            max_galaxies: 5,
            max_systems: 200,
            max_positions: 15,
        };
        let json = serde_json::to_string(&s).unwrap();
        let s2: UniverseSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(s.name, s2.name);
        assert!((s.speed_factor - s2.speed_factor).abs() < f64::EPSILON);
        assert_eq!(s.max_galaxies, s2.max_galaxies);
    }

    // -- Cross-type integration tests ---------------------------------------

    #[test]
    fn fleet_movement_with_all_ship_types() {
        let all_ships = [
            ShipType::SmallCargo,
            ShipType::LargeCargo,
            ShipType::LightFighter,
            ShipType::HeavyFighter,
            ShipType::Cruiser,
            ShipType::Battleship,
            ShipType::Battlecruiser,
            ShipType::Bomber,
            ShipType::Destroyer,
            ShipType::Deathstar,
            ShipType::Recycler,
            ShipType::EspionageProbe,
            ShipType::SolarSatellite,
            ShipType::ColonyShip,
        ];
        let mut ships = HashMap::new();
        for (i, s) in all_ships.iter().enumerate() {
            ships.insert(*s, (i as i32) + 1);
        }
        assert_eq!(ships.len(), 14);

        let fm = FleetMovement {
            id: 99,
            player_id: 1,
            mission: FleetMission::Expedition,
            ships,
            cargo: Resources::new(),
            origin: Coordinates::new(1, 1, 1),
            destination: Coordinates::new(9, 499, 15),
            departure_time: "2025-01-01T00:00:00Z".to_string(),
            arrival_time: "2025-01-02T00:00:00Z".to_string(),
            return_time: None,
            is_returning: false,
        };
        let json = serde_json::to_string(&fm).unwrap();
        let fm2: FleetMovement = serde_json::from_str(&json).unwrap();
        assert_eq!(fm.ships.len(), fm2.ships.len());
    }

    #[test]
    fn planet_with_all_building_types() {
        let all = [
            BuildingType::MetalMine,
            BuildingType::CrystalMine,
            BuildingType::DeuteriumSynthesizer,
            BuildingType::SolarPlant,
            BuildingType::FusionReactor,
            BuildingType::MetalStorage,
            BuildingType::CrystalStorage,
            BuildingType::DeuteriumTank,
            BuildingType::RoboticsFactory,
            BuildingType::Shipyard,
            BuildingType::ResearchLab,
            BuildingType::NaniteFactory,
            BuildingType::Terraformer,
            BuildingType::MissileSilo,
            BuildingType::AllianceDepot,
            BuildingType::SpaceDock,
        ];
        let mut buildings = HashMap::new();
        for b in all {
            buildings.insert(b, 1);
        }
        assert_eq!(buildings.len(), 16);
    }

    #[test]
    fn planet_with_all_defense_types() {
        let all = [
            DefenseType::RocketLauncher,
            DefenseType::LightLaser,
            DefenseType::HeavyLaser,
            DefenseType::GaussCannon,
            DefenseType::IonCannon,
            DefenseType::PlasmaTurret,
            DefenseType::SmallShieldDome,
            DefenseType::LargeShieldDome,
            DefenseType::AntiBallisticMissile,
            DefenseType::InterplanetaryMissile,
        ];
        let mut defenses = HashMap::new();
        for d in all {
            defenses.insert(d, 10);
        }
        assert_eq!(defenses.len(), 10);
    }

    #[test]
    fn resources_subtract_then_add_restores() {
        let original = Resources {
            metal: 500,
            crystal: 300,
            deuterium: 100,
            energy: 50,
            dark_matter: 10,
        };
        let cost = Resources {
            metal: 200,
            crystal: 100,
            deuterium: 50,
            energy: 20,
            dark_matter: 5,
        };
        let mut r = original.clone();
        assert!(r.subtract(&cost));
        r.add(&cost);
        assert_eq!(r, original);
    }

    #[test]
    fn coordinates_large_distance_across_universe() {
        let a = Coordinates::new(1, 1, 1);
        let b = Coordinates::new(9, 499, 15);
        // Different galaxy takes priority
        let dist = a.distance_to(&b);
        assert_eq!(dist, 160_000); // 8 * 20_000
    }

    #[test]
    fn enum_parse_unknown_values_produce_errors() {
        assert!("Warp".parse::<BuildingType>().is_err());
        assert!("Starfighter".parse::<ShipType>().is_err());
        assert!("MegaCannon".parse::<DefenseType>().is_err());
        assert!("WarpTechnology".parse::<ResearchType>().is_err());
        assert!("Invasion".parse::<FleetMission>().is_err());
        assert!("Broadcast".parse::<MessageType>().is_err());
    }
}
