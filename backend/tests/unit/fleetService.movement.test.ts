const mockCalculateFleetMovementRust = jest.fn();

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
    process.env.CORE_TRANSPORT = 'grpc';
    (FleetService as any).movementCache.clear();
  });

  afterAll(() => {
    process.env.NODE_ENV = envBackup.NODE_ENV;
    process.env.CORE_TRANSPORT = envBackup.CORE_TRANSPORT;
  });

  it('uses gRPC movement result and caches repeat requests', async () => {
    mockCalculateFleetMovementRust.mockResolvedValue({
      distance: 2795,
      fleetSpeed: 5000,
      travelTimeSeconds: 2013,
      fuelNeeded: 1300,
      cargoCapacity: 22700,
    });

    const a = await (FleetService as any).calculateFleetMovement(origin, payload);
    const b = await (FleetService as any).calculateFleetMovement(origin, payload);

    expect(a).toEqual({
      fuelNeeded: 1300,
      travelTimeSeconds: 2013,
      cargoCapacity: 22700,
    });
    expect(b).toEqual(a);
    expect(mockCalculateFleetMovementRust).toHaveBeenCalledTimes(1);
  });

  it('falls back to local movement when gRPC fails', async () => {
    mockCalculateFleetMovementRust.mockRejectedValue(new Error('grpc unavailable'));

    const result = await (FleetService as any).calculateFleetMovement(origin, {
      targetGalaxy: 1,
      targetSystem: 10,
      targetPosition: 8,
      ships: { small_cargo: 1 },
    });

    expect(result).toEqual({
      fuelNeeded: 101.5,
      travelTimeSeconds: 731,
      cargoCapacity: 4898.5,
    });
    expect(mockCalculateFleetMovementRust).toHaveBeenCalledTimes(1);
  });
});
