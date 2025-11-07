import { pool } from '../config/database';
import {
  AdminSetting,
  SettingCategory,
  GameEvent,
  EventType,
  TriggerEventAction,
  ResourceAnalytics,
  CombatAnalytics,
} from '../types/admin';
import { logAdminAction } from '../middleware/adminAuth';

/**
 * Admin Settings Service
 * Manages game configuration and settings
 */
export class AdminSettingsService {
  /**
   * Get all settings
   */
  static async getAllSettings(category?: SettingCategory): Promise<AdminSetting[]> {
    let query = 'SELECT * FROM admin_settings';
    const params: any[] = [];

    if (category) {
      query += ' WHERE setting_category = $1';
      params.push(category);
    }

    query += ' ORDER BY setting_category, setting_key';

    const result = await pool.query(query, params);
    return result.rows.map((row) => ({
      ...row,
      setting_value: row.setting_value,
    }));
  }

  /**
   * Get setting by key
   */
  static async getSetting(key: string): Promise<AdminSetting | null> {
    const result = await pool.query(
      'SELECT * FROM admin_settings WHERE setting_key = $1',
      [key]
    );

    if (result.rows.length === 0) return null;

    if (!result.rows[0]) {
      throw new Error('Failed to get setting: no result returned');
    }
    return {
      ...result.rows[0],
      setting_value: result.rows[0].setting_value,
    };
  }

  /**
   * Update setting
   */
  static async updateSetting(
    key: string,
    value: any,
    adminId: number,
    adminUsername: string
  ): Promise<AdminSetting> {
    const client = await pool.connect();

    try {
      await client.query('BEGIN');

      // Get current setting
      const currentResult = await client.query(
        'SELECT * FROM admin_settings WHERE setting_key = $1',
        [key]
      );

      if (currentResult.rows.length === 0) {
        throw new Error('Setting not found');
      }

      const beforeState = currentResult.rows[0];

      // Update setting
      const result = await client.query(
        `UPDATE admin_settings 
         SET setting_value = $1, modified_by = $2
         WHERE setting_key = $3
         RETURNING *`,
        [JSON.stringify(value), adminId, key]
      );

      if (!result.rows[0]) {
        await client.query('ROLLBACK');
        throw new Error('Failed to update setting: no result returned');
      }
      const afterState = result.rows[0];

      await client.query('COMMIT');

      // Log action
      await logAdminAction(
        adminId,
        adminUsername,
        'update_setting',
        'game_config',
        'setting',
        afterState.id,
        { key, value },
        afterState.requires_restart ? 'high' : 'medium',
        true,
        null,
        { value: beforeState.setting_value },
        { value: afterState.setting_value }
      );

      return {
        ...afterState,
        setting_value: afterState.setting_value,
      };
    } catch (error) {
      await client.query('ROLLBACK');
      throw error;
    } finally {
      client.release();
    }
  }

  /**
   * Create new setting
   */
  static async createSetting(
    setting: Partial<AdminSetting>,
    adminId: number,
    adminUsername: string
  ): Promise<AdminSetting> {
    const result = await pool.query(
      `INSERT INTO admin_settings (
        setting_key, setting_value, setting_category, description,
        data_type, is_public, requires_restart, modified_by
      ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
      RETURNING *`,
      [
        setting.setting_key,
        JSON.stringify(setting.setting_value),
        setting.setting_category,
        setting.description || null,
        setting.data_type || 'string',
        setting.is_public || false,
        setting.requires_restart || false,
        adminId,
      ]
    );

    // Log action
    if (!result.rows[0]) {
      throw new Error('Failed to create setting: no result returned');
    }
    await logAdminAction(
      adminId,
      adminUsername,
      'create_setting',
      'game_config',
      'setting',
      result.rows[0].id,
      setting,
      'medium'
    );

    return result.rows[0];
  }

  /**
   * Delete setting
   */
  static async deleteSetting(
    key: string,
    adminId: number,
    adminUsername: string
  ): Promise<void> {
    const result = await pool.query(
      'DELETE FROM admin_settings WHERE setting_key = $1 RETURNING *',
      [key]
    );

    if (result.rows.length === 0) {
      throw new Error('Setting not found');
    }

    if (!result.rows[0]) {
      throw new Error('Failed to delete setting: no result returned');
    }

    // Log action
    await logAdminAction(
      adminId,
      adminUsername,
      'delete_setting',
      'game_config',
      'setting',
      result.rows[0].id,
      { key },
      'high'
    );
  }

  /**
   * Get setting history
   */
  static async getSettingHistory(key: string, limit: number = 10): Promise<any[]> {
    const result = await pool.query(
      `SELECT 
        al.*,
        u.username as modified_by_username
       FROM admin_audit_logs al
       LEFT JOIN users u ON al.admin_id = u.id
       WHERE al.action_type = 'update_setting'
         AND al.action_details->>'key' = $1
       ORDER BY al.timestamp DESC
       LIMIT $2`,
      [key, limit]
    );

    return result.rows;
  }
}

/**
 * Admin Events Service
 * Manages game events and announcements
 */
export class AdminEventsService {
  /**
   * Create game event
   */
  static async createEvent(
    action: TriggerEventAction,
    adminId: number,
    adminUsername: string
  ): Promise<GameEvent> {
    const result = await pool.query(
      `INSERT INTO game_events (
        event_type, event_name, event_description, event_data,
        start_time, end_time, target_scope, target_ids,
        created_by, priority, rewards
      ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
      RETURNING *`,
      [
        action.event_type,
        action.event_name,
        action.event_description || null,
        action.event_data ? JSON.stringify(action.event_data) : null,
        action.start_time,
        action.end_time || null,
        action.target_scope,
        action.target_ids || null,
        adminId,
        action.priority || 5,
        action.rewards ? JSON.stringify(action.rewards) : null,
      ]
    );

    // Log action
    if (!result.rows[0]) {
      throw new Error('Failed to create event: no result returned');
    }
    await logAdminAction(
      adminId,
      adminUsername,
      'create_event',
      'game_config',
      'event',
      result.rows[0].id,
      action,
      'high'
    );

    return result.rows[0];
  }

