import express, { Request, Response } from 'express';
import jwt from 'jsonwebtoken';
import { analyticsService } from '../services/analyticsService';
import { authenticateToken } from '../middleware/auth';
import { requireAdmin, requirePermission } from '../middleware/adminAuth';
import { AuthRequest } from '../types';

const router = express.Router();

const resolveUserIdFromToken = (req: Request): number | undefined => {
  const authHeader = req.headers['authorization'];
  if (!authHeader || !authHeader.startsWith('Bearer ')) {
    return undefined;
  }

  const token = authHeader.split(' ')[1];
  try {
    const secret = process.env.JWT_SECRET || 'your_super_secret_jwt_key';
    const decoded = jwt.verify(token, secret) as { userId: number };
    return decoded.userId;
  } catch {
    return undefined;
  }
};

router.post('/events', async (req: Request, res: Response) => {
  try {
    const { eventType, sessionId, properties } = req.body;
    if (!eventType) {
      return res.status(400).json({ error: 'eventType is required' });
    }

    const userId =
      (req as AuthRequest)?.user?.id ||
      resolveUserIdFromToken(req);

    await analyticsService.trackEvent({
      eventType,
      sessionId,
      properties,
      userId: userId || undefined,
      userAgent: req.get('user-agent') || undefined,
      ipAddress: req.ip
    });

    res.json({ success: true });
  } catch (error: any) {
    console.error('Analytics event error:', error);
    res.status(500).json({ error: 'Failed to record event' });
  }
});

router.get(
  '/usage',
  authenticateToken,
  requireAdmin,
  requirePermission('analytics:view'),
  async (req: Request, res: Response) => {
    try {
      const days = parseInt((req.query.days as string) || '7', 10);
      const data = await analyticsService.getUsageSummary(
        Number.isNaN(days) ? 7 : days
      );
      res.json({ success: true, data });
    } catch (error: any) {
      console.error('Analytics usage error:', error);
      res.status(500).json({ error: 'Failed to load analytics data' });
    }
  }
);

export default router;
