//! Canonical server-side gameplay catalogue and pricing adapter.
//!
//! HTTP payloads contribute only item identifiers and quantities. Every type,
//! target level, prerequisite, cost, energy requirement, duration, and queue
//! input is reconstructed here from `game-domain`, `game-economy`, and the
//! persisted owner snapshot before the durable repository revalidates it.

use std::fmt;

use game_domain::{BuildingType, ResearchType, ShipType};
use platform_db::{GameplayPlanetRow, GameplayQueueInput, GameplayResearchRow, GameplayWriteError};

const MAX_PUBLIC_SHIP_QUANTITY: i64 = 1_000_000_000;
const MAX_QUEUE_DURATION_SECONDS: i64 = 10 * 365 * 24 * 60 * 60;

pub(super) const BUILDINGS: [BuildingType; 16] = [
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

pub(super) const RESEARCH: [ResearchType; 16] = [
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

pub(super) const SHIPS: [ShipType; 14] = [
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CanonicalQuote {
    pub input: GameplayQueueInput,
    pub api_id: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CanonicalShipOption {
    pub ship_type: ShipType,
    pub metal: i64,
    pub crystal: i64,
    pub deuterium: i64,
    pub build_time_seconds: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum CatalogError {
    UnknownBuilding,
    UnknownResearch,
    UnknownShip,
    MissingPrerequisite(String),
    InvalidQuantity,
    Overflow,
}

impl fmt::Display for CatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownBuilding => write!(formatter, "Building type not found"),
            Self::UnknownResearch => write!(formatter, "Research technology not found"),
            Self::UnknownShip => write!(formatter, "Ship type not found"),
            Self::MissingPrerequisite(requirement) => {
                write!(formatter, "Missing prerequisite: {requirement}")
            }
            Self::InvalidQuantity => write!(
                formatter,
                "Ship quantity must be between 1 and {MAX_PUBLIC_SHIP_QUANTITY}"
            ),
            Self::Overflow => write!(formatter, "Calculated order exceeds supported limits"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Requirement {
    Building(BuildingType, i32),
    Research(ResearchType, i32),
}

pub(super) fn building_quote(
    user_id: &str,
    planet: &GameplayPlanetRow,
    research: &GameplayResearchRow,
    raw_type: &str,
    universe_speed: i32,
) -> Result<CanonicalQuote, CatalogError> {
    let building = parse_building(raw_type).ok_or(CatalogError::UnknownBuilding)?;
    ensure_requirements(planet, research, building_requirements(building))?;
    let current_level = building_level(planet, building);
    let target_level = current_level.checked_add(1).ok_or(CatalogError::Overflow)?;
    let cost = game_economy::building_cost(&building.to_string(), target_level);
    let robotics = building_level(planet, BuildingType::RoboticsFactory);
    let nanite = building_level(planet, BuildingType::NaniteFactory);
    let duration = game_economy::building_construction_time(
        cost.metal,
        cost.crystal,
        robotics,
        nanite,
        universe_speed,
    );
    Ok(CanonicalQuote {
        input: GameplayQueueInput {
            user_id: user_id.to_string(),
            planet_id: planet.id.clone(),
            item_type: building_db_key(building).to_string(),
            target_level: Some(target_level),
            quantity: None,
            metal_cost: economy_amount(cost.metal)?,
            crystal_cost: economy_amount(cost.crystal)?,
            deuterium_cost: economy_amount(cost.deuterium)?,
            energy_required: economy_amount(cost.energy)?,
            duration_seconds: economy_duration(duration)?,
        },
        api_id: building_api_id(building),
    })
}

pub(super) fn research_quote(
    user_id: &str,
    planet: &GameplayPlanetRow,
    research: &GameplayResearchRow,
    raw_type: &str,
    universe_speed: i32,
) -> Result<CanonicalQuote, CatalogError> {
    let technology = parse_research(raw_type).ok_or(CatalogError::UnknownResearch)?;
    ensure_requirements(planet, research, research_requirements(technology))?;
    let current_level = research_level(research, technology);
    let target_level = current_level.checked_add(1).ok_or(CatalogError::Overflow)?;
    let cost = game_economy::research_cost(&technology.to_string(), target_level);
    let lab = building_level(planet, BuildingType::ResearchLab);
    let duration = game_economy::research_time(cost.metal, cost.crystal, lab, universe_speed);
    let energy_required = if technology == ResearchType::GravitonTechnology {
        graviton_energy(target_level)?
    } else {
        economy_amount(cost.energy)?
    };
    Ok(CanonicalQuote {
        input: GameplayQueueInput {
            user_id: user_id.to_string(),
            planet_id: planet.id.clone(),
            item_type: research_db_key(technology).to_string(),
            target_level: Some(target_level),
            quantity: None,
            metal_cost: economy_amount(cost.metal)?,
            crystal_cost: economy_amount(cost.crystal)?,
            deuterium_cost: economy_amount(cost.deuterium)?,
            energy_required,
            duration_seconds: economy_duration(duration)?,
        },
        api_id: research_api_id(technology),
    })
}

pub(super) fn ship_quote(
    user_id: &str,
    planet: &GameplayPlanetRow,
    research: &GameplayResearchRow,
    raw_type: &str,
    quantity: i64,
    universe_speed: i32,
) -> Result<CanonicalQuote, CatalogError> {
    let ship = parse_ship(raw_type).ok_or(CatalogError::UnknownShip)?;
    if !(1..=MAX_PUBLIC_SHIP_QUANTITY).contains(&quantity) {
        return Err(CatalogError::InvalidQuantity);
    }
    ensure_requirements(planet, research, ship_requirements(ship))?;
    let per_unit = game_economy::ship_cost(&ship.to_string());
    let shipyard = building_level(planet, BuildingType::Shipyard);
    let nanite = building_level(planet, BuildingType::NaniteFactory);
    let unit_duration = game_economy::shipyard_construction_time(
        per_unit.metal,
        per_unit.crystal,
        shipyard,
        nanite,
        universe_speed,
    );
    Ok(CanonicalQuote {
        input: GameplayQueueInput {
            user_id: user_id.to_string(),
            planet_id: planet.id.clone(),
            item_type: ship_db_key(ship).to_string(),
            target_level: None,
            quantity: Some(quantity),
            metal_cost: checked_total(per_unit.metal, quantity)?,
            crystal_cost: checked_total(per_unit.crystal, quantity)?,
            deuterium_cost: checked_total(per_unit.deuterium, quantity)?,
            energy_required: checked_total(per_unit.energy, quantity)?,
            duration_seconds: checked_duration_total(unit_duration, quantity)?,
        },
        api_id: ship_api_id(ship),
    })
}

pub(super) fn ship_options(
    planet: &GameplayPlanetRow,
    research: &GameplayResearchRow,
    universe_speed: i32,
) -> Result<Vec<CanonicalShipOption>, CatalogError> {
    SHIPS
        .into_iter()
        .filter(|ship| ensure_requirements(planet, research, ship_requirements(*ship)).is_ok())
        .map(|ship| {
            let cost = game_economy::ship_cost(&ship.to_string());
            let duration = game_economy::shipyard_construction_time(
                cost.metal,
                cost.crystal,
                building_level(planet, BuildingType::Shipyard),
                building_level(planet, BuildingType::NaniteFactory),
                universe_speed,
            );
            Ok(CanonicalShipOption {
                ship_type: ship,
                metal: economy_amount(cost.metal)?,
                crystal: economy_amount(cost.crystal)?,
                deuterium: economy_amount(cost.deuterium)?,
                build_time_seconds: economy_duration(duration)?,
            })
        })
        .collect()
}

pub(super) fn building_level(planet: &GameplayPlanetRow, building: BuildingType) -> i32 {
    planet
        .buildings
        .get(building_db_key(building))
        .copied()
        .unwrap_or(0)
}

pub(super) fn research_level(research: &GameplayResearchRow, technology: ResearchType) -> i32 {
    research
        .levels
        .get(research_db_key(technology))
        .copied()
        .unwrap_or(0)
}

pub(super) fn building_api_id(building: BuildingType) -> &'static str {
    match building {
        BuildingType::MetalMine => "metalMine",
        BuildingType::CrystalMine => "crystalMine",
        BuildingType::DeuteriumSynthesizer => "deuteriumSynthesizer",
        BuildingType::SolarPlant => "solarPlant",
        BuildingType::FusionReactor => "fusionReactor",
        BuildingType::MetalStorage => "metalStorage",
        BuildingType::CrystalStorage => "crystalStorage",
        BuildingType::DeuteriumTank => "deuteriumTank",
        BuildingType::RoboticsFactory => "roboticsFactory",
        BuildingType::Shipyard => "shipyard",
        BuildingType::ResearchLab => "researchLab",
        BuildingType::NaniteFactory => "naniteFactory",
        BuildingType::Terraformer => "terraformer",
        BuildingType::MissileSilo => "missileSilo",
        BuildingType::AllianceDepot => "allianceDepot",
        BuildingType::SpaceDock => "spaceDock",
    }
}

pub(super) fn building_name(building: BuildingType) -> &'static str {
    match building {
        BuildingType::MetalMine => "Metal Mine",
        BuildingType::CrystalMine => "Crystal Mine",
        BuildingType::DeuteriumSynthesizer => "Deuterium Synthesizer",
        BuildingType::SolarPlant => "Solar Plant",
        BuildingType::FusionReactor => "Fusion Reactor",
        BuildingType::MetalStorage => "Metal Storage",
        BuildingType::CrystalStorage => "Crystal Storage",
        BuildingType::DeuteriumTank => "Deuterium Tank",
        BuildingType::RoboticsFactory => "Robotics Factory",
        BuildingType::Shipyard => "Shipyard",
        BuildingType::ResearchLab => "Research Lab",
        BuildingType::NaniteFactory => "Nanite Factory",
        BuildingType::Terraformer => "Terraformer",
        BuildingType::MissileSilo => "Missile Silo",
        BuildingType::AllianceDepot => "Alliance Depot",
        BuildingType::SpaceDock => "Space Dock",
    }
}

pub(super) fn research_api_id(technology: ResearchType) -> &'static str {
    research_db_key(technology)
}

pub(super) fn ship_api_id(ship: ShipType) -> &'static str {
    match ship {
        ShipType::SmallCargo => "smallCargo",
        ShipType::LargeCargo => "largeCargo",
        ShipType::LightFighter => "lightFighter",
        ShipType::HeavyFighter => "heavyFighter",
        ShipType::Cruiser => "cruiser",
        ShipType::Battleship => "battleship",
        ShipType::Battlecruiser => "battlecruiser",
        ShipType::Bomber => "bomber",
        ShipType::Destroyer => "destroyer",
        ShipType::Deathstar => "deathstar",
        ShipType::Recycler => "recycler",
        ShipType::EspionageProbe => "espionageProbe",
        ShipType::SolarSatellite => "solarSatellite",
        ShipType::ColonyShip => "colonyShip",
    }
}

pub(super) fn research_name(technology: ResearchType) -> &'static str {
    match technology {
        ResearchType::EnergyTechnology => "Energy Technology",
        ResearchType::LaserTechnology => "Laser Technology",
        ResearchType::IonTechnology => "Ion Technology",
        ResearchType::HyperspaceTechnology => "Hyperspace Technology",
        ResearchType::PlasmaTechnology => "Plasma Technology",
        ResearchType::CombustionDrive => "Combustion Drive",
        ResearchType::ImpulseDrive => "Impulse Drive",
        ResearchType::HyperspaceDrive => "Hyperspace Drive",
        ResearchType::EspionageTechnology => "Espionage Technology",
        ResearchType::ComputerTechnology => "Computer Technology",
        ResearchType::Astrophysics => "Astrophysics",
        ResearchType::IntergalacticResearchNetwork => "Intergalactic Research Network",
        ResearchType::GravitonTechnology => "Graviton Technology",
        ResearchType::WeaponsTechnology => "Weapons Technology",
        ResearchType::ShieldingTechnology => "Shielding Technology",
        ResearchType::ArmourTechnology => "Armour Technology",
    }
}

pub(super) fn map_write_error(error: GameplayWriteError) -> GatewayGameplayError {
    match error {
        GameplayWriteError::NotFound => GatewayGameplayError::NotFound,
        GameplayWriteError::UniverseFull => {
            GatewayGameplayError::BadRequest("Universe has no free coordinates".to_string())
        }
        GameplayWriteError::QueueBusy | GameplayWriteError::StaleState => {
            GatewayGameplayError::Conflict("A conflicting gameplay order is active".to_string())
        }
        GameplayWriteError::InsufficientResources => GatewayGameplayError::BadRequest(
            "Insufficient resources or available energy".to_string(),
        ),
        GameplayWriteError::Invalid(message) => GatewayGameplayError::BadRequest(message),
        GameplayWriteError::Retryable(_) => GatewayGameplayError::Unavailable,
        GameplayWriteError::Database(_) => GatewayGameplayError::Unavailable,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum GatewayGameplayError {
    BadRequest(String),
    NotFound,
    Conflict(String),
    Unavailable,
}

pub(super) fn parse_building(value: &str) -> Option<BuildingType> {
    let token = normalized_token(value);
    BUILDINGS
        .into_iter()
        .find(|building| normalized_token(&building.to_string()) == token)
}

pub(super) fn parse_research(value: &str) -> Option<ResearchType> {
    let token = normalized_token(value);
    let token = match token.as_str() {
        "energytech" => "energytechnology",
        "lasertech" => "lasertechnology",
        "iontech" => "iontechnology",
        "hyperspacetech" => "hyperspacetechnology",
        "plasmatech" => "plasmatechnology",
        "espionagetech" => "espionagetechnology",
        "computertech" => "computertechnology",
        "weaponstech" => "weaponstechnology",
        "shieldingtech" => "shieldingtechnology",
        "armortechnology" | "armortech" => "armourtechnology",
        other => other,
    };
    RESEARCH
        .into_iter()
        .find(|technology| normalized_token(&technology.to_string()) == token)
}

pub(super) fn parse_ship(value: &str) -> Option<ShipType> {
    let token = normalized_token(value);
    SHIPS
        .into_iter()
        .find(|ship| normalized_token(&ship.to_string()) == token)
}

fn normalized_token(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn ensure_requirements(
    planet: &GameplayPlanetRow,
    research: &GameplayResearchRow,
    requirements: &[Requirement],
) -> Result<(), CatalogError> {
    for requirement in requirements {
        let (actual, needed, label) = match *requirement {
            Requirement::Building(building, level) => (
                building_level(planet, building),
                level,
                building.to_string(),
            ),
            Requirement::Research(technology, level) => (
                research_level(research, technology),
                level,
                technology.to_string(),
            ),
        };
        if actual < needed {
            return Err(CatalogError::MissingPrerequisite(format!(
                "{label} level {needed} (current {actual})"
            )));
        }
    }
    Ok(())
}

fn building_requirements(building: BuildingType) -> &'static [Requirement] {
    match building {
        BuildingType::FusionReactor => &[
            Requirement::Building(BuildingType::DeuteriumSynthesizer, 5),
            Requirement::Research(ResearchType::EnergyTechnology, 3),
        ],
        BuildingType::Shipyard => &[Requirement::Building(BuildingType::RoboticsFactory, 2)],
        BuildingType::NaniteFactory => &[
            Requirement::Building(BuildingType::RoboticsFactory, 10),
            Requirement::Research(ResearchType::ComputerTechnology, 10),
        ],
        BuildingType::Terraformer => &[
            Requirement::Building(BuildingType::NaniteFactory, 1),
            Requirement::Research(ResearchType::EnergyTechnology, 12),
        ],
        BuildingType::MissileSilo => &[Requirement::Building(BuildingType::Shipyard, 1)],
        BuildingType::SpaceDock => &[Requirement::Building(BuildingType::Shipyard, 2)],
        _ => &[],
    }
}

fn research_requirements(technology: ResearchType) -> &'static [Requirement] {
    use BuildingType::ResearchLab;
    match technology {
        ResearchType::EnergyTechnology => &[Requirement::Building(ResearchLab, 1)],
        ResearchType::LaserTechnology => &[
            Requirement::Building(ResearchLab, 1),
            Requirement::Research(ResearchType::EnergyTechnology, 2),
        ],
        ResearchType::IonTechnology => &[
            Requirement::Building(ResearchLab, 4),
            Requirement::Research(ResearchType::EnergyTechnology, 4),
            Requirement::Research(ResearchType::LaserTechnology, 5),
        ],
        ResearchType::HyperspaceTechnology => &[
            Requirement::Building(ResearchLab, 7),
            Requirement::Research(ResearchType::EnergyTechnology, 5),
            Requirement::Research(ResearchType::ShieldingTechnology, 5),
        ],
        ResearchType::PlasmaTechnology => &[
            Requirement::Building(ResearchLab, 4),
            Requirement::Research(ResearchType::EnergyTechnology, 8),
            Requirement::Research(ResearchType::LaserTechnology, 10),
            Requirement::Research(ResearchType::IonTechnology, 5),
        ],
        ResearchType::CombustionDrive => &[
            Requirement::Building(ResearchLab, 1),
            Requirement::Research(ResearchType::EnergyTechnology, 1),
        ],
        ResearchType::ImpulseDrive => &[
            Requirement::Building(ResearchLab, 2),
            Requirement::Research(ResearchType::EnergyTechnology, 1),
        ],
        ResearchType::HyperspaceDrive => &[
            Requirement::Building(ResearchLab, 7),
            Requirement::Research(ResearchType::HyperspaceTechnology, 3),
        ],
        ResearchType::EspionageTechnology => &[Requirement::Building(ResearchLab, 3)],
        ResearchType::ComputerTechnology => &[Requirement::Building(ResearchLab, 1)],
        ResearchType::Astrophysics => &[
            Requirement::Building(ResearchLab, 3),
            Requirement::Research(ResearchType::EspionageTechnology, 4),
            Requirement::Research(ResearchType::ImpulseDrive, 3),
        ],
        ResearchType::IntergalacticResearchNetwork => &[
            Requirement::Building(ResearchLab, 10),
            Requirement::Research(ResearchType::ComputerTechnology, 8),
            Requirement::Research(ResearchType::HyperspaceTechnology, 8),
        ],
        ResearchType::GravitonTechnology => &[Requirement::Building(ResearchLab, 12)],
        ResearchType::WeaponsTechnology => &[Requirement::Building(ResearchLab, 4)],
        ResearchType::ShieldingTechnology => &[
            Requirement::Building(ResearchLab, 6),
            Requirement::Research(ResearchType::EnergyTechnology, 3),
        ],
        ResearchType::ArmourTechnology => &[Requirement::Building(ResearchLab, 2)],
    }
}

fn ship_requirements(ship: ShipType) -> &'static [Requirement] {
    use BuildingType::Shipyard;
    match ship {
        ShipType::SmallCargo => &[
            Requirement::Building(Shipyard, 2),
            Requirement::Research(ResearchType::CombustionDrive, 2),
        ],
        ShipType::LargeCargo => &[
            Requirement::Building(Shipyard, 4),
            Requirement::Research(ResearchType::CombustionDrive, 6),
        ],
        ShipType::LightFighter => &[
            Requirement::Building(Shipyard, 1),
            Requirement::Research(ResearchType::CombustionDrive, 1),
        ],
        ShipType::HeavyFighter => &[
            Requirement::Building(Shipyard, 3),
            Requirement::Research(ResearchType::ArmourTechnology, 2),
            Requirement::Research(ResearchType::ImpulseDrive, 2),
        ],
        ShipType::Cruiser => &[
            Requirement::Building(Shipyard, 5),
            Requirement::Research(ResearchType::ImpulseDrive, 4),
            Requirement::Research(ResearchType::IonTechnology, 2),
        ],
        ShipType::Battleship => &[
            Requirement::Building(Shipyard, 7),
            Requirement::Research(ResearchType::HyperspaceDrive, 4),
        ],
        ShipType::Battlecruiser => &[
            Requirement::Building(Shipyard, 8),
            Requirement::Research(ResearchType::HyperspaceTechnology, 5),
            Requirement::Research(ResearchType::HyperspaceDrive, 5),
            Requirement::Research(ResearchType::LaserTechnology, 12),
        ],
        ShipType::Bomber => &[
            Requirement::Building(Shipyard, 8),
            Requirement::Research(ResearchType::ImpulseDrive, 6),
            Requirement::Research(ResearchType::PlasmaTechnology, 5),
        ],
        ShipType::Destroyer => &[
            Requirement::Building(Shipyard, 9),
            Requirement::Research(ResearchType::HyperspaceTechnology, 5),
            Requirement::Research(ResearchType::HyperspaceDrive, 6),
        ],
        ShipType::Deathstar => &[
            Requirement::Building(Shipyard, 12),
            Requirement::Research(ResearchType::HyperspaceTechnology, 6),
            Requirement::Research(ResearchType::HyperspaceDrive, 7),
            Requirement::Research(ResearchType::GravitonTechnology, 1),
        ],
        ShipType::Recycler => &[
            Requirement::Building(Shipyard, 4),
            Requirement::Research(ResearchType::CombustionDrive, 6),
            Requirement::Research(ResearchType::ShieldingTechnology, 2),
        ],
        ShipType::EspionageProbe => &[
            Requirement::Building(Shipyard, 3),
            Requirement::Research(ResearchType::CombustionDrive, 3),
            Requirement::Research(ResearchType::EspionageTechnology, 2),
        ],
        ShipType::SolarSatellite => &[Requirement::Building(Shipyard, 1)],
        ShipType::ColonyShip => &[
            Requirement::Building(Shipyard, 4),
            Requirement::Research(ResearchType::ImpulseDrive, 3),
        ],
    }
}

fn economy_amount(value: f64) -> Result<i64, CatalogError> {
    if !value.is_finite() || value < 0.0 || value > i64::MAX as f64 {
        return Err(CatalogError::Overflow);
    }
    Ok(value.floor() as i64)
}

fn economy_duration(value: f64) -> Result<i64, CatalogError> {
    if !value.is_finite() || value < 0.0 || value > MAX_QUEUE_DURATION_SECONDS as f64 {
        return Err(CatalogError::Overflow);
    }
    Ok((value.ceil() as i64).clamp(1, MAX_QUEUE_DURATION_SECONDS))
}

fn checked_total(per_unit: f64, quantity: i64) -> Result<i64, CatalogError> {
    economy_amount(per_unit)?
        .checked_mul(quantity)
        .ok_or(CatalogError::Overflow)
}

fn checked_duration_total(per_unit: f64, quantity: i64) -> Result<i64, CatalogError> {
    let unit = economy_duration(per_unit)?;
    let total = unit.checked_mul(quantity).ok_or(CatalogError::Overflow)?;
    if total > MAX_QUEUE_DURATION_SECONDS {
        return Err(CatalogError::Overflow);
    }
    Ok(total)
}

fn graviton_energy(target_level: i32) -> Result<i64, CatalogError> {
    let exponent =
        u32::try_from(target_level.saturating_sub(1)).map_err(|_| CatalogError::Overflow)?;
    300_000_i64
        .checked_mul(2_i64.checked_pow(exponent).ok_or(CatalogError::Overflow)?)
        .ok_or(CatalogError::Overflow)
}

fn building_db_key(building: BuildingType) -> &'static str {
    match building {
        BuildingType::MetalMine => "metal_mine",
        BuildingType::CrystalMine => "crystal_mine",
        BuildingType::DeuteriumSynthesizer => "deuterium_synthesizer",
        BuildingType::SolarPlant => "solar_plant",
        BuildingType::FusionReactor => "fusion_reactor",
        BuildingType::MetalStorage => "metal_storage",
        BuildingType::CrystalStorage => "crystal_storage",
        BuildingType::DeuteriumTank => "deuterium_tank",
        BuildingType::RoboticsFactory => "robotics_factory",
        BuildingType::Shipyard => "shipyard",
        BuildingType::ResearchLab => "research_lab",
        BuildingType::NaniteFactory => "nanite_factory",
        BuildingType::Terraformer => "terraformer",
        BuildingType::MissileSilo => "missile_silo",
        BuildingType::AllianceDepot => "alliance_depot",
        BuildingType::SpaceDock => "space_dock",
    }
}

