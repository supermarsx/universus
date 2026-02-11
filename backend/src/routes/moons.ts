import express, { Request, Response } from 'express';
import { authenticateToken, assertAuthenticated } from '../middleware/auth';
import { AuthRequest } from '../types';
import moonService from '../services/moonService';
import { BuildingService } from '../services/buildingService';
import { ShipyardService } from '../services/shipyardService';
import phalanxService from '../services/phalanxService';
import jumpGateService from '../services/jumpGateService';
import { pool } from '../config/database';

const router = express.Router();

router.use(authenticateToken, assertAuthenticated);


router.get('/:planetId', async (req: AuthRequest, res: Response) => {
  try {
    const planetId = parseInt(req.params.planetId, 10);
    const moon = await moonService.getMoonByPlanetId(planetId);
    if (!moon) {
      return res.status(404).json({ success: false, error: 'Moon not found' });
    }

    const authReq = req as AuthRequest;
    if (moon.user_id !== authReq.user!.id) {
      return res.status(403).json({ success: false, error: 'Access denied' });
    }


    const [constructionQueue, shipyardQueue] = await Promise.all([
      BuildingService.getConstructionQueue({
        planetId,
        locationType: 'moon',
        moonId: moon.id,
      }),
      ShipyardService.getQueue({
        planetId,
        locationType: 'moon',
        moonId: moon.id,
      }),
    ]);

    res.json({
      success: true,
      data: {
        moon,
        constructionQueue,
        shipyardQueue,
      },
    });
  } catch (error: any) {
    console.error('Get moon error:', error);
    res.status(500).json({ success: false, error: 'Failed to load moon' });
  }
});

router.get('/', async (req: AuthRequest, res: Response) => {
  try {
    const authReq = req as AuthRequest;
    const moons = await moonService.listMoonsByUser(authReq.user!.id);
    res.json({ success: true, data: moons });
  } catch (error: any) {
    console.error('List moons error:', error);
    res.status(500).json({ success: false, error: 'Failed to list moons' });
  }
});

router.get('/id/:moonId', async (req: AuthRequest, res: Response) => {
  try {
    const moonId = parseInt(req.params.moonId, 10);
    if (!Number.isFinite(moonId)) {
      return res.status(400).json({ success: false, error: 'Invalid moon id' });
    }

    const moon = await moonService.getMoonById(moonId);
    if (!moon) {
      return res.status(404).json({ success: false, error: 'Moon not found' });
    }

    const authReq = req as AuthRequest;
    if (moon.user_id !== authReq.user!.id) {
      return res.status(403).json({ success: false, error: 'Access denied' });
    }

    const [constructionQueue, shipyardQueue] = await Promise.all([
      BuildingService.getConstructionQueue({
        planetId: moon.planet_id,
        locationType: 'moon',
        moonId: moon.id,
      }),
      ShipyardService.getQueue({
        planetId: moon.planet_id,
        locationType: 'moon',
        moonId: moon.id,
      }),
    ]);

    res.json({
      success: true,
      data: {
        moon,
        constructionQueue,
        shipyardQueue,
      },
    });
  } catch (error: any) {
    console.error('Get moon by id error:', error);
    res.status(500).json({ success: false, error: 'Failed to load moon' });
  }
});

router.get('/public/:moonId', async (req: AuthRequest, res: Response) => {
  try {
    const moonId = parseInt(req.params.moonId, 10);
    if (!Number.isFinite(moonId)) {
      return res.status(400).json({ success: false, error: 'Invalid moon id' });
    }

    const result = await pool.query(
      `SELECT m.id, m.diameter, m.sensor_phalanx, m.jump_gate, m.created_at,
              p.galaxy, p.system, p.position,
              u.username AS owner_alias
       FROM moons m
       JOIN planets p ON p.id = m.planet_id
       JOIN users u ON u.id = m.user_id
       WHERE m.id = $1`,
      [moonId]
    );

    if (result.rows.length === 0) {
      return res.status(404).json({ success: false, error: 'Moon not found' });
    }

    const row = result.rows[0];
    res.json({
      success: true,
      data: {
        id: row.id,
        diameter: row.diameter,
        ownerAlias: row.owner_alias,
        hasSensorPhalanx: Number(row.sensor_phalanx || 0) > 0,
        hasJumpGate: Number(row.jump_gate || 0) > 0,
        coordinates: {
          galaxy: row.galaxy,
          system: row.system,
          position: row.position,
        },
        createdAt: row.created_at,
      },
    });
  } catch (error: any) {
    console.error('Get public moon info error:', error);
    res.status(500).json({ success: false, error: 'Failed to load moon info' });
  }
});

