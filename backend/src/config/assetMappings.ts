/**
 * @module backend/config/assetMappings
 *
 * Centralized mapping between game-visible entity names (ships, buildings,
 * resources, and backgrounds) and their corresponding asset filenames. Helpers
 * in this module return full, client-ready asset paths so callers don't need
 * to know the on-disk layout or file extensions.
 *
 * Guidelines:
 * - Keys in mapping objects are the human readable names used across the game
 *   and UI (e.g. 'Light Fighter', 'Metal Mine'). Values are canonical
 *   filenames without file extensions (e.g. 'fighter-interceptor').
 * - Helper functions provide safe defaults to avoid missing-image UI states.
 * - Random background functions use Math.random() (non-deterministic).
 */

/**
 * AssetMapping
 *
 * A simple dictionary mapping a human-readable game/entity name to the
 * canonical asset filename (without extension).
 *
 * Example:
 *   const mapping: AssetMapping = { 'Light Fighter': 'fighter-interceptor' };
 *
 * @typedef {Object.<string, string>} AssetMapping
 */
export interface AssetMapping {
  /** Human-readable entity name -> canonical asset filename (no extension) */
  [key: string]: string;
}

/**
 * shipAssets
 *
 * Map of ship types (human readable) to their canonical asset filenames
 * (without extensions). These values are intended to be stable identifiers
 * for client-side image files located under `/assets/ships/`.
 *
 * Read-only semantics are assumed at runtime; mutate only if you know what
 * you're doing.
 *
 * @constant {AssetMapping}
 */
export const shipAssets: AssetMapping = {
  // Fighters
  'Light Fighter': 'fighter-interceptor',
  'Heavy Fighter': 'fighter-assault',
  'Scout': 'fighter-scout',
  
  // Cruisers
  'Cruiser': 'cruiser-medium',
  'Fast Cruiser': 'cruiser-fast',
  'Heavy Cruiser': 'cruiser-heavy',
  
  // Battleships
  'Battleship': 'battleship-dreadnought',
  'Siege Battleship': 'battleship-siege',
  
  // Carriers
  'Strike Carrier': 'carrier-strike',
  'Support Carrier': 'carrier-support',
  'Fleet Carrier': 'carrier-fleet',
  
  // Destroyers
  'Destroyer': 'destroyer-1',
  'Missile Destroyer': 'destroyer-missile',
  'Heavy Destroyer': 'destroyer-heavy',
  'Stealth Destroyer': 'destroyer-stealth',
  
  // Corvettes
  'Fast Attack Corvette': 'corvette-fast-attack',
  'Patrol Corvette': 'corvette-patrol',
  
  // Frigates
  'Escort Frigate': 'frigate-escort',
  'Missile Frigate': 'frigate-missile',
  
  // Dreadnoughts
  'Titan Dreadnought': 'dreadnought-titan',
  'Super Dreadnought': 'dreadnought-super',
  
  // Stealth
  'Stealth Infiltrator': 'stealth-infiltrator',
  'Ghost Ship': 'stealth-ghost',
  
  // Mining
  'Industrial Miner': 'miner-industrial',
  'Deep Space Harvester': 'miner-deep-space',
  
  // Research
  'Scientific Explorer': 'research-explorer',
  'Quantum Lab Ship': 'research-quantum',
  
  // Support
  'Cargo Freighter': 'support-cargo-freighter',
  'Colony Ship': 'support-colony-ship',
  'Medical Frigate': 'support-medical',
  'Shield Generator Ship': 'support-shield-generator',
};

/**
 * buildingAssets
 *
 * Map of building types (human readable) to canonical asset filenames
 * (without extensions). These correspond to files under
 * `/assets/buildings/` on the client.
 *
 * @constant {AssetMapping}
 */
export const buildingAssets: AssetMapping = {
  // Production
  'Metal Mine': 'metal-mine-1',
  'Crystal Mine': 'crystal-mine-1',
  'Deuterium Synthesizer': 'deuterium-plant',
  'Solar Plant': 'solar-plant',
  
  // Energy
  'Fusion Reactor': 'fusion-reactor-1',
  'Antimatter Plant': 'antimatter-plant',
  'Geothermal Plant': 'geothermal-plant',
  
  // Research
  'Research Lab': 'research-lab-basic',
  'Advanced Research Lab': 'research-lab-advanced',
  'AI Research Lab': 'research-ai-lab',
  'Graviton Research Facility': 'research-graviton',
  
  // Military
  'Shipyard': 'shipyard-1',
  'Defense Turret': 'defense-turret',
  'Missile Battery': 'missile-battery',
  'Missile Silo': 'missile-silo',
  'Plasma Turret': 'plasma-turret',
  'Ion Cannon': 'ion-cannon',
  'Shield Dome': 'shield-dome',
  
  // Special
  'Robotics Factory': 'robotics-factory',
  'Nanite Factory': 'nanite-factory',
  'Terraformer': 'terraformer',
  'Alliance Depot': 'alliance-depot',
  'Hydroponic Farm': 'hydroponic-farm',
  'Space Elevator': 'space-elevator',
  'Command Center': 'command-center',
  'Sensor Array': 'sensor-array',
  'Jump Gate': 'jump-gate',
  'Cloning Facility': 'cloning-facility',
  'Quantum Computer': 'quantum-computer',
  'Recycling Plant': 'recycling-plant',
  'Hangar Bay': 'hangar-bay',
  'Orbital Shipyard': 'orbital-shipyard',
  'Particle Accelerator': 'particle-accelerator',
  'Spaceport': 'spaceport',
  'Trading Post': 'trading-post',
  'Academy': 'academy',
  'Prison': 'prison',
  'Monument': 'monument',
  'Weather Control': 'weather-control',
  'Dark Matter Facility': 'dark-matter-facility',
};

