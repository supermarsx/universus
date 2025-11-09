/**
 * @module backend/routes/users
 *
 * User-related API routes: profile retrieval, simple leaderboard proxy
 * endpoints and user-specific data access for authenticated clients.
 */

import express, { Request, Response } from 'express';
import { authenticateToken } from '../middleware/auth';
import { pool } from '../config/database';
import { AuthRequest } from '../types';

const router = express.Router();

router.use(authenticateToken);

router.get('/me', async (req: Request, res: Response) => {
  try {
    const authReq = req as AuthRequest;
    const user = authReq.user;
    
    // Get research levels
    const researchResult = await pool.query(
      'SELECT * FROM research WHERE user_id = $1',
      [user!.id]
    );

    res.json({
      user,
      research: researchResult.rows[0] || {},
    });
  } catch (error: any) {
    console.error('Error fetching user data:', error);
    res.status(500).json({ error: error.message });
  }
});

router.get('/leaderboard', async (req: Request, res: Response) => {
  try {
    const result = await pool.query(
      `SELECT u.id, u.username, ps.total_score, ps.economy_score, ps.research_score, ps.military_score
       FROM users u
       JOIN player_scores ps ON u.id = ps.user_id
       ORDER BY ps.total_score DESC
       LIMIT 100`
    );

    res.json(result.rows);
  } catch (error: any) {
    console.error('Error fetching leaderboard:', error);
    res.status(500).json({ error: error.message });
  }
});

export default router;
