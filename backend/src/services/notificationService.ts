/**
 * @module backend/services/notificationService
 *
 * PHASE 6: NOTIFICATION SERVICE
 * Comprehensive notification system for all game events. Handles creation,
 * batching and delivery of notifications and integrates with the realtime
 * system to push notifications to clients.
 */

import { pool } from '../config/database';
import redis from '../config/redis';
import { getRealtimeHandler } from '../socket';
import {
  Notification,
  NotificationType,
  NotificationCategory,
  NotificationPreferences,
  CreateNotificationRequest,
  GetNotificationsRequest,
  NotificationsResponse,
  NotificationBatch,
  UserUnreadStats,
  NotificationEvent,
} from '../types/realtime';

class NotificationService {
  // =====================================================
  // NOTIFICATION TYPES
  // =====================================================

  async getAllNotificationTypes(): Promise<NotificationType[]> {
    const result = await pool.query(
      `SELECT * FROM notification_types WHERE is_active = TRUE ORDER BY category, type_name`
    );
    return result.rows;
  }

  async getNotificationTypeByName(typeName: string): Promise<NotificationType | null> {
    const result = await pool.query(
      `SELECT * FROM notification_types WHERE type_name = $1`,
      [typeName]
    );
    return result.rows[0] || null;
  }

  // =====================================================
  // CREATE NOTIFICATIONS
  // =====================================================

  async createNotification(request: CreateNotificationRequest): Promise<Notification> {
    const {
      userId,
      notificationTypeId,
      title,
      message,
      priority = 1,
      actionUrl,
      actionLabel,
      referenceType,
      referenceId,
      metadata,
    } = request;

    // Check user preferences
    const shouldSend = await this.checkUserPreferences(userId, notificationTypeId, priority);
    if (!shouldSend) {
      throw new Error('Notification blocked by user preferences');
    }

    const result = await pool.query(
      `INSERT INTO notifications 
       (user_id, notification_type_id, title, message, priority, action_url, 
        action_label, reference_type, reference_id, metadata)
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
       RETURNING *`,
      [
        userId,
        notificationTypeId,
        title,
        message,
        priority,
        actionUrl,
        actionLabel,
        referenceType,
        referenceId,
        metadata ? JSON.stringify(metadata) : null,
      ]
    );

    const notification = result.rows[0];

    // Get notification type info
    const typeInfo = await pool.query(
      `SELECT type_name, category, icon FROM notification_types WHERE id = $1`,
      [notificationTypeId]
    );

    // Increment unread count in Redis
    await this.incrementUnreadCount(userId);

    const payload: Notification = {
      ...notification,
      type_name: typeInfo.rows[0]?.type_name,
      category: typeInfo.rows[0]?.category,
      icon: typeInfo.rows[0]?.icon,
    };

    this.emitRealtimeNotification(payload);

    return payload;
  }

  async createBatchNotifications(batch: NotificationBatch): Promise<number> {
    const { userIds, notificationTypeId, title, message, priority = 1, metadata } = batch;

    if (userIds.length === 0) {
      return 0;
    }

    // Build bulk insert
    const values: string[] = [];
    const params: any[] = [];
    let paramIndex = 1;

    for (const userId of userIds) {
      // Check user preferences
      const shouldSend = await this.checkUserPreferences(userId, notificationTypeId, priority);
      if (!shouldSend) continue;

      values.push(
        `($${paramIndex++}, $${paramIndex++}, $${paramIndex++}, $${paramIndex++}, $${paramIndex++}, $${paramIndex++})`
      );
      params.push(
        userId,
        notificationTypeId,
        title,
        message,
        priority,
        metadata ? JSON.stringify(metadata) : null
      );
    }

    if (values.length === 0) {
      return 0;
    }

    const query = `
      INSERT INTO notifications 
      (user_id, notification_type_id, title, message, priority, metadata)
      VALUES ${values.join(', ')}
    `;

    const result = await pool.query(query, params);

    // Increment unread counts in Redis
    await Promise.all(userIds.map((userId) => this.incrementUnreadCount(userId)));

    return result.rowCount || 0;
  }

  // Quick notification creators for common events
  async notifyFleetArrived(userId: number, fleetId: number, location: string): Promise<void> {
    const type = await this.getNotificationTypeByName('fleet_arrived');
    if (!type) return;

    await this.createNotification({
      userId,
      notificationTypeId: type.id,
      title: 'Fleet Arrived',
      message: `Your fleet has arrived at ${location}`,
      priority: 2,
      actionUrl: `/fleet?id=${fleetId}`,
      actionLabel: 'View Fleet',
      referenceType: 'fleet',
      referenceId: fleetId,
    });
  }

