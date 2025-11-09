import { Router, Response } from 'express';
import os from 'os';
import { AdminAuthRequest } from '../types/admin';
import { 
  requireAdmin,
  requirePermission,
  requirePermissions,
  rateLimit,
} from '../middleware/adminAuth';
import { AdminUserService } from '../services/adminUserService';
import { AdminMonitoringService } from '../services/adminMonitoringService';
import { 
  AdminSettingsService,
  AdminEventsService,
  AdminAnalyticsService,
} from '../services/adminSettingsService';
import { pool } from '../config/database';
import { redis } from '../config/redis';
import LeaderboardScheduler from '../services/leaderboardScheduler';
import { getRealtimeHandler } from '../socket';
import chatService from '../services/chatService';

const router = Router();

// ========================================
// ADMIN DASHBOARD
// ========================================

/**
 * GET /api/admin/dashboard
 * Get comprehensive admin dashboard data
 */
router.get(
  '/dashboard',
  requireAdmin,
  async (req: AdminAuthRequest, res: Response): Promise<void> => {
    try {
      const [
        serverHealth,
        userAnalytics,
        resourceAnalytics,
        combatAnalytics,
        recentLogs,
        activeEvents,
        notifications,
        onlineAdmins,
      ] = await Promise.all([
        AdminMonitoringService.getServerHealth(),
        AdminUserService.getUserAnalytics(),
        AdminAnalyticsService.getResourceAnalytics(),
        AdminAnalyticsService.getCombatAnalytics(),
        pool.query(
          'SELECT * FROM admin_audit_logs ORDER BY timestamp DESC LIMIT 20'
        ),
        AdminEventsService.getActiveEvents(),
        AdminMonitoringService.getNotifications(
          req.user!.id,
          req.adminLevel!,
          true
        ),
        AdminMonitoringService.getOnlineAdminsCount(),
      ]);

      res.json({
        server_health: serverHealth,
        user_analytics: userAnalytics,
        resource_analytics: resourceAnalytics,
        combat_analytics: combatAnalytics,
        recent_audit_logs: recentLogs.rows,
        active_events: activeEvents,
        pending_reports: 0,
        critical_alerts: notifications.filter((n) => n.priority === 'critical'),
        online_admins: onlineAdmins,
      });
    } catch (error: any) {
      console.error('Dashboard error:', error);
      res.status(500).json({ error: error.message });
    }
  }
);

// ========================================
// USER MANAGEMENT
// ========================================

/**
 * GET /api/admin/users
 * Get all users with filtering and pagination
 */
router.get(
  '/users',
  requireAdmin,
  requirePermission('user:read'),
  async (req: AdminAuthRequest, res: Response): Promise<void> => {
    try {
      const filter = {
        search: req.query.search as string,
        status: req.query.status as string,
        dateFrom: req.query.dateFrom ? new Date(req.query.dateFrom as string) : undefined,
        dateTo: req.query.dateTo ? new Date(req.query.dateTo as string) : undefined,
        page: parseInt(req.query.page as string) || 1,
        limit: parseInt(req.query.limit as string) || 50,
        sortBy: req.query.sortBy as string,
        sortOrder: req.query.sortOrder as 'ASC' | 'DESC',
      };

      const result = await AdminUserService.getUsers(filter);
      res.json(result);
    } catch (error: any) {
      console.error('Get users error:', error);
      res.status(500).json({ error: error.message });
    }
  }
);

/**
 * GET /api/admin/users/:id
 * Get detailed user information
 */
router.get(
  '/users/:id',
  requireAdmin,
  requirePermission('user:read'),
  async (req: AdminAuthRequest, res: Response): Promise<void> => {
    try {
      const userId = parseInt(req.params.id);
      const user = await AdminUserService.getUserDetails(userId);
      res.json(user);
    } catch (error: any) {
      console.error('Get user details error:', error);
      res.status(404).json({ error: error.message });
    }
  }
);

/**
 * POST /api/admin/users/:id/block
 * Block/ban a user
 */
