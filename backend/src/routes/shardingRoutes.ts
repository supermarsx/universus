// =====================================================
// SHARDING API ROUTES
// Enterprise server sharding endpoints
// =====================================================

import express, { Request, Response } from 'express';
import { AuthRequest } from '../types';
import { authenticateToken } from '../middleware/auth';
import { requireAdmin } from '../middleware/adminAuth';
import serverDiscoveryService from '../services/serverDiscoveryService';
import playerRoutingService from '../services/playerRoutingService';
import crossServerCommunication from '../services/crossServerCommunicationService';
import globalLeaderboardService from '../services/globalLeaderboardService';
import { getUserId } from '../utils/authHelpers';

const router = express.Router();

// =====================================================
// SERVER MANAGEMENT ENDPOINTS
// =====================================================

/**
 * GET /api/shards/servers - List all servers
 */
router.get('/servers', requireAdmin, async (req: Request, res: Response) => {
  try {
    const servers = await serverDiscoveryService.getAllServers();
    res.json({ success: true, data: servers });
  } catch (error: any) {
    res.status(500).json({ success: false, error: error.message });
  }
});

/**
 * POST /api/shards/servers/register - Register new server
 */
router.post('/servers/register', requireAdmin, async (req: Request, res: Response) => {
  try {
    const server = await serverDiscoveryService.registerServer(req.body);
    res.json({ success: true, data: server });
  } catch (error: any) {
    res.status(500).json({ success: false, error: error.message });
  }
});

/**
 * GET /api/shards/servers/:id/health - Get server health
 */
router.get('/servers/:id/health', requireAdmin, async (req: Request, res: Response) => {
  try {
    const health = await serverDiscoveryService.checkServerHealth(req.params.id);
    res.json({ success: true, data: health });
  } catch (error: any) {
    res.status(500).json({ success: false, error: error.message });
  }
});

/**
 * PUT /api/shards/servers/:id/config - Update server config
 */
router.put('/servers/:id/config', requireAdmin, async (req: Request, res: Response) => {
  try {
    const server = await serverDiscoveryService.updateServerConfig(req.params.id, req.body);
    res.json({ success: true, data: server });
  } catch (error: any) {
    res.status(500).json({ success: false, error: error.message });
  }
});

/**
 * POST /api/shards/servers/:id/heartbeat - Record heartbeat
 */
router.post('/servers/:id/heartbeat', async (req: Request, res: Response) => {
  try {
    await serverDiscoveryService.recordHeartbeat(req.params.id);
    res.json({ success: true });
  } catch (error: any) {
    res.status(500).json({ success: false, error: error.message });
  }
});

/**
 * PUT /api/shards/servers/:id/health - Update server health
 */
router.put('/servers/:id/health', async (req: Request, res: Response) => {
  try {
    await serverDiscoveryService.updateServerHealth({
      server_id: req.params.id,
      ...req.body
    });
    res.json({ success: true });
  } catch (error: any) {
    res.status(500).json({ success: false, error: error.message });
  }
});

/**
 * DELETE /api/shards/servers/:id - Deregister server
 */
router.delete('/servers/:id', requireAdmin, async (req: Request, res: Response) => {
  try {
    await serverDiscoveryService.deregisterServer(req.params.id);
    res.json({ success: true });
  } catch (error: any) {
    res.status(500).json({ success: false, error: error.message });
  }
});

/**
 * GET /api/shards/servers/stats - Get server statistics
 */
router.get('/servers/stats', requireAdmin, async (req: Request, res: Response) => {
  try {
    const stats = await serverDiscoveryService.getServerStatistics();
    res.json({ success: true, data: stats });
  } catch (error: any) {
    res.status(500).json({ success: false, error: error.message });
  }
});

// =====================================================
// PLAYER ROUTING ENDPOINTS
// =====================================================

/**
 * POST /api/shards/routing/calculate - Calculate optimal server
 */
router.post('/routing/calculate', authenticateToken, async (req: AuthRequest, res: Response) => {
  try {
    const userId = getUserId(req);
    if (userId === null) return res.status(401).json({ success: false, error: 'Unauthorized' });

    const routing = await playerRoutingService.routePlayer({
      user_id: userId,
      ...req.body
    });
    res.json({ success: true, data: routing });
  } catch (error: any) {
    res.status(500).json({ success: false, error: error.message });
  }
});

