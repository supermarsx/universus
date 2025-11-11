/**
 * @module backend/routes/universeRoutes
 *
 * Universe management and seeding endpoints. Includes admin-only routes for
 * generating and managing universes and utility endpoints used during
 * server bootstrap and testing.
 */

// =====================================================
// UNIVERSE SEEDING ROUTES
// REST API endpoints for universe management
// =====================================================

import express, { Response } from 'express';
import { AuthRequest } from '../types';
import { getUserId } from '../utils/authHelpers';
import universeSeedingService from '../services/universeSeedingService';
import playerPlacementService from '../services/playerPlacementService';
import universeMaintenanceService from '../services/universeMaintenanceService';
import { authenticateToken } from '../middleware/auth';
import { requirePermission } from '../middleware/adminAuth';

const BOT_SERVICE_URL = process.env.BOT_SERVICE_URL || 'http://bot-service:4001';

const router = express.Router();

// All routes require authentication
router.use(authenticateToken);



// =====================================================
// UNIVERSE MANAGEMENT ENDPOINTS
// =====================================================

/**
 * GET /api/universe
 * Get all universes
 */
router.get('/', async (req: AuthRequest, res: Response) => {
  try {
    const universes = await universeSeedingService.getAllUniverses();
    
    res.json({
      success: true,
      count: universes.length,
      universes
    });
  } catch (error) {
    console.error('Error fetching universes:', error);
    res.status(500).json({
      success: false,
      message: 'Failed to fetch universes'
    });
  }
});

/**
 * GET /api/universe/:id
 * Get universe by ID
 */
router.get('/:id', async (req: AuthRequest, res: Response) => {
  try {
    const universeId = parseInt(req.params.id);
    const universe = await universeSeedingService.getUniverseById(universeId);
    
    if (!universe) {
      return res.status(404).json({
        success: false,
        message: 'Universe not found'
      });
    }
    
    res.json({
      success: true,
      universe
    });
  } catch (error) {
    console.error('Error fetching universe:', error);
    res.status(500).json({
      success: false,
      message: 'Failed to fetch universe'
    });
  }
});

/**
 * POST /api/universe/create
 * Create a new universe
 */
router.post('/create', async (req: AuthRequest, res: Response) => {
  try {
    const result = await universeSeedingService.createUniverse(req.body);
    
    if (!result.success) {
      return res.status(400).json(result);
    }
    
    res.json(result);
  } catch (error) {
    console.error('Error creating universe:', error);
    res.status(500).json({
      success: false,
      message: 'Failed to create universe'
    });
  }
});

/**
 * POST /api/universe/:id/seed
 * Seed a universe with galaxies, bots, etc.
 */
router.post('/:id/seed', async (req: AuthRequest, res: Response) => {
  try {
    const universeId = parseInt(req.params.id);
    
    const seedRequest = {
      universeId,
      generateGalaxies: req.body.generateGalaxies !== false,
      generateBots: req.body.generateBots !== false,
      generateAlliances: req.body.generateAlliances !== false,
      distributeResources: req.body.distributeResources !== false
    };
    
    const result = await universeSeedingService.seedUniverse(seedRequest);
    
    if (!result.success) {
      return res.status(400).json(result);
    }
    
    res.json(result);
  } catch (error) {
    console.error('Error seeding universe:', error);
    res.status(500).json({
      success: false,
      message: 'Failed to seed universe'
    });
  }
});

/**
 * GET /api/universe/:id/galaxies
 * Get all galaxies in a universe
 */
router.get('/:id/galaxies', async (req: AuthRequest, res: Response) => {
  try {
    const universeId = parseInt(req.params.id);
    const galaxies = await universeSeedingService.getGalaxiesForUniverse(universeId);
    
    res.json({
      success: true,
      count: galaxies.length,
      galaxies
    });
  } catch (error) {
    console.error('Error fetching galaxies:', error);
    res.status(500).json({
      success: false,
      message: 'Failed to fetch galaxies'
    });
  }
});

// =====================================================
// PLAYER PLACEMENT ENDPOINTS
// =====================================================

/**
 * POST /api/universe/:id/place-player
 * Place a player in the universe
 */
router.post('/:id/place-player', async (req: AuthRequest, res: Response) => {
  try {
    const universeId = parseInt(req.params.id);
    const userId = getUserId(req);
    if (userId === null) {
      return res.status(401).json({ success: false, message: 'Unauthorized' });
    }
    
    const placementRequest = {
      universeId,
      userId,
      preferredPlaystyle: req.body.preferredPlaystyle,
      allianceId: req.body.allianceId,
      useCustomLocation: req.body.useCustomLocation,
      customGalaxy: req.body.customGalaxy,
      customSystem: req.body.customSystem
    };
    
    const result = await playerPlacementService.placePlayer(placementRequest);
    
    if (!result.success) {
      return res.status(400).json(result);
    }
    
    res.json(result);
  } catch (error) {
    console.error('Error placing player:', error);
    res.status(500).json({
      success: false,
      message: 'Failed to place player'
    });
  }
});

