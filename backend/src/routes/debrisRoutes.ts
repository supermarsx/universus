// =====================================================
// DEBRIS ROUTES
// REST API endpoints for debris, salvage, and component systems
// =====================================================

import express, { Request, Response } from 'express';
import debrisService from '../services/debrisService';
import salvageService from '../services/salvageService';
import componentService from '../services/componentService';
import { authenticateToken } from '../middleware/auth';
import pool from '../config/database';

const router = express.Router();

// All routes require authentication
router.use(authenticateToken);

// =====================================================
// DEBRIS FIELD ENDPOINTS
// =====================================================

/**
 * GET /api/debris
 * Get all active debris fields
 */
router.get('/', async (req: Request, res: Response) => {
  try {
    const limit = parseInt(req.query.limit as string) || 100;
    const debrisFields = await debrisService.getActiveDebrisFields(limit);
    
    res.json({
      success: true,
      count: debrisFields.length,
      debris: debrisFields
    });
  } catch (error) {
    console.error('Error fetching debris fields:', error);
    res.status(500).json({
      success: false,
      message: 'Failed to fetch debris fields'
    });
  }
});

/**
 * GET /api/debris/:id
 * Get debris field by ID
 */
router.get('/:id', async (req: Request, res: Response) => {
  try {
    const debrisId = parseInt(req.params.id);
    const debris = await debrisService.getDebrisById(debrisId);
    
    if (!debris) {
      return res.status(404).json({
        success: false,
        message: 'Debris field not found'
      });
    }
    
    res.json({
      success: true,
      debris
    });
  } catch (error) {
    console.error('Error fetching debris:', error);
    res.status(500).json({
      success: false,
      message: 'Failed to fetch debris field'
    });
  }
});

/**
 * GET /api/debris/location/:galaxy/:system/:position
 * Get debris at specific location
 */
router.get('/location/:galaxy/:system/:position', async (req: Request, res: Response) => {
  try {
    const galaxy = parseInt(req.params.galaxy);
    const system = parseInt(req.params.system);
    const position = parseInt(req.params.position);
    
    const debris = await debrisService.getDebrisAtLocation(galaxy, system, position);
    
    res.json({
      success: true,
      count: debris.length,
      debris
    });
  } catch (error) {
    console.error('Error fetching debris at location:', error);
    res.status(500).json({
      success: false,
      message: 'Failed to fetch debris at location'
    });
  }
});

/**
 * POST /api/debris/search
 * Search debris fields with filters
 */
router.post('/search', async (req: Request, res: Response) => {
  try {
    const filters = req.body;
    const debrisFields = await debrisService.searchDebrisFields(filters);
    
    res.json({
      success: true,
      count: debrisFields.length,
      debris: debrisFields
    });
  } catch (error) {
    console.error('Error searching debris:', error);
    res.status(500).json({
      success: false,
      message: 'Failed to search debris fields'
    });
  }
});

/**
 * POST /api/debris/generate
 * Generate debris from combat (internal use)
 */
router.post('/generate', async (req: Request, res: Response) => {
  try {
    const result = await debrisService.generateDebrisFromCombat(req.body);
    
    if (result.error) {
      return res.status(400).json({ error: result.error });
    }
    
    res.json(result);
  } catch (error) {
    console.error('Error generating debris:', error);
    res.status(500).json({
      error: 'Failed to generate debris field'
    });
  }
});

/**
 * GET /api/debris/stats
 * Get debris system statistics
 */
router.get('/system/stats', async (req: Request, res: Response) => {
  try {
    const stats = await debrisService.getDebrisSystemStats();
    
    res.json({
      success: true,
      stats
    });
  } catch (error) {
    console.error('Error fetching debris stats:', error);
    res.status(500).json({
      success: false,
      message: 'Failed to fetch debris statistics'
    });
  }
});

// =====================================================
// SALVAGE OPERATION ENDPOINTS
// =====================================================

/**
 * POST /api/debris/salvage/start
 * Start a salvage operation
 */
router.post('/salvage/start', async (req: Request, res: Response) => {
  try {
    const userId = (req as any).user.id;
    
    const request = {
      userId,
      debrisId: req.body.debrisId,
      salvageType: req.body.salvageType,
      fleetId: req.body.fleetId,
      shipTypes: req.body.shipTypes,
      cargoCapacity: req.body.cargoCapacity
    };
    
    const result = await salvageService.startSalvageOperation(request);
    
    if (result.error) {
      return res.status(400).json({ error: result.error });
    }
    
    res.json(result);
  } catch (error) {
    console.error('Error starting salvage operation:', error);
    res.status(500).json({
      error: 'Failed to start salvage operation'
    });
  }
});

/**
 * POST /api/debris/salvage/:id/complete
 * Complete a salvage operation (for testing/admin)
 */