/**
 * POST /api/shards/routing/migrate - Migrate player
 */
router.post('/routing/migrate', requireAdmin, async (req: Request, res: Response) => {
  try {
    const result = await playerRoutingService.migratePlayer(req.body);
    res.json({ success: true, data: result });
  } catch (error: any) {
    res.status(500).json({ success: false, error: error.message });
  }
});

/**
 * GET /api/shards/routing/player/:id - Get player routing
 */
router.get('/routing/player/:id', authenticateToken, async (req: Request, res: Response) => {
  try {
    const assignment = await playerRoutingService.getPlayerAssignment(parseInt(req.params.id));
    res.json({ success: true, data: assignment });
  } catch (error: any) {
    res.status(500).json({ success: false, error: error.message });
  }
});

/**
 * GET /api/shards/routing/servers/available - Available servers
 */
router.get('/routing/servers/available', async (req: Request, res: Response) => {
  try {
    const servers = await serverDiscoveryService.getHealthyServers();
    res.json({ success: true, data: servers });
  } catch (error: any) {
    res.status(500).json({ success: false, error: error.message });
  }
});

/**
 * POST /api/shards/routing/balance - Auto-balance players
 */
router.post('/routing/balance', requireAdmin, async (req: Request, res: Response) => {
  try {
    const count = await playerRoutingService.autoBalancePlayers();
    res.json({ success: true, data: { migrated_count: count } });
  } catch (error: any) {
    res.status(500).json({ success: false, error: error.message });
  }
});

/**
 * GET /api/shards/routing/stats - Routing statistics
 */
router.get('/routing/stats', requireAdmin, async (req: Request, res: Response) => {
  try {
    const stats = await playerRoutingService.getRoutingStatistics();
    res.json({ success: true, data: stats });
  } catch (error: any) {
    res.status(500).json({ success: false, error: error.message });
  }
});

// =====================================================
// CROSS-SERVER COMMUNICATION ENDPOINTS
// =====================================================

/**
 * POST /api/shards/messages/broadcast - Broadcast message
 */
router.post('/messages/broadcast', requireAdmin, async (req: Request, res: Response) => {
  try {
    await crossServerCommunication.broadcastToAllServers(
      req.body.message_type,
      req.body.payload,
      req.body.priority
    );
    res.json({ success: true });
  } catch (error: any) {
    res.status(500).json({ success: false, error: error.message });
  }
});

/**
 * POST /api/shards/messages/send - Send to specific servers
 */
router.post('/messages/send', requireAdmin, async (req: Request, res: Response) => {
  try {
    await crossServerCommunication.sendToServers(
      req.body.target_servers,
      req.body.message_type,
      req.body.payload,
      req.body.priority
    );
    res.json({ success: true });
  } catch (error: any) {
    res.status(500).json({ success: false, error: error.message });
  }
});

/**
 * GET /api/shards/messages/history - Event history
 */
router.get('/messages/history', requireAdmin, async (req: Request, res: Response) => {
  try {
    const history = await crossServerCommunication.getEventHistory(
      parseInt(req.query.limit as string) || 100,
      req.query.type as string,
      req.query.server_id as string
    );
    res.json({ success: true, data: history });
  } catch (error: any) {
    res.status(500).json({ success: false, error: error.message });
  }
});

/**
 * GET /api/shards/messages/stats - Messaging statistics
 */
router.get('/messages/stats', requireAdmin, async (req: Request, res: Response) => {
  try {
    const stats = await crossServerCommunication.getMessagingStatistics();
    res.json({ success: true, data: stats });
  } catch (error: any) {
    res.status(500).json({ success: false, error: error.message });
  }
});

/**
 * GET /api/shards/messages/status - Communication status
 */
router.get('/messages/status', requireAdmin, async (req: Request, res: Response) => {
  try {
    const status = crossServerCommunication.getStatus();
    res.json({ success: true, data: status });
  } catch (error: any) {
    res.status(500).json({ success: false, error: error.message });
  }
});

