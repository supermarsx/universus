const mockHttpIsConfigured = jest.fn();
const mockHttpResolveDefenseRebuild = jest.fn();
const mockHttpComputeAttackerDistribution = jest.fn();
const mockHttpComputeEspionageOutcome = jest.fn();
const mockHttpComputeMissionCargoTransfer = jest.fn();
const mockHttpComputeHarvestCollection = jest.fn();

jest.mock('../../src/services/rustHttpHelperClientService', () => ({
  RustHttpHelperClientService: {
    isConfigured: mockHttpIsConfigured,
    resolveDefenseRebuild: mockHttpResolveDefenseRebuild,
    computeAttackerDistribution: mockHttpComputeAttackerDistribution,
    computeEspionageOutcome: mockHttpComputeEspionageOutcome,
    computeMissionCargoTransfer: mockHttpComputeMissionCargoTransfer,
    computeHarvestCollection: mockHttpComputeHarvestCollection,
  },
}));

jest.mock('../../src/services/messagingService', () => ({
  MessagingService: jest.fn().mockImplementation(() => ({
    sendEspionageReport: jest.fn(),
  })),
}));

jest.mock('../../src/services/fleetScheduler', () => ({
  __esModule: true,
  default: {
    registerCallbacks: jest.fn(),
    scheduleArrival: jest.fn().mockResolvedValue(undefined),
    scheduleReturn: jest.fn().mockResolvedValue(undefined),
    unschedule: jest.fn().mockResolvedValue(undefined),
  },
}));

import { FleetService } from '../../src/services/fleetService';