router.post('/salvage/:id/complete', async (req: Request, res: Response) => {
  try {
    const operationId = parseInt(req.params.id);
    const result = await salvageService.completeSalvageOperation(operationId);
    
    if (!result.success) {
      return res.status(400).json(result);
    }
    
    res.json(result);
  } catch (error) {
    console.error('Error completing salvage:', error);
    res.status(500).json({
      success: false,
      message: 'Failed to complete salvage operation'
    });
  }
});

/**
 * POST /api/debris/salvage/:id/cancel
 * Cancel a salvage operation
 */
router.post('/salvage/:id/cancel', async (req: Request, res: Response) => {
  try {
    const operationId = parseInt(req.params.id);
    const userId = (req as any).user.id;
    
    const success = await salvageService.cancelSalvageOperation(operationId, userId);
    
    if (!success) {
      return res.status(400).json({
        success: false,
        message: 'Failed to cancel salvage operation'
      });
    }
    
    res.json({
      success: true,
      message: 'Salvage operation cancelled'
    });
  } catch (error) {
    console.error('Error cancelling salvage:', error);
    res.status(500).json({
      success: false,
      message: 'Failed to cancel salvage operation'
    });
  }
});

/**
 * GET /api/debris/salvage/user/active
 * Get user's active salvage operations
 */
router.get('/salvage/user/active', async (req: Request, res: Response) => {
  try {
    const userId = (req as any).user.id;
    const operations = await salvageService.getUserActiveSalvageOperations(userId);
    
    res.json({
      success: true,
      count: operations.length,
      operations
    });
  } catch (error) {
    console.error('Error fetching active salvage operations:', error);
    res.status(500).json({
      success: false,
      message: 'Failed to fetch salvage operations'
    });
  }
});

/**
 * GET /api/debris/salvage/:id
 * Get salvage operation by ID
 */
router.get('/salvage/:id', async (req: Request, res: Response) => {
  try {
    const operationId = parseInt(req.params.id);
    const operation = await salvageService.getSalvageOperationById(operationId);
    
    if (!operation) {
      return res.status(404).json({
        success: false,
        message: 'Salvage operation not found'
      });
    }
    
    res.json({
      success: true,
      operation
    });
  } catch (error) {
    console.error('Error fetching salvage operation:', error);
    res.status(500).json({
      success: false,
      message: 'Failed to fetch salvage operation'
    });
  }
});

/**
 * GET /api/debris/salvage/profile/:userId
 * Get user salvage profile
 */
router.get('/salvage/profile', async (req: Request, res: Response) => {
  try {
    const userId = (req as any).user.id;
    const profile = await salvageService.getUserSalvageProfile(userId);
    
    if (!profile) {
      return res.status(404).json({
        success: false,
        message: 'User salvage profile not found'
      });
    }
    
    res.json({
      success: true,
      profile
    });
  } catch (error) {
    console.error('Error fetching salvage profile:', error);
    res.status(500).json({
      success: false,
      message: 'Failed to fetch salvage profile'
    });
  }
});

/**
 * GET /api/debris/salvage/leaderboard
 * Get salvage leaderboard
 */
router.get('/salvage/leaderboard', async (req: Request, res: Response) => {
  try {
    const limit = parseInt(req.query.limit as string) || 100;
    const leaderboard = await salvageService.getSalvageLeaderboard(limit);
    
    res.json({
      success: true,
      count: leaderboard.length,
      leaderboard
    });
  } catch (error) {
    console.error('Error fetching salvage leaderboard:', error);
    res.status(500).json({
      success: false,
      message: 'Failed to fetch leaderboard'
    });
  }
});

/**
 * POST /api/debris/salvage/efficiency
 * Calculate salvage efficiency
 */
router.post('/salvage/efficiency', async (req: Request, res: Response) => {
  try {
    const userId = (req as any).user.id;
    const { debrisId, salvageType } = req.body;
    
    const efficiency = await salvageService.calculateSalvageEfficiency(userId, debrisId, salvageType);
    
    res.json({
      success: true,
      efficiency
    });
  } catch (error) {
    console.error('Error calculating efficiency:', error);
    res.status(500).json({
      success: false,
      message: 'Failed to calculate efficiency'
    });
  }
});

// =====================================================
// COMPONENT ENDPOINTS
// =====================================================

/**
 * GET /api/debris/components
 * Get all components (with optional filters)
 */
router.get('/components', async (req: Request, res: Response) => {
  try {
    const filters = {
      type: req.query.type as any,
      rarity: req.query.rarity as any,
      minValue: req.query.minValue ? parseInt(req.query.minValue as string) : undefined,
      maxValue: req.query.maxValue ? parseInt(req.query.maxValue as string) : undefined,
      tradeable: req.query.tradeable === 'true' ? true : undefined,
      sourceShip: req.query.sourceShip as string,
      minTechLevel: req.query.minTechLevel ? parseInt(req.query.minTechLevel as string) : undefined
    };
    
    const components = await componentService.searchComponents(filters);
    
    res.json({
      success: true,
      count: components.length,
      components
    });
  } catch (error) {
    console.error('Error fetching components:', error);
    res.status(500).json({
      success: false,
      message: 'Failed to fetch components'
    });
  }
});

