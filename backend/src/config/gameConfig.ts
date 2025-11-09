/**
 * @module backend/config/gameConfig
 *
 * Centralized game configuration for Universus. This module defines the
 * canonical data shapes for buildings, ships, defenses and research along
 * with constants containing the concrete game values. It also provides a set
 * of helper functions to calculate derived values such as scaled costs,
 * build times, production rates and storage capacity.
 *
 * The configuration objects use simple plain-objects for ease of serialization
 * and editing. Helper functions accept human-readable keys (e.g. 'metal_mine')
 * and perform validation, throwing on unknown types.
 */

/**
 * BuildingCost
 *
 * Represents the resource costs for buildings, ships or research. All values
 * are integer resource amounts. `energy` is optional because not all
 * technologies require energy as a resource.
 *
 * @property {number} metal - Metal cost
 * @property {number} crystal - Crystal cost
 * @property {number} deuterium - Deuterium cost
 * @property {number} [energy] - Optional energy cost
 */
export interface BuildingCost {
  metal: number;
  crystal: number;
  deuterium: number;
  energy?: number;
}

/**
 * BuildingConfig
 *
 * Configuration for a single building type.
 *
 * - `baseCost`: resources required to build level 1.
 * - `costMultiplier`: exponential growth factor per level.
 * - `baseProduction`/`productionMultiplier`: optional fields for resource
 *    producing buildings (per-level production formula).
 * - `baseTime`: base build time (seconds) used as a multiplier by helpers.
 * - `requirements`: optional prerequisites (other buildings or research levels).
 *
 * @property {BuildingCost} baseCost
 * @property {number} costMultiplier
 * @property {number} [baseProduction]
 * @property {number} [productionMultiplier]
 * @property {number} baseTime - Base construction time in seconds
 * @property {Object} [requirements]
 */
export interface BuildingConfig {
  baseCost: BuildingCost;
  costMultiplier: number;
  baseProduction?: number;
  productionMultiplier?: number;
  baseTime: number;
  requirements?: {
    buildings?: { [key: string]: number };
    research?: { [key: string]: number };
  };
}

/**
 * ShipConfig
 *
 * Defines the properties for ships and defensive units. Many attributes are
 * used by combat, movement and construction systems.
 *
 * @property {BuildingCost} cost
 * @property {number} structurePoints - Hull/HP value
 * @property {number} shieldPower - Shield strength
 * @property {number} weaponPower - Offensive weapon strength
 * @property {number} cargo - Cargo capacity
 * @property {number} baseSpeed - Movement speed (units)
 * @property {number} fuelConsumption - Fuel consumed per trip/unit
 * @property {Object.<string, number>} [rapidFire] - Rapid-fire multipliers vs other unit types
 * @property {number} buildTime - Time in seconds to build
 */
export interface ShipConfig {
  cost: BuildingCost;
  structurePoints: number;
  shieldPower: number;
  weaponPower: number;
  cargo: number;
  baseSpeed: number;
  fuelConsumption: number;
  rapidFire?: { [key: string]: number };
  buildTime: number;
}

/**
 * ResearchConfig
 *
 * Holds metadata and cost information for research topics.
 *
 * @property {BuildingCost} baseCost
 * @property {number} costMultiplier
 * @property {number} baseTime - Research base time in seconds
 * @property {string} [displayName]
 * @property {string} [description]
 * @property {string} [category]
 * @property {Object} [requirements]
 */
export interface ResearchConfig {
  baseCost: BuildingCost;
  costMultiplier: number;
  baseTime: number;
  displayName?: string;
  description?: string;
  category?: string;
  requirements?: {
    buildings?: { [key: string]: number };
    research?: { [key: string]: number };
  };
}

/**
 * BUILDINGS
 *
 * Mapping of building keys to their `BuildingConfig`. Keys are stable
 * identifiers used throughout the game logic (snake_case). Values describe
 * costs, time and production parameters.
 *
 * Read-only at runtime; treat as configuration data.
 *
 * @constant {Object.<string, BuildingConfig>}
 */
