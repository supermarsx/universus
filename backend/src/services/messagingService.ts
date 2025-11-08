import { Pool } from 'pg';
import playerBlockService from './playerBlockService';

/**
 * Message types enumeration
 */
export enum MessageType {
  PLAYER_MESSAGE = 'player_message',
  COMBAT_REPORT = 'combat_report',
  ESPIONAGE_REPORT = 'espionage_report',
  SYSTEM_NOTIFICATION = 'system_notification',
  ALLIANCE_MESSAGE = 'alliance_message',
  ALLIANCE_CIRCULAR = 'alliance_circular',
}

/**
 * Interface representing a message
 */
export interface Message {
  id: number;
  fromUserId: number | null;
  toUserId: number;
  fromUsername?: string;
  toUsername?: string;
  subject: string;
  content: string;
  messageType: MessageType;
  isRead: boolean;
  createdAt: Date;
  metadata?: any;
}

/**
 * Interface for message creation
 */
export interface CreateMessageData {
  fromUserId?: number;
  toUserId: number;
  subject: string;
  content: string;
  messageType: MessageType;
  metadata?: any;
}

/**
 * Messaging Service
 *
 * Handles all in-game messaging operations including:
 * - Player-to-player messages
 * - Combat reports
 * - Espionage reports
 * - System notifications
 * - Alliance communications
 *
 * @class MessagingService
 */
export class MessagingService {
  private db: Pool;

  /**
   * Creates an instance of MessagingService
   *
   * @param {Pool} db - PostgreSQL connection pool
   */
  constructor(db: Pool) {
    this.db = db;
  }

  /**
   * Send a message to a user
   *
   * @param {CreateMessageData} messageData - Message data
   * @returns {Promise<Message>} The created message
   * @throws {Error} If message creation fails
   *
   * @example
   * const message = await messagingService.sendMessage({
   *   fromUserId: 1,
   *   toUserId: 2,
   *   subject: 'Alliance Invitation',
   *   content: 'Would you like to join our alliance?',
   *   messageType: MessageType.PLAYER_MESSAGE
   * });
   */
  async sendMessage(messageData: CreateMessageData): Promise<Message> {
    const client = await this.db.connect();

    try {
      if (
        messageData.fromUserId &&
        (await playerBlockService.isBlockedEither(
          messageData.fromUserId,
          messageData.toUserId,
          'messages'
        ))
      ) {
        throw new Error('Messaging between these players is blocked.');
      }

      await client.query('BEGIN');

      const query = `
        INSERT INTO messages (from_user_id, to_user_id, subject, content, message_type, metadata, created_at)
        VALUES ($1, $2, $3, $4, $5, $6, NOW())
        RETURNING *
      `;

      const values = [
        messageData.fromUserId || null,
        messageData.toUserId,
        messageData.subject,
        messageData.content,
        messageData.messageType,
        messageData.metadata ? JSON.stringify(messageData.metadata) : null,
      ];

      const result = await client.query(query, values);

      await client.query('COMMIT');

      return this.mapRowToMessage(result.rows[0]);
    } catch (error) {
      await client.query('ROLLBACK');
      console.error('Error sending message:', error);
      throw error;
    } finally {
      client.release();
    }
  }

  /**
   * Get user's inbox messages
   *
   * @param {number} userId - The ID of the user
   * @param {number} limit - Maximum number of messages to retrieve (default: 50)
   * @param {number} offset - Pagination offset (default: 0)
   * @param {MessageType} [messageType] - Filter by message type (optional)
   * @returns {Promise<Message[]>} Array of messages
   *
   * @example
   * const inbox = await messagingService.getInbox(123, 20, 0, MessageType.PLAYER_MESSAGE);
   */
  async getInbox(
    userId: number,
    limit: number = 50,
    offset: number = 0,
    messageType?: MessageType
  ): Promise<Message[]> {
    try {
      let query = `
        SELECT 
          m.*,
          u_from.username as from_username,
          u_to.username as to_username
        FROM messages m
        LEFT JOIN users u_from ON m.from_user_id = u_from.id
        LEFT JOIN users u_to ON m.to_user_id = u_to.id
        WHERE m.to_user_id = $1
      `;

      const values: any[] = [userId];

      if (messageType) {
        query += ' AND m.message_type = $2';
        values.push(messageType);
      }

      query += ' ORDER BY m.created_at DESC LIMIT $' + (values.length + 1) + ' OFFSET $' + (values.length + 2);
      values.push(limit, offset);

      const result = await this.db.query(query, values);

      return result.rows.map((row) => this.mapRowToMessage(row));
    } catch (error) {
      console.error('Error getting inbox:', error);
      throw error;
    }
  }