  async notifyFleetReturned(userId: number, fleetId: number, location: string): Promise<void> {
    const type = await this.getNotificationTypeByName('fleet_returned');
    if (!type) return;

    await this.createNotification({
      userId,
      notificationTypeId: type.id,
      title: 'Fleet Returned',
      message: `Your fleet returned to ${location}`,
      priority: 2,
      actionUrl: `/fleet?id=${fleetId}`,
      actionLabel: 'View Fleet',
      referenceType: 'fleet',
      referenceId: fleetId,
    });
  }

  async notifyCombatReport(userId: number, combatId: number, winner: string, location: string): Promise<void> {
    const type = await this.getNotificationTypeByName('combat_report');
    if (!type) return;

    await this.createNotification({
      userId,
      notificationTypeId: type.id,
      title: 'Combat Report Available',
      message: `${winner.toUpperCase()} at ${location}`,
      priority: 3,
      actionUrl: `/combat?id=${combatId}`,
      actionLabel: 'View Report',
      referenceType: 'combat',
      referenceId: combatId,
    });
  }

  async notifyColonizationResult(
    userId: number,
    location: string,
    success: boolean,
    planetId?: number
  ): Promise<void> {
    const typeName = success ? 'colonization_success' : 'colonization_failed';
    const fallbackType = await this.getNotificationTypeByName(typeName);
    const defaultType =
      fallbackType || (await this.getNotificationTypeByName(success ? 'fleet_arrived' : 'fleet_returned'));

    if (!defaultType) return;

    await this.createNotification({
      userId,
      notificationTypeId: defaultType.id,
      title: success ? 'Colonization Successful' : 'Colonization Failed',
      message: success
        ? `A new colony has been established at ${location}.`
        : `Colonization attempt at ${location} failed.`,
      priority: 2,
      actionUrl: success ? '/overview' : '/fleet',
      actionLabel: success ? 'View Colony' : 'Review Fleet',
      referenceType: success ? 'planet' : 'fleet',
      referenceId: success ? planetId : undefined,
    });
  }

  async notifyUnderAttack(
    userId: number,
    attackerName: string,
    planetName: string,
    combatId: number
  ): Promise<void> {
    const type = await this.getNotificationTypeByName('under_attack');
    if (!type) return;

    await this.createNotification({
      userId,
      notificationTypeId: type.id,
      title: 'Under Attack!',
      message: `${attackerName} is attacking your planet ${planetName}!`,
      priority: 5,
      actionUrl: `/combat?id=${combatId}`,
      actionLabel: 'View Battle',
      referenceType: 'combat',
      referenceId: combatId,
    });
  }

  async notifyBuildingComplete(
    userId: number,
    buildingName: string,
    planetId: number
  ): Promise<void> {
    const type = await this.getNotificationTypeByName('building_complete');
    if (!type) return;

    await this.createNotification({
      userId,
      notificationTypeId: type.id,
      title: 'Building Complete',
      message: `${buildingName} construction completed`,
      priority: 1,
      actionUrl: `/planet/${planetId}`,
      actionLabel: 'View Planet',
      referenceType: 'planet',
      referenceId: planetId,
    });
  }

  async notifyResearchComplete(userId: number, technologyName: string): Promise<void> {
    const type = await this.getNotificationTypeByName('research_complete');
    if (!type) return;

    await this.createNotification({
      userId,
      notificationTypeId: type.id,
      title: 'Research Complete',
      message: `${technologyName} research completed`,
      priority: 2,
      actionUrl: '/research',
      actionLabel: 'View Research',
      referenceType: 'research',
      referenceId: 0,
    });
  }

  async notifyTradeComplete(
    userId: number,
    resource: string,
    amount: number,
    tradeId: number
  ): Promise<void> {
    const type = await this.getNotificationTypeByName('trade_complete');
    if (!type) return;

    await this.createNotification({
      userId,
      notificationTypeId: type.id,
      title: 'Trade Completed',
      message: `Trade completed: ${amount.toLocaleString()} ${resource}`,
      priority: 2,
      actionUrl: `/trade?id=${tradeId}`,
      actionLabel: 'View Trade',
      referenceType: 'trade',
      referenceId: tradeId,
    });
  }

  async notifyAllianceInvite(
    userId: number,
    allianceName: string,
    allianceId: number
  ): Promise<void> {
    const type = await this.getNotificationTypeByName('alliance_invite');
    if (!type) return;

    await this.createNotification({
      userId,
      notificationTypeId: type.id,
      title: 'Alliance Invitation',
      message: `You have been invited to join ${allianceName}`,
      priority: 3,
      actionUrl: `/alliance/${allianceId}`,
      actionLabel: 'View Invitation',
      referenceType: 'alliance',
      referenceId: allianceId,
    });
  }

  // =====================================================
  // READ NOTIFICATIONS
  // =====================================================

