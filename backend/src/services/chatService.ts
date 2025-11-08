/**
 * PHASE 6: CHAT SERVICE
 * Comprehensive chat system for global, alliance, sector, and private messaging
 */

import { pool } from '../config/database';
import redis from '../config/redis';
import playerBlockService from './playerBlockService';
import {
  ChatChannel,
  ChatMessage,
  ChatMessageType,
  PrivateConversation,
  PrivateMessage,
  ChatRestriction,
  ChatRestrictionType,
  ChatHistoryResponse,
  PrivateConversationsResponse,
  PrivateMessagesResponse,
  SendChatMessageRequest,
  SendPrivateMessageRequest,
  GetChatHistoryRequest,
  ChatActivityStats,
  RateLimitInfo,
} from '../types/realtime';

class ChatService {
  // =====================================================
  // CHANNEL MANAGEMENT
  // =====================================================

  async getAllChannels(): Promise<ChatChannel[]> {
    const result = await pool.query(
      `SELECT * FROM chat_channels WHERE is_active = TRUE ORDER BY id`
    );
    return result.rows;
  }

  async getChannelById(channelId: number): Promise<ChatChannel | null> {
    const result = await pool.query(
      `SELECT * FROM chat_channels WHERE id = $1`,
      [channelId]
    );
    return result.rows[0] || null;
  }

  async getChannelByName(channelName: string): Promise<ChatChannel | null> {
    const result = await pool.query(
      `SELECT * FROM chat_channels WHERE channel_name = $1`,
      [channelName]
    );
    return result.rows[0] || null;
  }

  // =====================================================
  // CHAT MESSAGES
  // =====================================================

  async sendMessage(
    userId: number,
    request: SendChatMessageRequest
  ): Promise<ChatMessage> {
    const { channelId, message, messageType = ChatMessageType.TEXT } = request;

    // Check rate limiting
    const canSend = await this.checkRateLimit(userId, channelId);
    if (!canSend) {
      throw new Error('Rate limit exceeded. Please wait before sending another message.');
    }

    // Check restrictions
    const isRestricted = await this.isUserRestricted(userId, channelId);
    if (isRestricted) {
      throw new Error('You are restricted from sending messages in this channel.');
    }

    // Validate message length
    const channel = await this.getChannelById(channelId);
    if (!channel) {
      throw new Error('Channel not found');
    }

    if (message.length > channel.max_message_length) {
      throw new Error(`Message exceeds maximum length of ${channel.max_message_length} characters`);
    }

    // Insert message
    const result = await pool.query(
      `INSERT INTO chat_messages 
       (channel_id, user_id, message, message_type)
       VALUES ($1, $2, $3, $4)
       RETURNING *`,
      [channelId, userId, message, messageType]
    );

    const chatMessage = result.rows[0];
    if (!chatMessage) {
      throw new Error('Failed to create chat message');
    }

    // Update rate limit in Redis
    await this.updateRateLimit(userId, channelId);

    // Get user info for broadcast
    const userInfo = await pool.query(
      `SELECT u.username, a.tag as alliance_tag
       FROM users u
       LEFT JOIN alliances a ON u.alliance_id = a.id
       WHERE u.id = $1`,
      [userId]
    );

    return {
      ...chatMessage,
      username: userInfo.rows[0]?.username,
      alliance_tag: userInfo.rows[0]?.alliance_tag,
    };
  }

  async getChatHistory(request: GetChatHistoryRequest): Promise<ChatHistoryResponse> {
    const { channelId, limit = 50, before } = request;

    let query = `
      SELECT 
        cm.*,
        u.username,
        a.tag as alliance_tag
      FROM chat_messages cm
      JOIN users u ON cm.user_id = u.id
      LEFT JOIN alliances a ON u.alliance_id = a.id
      WHERE cm.channel_id = $1 AND cm.is_deleted = FALSE
    `;
    const params: any[] = [channelId];

    if (before) {
      query += ` AND cm.created_at < $2`;
      params.push(before);
    }

    query += ` ORDER BY cm.created_at DESC LIMIT $${params.length + 1}`;
    params.push(limit + 1); // Fetch one extra to check if there are more

    const result = await pool.query(query, params);
    const messages = result.rows;
    const hasMore = messages.length > limit;

    if (hasMore) {
      messages.pop(); // Remove the extra message
    }

    // Get total count
    const countResult = await pool.query(
      `SELECT COUNT(*) FROM chat_messages WHERE channel_id = $1 AND is_deleted = FALSE`,
      [channelId]
    );
    const total = parseInt(countResult.rows[0]?.count || '0');

    return {
      messages: messages.reverse(), // Reverse to get chronological order
      hasMore,
      total,
    };
  }