export const BUILDINGS: { [key: string]: BuildingConfig } = {
  metal_mine: {
    baseCost: { metal: 60, crystal: 15, deuterium: 0 },
    costMultiplier: 1.5,
    baseProduction: 30,
    productionMultiplier: 1.1,
    baseTime: 60,
  },
  crystal_mine: {
    baseCost: { metal: 48, crystal: 24, deuterium: 0 },
    costMultiplier: 1.6,
    baseProduction: 20,
    productionMultiplier: 1.1,
    baseTime: 60,
  },
  deuterium_synthesizer: {
    baseCost: { metal: 225, crystal: 75, deuterium: 0 },
    costMultiplier: 1.5,
    baseProduction: 10,
    productionMultiplier: 1.1,
    baseTime: 60,
  },
  solar_plant: {
    baseCost: { metal: 75, crystal: 30, deuterium: 0 },
    costMultiplier: 1.5,
    baseProduction: 20,
    productionMultiplier: 1.1,
    baseTime: 60,
  },
  fusion_reactor: {
    baseCost: { metal: 900, crystal: 360, deuterium: 180 },
    costMultiplier: 1.8,
    baseProduction: 50,
    productionMultiplier: 1.05,
    baseTime: 180,
    requirements: {
      buildings: { deuterium_synthesizer: 5 },
      research: { energy_technology: 3 },
    },
  },
  robotics_factory: {
    baseCost: { metal: 400, crystal: 120, deuterium: 200 },
    costMultiplier: 2.0,
    baseTime: 120,
  },
  nanite_factory: {
    baseCost: { metal: 1000000, crystal: 500000, deuterium: 100000 },
    costMultiplier: 2.0,
    baseTime: 3600,
    requirements: {
      buildings: { robotics_factory: 10 },
      research: { computer_technology: 10 },
    },
  },
  shipyard: {
    baseCost: { metal: 400, crystal: 200, deuterium: 100 },
    costMultiplier: 2.0,
    baseTime: 120,
    requirements: {
      buildings: { robotics_factory: 2 },
    },
  },
  metal_storage: {
    baseCost: { metal: 1000, crystal: 0, deuterium: 0 },
    costMultiplier: 2.0,
    baseTime: 60,
  },
  crystal_storage: {
    baseCost: { metal: 1000, crystal: 500, deuterium: 0 },
    costMultiplier: 2.0,
    baseTime: 60,
  },
  deuterium_tank: {
    baseCost: { metal: 1000, crystal: 1000, deuterium: 0 },
    costMultiplier: 2.0,
    baseTime: 60,
  },
  research_lab: {
    baseCost: { metal: 200, crystal: 400, deuterium: 200 },
    costMultiplier: 2.0,
    baseTime: 120,
  },
  alliance_depot: {
    baseCost: { metal: 20000, crystal: 40000, deuterium: 0 },
    costMultiplier: 2.0,
    baseTime: 240,
  },
  missile_silo: {
    baseCost: { metal: 20000, crystal: 20000, deuterium: 1000 },
    costMultiplier: 2.0,
    baseTime: 240,
    requirements: {
      buildings: { shipyard: 1 },
    },
  },
  lunar_base: {
    baseCost: { metal: 20000, crystal: 40000, deuterium: 0 },
    costMultiplier: 2.0,
    baseTime: 240,
  },
  sensor_phalanx: {
    baseCost: { metal: 20000, crystal: 40000, deuterium: 20000 },
    costMultiplier: 2.0,
    baseTime: 300,
    requirements: {
      buildings: { lunar_base: 1 },
      research: { computer_technology: 8 },
    },
  },
  jump_gate: {
    baseCost: { metal: 2000000, crystal: 4000000, deuterium: 2000000 },
    costMultiplier: 2.0,
    baseTime: 600,
    requirements: {
      buildings: { lunar_base: 1, moon_shipyard: 1 },
      research: { hyperspace_technology: 7 },
    },
  },
  moon_shipyard: {
    baseCost: { metal: 200, crystal: 400, deuterium: 200 },
    costMultiplier: 2.0,
    baseTime: 120,
    requirements: {
      buildings: { lunar_base: 1 },
    },
  },
  moon_robotics_factory: {
    baseCost: { metal: 400, crystal: 120, deuterium: 200 },
    costMultiplier: 2.0,
    baseTime: 120,
    requirements: {
      buildings: { lunar_base: 1 },
    },
  },
  moon_nanite_factory: {
    baseCost: { metal: 1000000, crystal: 500000, deuterium: 100000 },
    costMultiplier: 2.0,
    baseTime: 600,
    requirements: {
      buildings: { moon_robotics_factory: 10 },
    },
  },
};

