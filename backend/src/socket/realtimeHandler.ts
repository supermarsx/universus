/**
 * PHASE 6: REALTIME SOCKET HANDLER
 * Enhanced Socket.io integration for comprehensive real-time features
 */

import { Server as SocketIOServer, Socket } from 'socket.io';
import {
  ChatMessageEvent,
  PrivateMessageEvent,
  NotificationEvent,
  PlayerStatusEvent,
  FleetMovementEvent,
  CombatAlertEvent,
  TradeUpdateEvent,
  PlayerStatus,
} from '../types/realtime';
import chatService from '../services/chatService';
import notificationService from '../services/notificationService';
import { pool } from '../config/database';
import redis from '../config/redis';

interface AuthenticatedSocket extends Socket {
  userId: number;
  username: string;
}

export class RealtimeSocketHandler {
  private io: SocketIOServer;

  constructor(io: SocketIOServer) {
    this.io = io;
    this.setupEventHandlers();
  }

  private setupEventHandlers(): void {
    this.io.on('connection', (socket: Socket) => {
      this.handleConnection(socket as AuthenticatedSocket);
    });
  }

  private async handleConnection(socket: AuthenticatedSocket): Promise<void> {
    const userId = socket.userId;
    const username = socket.username;

    console.log(`[Realtime] User connected: ${username} (${userId})`);

    // Join user's personal room
    socket.join(`user:${userId}`);

    // Update player status
    await this.updatePlayerStatus(userId, PlayerStatus.ONLINE, socket.id);

    // Broadcast player online status
    this.broadcastPlayerStatus(userId, username, PlayerStatus.ONLINE);

    // Setup event listeners
    this.setupChatListeners(socket);
    this.setupPrivateMessageListeners(socket);
    this.setupNotificationListeners(socket);
    this.setupPlayerStatusListeners(socket);
    this.setupFleetListeners(socket);
    this.setupTradeListeners(socket);
    this.setupConfigurationListeners(socket);

    // Handle disconnect
    socket.on('disconnect', () => this.handleDisconnect(socket));
  }

  // =====================================================
  // CHAT LISTENERS
  // =====================================================

  private setupChatListeners(socket: AuthenticatedSocket): void {
    const userId = socket.userId;
    const username = socket.username;

    // Subscribe to chat channel
    socket.on('chat:subscribe', async (channelId: number) => {
      try {
        const channel = await chatService.getChannelById(channelId);
        if (!channel) {
          socket.emit('error', { message: 'Channel not found' });
          return;
        }

        socket.join(`chat:${channelId}`);
        console.log(`[Chat] ${username} joined channel ${channel.channel_name}`);

        // Send recent history
        const history = await chatService.getChatHistory({ channelId, limit: 50 });
        socket.emit('chat:history', { channelId, messages: history.messages });
      } catch (error: any) {
        socket.emit('error', { message: error.message });
      }
    });

    // Unsubscribe from chat channel
    socket.on('chat:unsubscribe', (channelId: number) => {
      socket.leave(`chat:${channelId}`);
      console.log(`[Chat] ${username} left channel ${channelId}`);
    });

    // Send chat message
    socket.on('chat:message', async (data: { channelId: number; message: string }) => {
      try {
        const chatMessage = await chatService.sendMessage(userId, {
          channelId: data.channelId,
          message: data.message,
        });

        const event: ChatMessageEvent = {
          channelId: data.channelId,
          channelName: '',
          userId,
          username,
          message: data.message,
          messageType: chatMessage.message_type,
          timestamp: chatMessage.created_at,
          messageId: chatMessage.id,
        };

        // Broadcast to all users in channel
        this.io.to(`chat:${data.channelId}`).emit('chat:new_message', event);

        // Log activity
        await this.logPlayerActivity(userId, 'chat_message', { channelId: data.channelId });
      } catch (error: any) {
        socket.emit('error', { message: error.message });
      }
    });

    // Edit message
    socket.on('chat:edit', async (data: { messageId: number; newMessage: string }) => {
      try {
        await chatService.editMessage(data.messageId, userId, data.newMessage);
        this.io.to(`chat:*`).emit('chat:message_edited', {
          messageId: data.messageId,
          newMessage: data.newMessage,
          editedAt: new Date(),
        });
      } catch (error: any) {
        socket.emit('error', { message: error.message });
      }
    });

    // Delete message
    socket.on('chat:delete', async (messageId: number) => {
      try {
        await chatService.deleteMessage(messageId, userId);
        this.io.to(`chat:*`).emit('chat:message_deleted', { messageId });
      } catch (error: any) {
        socket.emit('error', { message: error.message });
      }
    });
  }

