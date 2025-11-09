/**
 * @module backend/services/chatService
 *
 * PHASE 6: CHAT SERVICE
 * Comprehensive chat system for global, alliance, sector, and private messaging.
 * Responsible for channel management, message persistence, moderation tools,
 * rate limiting, and history retrieval. Integrates with Redis for rate
 * limiting and shadow-banning checks.
 */

import { pool } from '../config/database';
import redis from '../config/redis';
import playerBlockService from './playerBlockService';
import {
  ChatChannel,
  ChatMessage,
  ChatMessageType,
  ChatReactionType,
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

const ALLOWED_REACTIONS: ChatReactionType[] = [
  ChatReactionType.THUMBS_UP,
  ChatReactionType.THUMBS_DOWN,
  ChatReactionType.ROFL,
  ChatReactionType.CLAP,
  ChatReactionType.ANGRY,
  ChatReactionType.CRY,
];

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
    const {
      channelId,
      message,
      messageType = ChatMessageType.TEXT,
      isAnnouncement = false,
      announcementExpiresAt,
      pinMessage = false,
    } = request;

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

    if ((isAnnouncement || pinMessage) && !(await this.isUserModeratorOrAdmin(userId))) {
      throw new Error('Admin privileges are required for announcements or pinned messages.');
    }

    if (isAnnouncement && channel.channel_type !== 'global') {
      throw new Error('Announcements are restricted to the world chat.');
    }

    // Insert message
    const result = await pool.query(
      `INSERT INTO chat_messages 
       (channel_id, user_id, message, message_type, is_announcement, announcement_expires_at, is_pinned, pinned_by, pinned_at)
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
       RETURNING *`,
      [
        channelId,
        userId,
        message,
        messageType,
        isAnnouncement,
        announcementExpiresAt ? new Date(announcementExpiresAt) : null,
        pinMessage,
        pinMessage ? userId : null,
        pinMessage ? new Date() : null,
      ]
    );

    const chatMessage = result.rows[0];
    if (!chatMessage) {
      throw new Error('Failed to create chat message');
    }

    // Update rate limit in Redis
    await this.updateRateLimit(userId, channelId);

    const hydrated = await this.getMessageById(chatMessage.id, userId);
    if (!hydrated) {
      throw new Error('Failed to load chat message');
    }
    return hydrated;
  }

  async getChatHistory(request: GetChatHistoryRequest): Promise<ChatHistoryResponse> {
    const { channelId, limit = 50, before, viewerUserId } = request;

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

    const chronological = messages.reverse().map((row) => this.normalizeChatRow(row));
    const pinnedMessages = await this.getPinnedMessages(channelId);
    const announcements = await this.getActiveAnnouncements(channelId);

    await this.attachReactionSummaries(
      [...chronological, ...pinnedMessages, ...announcements],
      viewerUserId
    );

    // Get total count
    const countResult = await pool.query(
      `SELECT COUNT(*) FROM chat_messages WHERE channel_id = $1 AND is_deleted = FALSE`,
      [channelId]
    );
    const total = parseInt(countResult.rows[0]?.count || '0');

    return {
      messages: chronological,
      pinnedMessages,
      announcements,
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

  async pinMessage(
    messageId: number,
    userId: number,
    shouldPin: boolean
  ): Promise<ChatMessage> {
    if (!(await this.isUserModeratorOrAdmin(userId))) {
      throw new Error('Admin access required to pin messages.');
    }

    const result = await pool.query(
      `UPDATE chat_messages
       SET is_pinned = $1,
           pinned_by = CASE WHEN $1 THEN $2 ELSE NULL END,
           pinned_at = CASE WHEN $1 THEN CURRENT_TIMESTAMP ELSE NULL END
       WHERE id = $3 AND is_deleted = FALSE
       RETURNING *`,
      [shouldPin, userId, messageId]
    );

    if (!result.rows.length) {
      throw new Error('Message not found');
    }

    const hydrated = await this.getMessageById(messageId, userId);
    if (!hydrated) {
      throw new Error('Failed to load pinned message');
    }
    return hydrated;
  }

  async markAnnouncement(
    messageId: number,
    userId: number,
    isAnnouncement: boolean,
    expiresAt?: Date | null
  ): Promise<ChatMessage> {
    if (!(await this.isUserModeratorOrAdmin(userId))) {
      throw new Error('Admin access required to manage announcements.');
    }

    const messageRow = await pool.query(
      `SELECT cm.channel_id, cc.channel_type
       FROM chat_messages cm
       JOIN chat_channels cc ON cc.id = cm.channel_id
       WHERE cm.id = $1 AND cm.is_deleted = FALSE`,
      [messageId]
    );

    if (!messageRow.rows.length) {
      throw new Error('Message not found');
    }

    if (isAnnouncement && messageRow.rows[0].channel_type !== 'global') {
      throw new Error('Announcements are limited to the world chat.');
    }

    await pool.query(
      `UPDATE chat_messages
       SET is_announcement = $1,
           announcement_expires_at = $2
       WHERE id = $3`,
      [isAnnouncement, expiresAt ? new Date(expiresAt) : null, messageId]
    );

    const hydrated = await this.getMessageById(messageId, userId);
    if (!hydrated) {
      throw new Error('Failed to load announcement state');
    }
    return hydrated;
  }

  async toggleReaction(
    messageId: number,
    userId: number,
    reaction: ChatReactionType
  ): Promise<{
    messageId: number;
    channelId: number;
    reactions: Partial<Record<ChatReactionType, number>>;
    viewerReactions: ChatReactionType[];
  }> {
    if (!ALLOWED_REACTIONS.includes(reaction)) {
      throw new Error('Unsupported reaction type.');
    }

    const messageRow = await pool.query(
      `SELECT channel_id FROM chat_messages WHERE id = $1 AND is_deleted = FALSE`,
      [messageId]
    );

    if (!messageRow.rows.length) {
      throw new Error('Message not found');
    }

    const existing = await pool.query(
      `SELECT id FROM chat_message_reactions 
       WHERE message_id = $1 AND user_id = $2 AND reaction_type = $3`,
      [messageId, userId, reaction]
    );

    if (existing.rows.length) {
      await pool.query(
        `DELETE FROM chat_message_reactions WHERE id = $1`,
        [existing.rows[0].id]
      );
    } else {
      await pool.query(
        `INSERT INTO chat_message_reactions (message_id, user_id, reaction_type)
         VALUES ($1, $2, $3)
         ON CONFLICT (message_id, user_id, reaction_type) DO NOTHING`,
        [messageId, userId, reaction]
      );
    }

    const reactions = await this.getReactionSummary(messageId);
    const viewerReactions = await this.getViewerReactionsForMessage(messageId, userId);

    return {
      messageId,
      channelId: messageRow.rows[0].channel_id,
      reactions,
      viewerReactions,
    };
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

  private normalizeChatRow(row: any): ChatMessage {
    return {
      ...row,
      is_announcement: Boolean(row.is_announcement),
      is_pinned: Boolean(row.is_pinned),
      reactions: row.reactions ?? {},
      viewerReactions: row.viewerReactions ?? [],
    };
  }

  private async getPinnedMessages(channelId: number, limit: number = 5): Promise<ChatMessage[]> {
    const result = await pool.query(
      `SELECT 
         cm.*,
         u.username,
         a.tag as alliance_tag
       FROM chat_messages cm
       JOIN users u ON cm.user_id = u.id
       LEFT JOIN alliances a ON u.alliance_id = a.id
       WHERE cm.channel_id = $1
         AND cm.is_deleted = FALSE
         AND cm.is_pinned = TRUE
       ORDER BY cm.pinned_at DESC NULLS LAST, cm.created_at DESC
       LIMIT $2`,
      [channelId, limit]
    );
    return result.rows.map((row) => this.normalizeChatRow(row));
  }

  private async getActiveAnnouncements(channelId: number, limit: number = 3): Promise<ChatMessage[]> {
    const result = await pool.query(
      `SELECT 
         cm.*,
         u.username,
         a.tag as alliance_tag
       FROM chat_messages cm
       JOIN users u ON cm.user_id = u.id
       LEFT JOIN alliances a ON u.alliance_id = a.id
       WHERE cm.channel_id = $1
         AND cm.is_deleted = FALSE
         AND cm.is_announcement = TRUE
         AND (cm.announcement_expires_at IS NULL OR cm.announcement_expires_at > CURRENT_TIMESTAMP)
       ORDER BY cm.created_at DESC
       LIMIT $2`,
      [channelId, limit]
    );
    return result.rows.map((row) => this.normalizeChatRow(row));
  }

  private async attachReactionSummaries(
    messages: ChatMessage[],
    viewerUserId?: number
  ): Promise<void> {
    if (!messages.length) return;
    const ids = Array.from(new Set(messages.map((msg) => msg.id))).filter(Boolean);
    if (!ids.length) return;

    const reactionResult = await pool.query(
      `SELECT message_id, reaction_type, COUNT(*) as count
       FROM chat_message_reactions
       WHERE message_id = ANY($1::int[])
       GROUP BY message_id, reaction_type`,
      [ids]
    );

    const reactionMap = new Map<number, Partial<Record<ChatReactionType, number>>>();
    reactionResult.rows.forEach((row) => {
      const existing = reactionMap.get(row.message_id) || {};
      existing[row.reaction_type as ChatReactionType] = Number(row.count);
      reactionMap.set(row.message_id, existing);
    });

    const viewerMap = new Map<number, ChatReactionType[]>();
    if (viewerUserId) {
      const viewerResult = await pool.query(
        `SELECT message_id, reaction_type
         FROM chat_message_reactions
         WHERE message_id = ANY($1::int[]) AND user_id = $2`,
        [ids, viewerUserId]
      );
      viewerResult.rows.forEach((row) => {
        const entries = viewerMap.get(row.message_id) || [];
        entries.push(row.reaction_type as ChatReactionType);
        viewerMap.set(row.message_id, entries);
      });
    }

    messages.forEach((message) => {
      message.reactions = reactionMap.get(message.id) || {};
      message.viewerReactions = viewerMap.get(message.id) || [];
    });
  }

  private async getReactionSummary(
    messageId: number
  ): Promise<Partial<Record<ChatReactionType, number>>> {
    const result = await pool.query(
      `SELECT reaction_type, COUNT(*) as count
       FROM chat_message_reactions
       WHERE message_id = $1
       GROUP BY reaction_type`,
      [messageId]
    );

    const summary: Partial<Record<ChatReactionType, number>> = {};
    result.rows.forEach((row) => {
      summary[row.reaction_type as ChatReactionType] = Number(row.count);
    });
    return summary;
  }

  private async getViewerReactionsForMessage(
    messageId: number,
    userId: number
  ): Promise<ChatReactionType[]> {
    const result = await pool.query(
      `SELECT reaction_type
       FROM chat_message_reactions
       WHERE message_id = $1 AND user_id = $2`,
      [messageId, userId]
    );
    return result.rows.map((row) => row.reaction_type as ChatReactionType);
  }

  private async isUserModeratorOrAdmin(userId: number): Promise<boolean> {
    const result = await pool.query(
      `SELECT 
         u.is_admin,
         au.admin_level
       FROM users u
       LEFT JOIN admin_users au ON au.user_id = u.id AND au.is_active = TRUE
       WHERE u.id = $1`,
      [userId]
    );
    if (!result.rows.length) return false;
    if (result.rows[0].is_admin) return true;
    const level = result.rows[0].admin_level;
    return ['super_admin', 'game_admin', 'moderator'].includes(level);
  }

  private async getMessageById(
    messageId: number,
    viewerUserId?: number
  ): Promise<ChatMessage | null> {
    const result = await pool.query(
      `SELECT 
         cm.*,
         u.username,
         a.tag as alliance_tag
       FROM chat_messages cm
       JOIN users u ON cm.user_id = u.id
       LEFT JOIN alliances a ON u.alliance_id = a.id
       WHERE cm.id = $1`,
      [messageId]
    );

    if (!result.rows.length) {
      return null;
    }

    const message = this.normalizeChatRow(result.rows[0]);
    await this.attachReactionSummaries([message], viewerUserId);
    return message;
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