fn research_db_key(technology: ResearchType) -> &'static str {
    match technology {
        ResearchType::EnergyTechnology => "energy_technology",
        ResearchType::LaserTechnology => "laser_technology",
        ResearchType::IonTechnology => "ion_technology",
        ResearchType::HyperspaceTechnology => "hyperspace_technology",
        ResearchType::PlasmaTechnology => "plasma_technology",
        ResearchType::CombustionDrive => "combustion_drive",
        ResearchType::ImpulseDrive => "impulse_drive",
        ResearchType::HyperspaceDrive => "hyperspace_drive",
        ResearchType::EspionageTechnology => "espionage_technology",
        ResearchType::ComputerTechnology => "computer_technology",
        ResearchType::Astrophysics => "astrophysics",
        ResearchType::IntergalacticResearchNetwork => "intergalactic_research_network",
        ResearchType::GravitonTechnology => "graviton_technology",
        ResearchType::WeaponsTechnology => "weapons_technology",
        ResearchType::ShieldingTechnology => "shielding_technology",
        ResearchType::ArmourTechnology => "armour_technology",
    }
}

fn ship_db_key(ship: ShipType) -> &'static str {
    match ship {
        ShipType::SmallCargo => "small_cargo",
        ShipType::LargeCargo => "large_cargo",
        ShipType::LightFighter => "light_fighter",
        ShipType::HeavyFighter => "heavy_fighter",
        ShipType::Cruiser => "cruiser",
        ShipType::Battleship => "battleship",
        ShipType::Battlecruiser => "battlecruiser",
        ShipType::Bomber => "bomber",
        ShipType::Destroyer => "destroyer",
        ShipType::Deathstar => "deathstar",
        ShipType::Recycler => "recycler",
        ShipType::EspionageProbe => "espionage_probe",
        ShipType::SolarSatellite => "solar_satellite",
        ShipType::ColonyShip => "colony_ship",
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn planet() -> GameplayPlanetRow {
        GameplayPlanetRow {
            id: "7".to_string(),
            user_id: "3".to_string(),
            universe_id: 1,
            name: "Test".to_string(),
            galaxy: 1,
            system: 1,
            position: 1,
            temperature: 20,
            metal: 1_000_000_000,
            crystal: 1_000_000_000,
            deuterium: 1_000_000_000,
            energy: 1_000_000,
            buildings: BUILDINGS
                .into_iter()
                .map(|building| (building_db_key(building).to_string(), 20))
                .collect(),
            ships: BTreeMap::new(),
        }
    }

    fn research() -> GameplayResearchRow {
        GameplayResearchRow {
            user_id: "3".to_string(),
            levels: RESEARCH
                .into_iter()
                .map(|technology| (research_db_key(technology).to_string(), 20))
                .collect(),
        }
    }

    #[test]
    fn aliases_collapse_to_domain_types_and_database_keys() {
        let planet = planet();
        let research = research();
        let camel = building_quote("3", &planet, &research, "metalMine", 1).unwrap();
        let snake = building_quote("3", &planet, &research, "metal_mine", 1).unwrap();
        assert_eq!(camel.input.item_type, "metal_mine");
        assert_eq!(camel.input, snake.input);

        let short = research_quote("3", &planet, &research, "energy_tech", 1).unwrap();
        assert_eq!(short.input.item_type, "energy_technology");
        let ship = ship_quote("3", &planet, &research, "smallCargo", 2, 1).unwrap();
        assert_eq!(ship.input.item_type, "small_cargo");
    }

    #[test]
    fn quotes_ignore_all_client_pricing_and_use_economy_formulas() {
        let mut state = planet();
        state.buildings.insert("metal_mine".to_string(), 0);
        state.buildings.insert("robotics_factory".to_string(), 0);
        state.buildings.insert("nanite_factory".to_string(), 0);
        let quote = building_quote("3", &state, &research(), "MetalMine", 1).unwrap();
        assert_eq!(quote.input.target_level, Some(1));
        assert_eq!(quote.input.metal_cost, 60);
        assert_eq!(quote.input.crystal_cost, 15);
        assert_eq!(quote.input.duration_seconds, 108);

        let ships = ship_quote("3", &planet(), &research(), "small_cargo", 2, 1).unwrap();
        assert_eq!(ships.input.metal_cost, 4_000);
        assert_eq!(ships.input.crystal_cost, 4_000);
        assert_eq!(ships.input.quantity, Some(2));
    }

    #[test]
    fn prerequisites_and_overflow_are_enforced_before_persistence() {
        let mut state = planet();
        state.buildings.insert("shipyard".to_string(), 0);
        let error = ship_quote("3", &state, &research(), "smallCargo", 1, 1).unwrap_err();
        assert!(matches!(error, CatalogError::MissingPrerequisite(_)));
        assert_eq!(
            ship_quote("3", &planet(), &research(), "smallCargo", 0, 1),
            Err(CatalogError::InvalidQuantity)
        );
    }

    #[test]
    fn full_domain_catalogue_has_unique_external_and_storage_ids() {
        let building_api = BUILDINGS.map(building_api_id);
        let building_db = BUILDINGS.map(building_db_key);
        let research_api = RESEARCH.map(research_api_id);
        let research_db = RESEARCH.map(research_db_key);
        let ship_api = SHIPS.map(ship_api_id);
        let ship_db = SHIPS.map(ship_db_key);
        for ids in [
            building_api.as_slice(),
            building_db.as_slice(),
            research_api.as_slice(),
            research_db.as_slice(),
            ship_api.as_slice(),
            ship_db.as_slice(),
        ] {
            let unique = ids.iter().collect::<std::collections::BTreeSet<_>>();
            assert_eq!(unique.len(), ids.len());
        }
    }
}
