const mockHttpComputeEspionageOutcome = jest.fn();
const mockHttpIsConfigured = jest.fn();
const mockGetUserResearch = jest.fn();
const mockSendEspionageReport = jest.fn();

jest.mock('../../src/services/rustHttpHelperClientService', () => ({
  RustHttpHelperClientService: {
    isConfigured: mockHttpIsConfigured,
    computeEspionageOutcome: mockHttpComputeEspionageOutcome,
  },
}));

jest.mock('../../src/services/researchService', () => ({
  ResearchService: {
    getUserResearch: mockGetUserResearch,
  },
}));

jest.mock('../../src/services/messagingService', () => ({
  MessagingService: jest.fn().mockImplementation(() => ({
    sendEspionageReport: mockSendEspionageReport,
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

describe('FleetService.handleEspionageMission', () => {
  const envBackup = {
    NODE_ENV: process.env.NODE_ENV,
    CORE_ENGINE: process.env.CORE_ENGINE,
    CORE_HELPER_TRANSPORT: process.env.CORE_HELPER_TRANSPORT,
  };

  afterEach(() => {
    jest.clearAllMocks();
    process.env.NODE_ENV = envBackup.NODE_ENV;
    process.env.CORE_ENGINE = envBackup.CORE_ENGINE;
    process.env.CORE_HELPER_TRANSPORT = envBackup.CORE_HELPER_TRANSPORT;
  });

  it('uses HTTP helper espionage kernel with deterministic seed when enabled', async () => {
    process.env.NODE_ENV = 'production';
    process.env.CORE_ENGINE = 'rust';
    process.env.CORE_HELPER_TRANSPORT = 'http';
    mockHttpIsConfigured.mockReturnValue(true);

    const fleet = {
      id: 77,
      user_id: 10,
      target_galaxy: 1,
      target_system: 55,
      target_position: 9,
      ships: JSON.stringify({ espionage_probe: 8 }),
      return_time: new Date().toISOString(),
    } as any;

    const targetPlanet = {
      id: 901,
      user_id: 20,
      owner_username: 'Defender',
      galaxy: 1,
      system: 55,
      position: 9,
      name: 'Aster',
      metal: 1000,
      crystal: 800,
      deuterium: 600,
      energy: 50,
    };

    const client = {
      query: jest
        .fn()
        .mockResolvedValueOnce({ rows: [targetPlanet] })
        .mockResolvedValueOnce({ rows: [] }),
    } as any;

    mockGetUserResearch
      .mockResolvedValueOnce({ espionage_technology: 5 })
      .mockResolvedValueOnce({ espionage_technology: 2 });
    mockHttpComputeEspionageOutcome.mockResolvedValue({
      intelLevel: 'full',
      detected: true,
      detectionChance: 0.33,
      detailScore: 9,
      defenseScore: 2,
      engine: 'rust-grpc',
    });

    const result = await (FleetService as any).handleEspionageMission(fleet, client);

    expect(mockHttpComputeEspionageOutcome).toHaveBeenCalledWith({
      probes: 8,
      attackerEspionage: 5,
      defenderEspionage: 2,
      seed: '77:1:55:9:8',
    });
    expect(result).toEqual({
      type: 'espionage',
      success: true,
      detected: true,
      intelLevel: 'full',
      reportSummary: {
        resources: {
          metal: 1000,
          crystal: 800,
          deuterium: 600,
          energy: 50,
        },
        intelLevel: 'full',
        detected: true,
      },
    });
    expect(mockSendEspionageReport).toHaveBeenCalledTimes(1);
    expect(client.query).toHaveBeenLastCalledWith('DELETE FROM fleets WHERE id = $1', [77]);
  });

  it('falls back to local espionage computation when HTTP helper fails', async () => {
    process.env.NODE_ENV = 'production';
    process.env.CORE_ENGINE = 'rust';
    process.env.CORE_HELPER_TRANSPORT = 'http';
    mockHttpIsConfigured.mockReturnValue(true);

    const fleet = {
      id: 42,
      user_id: 100,
      target_galaxy: 2,
      target_system: 200,
      target_position: 11,
      ships: JSON.stringify({ espionage_probe: 3 }),
      return_time: new Date().toISOString(),
    } as any;

    const targetPlanet = {
      id: 777,
      user_id: 300,
      owner_username: 'Guardian',
      galaxy: 2,
      system: 200,
      position: 11,
      name: 'Bastion',
      metal: 500,
      crystal: 400,
      deuterium: 300,
      energy: 10,
    };

    const client = {
      query: jest
        .fn()
        .mockResolvedValueOnce({ rows: [targetPlanet] })
        .mockResolvedValueOnce({ rows: [] }),
    } as any;

    mockGetUserResearch
      .mockResolvedValueOnce({ espionage_technology: 4 })
      .mockResolvedValueOnce({ espionage_technology: 1 });
    mockHttpComputeEspionageOutcome.mockRejectedValue(new Error('helper unavailable'));

    const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => {});
    const randomSpy = jest.spyOn(Math, 'random').mockReturnValue(0.2);

    try {
      const result = await (FleetService as any).handleEspionageMission(fleet, client);
      expect(result.intelLevel).toBe('full');
      expect(result.detected).toBe(true);
      expect(mockHttpComputeEspionageOutcome).toHaveBeenCalledWith({
        probes: 3,
        attackerEspionage: 4,
        defenderEspionage: 1,
        seed: '42:2:200:11:3',
      });
      expect(warnSpy).toHaveBeenCalled();
    } finally {
      randomSpy.mockRestore();
      warnSpy.mockRestore();
    }
  });
});