/**
 * SHIPS
 *
 * Mapping of ship keys to `ShipConfig`. These entries power construction,
 * movement and combat simulation systems.
 *
 * @constant {Object.<string, ShipConfig>}
 */
export const SHIPS: { [key: string]: ShipConfig } = {
  small_cargo: {
    cost: { metal: 2000, crystal: 2000, deuterium: 0 },
    structurePoints: 4000,
    shieldPower: 10,
    weaponPower: 5,
    cargo: 5000,
    baseSpeed: 5000,
    fuelConsumption: 10,
    buildTime: 30,
  },
  large_cargo: {
    cost: { metal: 6000, crystal: 6000, deuterium: 0 },
    structurePoints: 12000,
    shieldPower: 25,
    weaponPower: 5,
    cargo: 25000,
    baseSpeed: 7500,
    fuelConsumption: 50,
    buildTime: 60,
  },
  light_fighter: {
    cost: { metal: 3000, crystal: 1000, deuterium: 0 },
    structurePoints: 4000,
    shieldPower: 10,
    weaponPower: 50,
    cargo: 50,
    baseSpeed: 12500,
    fuelConsumption: 20,
    buildTime: 45,
    rapidFire: { espionage_probe: 5, solar_satellite: 5 },
  },
  heavy_fighter: {
    cost: { metal: 6000, crystal: 4000, deuterium: 0 },
    structurePoints: 10000,
    shieldPower: 25,
    weaponPower: 150,
    cargo: 100,
    baseSpeed: 10000,
    fuelConsumption: 75,
    buildTime: 90,
    rapidFire: { small_cargo: 3, espionage_probe: 5, solar_satellite: 5 },
  },
  cruiser: {
    cost: { metal: 20000, crystal: 7000, deuterium: 2000 },
    structurePoints: 27000,
    shieldPower: 50,
    weaponPower: 400,
    cargo: 800,
    baseSpeed: 15000,
    fuelConsumption: 300,
    buildTime: 180,
    rapidFire: { light_fighter: 6, espionage_probe: 5, solar_satellite: 5, rocket_launcher: 10 },
  },
  battleship: {
    cost: { metal: 45000, crystal: 15000, deuterium: 0 },
    structurePoints: 60000,
    shieldPower: 200,
    weaponPower: 1000,
    cargo: 1500,
    baseSpeed: 10000,
    fuelConsumption: 500,
    buildTime: 300,
    rapidFire: { espionage_probe: 5, solar_satellite: 5 },
  },
  colony_ship: {
    cost: { metal: 10000, crystal: 20000, deuterium: 10000 },
    structurePoints: 30000,
    shieldPower: 100,
    weaponPower: 50,
    cargo: 7500,
    baseSpeed: 2500,
    fuelConsumption: 1000,
    buildTime: 600,
  },
  recycler: {
    cost: { metal: 10000, crystal: 6000, deuterium: 2000 },
    structurePoints: 16000,
    shieldPower: 10,
    weaponPower: 1,
    cargo: 20000,
    baseSpeed: 2000,
    fuelConsumption: 300,
    buildTime: 120,
  },
  espionage_probe: {
    cost: { metal: 0, crystal: 1000, deuterium: 0 },
    structurePoints: 1000,
    shieldPower: 0.01,
    weaponPower: 0.01,
    cargo: 0,
    baseSpeed: 100000000,
    fuelConsumption: 1,
    buildTime: 15,
  },
  bomber: {
    cost: { metal: 50000, crystal: 25000, deuterium: 15000 },
    structurePoints: 75000,
    shieldPower: 500,
    weaponPower: 1000,
    cargo: 500,
    baseSpeed: 4000,
    fuelConsumption: 700,
    buildTime: 420,
    rapidFire: {
      espionage_probe: 5,
      solar_satellite: 5,
      rocket_launcher: 20,
      light_laser: 20,
      heavy_laser: 10,
      ion_cannon: 10,
    },
  },
  destroyer: {
    cost: { metal: 60000, crystal: 50000, deuterium: 15000 },
    structurePoints: 110000,
    shieldPower: 500,
    weaponPower: 2000,
    cargo: 2000,
    baseSpeed: 5000,
    fuelConsumption: 1000,
    buildTime: 480,
    rapidFire: { espionage_probe: 5, light_laser: 10 },
  },
  deathstar: {
    cost: { metal: 5000000, crystal: 4000000, deuterium: 1000000 },
    structurePoints: 9000000,
    shieldPower: 50000,
    weaponPower: 200000,
    cargo: 1000000,
    baseSpeed: 100,
    fuelConsumption: 1,
    buildTime: 18000,
    rapidFire: {
      small_cargo: 250,
      large_cargo: 250,
      light_fighter: 200,
      heavy_fighter: 100,
      cruiser: 33,
      battleship: 30,
      colony_ship: 250,
      recycler: 250,
      espionage_probe: 1250,
      bomber: 25,
      destroyer: 5,
      rocket_launcher: 200,
      light_laser: 200,
      heavy_laser: 100,
      gauss_cannon: 50,
      ion_cannon: 100,
    },
  },
};

