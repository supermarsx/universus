import express from 'express';
import { LeaderboardService } from '../services/leaderboardService';
import { authenticateToken } from '../middleware/auth';
import { AuthRequest } from '../types';
import { pool } from '../config/database';
import { redis } from '../config/redis';

const router = express.Router();
const leaderboardService = new LeaderboardService(pool, redis);

// Apply authentication middleware to all routes
router.use(authenticateToken);

/**
 * GET /leaderboard/players
 * Get top players leaderboard
 * Query params: limit (default: 100), offset (default: 0)
 */
router.get('/players', async (req: AuthRequest, res) => {
  try {
    const limit = parseInt(req.query.limit as string) || 100;
    const offset = parseInt(req.query.offset as string) || 0;

    const players = await leaderboardService.getTopPlayers(limit, offset);

    res.json({
      success: true,
      data: players,
      pagination: {
        limit,
        offset,
        total: players.length,
      },
    });
  } catch (error: any) {
    console.error('Error fetching player leaderboard:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to fetch leaderboard',
    });
  }
});

/**
 * GET /leaderboard/alliances
 * Get top alliances leaderboard
 * Query params: limit (default: 50), offset (default: 0)
 */
router.get('/alliances', async (req: AuthRequest, res) => {
  try {
    const limit = parseInt(req.query.limit as string) || 50;
    const offset = parseInt(req.query.offset as string) || 0;

    const alliances = await leaderboardService.getTopAlliances(limit, offset);

    res.json({
      success: true,
      data: alliances,
      pagination: {
        limit,
        offset,
        total: alliances.length,
      },
    });
  } catch (error: any) {
    console.error('Error fetching alliance leaderboard:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to fetch leaderboard',
    });
  }
});

/**
 * GET /leaderboard/player/:userId
 * Get specific player's rank and surrounding players
 * Query params: range (default: 5)
 */
router.get('/player/:userId', async (req: AuthRequest, res) => {
  try {
    const userId = parseInt(req.params.userId);
    const range = parseInt(req.query.range as string) || 5;

    const result = await leaderboardService.getPlayerRank(userId, range);

    res.json({
      success: true,
      data: result,
    });
  } catch (error: any) {
    console.error('Error fetching player rank:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to fetch player rank',
    });
  }
});

/**
 * GET /leaderboard/me
 * Get current user's rank and surrounding players
 * Query params: range (default: 5)
 */
router.get('/me', async (req: AuthRequest, res) => {
  try {
    const userId = req.user!.id;
    const range = parseInt(req.query.range as string) || 5;

    const result = await leaderboardService.getPlayerRank(userId, range);

    res.json({
      success: true,
      data: result,
    });
  } catch (error: any) {
    console.error('Error fetching user rank:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to fetch your rank',
    });
  }
});

/**
 * POST /leaderboard/update
 * Manually trigger leaderboard update (admin only)
 */
router.post('/update', async (req: AuthRequest, res) => {
  try {
    // In production, add admin check here
    const playerCount = await leaderboardService.updatePlayerLeaderboard();
    const allianceCount = await leaderboardService.updateAllianceLeaderboard();

    res.json({
      success: true,
      data: {
        playersUpdated: playerCount,
        alliancesUpdated: allianceCount,
      },
    });
  } catch (error: any) {
    console.error('Error updating leaderboard:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to update leaderboard',
    });
  }
});

export default router;