  /**
   * Get user's sent messages
   *
   * @param {number} userId - The ID of the user
   * @param {number} limit - Maximum number of messages to retrieve (default: 50)
   * @param {number} offset - Pagination offset (default: 0)
   * @returns {Promise<Message[]>} Array of sent messages
   */
  async getSentMessages(
    userId: number,
    limit: number = 50,
    offset: number = 0
  ): Promise<Message[]> {
    try {
      const query = `
        SELECT 
          m.*,
          u_from.username as from_username,
          u_to.username as to_username
        FROM messages m
        LEFT JOIN users u_from ON m.from_user_id = u_from.id
        LEFT JOIN users u_to ON m.to_user_id = u_to.id
        WHERE m.from_user_id = $1
        ORDER BY m.created_at DESC
        LIMIT $2 OFFSET $3
      `;

      const result = await this.db.query(query, [userId, limit, offset]);

      return result.rows.map((row) => this.mapRowToMessage(row));
    } catch (error) {
      console.error('Error getting sent messages:', error);
      throw error;
    }
  }

  /**
   * Get a specific message by ID
   *
   * @param {number} messageId - The ID of the message
   * @param {number} userId - The ID of the requesting user
   * @returns {Promise<Message | null>} The message or null if not found/unauthorized
   */
  async getMessage(messageId: number, userId: number): Promise<Message | null> {
    try {
      const query = `
        SELECT 
          m.*,
          u_from.username as from_username,
          u_to.username as to_username
        FROM messages m
        LEFT JOIN users u_from ON m.from_user_id = u_from.id
        LEFT JOIN users u_to ON m.to_user_id = u_to.id
        WHERE m.id = $1 AND (m.to_user_id = $2 OR m.from_user_id = $2)
      `;

      const result = await this.db.query(query, [messageId, userId]);

      if (result.rows.length === 0) {
        return null;
      }

      return this.mapRowToMessage(result.rows[0]);
    } catch (error) {
      console.error('Error getting message:', error);
      throw error;
    }
  }

  /**
   * Mark a message as read
   *
   * @param {number} messageId - The ID of the message
   * @param {number} userId - The ID of the user (must be recipient)
   * @returns {Promise<boolean>} True if marked as read, false if not found/unauthorized
   */
  async markAsRead(messageId: number, userId: number): Promise<boolean> {
    try {
      const query = `
        UPDATE messages
        SET is_read = true
        WHERE id = $1 AND to_user_id = $2 AND is_read = false
        RETURNING id
      `;

      const result = await this.db.query(query, [messageId, userId]);

      return result.rows.length > 0;
    } catch (error) {
      console.error('Error marking message as read:', error);
      throw error;
    }
  }

  /**
   * Mark all messages as read for a user
   *
   * @param {number} userId - The ID of the user
   * @param {MessageType} [messageType] - Filter by message type (optional)
   * @returns {Promise<number>} Number of messages marked as read
   */
  async markAllAsRead(userId: number, messageType?: MessageType): Promise<number> {
    try {
      let query = 'UPDATE messages SET is_read = true WHERE to_user_id = $1 AND is_read = false';
      const values: any[] = [userId];

      if (messageType) {
        query += ' AND message_type = $2';
        values.push(messageType);
      }

      query += ' RETURNING id';

      const result = await this.db.query(query, values);

      return result.rows.length;
    } catch (error) {
      console.error('Error marking all as read:', error);
      throw error;
    }
  }

  /**
   * Delete a message
   *
   * @param {number} messageId - The ID of the message
   * @param {number} userId - The ID of the user (must be recipient or sender)
   * @returns {Promise<boolean>} True if deleted, false if not found/unauthorized
   */
  async deleteMessage(messageId: number, userId: number): Promise<boolean> {
    try {
      const query = `
        DELETE FROM messages
        WHERE id = $1 AND (to_user_id = $2 OR from_user_id = $2)
        RETURNING id
      `;

      const result = await this.db.query(query, [messageId, userId]);

      return result.rows.length > 0;
    } catch (error) {
      console.error('Error deleting message:', error);
      throw error;
    }
  }

  /**
   * Get unread message count for a user
   *
   * @param {number} userId - The ID of the user
   * @param {MessageType} [messageType] - Filter by message type (optional)
   * @returns {Promise<number>} Number of unread messages
   */
  async getUnreadCount(userId: number, messageType?: MessageType): Promise<number> {
    try {
      let query = 'SELECT COUNT(*) as count FROM messages WHERE to_user_id = $1 AND is_read = false';
      const values: any[] = [userId];

      if (messageType) {
        query += ' AND message_type = $2';
        values.push(messageType);
      }

      const result = await this.db.query(query, values);

      return parseInt(result.rows[0].count);
    } catch (error) {
      console.error('Error getting unread count:', error);
      throw error;
    }
  }