/**
 * DEFENSES
 *
 * Mapping of defensive structures to their `ShipConfig`-like definitions.
 * Although called `ShipConfig` for reuse, these entries represent stationary
 * defenses (lasers, turrets, shield domes) and typically have zero speed and
 * cargo.
 *
 * @constant {Object.<string, ShipConfig>}
 */
export const DEFENSES: { [key: string]: ShipConfig } = {
  rocket_launcher: {
    cost: { metal: 2000, crystal: 0, deuterium: 0 },
    structurePoints: 2000,
    shieldPower: 20,
    weaponPower: 80,
    cargo: 0,
    baseSpeed: 0,
    fuelConsumption: 0,
    buildTime: 30,
  },
  light_laser: {
    cost: { metal: 1500, crystal: 500, deuterium: 0 },
    structurePoints: 2000,
    shieldPower: 25,
    weaponPower: 100,
    cargo: 0,
    baseSpeed: 0,
    fuelConsumption: 0,
    buildTime: 30,
  },
  heavy_laser: {
    cost: { metal: 6000, crystal: 2000, deuterium: 0 },
    structurePoints: 8000,
    shieldPower: 100,
    weaponPower: 250,
    cargo: 0,
    baseSpeed: 0,
    fuelConsumption: 0,
    buildTime: 60,
  },
  gauss_cannon: {
    cost: { metal: 20000, crystal: 15000, deuterium: 2000 },
    structurePoints: 35000,
    shieldPower: 200,
    weaponPower: 1100,
    cargo: 0,
    baseSpeed: 0,
    fuelConsumption: 0,
    buildTime: 180,
  },
  ion_cannon: {
    cost: { metal: 2000, crystal: 6000, deuterium: 0 },
    structurePoints: 8000,
    shieldPower: 500,
    weaponPower: 150,
    cargo: 0,
    baseSpeed: 0,
    fuelConsumption: 0,
    buildTime: 90,
  },
  plasma_turret: {
    cost: { metal: 50000, crystal: 50000, deuterium: 30000 },
    structurePoints: 100000,
    shieldPower: 300,
    weaponPower: 3000,
    cargo: 0,
    baseSpeed: 0,
    fuelConsumption: 0,
    buildTime: 480,
  },
  small_shield_dome: {
    cost: { metal: 10000, crystal: 10000, deuterium: 0 },
    structurePoints: 20000,
    shieldPower: 2000,
    weaponPower: 1,
    cargo: 0,
    baseSpeed: 0,
    fuelConsumption: 0,
    buildTime: 240,
  },
  large_shield_dome: {
    cost: { metal: 50000, crystal: 50000, deuterium: 0 },
    structurePoints: 100000,
    shieldPower: 10000,
    weaponPower: 1,
    cargo: 0,
    baseSpeed: 0,
    fuelConsumption: 0,
    buildTime: 480,
  },
};