  /**
   * Activate event
   */
  static async activateEvent(
    eventId: number,
    adminId: number,
    adminUsername: string
  ): Promise<void> {
    await pool.query(
      'UPDATE game_events SET is_active = TRUE WHERE id = $1',
      [eventId]
    );

    // Log action
    await logAdminAction(
      adminId,
      adminUsername,
      'activate_event',
      'game_config',
      'event',
      eventId,
      null,
      'medium'
    );
  }

  /**
   * Deactivate event
   */
  static async deactivateEvent(
    eventId: number,
    adminId: number,
    adminUsername: string
  ): Promise<void> {
    await pool.query(
      'UPDATE game_events SET is_active = FALSE WHERE id = $1',
      [eventId]
    );

    // Log action
    await logAdminAction(
      adminId,
      adminUsername,
      'deactivate_event',
      'game_config',
      'event',
      eventId,
      null,
      'medium'
    );
  }

  /**
   * Get active events
   */
  static async getActiveEvents(): Promise<GameEvent[]> {
    const result = await pool.query(
      `SELECT * FROM game_events
       WHERE is_active = TRUE
         AND start_time <= NOW()
         AND (end_time IS NULL OR end_time > NOW())
       ORDER BY priority DESC, start_time ASC`
    );

    return result.rows;
  }

  /**
   * Get all events
   */
  static async getAllEvents(limit: number = 50): Promise<GameEvent[]> {
    const result = await pool.query(
      'SELECT * FROM game_events ORDER BY created_at DESC LIMIT $1',
      [limit]
    );

    return result.rows;
  }
}

/**
 * Admin Analytics Service
 * Provides analytics and reporting data
 */
export class AdminAnalyticsService {
  /**
   * Get resource analytics
   */
  static async getResourceAnalytics(): Promise<ResourceAnalytics> {
    const result = await pool.query(`
      SELECT 
        SUM(metal) as total_metal,
        SUM(crystal) as total_crystal,
        SUM(deuterium) as total_deuterium,
        AVG(metal) as avg_metal,
        AVG(crystal) as avg_crystal,
        AVG(deuterium) as avg_deuterium
      FROM planets
    `);

    const stats = result.rows[0];

    return {
      total_metal: parseFloat(stats.total_metal || 0),
      total_crystal: parseFloat(stats.total_crystal || 0),
      total_deuterium: parseFloat(stats.total_deuterium || 0),
      metal_production_rate: 0, // Would need production calculation
      crystal_production_rate: 0,
      deuterium_production_rate: 0,
      avg_resources_per_user: {
        metal: parseFloat(stats.avg_metal || 0),
        crystal: parseFloat(stats.avg_crystal || 0),
        deuterium: parseFloat(stats.avg_deuterium || 0),
      },
    };
  }

  /**
   * Get combat analytics
   */
  static async getCombatAnalytics(): Promise<CombatAnalytics> {
    // This would require combat logs table
    return {
      total_battles: 0,
      battles_today: 0,
      total_ships_destroyed: 0,
      most_used_ships: [],
      top_attackers: [],
      combat_balance_score: 0,
    };
  }

  /**
   * Get audit log statistics
   */
  static async getAuditStats(days: number = 30): Promise<any> {
    const result = await pool.query(
      `SELECT 
        action_category,
        COUNT(*) as action_count,
        COUNT(*) FILTER (WHERE success = TRUE) as successful_actions,
        COUNT(*) FILTER (WHERE success = FALSE) as failed_actions,
        COUNT(DISTINCT admin_id) as unique_admins
       FROM admin_audit_logs
       WHERE timestamp > NOW() - INTERVAL '${days} days'
       GROUP BY action_category`
    );

    return result.rows;
  }

  /**
   * Get top admins by activity
   */
  static async getTopAdmins(days: number = 30, limit: number = 10): Promise<any[]> {
    const result = await pool.query(
      `SELECT 
        admin_username,
        COUNT(*) as action_count,
        array_agg(DISTINCT action_category) as categories
       FROM admin_audit_logs
       WHERE timestamp > NOW() - INTERVAL '${days} days'
       GROUP BY admin_username
       ORDER BY action_count DESC
       LIMIT $1`,
      [limit]
    );

    return result.rows;
  }

  /**
   * Get player distribution by status
   */
  static async getPlayerDistribution(): Promise<any> {
    const result = await pool.query(`
      SELECT 
        account_status,
        COUNT(*) as count,
        ROUND(AVG(EXTRACT(epoch FROM (NOW() - created_at)) / 86400), 2) as avg_age_days
      FROM users
      GROUP BY account_status
    `);

    return result.rows;
  }

  /**
   * Get fleet activity
   */
  static async getFleetActivity(): Promise<any> {
    const result = await pool.query(`
      SELECT 
        mission_type,
        COUNT(*) as count,
        AVG(EXTRACT(epoch FROM (arrival_time - departure_time))) as avg_duration_seconds
      FROM fleets
      WHERE status = 'in_transit'
      GROUP BY mission_type
    `);

    return result.rows;
  }

  /**
   * Get alliance statistics
   */
  static async getAllianceStats(): Promise<any> {
    // Would need alliances table
    return {
      total_alliances: 0,
      total_members: 0,
      avg_members_per_alliance: 0,
      top_alliances: [],
    };
  }
}
