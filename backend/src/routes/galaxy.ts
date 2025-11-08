import express, { Request, Response } from 'express';
import { authenticateToken } from '../middleware/auth';
import { AuthRequest } from '../types';
import GalaxyService from '../services/galaxyService';

const router = express.Router();

router.use(authenticateToken);

router.get('/', async (req: Request, res: Response) => {
  try {
    const authReq = req as AuthRequest;
    const galaxy = parseInt(req.query.galaxy as string, 10) || 1;
    const system = parseInt(req.query.system as string, 10) || 1;
    const originPlanetId = req.query.originPlanetId
      ? parseInt(req.query.originPlanetId as string, 10)
      : undefined;

    const snapshot = await GalaxyService.getSystemSnapshot({
      userId: authReq.user!.id,
      galaxy,
      system,
      originPlanetId,
    });

    res.json(snapshot);
  } catch (error: any) {
    console.error('Error fetching galaxy:', error);
    res.status(500).json({ error: error.message });
  }
});

router.get('/debris', async (req: Request, res: Response) => {
  try {
    const authReq = req as AuthRequest;
    const galaxy = parseInt(req.query.galaxy as string, 10) || 1;
    const system = parseInt(req.query.system as string, 10) || 1;
    const originPlanetId = req.query.originPlanetId
      ? parseInt(req.query.originPlanetId as string, 10)
      : undefined;

    const snapshot = await GalaxyService.getSystemSnapshot({
      userId: authReq.user!.id,
      galaxy,
      system,
      originPlanetId,
    });

    const debris = snapshot.planets
      .filter((slot) => Boolean(slot.debris))
      .map((slot) => ({
        position: slot.position,
        debris: slot.debris,
      }));

    res.json(debris);
  } catch (error: any) {
    console.error('Error fetching debris:', error);
    res.status(500).json({ error: error.message });
  }
});

export default router;
