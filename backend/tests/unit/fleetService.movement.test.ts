const mockCalculateFleetMovementByTypeNapi = jest.fn();
const mockCalculateFleetMovementNapi = jest.fn();
const mockCalculateFleetMovementRust = jest.fn();

jest.mock('../../src/coreAdapter/rustCoreNapiClient', () => ({
  calculateFleetMovementByTypeNapi: mockCalculateFleetMovementByTypeNapi,
  calculateFleetMovementNapi: mockCalculateFleetMovementNapi,
  computeAttackerPostCombatDistributionNapi: jest.fn(),
  computeCombatReportSummaryNapi: jest.fn(),
  computeHarvestCollectionNapi: jest.fn(),
  computeEspionageOutcomeNapi: jest.fn(),
  computeMissionCargoTransferNapi: jest.fn(),
  isNapiAvailable: jest.fn(() => true),
  resolveDefenseLossesNapi: jest.fn(),
}));

jest.mock('../../src/coreAdapter/rustCoreClient', () => ({
  calculateFleetMovementRust: mockCalculateFleetMovementRust,
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

import { SHIPS } from '../../src/config/gameConfig';
import { FleetService } from '../../src/services/fleetService';

describe('FleetService.calculateFleetMovement', () => {
  const envBackup = {
    NODE_ENV: process.env.NODE_ENV,
    CORE_TRANSPORT: process.env.CORE_TRANSPORT,
  };

  const origin = { galaxy: 1, system: 10, position: 5 };
  const payload = {
    targetGalaxy: 1,
    targetSystem: 11,
    targetPosition: 7,
    ships: { small_cargo: 5, cruiser: 2 },
  };

  beforeEach(() => {
    jest.clearAllMocks();
    process.env.NODE_ENV = 'production';
    process.env.CORE_TRANSPORT = 'napi';
    (FleetService as any).movementCache.clear();
  });

  afterAll(() => {
    process.env.NODE_ENV = envBackup.NODE_ENV;
    process.env.CORE_TRANSPORT = envBackup.CORE_TRANSPORT;
  });

  it('tries by-type kernel first, then fast N-API fallback', async () => {
    mockCalculateFleetMovementByTypeNapi.mockRejectedValue(new Error('by-type unavailable'));
    mockCalculateFleetMovementNapi.mockResolvedValue({
      distance: 2795,
      fleetSpeed: 5000,
      travelTimeSeconds: 2013,
      fuelNeeded: 1200,
      cargoCapacity: 22800,
    });

    const result = await (FleetService as any).calculateFleetMovement(origin, payload);

    expect(result).toEqual({
      fuelNeeded: 1200,
      travelTimeSeconds: 2013,
      cargoCapacity: 22800,
    });
    expect(mockCalculateFleetMovementByTypeNapi).toHaveBeenCalledTimes(1);
    expect(mockCalculateFleetMovementNapi).toHaveBeenCalledTimes(1);
    expect(mockCalculateFleetMovementByTypeNapi.mock.invocationCallOrder[0]).toBeLessThan(
      mockCalculateFleetMovementNapi.mock.invocationCallOrder[0]
    );
    expect(mockCalculateFleetMovementRust).not.toHaveBeenCalled();
  });

  it('falls back to gRPC when by-type and fast N-API paths fail', async () => {
    mockCalculateFleetMovementByTypeNapi.mockRejectedValue(new Error('by-type unavailable'));
    mockCalculateFleetMovementNapi.mockRejectedValue(new Error('fast unavailable'));
    mockCalculateFleetMovementRust.mockResolvedValue({
      distance: 2795,
      fleetSpeed: 5000,
      travelTimeSeconds: 2013,
      fuelNeeded: 1300,
      cargoCapacity: 22700,
    });

    const result = await (FleetService as any).calculateFleetMovement(origin, payload);

    expect(result).toEqual({
      fuelNeeded: 1300,
      travelTimeSeconds: 2013,
      cargoCapacity: 22700,
    });
    expect(mockCalculateFleetMovementByTypeNapi).toHaveBeenCalledTimes(1);
    expect(mockCalculateFleetMovementNapi).toHaveBeenCalledTimes(1);
    expect(mockCalculateFleetMovementRust).toHaveBeenCalledTimes(1);
  });

  it('uses deterministic ship-map cache key for by-type path', async () => {
    const baseSpeedBefore = SHIPS.small_cargo.baseSpeed;
    const fuelBefore = SHIPS.small_cargo.fuelConsumption;
    const cargoBefore = SHIPS.small_cargo.cargo;

    mockCalculateFleetMovementByTypeNapi.mockResolvedValue({
      distance: 2795,
      fleetSpeed: 5000,
      travelTimeSeconds: 2013,
      fuelNeeded: 1300,
      cargoCapacity: 22700,
    });

    try {
      await (FleetService as any).calculateFleetMovement(origin, payload);
      SHIPS.small_cargo.baseSpeed = baseSpeedBefore + 123;
      SHIPS.small_cargo.fuelConsumption = fuelBefore + 7;
      SHIPS.small_cargo.cargo = cargoBefore + 111;
      await (FleetService as any).calculateFleetMovement(origin, payload);
    } finally {
      SHIPS.small_cargo.baseSpeed = baseSpeedBefore;
      SHIPS.small_cargo.fuelConsumption = fuelBefore;
      SHIPS.small_cargo.cargo = cargoBefore;
    }

    expect(mockCalculateFleetMovementByTypeNapi).toHaveBeenCalledTimes(1);
    expect(mockCalculateFleetMovementNapi).not.toHaveBeenCalled();
    expect(mockCalculateFleetMovementRust).not.toHaveBeenCalled();
  });
});