  async editMessage(messageId: number, userId: number, newMessage: string): Promise<void> {
    const result = await pool.query(
      `UPDATE chat_messages 
       SET message = $1, is_edited = TRUE, edited_at = CURRENT_TIMESTAMP
       WHERE id = $2 AND user_id = $3 AND is_deleted = FALSE
       RETURNING id`,
      [newMessage, messageId, userId]
    );

    if (result.rows.length === 0) {
      throw new Error('Message not found or cannot be edited');
    }
  }

  async deleteMessage(messageId: number, userId: number, isAdmin: boolean = false): Promise<void> {
    let query: string;
    let params: any[];

    if (isAdmin) {
      query = `UPDATE chat_messages 
               SET is_deleted = TRUE, deleted_at = CURRENT_TIMESTAMP
               WHERE id = $1`;
      params = [messageId];
    } else {
      query = `UPDATE chat_messages 
               SET is_deleted = TRUE, deleted_at = CURRENT_TIMESTAMP
               WHERE id = $1 AND user_id = $2`;
      params = [messageId, userId];
    }

    const result = await pool.query(query, params);

    if (result.rowCount === 0) {
      throw new Error('Message not found or cannot be deleted');
    }
  }

  async flagMessage(
    messageId: number,
    flaggedBy: number,
    reason: string
  ): Promise<void> {
    await pool.query(
      `UPDATE chat_messages 
       SET is_flagged = TRUE, flag_reason = $1, flagged_by = $2, flagged_at = CURRENT_TIMESTAMP
       WHERE id = $3`,
      [reason, flaggedBy, messageId]
    );
  }

  // =====================================================
  // PRIVATE MESSAGING
  // =====================================================

  async sendPrivateMessage(
    senderId: number,
    request: SendPrivateMessageRequest
  ): Promise<PrivateMessage> {
    const { receiverId, message } = request;

    if (senderId === receiverId) {
      throw new Error('Cannot send message to yourself');
    }

    // Get or create conversation
    const conversation = await this.getOrCreateConversation(senderId, receiverId);

    // Check if blocked
    const isBlocked = await this.isUserBlocked(senderId, receiverId);
    if (isBlocked) {
      throw new Error('Cannot send message to this user');
    }

    // Insert message
    const result = await pool.query(
      `INSERT INTO private_messages 
       (conversation_id, sender_id, message)
       VALUES ($1, $2, $3)
       RETURNING *`,
      [conversation.id, senderId, message]
    );

    const privateMessage = result.rows[0];
    if (!privateMessage) {
      throw new Error('Failed to create private message');
    }

    // Get sender info
    const userInfo = await pool.query(
      `SELECT username FROM users WHERE id = $1`,
      [senderId]
    );

    return {
      ...privateMessage,
      sender_username: userInfo.rows[0]?.username,
    };
  }

  private async getOrCreateConversation(
    user1Id: number,
    user2Id: number
  ): Promise<PrivateConversation> {
    // Ensure user1_id < user2_id for consistency
    const [smallerId, largerId] = user1Id < user2Id ? [user1Id, user2Id] : [user2Id, user1Id];

    // Try to get existing conversation
    let result = await pool.query(
      `SELECT * FROM private_conversations 
       WHERE user1_id = $1 AND user2_id = $2`,
      [smallerId, largerId]
    );

    if (result.rows.length > 0) {
      return result.rows[0];
    }

    // Create new conversation
    result = await pool.query(
      `INSERT INTO private_conversations (user1_id, user2_id)
       VALUES ($1, $2)
       RETURNING *`,
      [smallerId, largerId]
    );

    if (result.rows.length === 0) {
      throw new Error('Failed to create conversation');
    }
    return result.rows[0];
  }