  async getUserNotifications(request: GetNotificationsRequest): Promise<NotificationsResponse> {
    const { userId, unreadOnly = false, category, limit = 50, offset = 0 } = request;

    let query = `
      SELECT 
        n.*,
        nt.type_name,
        nt.category,
        nt.icon
      FROM notifications n
      JOIN notification_types nt ON n.notification_type_id = nt.id
      WHERE n.user_id = $1 AND n.is_archived = FALSE
    `;
    const params: any[] = [userId];
    let paramIndex = 2;

    if (unreadOnly) {
      query += ` AND n.is_read = FALSE`;
    }

    if (category) {
      query += ` AND nt.category = $${paramIndex++}`;
      params.push(category);
    }

    // Check for expired notifications
    query += ` AND (n.expires_at IS NULL OR n.expires_at > CURRENT_TIMESTAMP)`;

    query += ` ORDER BY n.created_at DESC LIMIT $${paramIndex++} OFFSET $${paramIndex++}`;
    params.push(limit, offset);

    const result = await pool.query(query, params);

    // Get unread count
    const unreadResult = await pool.query(
      `SELECT COUNT(*) FROM notifications 
       WHERE user_id = $1 AND is_read = FALSE AND is_archived = FALSE
         AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP)`,
      [userId]
    );

    // Get total count
    const totalResult = await pool.query(
      `SELECT COUNT(*) FROM notifications 
       WHERE user_id = $1 AND is_archived = FALSE
         AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP)`,
      [userId]
    );

    return {
      notifications: result.rows,
      total: parseInt(totalResult.rows[0].count),
      unreadCount: parseInt(unreadResult.rows[0].count),
    };
  }

  async getNotificationById(notificationId: number, userId: number): Promise<Notification | null> {
    const result = await pool.query(
      `SELECT 
         n.*,
         nt.type_name,
         nt.category,
         nt.icon
       FROM notifications n
       JOIN notification_types nt ON n.notification_type_id = nt.id
       WHERE n.id = $1 AND n.user_id = $2`,
      [notificationId, userId]
    );

    return result.rows[0] || null;
  }

  // =====================================================
  // UPDATE NOTIFICATIONS
  // =====================================================

  async markAsRead(notificationId: number, userId: number): Promise<void> {
    const result = await pool.query(
      `UPDATE notifications 
       SET is_read = TRUE, read_at = CURRENT_TIMESTAMP
       WHERE id = $1 AND user_id = $2 AND is_read = FALSE`,
      [notificationId, userId]
    );

    if (result.rowCount && result.rowCount > 0) {
      await this.decrementUnreadCount(userId);
    }
  }

  async markAllAsRead(userId: number): Promise<number> {
    const result = await pool.query(
      `UPDATE notifications 
       SET is_read = TRUE, read_at = CURRENT_TIMESTAMP
       WHERE user_id = $1 AND is_read = FALSE`,
      [userId]
    );

    const count = result.rowCount || 0;
    if (count > 0) {
      await redis.set(`notifications:unread:${userId}`, '0');
    }

    return count;
  }

  async archiveNotification(notificationId: number, userId: number): Promise<void> {
    await pool.query(
      `UPDATE notifications 
       SET is_archived = TRUE, archived_at = CURRENT_TIMESTAMP
       WHERE id = $1 AND user_id = $2`,
      [notificationId, userId]
    );
  }

  async deleteNotification(notificationId: number, userId: number): Promise<void> {
    const result = await pool.query(
      `DELETE FROM notifications WHERE id = $1 AND user_id = $2 RETURNING is_read`,
      [notificationId, userId]
    );

    if (result.rows.length > 0 && !result.rows[0].is_read) {
      await this.decrementUnreadCount(userId);
    }
  }

  // =====================================================
  // USER PREFERENCES
  // =====================================================

  async getUserPreferences(userId: number): Promise<NotificationPreferences[]> {
    const result = await pool.query(
      `SELECT np.*, nt.type_name, nt.category
       FROM notification_preferences np
       JOIN notification_types nt ON np.notification_type_id = nt.id
       WHERE np.user_id = $1
       ORDER BY nt.category, nt.type_name`,
      [userId]
    );

    return result.rows;
  }

  async updatePreference(
    userId: number,
    notificationTypeId: number,
    updates: Partial<NotificationPreferences>
  ): Promise<void> {
    const { enabled, sound_enabled, desktop_enabled, min_priority } = updates;

    await pool.query(
      `INSERT INTO notification_preferences 
       (user_id, notification_type_id, enabled, sound_enabled, desktop_enabled, min_priority)
       VALUES ($1, $2, $3, $4, $5, $6)
       ON CONFLICT (user_id, notification_type_id)
       DO UPDATE SET
         enabled = COALESCE(EXCLUDED.enabled, notification_preferences.enabled),
         sound_enabled = COALESCE(EXCLUDED.sound_enabled, notification_preferences.sound_enabled),
         desktop_enabled = COALESCE(EXCLUDED.desktop_enabled, notification_preferences.desktop_enabled),
         min_priority = COALESCE(EXCLUDED.min_priority, notification_preferences.min_priority),
         updated_at = CURRENT_TIMESTAMP`,
      [userId, notificationTypeId, enabled, sound_enabled, desktop_enabled, min_priority]
    );
  }

