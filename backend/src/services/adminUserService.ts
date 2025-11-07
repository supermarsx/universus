import { pool } from '../config/database';
import {
  UserBlock,
  PlayerTag,
  BlockType,
  TagCategory,
  BlockUserAction,
  TagUserAction,
  AdjustResourcesAction,
  PaginatedResponse,
  AdminFilter,
  BulkAction,
  UserAnalytics,
} from '../types/admin';
import { logAdminAction } from '../middleware/adminAuth';

/**
 * Admin User Management Service
 * Handles all user administration operations
 */
export class AdminUserService {
  /**
   * Get all users with filtering and pagination
   */
  static async getUsers(filter: AdminFilter): Promise<PaginatedResponse<any>> {
    const page = filter.page || 1;
    const limit = filter.limit || 50;
    const offset = (page - 1) * limit;

    let whereConditions: string[] = [];
    let params: any[] = [];
    let paramIndex = 1;

    // Build WHERE conditions
    if (filter.search) {
      whereConditions.push(`(u.username ILIKE $${paramIndex} OR u.email ILIKE $${paramIndex})`);
      params.push(`%${filter.search}%`);
      paramIndex++;
    }

    if (filter.status) {
      whereConditions.push(`u.account_status = $${paramIndex}`);
      params.push(filter.status);
      paramIndex++;
    }

    if (filter.dateFrom) {
      whereConditions.push(`u.created_at >= $${paramIndex}`);
      params.push(filter.dateFrom);
      paramIndex++;
    }

    if (filter.dateTo) {
      whereConditions.push(`u.created_at <= $${paramIndex}`);
      params.push(filter.dateTo);
      paramIndex++;
    }

    const whereClause = whereConditions.length > 0 
      ? `WHERE ${whereConditions.join(' AND ')}` 
      : '';

    // Get total count
    const countResult = await pool.query(
      `SELECT COUNT(*) FROM users u ${whereClause}`,
      params
    );
    if (!countResult.rows[0]) {
      throw new Error('Failed to get user count: no result returned');
    }
    const total = parseInt(countResult.rows[0].count);

    // Get paginated data
    const sortBy = filter.sortBy || 'created_at';
    const sortOrder = filter.sortOrder || 'DESC';
    
    const dataResult = await pool.query(
      `SELECT 
        u.*,
        COALESCE(
          (SELECT json_agg(json_build_object('name', tag_name, 'category', tag_category, 'color', tag_color))
           FROM admin_player_tags WHERE user_id = u.id AND is_active = TRUE),
          '[]'
        ) as tags,
        COALESCE(
          (SELECT COUNT(*) FROM user_blocks WHERE user_id = u.id AND is_active = TRUE),
          0
        ) as active_blocks,
        (SELECT last_login FROM admin_users WHERE user_id = u.id) as admin_last_login
       FROM users u
       ${whereClause}
       ORDER BY ${sortBy} ${sortOrder}
       LIMIT $${paramIndex} OFFSET $${paramIndex + 1}`,
      [...params, limit, offset]
    );

    return {
      data: dataResult.rows,
      total,
      page,
      limit,
      totalPages: Math.ceil(total / limit),
    };
  }

  /**
   * Get user by ID with full details
   */
  static async getUserDetails(userId: number): Promise<any> {
    const result = await pool.query(
      `SELECT 
        u.*,
        COALESCE(
          (SELECT json_agg(apt.*)
           FROM admin_player_tags apt WHERE apt.user_id = u.id AND apt.is_active = TRUE),
          '[]'
        ) as tags,
        COALESCE(
          (SELECT json_agg(ub.*)
           FROM user_blocks ub WHERE ub.user_id = u.id ORDER BY ub.start_time DESC LIMIT 10),
          '[]'
        ) as blocks,
        (SELECT COUNT(*) FROM planets WHERE user_id = u.id) as planet_count,
        (SELECT SUM(metal + crystal + deuterium) FROM planets WHERE user_id = u.id) as total_resources,
        (SELECT COUNT(*) FROM fleets WHERE user_id = u.id) as fleet_count
       FROM users u
       WHERE u.id = $1`,
      [userId]
    );

    if (result.rows.length === 0) {
      throw new Error('User not found');
    }

    if (!result.rows[0]) {
      throw new Error('Failed to get user details: no result returned');
    }
    return result.rows[0];
  }

