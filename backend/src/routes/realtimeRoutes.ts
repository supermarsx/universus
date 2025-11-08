/**
 * PHASE 6: REALTIME COMMUNICATION ROUTES
 * REST API endpoints for chat, notifications, player status, and trading
 */

import express from 'express';
import { authenticateToken } from '../middleware/auth';
import chatService from '../services/chatService';
import notificationService from '../services/notificationService';
import { pool } from '../config/database';
import { getRealtimeHandler } from '../socket';
import {
  ChatMessageType,
  ChatRestrictionType,
  ChatReactionType,
  NotificationCategory,
  PlayerStatus,
  TradeOfferType,
  TradeOfferStatus,
  ResourceType,
} from '../types/realtime';

const router = express.Router();

// All routes require authentication
router.use(authenticateToken);

const resolveAuthUser = (req: any) => {
  const user = req.user || {};
  const adminLevel = user.admin_level ?? user.adminLevel ?? null;
  return {
    id: user.id ?? user.userId,
    isAdmin: user.is_admin ?? user.isAdmin ?? false,
    isModerator:
      user.is_moderator ??
      user.isModerator ??
      (adminLevel ? ['moderator', 'game_admin', 'super_admin'].includes(adminLevel) : false),
  };
};

// =====================================================
// CHAT ROUTES
// =====================================================

// Get all chat channels
router.get('/chat/channels', async (req, res) => {
  try {
    const channels = await chatService.getAllChannels();
    res.json({ channels });
  } catch (error: any) {
    res.status(500).json({ error: error.message });
  }
});

// Get chat history for a channel
router.get('/chat/channels/:channelId/messages', async (req, res) => {
  try {
    const { id: userId } = resolveAuthUser(req);
    const channelId = parseInt(req.params.channelId);
    const limit = parseInt(req.query.limit as string) || 50;
    const before = req.query.before ? new Date(req.query.before as string) : undefined;

    const history = await chatService.getChatHistory({
      channelId,
      limit,
      before,
      viewerUserId: userId,
    });
    res.json(history);
  } catch (error: any) {
    res.status(500).json({ error: error.message });
  }
});

// Send chat message (REST endpoint - WebSocket is preferred)
router.post('/chat/channels/:channelId/messages', async (req, res) => {
  try {
    const { id: userId } = resolveAuthUser(req);
    const channelId = parseInt(req.params.channelId);
    const { message, messageType, isAnnouncement, announcementExpiresAt, pinMessage } = req.body;

    const chatMessage = await chatService.sendMessage(userId, {
      channelId,
      message,
      messageType: messageType || ChatMessageType.TEXT,
      isAnnouncement,
      announcementExpiresAt: announcementExpiresAt ? new Date(announcementExpiresAt) : undefined,
      pinMessage,
    });

    res.json({ message: chatMessage });
  } catch (error: any) {
    res.status(400).json({ error: error.message });
  }
});

// Edit chat message
router.put('/chat/messages/:messageId', async (req, res) => {
  try {
    const { id: userId } = resolveAuthUser(req);
    const messageId = parseInt(req.params.messageId);
    const { message } = req.body;

    await chatService.editMessage(messageId, userId, message);
    res.json({ success: true });
  } catch (error: any) {
    res.status(400).json({ error: error.message });
  }
});

// Delete chat message
router.delete('/chat/messages/:messageId', async (req, res) => {
  try {
    const { id: userId, isAdmin } = resolveAuthUser(req);
    const messageId = parseInt(req.params.messageId);

    await chatService.deleteMessage(messageId, userId, isAdmin);
    res.json({ success: true });
  } catch (error: any) {
    res.status(400).json({ error: error.message });
  }
});

// Flag message (report)
router.post('/chat/messages/:messageId/flag', async (req, res) => {
  try {
    const { id: userId } = resolveAuthUser(req);
    const messageId = parseInt(req.params.messageId);
    const { reason } = req.body;

    await chatService.flagMessage(messageId, userId, reason);
    res.json({ success: true });
  } catch (error: any) {
    res.status(400).json({ error: error.message });
  }
});

