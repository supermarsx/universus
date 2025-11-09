// Manual Jest mock for i18next used in tests

const shipNames: Record<string, [string, string]> = {
  small_cargo: ['Small Cargo', 'Navio de Carga Pequeno'],
  large_cargo: ['Large Cargo', 'Navio de Carga Grande'],
  light_fighter: ['Light Fighter', 'Caçador Leve'],
  heavy_fighter: ['Heavy Fighter', 'Caçador Pesado'],
  cruiser: ['Cruiser', 'Cruzador'],
  battleship: ['Battleship', 'Navio de Batalha'],
  colony_ship: ['Colony Ship', 'Navio Colônia'],
  recycler: ['Recycler', 'Reciclador'],
  espionage_probe: ['Espionage Probe', 'Sonda de Espionagem'],
  bomber: ['Bomber', 'Bombardeiro'],
  destroyer: ['Destroyer', 'Destróier'],
  deathstar: ['Deathstar', 'Estrela da Morte'],
};

const defenseNames: Record<string, [string, string]> = {
  rocket_launcher: ['Rocket Launcher', 'Lançador de Foguetes'],
  light_laser: ['Light Laser', 'Laser Leve'],
  heavy_laser: ['Heavy Laser', 'Laser Pesado'],
  gauss_cannon: ['Gauss Cannon', 'Canhão Gauss'],
  ion_cannon: ['Ion Cannon', 'Canhão Íon'],
  plasma_turret: ['Plasma Turret', 'Torreta de Plasma'],
};

const translations: Record<string, string> = {
  'shipyard.shipyardLabel': 'Shipyard',
  'shipyard.noMoon': 'No moon present',
  'shipyard.buildMoonShipyard': 'Build a moon shipyard',
  'shipyard.moonShipyardLevel': 'Moon shipyard level {{level}}',
  'shipyard.planetaryShipyardLevel': 'Planet shipyard level {{level}}',
  'shipyard.selectProductionLocation': 'Select a production location',
  'shipyard.buildShipyard': 'Build a shipyard',
  'shipyard.buildShipyardForDefense': 'Build a shipyard to produce defenses',
  'shipyard.buildMoonShipyardForDefense': 'Build a moon shipyard to produce defenses',
  'shipyard.inHangar': '{{count}} in hangar',
  'shipyard.deployed': '{{count}} deployed',
  'shipyard.build': 'Build',
  'shipyard.insufficientResources': 'Insufficient resources',
  'shipyard.notificationTitle': 'Shipyard',
  'shipyard.productionStarted': 'Production started',
  'shipyard.failedToStartProduction': 'Failed to start production',
  'shipyard.productionCancelled': 'Production cancelled',
  'shipyard.failedToCancelProduction': 'Failed to cancel production',
  'shipyard.shipProductionQueue': 'Ship Production Queue',
  'shipyard.moonProductionQueue': 'Moon Production Queue',
  'shipyard.eta': 'ETA',
  'shipyard.cancel': 'Cancel',
  'shipyard.cancelConfirm': 'Are you sure you want to cancel?',
};

function t(key: string, opts?: Record<string, any>): string {
  if (key.startsWith('shipyard.ships.')) {
    const parts = key.split('.');
    const shipKey = parts[2];
    const field = parts[3];
    const name = shipNames[shipKey];
    if (name) {
      if (field === 'name') return name[0];
      if (field === 'description') return `${name[0]} description`;
    }
    return key;
  }

  if (key.startsWith('shipyard.defense.')) {
    const parts = key.split('.');
    const defKey = parts[2];
    const field = parts[3];
    const name = defenseNames[defKey];
    if (name) {
      if (field === 'name') return name[0];
      if (field === 'description') return `${name[0]} description`;
    }
    return key;
  }

  const val = translations[key];
  if (!val) return key;

  if (opts) return val.replace(/{{(\w+)}}/g, (_, k) => String(opts[k]));
  return val;
}

const i18next = { t };

export default i18next;
module.exports = i18next;
