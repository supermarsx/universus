import express, { Request, Response } from 'express';
import { authenticateToken } from '../middleware/auth';
import { AuthRequest } from '../types';
import AcsService from '../services/acsService';

const router = express.Router();

router.use(authenticateToken);

router.get('/', async (req: Request, res: Response) => {
  try {
    const authReq = req as AuthRequest;
    const groups = await AcsService.listAllianceGroups(authReq.user!.id);
    res.json({ success: true, groups });
  } catch (error: any) {
    console.error('Error loading ACS groups:', error);
    res.status(500).json({ success: false, message: error.message || 'Failed to load ACS groups' });
  }
});

router.post('/', async (req: Request, res: Response) => {
  try {
    const authReq = req as AuthRequest;
    const payload = req.body;
    const group = await AcsService.createGroup(authReq.user!.id, {
      missionType: payload.missionType,
      targetGalaxy: payload.targetGalaxy,
      targetSystem: payload.targetSystem,
      targetPosition: payload.targetPosition,
      departureWindowStart: payload.departureWindowStart,
      departureWindowEnd: payload.departureWindowEnd,
      notes: payload.notes,
    });
    res.status(201).json({ success: true, group });
  } catch (error: any) {
    console.error('Error creating ACS group:', error);
    res.status(400).json({ success: false, message: error.message || 'Failed to create ACS group' });
  }
});

router.post('/:id/join', async (req: Request, res: Response) => {
  try {
    const authReq = req as AuthRequest;
    const groupId = parseInt(req.params.id, 10);
    await AcsService.joinGroup(authReq.user!.id, groupId, req.body.planetId);
    res.json({ success: true });
  } catch (error: any) {
    console.error('Error joining ACS group:', error);
    res.status(400).json({ success: false, message: error.message || 'Failed to join ACS group' });
  }
});

router.delete('/:id/leave', async (req: Request, res: Response) => {
  try {
    const authReq = req as AuthRequest;
    const groupId = parseInt(req.params.id, 10);
    await AcsService.leaveGroup(authReq.user!.id, groupId);
    res.json({ success: true });
  } catch (error: any) {
    console.error('Error leaving ACS group:', error);
    res.status(400).json({ success: false, message: error.message || 'Failed to leave ACS group' });
  }
});

export default router;
