jest.setTimeout(10000);

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

jest.mock('../../src/config/gameConfig', () => ({
  SHIPS: {
    light_fighter: { shieldPower: 10, structurePoints: 100, weaponPower: 50, cargo: 100, rapidFire: {}, cost: { metal: 3000, crystal: 1000 } },
    cruiser: { shieldPower: 50, structurePoints: 400, weaponPower: 200, cargo: 800, rapidFire: {}, cost: { metal: 20000, crystal: 7000 } },
  },
  DEFENSES: {},
}));

describe('CombatService deterministic simulation', () => {
  test('simulateBattle is deterministic given the same seed', async () => {
    // Require after mocks to ensure module-level imports pick up the mocks
    const { CombatService } = require('../../src/services/combatService');

    const attacker = { light_fighter: 10, cruiser: 2 };
    const defender = { light_fighter: 8, cruiser: 1 };
    const defenses = {};
    const attackerTech = { weapons_technology: 0, shielding_technology: 0, armor_technology: 0 };
    const defenderTech = { weapons_technology: 0, shielding_technology: 0, armor_technology: 0 };
    const planetResources = { metal: 10000, crystal: 5000, deuterium: 1000 };

    const seed = 42;

    const resultA = await CombatService.simulateBattle(
      attacker,
      defender,
      defenses,
      attackerTech,
      defenderTech,
      planetResources,
      /* planetId */ undefined,
      seed
    );

    const resultB = await CombatService.simulateBattle(
      attacker,
      defender,
      defenses,
      attackerTech,
      defenderTech,
      planetResources,
      /* planetId */ undefined,
      seed
    );

    expect(resultA.winner).toEqual(resultB.winner);
    expect(resultA.attackerLosses).toEqual(resultB.attackerLosses);
    expect(resultA.defenderLosses).toEqual(resultB.defenderLosses);
    expect(resultA.debris).toEqual(resultB.debris);
    expect(resultA.loot).toEqual(resultB.loot);
    expect(resultA.rounds.length).toEqual(resultB.rounds.length);
  });
});
