import express, { Request, Response } from 'express';
import { authenticateToken } from '../middleware/auth';
import { AuthRequest } from '../types';
import playerBlockService, { BlockScope } from '../services/playerBlockService';
import { pool } from '../config/database';

const router = express.Router();

router.use(authenticateToken);

router.get('/', async (req: AuthRequest, res: Response) => {
  try {
    const userId = req.user!.id;
    const blocks = await playerBlockService.listBlocks(userId);
    res.json({ success: true, data: blocks });
  } catch (error: any) {
    console.error('List player blocks failed:', error);
    res.status(500).json({ success: false, error: 'Failed to load block list' });
  }
});

router.post('/', async (req: AuthRequest, res: Response) => {
  try {
    const userId = req.user!.id;
    const { blockedUserId, username, scope = 'all', reason } = req.body || {};

    let targetId = blockedUserId ? parseInt(blockedUserId, 10) : null;

    if (!targetId && username) {
      const lookup = await pool.query(
        'SELECT id FROM users WHERE LOWER(username) = LOWER($1)',
        [username]
      );
      targetId = lookup.rows[0]?.id || null;
    }

    if (!targetId) {
      return res.status(400).json({ success: false, error: 'User not found' });
    }

    if (targetId === userId) {
      return res.status(400).json({ success: false, error: 'You cannot block yourself' });
    }

    const blockScope: BlockScope = ['all', 'chat', 'messages'].includes(scope)
      ? scope
      : 'all';

    const entry = await playerBlockService.blockUser(userId, targetId, blockScope, reason);
    res.json({ success: true, data: entry, message: 'Player blocked' });
  } catch (error: any) {
    console.error('Block player failed:', error);
    res.status(500).json({ success: false, error: 'Failed to block player' });
  }
});

router.delete('/:targetIdentifier', async (req: AuthRequest, res: Response) => {
  try {
    const userId = req.user!.id;
    const identifier = req.params.targetIdentifier;
    let targetId = parseInt(identifier, 10);

    if (Number.isNaN(targetId)) {
      const lookup = await pool.query(
        'SELECT id FROM users WHERE LOWER(username) = LOWER($1)',
        [identifier]
      );
      targetId = lookup.rows[0]?.id || NaN;
    }

    if (Number.isNaN(targetId)) {
      return res.status(404).json({ success: false, error: 'User not found' });
    }

    const scope = (req.query.scope as BlockScope) || undefined;
    const removed = await playerBlockService.unblockUser(userId, targetId, scope);

    if (!removed) {
      return res.status(404).json({ success: false, error: 'Block not found' });
    }

    res.json({ success: true, message: 'Player unblocked' });
  } catch (error: any) {
    console.error('Unblock player failed:', error);
    res.status(500).json({ success: false, error: 'Failed to unblock player' });
  }
});

export default router;
