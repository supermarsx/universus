import { pool } from '../config/database';
import {
  ServerMetric,
  ServerHealth,
  AdminNotification,
  NotificationPriority,
} from '../types/admin';
import { logAdminAction } from '../middleware/adminAuth';
import os from 'os';

/**
 * Admin Monitoring Service
 * Handles server monitoring, metrics collection, and health checks
 */
export class AdminMonitoringService {
  /**
   * Collect and store server metrics
   */
  static async collectServerMetrics(): Promise<void> {
    const metrics: Partial<ServerMetric>[] = [];

    // CPU Usage
    const cpuUsage = os.loadavg()[0] / os.cpus().length * 100;
    metrics.push({
      metric_type: 'system',
      metric_name: 'cpu_usage',
      metric_value: cpuUsage,
      metric_unit: 'percent',
    });

    // Memory Usage
    const totalMem = os.totalmem();
    const freeMem = os.freemem();
    const memoryUsage = ((totalMem - freeMem) / totalMem) * 100;
    metrics.push({
      metric_type: 'system',
      metric_name: 'memory_usage',
      metric_value: memoryUsage,
      metric_unit: 'percent',
    });

    // Database connections
    const dbResult = await pool.query(
      `SELECT count(*) as active_connections 
       FROM pg_stat_activity 
       WHERE datname = current_database()`
    );
    metrics.push({
      metric_type: 'database',
      metric_name: 'active_connections',
      metric_value: parseInt(dbResult.rows[0]?.active_connections || '0'),
      metric_unit: 'count',
    });

    // Active players
    const playersResult = await pool.query(
      `SELECT COUNT(*) as active_players 
       FROM users 
       WHERE last_login > NOW() - INTERVAL '15 minutes'`
    );
    metrics.push({
      metric_type: 'game',
      metric_name: 'active_players',
      metric_value: parseInt(playersResult.rows[0]?.active_players || '0'),
      metric_unit: 'count',
    });

    // Insert metrics
    for (const metric of metrics) {
      await pool.query(
        `INSERT INTO server_monitoring (
          metric_type, metric_name, metric_value, metric_unit
        ) VALUES ($1, $2, $3, $4)`,
        [metric.metric_type, metric.metric_name, metric.metric_value, metric.metric_unit]
      );

      // Check thresholds
      await this.checkThreshold(metric as ServerMetric);
    }
  }

  /**
   * Check if metric exceeds threshold and send alerts
   */
  static async checkThreshold(metric: ServerMetric): Promise<void> {
    const thresholds: Record<string, number> = {
      cpu_usage: 80,
      memory_usage: 85,
      active_connections: 15,
      error_rate: 5,
    };

    const threshold = thresholds[metric.metric_name];
    if (!threshold) return;

    if (metric.metric_value > threshold) {
      // Update metric
      await pool.query(
        `UPDATE server_monitoring 
         SET threshold_exceeded = TRUE 
         WHERE id = $1`,
        [metric.id]
      );

      // Create critical notification
      await this.createNotification({
        notification_type: 'threshold_exceeded',
        priority: 'critical',
        title: `${metric.metric_name} Threshold Exceeded`,
        message: `${metric.metric_name} is at ${metric.metric_value.toFixed(2)}${metric.metric_unit} (threshold: ${threshold}${metric.metric_unit})`,
        data: { metric },
        target_admin_level: 'game_admin',
        requires_acknowledgment: true,
      });
    }
  }