// =====================================================
// GLOBAL LEADERBOARD ENDPOINTS
// =====================================================

/**
 * GET /api/shards/leaderboards/:category - Get leaderboard
 */
router.get('/leaderboards/:category', async (req: Request, res: Response) => {
  try {
    const leaderboard = await globalLeaderboardService.getGlobalLeaderboard({
      category: req.params.category as any,
      limit: parseInt(req.query.limit as string) || 50,
      offset: parseInt(req.query.offset as string) || 0,
      server_id: req.query.server_id as string,
      alliance_id: req.query.alliance_id ? parseInt(req.query.alliance_id as string) : undefined
    });
    res.json({ success: true, data: leaderboard });
  } catch (error: any) {
    res.status(500).json({ success: false, error: error.message });
  }
});

/**
 * POST /api/shards/leaderboards/update - Update player ranking
 */
router.post('/leaderboards/update', authenticateToken, async (req: AuthRequest, res: Response) => {
  try {
    const userId = getUserId(req);
    if (userId === null) return res.status(401).json({ success: false, error: 'Unauthorized' });

    await globalLeaderboardService.updatePlayerEntry(
      userId,
      req.body.server_id,
      req.body.category,
      req.body.score,
      req.body.alliance_id
    );
    res.json({ success: true });
  } catch (error: any) {
    res.status(500).json({ success: false, error: error.message });
  }
});

/**
 * GET /api/shards/leaderboards/:category/player/:id - Get player rank
 */
router.get('/leaderboards/:category/player/:id', async (req: Request, res: Response) => {
  try {
    const rank = await globalLeaderboardService.getPlayerRank(
      parseInt(req.params.id),
      req.params.category as any
    );
    res.json({ success: true, data: rank });
  } catch (error: any) {
    res.status(500).json({ success: false, error: error.message });
  }
});

/**
 * GET /api/shards/leaderboards/:category/top - Get top players
 */
router.get('/leaderboards/:category/top', async (req: Request, res: Response) => {
  try {
    const topPlayers = await globalLeaderboardService.getTopPlayers(
      req.params.category as any,
      parseInt(req.query.limit as string) || 10
    );
    res.json({ success: true, data: topPlayers });
  } catch (error: any) {
    res.status(500).json({ success: false, error: error.message });
  }
});

/**
 * POST /api/shards/leaderboards/snapshot - Create snapshot
 */
router.post('/leaderboards/snapshot', requireAdmin, async (req: Request, res: Response) => {
  try {
    await globalLeaderboardService.createDailySnapshot();
    res.json({ success: true });
  } catch (error: any) {
    res.status(500).json({ success: false, error: error.message });
  }
});

/**
 * GET /api/shards/leaderboards/stats - Leaderboard statistics
 */
router.get('/leaderboards/stats', requireAdmin, async (req: Request, res: Response) => {
  try {
    const stats = await globalLeaderboardService.getStatistics();
    res.json({ success: true, data: stats });
  } catch (error: any) {
    res.status(500).json({ success: false, error: error.message });
  }
});

// =====================================================
// HEALTH & MONITORING ENDPOINTS
// =====================================================

/**
 * GET /api/shards/health/overview - System health overview
 */
router.get('/health/overview', async (req: Request, res: Response) => {
  try {
    const stats = await serverDiscoveryService.getServerStatistics();
    const routingStats = await playerRoutingService.getRoutingStatistics();
    const messagingStats = await crossServerCommunication.getMessagingStatistics();
    
    res.json({
      success: true,
      data: {
        servers: stats,
        routing: routingStats,
        messaging: messagingStats,
        timestamp: new Date()
      }
    });
  } catch (error: any) {
    res.status(500).json({ success: false, error: error.message });
  }
});

/**
 * GET /api/shards/health/servers - All server health
 */
router.get('/health/servers', requireAdmin, async (req: Request, res: Response) => {
  try {
    const servers = await serverDiscoveryService.getAllServers();
    const healthChecks = await Promise.all(
      servers.map(s => serverDiscoveryService.checkServerHealth(s.server_id))
    );
    res.json({ success: true, data: healthChecks });
  } catch (error: any) {
    res.status(500).json({ success: false, error: error.message });
  }
});

export default router;