router.post(
  '/users/:id/block',
  requireAdmin,
  requirePermission('user:ban'),
  rateLimit(10, 60000),
  async (req: AdminAuthRequest, res: Response): Promise<void> => {
    try {
      const userId = parseInt(req.params.id);
      const action = {
        user_id: userId,
        block_type: req.body.block_type,
        reason: req.body.reason,
        duration_minutes: req.body.duration_minutes,
        is_permanent: req.body.is_permanent,
        severity_level: req.body.severity_level,
      };

      const block = await AdminUserService.blockUser(
        action,
        req.user!.id,
        req.user!.username
      );

      res.json({ success: true, block });
    } catch (error: any) {
      console.error('Block user error:', error);
      res.status(500).json({ error: error.message });
    }
  }
);

/**
 * POST /api/admin/blocks/:id/unblock
 * Unblock a user
 */
router.post(
  '/blocks/:id/unblock',
  requireAdmin,
  requirePermission('user:ban'),
  async (req: AdminAuthRequest, res: Response): Promise<void> => {
    try {
      const blockId = parseInt(req.params.id);
      await AdminUserService.unblockUser(
        blockId,
        req.body.reason,
        req.user!.id,
        req.user!.username
      );

      res.json({ success: true });
    } catch (error: any) {
      console.error('Unblock user error:', error);
      res.status(500).json({ error: error.message });
    }
  }
);

/**
 * POST /api/admin/users/:id/tag
 * Tag a user
 */
router.post(
  '/users/:id/tag',
  requireAdmin,
  requirePermission('user:tag'),
  async (req: AdminAuthRequest, res: Response): Promise<void> => {
    try {
      const userId = parseInt(req.params.id);
      const action = {
        user_id: userId,
        tag_name: req.body.tag_name,
        tag_category: req.body.tag_category,
        tag_color: req.body.tag_color,
        description: req.body.description,
        expires_at: req.body.expires_at ? new Date(req.body.expires_at) : undefined,
      };

      const tag = await AdminUserService.tagUser(
        action,
        req.user!.id,
        req.user!.username
      );

      res.json({ success: true, tag });
    } catch (error: any) {
      console.error('Tag user error:', error);
      res.status(500).json({ error: error.message });
    }
  }
);

/**
 * DELETE /api/admin/tags/:id
 * Remove tag from user
 */
router.delete(
  '/tags/:id',
  requireAdmin,
  requirePermission('user:tag'),
  async (req: AdminAuthRequest, res: Response): Promise<void> => {
    try {
      const tagId = parseInt(req.params.id);
      await AdminUserService.removeTag(tagId, req.user!.id, req.user!.username);
      res.json({ success: true });
    } catch (error: any) {
      console.error('Remove tag error:', error);
      res.status(500).json({ error: error.message });
    }
  }
);

/**
 * POST /api/admin/users/:id/resources
 * Adjust user resources
 */
router.post(
  '/users/:id/resources',
  requireAdmin,
  requirePermission('game:resources'),
  rateLimit(20, 60000),
  async (req: AdminAuthRequest, res: Response): Promise<void> => {
    try {
      const userId = parseInt(req.params.id);
      const action = {
        user_id: userId,
        planet_id: req.body.planet_id,
        metal: req.body.metal,
        crystal: req.body.crystal,
        deuterium: req.body.deuterium,
        dark_matter: req.body.dark_matter,
        reason: req.body.reason,
      };

      await AdminUserService.adjustResources(
        action,
        req.user!.id,
        req.user!.username
      );

      res.json({ success: true });
    } catch (error: any) {
      console.error('Adjust resources error:', error);
      res.status(500).json({ error: error.message });
    }
  }
);

/**
 * POST /api/admin/users/bulk-action
 * Perform bulk action on multiple users
 */
router.post(
  '/users/bulk-action',
  requireAdmin,
  requirePermission('game:config'),
  rateLimit(5, 60000),
  async (req: AdminAuthRequest, res: Response): Promise<void> => {
    try {
      const action = {
        action: req.body.action,
        target_ids: req.body.target_ids,
        params: req.body.params,
        reason: req.body.reason,
      };

      const result = await AdminUserService.bulkAction(
        action,
        req.user!.id,
        req.user!.username
      );

      res.json({ 
        success: true, 
        successful_actions: result.success, 
        failed_actions: result.failed, 
        errors: result.errors 
      });
    } catch (error: any) {
      console.error('Bulk action error:', error);
      res.status(500).json({ error: error.message });
    }
  }
);