router.post('/chat/messages/:messageId/pin', async (req, res) => {
  try {
    const { id: userId, isAdmin, isModerator } = resolveAuthUser(req);
    if (!(isAdmin || isModerator)) {
      return res.status(403).json({ error: 'Admin access required' });
    }
    const messageId = parseInt(req.params.messageId);
    const shouldPin = req.body?.pinned !== false;

    const message = await chatService.pinMessage(messageId, userId, shouldPin);
    const handler = getRealtimeHandler();
    handler?.broadcastChatPinUpdate(message.channel_id, message);

    res.json({ message });
  } catch (error: any) {
    res.status(400).json({ error: error.message });
  }
});

router.post('/chat/messages/:messageId/announcement', async (req, res) => {
  try {
    const { id: userId, isAdmin, isModerator } = resolveAuthUser(req);
    if (!(isAdmin || isModerator)) {
      return res.status(403).json({ error: 'Admin access required' });
    }
    const messageId = parseInt(req.params.messageId);
    const { isAnnouncement, expiresAt } = req.body || {};

    const message = await chatService.markAnnouncement(
      messageId,
      userId,
      Boolean(isAnnouncement),
      expiresAt ? new Date(expiresAt) : null
    );
    const handler = getRealtimeHandler();
    handler?.broadcastChatAnnouncementUpdate(message.channel_id, message);

    res.json({ message });
  } catch (error: any) {
    res.status(400).json({ error: error.message });
  }
});

router.post('/chat/messages/:messageId/reactions', async (req, res) => {
  try {
    const { id: userId } = resolveAuthUser(req);
    const messageId = parseInt(req.params.messageId);
    const { reactionType } = req.body;
    const result = await chatService.toggleReaction(
      messageId,
      userId,
      reactionType as ChatReactionType
    );

    const handler = getRealtimeHandler();
    handler?.broadcastChatReactionUpdate(result.channelId, result.messageId, result.reactions);

    res.json(result);
  } catch (error: any) {
    res.status(400).json({ error: error.message });
  }
});

// Get chat activity stats (admin only)
router.get('/chat/stats', async (req, res) => {
  try {
    const { isAdmin } = resolveAuthUser(req);
    if (!isAdmin) {
      return res.status(403).json({ error: 'Admin access required' });
    }

    const stats = await chatService.getChatActivityStats();
    res.json({ stats });
  } catch (error: any) {
    res.status(500).json({ error: error.message });
  }
});

// =====================================================
// PRIVATE MESSAGE ROUTES
// =====================================================

// Get user's private conversations
router.get('/chat/conversations', async (req, res) => {
  try {
    const { id: userId } = resolveAuthUser(req);
    const limit = parseInt(req.query.limit as string) || 20;
    const offset = parseInt(req.query.offset as string) || 0;

    const result = await chatService.getPrivateConversations(userId, limit, offset);
    res.json(result);
  } catch (error: any) {
    res.status(500).json({ error: error.message });
  }
});

// Get messages from a conversation
router.get('/chat/conversations/:conversationId/messages', async (req, res) => {
  try {
    const { id: userId } = resolveAuthUser(req);
    const conversationId = parseInt(req.params.conversationId);
    const limit = parseInt(req.query.limit as string) || 50;
    const before = req.query.before ? new Date(req.query.before as string) : undefined;

    const result = await chatService.getPrivateMessages(userId, conversationId, limit, before);
    res.json(result);
  } catch (error: any) {
    res.status(500).json({ error: error.message });
  }
});

// Send private message (REST endpoint - WebSocket is preferred)
router.post('/chat/private', async (req, res) => {
  try {
    const { id: userId } = resolveAuthUser(req);
    const { receiverId, message } = req.body;

    const privateMessage = await chatService.sendPrivateMessage(userId, {
      receiverId: parseInt(receiverId),
      message,
    });

    res.json({ message: privateMessage });
  } catch (error: any) {
    res.status(400).json({ error: error.message });
  }
});

// =====================================================
// NOTIFICATION ROUTES
// =====================================================

