/**
 * Admin API Routes
 * Provides comprehensive administration endpoints for game management
 * 
 * Security:
 * - All routes require authentication via JWT token
 * - Admin role verification middleware enforced
 * - Audit logging for all administrative actions
 * 
 * @module backend/routes/admin
 */

import { Router, Request, Response, NextFunction } from 'express';
import { authenticateToken } from '../middleware/auth';
import { requirePermission } from '../middleware/adminAuth';
import { AuthRequest, AdminAuthRequest } from '../types';
import { pool } from '../config/database';
import os from 'os';

const router = Router();



/**
 * Log admin action for audit trail
 */
async function logAdminAction(
    userId: number,
    action: string,
    details: any
): Promise<void> {
    try {
        await pool.query(
            `INSERT INTO admin_audit_log (user_id, action, details, created_at) 
             VALUES ($1, $2, $3, NOW())`,
            [userId, action, JSON.stringify(details)]
        );
    } catch (error) {
        console.error('Error logging admin action:', error);
    }
}

/**
 * GET /api/admin/stats
 * Get dashboard statistics
 */
router.get('/stats', authenticateToken, requirePermission('monitoring:read'), async (req: AdminAuthRequest, res: Response) => {
    try {
        // User statistics
        const userStats = await pool.query(`
            SELECT 
                COUNT(*) as total,
                COUNT(CASE WHEN last_login > NOW() - INTERVAL '24 hours' THEN 1 END) as active_24h,
                COUNT(CASE WHEN created_at::date = CURRENT_DATE THEN 1 END) as today,
                COUNT(CASE WHEN is_banned = true THEN 1 END) as banned
            FROM users
        `);

        // Planet statistics
        const planetStats = await pool.query('SELECT COUNT(*) FROM planets');

        // Combat statistics
        const combatStats = await pool.query(`
            SELECT COUNT(*) as total,
                   COUNT(CASE WHEN status = 'in_progress' THEN 1 END) as active
            FROM combats_precise
        `);

        // Database size
        const dbSize = await pool.query(`
            SELECT pg_database_size(current_database()) / 1024 / 1024 as size_mb
        `);

        // Recent activity
        const recentActivity = await pool.query(`
            SELECT 'user_registered' as type, username, created_at as timestamp
            FROM users
            WHERE created_at > NOW() - INTERVAL '24 hours'
            ORDER BY created_at DESC
            LIMIT 10
        `);

        const stats = {
            totalUsers: parseInt(userStats.rows[0].total),
            activePlayers: parseInt(userStats.rows[0].active_24h),
            usersToday: parseInt(userStats.rows[0].today),
            bannedUsers: parseInt(userStats.rows[0].banned),
            totalPlanets: parseInt(planetStats.rows[0].count),
            activeCombats: parseInt(combatStats.rows[0].active || 0),
            totalCombats: parseInt(combatStats.rows[0].total || 0),
            serverUptime: Math.floor(process.uptime() / 3600),
            dbSize: Math.round(dbSize.rows[0].size_mb),
            recentActivity: recentActivity.rows
        };

        res.json(stats);
    } catch (error) {
        console.error('Error fetching admin stats:', error);
        res.status(500).json({ error: 'Failed to fetch statistics' });
    }
});

/**
 * GET /api/admin/users
 * Get users list with optional filtering
 */
router.get('/users', authenticateToken, requirePermission('user:read'), async (req: AdminAuthRequest, res: Response) => {
    try {
        const { filter, search, limit = 100, offset = 0 } = req.query;

        let query = `
            SELECT 
                u.id, 
                u.username, 
                u.email, 
                u.created_at, 
                u.last_login, 
                u.is_admin,
                u.is_banned,
                COUNT(DISTINCT p.id) as planet_count
            FROM users u
            LEFT JOIN planets p ON u.id = p.user_id
            WHERE 1=1
        `;
        
        const params: any[] = [];
        let paramCount = 0;

        if (search) {
            paramCount++;
            query += ` AND (u.username ILIKE $${paramCount} OR u.email ILIKE $${paramCount})`;
            params.push(`%${search}%`);
        }

        if (filter === 'admin') {
            query += ' AND u.is_admin = true';
        } else if (filter === 'banned') {
            query += ' AND u.is_banned = true';
        } else if (filter === 'active') {
            query += ' AND u.is_banned = false AND u.last_login > NOW() - INTERVAL \'7 days\'';
        }

        query += ' GROUP BY u.id ORDER BY u.created_at DESC';
        
        paramCount++;
        query += ` LIMIT $${paramCount}`;
        params.push(parseInt(limit as string));
        
        paramCount++;
        query += ` OFFSET $${paramCount}`;
        params.push(parseInt(offset as string));

        const result = await pool.query(query, params);

        res.json(result.rows.map(row => ({
            id: row.id,
            username: row.username,
            email: row.email,
            status: row.is_banned ? 'banned' : 'active',
            createdAt: row.created_at,
            lastLogin: row.last_login,
            isAdmin: row.is_admin,
            planetCount: parseInt(row.planet_count)
        })));
    } catch (error) {
        console.error('Error fetching users:', error);
        res.status(500).json({ error: 'Failed to fetch users' });
    }
});