  // =====================================================
  // PRIVATE MESSAGE LISTENERS
  // =====================================================

  private setupPrivateMessageListeners(socket: AuthenticatedSocket): void {
    const userId = socket.userId;
    const username = socket.username;

    // Send private message
    socket.on('pm:send', async (data: { receiverId: number; message: string }) => {
      try {
        const privateMessage = await chatService.sendPrivateMessage(userId, {
          receiverId: data.receiverId,
          message: data.message,
        });

        const event: PrivateMessageEvent = {
          conversationId: privateMessage.conversation_id,
          senderId: userId,
          senderUsername: username,
          receiverId: data.receiverId,
          message: data.message,
          timestamp: privateMessage.created_at,
          messageId: privateMessage.id,
        };

        // Send to receiver
        this.io.to(`user:${data.receiverId}`).emit('pm:new_message', event);

        // Confirm to sender
        socket.emit('pm:sent', event);
      } catch (error: any) {
        socket.emit('error', { message: error.message });
      }
    });

    // Mark conversation as read
    socket.on('pm:mark_read', async (conversationId: number) => {
      try {
        await chatService.markMessagesAsRead(conversationId, userId);
        socket.emit('pm:marked_read', { conversationId });
      } catch (error: any) {
        socket.emit('error', { message: error.message });
      }
    });

    // Subscribe to conversation updates
    socket.on('pm:subscribe', (conversationId: number) => {
      socket.join(`conversation:${conversationId}`);
    });

    // Typing indicator
    socket.on('pm:typing', (data: { conversationId: number; receiverId: number }) => {
      this.io.to(`user:${data.receiverId}`).emit('pm:user_typing', {
        conversationId: data.conversationId,
        userId,
        username,
      });
    });
  }

  // =====================================================
  // NOTIFICATION LISTENERS
  // =====================================================

  private setupNotificationListeners(socket: AuthenticatedSocket): void {
    const userId = socket.userId;

    // Mark notification as read
    socket.on('notification:mark_read', async (notificationId: number) => {
      try {
        await notificationService.markAsRead(notificationId, userId);
        socket.emit('notification:read', { notificationId });
      } catch (error: any) {
        socket.emit('error', { message: error.message });
      }
    });

    // Mark all notifications as read
    socket.on('notification:mark_all_read', async () => {
      try {
        const count = await notificationService.markAllAsRead(userId);
        socket.emit('notification:all_read', { count });
      } catch (error: any) {
        socket.emit('error', { message: error.message });
      }
    });

    // Get unread count
    socket.on('notification:get_unread_count', async () => {
      try {
        const count = await notificationService.getUnreadCount(userId);
        socket.emit('notification:unread_count', { count });
      } catch (error: any) {
        socket.emit('error', { message: error.message });
      }
    });
  }

  // =====================================================
  // PLAYER STATUS LISTENERS
  // =====================================================

  private setupPlayerStatusListeners(socket: AuthenticatedSocket): void {
    const userId = socket.userId;
    const username = socket.username;

    // Update status
    socket.on('status:update', async (data: { status: PlayerStatus; statusMessage?: string }) => {
      try {
        await this.updatePlayerStatus(userId, data.status, socket.id, data.statusMessage);
        this.broadcastPlayerStatus(userId, username, data.status, data.statusMessage);
      } catch (error: any) {
        socket.emit('error', { message: error.message });
      }
    });

    // Request online players
    socket.on('status:get_online_players', async () => {
      try {
        const result = await pool.query(
          `SELECT user_id, status, status_message, last_activity
           FROM player_status
           WHERE status = 'online' AND last_activity > CURRENT_TIMESTAMP - INTERVAL '5 minutes'
           LIMIT 100`
        );

        socket.emit('status:online_players', { players: result.rows });
      } catch (error: any) {
        socket.emit('error', { message: error.message });
      }
    });

    // Subscribe to user status
    socket.on('status:subscribe', (targetUserId: number) => {
      socket.join(`user_status:${targetUserId}`);
    });
  }

  // =====================================================
  // FLEET LISTENERS
  // =====================================================