describe('FleetService helper transport selection', () => {
  const envBackup = {
    NODE_ENV: process.env.NODE_ENV,
    CORE_ENGINE: process.env.CORE_ENGINE,
    CORE_HELPER_TRANSPORT: process.env.CORE_HELPER_TRANSPORT,
    RUST_HTTP_HELPER_URL: process.env.RUST_HTTP_HELPER_URL,
  };

  beforeEach(() => {
    jest.clearAllMocks();
    process.env.NODE_ENV = 'production';
    process.env.CORE_ENGINE = 'rust';
    process.env.CORE_HELPER_TRANSPORT = 'http';
    process.env.RUST_HTTP_HELPER_URL = 'http://rust-helper:8080';
    mockHttpIsConfigured.mockReturnValue(true);
  });

  afterAll(() => {
    process.env.NODE_ENV = envBackup.NODE_ENV;
    process.env.CORE_ENGINE = envBackup.CORE_ENGINE;
    process.env.CORE_HELPER_TRANSPORT = envBackup.CORE_HELPER_TRANSPORT;
    process.env.RUST_HTTP_HELPER_URL = envBackup.RUST_HTTP_HELPER_URL;
  });

  it('uses HTTP helper for espionage outcome before local fallback', async () => {
    mockHttpComputeEspionageOutcome.mockResolvedValue({
      intelLevel: 'standard',
      detected: false,
      detectionChance: 0.2,
      detailScore: 3,
      defenseScore: 4,
      engine: 'rust-grpc',
    });

    const result = await (FleetService as any).computeEspionageOutcome(6, 3, 4, 'seed-123');

    expect(result).toEqual({ intelLevel: 'standard', detected: false });
    expect(mockHttpComputeEspionageOutcome).toHaveBeenCalledWith({
      probes: 6,
      attackerEspionage: 3,
      defenderEspionage: 4,
      seed: 'seed-123',
    });
  });

  it('falls back from HTTP helper to local mission cargo transfer', async () => {
    mockHttpComputeMissionCargoTransfer.mockRejectedValue(new Error('gateway timeout'));

    const result = await (FleetService as any).computeMissionCargoTransfer(
      { metal: 100, crystal: 200, deuterium: 300 },
      true
    );

    expect(mockHttpComputeMissionCargoTransfer).toHaveBeenCalledWith({
      metal: 100,
      crystal: 200,
      deuterium: 300,
      clampNonNegative: true,
    });
    expect(result).toEqual({
      transferMetal: 100,
      transferCrystal: 200,
      transferDeuterium: 300,
      remainingMetal: 0,
      remainingCrystal: 0,
      remainingDeuterium: 0,
      totalTransfer: 600,
    });
  });

  it('uses HTTP helper for harvest collection before local fallback', async () => {
    mockHttpComputeHarvestCollection.mockResolvedValue({
      collectedMetal: 300,
      collectedCrystal: 200,
      updatedMetal: 10,
      updatedCrystal: 0,
      recyclerCapacity: 500,
      empty: false,
      engine: 'rust-grpc',
    });

    const result = await (FleetService as any).computeHarvestCollection(310, 200, 1, 500);

    expect(result).toEqual({
      collectedMetal: 300,
      collectedCrystal: 200,
      updatedMetal: 10,
      updatedCrystal: 0,
      recyclerCapacity: 500,
      empty: false,
      engine: 'rust-grpc',
    });
    expect(mockHttpComputeHarvestCollection).toHaveBeenCalledWith({
      debrisMetal: 310,
      debrisCrystal: 200,
      recyclerCount: 1,
      recyclerCargoCapacity: 500,
    });
  });

  it('uses HTTP helper for attacker distribution before local fallback', async () => {
    const participants = [
      {
        fleet: { id: 201, return_time: new Date().toISOString() },
        ships: { light_fighter: 10 },
      },
      {
        fleet: { id: 202, return_time: new Date().toISOString() },
        ships: { light_fighter: 30 },
      },
    ] as any;

    mockHttpComputeAttackerDistribution.mockResolvedValue({
      participants: [
        {
          survivors: { light_fighter: 5 },
          loot: { metal: 30, crystal: 20, deuterium: 10 },
        },
        {
          survivors: { light_fighter: 15 },
          loot: { metal: 70, crystal: 40, deuterium: 30 },
        },
      ],
      engine: 'rust-grpc',
    });

    const client = {
      query: jest.fn().mockResolvedValue({ rows: [] }),
    } as any;

    await (FleetService as any).updateAttackerFleetsAfterCombat(
      participants,
      {
        winner: 'attacker',
        attackerLosses: { light_fighter: 20 },
        loot: { metal: 100, crystal: 60, deuterium: 40 },
      },
      client
    );

    expect(mockHttpComputeAttackerDistribution).toHaveBeenCalledWith({
      participants: [{ light_fighter: 10 }, { light_fighter: 30 }],
      totalLosses: { light_fighter: 20 },
      loot: { metal: 100, crystal: 60, deuterium: 40 },
      winner: 'attacker',
    });
    expect(client.query).toHaveBeenCalledTimes(2);
  });

  it('uses HTTP helper for defense rebuild before local fallback', async () => {
    mockHttpResolveDefenseRebuild.mockResolvedValue({
      updated: { rocket_launcher: 8 },
      engine: 'rust-grpc',
    });

    const client = {
      query: jest.fn().mockResolvedValue({ rows: [] }),
    } as any;

    await (FleetService as any).applyDefenderLosses(
      55,
      { rocket_launcher: 10 },
      {
        defenderLosses: { rocket_launcher: 4 },
        combatId: 99,
      },
      client
    );

    expect(mockHttpResolveDefenseRebuild).toHaveBeenCalledWith({
      current: { rocket_launcher: 10 },
      losses: { rocket_launcher: 4 },
      rebuildRate: 0.7,
      seed: '55:99:defense',
    });
    expect(client.query).toHaveBeenCalledTimes(1);
    expect(client.query).toHaveBeenCalledWith(
      expect.stringContaining('UPDATE planets SET rocket_launcher = $1 WHERE id = $2'),
      [8, 55]
    );
  });

  it('skips HTTP helper when URL is not configured and uses local computation', async () => {
    process.env.RUST_HTTP_HELPER_URL = '';
    mockHttpIsConfigured.mockReturnValue(false);
    const randomSpy = jest.spyOn(Math, 'random').mockReturnValue(0.1);

    try {
      const result = await (FleetService as any).computeEspionageOutcome(9, 5, 1, 'seed-local');
      expect(mockHttpComputeEspionageOutcome).not.toHaveBeenCalled();
      expect(result).toEqual({ intelLevel: 'full', detected: true });
    } finally {
      randomSpy.mockRestore();
    }
  });
});