// ========================================
// MONITORING & METRICS
// ========================================

/**
 * GET /api/admin/monitoring/health
 * Get current server health
 */
router.get(
  '/monitoring/health',
  requireAdmin,
  async (req: AdminAuthRequest, res: Response): Promise<void> => {
    try {
      const health = await AdminMonitoringService.getServerHealth();
      res.json(health);
    } catch (error: any) {
      console.error('Get health error:', error);
      res.status(500).json({ error: error.message });
    }
  }
);

/**
 * GET /api/admin/monitoring/metrics/:name
 * Get metrics history
 */
router.get(
  '/monitoring/metrics/:name',
  requireAdmin,
  requirePermission('monitoring:read'),
  async (req: AdminAuthRequest, res: Response): Promise<void> => {
    try {
      const metricName = req.params.name;
      const hours = parseInt(req.query.hours as string) || 24;
      const metrics = await AdminMonitoringService.getMetricsHistory(metricName, hours);
      res.json(metrics);
    } catch (error: any) {
      console.error('Get metrics error:', error);
      res.status(500).json({ error: error.message });
    }
  }
);

/**
 * GET /api/admin/monitoring/activity
 * Get real-time player activity
 */
router.get(
  '/monitoring/activity',
  requireAdmin,
  async (req: AdminAuthRequest, res: Response): Promise<void> => {
    try {
      const activity = await AdminMonitoringService.getPlayerActivity();
      res.json(activity);
    } catch (error: any) {
      console.error('Get activity error:', error);
      res.status(500).json({ error: error.message });
    }
  }
);

/**
 * GET /api/admin/monitoring/database
 * Get database statistics
 */
router.get(
  '/monitoring/database',
  requireAdmin,
  requirePermission('monitoring:read'),
  async (req: AdminAuthRequest, res: Response): Promise<void> => {
    try {
      const stats = await AdminMonitoringService.getDatabaseStats();
      res.json(stats);
    } catch (error: any) {
      console.error('Get database stats error:', error);
      res.status(500).json({ error: error.message });
    }
  }
);

/**
 * GET /api/admin/monitoring/scaling
 * Get process, socket, and leaderboard scheduler metrics
 */
router.get(
  '/monitoring/scaling',
  requireAdmin,
  requirePermission('monitoring:read'),
  async (req: AdminAuthRequest, res: Response): Promise<void> => {
    try {
      const handler = getRealtimeHandler();
      const socketStats = handler
        ? handler.getStats()
        : { connectedClients: 0, rooms: 0, namespaces: 0, adapterName: 'n/a' };

      const memoryUsage = process.memoryUsage();
      let redisLatencyMs: number | null = null;

      try {
        const start = Date.now();
        await redis.ping();
        redisLatencyMs = Date.now() - start;
      } catch (error) {
        console.warn('Redis latency check failed:', error);
      }

      res.json({
        process: {
          uptimeSeconds: process.uptime(),
          loadAverage: os.loadavg(),
          memory: {
            rss: memoryUsage.rss,
            heapUsed: memoryUsage.heapUsed,
            heapTotal: memoryUsage.heapTotal,
          },
        },
        sockets: socketStats,
        redis: {
          status: redis.status,
          latencyMs: redisLatencyMs,
        },
        leaderboard: LeaderboardScheduler.getStatus(),
      });
    } catch (error: any) {
      console.error('Get scaling metrics error:', error);
      res.status(500).json({ error: error.message });
    }
  }
);

router.post(
  '/monitoring/leaderboard/rebuild',
  requireAdmin,
  requirePermission('monitoring:write'),
  async (req: AdminAuthRequest, res: Response): Promise<void> => {
    try {
      await LeaderboardScheduler.triggerRebuild();
      res.json({ success: true, message: 'Leaderboard rebuild triggered.' });
    } catch (error: any) {
      console.error('Manual leaderboard rebuild error:', error);
      res.status(500).json({ error: error.message || 'Failed to rebuild leaderboard' });
    }
  }
);

// ========================================
// CHAT MODERATION
// ========================================