// Get user notifications
router.get('/notifications', async (req, res) => {
  try {
    const { id: userId } = resolveAuthUser(req);
    const unreadOnly = req.query.unreadOnly === 'true';
    const category = req.query.category as NotificationCategory | undefined;
    const limit = parseInt(req.query.limit as string) || 50;
    const offset = parseInt(req.query.offset as string) || 0;

    const result = await notificationService.getUserNotifications({
      userId,
      unreadOnly,
      category,
      limit,
      offset,
    });

    res.json(result);
  } catch (error: any) {
    res.status(500).json({ error: error.message });
  }
});

// Get notification by ID
router.get('/notifications/:notificationId', async (req, res) => {
  try {
    const { id: userId } = resolveAuthUser(req);
    const notificationId = parseInt(req.params.notificationId);

    const notification = await notificationService.getNotificationById(notificationId, userId);
    if (!notification) {
      return res.status(404).json({ error: 'Notification not found' });
    }

    res.json({ notification });
  } catch (error: any) {
    res.status(500).json({ error: error.message });
  }
});

// Mark notification as read
router.put('/notifications/:notificationId/read', async (req, res) => {
  try {
    const { id: userId } = resolveAuthUser(req);
    const notificationId = parseInt(req.params.notificationId);

    await notificationService.markAsRead(notificationId, userId);
    res.json({ success: true });
  } catch (error: any) {
    res.status(400).json({ error: error.message });
  }
});

// Mark all notifications as read
router.put('/notifications/read/all', async (req, res) => {
  try {
    const { id: userId } = resolveAuthUser(req);
    const count = await notificationService.markAllAsRead(userId);
    res.json({ success: true, count });
  } catch (error: any) {
    res.status(400).json({ error: error.message });
  }
});

// Archive notification
router.put('/notifications/:notificationId/archive', async (req, res) => {
  try {
    const { id: userId } = resolveAuthUser(req);
    const notificationId = parseInt(req.params.notificationId);

    await notificationService.archiveNotification(notificationId, userId);
    res.json({ success: true });
  } catch (error: any) {
    res.status(400).json({ error: error.message });
  }
});

// Delete notification
router.delete('/notifications/:notificationId', async (req, res) => {
  try {
    const { id: userId } = resolveAuthUser(req);
    const notificationId = parseInt(req.params.notificationId);

    await notificationService.deleteNotification(notificationId, userId);
    res.json({ success: true });
  } catch (error: any) {
    res.status(400).json({ error: error.message });
  }
});

// Get unread count
router.get('/notifications/unread/count', async (req, res) => {
  try {
    const { id: userId } = resolveAuthUser(req);
    const count = await notificationService.getUnreadCount(userId);
    res.json({ count });
  } catch (error: any) {
    res.status(500).json({ error: error.message });
  }
});

// Get notification preferences
router.get('/notifications/preferences', async (req, res) => {
  try {
    const { id: userId } = resolveAuthUser(req);
    const preferences = await notificationService.getUserPreferences(userId);
    res.json({ preferences });
  } catch (error: any) {
    res.status(500).json({ error: error.message });
  }
});

// Update notification preference
router.put('/notifications/preferences/:typeId', async (req, res) => {
  try {
    const { id: userId } = resolveAuthUser(req);
    const typeId = parseInt(req.params.typeId);
    const updates = req.body;

    await notificationService.updatePreference(userId, typeId, updates);
    res.json({ success: true });
  } catch (error: any) {
    res.status(400).json({ error: error.message });
  }
});

// Get notification types
router.get('/notifications/types/all', async (req, res) => {
  try {
    const types = await notificationService.getAllNotificationTypes();
    res.json({ types });
  } catch (error: any) {
    res.status(500).json({ error: error.message });
  }
});

// =====================================================
// PLAYER STATUS ROUTES
// =====================================================