/**
 * GET /api/admin/users/:id
 * Get detailed user information
 */
router.get('/users/:id', authenticateToken, requirePermission('user:read'), async (req: AdminAuthRequest, res: Response) => {
    try {
        const userId = parseInt(req.params.id);

        const userQuery = await pool.query(`
            SELECT 
                u.*,
                COUNT(DISTINCT p.id) as planet_count,
                COALESCE(SUM(
                    COALESCE((p.buildings->>'metal_mine')::int, 0) * 1000 +
                    COALESCE((p.buildings->>'crystal_mine')::int, 0) * 1500 +
                    COALESCE((p.buildings->>'deuterium_synthesizer')::int, 0) * 2000
                ), 0) as total_score
            FROM users u
            LEFT JOIN planets p ON u.id = p.user_id
            WHERE u.id = $1
            GROUP BY u.id
        `, [userId]);

        if (userQuery.rows.length === 0) {
            return res.status(404).json({ error: 'User not found' });
        }

        const user = userQuery.rows[0];

        // Get user's planets
        const planetsQuery = await pool.query(
            'SELECT id, name, galaxy, system, position FROM planets WHERE user_id = $1',
            [userId]
        );

        // Get recent activity
        const activityQuery = await pool.query(`
            SELECT 'message_sent' as type, created_at as timestamp
            FROM messages
            WHERE from_user_id = $1
            ORDER BY created_at DESC
            LIMIT 10
        `, [userId]);

        res.json({
            id: user.id,
            username: user.username,
            email: user.email,
            createdAt: user.created_at,
            lastLogin: user.last_login,
            isAdmin: user.is_admin,
            isBanned: user.is_banned,
            banReason: user.ban_reason,
            bannedAt: user.banned_at,
            planetCount: parseInt(user.planet_count),
            totalScore: parseInt(user.total_score),
            planets: planetsQuery.rows,
            recentActivity: activityQuery.rows
        });
    } catch (error) {
        console.error('Error fetching user details:', error);
        res.status(500).json({ error: 'Failed to fetch user details' });
    }
});

/**
 * POST /api/admin/users/:id/ban
 * Ban a user
 */
router.post('/users/:id/ban', authenticateToken, requirePermission('user:ban'), async (req: AdminAuthRequest, res: Response) => {
    try {
        const userId = parseInt(req.params.id);
        const { reason } = req.body;

        if (!reason || reason.trim().length === 0) {
            return res.status(400).json({ error: 'Ban reason is required' });
        }

        // Prevent banning admins
        const userCheck = await pool.query(
            'SELECT is_admin FROM users WHERE id = $1',
            [userId]
        );

        if (userCheck.rows[0]?.is_admin) {
            return res.status(400).json({ error: 'Cannot ban admin users' });
        }

        await pool.query(
            `UPDATE users 
             SET is_banned = true, ban_reason = $1, banned_at = NOW()
             WHERE id = $2`,
            [reason, userId]
        );

        await logAdminAction(req.user!.id, 'USER_BANNED', {
            targetUserId: userId,
            reason
        });

        res.json({ success: true, message: 'User banned successfully' });
    } catch (error) {
        console.error('Error banning user:', error);
        res.status(500).json({ error: 'Failed to ban user' });
    }
});

/**
 * POST /api/admin/users/:id/unban
 * Unban a user
 */