  /**
   * Get current server health
   */
  static async getServerHealth(): Promise<ServerHealth> {
    // Get latest metrics from last 5 minutes
    const metricsResult = await pool.query(`
      SELECT 
        metric_name,
        AVG(metric_value) as avg_value,
        metric_unit
      FROM server_monitoring
      WHERE timestamp > NOW() - INTERVAL '5 minutes'
      GROUP BY metric_name, metric_unit
    `);

    const metrics: Record<string, number> = {};
    metricsResult.rows.forEach((row) => {
      metrics[row.metric_name] = parseFloat(row.avg_value);
    });

    const cpuUsage = metrics.cpu_usage || 0;
    const memoryUsage = metrics.memory_usage || 0;
    const activePlayers = metrics.active_players || 0;
    const errorRate = metrics.error_rate || 0;

    // Calculate overall status
    let status: 'healthy' | 'warning' | 'critical' = 'healthy';
    if (cpuUsage > 80 || memoryUsage > 85 || errorRate > 5) {
      status = 'critical';
    } else if (cpuUsage > 60 || memoryUsage > 70 || errorRate > 2) {
      status = 'warning';
    }

    // Get uptime
    const uptime = os.uptime();

    return {
      cpu_usage: cpuUsage,
      memory_usage: memoryUsage,
      database_connections: metrics.active_connections || 0,
      active_players: activePlayers,
      api_response_time: metrics.api_response_time || 0,
      error_rate: errorRate,
      uptime,
      status,
    };
  }

  /**
   * Get metrics history
   */
  static async getMetricsHistory(
    metricName: string,
    hours: number = 24
  ): Promise<ServerMetric[]> {
    const result = await pool.query(
      `SELECT * FROM server_monitoring
       WHERE metric_name = $1
         AND timestamp > NOW() - INTERVAL '${hours} hours'
       ORDER BY timestamp ASC`,
      [metricName]
    );

    return result.rows;
  }

  /**
   * Create admin notification
   */
  static async createNotification(
    notification: Partial<AdminNotification>
  ): Promise<AdminNotification> {
    const result = await pool.query(
      `INSERT INTO admin_notifications (
        notification_type, priority, title, message, data,
        target_admin_level, target_admin_ids, action_url,
        requires_acknowledgment, expires_at
      ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
      RETURNING *`,
      [
        notification.notification_type,
        notification.priority || 'medium',
        notification.title,
        notification.message,
        notification.data ? JSON.stringify(notification.data) : null,
        notification.target_admin_level || null,
        notification.target_admin_ids || null,
        notification.action_url || null,
        notification.requires_acknowledgment || false,
        notification.expires_at || null,
      ]
    );

    if (!result.rows[0]) {
      throw new Error('Failed to create notification: no result returned');
    }
    return result.rows[0];
  }

  /**
   * Get notifications for admin
   */
  static async getNotifications(
    adminId: number,
    adminLevel: string,
    unreadOnly: boolean = false
  ): Promise<AdminNotification[]> {
    let query = `
      SELECT * FROM admin_notifications
      WHERE (
        target_admin_level IS NULL
        OR target_admin_level = $2
        OR target_admin_ids @> ARRAY[$1]::INTEGER[]
      )
      AND (expires_at IS NULL OR expires_at > NOW())
    `;

    if (unreadOnly) {
      query += ` AND (is_read = FALSE OR NOT (read_by @> ARRAY[$1]::INTEGER[]))`;
    }

    query += ` ORDER BY priority DESC, created_at DESC LIMIT 100`;

    const result = await pool.query(query, [adminId, adminLevel]);
    return result.rows;
  }

  /**
   * Mark notification as read
   */
  static async markNotificationRead(
    notificationId: number,
    adminId: number
  ): Promise<void> {
    await pool.query(
      `UPDATE admin_notifications
       SET read_by = array_append(COALESCE(read_by, '{}'), $2)
       WHERE id = $1 AND NOT (read_by @> ARRAY[$2]::INTEGER[])`,
      [notificationId, adminId]
    );
  }

  /**
   * Acknowledge notification
   */
  static async acknowledgeNotification(
    notificationId: number,
    adminId: number
  ): Promise<void> {
    await pool.query(
      `UPDATE admin_notifications
       SET acknowledged_by = array_append(COALESCE(acknowledged_by, '{}'), $2)
       WHERE id = $1 AND NOT (acknowledged_by @> ARRAY[$2]::INTEGER[])`,
      [notificationId, adminId]
    );
  }