/**
 * RESEARCH
 *
 * Mapping of research keys to `ResearchConfig` containing cost, time,
 * display metadata and prerequisites.
 *
 * @constant {Object.<string, ResearchConfig>}
 */
export const RESEARCH: { [key: string]: ResearchConfig } = {
  energy_technology: {
    displayName: 'Energy Technology',
    description: 'Boosts planetary energy output and unlocks advanced tech tiers.',
    category: 'energy',
    baseCost: { metal: 0, crystal: 800, deuterium: 400 },
    costMultiplier: 2.0,
    baseTime: 120,
    requirements: { buildings: { research_lab: 1 } },
  },
  laser_technology: {
    displayName: 'Laser Technology',
    description: 'Foundation for laser weaponry and advanced ship systems.',
    category: 'weapons',
    baseCost: { metal: 200, crystal: 100, deuterium: 0 },
    costMultiplier: 2.0,
    baseTime: 120,
    requirements: {
      buildings: { research_lab: 1 },
      research: { energy_technology: 2 },
    },
  },
  ion_technology: {
    displayName: 'Ion Technology',
    description: 'Enables ion cannons that pierce shields with focused energy.',
    category: 'weapons',
    baseCost: { metal: 1000, crystal: 300, deuterium: 100 },
    costMultiplier: 2.0,
    baseTime: 180,
    requirements: {
      buildings: { research_lab: 4 },
      research: { energy_technology: 4, laser_technology: 5 },
    },
  },
  hyperspace_technology: {
    displayName: 'Hyperspace Technology',
    description: 'Research into hyperspace travel and energy manipulation.',
    category: 'propulsion',
    baseCost: { metal: 0, crystal: 4000, deuterium: 2000 },
    costMultiplier: 2.0,
    baseTime: 240,
    requirements: {
      buildings: { research_lab: 7 },
      research: { energy_technology: 5, shielding_technology: 5 },
    },
  },
  plasma_technology: {
    displayName: 'Plasma Technology',
    description: 'Unlocks devastating plasma weapon systems.',
    category: 'weapons',
    baseCost: { metal: 2000, crystal: 4000, deuterium: 1000 },
    costMultiplier: 2.0,
    baseTime: 300,
    requirements: {
      buildings: { research_lab: 4 },
      research: { energy_technology: 8, laser_technology: 10, ion_technology: 5 },
    },
  },
  combustion_drive: {
    displayName: 'Combustion Drive',
    description: 'Basic propulsion system for light civilian and combat ships.',
    category: 'propulsion',
    baseCost: { metal: 400, crystal: 0, deuterium: 600 },
    costMultiplier: 2.0,
    baseTime: 120,
    requirements: {
      buildings: { research_lab: 1 },
      research: { energy_technology: 1 },
    },
  },
  impulse_drive: {
    displayName: 'Impulse Drive',
    description: 'Advanced drive core for medium-class vessels.',
    category: 'propulsion',
    baseCost: { metal: 2000, crystal: 4000, deuterium: 600 },
    costMultiplier: 2.0,
    baseTime: 180,
    requirements: {
      buildings: { research_lab: 2 },
      research: { energy_technology: 1 },
    },
  },
  hyperspace_drive: {
    displayName: 'Hyperspace Drive',
    description: 'Fastest propulsion for capital ships using hyperspace tunnels.',
    category: 'propulsion',
    baseCost: { metal: 10000, crystal: 20000, deuterium: 6000 },
    costMultiplier: 2.0,
    baseTime: 240,
    requirements: {
      buildings: { research_lab: 7 },
      research: { hyperspace_technology: 3 },
    },
  },
  espionage_technology: {
    displayName: 'Espionage Technology',
    description: 'Improves intel gathering and sensor efficiency.',
    category: 'intelligence',
    baseCost: { metal: 200, crystal: 1000, deuterium: 200 },
    costMultiplier: 2.0,
    baseTime: 120,
    requirements: { buildings: { research_lab: 3 } },
  },
  computer_technology: {
    displayName: 'Computer Technology',
    description: 'Expands command-and-control capacity for fleets.',
    category: 'infrastructure',
    baseCost: { metal: 0, crystal: 400, deuterium: 600 },
    costMultiplier: 2.0,
    baseTime: 120,
    requirements: { buildings: { research_lab: 1 } },
  },
  astrophysics: {
    displayName: 'Astrophysics',
    description: 'Essential for deep-space exploration and additional colonies.',
    category: 'advanced',
    baseCost: { metal: 4000, crystal: 8000, deuterium: 4000 },
    costMultiplier: 1.75,
    baseTime: 240,
    requirements: {
      buildings: { research_lab: 3 },
      research: { espionage_technology: 4, impulse_drive: 3 },
    },
  },
  intergalactic_research_network: {
    displayName: 'Intergalactic Research Network',
    description: 'Links laboratories across galaxies for collaborative research.',
    category: 'advanced',
    baseCost: { metal: 240000, crystal: 400000, deuterium: 160000 },
    costMultiplier: 2.0,
    baseTime: 480,
    requirements: {
      buildings: { research_lab: 10 },
      research: { computer_technology: 8, hyperspace_technology: 8 },
    },
  },
  graviton_technology: {
    displayName: 'Graviton Technology',
    description: 'Harnesses gravity manipulation for superweapons.',
    category: 'advanced',
    baseCost: { metal: 0, crystal: 0, deuterium: 0, energy: 300000 },
    costMultiplier: 3.0,
    baseTime: 4320,
    requirements: { buildings: { research_lab: 12 } },
  },
  weapons_technology: {
    displayName: 'Weapons Technology',
    description: 'Increases weapon damage for all ships and defenses.',
    category: 'weapons',
    baseCost: { metal: 800, crystal: 200, deuterium: 0 },
    costMultiplier: 2.0,
    baseTime: 120,
    requirements: { buildings: { research_lab: 4 } },
  },
  shielding_technology: {
    displayName: 'Shielding Technology',
    description: 'Enhances defensive shield generators.',
    category: 'defense',
    baseCost: { metal: 200, crystal: 600, deuterium: 0 },
    costMultiplier: 2.0,
    baseTime: 120,
    requirements: {
      buildings: { research_lab: 6 },
      research: { energy_technology: 3 },
    },
  },
  armor_technology: {
    displayName: 'Armor Technology',
    description: 'Strengthens hull plating for all units.',
    category: 'defense',
    baseCost: { metal: 1000, crystal: 0, deuterium: 0 },
    costMultiplier: 2.0,
    baseTime: 120,
    requirements: { buildings: { research_lab: 2 } },
  },
};

