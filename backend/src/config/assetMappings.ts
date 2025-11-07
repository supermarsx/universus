/**
 * Asset Mappings for Universus
 * Maps game entities to their visual asset filenames
 */

export interface AssetMapping {
  [key: string]: string;
}

/**
 * Ship type to asset filename mapping
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
 * Building type to asset filename mapping
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
 * Planet types for background images
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
 * Space backgrounds for various pages
 */
export const spaceBackgrounds: string[] = [
  'deep-space-1',
  'deep-space-2',
  'deep-space-3',
  'starfield-blue',
  'starfield-purple',
];

/**
 * Environment backgrounds
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
 * Get ship asset path
 */
export function getShipAsset(shipType: string): string {
  const assetName = shipAssets[shipType] || 'fighter-interceptor';
  return `/assets/ships/${assetName}.png`;
}

/**
 * Get building asset path
 */
export function getBuildingAsset(buildingType: string): string {
  const assetName = buildingAssets[buildingType] || 'metal-mine-1';
  return `/assets/buildings/${assetName}.png`;
}

/**
 * Get random planet background
 */
export function getRandomPlanetBackground(): string {
  const planet = planetBackgrounds[Math.floor(Math.random() * planetBackgrounds.length)];
  return `/assets/planets/${planet}.png`;
}

/**
 * Get random space background
 */
export function getRandomSpaceBackground(): string {
  const space = spaceBackgrounds[Math.floor(Math.random() * spaceBackgrounds.length)];
  return `/assets/backgrounds/${space}.png`;
}

/**
 * Get random environment background
 */
export function getRandomEnvironmentBackground(): string {
  const env = environmentBackgrounds[Math.floor(Math.random() * environmentBackgrounds.length)];
  return `/assets/environments/${env}.png`;
}

/**
 * Get resource icon path
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