  /**
   * Block/Ban a user
   */
  static async blockUser(
    action: BlockUserAction,
    adminId: number,
    adminUsername: string
  ): Promise<UserBlock> {
    const endTime = action.is_permanent
      ? null
      : action.duration_minutes
      ? new Date(Date.now() + action.duration_minutes * 60000)
      : null;

    const result = await pool.query(
      `INSERT INTO user_blocks (
        user_id, block_type, reason, duration_minutes, end_time,
        is_permanent, blocked_by, severity_level
      ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
      RETURNING *`,
      [
        action.user_id,
        action.block_type,
        action.reason,
        action.duration_minutes || null,
        endTime,
        action.is_permanent || false,
        adminId,
        action.severity_level || 3,
      ]
    );

    if (!result.rows[0]) {
      throw new Error('Failed to block user: no result returned');
    }

    // Update user status
    if (action.block_type === 'ban') {
      await pool.query(
        `UPDATE users SET account_status = 'banned' WHERE id = $1`,
        [action.user_id]
      );
    }

    // Log action
    await logAdminAction(
      adminId,
      adminUsername,
      `block_user_${action.block_type}`,
      'user_management',
      'user',
      action.user_id,
      action,
      action.block_type === 'ban' ? 'high' : 'medium'
    );

    return result.rows[0];
  }

  /**
   * Unblock a user
   */
  static async unblockUser(
    blockId: number,
    reason: string,
    adminId: number,
    adminUsername: string
  ): Promise<void> {
    const blockResult = await pool.query(
      `SELECT * FROM user_blocks WHERE id = $1`,
      [blockId]
    );

    if (blockResult.rows.length === 0) {
      throw new Error('Block not found');
    }

    if (!blockResult.rows[0]) {
      throw new Error('Failed to get block details: no result returned');
    }
    const block = blockResult.rows[0];

    await pool.query(
      `UPDATE user_blocks 
       SET is_active = FALSE, unblocked_by = $1, unblock_time = NOW(), unblock_reason = $2
       WHERE id = $3`,
      [adminId, reason, blockId]
    );

    // Update user status if it was a ban
    if (block.block_type === 'ban') {
      await pool.query(
        `UPDATE users SET account_status = 'active' WHERE id = $1`,
        [block.user_id]
      );
    }

    // Log action
    await logAdminAction(
      adminId,
      adminUsername,
      'unblock_user',
      'user_management',
      'user',
      block.user_id,
      { block_id: blockId, reason },
      'medium'
    );
  }

  /**
   * Tag a user
   */
  static async tagUser(
    action: TagUserAction,
    adminId: number,
    adminUsername: string
  ): Promise<PlayerTag> {
    const result = await pool.query(
      `INSERT INTO admin_player_tags (
        user_id, tag_name, tag_category, tag_color, description, added_by, expires_at
      ) VALUES ($1, $2, $3, $4, $5, $6, $7)
      ON CONFLICT (user_id, tag_name) DO UPDATE
      SET is_active = TRUE, added_at = NOW()
      RETURNING *`,
      [
        action.user_id,
        action.tag_name,
        action.tag_category,
        action.tag_color || '#3b82f6',
        action.description || null,
        adminId,
        action.expires_at || null,
      ]
    );

    if (!result.rows[0]) {
      throw new Error('Failed to tag user: no result returned');
    }

    // Log action
    await logAdminAction(
      adminId,
      adminUsername,
      'tag_user',
      'user_management',
      'user',
      action.user_id,
      action,
      'low'
    );

    return result.rows[0];
  }

  /**
   * Remove tag from user
   */
  static async removeTag(
    tagId: number,
    adminId: number,
    adminUsername: string
  ): Promise<void> {
    const tagResult = await pool.query(
      `SELECT * FROM admin_player_tags WHERE id = $1`,
      [tagId]
    );

    if (tagResult.rows.length === 0) {
      throw new Error('Tag not found');
    }

    if (!tagResult.rows[0]) {
      throw new Error('Failed to get tag details: no result returned');
    }

    await pool.query(
      `UPDATE admin_player_tags SET is_active = FALSE WHERE id = $1`,
      [tagId]
    );

    // Log action
    await logAdminAction(
      adminId,
      adminUsername,
      'remove_tag',
      'user_management',
      'user',
      tagResult.rows[0].user_id,
      { tag_id: tagId },
      'low'
    );
  }

