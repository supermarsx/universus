/**
 * @module backend/routes/messages
 *
 * Messaging API routes. Handles inbox, sending, unread counts and message
 * retrieval for authenticated users.
 */

import express from 'express';
import { MessagingService, MessageType } from '../services/messagingService';
import { authenticateToken, assertAuthenticated } from '../middleware/auth';
import { AuthRequest } from '../types';
import { pool } from '../config/database';
import { AllianceService } from '../services/allianceService';
import { AlliancePermission } from '../types/alliance';
import { getUserId } from '../utils/authHelpers';

const router = express.Router();
const messagingService = new MessagingService(pool);
const allianceService = new AllianceService();

// Apply authentication middleware to all routes
router.use(authenticateToken, assertAuthenticated);


/**
 * GET /messages/inbox
 * Get user's inbox messages
 * Query params: limit, offset, type
 */
router.get('/inbox', async (req: AuthRequest, res) => {
  try {
    const userId = getUserId(req)!;
    const limit = parseInt(req.query.limit as string) || 50;
    const offset = parseInt(req.query.offset as string) || 0;
    const messageType = req.query.type as MessageType | undefined;

    const messages = await messagingService.getInbox(userId, limit, offset, messageType);

    res.json({
      success: true,
      data: messages,
      pagination: {
        limit,
        offset,
        total: messages.length,
      },
    });
  } catch (error: any) {
    console.error('Error fetching inbox:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to fetch inbox',
    });
  }
});


/**
 * GET /messages/sent
 * Get user's sent messages
 * Query params: limit, offset
 */
router.get('/sent', async (req: AuthRequest, res) => {
  try {
    const userId = getUserId(req)!;
    const limit = parseInt(req.query.limit as string) || 50;
    const offset = parseInt(req.query.offset as string) || 0;

    const messages = await messagingService.getSentMessages(userId, limit, offset);

    res.json({
      success: true,
      data: messages,
      pagination: {
        limit,
        offset,
        total: messages.length,
      },
    });
  } catch (error: any) {
    console.error('Error fetching sent messages:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to fetch sent messages',
    });
  }
});


/**
 * GET /messages/unread-count
 * Get unread message count
 * Query params: type (optional)
 */
router.get('/unread-count', async (req: AuthRequest, res) => {
  try {
    const userId = getUserId(req)!;
    const messageType = req.query.type as MessageType | undefined;

    const count = await messagingService.getUnreadCount(userId, messageType);

    res.json({
      success: true,
      data: { count },
    });
  } catch (error: any) {
    console.error('Error fetching unread count:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to fetch unread count',
    });
  }
});


/**
 * GET /messages/:id
 * Get a specific message
 */
router.get('/:id', async (req: AuthRequest, res) => {
  try {
    const userId = getUserId(req)!;
    const messageId = parseInt(req.params.id);

    const message = await messagingService.getMessage(messageId, userId);

    if (!message) {
      return res.status(404).json({
        success: false,
        error: 'Message not found',
      });
    }

    // Auto-mark as read when retrieving
    if (!message.isRead && message.toUserId === userId) {
      await messagingService.markAsRead(messageId, userId);
    }

    res.json({
      success: true,
      data: message,
    });
  } catch (error: any) {
    console.error('Error fetching message:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to fetch message',
    });
  }
});


/**
 * POST /messages/send
 * Send a new message
 * Body: { toUserId, subject, content }
 */
router.post('/send', async (req: AuthRequest, res) => {
  try {
    const fromUserId = getUserId(req)!;
    const { toUserId, subject, content } = req.body;

    if (!toUserId || !subject || !content) {
      return res.status(400).json({
        success: false,
        error: 'Missing required fields: toUserId, subject, content',
      });
    }

    // Check if recipient exists
    const recipientCheck = await pool.query('SELECT id FROM users WHERE id = $1', [toUserId]);

    if (recipientCheck.rows.length === 0) {
      return res.status(404).json({
        success: false,
        error: 'Recipient not found',
      });
    }

    const message = await messagingService.sendMessage({
      fromUserId,
      toUserId,
      subject,
      content,
      messageType: MessageType.PLAYER_MESSAGE,
    });

    res.json({
      success: true,
      data: message,
      message: 'Message sent successfully',
    });
  } catch (error: any) {
    console.error('Error sending message:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to send message',
    });
  }
});

/**
 * PUT /messages/:id/read
 * Mark a message as read
 */
router.put('/:id/read', async (req: AuthRequest, res) => {
  try {
    const userId = getUserId(req)!;
    const messageId = parseInt(req.params.id);

    const success = await messagingService.markAsRead(messageId, userId);

    if (!success) {
      return res.status(404).json({
        success: false,
        error: 'Message not found or already read',
      });
    }

    res.json({
      success: true,
      message: 'Message marked as read',
    });
  } catch (error: any) {
    console.error('Error marking message as read:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to mark message as read',
    });
  }
});

/**
 * PUT /messages/mark-all-read
 * Mark all messages as read
 * Query params: type (optional)
 */
router.put('/mark-all-read', async (req: AuthRequest, res) => {
  try {
    const userId = getUserId(req)!;
    const messageType = req.query.type as MessageType | undefined;

    const count = await messagingService.markAllAsRead(userId, messageType);

    res.json({
      success: true,
      data: { count },
      message: `${count} messages marked as read`,
    });
  } catch (error: any) {
    console.error('Error marking all as read:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to mark all as read',
    });
  }
});

/**
 * DELETE /messages/:id
 * Delete a message
 */
router.delete('/:id', async (req: AuthRequest, res) => {
  try {
    const userId = getUserId(req)!;
    const messageId = parseInt(req.params.id);

    const success = await messagingService.deleteMessage(messageId, userId);

    if (!success) {
      return res.status(404).json({
        success: false,
        error: 'Message not found',
      });
    }

    res.json({
      success: true,
      message: 'Message deleted successfully',
    });
  } catch (error: any) {
    console.error('Error deleting message:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to delete message',
    });
  }
});

/**
 * POST /messages/alliance-circular
 * Send message to all alliance members
 * Body: { subject, content }
 */
router.post('/alliance-circular', async (req: AuthRequest, res) => {
  try {
    const userId = getUserId(req)!;
    const { subject, content } = req.body;

    if (!subject || !content) {
      return res.status(400).json({
        success: false,
        error: 'Missing required fields: subject, content',
      });
    }

    // Check if user is in an alliance
    const userQuery = await pool.query('SELECT alliance_id FROM users WHERE id = $1', [userId]);

    const allianceId = userQuery.rows[0]?.alliance_id;

    if (!allianceId) {
      return res.status(400).json({
        success: false,
        error: 'You are not in an alliance',
      });
    }

    const canSend = await allianceService.checkPermission(
      allianceId,
      userId,
      AlliancePermission.SEND_ANNOUNCEMENTS
    );

    if (!canSend) {
      return res.status(403).json({
        success: false,
        error: 'You do not have permission to send alliance circulars',
      });
    }

    const sentCount = await messagingService.sendAllianceCircular(
      allianceId,
      userId,
      subject,
      content
    );

    res.json({
      success: true,
      data: { sentCount },
      message: `Message sent to ${sentCount} alliance members`,
    });
  } catch (error: any) {
    console.error('Error sending alliance circular:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to send alliance circular',
    });
  }
});

export default router;


