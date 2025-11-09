/**
 * @module backend/routes/research
 *
 * Research endpoints for starting, cancelling and querying the player's
 * research queue and overview.
 */

import express, { Request, Response } from 'express';
import { authenticateToken } from '../middleware/auth';
import { ResearchService } from '../services/researchService';
import { AuthRequest } from '../types';

const router = express.Router();

router.use(authenticateToken);

router.get('/', async (req: Request, res: Response) => {
  try {
    const authReq = req as AuthRequest;
    const planetId = req.query.planetId
      ? parseInt(req.query.planetId as string, 10)
      : undefined;

    const overview = await ResearchService.getResearchOverview(
      authReq.user!.id,
      planetId
    );
    
    res.json(overview);
  } catch (error: any) {
    console.error('Error fetching research:', error);
    res.status(500).json({ error: error.message });
  }
});

router.post('/start', async (req: Request, res: Response) => {
  try {
    const authReq = req as AuthRequest;
    const { planetId, researchType } = req.body;

    if (!planetId || !researchType) {
      return res.status(400).json({ error: 'Planet ID and research type required' });
    }

    const result = await ResearchService.startResearch(
      authReq.user!.id,
      parseInt(planetId, 10),
      researchType
    );

    res.status(201).json(result);
  } catch (error: any) {
    console.error('Error starting research:', error);
    res.status(400).json({ error: error.message });
  }
});

router.delete('/queue/:id', async (req: Request, res: Response) => {
  try {
    const authReq = req as AuthRequest;
    const queueId = parseInt(req.params.id);

    await ResearchService.cancelResearch(authReq.user!.id, queueId);
    res.status(200).json({ message: 'Research cancelled' });
  } catch (error: any) {
    console.error('Error cancelling research:', error);
    res.status(400).json({ error: error.message });
  }
});

export default router;