/**
 * GET /api/debris/components/:id
 * Get component by ID
 */
router.get('/components/:id', async (req: Request, res: Response) => {
  try {
    const componentId = parseInt(req.params.id);
    const component = await componentService.getComponentById(componentId);
    
    if (!component) {
      return res.status(404).json({
        success: false,
        message: 'Component not found'
      });
    }
    
    res.json({
      success: true,
      component
    });
  } catch (error) {
    console.error('Error fetching component:', error);
    res.status(500).json({
      success: false,
      message: 'Failed to fetch component'
    });
  }
});

/**
 * GET /api/debris/components/inventory/my
 * Get user's component inventory
 */
router.get('/components/inventory/my', async (req: Request, res: Response) => {
  try {
    const userId = (req as any).user.id;
    const inventory = await componentService.getPlayerInventory(userId);
    
    res.json({
      success: true,
      count: inventory.length,
      inventory
    });
  } catch (error) {
    console.error('Error fetching inventory:', error);
    res.status(500).json({
      success: false,
      message: 'Failed to fetch inventory'
    });
  }
});

/**
 * GET /api/debris/components/equipped
 * Get user's equipped components
 */
router.get('/components/equipped', async (req: Request, res: Response) => {
  try {
    const userId = (req as any).user.id;
    const equipped = await componentService.getPlayerEquippedComponents(userId);
    
    res.json({
      success: true,
      count: equipped.length,
      equipped
    });
  } catch (error) {
    console.error('Error fetching equipped components:', error);
    res.status(500).json({
      success: false,
      message: 'Failed to fetch equipped components'
    });
  }
});

/**
 * POST /api/debris/components/:id/recycle
 * Recycle component for resources
 */
router.post('/components/:id/recycle', async (req: Request, res: Response) => {
  try {
    const userId = (req as any).user.id;
    const componentId = parseInt(req.params.id);
    const recycleAll = req.body.recycleAll || false;
    const quantity = req.body.quantity || 1;
    
    const result = await componentService.recycleComponent({
      component_id: componentId,
      user_id: userId,
      quantity: quantity,
      recycle_all: recycleAll
    });
    
    if (result.error) {
      return res.status(400).json({ error: result.error });
    }
    
    res.json(result);
  } catch (error) {
    console.error('Error recycling component:', error);
    res.status(500).json({
      error: 'Failed to recycle component'
    });
  }
});

/**
 * POST /api/debris/components/recycle/bulk/:rarity
 * Bulk recycle components by rarity
 */
router.post('/components/recycle/bulk/:rarity', async (req: Request, res: Response) => {
  try {
    const userId = (req as any).user.id;
    const rarity = req.params.rarity as any;
    
    const result = await componentService.bulkRecycleByRarity(userId, rarity);
    
    if (result.error) {
      return res.status(400).json({ error: result.error });
    }
    
    res.json(result);
  } catch (error) {
    console.error('Error bulk recycling:', error);
    res.status(500).json({
      error: 'Failed to bulk recycle components'
    });
  }
});

/**
 * POST /api/debris/components/:id/equip
 * Equip component to ship
 */
router.post('/components/:id/equip', async (req: Request, res: Response) => {
  try {
    const userId = (req as any).user.id;
    const componentId = parseInt(req.params.id);
    const { shipType } = req.body;
    
    if (!shipType) {
      return res.status(400).json({
        success: false,
        message: 'Ship type required'
      });
    }
    
    const success = await componentService.equipComponent(userId, componentId, shipType);
    
    if (!success) {
      return res.status(400).json({
        success: false,
        message: 'Failed to equip component'
      });
    }
    
    res.json({
      success: true,
      message: 'Component equipped successfully'
    });
  } catch (error) {
    console.error('Error equipping component:', error);
    res.status(500).json({
      success: false,
      message: 'Failed to equip component'
    });
  }
});

/**
 * POST /api/debris/components/:id/unequip
 * Unequip component
 */
router.post('/components/:id/unequip', async (req: Request, res: Response) => {
  try {
    const userId = (req as any).user.id;
    const componentId = parseInt(req.params.id);
    
    const success = await componentService.unequipComponent(userId, componentId);
    
    if (!success) {
      return res.status(400).json({
        success: false,
        message: 'Failed to unequip component'
      });
    }
    
    res.json({
      success: true,
      message: 'Component unequipped successfully'
    });
  } catch (error) {
    console.error('Error unequipping component:', error);
    res.status(500).json({
      success: false,
      message: 'Failed to unequip component'
    });
  }
});