  async getPrivateConversations(
    userId: number,
    limit: number = 20,
    offset: number = 0
  ): Promise<PrivateConversationsResponse> {
    const result = await pool.query(
      `SELECT 
         pc.*,
         CASE 
           WHEN pc.user1_id = $1 THEN u2.id
           ELSE u1.id
         END as other_user_id,
         CASE 
           WHEN pc.user1_id = $1 THEN u2.username
           ELSE u1.username
         END as other_username,
         CASE 
           WHEN pc.user1_id = $1 THEN pc.user1_unread_count
           ELSE pc.user2_unread_count
         END as unread_count,
         (SELECT message FROM private_messages 
          WHERE conversation_id = pc.id 
          ORDER BY created_at DESC LIMIT 1) as last_message
       FROM private_conversations pc
       JOIN users u1 ON pc.user1_id = u1.id
       JOIN users u2 ON pc.user2_id = u2.id
       WHERE pc.user1_id = $1 OR pc.user2_id = $1
       ORDER BY pc.last_message_at DESC
       LIMIT $2 OFFSET $3`,
      [userId, limit, offset]
    );

    const countResult = await pool.query(
      `SELECT COUNT(*) FROM private_conversations 
       WHERE user1_id = $1 OR user2_id = $1`,
      [userId]
    );

    return {
      conversations: result.rows,
      total: parseInt(countResult.rows[0]?.count || '0'),
    };
  }

  async getPrivateMessages(
    userId: number,
    conversationId: number,
    limit: number = 50,
    before?: Date
  ): Promise<PrivateMessagesResponse> {
    // Verify user is part of conversation
    const convResult = await pool.query(
      `SELECT * FROM private_conversations 
       WHERE id = $1 AND (user1_id = $2 OR user2_id = $2)`,
      [conversationId, userId]
    );

    if (convResult.rows.length === 0) {
      throw new Error('Conversation not found or access denied');
    }

    let query = `
      SELECT 
        pm.*,
        u.username as sender_username
      FROM private_messages pm
      JOIN users u ON pm.sender_id = u.id
      WHERE pm.conversation_id = $1
    `;
    const params: any[] = [conversationId];

    // Check deletion status based on user
    const conversation = convResult.rows[0];
    if (!conversation) {
      throw new Error('Conversation not found or access denied');
    }
    if (conversation.user1_id === userId) {
      query += ` AND pm.is_deleted_by_receiver = FALSE`;
    } else {
      query += ` AND pm.is_deleted_by_sender = FALSE`;
    }

    if (before) {
      query += ` AND pm.created_at < $${params.length + 1}`;
      params.push(before);
    }

    query += ` ORDER BY pm.created_at DESC LIMIT $${params.length + 1}`;
    params.push(limit + 1);

    const result = await pool.query(query, params);
    const messages = result.rows;
    const hasMore = messages.length > limit;

    if (hasMore) {
      messages.pop();
    }

    // Mark messages as read
    await this.markMessagesAsRead(conversationId, userId);

    // Get total count
    const countResult = await pool.query(
      `SELECT COUNT(*) FROM private_messages WHERE conversation_id = $1`,
      [conversationId]
    );

    return {
      messages: messages.reverse(),
      hasMore,
      total: parseInt(countResult.rows[0]?.count || '0'),
    };
  }

  async markMessagesAsRead(conversationId: number, userId: number): Promise<void> {
    await pool.query(
      `UPDATE private_messages 
       SET is_read = TRUE, read_at = CURRENT_TIMESTAMP
       WHERE conversation_id = $1 AND sender_id != $2 AND is_read = FALSE`,
      [conversationId, userId]
    );

    // Reset unread count in conversation
    const conversation = await pool.query(
      `SELECT user1_id, user2_id FROM private_conversations WHERE id = $1`,
      [conversationId]
    );

    if (conversation.rows.length > 0) {
      const conv = conversation.rows[0];
      if (conv && conv.user1_id === userId) {
        await pool.query(
          `UPDATE private_conversations SET user1_unread_count = 0 WHERE id = $1`,
          [conversationId]
        );
      } else {
        await pool.query(
          `UPDATE private_conversations SET user2_unread_count = 0 WHERE id = $1`,
          [conversationId]
        );
      }
    }
  }

  // =====================================================
  // RESTRICTIONS & MODERATION
  // =====================================================

