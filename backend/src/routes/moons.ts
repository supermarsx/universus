import express, { Request, Response } from 'express';
import { authenticateToken } from '../middleware/auth';
import { AuthRequest } from '../types';
import moonService from '../services/moonService';
import { BuildingService } from '../services/buildingService';
import { ShipyardService } from '../services/shipyardService';
import phalanxService from '../services/phalanxService';
import jumpGateService from '../services/jumpGateService';

const router = express.Router();

router.use(authenticateToken);

router.get('/:planetId', async (req: AuthRequest, res: Response) => {
  try {
    const planetId = parseInt(req.params.planetId, 10);
    const moon = await moonService.getMoonByPlanetId(planetId);
    if (!moon) {
      return res.status(404).json({ success: false, error: 'Moon not found' });
    }

    if (moon.user_id !== req.user!.id) {
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
    const moons = await moonService.listMoonsByUser(req.user!.id);
    res.json({ success: true, data: moons });
  } catch (error: any) {
    console.error('List moons error:', error);
    res.status(500).json({ success: false, error: 'Failed to list moons' });
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

    const result = await phalanxService.performScan({
      userId: req.user!.id,
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
    const result = await jumpGateService.jumpFleet(req.user!.id, fromMoonId, toMoonId, fleetIds);
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
    const moonId = parseInt(req.params.moonId, 10);
    const { numDeathstars } = req.body;
    if (!Number.isFinite(numDeathstars) || numDeathstars < 1) {
      return res.status(400).json({ success: false, error: 'Invalid number of Deathstars' });
    }
    // TODO: Validate attacker owns the Deathstars and they are present at the moon
    const result = await (await import('../services/destroyMoonService')).default.attemptDestruction(req.user!.id, moonId, numDeathstars);
    if (result.error) {
      return res.status(400).json({ success: false, error: result.error });
    }
    res.json({ success: true, data: result });
  } catch (error: any) {
    console.error('Destroy Moon error:', error);
    res.status(500).json({ success: false, error: 'Moon destruction failed' });
  }
});

export default router;
