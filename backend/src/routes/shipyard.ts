import express, { Request, Response } from 'express';
import { authenticateToken } from '../middleware/auth';
import { ShipyardService } from '../services/shipyardService';
import { LocationType } from '../services/locationService';
import { AuthRequest } from '../types';

const router = express.Router();

router.use(authenticateToken);

router.get('/:planetId/queue', async (req: Request, res: Response) => {
  try {
    const planetId = parseInt(req.params.planetId, 10);
    const locationType = (req.query.locationType as LocationType) || 'planet';
    const moonId =
      req.query.moonId !== undefined
        ? parseInt(req.query.moonId as string, 10)
        : undefined;

    const queue = await ShipyardService.getQueue({
      planetId,
      locationType,
      moonId,
    });
    res.json(queue);
  } catch (error: any) {
    console.error('Error fetching shipyard queue:', error);
    res.status(500).json({ error: error.message });
  }
});

router.post('/:planetId/build', async (req: Request, res: Response) => {
  try {
    const authReq = req as AuthRequest;
    const planetId = parseInt(req.params.planetId, 10);
    const { unitType, quantity, locationType, moonId } = req.body;

    if (!unitType || !quantity) {
      return res.status(400).json({ error: 'Unit type and quantity required' });
    }

    const result = await ShipyardService.startProduction(
      authReq.user!.id,
      unitType,
      parseInt(quantity, 10),
      {
        planetId,
        locationType,
        moonId: moonId ? parseInt(moonId, 10) : undefined,
        expectedPlanetId: planetId,
      }
    );

    res.status(201).json(result);
  } catch (error: any) {
    console.error('Error starting production:', error);
    res.status(400).json({ error: error.message });
  }
});

router.delete('/queue/:id', async (req: Request, res: Response) => {
  try {
    const authReq = req as AuthRequest;
    const queueId = parseInt(req.params.id);

    await ShipyardService.cancelProduction(authReq.user!.id, queueId);
    res.status(200).json({ message: 'Production cancelled' });
  } catch (error: any) {
    console.error('Error cancelling production:', error);
    res.status(400).json({ error: error.message });
  }
});

export default router;