  private async checkUserPreferences(
    userId: number,
    notificationTypeId: number,
    priority: number
  ): Promise<boolean> {
    const result = await pool.query(
      `SELECT enabled, min_priority FROM notification_preferences 
       WHERE user_id = $1 AND notification_type_id = $2`,
      [userId, notificationTypeId]
    );

    if (result.rows.length === 0) {
      return true; // No preference set, allow by default
    }

    const pref = result.rows[0];
    return pref.enabled && priority >= pref.min_priority;
  }

  // =====================================================
  // UNREAD COUNT MANAGEMENT (Redis Cache)
  // =====================================================

  async getUnreadCount(userId: number): Promise<number> {
    const cached = await redis.get(`notifications:unread:${userId}`);
    if (cached) {
      return parseInt(cached);
    }

    // Cache miss, query database
    const result = await pool.query(
      `SELECT COUNT(*) FROM notifications 
       WHERE user_id = $1 AND is_read = FALSE AND is_archived = FALSE
         AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP)`,
      [userId]
    );

    const count = parseInt(result.rows[0].count);
    await redis.setex(`notifications:unread:${userId}`, 300, count.toString()); // Cache for 5 minutes

    return count;
  }

  private async incrementUnreadCount(userId: number): Promise<void> {
    await redis.incr(`notifications:unread:${userId}`);
  }

  private async decrementUnreadCount(userId: number): Promise<void> {
    const current = await redis.get(`notifications:unread:${userId}`);
    if (current && parseInt(current) > 0) {
      await redis.decr(`notifications:unread:${userId}`);
    }
  }

  // =====================================================
  // ANALYTICS
  // =====================================================

  async getUserUnreadStats(userId: number): Promise<UserUnreadStats> {
    const result = await pool.query(
      `SELECT * FROM v_user_unread_notifications WHERE user_id = $1`,
      [userId]
    );

    return result.rows[0] || {
      user_id: userId,
      unread_count: 0,
      urgent_count: 0,
      unread_combat: 0,
      unread_fleet: 0,
      unread_trade: 0,
    };
  }

  async getNotificationStats(): Promise<any> {
    const result = await pool.query(`
      SELECT 
        nt.category,
        COUNT(n.id) as total_notifications,
        COUNT(n.id) FILTER (WHERE n.is_read = FALSE) as unread_notifications,
        AVG(EXTRACT(EPOCH FROM (n.read_at - n.created_at))) as avg_read_time_seconds
      FROM notifications n
      JOIN notification_types nt ON n.notification_type_id = nt.id
      WHERE n.created_at > CURRENT_TIMESTAMP - INTERVAL '24 hours'
      GROUP BY nt.category
    `);

    return result.rows;
  }

  // =====================================================
  // CLEANUP
  // =====================================================

  async cleanupOldNotifications(daysToKeep: number = 30): Promise<number> {
    const result = await pool.query(
      `DELETE FROM notifications 
       WHERE is_archived = TRUE 
         AND archived_at < CURRENT_TIMESTAMP - ($1 || ' days')::INTERVAL`,
      [daysToKeep]
    );

    return result.rowCount || 0;
  }

  async cleanupExpiredNotifications(): Promise<number> {
    const result = await pool.query(
      `DELETE FROM notifications 
       WHERE expires_at IS NOT NULL AND expires_at < CURRENT_TIMESTAMP`
    );

    return result.rowCount || 0;
  }

  // Auto-cleanup scheduler (call this every hour)
  async performScheduledCleanup(): Promise<void> {
    const expired = await this.cleanupExpiredNotifications();
    const old = await this.cleanupOldNotifications(30);
    console.log(`Notification cleanup: ${expired} expired, ${old} old notifications removed`);
  }

  private emitRealtimeNotification(notification: Notification): void {
    const handler = getRealtimeHandler();
    if (!handler) return;

    const event: NotificationEvent = {
      notificationId: notification.id,
      userId: notification.user_id,
      type: (notification as any).type_name,
      category: (notification as any).category,
      title: notification.title,
      message: notification.message,
      priority: notification.priority,
      actionUrl: notification.action_url || undefined,
      actionLabel: notification.action_label || undefined,
      icon: (notification as any).icon,
      timestamp: notification.created_at || new Date(),
    };

    handler.broadcastNotification(notification.user_id, event);
  }
}

export default new NotificationService();
