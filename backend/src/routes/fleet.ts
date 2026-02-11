import express, { Request, Response } from 'express';
import { authenticateToken } from '../middleware/auth';
import { FleetService } from '../services/fleetService';
import allianceLogisticsService from '../services/allianceLogisticsService';
import { FleetHelperService } from '../services/fleetHelperService';
import { RustHttpHelperClientService } from '../services/rustHttpHelperClientService';
import { AuthRequest } from '../types';

const router = express.Router();

router.use(authenticateToken);

async function runHelperWithRustFallback<T>(
  operation: string,
  rustCall: () => Promise<T>,
  localCall: () => Promise<T>
): Promise<T> {
  if (!RustHttpHelperClientService.isConfigured()) {
    return localCall();
  }

  try {
    return await rustCall();
  } catch (error: any) {
    console.warn(`[Fleet Helpers] Rust HTTP helper failed for ${operation}, falling back:`, error?.message || error);
    return localCall();
  }
}

router.get('/', async (req: Request, res: Response) => {
  try {
    const authReq = req as AuthRequest;
    const fleets = await FleetService.getUserFleets(authReq.user!.id);
    res.json(fleets);
  } catch (error: any) {
    console.error('Error fetching fleets:', error);
    res.status(500).json({ error: error.message });
  }
});

router.get('/reports', async (req: Request, res: Response) => {
  try {
    const authReq = req as AuthRequest;
    const limit = req.query.limit ? parseInt(req.query.limit as string, 10) : 5;
    const reports = await FleetService.getRecentCombatReports(authReq.user!.id, limit || 5);
    res.json(reports);
  } catch (error: any) {
    console.error('Error fetching combat reports:', error);
    res.status(500).json({ error: error.message });
  }
});

router.post('/helpers/movement', async (req: Request, res: Response) => {
  try {
    const origin = req.body?.origin || {};
    const target = req.body?.target || {};
    const ships = req.body?.ships || {};

    const toInt = (value: unknown): number => Math.trunc(Number(value));
    const isCoordinateValid = (coord: any): boolean => {
      return (
        Number.isFinite(toInt(coord?.galaxy)) &&
        Number.isFinite(toInt(coord?.system)) &&
        Number.isFinite(toInt(coord?.position))
      );
    };

    if (!isCoordinateValid(origin) || !isCoordinateValid(target) || typeof ships !== 'object' || Array.isArray(ships)) {
      return res.status(400).json({ success: false, error: 'Invalid fleet helper movement request' });
    }

    const normalizedInput = {
      origin: {
        galaxy: toInt(origin.galaxy),
        system: toInt(origin.system),
        position: toInt(origin.position),
      },
      target: {
        galaxy: toInt(target.galaxy),
        system: toInt(target.system),
        position: toInt(target.position),
      },
      ships: Object.entries(ships).reduce<Record<string, number>>((acc, [shipType, count]) => {
        const normalized = Math.max(0, toInt(count));
        if (normalized > 0) {
          acc[shipType] = normalized;
        }
        return acc;
      }, {}),
    };

    const result = await runHelperWithRustFallback(
      'movement',
      () => RustHttpHelperClientService.calculateMovement(normalizedInput),
      () => FleetHelperService.calculateMovement(normalizedInput)
    );

    return res.json({ success: true, data: result });
  } catch (error: any) {
    console.error('Error calculating fleet movement helper:', error);
    return res.status(500).json({ success: false, error: error.message || 'Fleet movement helper failed' });
  }
});

router.post('/helpers/combat/defense-rebuild', async (req: Request, res: Response) => {
  try {
    const current = req.body?.current || {};
    const losses = req.body?.losses || {};
    const rebuildRate = req.body?.rebuildRate;
    const seed = req.body?.seed;

    if (
      typeof current !== 'object' ||
      Array.isArray(current) ||
      typeof losses !== 'object' ||
      Array.isArray(losses)
    ) {
      return res.status(400).json({ success: false, error: 'Invalid defense rebuild request' });
    }

    const normalizedInput = {
      current,
      losses,
      rebuildRate: Number.isFinite(Number(rebuildRate)) ? Number(rebuildRate) : undefined,
      seed: typeof seed === 'string' ? seed : undefined,
    };

    const result = await runHelperWithRustFallback(
      'combat/defense-rebuild',
      () => RustHttpHelperClientService.resolveDefenseRebuild(normalizedInput),
      () => FleetHelperService.resolveDefenseRebuild(normalizedInput)
    );

    return res.json({ success: true, data: result });
  } catch (error: any) {
    console.error('Error resolving defense rebuild helper:', error);
    return res.status(500).json({ success: false, error: error.message || 'Defense rebuild helper failed' });
  }
});