/**
 * calculateBuildingCost
 *
 * Calculate the resource cost for upgrading a building from level N to
 * level N+1 using the building's configured `costMultiplier`.
 *
 * @param {string} buildingType - Key of the building in `BUILDINGS` (e.g. 'metal_mine')
 * @param {number} currentLevel - Current level of the building (0-based)
 * @returns {BuildingCost} Scaled cost for the next level
 * @throws Will throw if `buildingType` is not defined in `BUILDINGS`.
 *
 * Notes:
 * - Uses Math.floor to keep resource integers.
 */
export function calculateBuildingCost(
  buildingType: string,
  currentLevel: number
): BuildingCost {
  const config = BUILDINGS[buildingType];
  if (!config) throw new Error(`Unknown building type: ${buildingType}`);

  const factor = Math.pow(config.costMultiplier, currentLevel);
  return {
    metal: Math.floor(config.baseCost.metal * factor),
    crystal: Math.floor(config.baseCost.crystal * factor),
    deuterium: Math.floor(config.baseCost.deuterium * factor),
    energy: config.baseCost.energy
      ? Math.floor(config.baseCost.energy * factor)
      : undefined,
  };
}

/**
 * calculateBuildingTime
 *
 * Compute the build time (in seconds) for the next level of a building.
 * The formula uses the summed metal+crystal cost scaled by the building's
 * `baseTime` and is reduced by automation technologies like robotics and
 * nanite factories.
 *
 * @param {string} buildingType - Building key from `BUILDINGS`
 * @param {number} currentLevel - Current level to calculate next-level time for
 * @param {number} [roboticsLevel=0] - Robotics factory level: reduces time linearly
 * @param {number} [naniteLevel=0] - Nanite factory level: halves time per level (exponential)
 * @returns {number} Build time in seconds (rounded down)
 */