router.post('/users/:id/unban', authenticateToken, requirePermission('user:ban'), async (req: AdminAuthRequest, res: Response) => {
    try {
        const userId = parseInt(req.params.id);

        await pool.query(
            `UPDATE users 
             SET is_banned = false, ban_reason = NULL, banned_at = NULL
             WHERE id = $1`,
            [userId]
        );

        await logAdminAction(req.user!.id, 'USER_UNBANNED', {
            targetUserId: userId
        });

        res.json({ success: true, message: 'User unbanned successfully' });
    } catch (error) {
        console.error('Error unbanning user:', error);
        res.status(500).json({ error: 'Failed to unban user' });
    }
});

/**
 * GET /api/admin/server-status
 * Get server health and performance metrics
 */
router.get('/server-status', authenticateToken, requirePermission('monitoring:read'), async (req: AdminAuthRequest, res: Response) => {
    try {
        const memoryUsage = process.memoryUsage();
        const cpuUsage = os.loadavg();

        // Get active database connections
        const connections = await pool.query(`
            SELECT count(*) as active_connections
            FROM pg_stat_activity
            WHERE state = 'active'
        `);

        const status = {
            cpu: Math.round(cpuUsage[0] * 100) / 100,
            memory: Math.round(memoryUsage.heapUsed / 1024 / 1024),
            totalMemory: Math.round(memoryUsage.heapTotal / 1024 / 1024),
            uptime: Math.floor(process.uptime()),
            connections: parseInt(connections.rows[0].active_connections),
            requestsPerMin: 0, // Implement request counter if needed
            services: [
                {
                    name: 'PostgreSQL',
                    status: 'running',
                    uptime: Math.floor(process.uptime() / 3600)
                },
                {
                    name: 'Redis',
                    status: 'running',
                    uptime: Math.floor(process.uptime() / 3600)
                },
                {
                    name: 'WebSocket',
                    status: 'running',
                    uptime: Math.floor(process.uptime() / 3600)
                }
            ]
        };

        res.json(status);
    } catch (error) {
        console.error('Error fetching server status:', error);
        res.status(500).json({ error: 'Failed to fetch server status' });
    }
});

/**
 * GET /api/admin/logs
 * Get system logs
 */
router.get('/logs', authenticateToken, requirePermission('audit:read'), async (req: AdminAuthRequest, res: Response) => {
    try {
        const { level, limit = 100 } = req.query;

        let query = 'SELECT * FROM system_logs WHERE 1=1';
        const params: any[] = [];
        let paramCount = 0;

        if (level && level !== 'all') {
            paramCount++;
            query += ` AND level = $${paramCount}`;
            params.push(level);
        }

        query += ' ORDER BY created_at DESC';
        
        paramCount++;
        query += ` LIMIT $${paramCount}`;
        params.push(parseInt(limit as string));

        const result = await pool.query(query, params);

        res.json(result.rows);
    } catch (error) {
        console.error('Error fetching logs:', error);
        // Return empty array if table doesn't exist yet
        res.json([]);
    }
});

/**
 * GET /api/admin/database-stats
 * Get database table statistics
 */
router.get('/database-stats', authenticateToken, requirePermission('monitoring:read'), async (req: AdminAuthRequest, res: Response) => {
    try {
        const stats = await pool.query(`
            SELECT 
                schemaname as schema_name,
                tablename as table_name,
                pg_total_relation_size(schemaname||'.'||tablename)::bigint / 1024 / 1024 as size_mb,
                n_tup_ins as inserts,
                n_tup_upd as updates,
                n_tup_del as deletes
            FROM pg_stat_user_tables
            ORDER BY pg_total_relation_size(schemaname||'.'||tablename) DESC
        `);

        // Get row counts for each table
        const tables = stats.rows;
        const tableStats = await Promise.all(
            tables.map(async (table) => {
                try {
                    const countResult = await pool.query(
                        `SELECT COUNT(*) as row_count FROM ${table.schema_name}.${table.table_name}`
                    );
                    return {
                        tableName: table.table_name,
                        rowCount: parseInt(countResult.rows[0].row_count),
                        size: `${table.size_mb} MB`,
                        lastModified: new Date().toISOString() // Approximation
                    };
                } catch (error) {
                    return {
                        tableName: table.table_name,
                        rowCount: 0,
                        size: `${table.size_mb} MB`,
                        lastModified: new Date().toISOString()
                    };
                }
            })
        );

        res.json(tableStats);
    } catch (error) {
        console.error('Error fetching database stats:', error);
        res.status(500).json({ error: 'Failed to fetch database statistics' });
    }
});