router.post(
  '/chat/restrictions',
  requireAdmin,
  requirePermission('monitoring:write'),
  async (req: AdminAuthRequest, res: Response): Promise<void> => {
    try {
      const { userId, restrictionType, reason, durationMinutes, channelId } = req.body || {};

      if (!userId || !restrictionType || !reason) {
        res.status(400).json({ error: 'userId, restrictionType, and reason are required' });
        return;
      }

      const allowed = ['mute', 'ban', 'slowmode', 'shadow'];
      if (!allowed.includes(restrictionType)) {
        res.status(400).json({ error: 'Invalid restriction type' });
        return;
      }

      await chatService.restrictUser(
        userId,
        channelId ? Number(channelId) : null,
        restrictionType,
        reason,
        req.user!.id,
        durationMinutes ? Number(durationMinutes) : undefined
      );

      res.json({ success: true, message: 'Restriction applied' });
    } catch (error: any) {
      console.error('Apply chat restriction error:', error);
      res.status(500).json({ error: error.message });
    }
  }
);

router.delete(
  '/chat/restrictions',
  requireAdmin,
  requirePermission('monitoring:write'),
  async (req: AdminAuthRequest, res: Response): Promise<void> => {
    try {
      const { userId, restrictionType, channelId } = req.body || {};
      if (!userId || !restrictionType) {
        res.status(400).json({ error: 'userId and restrictionType are required' });
        return;
      }

      await chatService.removeRestriction(
        userId,
        channelId ? Number(channelId) : null,
        restrictionType
      );

      res.json({ success: true, message: 'Restriction removed' });
    } catch (error: any) {
      console.error('Remove chat restriction error:', error);
      res.status(500).json({ error: error.message });
    }
  }
);

// ========================================
// NOTIFICATIONS
// ========================================

/**
 * GET /api/admin/notifications
 * Get admin notifications
 */
router.get(
  '/notifications',
  requireAdmin,
  async (req: AdminAuthRequest, res: Response): Promise<void> => {
    try {
      const unreadOnly = req.query.unread === 'true';
      const notifications = await AdminMonitoringService.getNotifications(
        req.user!.id,
        req.adminLevel!,
        unreadOnly
      );
      res.json(notifications);
    } catch (error: any) {
      console.error('Get notifications error:', error);
      res.status(500).json({ error: error.message });
    }
  }
);

/**
 * POST /api/admin/notifications/:id/read
 * Mark notification as read
 */
router.post(
  '/notifications/:id/read',
  requireAdmin,
  async (req: AdminAuthRequest, res: Response): Promise<void> => {
    try {
      const notificationId = parseInt(req.params.id);
      await AdminMonitoringService.markNotificationRead(notificationId, req.user!.id);
      res.json({ success: true });
    } catch (error: any) {
      console.error('Mark read error:', error);
      res.status(500).json({ error: error.message });
    }
  }
);

/**
 * POST /api/admin/notifications/:id/acknowledge
 * Acknowledge notification
 */
router.post(
  '/notifications/:id/acknowledge',
  requireAdmin,
  async (req: AdminAuthRequest, res: Response): Promise<void> => {
    try {
      const notificationId = parseInt(req.params.id);
      await AdminMonitoringService.acknowledgeNotification(notificationId, req.user!.id);
      res.json({ success: true });
    } catch (error: any) {
      console.error('Acknowledge error:', error);
      res.status(500).json({ error: error.message });
    }
  }
);

// ========================================
// SETTINGS & CONFIGURATION
// ========================================

/**
 * GET /api/admin/settings
 * Get all settings
 */
router.get(
  '/settings',
  requireAdmin,
  requirePermission('game:config'),
  async (req: AdminAuthRequest, res: Response): Promise<void> => {
    try {
      const category = req.query.category as any;
      const settings = await AdminSettingsService.getAllSettings(category);
      res.json(settings);
    } catch (error: any) {
      console.error('Get settings error:', error);
      res.status(500).json({ error: error.message });
    }
  }
);

/**
 * GET /api/admin/settings/:key
 * Get specific setting
 */
