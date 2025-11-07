// @ts-nocheck
// Ship type to asset mapping
const SHIP_ASSETS = {
    'small_cargo': 'support-cargo-freighter',
    'large_cargo': 'support-cargo-freighter',
    'light_fighter': 'fighter-interceptor',
    'heavy_fighter': 'fighter-assault',
    'cruiser': 'cruiser-medium',
    'battleship': 'battleship-dreadnought',
    'colony_ship': 'support-colony-ship',
    'recycler': 'miner-industrial'
};

// Defense type to asset mapping  
const DEFENSE_ASSETS = {
    'rocket_launcher': 'missile-battery',
    'light_laser': 'defense-turret',
    'heavy_laser': 'defense-turret',
    'gauss_cannon': 'plasma-turret',
    'ion_cannon': 'ion-cannon',
    'plasma_turret': 'plasma-turret'
};

// Helper function to get ship image path
function getShipImage(shipType) {
    const assetName = SHIP_ASSETS[shipType] || 'fighter-interceptor';
    return `/assets/ships/${assetName}.png`;
}

// Helper function to get defense image path
function getDefenseImage(defenseType) {
    const assetName = DEFENSE_ASSETS[defenseType] || 'defense-turret';
    return `/assets/buildings/${assetName}.png`;
}
