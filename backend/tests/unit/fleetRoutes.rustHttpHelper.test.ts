import express from 'express';
import request from 'supertest';
import fleetRoutes from '../../src/routes/fleet';
import { FleetHelperService } from '../../src/services/fleetHelperService';
import { RustHttpHelperClientService } from '../../src/services/rustHttpHelperClientService';

jest.mock('../../src/middleware/auth', () => ({
  authenticateToken: (req: any, _res: any, next: any) => {
    req.user = { id: 1 };
    next();
  },
}));

jest.mock('../../src/services/fleetService', () => ({
  FleetService: {
    getUserFleets: jest.fn(),
    getRecentCombatReports: jest.fn(),
    dispatchFleet: jest.fn(),
    recallFleet: jest.fn(),
    getMissionHistory: jest.fn(),
    cancelFleet: jest.fn(),
  },
}));

jest.mock('../../src/services/allianceLogisticsService', () => ({
  __esModule: true,
  default: {
    cancelDepotSessionByFleet: jest.fn(),
  },
}));

jest.mock('../../src/services/fleetHelperService', () => ({
  FleetHelperService: {
    calculateMovement: jest.fn(),
    resolveDefenseRebuild: jest.fn(),
    computeAttackerDistribution: jest.fn(),
  },
}));

jest.mock('../../src/services/rustHttpHelperClientService', () => ({
  RustHttpHelperClientService: {
    isConfigured: jest.fn(),
    calculateMovement: jest.fn(),
    resolveDefenseRebuild: jest.fn(),
    computeAttackerDistribution: jest.fn(),
  },
}));

describe('fleet helper routes with Rust HTTP helper proxy', () => {
  const app = express();
  app.use(express.json());
  app.use('/api/fleet', fleetRoutes);

  beforeEach(() => {
    jest.clearAllMocks();
  });

  test('uses Rust HTTP helper first for movement when configured', async () => {
    (RustHttpHelperClientService.isConfigured as jest.Mock).mockReturnValue(true);
    (RustHttpHelperClientService.calculateMovement as jest.Mock).mockResolvedValue({
      distance: 1010,
      fleetSpeed: 5000,
      travelTimeSeconds: 728,
      fuelNeeded: 101,
      cargoCapacity: 4899,
      engine: 'typescript',
    });

    const response = await request(app).post('/api/fleet/helpers/movement').send({
      origin: { galaxy: 1, system: 1, position: 1 },
      target: { galaxy: 1, system: 1, position: 3 },
      ships: { small_cargo: 1 },
    });

    expect(response.status).toBe(200);
    expect(response.body.success).toBe(true);
    expect(RustHttpHelperClientService.calculateMovement).toHaveBeenCalledTimes(1);
    expect(FleetHelperService.calculateMovement).not.toHaveBeenCalled();
  });

  test('falls back to FleetHelperService movement when Rust HTTP helper errors', async () => {
    (RustHttpHelperClientService.isConfigured as jest.Mock).mockReturnValue(true);
    (RustHttpHelperClientService.calculateMovement as jest.Mock).mockRejectedValue(new Error('bad gateway'));
    (FleetHelperService.calculateMovement as jest.Mock).mockResolvedValue({
      distance: 1010,
      fleetSpeed: 5000,
      travelTimeSeconds: 728,
      fuelNeeded: 101,
      cargoCapacity: 4899,
      engine: 'typescript',
    });

    const response = await request(app).post('/api/fleet/helpers/movement').send({
      origin: { galaxy: 1, system: 1, position: 1 },
      target: { galaxy: 1, system: 1, position: 3 },
      ships: { small_cargo: 1 },
    });

    expect(response.status).toBe(200);
    expect(response.body.success).toBe(true);
    expect(RustHttpHelperClientService.calculateMovement).toHaveBeenCalledTimes(1);
    expect(FleetHelperService.calculateMovement).toHaveBeenCalledTimes(1);
  });

  test('preserves validation shape for invalid movement requests', async () => {
    (RustHttpHelperClientService.isConfigured as jest.Mock).mockReturnValue(true);

    const response = await request(app).post('/api/fleet/helpers/movement').send({
      origin: { galaxy: 1, system: 1 },
      target: { galaxy: 1, system: 1, position: 3 },
      ships: { small_cargo: 1 },
    });

    expect(response.status).toBe(400);
    expect(response.body).toEqual({ success: false, error: 'Invalid fleet helper movement request' });
    expect(RustHttpHelperClientService.calculateMovement).not.toHaveBeenCalled();
    expect(FleetHelperService.calculateMovement).not.toHaveBeenCalled();
  });

  test('uses fallback for defense rebuild helper when Rust HTTP helper errors', async () => {
    (RustHttpHelperClientService.isConfigured as jest.Mock).mockReturnValue(true);
    (RustHttpHelperClientService.resolveDefenseRebuild as jest.Mock).mockRejectedValue(new Error('timeout'));
    (FleetHelperService.resolveDefenseRebuild as jest.Mock).mockResolvedValue({
      updated: { rocket_launcher: 8 },
      engine: 'typescript',
    });

    const response = await request(app).post('/api/fleet/helpers/combat/defense-rebuild').send({
      current: { rocket_launcher: 10 },
      losses: { rocket_launcher: 4 },
      rebuildRate: 0.7,
      seed: 'abc',
    });

    expect(response.status).toBe(200);
    expect(response.body.success).toBe(true);
    expect(RustHttpHelperClientService.resolveDefenseRebuild).toHaveBeenCalledTimes(1);
    expect(FleetHelperService.resolveDefenseRebuild).toHaveBeenCalledTimes(1);
  });

  test('uses fallback for attacker distribution helper when Rust HTTP helper errors', async () => {
    (RustHttpHelperClientService.isConfigured as jest.Mock).mockReturnValue(true);
    (RustHttpHelperClientService.computeAttackerDistribution as jest.Mock).mockRejectedValue(new Error('timeout'));
    (FleetHelperService.computeAttackerDistribution as jest.Mock).mockResolvedValue({
      participants: [
        { survivors: { light_fighter: 5 }, loot: { metal: 50, crystal: 30, deuterium: 20 } },
        { survivors: { light_fighter: 15 }, loot: { metal: 50, crystal: 30, deuterium: 20 } },
      ],
      engine: 'typescript',
    });

    const response = await request(app).post('/api/fleet/helpers/combat/attacker-distribution').send({
      participants: [{ light_fighter: 10 }, { light_fighter: 30 }],
      totalLosses: { light_fighter: 20 },
      loot: { metal: 100, crystal: 60, deuterium: 40 },
      winner: 'attacker',
    });

    expect(response.status).toBe(200);
    expect(response.body.success).toBe(true);
    expect(RustHttpHelperClientService.computeAttackerDistribution).toHaveBeenCalledTimes(1);
    expect(FleetHelperService.computeAttackerDistribution).toHaveBeenCalledTimes(1);
  });
});