  private setupFleetListeners(socket: AuthenticatedSocket): void {
    const userId = socket.userId;

    // Subscribe to fleet updates
    socket.on('fleet:subscribe', (fleetId: number) => {
      socket.join(`fleet:${fleetId}`);
      console.log(`[Fleet] User ${userId} watching fleet ${fleetId}`);
    });

    // Unsubscribe from fleet updates
    socket.on('fleet:unsubscribe', (fleetId: number) => {
      socket.leave(`fleet:${fleetId}`);
    });

    // Request fleet status
    socket.on('fleet:get_status', async (fleetId: number) => {
      try {
        const result = await pool.query(
          `SELECT * FROM fleet_events WHERE fleet_id = $1 ORDER BY created_at DESC LIMIT 1`,
          [fleetId]
        );

        if (result.rows.length > 0) {
          socket.emit('fleet:status', result.rows[0]);
        }
      } catch (error: any) {
        socket.emit('error', { message: error.message });
      }
    });
  }

  // =====================================================
  // TRADE LISTENERS
  // =====================================================

  private setupTradeListeners(socket: AuthenticatedSocket): void {
    const userId = socket.userId;

    // Subscribe to trade updates
    socket.on('trade:subscribe', () => {
      socket.join('trade:global');
      console.log(`[Trade] User ${userId} subscribed to trade updates`);
    });

    // Unsubscribe from trade updates
    socket.on('trade:unsubscribe', () => {
      socket.leave('trade:global');
    });
  }

  // =====================================================
  // DISCONNECT HANDLER
  // =====================================================

  private async handleDisconnect(socket: AuthenticatedSocket): Promise<void> {
    const userId = socket.userId;
    const username = socket.username;

    console.log(`[Realtime] User disconnected: ${username} (${userId})`);

    // Update player status to offline
    await this.updatePlayerStatus(userId, PlayerStatus.OFFLINE, null);

    // Broadcast player offline status
    this.broadcastPlayerStatus(userId, username, PlayerStatus.OFFLINE);

    // Log activity
    await this.logPlayerActivity(userId, 'logout');
  }

  // =====================================================
  // BROADCAST METHODS
  // =====================================================

  // Broadcast notification to user
  async broadcastNotification(userId: number, notification: NotificationEvent): Promise<void> {
    this.io.to(`user:${userId}`).emit('notification:new', notification);
  }

  // Broadcast player status change
  private broadcastPlayerStatus(
    userId: number,
    username: string,
    status: PlayerStatus,
    statusMessage?: string
  ): void {
    const event: PlayerStatusEvent = {
      userId,
      username,
      status,
      statusMessage,
      lastActivity: new Date(),
    };

    // Broadcast to all connected users
    this.io.emit('player:status_change', event);

    // Also broadcast to status subscribers
    this.io.to(`user_status:${userId}`).emit('status:update', event);
  }

  // Broadcast fleet movement update
  async broadcastFleetMovement(fleetId: number, event: FleetMovementEvent): Promise<void> {
    this.io.to(`fleet:${fleetId}`).emit('fleet:movement', event);

    // Also notify fleet owner
    this.io.to(`user:${event.ownerId}`).emit('fleet:update', event);
  }

  // Broadcast combat alert
  async broadcastCombatAlert(alert: CombatAlertEvent): Promise<void> {
    // Send to attacker
    this.io.to(`user:${alert.attackerId}`).emit('combat:alert', alert);

    // Send to defender
    this.io.to(`user:${alert.defenderId}`).emit('combat:alert', alert);

    // Broadcast to combat channel
    this.io.to('chat:combat').emit('combat:public_alert', {
      attackerUsername: alert.attackerUsername,
      defenderUsername: alert.defenderUsername,
      severity: alert.severity,
    });
  }

  // Broadcast trade update
  async broadcastTradeUpdate(trade: TradeUpdateEvent): Promise<void> {
    this.io.to('trade:global').emit('trade:new_offer', trade);
  }

  // =====================================================
  // RESEARCH EVENTS
  // =====================================================

  public emitResearchUpdate(userId: number, payload: any): void {
    this.io.to(`user:${userId}`).emit('researchUpdate', payload);
  }

  public emitResearchComplete(userId: number, payload: any): void {
    this.io.to(`user:${userId}`).emit('researchComplete', payload);
  }

  public emitFleetUpdate(userId: number, payload: any): void {
    this.io.to(`user:${userId}`).emit('fleetUpdate', payload);
  }

  // =====================================================
  // CONFIGURATION LISTENERS (Phase 7)
  // =====================================================