// Get online players
router.get('/players/online', async (req, res) => {
  try {
    const limit = parseInt(req.query.limit as string) || 100;
    const allianceId = req.query.allianceId ? parseInt(req.query.allianceId as string) : undefined;

    let query = `
      SELECT 
        ps.user_id,
        u.username,
        ps.status,
        ps.status_message,
        ps.last_activity,
        a.tag as alliance_tag
      FROM player_status ps
      JOIN users u ON ps.user_id = u.id
      LEFT JOIN alliances a ON u.alliance_id = a.id
      WHERE ps.status = 'online' AND ps.last_activity > CURRENT_TIMESTAMP - INTERVAL '5 minutes'
    `;
    const params: any[] = [];

    if (allianceId) {
      query += ` AND u.alliance_id = $1`;
      params.push(allianceId);
    }

    query += ` ORDER BY ps.last_activity DESC LIMIT $${params.length + 1}`;
    params.push(limit);

    const result = await pool.query(query, params);
    res.json({ players: result.rows, count: result.rows.length });
  } catch (error: any) {
    res.status(500).json({ error: error.message });
  }
});

// Get player status
router.get('/players/:userId/status', async (req, res) => {
  try {
    const userId = parseInt(req.params.userId);

    const result = await pool.query(
      `SELECT * FROM player_status WHERE user_id = $1`,
      [userId]
    );

    if (result.rows.length === 0) {
      return res.json({ status: 'offline' });
    }

    res.json({ status: result.rows[0] });
  } catch (error: any) {
    res.status(500).json({ error: error.message });
  }
});

// =====================================================
// TRADE ROUTES
// =====================================================

// Get active trade offers
router.get('/trade/offers', async (req, res) => {
  try {
    const status = (req.query.status as TradeOfferStatus) || TradeOfferStatus.ACTIVE;
    const resourceOffered = req.query.resourceOffered as ResourceType | undefined;
    const resourceWanted = req.query.resourceWanted as ResourceType | undefined;
    const limit = parseInt(req.query.limit as string) || 50;
    const offset = parseInt(req.query.offset as string) || 0;

    let query = `
      SELECT 
        t.*,
        u.username as seller_username,
        a.tag as seller_alliance_tag,
        EXTRACT(EPOCH FROM (t.expires_at - CURRENT_TIMESTAMP)) as seconds_until_expiry
      FROM trade_offers t
      JOIN users u ON t.seller_id = u.id
      LEFT JOIN alliances a ON u.alliance_id = a.id
      WHERE t.status = $1
    `;
    const params: any[] = [status];
    let paramIndex = 2;

    if (resourceOffered) {
      query += ` AND t.resource_offered = $${paramIndex++}`;
      params.push(resourceOffered);
    }

    if (resourceWanted) {
      query += ` AND t.resource_wanted = $${paramIndex++}`;
      params.push(resourceWanted);
    }

    if (status === TradeOfferStatus.ACTIVE) {
      query += ` AND t.expires_at > CURRENT_TIMESTAMP`;
    }

    query += ` ORDER BY t.created_at DESC LIMIT $${paramIndex++} OFFSET $${paramIndex++}`;
    params.push(limit, offset);

    const result = await pool.query(query, params);

    const countResult = await pool.query(
      `SELECT COUNT(*) FROM trade_offers WHERE status = $1`,
      [status]
    );

    res.json({
      offers: result.rows,
      total: parseInt(countResult.rows[0].count),
    });
  } catch (error: any) {
    res.status(500).json({ error: error.message });
  }
});

// Create trade offer
router.post('/trade/offers', async (req, res) => {
  try {
    const { id: userId } = resolveAuthUser(req);
    const {
      offerType,
      resourceOffered,
      amountOffered,
      resourceWanted,
      amountWanted,
      minReputation,
      allianceOnly,
      targetAllianceId,
      expiresInHours,
    } = req.body;

    // Validate resources
    if (amountOffered <= 0 || amountWanted <= 0) {
      return res.status(400).json({ error: 'Invalid amounts' });
    }

    // Calculate exchange rate
    const exchangeRate = amountWanted / amountOffered;

    // Calculate expiration
    const expiresAt = new Date(Date.now() + (expiresInHours || 168) * 60 * 60 * 1000);

    const result = await pool.query(
      `INSERT INTO trade_offers 
       (seller_id, offer_type, resource_offered, amount_offered, resource_wanted, 
        amount_wanted, exchange_rate, min_reputation, alliance_only, target_alliance_id, expires_at)
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
       RETURNING *`,
      [
        userId,
        offerType || TradeOfferType.SELL,
        resourceOffered,
        amountOffered,
        resourceWanted,
        amountWanted,
        exchangeRate,
        minReputation || 0,
        allianceOnly || false,
        targetAllianceId || null,
        expiresAt,
      ]
    );

    res.json({ offer: result.rows[0] });
  } catch (error: any) {
    res.status(400).json({ error: error.message });
  }
});

