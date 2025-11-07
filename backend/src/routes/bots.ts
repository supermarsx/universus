import express, { Router, Request, Response } from 'express';
import { authenticateToken } from '../middleware/auth';
import { BotService } from '../services/botService';
import { BotAIService } from '../services/botAIService';

const router: Router = express.Router();

// Admin authorization middleware
const adminMiddleware = async (req: Request, res: Response, next: Function) => {
  try {
    const userId = (req as any).user?.id;
    
    if (!userId) {
      return res.status(401).json({ error: 'Not authenticated' });
    }
    
    // Check if user is admin (from previous admin system)
    const { pool } = await import('../config/database');
    const result = await pool.query(
      'SELECT is_admin FROM users WHERE id = $1',
      [userId]
    );
    
    if (!result.rows[0]?.is_admin) {
      return res.status(403).json({ error: 'Admin access required' });
    }
    
    next();
  } catch (error) {
    res.status(500).json({ error: 'Authorization check failed' });
  }
};

/**
 * GET /api/admin/bots
 * Get all bots with optional filtering
 */
router.get('/', authenticateToken, adminMiddleware, async (req: Request, res: Response) => {
  try {
    const filters: any = {};
    
    if (req.query.is_active !== undefined) {
      filters.is_active = req.query.is_active === 'true';
    }
    
    if (req.query.personality_type) {
      filters.personality_type = req.query.personality_type as string;
    }
    
    if (req.query.min_difficulty) {
      filters.min_difficulty = parseInt(req.query.min_difficulty as string);
    }
    
    if (req.query.max_difficulty) {
      filters.max_difficulty = parseInt(req.query.max_difficulty as string);
    }
    
    const bots = await BotService.getAllBots(filters);
    
    res.json({
      success: true,
      data: bots
    });
  } catch (error) {
    console.error('Error fetching bots:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to fetch bots'
    });
  }
});

/**
 * GET /api/admin/bots/:id
 * Get bot by ID with detailed information
 */
router.get('/:id', authenticateToken, adminMiddleware, async (req: Request, res: Response) => {
  try {
    const botId = parseInt(req.params.id);
    
    const bot = await BotService.getBotById(botId);
    
    if (!bot) {
      return res.status(404).json({
        success: false,
        error: 'Bot not found'
      });
    }
    
    // Get recent actions
    const recentActions = await BotService.getActionHistory(botId, 50);
    
    // Get statistics
    const statistics = await BotService.getStatistics(botId);
    
    // Get targets
    const targets = await BotService.getTargets(botId, 20);
    
    res.json({
      success: true,
      data: {
        bot,
        recentActions,
        statistics,
        targets
      }
    });
  } catch (error) {
    console.error('Error fetching bot details:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to fetch bot details'
    });
  }
});

/**
 * POST /api/admin/bots
 * Create a new bot
 */
router.post('/', authenticateToken, adminMiddleware, async (req: Request, res: Response) => {
  try {
    const { username, email, personality_type, difficulty_level } = req.body;
    
    if (!username || !email || !personality_type) {
      return res.status(400).json({
        success: false,
        error: 'Missing required fields: username, email, personality_type'
      });
    }
    
    const validPersonalities = [
      'aggressive_conqueror',
      'strategic_builder',
      'diplomatic_negotiator',
      'resource_hoarder',
      'speed_rusher',
      'tech_enthusiast',
      'alliance_focused',
      'solo_survivor'
    ];
    
    if (!validPersonalities.includes(personality_type)) {
      return res.status(400).json({
        success: false,
        error: 'Invalid personality type'
      });
    }
    
    const difficultyLvl = difficulty_level || 5;
    if (difficultyLvl < 1 || difficultyLvl > 10) {
      return res.status(400).json({
        success: false,
        error: 'Difficulty level must be between 1 and 10'
      });
    }
    
    const bot = await BotService.createBot(
      username,
      email,
      personality_type,
      difficultyLvl
    );
    
    res.status(201).json({
      success: true,
      data: bot
    });
  } catch (error: any) {
    console.error('Error creating bot:', error);
    
    if (error.message?.includes('duplicate')) {
      return res.status(409).json({
        success: false,
        error: 'Username or email already exists'
      });
    }
    
    res.status(500).json({
      success: false,
      error: 'Failed to create bot'
    });
  }
});

/**
 * PUT /api/admin/bots/:id
 * Update bot configuration
 */
router.put('/:id', authenticateToken, adminMiddleware, async (req: Request, res: Response) => {
  try {
    const botId = parseInt(req.params.id);
    const updates = req.body;
    
    const bot = await BotService.updateBot(botId, updates);
    
    res.json({
      success: true,
      data: bot
    });
  } catch (error) {
    console.error('Error updating bot:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to update bot'
    });
  }
});

/**
 * DELETE /api/admin/bots/:id
 * Delete a bot
 */
router.delete('/:id', authenticateToken, adminMiddleware, async (req: Request, res: Response) => {
  try {
    const botId = parseInt(req.params.id);
    
    await BotService.deleteBot(botId);
    
    res.json({
      success: true,
      message: 'Bot deleted successfully'
    });
  } catch (error) {
    console.error('Error deleting bot:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to delete bot'
    });
  }
});

/**
 * POST /api/admin/bots/bulk
 * Bulk create bots
 */
router.post('/bulk', authenticateToken, adminMiddleware, async (req: Request, res: Response) => {
  try {
    const { count, personality_type, difficulty_level } = req.body;
    
    if (!count || !personality_type) {
      return res.status(400).json({
        success: false,
        error: 'Missing required fields: count, personality_type'
      });
    }
    
    if (count < 1 || count > 50) {
      return res.status(400).json({
        success: false,
        error: 'Count must be between 1 and 50'
      });
    }
    
    const created = await BotService.bulkCreateBots(
      count,
      personality_type,
      difficulty_level || 5
    );
    
    res.status(201).json({
      success: true,
      data: {
        requested: count,
        created: created
      }
    });
  } catch (error) {
    console.error('Error bulk creating bots:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to bulk create bots'
    });
  }
});

