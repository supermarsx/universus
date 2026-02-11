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
    computeEspionageOutcome: jest.fn(),
    computeMissionCargoTransfer: jest.fn(),
    computeHarvestCollection: jest.fn(),
  },
}));

jest.mock('../../src/services/rustHttpHelperClientService', () => ({
  RustHttpHelperClientService: {
    isConfigured: jest.fn(),
    calculateMovement: jest.fn(),
    resolveDefenseRebuild: jest.fn(),
    computeAttackerDistribution: jest.fn(),
    computeEspionageOutcome: jest.fn(),
    computeMissionCargoTransfer: jest.fn(),
    computeHarvestCollection: jest.fn(),
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

  test('uses Rust HTTP helper first for espionage outcome when configured', async () => {
    (RustHttpHelperClientService.isConfigured as jest.Mock).mockReturnValue(true);
    (RustHttpHelperClientService.computeEspionageOutcome as jest.Mock).mockResolvedValue({
      intelLevel: 'standard',
      detected: false,
      detectionChance: 0.35,
      detailScore: 4.1,
      defenseScore: 3,
      engine: 'rust-http',
    });

    const response = await request(app).post('/api/fleet/helpers/espionage-outcome').send({
      probes: 6,
      attackerEspionage: 3,
      defenderEspionage: 2,
      seed: 'fleet:12',
    });

    expect(response.status).toBe(200);
    expect(response.body.success).toBe(true);
    expect(RustHttpHelperClientService.computeEspionageOutcome).toHaveBeenCalledTimes(1);
    expect(FleetHelperService.computeEspionageOutcome).not.toHaveBeenCalled();
  });

  test('falls back for mission cargo transfer helper when Rust HTTP helper errors', async () => {
    (RustHttpHelperClientService.isConfigured as jest.Mock).mockReturnValue(true);
    (RustHttpHelperClientService.computeMissionCargoTransfer as jest.Mock).mockRejectedValue(
      new Error('bad gateway')
    );
    (FleetHelperService.computeMissionCargoTransfer as jest.Mock).mockResolvedValue({
      transferMetal: 100,
      transferCrystal: 200,
      transferDeuterium: 300,
      remainingMetal: 0,
      remainingCrystal: 0,
      remainingDeuterium: 0,
      totalTransfer: 600,
      engine: 'typescript',
    });

    const response = await request(app).post('/api/fleet/helpers/mission-cargo-transfer').send({
      metal: 100,
      crystal: 200,
      deuterium: 300,
      clampNonNegative: true,
    });

    expect(response.status).toBe(200);
    expect(response.body.success).toBe(true);
    expect(RustHttpHelperClientService.computeMissionCargoTransfer).toHaveBeenCalledTimes(1);
    expect(FleetHelperService.computeMissionCargoTransfer).toHaveBeenCalledTimes(1);
  });

  test('falls back for harvest collection helper when Rust HTTP helper errors', async () => {
    (RustHttpHelperClientService.isConfigured as jest.Mock).mockReturnValue(true);
    (RustHttpHelperClientService.computeHarvestCollection as jest.Mock).mockRejectedValue(
      new Error('timeout')
    );
    (FleetHelperService.computeHarvestCollection as jest.Mock).mockResolvedValue({
      collectedMetal: 1000,
      collectedCrystal: 500,
      updatedMetal: 0,
      updatedCrystal: 0,
      recyclerCapacity: 1500,
      empty: false,
      engine: 'typescript',
    });

    const response = await request(app).post('/api/fleet/helpers/harvest-collection').send({
      debrisMetal: 1000,
      debrisCrystal: 500,
      recyclerCount: 10,
      recyclerCargoCapacity: 150,
    });

    expect(response.status).toBe(200);
    expect(response.body.success).toBe(true);
    expect(RustHttpHelperClientService.computeHarvestCollection).toHaveBeenCalledTimes(1);
    expect(FleetHelperService.computeHarvestCollection).toHaveBeenCalledTimes(1);
  });

  test('preserves validation shape for invalid espionage outcome requests', async () => {
    (RustHttpHelperClientService.isConfigured as jest.Mock).mockReturnValue(true);

    const response = await request(app).post('/api/fleet/helpers/espionage-outcome').send({
      probes: 'x',
      attackerEspionage: 2,
      defenderEspionage: 4,
    });

    expect(response.status).toBe(400);
    expect(response.body).toEqual({ success: false, error: 'Invalid espionage outcome request' });
    expect(RustHttpHelperClientService.computeEspionageOutcome).not.toHaveBeenCalled();
    expect(FleetHelperService.computeEspionageOutcome).not.toHaveBeenCalled();
  });
});