  /**
   * Adjust user resources
   */
  static async adjustResources(
    action: AdjustResourcesAction,
    adminId: number,
    adminUsername: string
  ): Promise<void> {
    const client = await pool.connect();

    try {
      await client.query('BEGIN');

      // Get current state
      const beforeState = await client.query(
        action.planet_id
          ? `SELECT metal, crystal, deuterium FROM planets WHERE id = $1`
          : `SELECT dark_matter FROM users WHERE id = $1`,
        [action.planet_id || action.user_id]
      );

      // Update resources
      if (action.planet_id) {
        await client.query(
          `UPDATE planets 
           SET metal = metal + $1, crystal = crystal + $2, deuterium = deuterium + $3
           WHERE id = $4`,
          [action.metal || 0, action.crystal || 0, action.deuterium || 0, action.planet_id]
        );
      }

      if (action.dark_matter) {
        await client.query(
          `UPDATE users SET dark_matter = dark_matter + $1 WHERE id = $2`,
          [action.dark_matter, action.user_id]
        );
      }

      // Get new state
      const afterState = await client.query(
        action.planet_id
          ? `SELECT metal, crystal, deuterium FROM planets WHERE id = $1`
          : `SELECT dark_matter FROM users WHERE id = $1`,
        [action.planet_id || action.user_id]
      );

      await client.query('COMMIT');

      // Log action
      if (!beforeState.rows[0] || !afterState.rows[0]) {
        throw new Error('Failed to get resource state for logging: no result returned');
      }
      await logAdminAction(
        adminId,
        adminUsername,
        'adjust_resources',
        'data_modification',
        action.planet_id ? 'planet' : 'user',
        action.planet_id || action.user_id,
        action,
        'high',
        true,
        null,
        beforeState.rows[0],
        afterState.rows[0]
      );
    } catch (error) {
      await client.query('ROLLBACK');
      throw error;
    } finally {
      client.release();
    }
  }

  /**
   * Bulk action on users
   */
  static async bulkAction(
    action: BulkAction,
    adminId: number,
    adminUsername: string
  ): Promise<{ success: number; failed: number; errors: any[] }> {
    let success = 0;
    let failed = 0;
    const errors: any[] = [];

    for (const userId of action.target_ids) {
      try {
        switch (action.action) {
          case 'ban':
            await this.blockUser(
              {
                user_id: userId,
                block_type: 'ban',
                reason: action.reason || 'Bulk ban',
                is_permanent: action.params?.is_permanent,
                duration_minutes: action.params?.duration_minutes,
              },
              adminId,
              adminUsername
            );
            break;

          case 'tag':
            await this.tagUser(
              {
                user_id: userId,
                tag_name: action.params?.tag_name,
                tag_category: action.params?.tag_category,
              },
              adminId,
              adminUsername
            );
            break;

          default:
            throw new Error(`Unknown action: ${action.action}`);
        }

        success++;
      } catch (error: any) {
        failed++;
        errors.push({ user_id: userId, error: error.message });
      }
    }

    // Log bulk action
    await logAdminAction(
      adminId,
      adminUsername,
      `bulk_${action.action}`,
      'user_management',
      'bulk',
      null,
      {
        target_count: action.target_ids.length,
        success,
        failed,
        action: action.action,
      },
      'high'
    );

    return { success, failed, errors };
  }

  /**
   * Get user analytics
   */
  static async getUserAnalytics(): Promise<UserAnalytics> {
    const result = await pool.query(`
      SELECT 
        COUNT(*) as total_users,
        COUNT(*) FILTER (WHERE last_login > NOW() - INTERVAL '1 day') as active_users_today,
        COUNT(*) FILTER (WHERE last_login > NOW() - INTERVAL '7 days') as active_users_week,
        COUNT(*) FILTER (WHERE last_login > NOW() - INTERVAL '30 days') as active_users_month,
        COUNT(*) FILTER (WHERE created_at > NOW() - INTERVAL '1 day') as new_users_today,
        COUNT(*) FILTER (WHERE created_at > NOW() - INTERVAL '7 days') as new_users_week,
        COUNT(*) FILTER (WHERE created_at > NOW() - INTERVAL '30 days') as new_users_month,
        COUNT(*) FILTER (WHERE is_banned = TRUE) as banned_users,
        COUNT(*) FILTER (WHERE account_status = 'suspended') as suspended_users
      FROM users
    `);

    if (!result.rows[0]) {
      throw new Error('Failed to get user analytics: no result returned');
    }
    const stats = result.rows[0];

    return {
      total_users: parseInt(stats.total_users),
      active_users_today: parseInt(stats.active_users_today),
      active_users_week: parseInt(stats.active_users_week),
      active_users_month: parseInt(stats.active_users_month),
      new_users_today: parseInt(stats.new_users_today),
      new_users_week: parseInt(stats.new_users_week),
      new_users_month: parseInt(stats.new_users_month),
      banned_users: parseInt(stats.banned_users),
      suspended_users: parseInt(stats.suspended_users),
      retention_rate_7day: stats.active_users_week / Math.max(stats.new_users_week, 1),
      retention_rate_30day: stats.active_users_month / Math.max(stats.new_users_month, 1),
      avg_session_duration: 0, // Would need session tracking
      churn_rate: 0, // Would need historical data
    };
  }
}