  /**
   * Get online admins count
   */
  static async getOnlineAdminsCount(): Promise<number> {
    const result = await pool.query(
      `SELECT COUNT(*) as count
       FROM admin_users au
       JOIN users u ON au.user_id = u.id
       WHERE au.is_active = TRUE
         AND u.last_login > NOW() - INTERVAL '15 minutes'`
    );

    if (!result.rows[0]) {
      return 0;
    }
    return parseInt(result.rows[0].count);
  }

  /**
   * Get real-time player activity
   */
  static async getPlayerActivity(): Promise<any> {
    const result = await pool.query(`
      SELECT 
        COUNT(*) FILTER (WHERE last_login > NOW() - INTERVAL '5 minutes') as online_now,
        COUNT(*) FILTER (WHERE last_login > NOW() - INTERVAL '15 minutes') as online_15min,
        COUNT(*) FILTER (WHERE last_login > NOW() - INTERVAL '1 hour') as online_1hour,
        COUNT(*) FILTER (WHERE created_at > NOW() - INTERVAL '1 day') as new_today,
        (SELECT COUNT(*) FROM fleets WHERE status = 'in_transit') as active_fleets,
        (SELECT COUNT(*) FROM messages WHERE created_at > NOW() - INTERVAL '1 hour') as recent_messages
      FROM users
    `);

    if (!result.rows[0]) {
      throw new Error('Failed to get player activity: no result returned');
    }
    return result.rows[0];
  }

  /**
   * Get database performance stats
   */
  static async getDatabaseStats(): Promise<any> {
    const result = await pool.query(`
      SELECT 
        (SELECT COUNT(*) FROM pg_stat_activity WHERE datname = current_database()) as connections,
        (SELECT pg_database_size(current_database())) as database_size,
        (SELECT SUM(seq_scan + idx_scan) FROM pg_stat_user_tables) as total_scans,
        (SELECT SUM(n_tup_ins + n_tup_upd + n_tup_del) FROM pg_stat_user_tables) as total_modifications
    `);

    if (!result.rows[0]) {
      throw new Error('Failed to get database stats: no result returned');
    }
    return result.rows[0];
  }

  /**
   * Clean up old metrics (retention policy)
   */
  static async cleanupOldMetrics(daysToKeep: number = 30): Promise<number> {
    const result = await pool.query(
      `DELETE FROM server_monitoring
       WHERE timestamp < NOW() - INTERVAL '${daysToKeep} days'`
    );

    return result.rowCount || 0;
  }
}

/**
 * Start monitoring interval
 * Call this when server starts to begin automatic metric collection
 */
export function startMonitoring(intervalMs: number = 60000): NodeJS.Timeout {
  console.log('Starting server monitoring...');
  
  // Collect metrics immediately
  AdminMonitoringService.collectServerMetrics().catch(console.error);
  
  // Then collect every interval
  return setInterval(() => {
    AdminMonitoringService.collectServerMetrics().catch(console.error);
  }, intervalMs);
}

/**
 * Auto-expire user blocks
 * Should be called periodically (e.g., every 5 minutes)
 */
export async function autoExpireBlocks(): Promise<void> {
  const result = await pool.query('SELECT auto_expire_blocks()');
  if (!result.rows[0]) {
    throw new Error('Failed to auto-expire blocks: no result returned');
  }
  const expired = result.rows[0].auto_expire_blocks;
  
  if (expired > 0) {
    console.log(`Auto-expired ${expired} user blocks`);
    await AdminMonitoringService.createNotification({
      notification_type: 'system',
      priority: 'low',
      title: 'User Blocks Expired',
      message: `${expired} user block(s) have been automatically expired`,
      data: { count: expired },
    });
  }
}
