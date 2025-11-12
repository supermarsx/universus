import { CombatService } from '../../src/services/combatService';

jest.mock('../../src/coreAdapter/rustCoreClient', () => ({
  simulateBattleRust: jest.fn(async (battleId: string, playerIds: string[], seed?: string) => {
    return {
      winner: 'attacker',
      rounds: [],
      attackerLosses: { light_fighter: 0 },
      defenderLosses: { light_fighter: 1 },
      loot: { metal: 100, crystal: 50, deuterium: 0 },
      debris: { metal: 10, crystal: 5 },
    };
  }),
}));

describe('CombatService - Rust adapter toggle', () => {
  const OLD = process.env.CORE_ENGINE;
  beforeAll(() => { process.env.CORE_ENGINE = 'rust'; });
  afterAll(() => { process.env.CORE_ENGINE = OLD; });

  it('delegates to rust client and returns result', async () => {
    const result = await CombatService.simulateBattle({ light_fighter: 1 }, {}, {}, {}, {}, { metal: 0, crystal: 0, deuterium: 0 }, 1, 1234);
    expect(result.winner).toBe('attacker');
    expect(result.loot.metal).toBe(100);
  });
});
