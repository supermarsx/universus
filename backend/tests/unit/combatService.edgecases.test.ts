jest.setTimeout(20000);

// Mock heavy/side-effect modules before importing CombatService
jest.mock('../../src/config/database', () => ({
  pool: {
    query: jest.fn().mockResolvedValue({ rows: [], rowCount: 0 }),
    on: jest.fn(),
  },
}));

jest.mock('../../src/services/millisecondCombatTracker', () => ({
  combatTracker: {
    executeCombatAtArrival: jest.fn().mockResolvedValue(undefined),
    logCombatRound: jest.fn().mockResolvedValue(undefined),
    completeCombat: jest.fn().mockResolvedValue(undefined),
    getCurrentTimeMicros: () => Date.now(),
  },
}));

jest.mock('../../src/services/notificationService', () => ({
  default: {
    notifyUnderAttack: jest.fn(),
    notifyFleetArrived: jest.fn(),
    notifyCombatReport: jest.fn(),
    notifyFleetReturned: jest.fn(),
  },
}));

jest.mock('../../src/socket', () => ({
  getRealtimeHandler: () => null,
}));

jest.mock('../../src/services/gameConfigAdapter', () => ({
  gameConfig: {
    getCombatConfig: async () => ({ maxRounds: 10 }),
  },
}));

describe('CombatService deterministic edge cases', () => {
  test('rapid-fire configuration remains deterministic', async () => {
    jest.resetModules();

    // Light fighters have rapidFire against cruisers
    jest.doMock('../../src/config/gameConfig', () => ({
      SHIPS: {
        light_fighter: { shieldPower: 10, structurePoints: 100, weaponPower: 50, cargo: 100, rapidFire: { cruiser: 3 }, cost: { metal: 3000, crystal: 1000 } },
        cruiser: { shieldPower: 50, structurePoints: 400, weaponPower: 200, cargo: 800, rapidFire: {}, cost: { metal: 20000, crystal: 7000 } },
      },
      DEFENSES: {},
    }));

    const { CombatService } = require('../../src/services/combatService');

    const attacker = { light_fighter: 20, cruiser: 0 };
    const defender = { light_fighter: 0, cruiser: 3 };
    const defenses = {};
    const attackerTech = { weapons_technology: 0, shielding_technology: 0, armor_technology: 0 };
    const defenderTech = { weapons_technology: 0, shielding_technology: 0, armor_technology: 0 };
    const planetResources = { metal: 10000, crystal: 5000, deuterium: 1000 };
    const seed = 12345;

    const a = await CombatService.simulateBattle(attacker, defender, defenses, attackerTech, defenderTech, planetResources, undefined, seed);
    const b = await CombatService.simulateBattle(attacker, defender, defenses, attackerTech, defenderTech, planetResources, undefined, seed);

    expect(a.winner).toEqual(b.winner);
    expect(a.attackerLosses).toEqual(b.attackerLosses);
    expect(a.defenderLosses).toEqual(b.defenderLosses);
    expect(a.debris).toEqual(b.debris);
    expect(a.loot).toEqual(b.loot);
  });

  test('explosion-prone hull values deterministic', async () => {
    jest.resetModules();

    // Make a fragile ship type (high explosion chance when destroyed)
    jest.doMock('../../src/config/gameConfig', () => ({
      SHIPS: {
        glass_ship: { shieldPower: 1, structurePoints: 10, weaponPower: 5, cargo: 10, rapidFire: {}, cost: { metal: 500, crystal: 200 } },
        bomber: { shieldPower: 20, structurePoints: 300, weaponPower: 150, cargo: 500, rapidFire: {}, cost: { metal: 15000, crystal: 5000 } },
      },
      DEFENSES: {},
    }));

    const { CombatService } = require('../../src/services/combatService');

    const attacker = { bomber: 2 };
    const defender = { glass_ship: 30 };
    const defenses = {};
    const attackerTech = { weapons_technology: 2, shielding_technology: 0, armor_technology: 0 };
    const defenderTech = { weapons_technology: 0, shielding_technology: 0, armor_technology: 0 };
    const planetResources = { metal: 5000, crystal: 2000, deuterium: 500 };
    const seed = 999;

    const a = await CombatService.simulateBattle(attacker, defender, defenses, attackerTech, defenderTech, planetResources, undefined, seed);
    const b = await CombatService.simulateBattle(attacker, defender, defenses, attackerTech, defenderTech, planetResources, undefined, seed);

    expect(a.winner).toEqual(b.winner);
    expect(a.attackerLosses).toEqual(b.attackerLosses);
    expect(a.defenderLosses).toEqual(b.defenderLosses);
    expect(a.debris).toEqual(b.debris);
    expect(a.loot).toEqual(b.loot);
  });

  test('shield regeneration and tech modifiers deterministic', async () => {
    jest.resetModules();

    jest.doMock('../../src/config/gameConfig', () => ({
      SHIPS: {
        defender_corvette: { shieldPower: 30, structurePoints: 200, weaponPower: 80, cargo: 200, rapidFire: {}, cost: { metal: 10000, crystal: 3000 } },
        attacker_destroyer: { shieldPower: 20, structurePoints: 250, weaponPower: 120, cargo: 300, rapidFire: {}, cost: { metal: 12000, crystal: 4000 } },
      },
      DEFENSES: {},
    }));

    const { CombatService } = require('../../src/services/combatService');

    const attacker = { attacker_destroyer: 3 };
    const defender = { defender_corvette: 4 };
    const defenses = {};
    const attackerTech = { weapons_technology: 3, shielding_technology: 4, armor_technology: 2 };
    const defenderTech = { weapons_technology: 1, shielding_technology: 5, armor_technology: 3 };
    const planetResources = { metal: 8000, crystal: 3000, deuterium: 1200 };
    const seed = 2025;

    const a = await CombatService.simulateBattle(attacker, defender, defenses, attackerTech, defenderTech, planetResources, undefined, seed);
    const b = await CombatService.simulateBattle(attacker, defender, defenses, attackerTech, defenderTech, planetResources, undefined, seed);

    expect(a.winner).toEqual(b.winner);
    expect(a.attackerLosses).toEqual(b.attackerLosses);
    expect(a.defenderLosses).toEqual(b.defenderLosses);
    expect(a.debris).toEqual(b.debris);
    expect(a.loot).toEqual(b.loot);
  });
});