  private setupConfigurationListeners(socket: AuthenticatedSocket): void {
    const userId = socket.userId;

    // Subscribe to configuration updates (admin only)
    socket.on('config:subscribe', async () => {
      try {
        // Verify user is admin
        const result = await pool.query(
          'SELECT is_admin FROM users WHERE id = $1',
          [userId]
        );

        if (result.rows.length > 0 && result.rows[0].is_admin) {
          socket.join('config:updates');
          console.log(`[Config] Admin ${socket.username} subscribed to configuration updates`);
        }
      } catch (error: any) {
        socket.emit('error', { message: 'Failed to subscribe to configuration updates' });
      }
    });

    // Unsubscribe from configuration updates
    socket.on('config:unsubscribe', () => {
      socket.leave('config:updates');
      console.log(`[Config] ${socket.username} unsubscribed from configuration updates`);
    });
  }

  // Broadcast configuration change to all admins
  async broadcastConfigurationChange(data: {
    key: string;
    oldValue: any;
    newValue: any;
    changedBy: number;
    changedByUsername: string;
    timestamp: Date;
  }): Promise<void> {
    this.io.to('config:updates').emit('config:changed', {
      key: data.key,
      oldValue: data.oldValue,
      newValue: data.newValue,
      changedBy: data.changedBy,
      changedByUsername: data.changedByUsername,
      timestamp: data.timestamp,
    });

    console.log(`[Config] Broadcasted change: ${data.key} by ${data.changedByUsername}`);
  }

  // Broadcast configuration reload to all connected clients
  async broadcastConfigurationReload(): Promise<void> {
    this.io.emit('config:reload', {
      timestamp: new Date(),
      message: 'Configuration has been reloaded. Please refresh to see changes.',
    });

    console.log('[Config] Broadcasted configuration reload to all clients');
  }

  // Broadcast bulk configuration update
  async broadcastBulkConfigurationUpdate(data: {
    changes: Array<{ key: string; oldValue: any; newValue: any }>;
    changedBy: number;
    changedByUsername: string;
    timestamp: Date;
  }): Promise<void> {
    this.io.to('config:updates').emit('config:bulk_update', {
      changes: data.changes,
      changedBy: data.changedBy,
      changedByUsername: data.changedByUsername,
      timestamp: data.timestamp,
    });

    console.log(`[Config] Broadcasted bulk update: ${data.changes.length} changes by ${data.changedByUsername}`);
  }

  // =====================================================
  // UTILITY METHODS
  // =====================================================

  private async updatePlayerStatus(
    userId: number,
    status: PlayerStatus,
    socketId: string | null,
    statusMessage?: string
  ): Promise<void> {
    await pool.query(
      `INSERT INTO player_status 
       (user_id, status, socket_id, status_message, last_activity, session_count, updated_at)
       VALUES ($1, $2, $3, $4, CURRENT_TIMESTAMP, 1, CURRENT_TIMESTAMP)
       ON CONFLICT (user_id)
       DO UPDATE SET
         status = EXCLUDED.status,
         socket_id = EXCLUDED.socket_id,
         status_message = COALESCE(EXCLUDED.status_message, player_status.status_message),
         last_activity = CURRENT_TIMESTAMP,
         session_count = player_status.session_count + 1,
         updated_at = CURRENT_TIMESTAMP`,
      [userId, status, socketId, statusMessage]
    );

    // Update online users set in Redis
    if (status === PlayerStatus.ONLINE) {
      await redis.sadd('online_users', userId.toString());
    } else {
      await redis.srem('online_users', userId.toString());
    }
  }

  private async logPlayerActivity(
    userId: number,
    activityType: string,
    activityData?: any
  ): Promise<void> {
    try {
      await pool.query(
        `INSERT INTO player_activity_log (user_id, activity_type, activity_data)
         VALUES ($1, $2, $3)`,
        [userId, activityType, activityData ? JSON.stringify(activityData) : null]
      );
    } catch (error) {
      console.error('Failed to log player activity:', error);
    }
  }

  // Get online player count
  async getOnlinePlayerCount(): Promise<number> {
    const count = await redis.scard('online_users');
    return count;
  }

  // Get all online user IDs
  async getOnlineUserIds(): Promise<number[]> {
    const userIds = await redis.smembers('online_users');
    return userIds.map((id) => parseInt(id));
  }
}

export default RealtimeSocketHandler;
