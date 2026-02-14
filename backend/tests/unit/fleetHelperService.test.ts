import { FleetHelperService } from '../../src/services/fleetHelperService';
import { calculateFleetMovementRust } from '../../src/coreAdapter/rustCoreClient';

jest.mock('../../src/coreAdapter/rustCoreClient', () => ({
  calculateFleetMovementRust: jest.fn(),
}));

describe('FleetHelperService', () => {
  afterEach(() => {
    jest.clearAllMocks();
  });

  test('uses Rust gRPC movement result when available', async () => {
    (calculateFleetMovementRust as jest.Mock).mockResolvedValue({
      distance: 1100,
      fleetSpeed: 5000,
      travelTimeSeconds: 792,
      fuelNeeded: 110,
      cargoCapacity: 4890,
    });

    const result = await FleetHelperService.calculateMovement({
      origin: { galaxy: 1, system: 1, position: 1 },
      target: { galaxy: 1, system: 1, position: 21 },
      ships: { small_cargo: 10 },
    });

    expect(result.engine).toBe('rust-grpc');
    expect(result.distance).toBe(1100);
    expect(calculateFleetMovementRust).toHaveBeenCalledTimes(1);
  });

  test('falls back to TypeScript movement when Rust gRPC fails', async () => {
    (calculateFleetMovementRust as jest.Mock).mockRejectedValue(new Error('grpc unavailable'));

    const result = await FleetHelperService.calculateMovement({
      origin: { galaxy: 1, system: 1, position: 1 },
      target: { galaxy: 1, system: 1, position: 3 },
      ships: { small_cargo: 1 },
    });

    expect(result.engine).toBe('typescript');
    expect(result.distance).toBe(1010);
    expect(result.fleetSpeed).toBe(5000);
    expect(result.travelTimeSeconds).toBe(728);
    expect(result.fuelNeeded).toBe(101);
    expect(result.cargoCapacity).toBe(4899);
  });

  test('returns deterministic TypeScript defense rebuild with seed', async () => {
    const input = {
      current: { rocket_launcher: 12 },
      losses: { rocket_launcher: 6 },
      rebuildRate: 0.7,
      seed: 'planet-12',
    };

    const a = await FleetHelperService.resolveDefenseRebuild(input);
    const b = await FleetHelperService.resolveDefenseRebuild(input);

    expect(a.engine).toBe('typescript');
    expect(a.updated.rocket_launcher).toBeGreaterThanOrEqual(6);
    expect(a.updated.rocket_launcher).toBeLessThanOrEqual(12);
    expect(a).toEqual(b);
  });

  test('returns TypeScript attacker distribution', async () => {
    const result = await FleetHelperService.computeAttackerDistribution({
      participants: [
        { light_fighter: 10 },
        { light_fighter: 30 },
      ],
      totalLosses: { light_fighter: 20 },
      loot: { metal: 100, crystal: 60, deuterium: 40 },
      winner: 'attacker',
    });

    expect(result.engine).toBe('typescript');
    expect(result.participants).toHaveLength(2);
    expect(result.participants[0].survivors.light_fighter).toBe(5);
    expect(result.participants[1].survivors.light_fighter).toBe(15);
    expect(result.participants[0].loot.metal + result.participants[1].loot.metal).toBe(100);
  });
});