/**
 * GET /api/debris/components/bonuses/:shipType
 * Get ship bonuses from equipped components
 */
router.get('/components/bonuses/:shipType', async (req: Request, res: Response) => {
  try {
    const userId = (req as any).user.id;
    const shipType = req.params.shipType;
    
    const bonuses = await componentService.getShipBonuses(userId, shipType);
    
    res.json({
      success: true,
      bonuses
    });
  } catch (error) {
    console.error('Error fetching ship bonuses:', error);
    res.status(500).json({
      success: false,
      message: 'Failed to fetch ship bonuses'
    });
  }
});

/**
 * POST /api/debris/components/:id/sell
 * Sell component to market
 */
router.post('/components/:id/sell', async (req: Request, res: Response) => {
  try {
    const userId = (req as any).user.id;
    const componentId = parseInt(req.params.id);
    const quantity = req.body.quantity || 1;
    
    const result = await componentService.sellComponent(userId, componentId, quantity);
    
    if (!result.success) {
      return res.status(400).json(result);
    }
    
    res.json(result);
  } catch (error) {
    console.error('Error selling component:', error);
    res.status(500).json({
      success: false,
      message: 'Failed to sell component'
    });
  }
});

/**
 * GET /api/debris/components/stats
 * Get component system statistics
 */
router.get('/components/stats', async (req: Request, res: Response) => {
  try {
    const stats = await componentService.getComponentStatistics();
    
    res.json({
      success: true,
      stats
    });
  } catch (error) {
    console.error('Error fetching component stats:', error);
    res.status(500).json({
      success: false,
      message: 'Failed to fetch component statistics'
    });
  }
});

/**
 * GET /api/debris/components/value/my
 * Get total value of user's components
 */
router.get('/components/value/my', async (req: Request, res: Response) => {
  try {
    const userId = (req as any).user.id;
    const totalValue = await componentService.getPlayerComponentValue(userId);
    
    res.json({
      success: true,
      totalValue
    });
  } catch (error) {
    console.error('Error fetching component value:', error);
    res.status(500).json({
      success: false,
      message: 'Failed to fetch component value'
    });
  }
});

// =====================================================
// DEBRIS CLAIMS ENDPOINTS
// =====================================================

/**
 * POST /api/debris/:id/claim
 * Claim a debris field
 */
router.post('/:id/claim', async (req: Request, res: Response) => {
  try {
    const userId = (req as any).user.id;
    const debrisId = parseInt(req.params.id);
    const { claimType, claimDuration, claimReason } = req.body;
    
    const result = await pool.query(
      `INSERT INTO debris_claims (
        debris_id, user_id, claim_type, claim_duration, claim_reason,
        claim_expires
      ) VALUES ($1, $2, $3, $4, $5, NOW() + INTERVAL '1 second' * $3)
      RETURNING *`,
      [debrisId, userId, claimType || 'exclusive', claimDuration || 3600, claimReason || 'Priority claim']
    );
    
    res.json({
      success: true,
      claim: result.rows[0],
      message: 'Debris field claimed successfully'
    });
  } catch (error) {
    console.error('Error claiming debris:', error);
    res.status(500).json({
      success: false,
      message: 'Failed to claim debris field'
    });
  }
});

/**
 * DELETE /api/debris/claims/:id
 * Remove a debris claim
 */
router.delete('/claims/:id', async (req: Request, res: Response) => {
  try {
    const userId = (req as any).user.id;
    const claimId = parseInt(req.params.id);
    
    const result = await pool.query(
      `UPDATE debris_claims
       SET is_active = FALSE
       WHERE id = $1 AND user_id = $2
       RETURNING id`,
      [claimId, userId]
    );
    
    if (result.rowCount === 0) {
      return res.status(404).json({
        success: false,
        message: 'Claim not found or not owned by user'
      });
    }
    
    res.json({
      success: true,
      message: 'Claim removed successfully'
    });
  } catch (error) {
    console.error('Error removing claim:', error);
    res.status(500).json({
      success: false,
      message: 'Failed to remove claim'
    });
  }
});

/**
 * GET /api/debris/claims/my
 * Get user's active claims
 */
router.get('/claims/my', async (req: Request, res: Response) => {
  try {
    const userId = (req as any).user.id;
    
    const result = await pool.query(
      `SELECT * FROM debris_claims
       WHERE user_id = $1 AND is_active = TRUE
       ORDER BY claim_start DESC`,
      [userId]
    );
    
    res.json({
      success: true,
      count: result.rows.length,
      claims: result.rows
    });
  } catch (error) {
    console.error('Error fetching claims:', error);
    res.status(500).json({
      success: false,
      message: 'Failed to fetch claims'
    });
  }
});

export default router;
