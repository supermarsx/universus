import express, { Response } from 'express';
import { authenticateToken, assertAuthenticated } from '../middleware/auth';
import { AuthRequest } from '../types';
import destroyMoonService from '../services/destroyMoonService';

const router = express.Router();

router.use(authenticateToken, assertAuthenticated);

// Compatibility endpoint aligned with moon mechanics spec.
router.post('/destroyMoon', async (req: AuthRequest, res: Response) => {
  try {
    const sourceMoonId = parseInt(req.body.sourceMoonId, 10);
    const targetMoonId = parseInt(req.body.targetMoonId, 10);
    const numDeathstars = parseInt(req.body.numDeathstars, 10);
    const speedPercent = Number.isFinite(Number(req.body.speedPercent))
      ? Number(req.body.speedPercent)
      : 100;

    if (!Number.isFinite(sourceMoonId) || !Number.isFinite(targetMoonId) || !Number.isFinite(numDeathstars) || numDeathstars < 1) {
      return res.status(400).json({ success: false, error: 'Invalid destroy moon request' });
    }

    const result = await destroyMoonService.scheduleDestruction(
      req.user!.id,
      sourceMoonId,
      targetMoonId,
      numDeathstars,
      speedPercent
    );

    res.json({ success: true, data: result });
  } catch (error: any) {
    console.error('Destroy moon schedule error:', error);
    res.status(400).json({ success: false, error: error?.message || 'Moon destruction failed' });
  }
});

export default router;