router.post('/:moonId/phalanx', async (req: AuthRequest, res: Response) => {
  try {
    const moonId = parseInt(req.params.moonId, 10);
    const targetGalaxy = parseInt(req.body.targetGalaxy, 10);

    const targetSystem = parseInt(req.body.targetSystem, 10);
    const targetPosition = parseInt(req.body.targetPosition, 10);

    if (
      !Number.isFinite(targetGalaxy) ||
      !Number.isFinite(targetSystem) ||
      !Number.isFinite(targetPosition)
    ) {
      return res.status(400).json({ success: false, error: 'Invalid coordinates' });
    }

    const authReq = req as AuthRequest;
    const result = await phalanxService.performScan({
      userId: authReq.user!.id,
      moonId,
      targetGalaxy,
      targetSystem,
      targetPosition,
    });


    res.json({ success: true, data: result });
  } catch (error: any) {
    console.error('Phalanx scan error:', error);
    res.status(400).json({ success: false, error: error.message || 'Phalanx scan failed' });
  }
});

// POST /api/moons/:moonId/jump-gate
router.post('/:moonId/jump-gate', async (req: AuthRequest, res: Response) => {
  try {
    const fromMoonId = parseInt(req.params.moonId, 10);
    const { toMoonId, fleetIds } = req.body;

    if (!Number.isFinite(toMoonId) || !Array.isArray(fleetIds) || fleetIds.length === 0) {
      return res.status(400).json({ success: false, error: 'Invalid request' });
    }
    const authReq = req as AuthRequest;
    const result = await jumpGateService.jumpFleet(authReq.user!.id, fromMoonId, toMoonId, fleetIds);

    if (!result.success) {
      return res.status(400).json({ success: false, error: result.error });
    }
    res.json({ success: true });
  } catch (error: any) {
    console.error('Jump Gate error:', error);
    res.status(500).json({ success: false, error: 'Jump Gate failed' });
  }
});

// POST /api/moons/:moonId/destroy
router.post('/:moonId/destroy', async (req: AuthRequest, res: Response) => {
  try {
    const sourceMoonId = parseInt(req.params.moonId, 10);
    const { targetMoonId, numDeathstars, speedPercent } = req.body;

    if (
      !Number.isFinite(sourceMoonId) ||
      !Number.isFinite(targetMoonId) ||
      !Number.isFinite(numDeathstars) ||
      numDeathstars < 1
    ) {
      return res.status(400).json({ success: false, error: 'Invalid destroy moon request' });
    }
    const authReq = req as AuthRequest;
    const moonResult = await pool.query(
      'SELECT deathstar FROM moons WHERE id = $1 AND user_id = $2',
      [sourceMoonId, authReq.user!.id]
    );

    if (moonResult.rows.length === 0) {
      return res.status(403).json({ success: false, error: 'Moon access denied' });
    }

    const available = parseInt(moonResult.rows[0]?.deathstar || '0', 10);
    if (available < numDeathstars) {
      return res.status(400).json({ success: false, error: 'Insufficient Deathstars at moon' });
    }
    const result = await (
      await import('../services/destroyMoonService')
    ).default.scheduleDestruction(
      authReq.user!.id,
      sourceMoonId,
      parseInt(targetMoonId, 10),
      parseInt(numDeathstars, 10),
      Number.isFinite(speedPercent) ? Number(speedPercent) : 100
    );

    res.json({ success: true, data: result });
  } catch (error: any) {
    console.error('Destroy Moon error:', error);
    res.status(400).json({ success: false, error: error?.message || 'Moon destruction failed' });
  }
});

router.post('/:moonId/jump', async (req: AuthRequest, res: Response) => {
  try {
    const fromMoonId = parseInt(req.params.moonId, 10);
    const { toMoonId, fleetIds } = req.body;

    if (!Number.isFinite(toMoonId) || !Array.isArray(fleetIds) || fleetIds.length === 0) {
      return res.status(400).json({ success: false, error: 'Invalid request' });
    }
    const authReq = req as AuthRequest;
    const result = await jumpGateService.jumpFleet(authReq.user!.id, fromMoonId, toMoonId, fleetIds);

    if (!result.success) {
      return res.status(400).json({ success: false, error: result.error });
    }
    res.json({ success: true });
  } catch (error: any) {
    console.error('Jump Gate error:', error);
    res.status(500).json({ success: false, error: 'Jump Gate failed' });
  }
});

router.post('/:moonId/scan', async (req: AuthRequest, res: Response) => {
  try {
    const moonId = parseInt(req.params.moonId, 10);
    const targetGalaxy = parseInt(req.body.targetGalaxy, 10);
    const targetSystem = parseInt(req.body.targetSystem, 10);
    const targetPosition = parseInt(req.body.targetPosition, 10);

    if (
      !Number.isFinite(targetGalaxy) ||
      !Number.isFinite(targetSystem) ||
      !Number.isFinite(targetPosition)
    ) {
      return res.status(400).json({ success: false, error: 'Invalid coordinates' });
    }

    const authReq = req as AuthRequest;
    const result = await phalanxService.performScan({
      userId: authReq.user!.id,
      moonId,
      targetGalaxy,
      targetSystem,
      targetPosition,
    });

    res.json({ success: true, data: result });
  } catch (error: any) {
    console.error('Phalanx scan error:', error);
    res.status(400).json({ success: false, error: error.message || 'Phalanx scan failed' });
  }
});

export default router;