/**
 * GET /api/admin/bots/:id/actions
 * Get bot action history
 */
router.get('/:id/actions', authenticateToken, adminMiddleware, async (req: Request, res: Response) => {
  try {
    const botId = parseInt(req.params.id);
    const limit = parseInt(req.query.limit as string) || 100;
    const actionType = req.query.action_type as string;
    
    const actions = await BotService.getActionHistory(botId, limit, actionType);
    
    res.json({
      success: true,
      data: actions
    });
  } catch (error) {
    console.error('Error fetching bot actions:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to fetch bot actions'
    });
  }
});

/**
 * GET /api/admin/bots/:id/statistics
 * Get bot statistics
 */
router.get('/:id/statistics', authenticateToken, adminMiddleware, async (req: Request, res: Response) => {
  try {
    const botId = parseInt(req.params.id);
    const startDate = req.query.start_date ? new Date(req.query.start_date as string) : undefined;
    const endDate = req.query.end_date ? new Date(req.query.end_date as string) : undefined;
    
    const statistics = await BotService.getStatistics(botId, startDate, endDate);
    
    res.json({
      success: true,
      data: statistics
    });
  } catch (error) {
    console.error('Error fetching bot statistics:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to fetch bot statistics'
    });
  }
});

/**
 * GET /api/admin/bots/leaderboard
 * Get bot leaderboard
 */
router.get('/leaderboard/top', authenticateToken, adminMiddleware, async (req: Request, res: Response) => {
  try {
    const limit = parseInt(req.query.limit as string) || 20;
    
    const leaderboard = await BotService.getLeaderboard(limit);
    
    res.json({
      success: true,
      data: leaderboard
    });
  } catch (error) {
    console.error('Error fetching bot leaderboard:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to fetch bot leaderboard'
    });
  }
});

/**
 * POST /api/admin/bots/:id/think
 * Force a bot to think immediately
 */
router.post('/:id/think', authenticateToken, adminMiddleware, async (req: Request, res: Response) => {
  try {
    const botId = parseInt(req.params.id);
    
    const bot = await BotService.getBotById(botId);
    
    if (!bot) {
      return res.status(404).json({
        success: false,
        error: 'Bot not found'
      });
    }
    
    // Trigger think cycle asynchronously
    BotAIService.think(bot).catch(err => {
      console.error(`Error in forced think for bot ${botId}:`, err);
    });
    
    res.json({
      success: true,
      message: 'Bot think cycle triggered'
    });
  } catch (error) {
    console.error('Error triggering bot think:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to trigger bot think'
    });
  }
});

/**
 * POST /api/admin/bots/process-all
 * Manually trigger processing of all bots
 */
router.post('/process/all', authenticateToken, adminMiddleware, async (req: Request, res: Response) => {
  try {
    // Trigger async processing
    BotAIService.processAllBots().catch(err => {
      console.error('Error in manual bot processing:', err);
    });
    
    res.json({
      success: true,
      message: 'Bot processing triggered'
    });
  } catch (error) {
    console.error('Error triggering bot processing:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to trigger bot processing'
    });
  }
});

/**
 * GET /api/admin/bots/personalities/list
 * Get list of available bot personalities
 */
router.get('/personalities/list', authenticateToken, adminMiddleware, async (req: Request, res: Response) => {
  try {
    const personalities = [
      {
        type: 'aggressive_conqueror',
        name: 'Aggressive Conqueror',
        description: 'Prioritizes military expansion, frequent attacks, rapid fleet building',
        traits: { aggression: 90, military: 95, economy: 40 }
      },
      {
        type: 'strategic_builder',
        name: 'Strategic Builder',
        description: 'Focuses on infrastructure, balanced development, defensive strategies',
        traits: { aggression: 30, economy: 85, research: 75 }
      },
      {
        type: 'diplomatic_negotiator',
        name: 'Diplomatic Negotiator',
        description: 'Alliance-focused, trade-oriented, peaceful expansion',
        traits: { diplomacy: 95, aggression: 15, economy: 70 }
      },
      {
        type: 'resource_hoarder',
        name: 'Resource Hoarder',
        description: 'Maximum resource gathering, conservative playstyle, long-term planning',
        traits: { economy: 95, risk_tolerance: 15, aggression: 10 }
      },
      {
        type: 'speed_rusher',
        name: 'Speed Rusher',
        description: 'Early game aggression, rapid technology advancement, timing-based attacks',
        traits: { aggression: 95, military: 90, risk_tolerance: 90 }
      },
      {
        type: 'tech_enthusiast',
        name: 'Tech Enthusiast',
        description: 'Research-focused, advanced technology, innovative strategies',
        traits: { research: 95, economy: 75, aggression: 35 }
      },
      {
        type: 'alliance_focused',
        name: 'Alliance-Focused',
        description: 'Team player, supports allies, coordinated attacks',
        traits: { diplomacy: 90, military: 65, economy: 65 }
      },
      {
        type: 'solo_survivor',
        name: 'Solo Survivor',
        description: 'Independent play, self-sufficiency, defensive positioning',
        traits: { economy: 80, military: 70, diplomacy: 30 }
      }
    ];
    
    res.json({
      success: true,
      data: personalities
    });
  } catch (error) {
    console.error('Error fetching personalities:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to fetch personalities'
    });
  }
});

export default router;