  /**
   * Send a combat report
   *
   * @param {number} attackerId - The ID of the attacker
   * @param {number} defenderId - The ID of the defender
   * @param {any} combatData - Combat simulation results
   * @returns {Promise<void>}
   */
  async sendCombatReport(attackerId: number, defenderId: number, combatData: any): Promise<void> {
    try {
      // Send to attacker
      await this.sendMessage({
        fromUserId: undefined,
        toUserId: attackerId,
        subject: 'Combat Report',
        content: 'A battle has occurred at the target planet.',
        messageType: MessageType.COMBAT_REPORT,
        metadata: {
          ...combatData,
          perspective: 'attacker',
        },
      });

      // Send to defender
      await this.sendMessage({
        fromUserId: undefined,
        toUserId: defenderId,
        subject: 'Combat Report - Defend',
        content: 'Your planet was attacked!',
        messageType: MessageType.COMBAT_REPORT,
        metadata: {
          ...combatData,
          perspective: 'defender',
        },
      });
    } catch (error) {
      console.error('Error sending combat reports:', error);
      throw error;
    }
  }

  /**
   * Send an espionage report
   *
   * @param {number} spyUserId - The ID of the spying user
   * @param {number} targetUserId - The ID of the target user
   * @param {any} espionageData - Espionage data
   * @returns {Promise<void>}
   */
  async sendEspionageReport(
    spyUserId: number,
    targetUserId: number,
    espionageData: any
  ): Promise<void> {
    try {
      await this.sendMessage({
        fromUserId: undefined,
        toUserId: spyUserId,
        subject: 'Espionage Report',
        content: `Intelligence gathered from target planet.`,
        messageType: MessageType.ESPIONAGE_REPORT,
        metadata: espionageData,
      });

      // Optionally notify defender if espionage was detected
      if (espionageData.detected) {
        await this.sendMessage({
          fromUserId: undefined,
          toUserId: targetUserId,
          subject: 'Espionage Detected',
          content: 'Enemy spy probes were detected near your planet!',
          messageType: MessageType.SYSTEM_NOTIFICATION,
          metadata: { spyUserId },
        });
      }
    } catch (error) {
      console.error('Error sending espionage report:', error);
      throw error;
    }
  }

  /**
   * Send a system notification
   *
   * @param {number} userId - The ID of the user
   * @param {string} subject - Notification subject
   * @param {string} content - Notification content
   * @param {any} [metadata] - Additional metadata
   * @returns {Promise<void>}
   */
  async sendSystemNotification(
    userId: number,
    subject: string,
    content: string,
    metadata?: any
  ): Promise<void> {
    try {
      await this.sendMessage({
        fromUserId: undefined,
        toUserId: userId,
        subject,
        content,
        messageType: MessageType.SYSTEM_NOTIFICATION,
        metadata,
      });
    } catch (error) {
      console.error('Error sending system notification:', error);
      throw error;
    }
  }

  /**
   * Send an alliance circular message to all members
   *
   * @param {number} allianceId - The ID of the alliance
   * @param {number} senderId - The ID of the sender
   * @param {string} subject - Message subject
   * @param {string} content - Message content
   * @returns {Promise<number>} Number of messages sent
   */
  async sendAllianceCircular(
    allianceId: number,
    senderId: number,
    subject: string,
    content: string
  ): Promise<number> {
    try {
      // Get all alliance members
      const membersQuery = await this.db.query(
        'SELECT id FROM users WHERE alliance_id = $1 AND id != $2',
        [allianceId, senderId]
      );

      let sentCount = 0;

      for (const member of membersQuery.rows) {
        await this.sendMessage({
          fromUserId: senderId,
          toUserId: member.id,
          subject: `[Alliance] ${subject}`,
          content,
          messageType: MessageType.ALLIANCE_CIRCULAR,
          metadata: { allianceId },
        });
        sentCount++;
      }

      return sentCount;
    } catch (error) {
      console.error('Error sending alliance circular:', error);
      throw error;
    }
  }

  /**
   * Map database row to Message interface
   *
   * @private
   * @param {any} row - Database row
   * @returns {Message} Message object
   */
  private mapRowToMessage(row: any): Message {
    return {
      id: row.id,
      fromUserId: row.from_user_id,
      toUserId: row.to_user_id,
      fromUsername: row.from_username,
      toUsername: row.to_username,
      subject: row.subject,
      content: row.content,
      messageType: row.message_type as MessageType,
      isRead: row.is_read,
      createdAt: new Date(row.created_at),
      metadata: row.metadata ? JSON.parse(row.metadata) : undefined,
    };
  }
}