/**
 * planetBackgrounds
 *
 * Array of available planet background base filenames (no extensions). Use
 * with `getRandomPlanetBackground()` to obtain a full path to a PNG.
 *
 * Example:
 *   planetBackgrounds.includes('planet-earth-like') === true
 *
 * @constant {string[]}
 */
export const planetBackgrounds: string[] = [
  'planet-earth-like',
  'planet-desert',
  'planet-rocky',
  'planet-tropical',
  'planet-arctic',
  'planet-volcanic',
  'planet-ocean',
  'planet-mountains',
  'planet-canyon',
  'planet-jungle',
  'planet-gas-jupiter',
  'planet-gas-saturn',
  'planet-gas-blue',
  'planet-gas-purple',
  'planet-ice-frozen',
  'planet-ice-blue',
  'planet-desert-red',
  'planet-lava-volcanic',
  'planet-metal-metallic',
];

/**
 * spaceBackgrounds
 *
 * Available deep-space / starfield background base filenames. Helpers will
 * append path and extension.
 *
 * @constant {string[]}
 */
export const spaceBackgrounds: string[] = [
  'deep-space-1',
  'deep-space-2',
  'deep-space-3',
  'starfield-blue',
  'starfield-purple',
];

/**
 * environmentBackgrounds
 *
 * Environment-themed backgrounds (asteroid fields, nebulae, wormholes, etc.).
 * Values are base filenames without extensions.
 *
 * @constant {string[]}
 */
export const environmentBackgrounds: string[] = [
  'asteroid-field',
  'nebula-red',
  'nebula-blue',
  'star-cluster',
  'black-hole',
  'wormhole',
  'supernova-remnant',
];

/**
 * getShipAsset
 *
 * Return the full client path to a ship asset for the provided ship type.
 *
 * @param {string} shipType - Human-readable ship type (case-sensitive, e.g. 'Light Fighter').
 * @returns {string} Full path to the PNG asset (e.g. '/assets/ships/fighter-interceptor.png').
 *
 * Behavior:
 * - If `shipType` is not found in `shipAssets`, returns a safe default
 *   ('fighter-interceptor') to avoid a missing-image situation on the UI.
 * - The function never returns null/undefined.
 *
 * @example
 *   getShipAsset('Scout') // '/assets/ships/fighter-scout.png'
 */
export function getShipAsset(shipType: string): string {
  const assetName = shipAssets[shipType] || 'fighter-interceptor';
  return `/assets/ships/${assetName}.png`;
}

/**
 * getBuildingAsset
 *
 * Return the full client path to a building asset for the provided building
 * type.
 *
 * @param {string} buildingType - Human-readable building name (e.g. 'Metal Mine').
 * @returns {string} Full path to the PNG asset (e.g. '/assets/buildings/metal-mine-1.png').
 *
 * Notes:
 * - Unknown building types fall back to 'metal-mine-1'.
 *
 * @example
 *   getBuildingAsset('Solar Plant') // '/assets/buildings/solar-plant.png'
 */
export function getBuildingAsset(buildingType: string): string {
  const assetName = buildingAssets[buildingType] || 'metal-mine-1';
  return `/assets/buildings/${assetName}.png`;
}

/**
 * getRandomPlanetBackground
 *
 * Select a random planet background from `planetBackgrounds` and return the
 * full asset path.
 *
 * @returns {string} Path to a randomly chosen planet background PNG.
 *
 * Notes on determinism:
 * - This uses Math.random() and is therefore non-deterministic. For tests
 *   requiring repeatable output, stub `Math.random` or implement a seeded RNG.
 */
export function getRandomPlanetBackground(): string {
  const planet = planetBackgrounds[Math.floor(Math.random() * planetBackgrounds.length)];
  return `/assets/planets/${planet}.png`;
}

/**
 * getRandomSpaceBackground
 *
 * Select a random space/starfield background and return the full asset path.
 *
 * @returns {string} Full path to a starfield/deep-space background PNG.
 */
export function getRandomSpaceBackground(): string {
  const space = spaceBackgrounds[Math.floor(Math.random() * spaceBackgrounds.length)];
  return `/assets/backgrounds/${space}.png`;
}

/**
 * getRandomEnvironmentBackground
 *
 * Select a random environment-themed background and return the full asset path.
 *
 * @returns {string} Full path to an environment-themed PNG asset.
 */
export function getRandomEnvironmentBackground(): string {
  const env = environmentBackgrounds[Math.floor(Math.random() * environmentBackgrounds.length)];
  return `/assets/environments/${env}.png`;
}

/**
 * getResourceIcon
 *
 * Return the full UI icon path for a given resource type. The lookup is
 * case-insensitive.
 *
 * @param {string} resourceType - Resource key (supported: 'metal', 'crystal', 'deuterium', 'energy').
 * @returns {string} Full path to the resource icon PNG. Unknown types fall back to the metal icon.
 *
 * @example
 *   getResourceIcon('Crystal') // '/assets/ui/resource-crystal.png'
 */
export function getResourceIcon(resourceType: string): string {
  const resourceMap: AssetMapping = {
    'metal': 'resource-metal',
    'crystal': 'resource-crystal',
    'deuterium': 'resource-deuterium',
    'energy': 'resource-energy',
  };
  const iconName = resourceMap[resourceType.toLowerCase()] || 'resource-metal';
  return `/assets/ui/${iconName}.png`;
}
