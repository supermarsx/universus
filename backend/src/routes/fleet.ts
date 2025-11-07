import express, { Request, Response } from 'express';
import { authenticateToken } from '../middleware/auth';
import { FleetService } from '../services/fleetService';
import { AuthRequest } from '../types';

const router = express.Router();

router.use(authenticateToken);

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
      cargo || { metal: 0, crystal: 0, deuterium: 0 }
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

export default router;