// Accept trade offer
router.post('/trade/offers/:offerId/accept', async (req, res) => {
  try {
    const { id: userId } = resolveAuthUser(req);
    const offerId = parseInt(req.params.offerId);

    // Get trade offer
    const offerResult = await pool.query(
      `SELECT * FROM trade_offers WHERE id = $1 AND status = 'active'`,
      [offerId]
    );

    if (offerResult.rows.length === 0) {
      return res.status(404).json({ error: 'Trade offer not found or expired' });
    }

    const offer = offerResult.rows[0];

    if (offer.seller_id === userId) {
      return res.status(400).json({ error: 'Cannot accept your own trade offer' });
    }

    // TODO: Verify buyer has enough resources
    // TODO: Deduct resources from buyer, add to seller
    // TODO: Add resources to buyer according to the offer

    // Mark offer as completed
    await pool.query(
      `UPDATE trade_offers 
       SET status = 'completed', buyer_id = $1, completed_at = CURRENT_TIMESTAMP
       WHERE id = $2`,
      [userId, offerId]
    );

    // Create transaction record
    await pool.query(
      `INSERT INTO trade_transactions 
       (trade_offer_id, seller_id, buyer_id, resource_given, amount_given, 
        resource_received, amount_received, exchange_rate)
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8)`,
      [
        offerId,
        offer.seller_id,
        userId,
        offer.resource_offered,
        offer.amount_offered,
        offer.resource_wanted,
        offer.amount_wanted,
        offer.exchange_rate,
      ]
    );

    res.json({ success: true });
  } catch (error: any) {
    res.status(400).json({ error: error.message });
  }
});

// Cancel trade offer
router.delete('/trade/offers/:offerId', async (req, res) => {
  try {
    const { id: userId } = resolveAuthUser(req);
    const offerId = parseInt(req.params.offerId);

    const result = await pool.query(
      `UPDATE trade_offers 
       SET status = 'cancelled'
       WHERE id = $1 AND seller_id = $2 AND status = 'active'
       RETURNING id`,
      [offerId, userId]
    );

    if (result.rows.length === 0) {
      return res.status(404).json({ error: 'Trade offer not found or cannot be cancelled' });
    }

    res.json({ success: true });
  } catch (error: any) {
    res.status(400).json({ error: error.message });
  }
});

// Get user's trade history
router.get('/trade/history', async (req, res) => {
  try {
    const { id: userId } = resolveAuthUser(req);
    const limit = parseInt(req.query.limit as string) || 50;
    const offset = parseInt(req.query.offset as string) || 0;

    const result = await pool.query(
      `SELECT 
         t.*,
         u1.username as seller_username,
         u2.username as buyer_username
       FROM trade_transactions t
       JOIN users u1 ON t.seller_id = u1.id
       JOIN users u2 ON t.buyer_id = u2.id
       WHERE t.seller_id = $1 OR t.buyer_id = $1
       ORDER BY t.created_at DESC
       LIMIT $2 OFFSET $3`,
      [userId, limit, offset]
    );

    const countResult = await pool.query(
      `SELECT COUNT(*) FROM trade_transactions WHERE seller_id = $1 OR buyer_id = $1`,
      [userId]
    );

    res.json({
      transactions: result.rows,
      total: parseInt(countResult.rows[0].count),
    });
  } catch (error: any) {
    res.status(500).json({ error: error.message });
  }
});

export default router;