router.get(
  '/settings/:key',
  requireAdmin,
  requirePermission('game:config'),
  async (req: AdminAuthRequest, res: Response): Promise<void> => {
    try {
      const setting = await AdminSettingsService.getSetting(req.params.key);
      if (!setting) {
        res.status(404).json({ error: 'Setting not found' });
        return;
      }
      res.json(setting);
    } catch (error: any) {
      console.error('Get setting error:', error);
      res.status(500).json({ error: error.message });
    }
  }
);

/**
 * PUT /api/admin/settings/:key
 * Update setting
 */
router.put(
  '/settings/:key',
  requireAdmin,
  requirePermission('game:config'),
  rateLimit(30, 60000),
  async (req: AdminAuthRequest, res: Response): Promise<void> => {
    try {
      const setting = await AdminSettingsService.updateSetting(
        req.params.key,
        req.body.value,
        req.user!.id,
        req.user!.username
      );
      res.json(setting);
    } catch (error: any) {
      console.error('Update setting error:', error);
      res.status(500).json({ error: error.message });
    }
  }
);

/**
 * GET /api/admin/settings/:key/history
 * Get setting change history
 */
router.get(
  '/settings/:key/history',
  requireAdmin,
  requirePermission('game:config'),
  async (req: AdminAuthRequest, res: Response): Promise<void> => {
    try {
      const limit = parseInt(req.query.limit as string) || 10;
      const history = await AdminSettingsService.getSettingHistory(req.params.key, limit);
      res.json(history);
    } catch (error: any) {
      console.error('Get history error:', error);
      res.status(500).json({ error: error.message });
    }
  }
);

// ========================================
// GAME EVENTS
// ========================================

/**
 * GET /api/admin/events
 * Get all game events
 */
router.get(
  '/events',
  requireAdmin,
  requirePermission('game:events'),
  async (req: AdminAuthRequest, res: Response): Promise<void> => {
    try {
      const limit = parseInt(req.query.limit as string) || 50;
      const events = await AdminEventsService.getAllEvents(limit);
      res.json(events);
    } catch (error: any) {
      console.error('Get events error:', error);
      res.status(500).json({ error: error.message });
    }
  }
);

/**
 * POST /api/admin/events
 * Create game event
 */
router.post(
  '/events',
  requireAdmin,
  requirePermission('game:events'),
  rateLimit(10, 60000),
  async (req: AdminAuthRequest, res: Response): Promise<void> => {
    try {
      const action = {
        event_type: req.body.event_type,
        event_name: req.body.event_name,
        event_description: req.body.event_description,
        event_data: req.body.event_data,
        start_time: new Date(req.body.start_time),
        end_time: req.body.end_time ? new Date(req.body.end_time) : undefined,
        target_scope: req.body.target_scope,
        target_ids: req.body.target_ids,
        rewards: req.body.rewards,
        priority: req.body.priority,
      };

      const event = await AdminEventsService.createEvent(
        action,
        req.user!.id,
        req.user!.username
      );

      res.json({ success: true, event });
    } catch (error: any) {
      console.error('Create event error:', error);
      res.status(500).json({ error: error.message });
    }
  }
);

/**
 * POST /api/admin/events/:id/activate
 * Activate game event
 */
router.post(
  '/events/:id/activate',
  requireAdmin,
  requirePermission('game:events'),
  async (req: AdminAuthRequest, res: Response): Promise<void> => {
    try {
      const eventId = parseInt(req.params.id);
      await AdminEventsService.activateEvent(eventId, req.user!.id, req.user!.username);
      res.json({ success: true });
    } catch (error: any) {
      console.error('Activate event error:', error);
      res.status(500).json({ error: error.message });
    }
  }
);

/**
 * POST /api/admin/events/:id/deactivate
 * Deactivate game event
 */
router.post(
  '/events/:id/deactivate',
  requireAdmin,
  requirePermission('game:events'),
  async (req: AdminAuthRequest, res: Response): Promise<void> => {
    try {
      const eventId = parseInt(req.params.id);
      await AdminEventsService.deactivateEvent(eventId, req.user!.id, req.user!.username);
      res.json({ success: true });
    } catch (error: any) {
      console.error('Deactivate event error:', error);
      res.status(500).json({ error: error.message });
    }
  }
);

// ========================================
// ANALYTICS & REPORTING
// ========================================