/**
 * GET /api/universe/:id/placements
 * Get all player placements in universe
 */
router.get('/:id/placements', async (req: AuthRequest, res: Response) => {
  try {
    const universeId = parseInt(req.params.id);
    const placements = await playerPlacementService.getUniversePlacements(universeId);
    
    res.json({
      success: true,
      count: placements.length,
      placements
    });
  } catch (error) {
    console.error('Error fetching placements:', error);
    res.status(500).json({
      success: false,
      message: 'Failed to fetch placements'
    });
  }
});

/**
 * GET /api/universe/:id/my-placement
 * Get current user's placement in universe
 */
router.get('/:id/my-placement', async (req: AuthRequest, res: Response) => {
  try {
    const universeId = parseInt(req.params.id);
    const userId = getUserId(req);
    if (userId === null) {
      return res.status(401).json({ success: false, message: 'Unauthorized' });
    }
    
    const placement = await playerPlacementService.getPlayerPlacement(userId, universeId);
    
    if (!placement) {
      return res.status(404).json({
        success: false,
        message: 'Placement not found'
      });
    }
    
    res.json({
      success: true,
      placement
    });
  } catch (error) {
    console.error('Error fetching placement:', error);
    res.status(500).json({
      success: false,
      message: 'Failed to fetch placement'
    });
  }
});

// =====================================================
// BOT GENERATION ENDPOINTS
// =====================================================

/**
 * POST /api/universe/:id/generate-bots
 * Generate bots for the universe
 */
router.post('/:id/generate-bots', requirePermission('universe:generate_bots'), async (req: AuthRequest, res: Response) => {
  try {
    const universeId = parseInt(req.params.id, 10);
    const targetUrl = `${BOT_SERVICE_URL}/api/admin/bots/universe/${universeId}/generate`;

    const response = await fetch(targetUrl, {
      method: 'POST',
      headers: {
        'content-type': 'application/json',
        authorization: req.headers.authorization || '',
      },
      body: JSON.stringify(req.body),
    });

    const payload = await response.json();

    if (!response.ok) {
      return res.status(response.status).json(payload);
    }

    res.json(payload);
  } catch (error) {
    console.error('Error proxying bot generation:', error);
    res.status(502).json({
      success: false,
      message: 'Bot service unavailable',
    });
  }
});

// =====================================================
// MAINTENANCE ENDPOINTS
// =====================================================

/**
 * POST /api/universe/:id/maintenance/population-balance
 * Run population balance maintenance
 */
router.post('/:id/maintenance/population-balance', async (req: AuthRequest, res: Response) => {
  try {
    const universeId = parseInt(req.params.id);
    const result = await universeMaintenanceService.runPopulationBalance(universeId);
    
    res.json(result);
  } catch (error) {
    console.error('Error running maintenance:', error);
    res.status(500).json({
      success: false,
      message: 'Failed to run maintenance'
    });
  }
});

/**
 * POST /api/universe/:id/maintenance/start
 * Start automatic maintenance for universe
 */
router.post('/:id/maintenance/start', async (req: AuthRequest, res: Response) => {
  try {
    const universeId = parseInt(req.params.id);
    
    universeMaintenanceService.startAutomaticMaintenance(universeId);
    
    res.json({
      success: true,
      message: `Automatic maintenance started for universe ${universeId}`
    });
  } catch (error) {
    console.error('Error starting maintenance:', error);
    res.status(500).json({
      success: false,
      message: 'Failed to start maintenance'
    });
  }
});

// =====================================================
// STATISTICS ENDPOINTS
// =====================================================

/**
 * GET /api/universe/:id/stats
 * Get universe statistics
 */
router.get('/:id/stats', async (req: AuthRequest, res: Response) => {
  try {
    const universeId = parseInt(req.params.id);
    
    // Get basic stats from database
    const statsResult = await require('../config/database').default.query(
      `SELECT 
        us.*,
        (SELECT COUNT(*) FROM player_placements WHERE universe_id = us.id) as player_count,
        (SELECT COUNT(*) FROM generated_bots WHERE universe_id = us.id AND is_active = TRUE) as bot_count,
        (SELECT COUNT(*) FROM galaxy_seeds WHERE universe_id = us.id AND is_generated = TRUE) as galaxy_count
       FROM universe_seeds us
       WHERE us.id = $1`,
      [universeId]
    );
    
    if (statsResult.rows.length === 0) {
      return res.status(404).json({
        success: false,
        message: 'Universe not found'
      });
    }
    
    res.json({
      success: true,
      stats: statsResult.rows[0]
    });
  } catch (error) {
    console.error('Error fetching stats:', error);
    res.status(500).json({
      success: false,
      message: 'Failed to fetch statistics'
    });
  }
});

export default router;