export function calculateBuildingTime(
  buildingType: string,
  currentLevel: number,
  roboticsLevel: number = 0,
  naniteLevel: number = 0
): number {
  const config = BUILDINGS[buildingType];
  if (!config) throw new Error(`Unknown building type: ${buildingType}`);

  const cost = calculateBuildingCost(buildingType, currentLevel);
  const totalCost = cost.metal + cost.crystal;

  let time = ((totalCost / 2500) * (1 / (1 + roboticsLevel))) * config.baseTime;

  // Nanite factory reduces time
  if (naniteLevel > 0) {
    time = time / Math.pow(2, naniteLevel);
  }

  return Math.floor(time);
}

/**
 * calculateResourceProduction
 *
 * Calculate the per-hour (or per-tick depending on gameSpeed semantics) resource
 * production for a resource-producing building.
 *
 * @param {string} buildingType - Building key from `BUILDINGS`
 * @param {number} level - Building level
 * @param {number} [gameSpeed=1] - Global game speed multiplier
 * @returns {number} Production amount (integer)
 */
export function calculateResourceProduction(
  buildingType: string,
  level: number,
  gameSpeed: number = 1
): number {
  const config = BUILDINGS[buildingType];
  if (!config || !config.baseProduction) return 0;

  const production =
    config.baseProduction * level * Math.pow(config.productionMultiplier!, level);
  return Math.floor(production * gameSpeed);
}

/**
 * calculateResearchCost
 *
 * Compute scaled resource costs for advancing a research topic. Uses the
 * research `costMultiplier` to calculate exponential growth per level.
 *
 * @param {string} researchType - Research key from `RESEARCH`
 * @param {number} currentLevel - Current research level
 * @returns {BuildingCost} Scaled resource costs
 */
export function calculateResearchCost(
  researchType: string,
  currentLevel: number
): BuildingCost {
  const config = RESEARCH[researchType];
  if (!config) throw new Error(`Unknown research type: ${researchType}`);

  const factor = Math.pow(config.costMultiplier, currentLevel);
  return {
    metal: Math.floor(config.baseCost.metal * factor),
    crystal: Math.floor(config.baseCost.crystal * factor),
    deuterium: Math.floor(config.baseCost.deuterium * factor),
    energy: config.baseCost.energy
      ? Math.floor(config.baseCost.energy * factor)
      : undefined,
  };
}

/**
 * calculateStorageCapacity
 *
 * Compute the storage capacity for resource storage structures. Uses an
 * exponential doubling formula based on level.
 *
 * @param {number} level - Storage level
 * @returns {number} Capacity (integer)
 */
export function calculateStorageCapacity(level: number): number {
  const baseCapacity = 10000;
  return Math.floor(baseCapacity * Math.pow(2, level));
}
