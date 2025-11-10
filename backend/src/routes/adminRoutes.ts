import { Router, Response } from 'express';
import os from 'os';
import { AdminAuthRequest } from '../types/admin';
import { 
  requireAdmin,
  requirePermission,
  requirePermissions,
  rateLimit,
  verifyAdmin2FA,
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
// ADMIN AUTHENTICATION
// ========================================

/**
 * POST /api/admin/login
 * Admin login with 2FA verification
 */
router.post('/login', authenticateToken, verifyAdmin2FA, async (req: AdminAuthRequest, res: Response) => {
  try {
    // Get admin user data
    const adminResult = await pool.query(
      `SELECT 
         au.*, 
         u.username, 
         u.email, 
         r.id as role_id, 
         r.name as role_name, 
         ARRAY_REMOVE(ARRAY_AGG(p.name), NULL) as permissions
       FROM admin_users au
       JOIN users u ON au.user_id = u.id
       JOIN roles r ON au.role_id = r.id
       LEFT JOIN role_permissions rp ON r.id = rp.role_id
       LEFT JOIN permissions p ON rp.permission_id = p.id
       WHERE au.user_id = $1 AND au.is_active = TRUE
       GROUP BY au.id, u.username, u.email, r.id, r.name`,
      [req.user!.id]
    );

    if (adminResult.rows.length === 0) {
      return res.status(403).json({ error: 'Admin access required' });
    }

    const admin = adminResult.rows[0];

    // Update last login
    await pool.query(
      'UPDATE admin_users SET last_login = NOW() WHERE id = $1',
      [admin.id]
    );

    // Log admin action
    await pool.query(`
      INSERT INTO admin_audit_logs (admin_user_id, action, resource_type, resource_id, details, ip_address)
      VALUES ($1, 'LOGIN', 'SESSION', NULL, $2, $3)
    `, [admin.id, JSON.stringify({ 
      username: admin.username,
      role: admin.role_name,
      twoFactorVerified: true
    }), req.ip]);

    res.json({
      success: true,
      message: 'Admin login successful',
      admin: {
        id: admin.id,
        username: admin.username,
        email: admin.email,
        role: admin.role_name,
        permissions: Array.isArray(admin.permissions) ? admin.permissions : []
      }
    });
  } catch (error: any) {
    console.error('Admin login error:', error);
    res.status(500).json({ error: error.message });
  }
});

/**
 * GET /api/admin/2fa/status
 * Check if 2FA is required for admin access
 */
router.get('/2fa/status', authenticateToken, async (req: AdminAuthRequest, res: Response) => {
  try {
    const result = await pool.query(
      `SELECT 
         au.user_id,
         COALESCE(tfa.is_enabled, FALSE) as two_factor_enabled,
         r.name as role_name
       FROM admin_users au
       JOIN users u ON au.user_id = u.id
       JOIN roles r ON au.role_id = r.id
       LEFT JOIN two_factor_auth tfa ON tfa.user_id = u.id
       WHERE au.user_id = $1 AND au.is_active = TRUE`,
      [req.user!.id]
    );

    if (result.rows.length === 0) {
      return res.status(403).json({ error: 'Admin access required' });
    }

    const admin = result.rows[0];

    res.json({
      success: true,
      isAdmin: true,
      role: admin.role_name,
      twoFactorRequired: !admin.two_factor_enabled,
      twoFactorEnabled: admin.two_factor_enabled
    });
  } catch (error: any) {
    console.error('Admin 2FA status error:', error);
    res.status(500).json({ error: error.message });
  }
});

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
  requirePermission('user:write'),
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
  requirePermission('game:config:read'),
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
  requirePermission('game:config:read'),
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
  requirePermission('game:config:write'),
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
  requirePermission('game:config:read'),
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
  requirePermission('analytics:view'),
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

// ========================================
// ROLE MANAGEMENT
// ========================================

/**
 * GET /api/admin/roles
 * Get all roles with their permissions
 */
router.get('/roles', requirePermission('admin:manage'), async (req: AdminAuthRequest, res: Response) => {
  try {
    const result = await pool.query(`
      SELECT 
        r.id,
        r.name,
        r.description,
        r.protected,
        COUNT(rp.permission_id) as permission_count,
        ARRAY_AGG(p.name ORDER BY p.name) as permissions
      FROM roles r
      LEFT JOIN role_permissions rp ON r.id = rp.role_id
      LEFT JOIN permissions p ON rp.permission_id = p.id
      GROUP BY r.id, r.name, r.description, r.protected
      ORDER BY r.name
    `);

    res.json({
      success: true,
      roles: result.rows
    });
  } catch (error: any) {
    console.error('Get roles error:', error);
    res.status(500).json({ error: error.message });
  }
});

/**
 * GET /api/admin/roles/:id
 * Get specific role with permissions
 */
router.get('/roles/:id', requirePermission('admin:manage'), async (req: AdminAuthRequest, res: Response) => {
  try {
    const roleId = parseInt(req.params.id);
    
    const roleResult = await pool.query(`
      SELECT id, name, description, protected
      FROM roles
      WHERE id = $1
    `, [roleId]);

    if (roleResult.rows.length === 0) {
      return res.status(404).json({ error: 'Role not found' });
    }

    const permissionsResult = await pool.query(`
      SELECT p.id, p.name, p.description
      FROM permissions p
      JOIN role_permissions rp ON p.id = rp.permission_id
      WHERE rp.role_id = $1
      ORDER BY p.name
    `, [roleId]);

    res.json({
      success: true,
      role: {
        ...roleResult.rows[0],
        permissions: permissionsResult.rows
      }
    });
  } catch (error: any) {
    console.error('Get role error:', error);
    res.status(500).json({ error: error.message });
  }
});

/**
 * POST /api/admin/roles
 * Create new role
 */
router.post('/roles', requirePermission('admin:manage'), async (req: AdminAuthRequest, res: Response) => {
  try {
    const { name, description, permissionIds } = req.body;

    if (!name || name.trim().length === 0) {
      return res.status(400).json({ error: 'Role name is required' });
    }

    const client = await pool.connect();
    try {
      await client.query('BEGIN');

      // Set session context for audit logging
      await client.query('SET app.current_admin_id = $1', [req.admin?.id]);
      await client.query('SET app.client_ip = $1', [req.ip]);

      // Create role
      const roleResult = await client.query(`
        INSERT INTO roles (name, description, protected)
        VALUES ($1, $2, FALSE)
        RETURNING id, name, description, protected
      `, [name.trim(), description]);

      const newRole = roleResult.rows[0];

      // Assign permissions if provided
      if (permissionIds && permissionIds.length > 0) {
        for (const permissionId of permissionIds) {
          await client.query(`
            INSERT INTO role_permissions (role_id, permission_id)
            VALUES ($1, $2)
            ON CONFLICT DO NOTHING
          `, [newRole.id, permissionId]);
        }
      }

      await client.query('COMMIT');

      res.json({
        success: true,
        role: newRole
      });
    } catch (error) {
      await client.query('ROLLBACK');
      throw error;
    } finally {
      client.release();
    }
  } catch (error: any) {
    console.error('Create role error:', error);
    if (error.code === '23505') {
      res.status(400).json({ error: 'Role name already exists' });
    } else {
      res.status(500).json({ error: error.message });
    }
  }
});

/**
 * PUT /api/admin/roles/:id
 * Update role
 */
router.put('/roles/:id', requirePermission('admin:manage'), async (req: AdminAuthRequest, res: Response) => {
  try {
    const roleId = parseInt(req.params.id);
    const { name, description, permissionIds } = req.body;

    // Check if role exists and is not protected
    const roleResult = await pool.query(`
      SELECT id, name, protected
      FROM roles
      WHERE id = $1
    `, [roleId]);

    if (roleResult.rows.length === 0) {
      return res.status(404).json({ error: 'Role not found' });
    }

    if (roleResult.rows[0].protected) {
      return res.status(403).json({ error: 'Cannot modify protected role' });
    }

    const client = await pool.connect();
    try {
      await client.query('BEGIN');

      // Set session context for audit logging
      await client.query('SET app.current_admin_id = $1', [req.admin?.id]);
      await client.query('SET app.client_ip = $1', [req.ip]);

      // Update role
      await client.query(`
        UPDATE roles
        SET name = $1, description = $2
        WHERE id = $3
      `, [name.trim(), description, roleId]);

      // Update permissions
      if (permissionIds !== undefined) {
        // Remove existing permissions
        await client.query(`
          DELETE FROM role_permissions
          WHERE role_id = $1
        `, [roleId]);

        // Add new permissions
        if (permissionIds.length > 0) {
          for (const permissionId of permissionIds) {
            await client.query(`
              INSERT INTO role_permissions (role_id, permission_id)
              VALUES ($1, $2)
            `, [roleId, permissionId]);
          }
        }
      }

      await client.query('COMMIT');

      res.json({
        success: true,
        message: 'Role updated successfully'
      });
    } catch (error) {
      await client.query('ROLLBACK');
      throw error;
    } finally {
      client.release();
    }
  } catch (error: any) {
    console.error('Update role error:', error);
    if (error.code === '23505') {
      res.status(400).json({ error: 'Role name already exists' });
    } else {
      res.status(500).json({ error: error.message });
    }
  }
});

/**
 * DELETE /api/admin/roles/:id
 * Delete role
 */
router.delete('/roles/:id', requirePermission('admin:manage'), async (req: AdminAuthRequest, res: Response) => {
  try {
    const roleId = parseInt(req.params.id);

    // Check if role exists and is not protected
    const roleResult = await pool.query(`
      SELECT id, name, protected
      FROM roles
      WHERE id = $1
    `, [roleId]);

    if (roleResult.rows.length === 0) {
      return res.status(404).json({ error: 'Role not found' });
    }

    if (roleResult.rows[0].protected) {
      return res.status(403).json({ error: 'Cannot delete protected role' });
    }

    // Check if role is assigned to any users
    const userCountResult = await pool.query(`
      SELECT COUNT(*) as count
      FROM admin_users
      WHERE role_id = $1
    `, [roleId]);

    if (parseInt(userCountResult.rows[0].count) > 0) {
      return res.status(400).json({ 
        error: 'Cannot delete role that is assigned to users',
        userCount: parseInt(userCountResult.rows[0].count)
      });
    }

    const client = await pool.connect();
    try {
      await client.query('BEGIN');

      // Set session context for audit logging
      await client.query('SET app.current_admin_id = $1', [req.admin?.id]);
      await client.query('SET app.client_ip = $1', [req.ip]);

      // Delete role permissions and role
      await client.query(`
        DELETE FROM role_permissions
        WHERE role_id = $1
      `, [roleId]);

      await client.query(`
        DELETE FROM roles
        WHERE id = $1
      `, [roleId]);

      await client.query('COMMIT');

      res.json({
        success: true,
        message: 'Role deleted successfully'
      });
    } catch (error) {
      await client.query('ROLLBACK');
      throw error;
    } finally {
      client.release();
    }
  } catch (error: any) {
    console.error('Delete role error:', error);
    res.status(500).json({ error: error.message });
  }
});

/**
 * GET /api/admin/permissions
 * Get all available permissions
 */
router.get('/permissions', requirePermission('admin:manage'), async (req: AdminAuthRequest, res: Response) => {
  try {
    const result = await pool.query(`
      SELECT id, name, description
      FROM permissions
      ORDER BY name
    `);

    res.json({
      success: true,
      permissions: result.rows
    });
  } catch (error: any) {
    console.error('Get permissions error:', error);
    res.status(500).json({ error: error.message });
  }
});

/**
 * GET /api/admin/users/:userId/role
 * Get user's current role
 */
router.get('/users/:userId/role', requirePermission('admin:manage'), async (req: AdminAuthRequest, res: Response) => {
  try {
    const userId = parseInt(req.params.userId);
    
    const result = await pool.query(`
      SELECT 
        au.id as admin_user_id,
        au.role_id,
        r.name as role_name,
        r.description as role_description
      FROM admin_users au
      LEFT JOIN roles r ON au.role_id = r.id
      WHERE au.user_id = $1
    `, [userId]);

    if (result.rows.length === 0) {
      return res.status(404).json({ error: 'Admin user not found' });
    }

    res.json({
      success: true,
      adminUser: result.rows[0]
    });
  } catch (error: any) {
    console.error('Get user role error:', error);
    res.status(500).json({ error: error.message });
  }
});

/**
 * PUT /api/admin/users/:userId/role
 * Assign role to admin user
 */
router.put('/users/:userId/role', requirePermission('admin:manage'), async (req: AdminAuthRequest, res: Response) => {
  try {
    const userId = parseInt(req.params.userId);
    const { roleId } = req.body;

    // Verify admin user exists
    const adminUserResult = await pool.query(`
      SELECT id, role_id
      FROM admin_users
      WHERE user_id = $1
    `, [userId]);

    if (adminUserResult.rows.length === 0) {
      return res.status(404).json({ error: 'Admin user not found' });
    }

    // Verify role exists
    if (roleId) {
      const roleResult = await pool.query(`
        SELECT id, name
        FROM roles
        WHERE id = $1
      `, [roleId]);

      if (roleResult.rows.length === 0) {
        return res.status(400).json({ error: 'Role not found' });
      }
    }

    const oldRoleId = adminUserResult.rows[0].role_id;

    // Set session context for audit logging
    await pool.query('SET app.current_admin_id = $1', [req.admin?.id]);
    await pool.query('SET app.client_ip = $1', [req.ip]);

    // Update role
    await pool.query(`
      UPDATE admin_users
      SET role_id = $1
      WHERE user_id = $2
    `, [roleId || null, userId]);

    // Clear permission cache for this user
    const cacheKey = `admin_permissions:${userId}`;
    await redis.del(cacheKey);

    

    res.json({
      success: true,
      message: 'Role assigned successfully'
    });
  } catch (error: any) {
    console.error('Assign user role error:', error);
    res.status(500).json({ error: error.message });
  }
});

/**
 * GET /api/admin/users/list
 * Get all admin users with their roles
 */
router.get('/users/list', requirePermission('admin:manage'), async (req: AdminAuthRequest, res: Response) => {
  try {
    const { page = 1, limit = 50, search } = req.query;
    const offset = (parseInt(page as string) - 1) * parseInt(limit as string);

    let whereClause = '';
    let queryParams: any[] = [];
    let paramIndex = 1;

    if (search) {
      whereClause = `WHERE (u.username ILIKE $${paramIndex} OR u.email ILIKE $${paramIndex})`;
      queryParams.push(`%${search}%`);
      paramIndex++;
    }

    queryParams.push(parseInt(limit as string), offset);

    const result = await pool.query(`
      SELECT 
        au.id as admin_user_id,
        au.user_id,
        u.username,
        u.email,
        u.is_active,
        au.role_id,
        r.name as role_name,
        r.description as role_description,
        au.created_at,
        au.last_login,
        ARRAY_AGG(p.name ORDER BY p.name) as permissions
      FROM admin_users au
      JOIN users u ON au.user_id = u.id
      LEFT JOIN roles r ON au.role_id = r.id
      LEFT JOIN role_permissions rp ON r.id = rp.role_id
      LEFT JOIN permissions p ON rp.permission_id = p.id
      ${whereClause}
      GROUP BY au.id, u.username, u.email, u.is_active, r.name, r.description
      ORDER BY u.username
      LIMIT $${paramIndex} OFFSET $${paramIndex + 1}
    `, queryParams);

    const countResult = await pool.query(`
      SELECT COUNT(*) as total
      FROM admin_users au
      JOIN users u ON au.user_id = u.id
      ${whereClause}
    `, queryParams.slice(0, -2));

    res.json({
      success: true,
      adminUsers: result.rows,
      pagination: {
        page: parseInt(page as string),
        limit: parseInt(limit as string),
        total: parseInt(countResult.rows[0].total),
        pages: Math.ceil(parseInt(countResult.rows[0].total) / parseInt(limit as string))
      }
    });
  } catch (error: any) {
    console.error('Get admin users error:', error);
    res.status(500).json({ error: error.message });
  }
});

/**
 * POST /api/admin/users/:userId/promote
 * Promote regular user to admin
 */
router.post('/users/:userId/promote', requirePermission('admin:manage'), async (req: AdminAuthRequest, res: Response) => {
  try {
    const userId = parseInt(req.params.userId);
    const { roleId } = req.body;

    if (!roleId) {
      return res.status(400).json({ error: 'Role ID is required' });
    }

    // Verify user exists and is not already admin
    const userResult = await pool.query(`
      SELECT id, username, email, is_admin
      FROM users
      WHERE id = $1
    `, [userId]);

    if (userResult.rows.length === 0) {
      return res.status(404).json({ error: 'User not found' });
    }

    if (userResult.rows[0].is_admin) {
      return res.status(400).json({ error: 'User is already an admin' });
    }

    // Verify role exists
    const roleResult = await pool.query(`
      SELECT id, name
      FROM roles
      WHERE id = $1
    `, [roleId]);

    if (roleResult.rows.length === 0) {
      return res.status(400).json({ error: 'Role not found' });
    }

    const client = await pool.connect();
    try {
      await client.query('BEGIN');

      // Set session context for audit logging
      await client.query('SET app.current_admin_id = $1', [req.admin?.id]);
      await client.query('SET app.client_ip = $1', [req.ip]);

      // Create admin user record
      const adminUserResult = await client.query(`
        INSERT INTO admin_users (user_id, role_id, created_by)
        VALUES ($1, $2, $3)
        RETURNING id
      `, [userId, roleId, req.admin?.id]);

      // Update user is_admin flag
      await client.query(`
        UPDATE users
        SET is_admin = TRUE
        WHERE id = $1
      `, [userId]);

      await client.query('COMMIT');

      res.json({
        success: true,
        message: 'User promoted to admin successfully',
        adminUserId: adminUserResult.rows[0].id
      });
    } catch (error) {
      await client.query('ROLLBACK');
      throw error;
    } finally {
      client.release();
    }
  } catch (error: any) {
    console.error('Promote user error:', error);
    if (error.code === '23505') {
      res.status(400).json({ error: 'User is already an admin' });
    } else {
      res.status(500).json({ error: error.message });
    }
  }
});

/**
 * POST /api/admin/users/:userId/demote
 * Demote admin to regular user
 */
router.post('/users/:userId/demote', requirePermission('admin:manage'), async (req: AdminAuthRequest, res: Response) => {
  try {
    const userId = parseInt(req.params.userId);

    // Cannot demote yourself
    if (userId === req.admin?.id) {
      return res.status(400).json({ error: 'Cannot demote yourself' });
    }

    // Verify user is admin
    const adminUserResult = await pool.query(`
      SELECT au.id, au.user_id, u.username, r.name as role_name
      FROM admin_users au
      JOIN users u ON au.user_id = u.id
      LEFT JOIN roles r ON au.role_id = r.id
      WHERE au.user_id = $1
    `, [userId]);

    if (adminUserResult.rows.length === 0) {
      return res.status(404).json({ error: 'Admin user not found' });
    }

    const client = await pool.connect();
    try {
      await client.query('BEGIN');

      // Set session context for audit logging
      await client.query('SET app.current_admin_id = $1', [req.admin?.id]);
      await client.query('SET app.client_ip = $1', [req.ip]);

      // Delete admin user record
      await client.query(`
        DELETE FROM admin_users
        WHERE user_id = $1
      `, [userId]);

      // Update user is_admin flag
      await client.query(`
        UPDATE users
        SET is_admin = FALSE
        WHERE id = $1
      `, [userId]);

      // Clear permission cache
      const cacheKey = `admin_permissions:${userId}`;
      await redis.del(cacheKey);

      await client.query('COMMIT');

      res.json({
        success: true,
        message: 'Admin demoted to regular user successfully'
      });
    } catch (error) {
      await client.query('ROLLBACK');
      throw error;
    } finally {
      client.release();
    }
  } catch (error: any) {
    console.error('Demote user error:', error);
    res.status(500).json({ error: error.message });
  }
});

// ========================================
// I18N / LOCALE MANAGEMENT
// ========================================

import { getAvailableLocales } from '../config/localeUtils';
import fs from 'fs';
import path from 'path';

/**
 * GET /api/admin/locales
 * List all available locale codes
 */
router.get('/locales', requireAdmin, requirePermission('game:config:write'), (req: AdminAuthRequest, res: Response) => {
  try {
    const locales = getAvailableLocales();
    res.json({ locales });
  } catch (error: any) {
    console.error('List locales error:', error);
    res.status(500).json({ error: error.message });
  }
});

/**
 * GET /api/admin/locales/:locale
 * Get the contents of a locale JSON file
 */
router.get('/locales/:locale', requireAdmin, requirePermission('game:config:write'), (req: AdminAuthRequest, res: Response) => {
  try {
    const { locale } = req.params;
    const locales = getAvailableLocales();
    if (!locales.includes(locale)) {
      return res.status(404).json({ error: 'Locale not found' });
    }
    const localePath = path.join(__dirname, '../../../frontend/locales', `${locale}.json`);
    const data = fs.readFileSync(localePath, 'utf-8');
    res.json({ locale, data: JSON.parse(data) });
  } catch (error: any) {
    console.error('Get locale error:', error);
    res.status(500).json({ error: error.message });
  }
});

/**
 * PUT /api/admin/locales/:locale
 * Update a locale JSON file
 */
router.put('/locales/:locale', requireAdmin, requirePermission('game:config:write'), (req: AdminAuthRequest, res: Response) => {
  try {
    const { locale } = req.params;
    const locales = getAvailableLocales();
    if (!locales.includes(locale)) {
      return res.status(404).json({ error: 'Locale not found' });
    }
    const localePath = path.join(__dirname, '../../../frontend/locales', `${locale}.json`);
    const newData = req.body.data;
    if (!newData || typeof newData !== 'object') {
      return res.status(400).json({ error: 'Invalid data' });
    }
    fs.writeFileSync(localePath, JSON.stringify(newData, null, 2), 'utf-8');
    res.json({ success: true });
  } catch (error: any) {
    console.error('Update locale error:', error);
    res.status(500).json({ error: error.message });
  }
});

export default router;