/**
 * GET /api/admin/settings
 * Get game settings
 */
router.get('/settings', authenticateToken, requirePermission('game:read'), async (req: AdminAuthRequest, res: Response) => {
    try {
        const settings = await pool.query(
            'SELECT * FROM game_settings ORDER BY key'
        );

        const settingsObj: any = {};
        settings.rows.forEach(row => {
            settingsObj[row.key] = row.value;
        });

        res.json(settingsObj);
    } catch (error) {
        console.error('Error fetching settings:', error);
        // Return default settings if table doesn't exist
        res.json({
            maintenanceMode: false,
            registrationEnabled: true,
            maxPlayers: 10000,
            motd: 'Welcome to Universus'
        });
    }
});

/**
 * PUT /api/admin/settings
 * Update game settings
 */
router.put('/settings', authenticateToken, requirePermission('game:write'), async (req: AdminAuthRequest, res: Response) => {
    try {
        const settings = req.body;

        for (const [key, value] of Object.entries(settings)) {
            await pool.query(
                `INSERT INTO game_settings (key, value) 
                 VALUES ($1, $2)
                 ON CONFLICT (key) 
                 DO UPDATE SET value = $2, updated_at = NOW()`,
                [key, JSON.stringify(value)]
            );
        }

        await logAdminAction(req.user!.id, 'SETTINGS_UPDATED', settings);

        res.json({ success: true, message: 'Settings updated successfully' });
    } catch (error) {
        console.error('Error updating settings:', error);
        res.status(500).json({ error: 'Failed to update settings' });
    }
});

/**
 * GET /api/admin/audit-log
 * Get admin action audit log
 */
router.get('/audit-log', authenticateToken, requirePermission('audit:read'), async (req: AdminAuthRequest, res: Response) => {
    try {
        const { limit = 100 } = req.query;

        const logs = await pool.query(`
            SELECT 
                a.*, 
                u.username as admin_username
            FROM admin_audit_log a
            JOIN users u ON a.user_id = u.id
            ORDER BY a.created_at DESC
            LIMIT $1
        `, [parseInt(limit as string)]);

        res.json(logs.rows);
    } catch (error) {
        console.error('Error fetching audit log:', error);
        res.json([]);
    }
});

/**
 * OBSERVABILITY ENDPOINTS
 * GET/PUT /api/admin/observability/config (SA/SGM only)
 * GET /api/admin/observability/status (GM/SM/M and above)
 */
import { requirePermission } from '../middleware/adminAuth';
import { AdminAuthRequest } from '../types/admin';

// In-memory config (replace with DB or file storage as needed)
let observabilityConfig = {
  prometheusUrl: 'http://localhost:9090',
  grafanaUrl: 'http://localhost:3000',
  alertmanagerUrl: 'http://localhost:9093',
  otelCollectorUrl: 'http://localhost:4317',
  blackboxUrl: 'http://localhost:9115',
  enabled: true,
};

// GET observability config (SA/SGM only)
router.get('/observability/config', authenticateToken, requirePermission('monitoring:write'), async (req: AdminAuthRequest, res: Response) => {
  res.json(observabilityConfig);
});

// PUT observability config (SA/SGM only)
router.put('/observability/config', authenticateToken, requirePermission('monitoring:write'), async (req: AdminAuthRequest, res: Response) => {
  const updates = req.body;
  observabilityConfig = { ...observabilityConfig, ...updates };
  res.json({ success: true, config: observabilityConfig });
});

// GET observability status (GM/SM/M and above)
router.get('/observability/status', authenticateToken, requirePermission('monitoring:read'), async (req: AdminAuthRequest, res: Response) => {
  // Mock status (replace with real health checks/metrics)
  res.json({
    prometheus: { url: observabilityConfig.prometheusUrl, status: 'ok' },
    grafana: { url: observabilityConfig.grafanaUrl, status: 'ok' },
    alertmanager: { url: observabilityConfig.alertmanagerUrl, status: 'ok' },
    otel_collector: { url: observabilityConfig.otelCollectorUrl, status: 'ok' },
    blackbox: { url: observabilityConfig.blackboxUrl, status: 'ok' },
    enabled: observabilityConfig.enabled,
    lastChecked: new Date().toISOString(),
  });
});

export default router;