/**
 * GET /api/admin/analytics/resources
 * Get resource analytics
 */
router.get(
  '/analytics/resources',
  requireAdmin,
  requirePermission('reports:read'),
  async (req: AdminAuthRequest, res: Response): Promise<void> => {
    try {
      const analytics = await AdminAnalyticsService.getResourceAnalytics();
      res.json(analytics);
    } catch (error: any) {
      console.error('Get resource analytics error:', error);
      res.status(500).json({ error: error.message });
    }
  }
);

/**
 * GET /api/admin/analytics/combat
 * Get combat analytics
 */
router.get(
  '/analytics/combat',
  requireAdmin,
  requirePermission('reports:read'),
  async (req: AdminAuthRequest, res: Response): Promise<void> => {
    try {
      const analytics = await AdminAnalyticsService.getCombatAnalytics();
      res.json(analytics);
    } catch (error: any) {
      console.error('Get combat analytics error:', error);
      res.status(500).json({ error: error.message });
    }
  }
);

/**
 * GET /api/admin/analytics/audit-stats
 * Get audit log statistics
 */
router.get(
  '/analytics/audit-stats',
  requireAdmin,
  requirePermission('reports:read'),
  async (req: AdminAuthRequest, res: Response): Promise<void> => {
    try {
      const days = parseInt(req.query.days as string) || 30;
      const stats = await AdminAnalyticsService.getAuditStats(days);
      res.json(stats);
    } catch (error: any) {
      console.error('Get audit stats error:', error);
      res.status(500).json({ error: error.message });
    }
  }
);

/**
 * GET /api/admin/analytics/top-admins
 * Get top admins by activity
 */
router.get(
  '/analytics/top-admins',
  requireAdmin,
  requirePermission('game:config'),
  async (req: AdminAuthRequest, res: Response): Promise<void> => {
    try {
      const days = parseInt(req.query.days as string) || 30;
      const limit = parseInt(req.query.limit as string) || 10;
      const topAdmins = await AdminAnalyticsService.getTopAdmins(days, limit);
      res.json(topAdmins);
    } catch (error: any) {
      console.error('Get top admins error:', error);
      res.status(500).json({ error: error.message });
    }
  }
);

// ========================================
// AUDIT LOGS
// ========================================

/**
 * GET /api/admin/audit-logs
 * Get audit logs with filtering
 */
router.get(
  '/audit-logs',
  requireAdmin,
  requirePermission('monitoring:read'),
  async (req: AdminAuthRequest, res: Response): Promise<void> => {
    try {
      const page = parseInt(req.query.page as string) || 1;
      const limit = parseInt(req.query.limit as string) || 50;
      const offset = (page - 1) * limit;

      let whereConditions: string[] = [];
      const params: any[] = [];
      let paramIndex = 1;

      if (req.query.admin_id) {
        whereConditions.push(`admin_id = $${paramIndex}`);
        params.push(parseInt(req.query.admin_id as string));
        paramIndex++;
      }

      if (req.query.action_category) {
        whereConditions.push(`action_category = $${paramIndex}`);
        params.push(req.query.action_category);
        paramIndex++;
      }

      if (req.query.dateFrom) {
        whereConditions.push(`timestamp >= $${paramIndex}`);
        params.push(new Date(req.query.dateFrom as string));
        paramIndex++;
      }

      const whereClause = whereConditions.length > 0
        ? `WHERE ${whereConditions.join(' AND ')}`
        : '';

      const countResult = await pool.query(
        `SELECT COUNT(*) FROM admin_audit_logs ${whereClause}`,
        params
      );
      const total = parseInt(countResult.rows[0].count);

      const logsResult = await pool.query(
        `SELECT * FROM admin_audit_logs ${whereClause}
         ORDER BY timestamp DESC
         LIMIT $${paramIndex} OFFSET $${paramIndex + 1}`,
        [...params, limit, offset]
      );

      res.json({
        data: logsResult.rows,
        total,
        page,
        limit,
        totalPages: Math.ceil(total / limit),
      });
    } catch (error: any) {
      console.error('Get audit logs error:', error);
      res.status(500).json({ error: error.message });
    }
  }
);

export default router;
