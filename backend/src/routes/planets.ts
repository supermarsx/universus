import express, { Request, Response } from 'express';
import { authenticateToken } from '../middleware/auth';
import { PlanetService } from '../services/planetService';
import { BuildingService } from '../services/buildingService';
import { AuthRequest } from '../types';

const router = express.Router();

router.use(authenticateToken);

router.get('/', async (req: Request, res: Response) => {
  try {
    const authReq = req as AuthRequest;
    const planets = await PlanetService.getPlanetsByUserId(authReq.user!.id);
    res.json(planets);
  } catch (error: any) {
    console.error('Error fetching planets:', error);
    res.status(500).json({ error: error.message });
  }
});

router.get('/:id', async (req: Request, res: Response) => {
  try {
    const authReq = req as AuthRequest;
    const planetId = parseInt(req.params.id);
    
    const planet = await PlanetService.updateResources(planetId);
    
    if (!planet || planet.user_id !== authReq.user!.id) {
      return res.status(403).json({ error: 'Access denied' });
    }

    const production = await PlanetService.getResourceProduction(planet);
    const constructionQueue = await BuildingService.getConstructionQueue(planetId);

    res.json({
      planet,
      production,
      constructionQueue,
    });
  } catch (error: any) {
    console.error('Error fetching planet:', error);
    res.status(500).json({ error: error.message });
  }
});

router.post('/:id/build', async (req: Request, res: Response) => {
  try {
    const authReq = req as AuthRequest;
    const planetId = parseInt(req.params.id);
    const { buildingType } = req.body;

    if (!buildingType) {
      return res.status(400).json({ error: 'Building type required' });
    }

    const construction = await BuildingService.startConstruction(
      authReq.user!.id,
      planetId,
      buildingType
    );

    res.status(201).json(construction);
  } catch (error: any) {
    console.error('Error starting construction:', error);
    res.status(400).json({ error: error.message });
  }
});

router.delete('/construction/:id', async (req: Request, res: Response) => {
  try {
    const authReq = req as AuthRequest;
    const constructionId = parseInt(req.params.id);

    await BuildingService.cancelConstruction(authReq.user!.id, constructionId);
    res.status(200).json({ message: 'Construction cancelled' });
  } catch (error: any) {
    console.error('Error cancelling construction:', error);
    res.status(400).json({ error: error.message });
  }
});

export default router;