router.post('/helpers/combat/attacker-distribution', async (req: Request, res: Response) => {
  try {
    const participants = req.body?.participants;
    const totalLosses = req.body?.totalLosses || {};
    const loot = req.body?.loot || {};
    const winner = req.body?.winner;

    if (
      !Array.isArray(participants) ||
      typeof totalLosses !== 'object' ||
      Array.isArray(totalLosses) ||
      typeof loot !== 'object' ||
      Array.isArray(loot) ||
      !['attacker', 'defender', 'draw'].includes(String(winner))
    ) {
      return res.status(400).json({ success: false, error: 'Invalid attacker distribution request' });
    }

    const normalizeFleet = (fleet: unknown): Record<string, number> => {
      if (!fleet || typeof fleet !== 'object' || Array.isArray(fleet)) return {};
      return Object.entries(fleet).reduce<Record<string, number>>((acc, [shipType, count]) => {
        const normalized = Math.max(0, Math.trunc(Number(count) || 0));
        if (normalized > 0) {
          acc[shipType] = normalized;
        }
        return acc;
      }, {});
    };

    const normalizeResource = (value: unknown): number => Math.max(0, Math.trunc(Number(value) || 0));

    const normalizedInput = {
      participants: participants.map((participant) => normalizeFleet(participant)),
      totalLosses: normalizeFleet(totalLosses),
      loot: {
        metal: normalizeResource((loot as Record<string, unknown>).metal),
        crystal: normalizeResource((loot as Record<string, unknown>).crystal),
        deuterium: normalizeResource((loot as Record<string, unknown>).deuterium),
      },
      winner: winner as 'attacker' | 'defender' | 'draw',
    };

    const result = await runHelperWithRustFallback(
      'combat/attacker-distribution',
      () => RustHttpHelperClientService.computeAttackerDistribution(normalizedInput),
      () => FleetHelperService.computeAttackerDistribution(normalizedInput)
    );

    return res.json({ success: true, data: result });
  } catch (error: any) {
    console.error('Error resolving attacker distribution helper:', error);
    return res.status(500).json({ success: false, error: error.message || 'Attacker distribution helper failed' });
  }
});

router.post('/dispatch', async (req: Request, res: Response) => {
  try {
    const authReq = req as AuthRequest;
    const {
      originPlanetId,
      targetGalaxy,
      targetSystem,
      targetPosition,
      missionType,
      ships,
      cargo,
      acsGroupId,
    } = req.body;

    if (!originPlanetId || !targetGalaxy || !targetSystem || !targetPosition || !missionType || !ships) {
      return res.status(400).json({ error: 'Missing required fields' });
    }

    const result = await FleetService.dispatchFleet(
      authReq.user!.id,
      parseInt(originPlanetId),
      parseInt(targetGalaxy),
      parseInt(targetSystem),
      parseInt(targetPosition),
      missionType,
      ships,
      cargo || { metal: 0, crystal: 0, deuterium: 0 },
      acsGroupId ? parseInt(acsGroupId, 10) : undefined
    );

    res.status(201).json(result);
  } catch (error: any) {
    console.error('Error dispatching fleet:', error);
    res.status(400).json({ error: error.message });
  }
});

router.post('/:id/recall', async (req: Request, res: Response) => {
  try {
    const authReq = req as AuthRequest;
    const fleetId = parseInt(req.params.id);

    await FleetService.recallFleet(authReq.user!.id, fleetId);
    await allianceLogisticsService.cancelDepotSessionByFleet(fleetId);

    res.status(200).json({ message: 'Fleet recalled' });
  } catch (error: any) {
    console.error('Error recalling fleet:', error);
    res.status(400).json({ error: error.message });
  }
});

router.get('/history', async (req: Request, res: Response) => {
  try {
    const authReq = req as AuthRequest;
    const limit = req.query.limit ? parseInt(req.query.limit as string, 10) : 25;
    const result = await FleetService.getMissionHistory(authReq.user!.id, limit);
    res.json(result);
  } catch (error: any) {
    console.error('Error fetching mission history:', error);
    res.status(500).json({ error: error.message });
  }
});

router.post('/:id/cancel', async (req: Request, res: Response) => {
  try {
    const authReq = req as AuthRequest;
    const fleetId = parseInt(req.params.id);

    const result = await FleetService.cancelFleet(authReq.user!.id, fleetId);
    await allianceLogisticsService.cancelDepotSessionByFleet(fleetId);

    res.json({ success: true, message: 'Fleet cancelled', data: result });
  } catch (error: any) {
    console.error('Error cancelling fleet:', error);
    res.status(400).json({ success: false, error: error.message });
  }
});

export default router;
