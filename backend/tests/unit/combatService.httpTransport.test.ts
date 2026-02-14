const mockSimulateCombatHttp = jest.fn();
const mockSimulateBattleRust = jest.fn();

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
    getCombatConfig: jest.fn(async () => ({ maxRounds: 6 })),
  },
}));

jest.mock('../../src/services/rustHttpHelperClientService', () => ({
  RustHttpHelperClientService: {
    simulateCombat: mockSimulateCombatHttp,
  },
}));

jest.mock('../../src/coreAdapter/rustCoreClient', () => ({
  simulateBattleRust: mockSimulateBattleRust,
}));

import { CombatService } from '../../src/services/combatService';

describe('CombatService HTTP transport', () => {
  const envBackup = {
    CORE_ENGINE: process.env.CORE_ENGINE,
    CORE_TRANSPORT: process.env.CORE_TRANSPORT,
    NODE_ENV: process.env.NODE_ENV,
  };

  beforeEach(() => {
    jest.clearAllMocks();
    process.env.NODE_ENV = 'production';
    process.env.CORE_ENGINE = 'rust';
  });

  afterAll(() => {
    process.env.CORE_ENGINE = envBackup.CORE_ENGINE;
    process.env.CORE_TRANSPORT = envBackup.CORE_TRANSPORT;
    process.env.NODE_ENV = envBackup.NODE_ENV;
  });

  it('uses HTTP transport when CORE_TRANSPORT=http', async () => {
    process.env.CORE_TRANSPORT = 'http';
    mockSimulateCombatHttp.mockResolvedValue({
      winner: 'attacker',
      rounds: [],
      attackerLosses: {},
      defenderLosses: {},
      loot: { metal: 10, crystal: 20, deuterium: 30 },
      debris: { metal: 5, crystal: 6 },
    });

    const result = await CombatService.simulateBattle(
      { light_fighter: 1 },
      {},
      {},
      {},
      {},
      { metal: 0, crystal: 0, deuterium: 0 },
      77,
      123
    );

    expect(result.winner).toBe('attacker');
    expect(mockSimulateCombatHttp).toHaveBeenCalledWith(
      expect.objectContaining({
        battle_id: '77',
        max_rounds: 6,
      })
    );
    expect(mockSimulateBattleRust).not.toHaveBeenCalled();
  });

  it('falls back to grpc transport when HTTP fails', async () => {
    process.env.CORE_TRANSPORT = 'http';
    mockSimulateCombatHttp.mockRejectedValue(new Error('http timeout'));
    mockSimulateBattleRust.mockResolvedValue({
      winner: 'defender',
      rounds: [],
      attackerLosses: { light_fighter: 1 },
      defenderLosses: {},
      loot: { metal: 0, crystal: 0, deuterium: 0 },
      debris: { metal: 0, crystal: 0 },
    });

    const result = await CombatService.simulateBattle(
      { light_fighter: 1 },
      {},
      {},
      {},
      {},
      { metal: 0, crystal: 0, deuterium: 0 },
      88,
      321
    );

    expect(result.winner).toBe('defender');
    expect(mockSimulateCombatHttp).toHaveBeenCalledTimes(1);
    expect(mockSimulateBattleRust).toHaveBeenCalledTimes(1);
  });

  it('uses grpc transport for auto mode (does not call HTTP)', async () => {
    process.env.CORE_TRANSPORT = 'auto';
    mockSimulateBattleRust.mockResolvedValue({
      winner: 'attacker',
      rounds: [],
      attackerLosses: {},
      defenderLosses: {},
      loot: { metal: 1, crystal: 2, deuterium: 3 },
      debris: { metal: 0, crystal: 0 },
    });

    const result = await CombatService.simulateBattle(
      { light_fighter: 2 },
      {},
      {},
      {},
      {},
      { metal: 0, crystal: 0, deuterium: 0 },
      99,
      777
    );

    expect(result.winner).toBe('attacker');
    expect(mockSimulateCombatHttp).not.toHaveBeenCalled();
    expect(mockSimulateBattleRust).toHaveBeenCalledTimes(1);
  });
});
