import { CombatService } from '../../src/services/combatService';
import { gameConfig } from '../../src/services/gameConfigAdapter';

const simulateBattleRustMock = jest.fn(async () => {
  return {
    winner: 'attacker',
    rounds: [],
    attackerLosses: { light_fighter: 0 },
    defenderLosses: { light_fighter: 1 },
    loot: { metal: 100, crystal: 50, deuterium: 0 },
    debris: { metal: 10, crystal: 5 },
  };
});

jest.mock('../../src/coreAdapter/rustCoreClient', () => ({
  simulateBattleRust: simulateBattleRustMock,
}));

jest.mock('../../src/services/gameConfigAdapter', () => ({
  gameConfig: {
    getCombatConfig: jest.fn(async () => ({ maxRounds: 7 })),
  },
}));

describe('CombatService - Rust adapter toggle', () => {
  const OLD = process.env.CORE_ENGINE;
  beforeAll(() => { process.env.CORE_ENGINE = 'rust'; });
  afterAll(() => { process.env.CORE_ENGINE = OLD; });
  beforeEach(() => {
    simulateBattleRustMock.mockClear();
  });

  it('delegates to rust client and returns result', async () => {
    const result = await CombatService.simulateBattle({ light_fighter: 1 }, {}, {}, {}, {}, { metal: 0, crystal: 0, deuterium: 0 }, 1, 1234);
    expect(result.winner).toBe('attacker');
    expect(result.loot.metal).toBe(100);
    expect(gameConfig.getCombatConfig).toHaveBeenCalledTimes(1);
    expect(simulateBattleRustMock).toHaveBeenCalledWith(
      expect.objectContaining({
        max_rounds: 7,
      })
    );
  });
});