  async restrictUser(
    userId: number,
    channelId: number | null,
    restrictionType: ChatRestrictionType,
    reason: string,
    restrictedBy: number,
    durationMinutes?: number
  ): Promise<void> {
    const expiresAt = durationMinutes
      ? new Date(Date.now() + durationMinutes * 60 * 1000)
      : null;

    await pool.query(
      `INSERT INTO chat_restrictions 
       (user_id, channel_id, restriction_type, reason, restricted_by, expires_at)
       VALUES ($1, $2, $3, $4, $5, $6)
       ON CONFLICT (user_id, channel_id, restriction_type) 
       DO UPDATE SET 
         reason = EXCLUDED.reason,
         expires_at = EXCLUDED.expires_at,
         created_at = CURRENT_TIMESTAMP`,
      [userId, channelId, restrictionType, reason, restrictedBy, expiresAt]
    );
  }

  async removeRestriction(
    userId: number,
    channelId: number | null,
    restrictionType: ChatRestrictionType
  ): Promise<void> {
    await pool.query(
      `DELETE FROM chat_restrictions 
       WHERE user_id = $1 AND channel_id = $2 AND restriction_type = $3`,
      [userId, channelId, restrictionType]
    );
  }

  async isUserRestricted(userId: number, channelId: number): Promise<boolean> {
    const result = await pool.query(
      `SELECT 1 FROM chat_restrictions 
       WHERE user_id = $1 
         AND (channel_id = $2 OR channel_id IS NULL)
         AND restriction_type IN ('mute', 'ban')
         AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP)`,
      [userId, channelId]
    );

    return result.rows.length > 0;
  }

  async isUserBlocked(userId: number, otherUserId: number): Promise<boolean> {
    return playerBlockService.isBlockedEither(userId, otherUserId, 'messages');
  }

  async isShadowBanned(userId: number, channelId: number | null): Promise<boolean> {
    const result = await pool.query(
      `SELECT 1 FROM chat_restrictions 
       WHERE user_id = $1
         AND restriction_type = 'shadow'
         AND (channel_id IS NULL OR channel_id = $2)
         AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP)`,
      [userId, channelId]
    );
    return result.rows.length > 0;
  }

  // =====================================================
  // RATE LIMITING
  // =====================================================

  async checkRateLimit(userId: number, channelId: number): Promise<boolean> {
    const channel = await this.getChannelById(channelId);
    if (!channel) return false;

    const key = `chat:ratelimit:${userId}:${channelId}`;
    const lastMessageTime = await redis.get(key);

    if (!lastMessageTime) {
      return true; // No previous message
    }

    const timeSinceLastMessage = Date.now() - parseInt(lastMessageTime);
    return timeSinceLastMessage >= channel.rate_limit_seconds * 1000;
  }

  private async updateRateLimit(userId: number, channelId: number): Promise<void> {
    const key = `chat:ratelimit:${userId}:${channelId}`;
    await redis.setex(key, 60, Date.now().toString()); // Cache for 60 seconds
  }

  // =====================================================
  // ANALYTICS
  // =====================================================

  async getChatActivityStats(): Promise<ChatActivityStats[]> {
    const result = await pool.query(`SELECT * FROM v_chat_activity`);
    return result.rows;
  }

  async getUserMessageCount(userId: number, since?: Date): Promise<number> {
    let query = `SELECT COUNT(*) FROM chat_messages WHERE user_id = $1 AND is_deleted = FALSE`;
    const params: any[] = [userId];

    if (since) {
      query += ` AND created_at >= $2`;
      params.push(since);
    }

    const result = await pool.query(query, params);
    return parseInt(result.rows[0]?.count || '0');
  }

  // =====================================================
  // CLEANUP
  // =====================================================

  async cleanupOldMessages(daysToKeep: number = 30): Promise<number> {
    const result = await pool.query(
      `DELETE FROM chat_messages 
       WHERE created_at < CURRENT_TIMESTAMP - ($1 || ' days')::INTERVAL`,
      [daysToKeep]
    );

    return result.rowCount || 0;
  }

  async autoExpireRestrictions(): Promise<number> {
    const result = await pool.query(
      `DELETE FROM chat_restrictions 
       WHERE expires_at IS NOT NULL AND expires_at < CURRENT_TIMESTAMP`
    );

    return result.rowCount || 0;
  }
}

export default new ChatService();
