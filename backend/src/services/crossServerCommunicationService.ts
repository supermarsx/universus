// =====================================================
// CROSS-SERVER COMMUNICATION SERVICE
// Redis pub/sub for real-time inter-server messaging
// =====================================================

import pool from '../config/database';
import { createClient, RedisClientType } from 'redis';
import {
  CrossServerEvent,
  ServerMessageEnvelope,
  MessageAcknowledgement,
  MessagePriority,
  ChatChannelType,
  ShardingApiResponse
} from '../types/sharding';

export class CrossServerCommunicationService {
  private publisher: RedisClientType | null = null;
  private subscriber: RedisClientType | null = null;
  private messageHandlers: Map<string, (message: ServerMessageEnvelope) => Promise<void>> = new Map();
  private connected: boolean = false;
  private readonly REDIS_URL = process.env.REDIS_URL || 'redis://localhost:6379';

  // Channel names
  private readonly CHANNELS = {
    BROADCAST: 'shard:broadcast',
    CHAT: 'shard:chat',
    EVENTS: 'shard:events',
    LEADERBOARD: 'shard:leaderboard',
    MARKET: 'shard:market',
    SYSTEM: 'shard:system',
    ALERTS: 'shard:alerts'
  };

  // =======================================================
  // CONNECTION MANAGEMENT
  // =====================================================

  /**
   * Initialize Redis connections
   */
  async initialize(): Promise<void> {
    if (this.connected) {
      console.log('Cross-server communication already initialized');
      return;
    }

    try {
      // Create publisher client
      this.publisher = createClient({ url: this.REDIS_URL });
      await this.publisher.connect();

      // Create subscriber client
      this.subscriber = createClient({ url: this.REDIS_URL });
      await this.subscriber.connect();

      // Subscribe to all channels
      await this.subscribeToChannels();

      this.connected = true;
      console.log('Cross-server communication initialized successfully');

    } catch (error) {
      console.error('Failed to initialize cross-server communication:', error);
      throw error;
    }
  }

  /**
   * Subscribe to all shard channels
   */
  private async subscribeToChannels(): Promise<void> {
    if (!this.subscriber) return;

    const channels = Object.values(this.CHANNELS);

    for (const channel of channels) {
      await this.subscriber.subscribe(channel, async (message) => {
        try {
          const envelope: ServerMessageEnvelope = JSON.parse(message);
          await this.handleIncomingMessage(envelope);
        } catch (error) {
          console.error(`Error processing message from ${channel}:`, error);
        }
      });
    }

    console.log(`Subscribed to ${channels.length} shard channels`);
  }

  /**
   * Disconnect from Redis
   */
  async disconnect(): Promise<void> {
    if (this.publisher) {
      await this.publisher.quit();
      this.publisher = null;
    }

    if (this.subscriber) {
      await this.subscriber.quit();
      this.subscriber = null;
    }

    this.connected = false;
    console.log('Cross-server communication disconnected');
  }

  // =====================================================
  // MESSAGE PUBLISHING
  // =====================================================

  /**
   * Broadcast message to all servers
   */
  async broadcastToAllServers(
    messageType: string,
    payload: any,
    priority: MessagePriority = MessagePriority.NORMAL
  ): Promise<void> {
    if (!this.publisher || !this.connected) {
      throw new Error('Publisher not connected');
    }

    const envelope: ServerMessageEnvelope = {
      message_id: this.generateMessageId(),
      source_server: process.env.SERVER_ID || 'unknown',
      target_servers: ['*'], // All servers
      message_type: messageType,
      payload,
      timestamp: new Date(),
      priority,
      ttl: 300 // 5 minutes
    };

    await this.publisher.publish(this.CHANNELS.BROADCAST, JSON.stringify(envelope));
    
    // Store in database for audit
    await this.storeEvent(envelope);
  }

  /**
   * Send message to specific servers
   */
  async sendToServers(
    targetServers: string[],
    messageType: string,
    payload: any,
    priority: MessagePriority = MessagePriority.NORMAL
  ): Promise<void> {
    if (!this.publisher || !this.connected) {
      throw new Error('Publisher not connected');
    }

    const envelope: ServerMessageEnvelope = {
      message_id: this.generateMessageId(),
      source_server: process.env.SERVER_ID || 'unknown',
      target_servers: targetServers,
      message_type: messageType,
      payload,
      timestamp: new Date(),
      priority,
      ttl: 300
    };

    // Publish to broadcast channel (servers will filter by target)
    await this.publisher.publish(this.CHANNELS.BROADCAST, JSON.stringify(envelope));
    
    await this.storeEvent(envelope);
  }

  /**
   * Publish chat message
   */
  async publishChatMessage(
    channelType: ChatChannelType,
    channelId: string,
    message: string,
    userId: number,
    targetServers?: string[]
  ): Promise<void> {
    const envelope: ServerMessageEnvelope = {
      message_id: this.generateMessageId(),
      source_server: process.env.SERVER_ID || 'unknown',
      target_servers: targetServers || ['*'],
      message_type: 'chat_message',
      payload: {
        channel_type: channelType,
        channel_id: channelId,
        message,
        user_id: userId,
        timestamp: new Date()
      },
      timestamp: new Date(),
      priority: MessagePriority.NORMAL
    };

    if (this.publisher && this.connected) {
      await this.publisher.publish(this.CHANNELS.CHAT, JSON.stringify(envelope));
    }
  }

  /**
   * Publish leaderboard update
   */
  async publishLeaderboardUpdate(
    category: string,
    updates: any[]
  ): Promise<void> {
    const envelope: ServerMessageEnvelope = {
      message_id: this.generateMessageId(),
      source_server: process.env.SERVER_ID || 'unknown',
      target_servers: ['*'],
      message_type: 'leaderboard_update',
      payload: {
        category,
        updates,
        timestamp: new Date()
      },
      timestamp: new Date(),
      priority: MessagePriority.HIGH
    };

    if (this.publisher && this.connected) {
      await this.publisher.publish(this.CHANNELS.LEADERBOARD, JSON.stringify(envelope));
    }
  }

  /**
   * Publish market price update
   */
  async publishMarketUpdate(
    resourceType: string,
    priceData: any
  ): Promise<void> {
    const envelope: ServerMessageEnvelope = {
      message_id: this.generateMessageId(),
      source_server: process.env.SERVER_ID || 'unknown',
      target_servers: ['*'],
      message_type: 'market_update',
      payload: {
        resource_type: resourceType,
        price_data: priceData,
        timestamp: new Date()
      },
      timestamp: new Date(),
      priority: MessagePriority.NORMAL
    };

    if (this.publisher && this.connected) {
      await this.publisher.publish(this.CHANNELS.MARKET, JSON.stringify(envelope));
    }
  }

  /**
   * Publish system alert
   */
  async publishSystemAlert(
    alertType: string,
    message: string,
    severity: 'low' | 'medium' | 'high' | 'critical',
    targetServers?: string[]
  ): Promise<void> {
    const envelope: ServerMessageEnvelope = {
      message_id: this.generateMessageId(),
      source_server: process.env.SERVER_ID || 'unknown',
      target_servers: targetServers || ['*'],
      message_type: 'system_alert',
      payload: {
        alert_type: alertType,
        message,
        severity,
        timestamp: new Date()
      },
      timestamp: new Date(),
      priority: severity === 'critical' ? MessagePriority.CRITICAL : MessagePriority.HIGH
    };

    if (this.publisher && this.connected) {
      await this.publisher.publish(this.CHANNELS.ALERTS, JSON.stringify(envelope));
    }
  }

  /**
   * Publish game event
   */
  async publishGameEvent(
    eventType: string,
    eventData: any,
    targetServers?: string[]
  ): Promise<void> {
    const envelope: ServerMessageEnvelope = {
      message_id: this.generateMessageId(),
      source_server: process.env.SERVER_ID || 'unknown',
      target_servers: targetServers || ['*'],
      message_type: 'game_event',
      payload: {
        event_type: eventType,
        event_data: eventData,
        timestamp: new Date()
      },
      timestamp: new Date(),
      priority: MessagePriority.NORMAL
    };

    if (this.publisher && this.connected) {
      await this.publisher.publish(this.CHANNELS.EVENTS, JSON.stringify(envelope));
    }
  }

  // =====================================================
  // MESSAGE HANDLING
  // =====================================================

  /**
   * Handle incoming message
   */
  private async handleIncomingMessage(envelope: ServerMessageEnvelope): Promise<void> {
    const currentServerId = process.env.SERVER_ID || 'unknown';

    // Check if message is for this server
    const isTargeted = 
      envelope.target_servers.includes('*') ||
      envelope.target_servers.includes(currentServerId);

    if (!isTargeted) {
      return; // Not for this server
    }

    // Check message TTL
    if (envelope.ttl) {
      const age = Date.now() - new Date(envelope.timestamp).getTime();
      if (age > envelope.ttl * 1000) {
        console.log(`Message ${envelope.message_id} expired (age: ${age}ms)`);
        return;
      }
    }

    // Get handler for this message type
    const handler = this.messageHandlers.get(envelope.message_type);

    if (handler) {
      try {
        await handler(envelope);
        
        // Send acknowledgement if required
        if (envelope.priority === MessagePriority.CRITICAL) {
          await this.sendAcknowledgement(envelope.message_id, envelope.source_server, 'processed');
        }

      } catch (error) {
        console.error(`Error handling message ${envelope.message_id}:`, error);
        
        // Send error acknowledgement
        await this.sendAcknowledgement(
          envelope.message_id,
          envelope.source_server,
          'failed',
          error instanceof Error ? error.message : 'Unknown error'
        );
      }
    } else {
      console.warn(`No handler registered for message type: ${envelope.message_type}`);
    }
  }

  /**
   * Register message handler
   */
  registerHandler(
    messageType: string,
    handler: (message: ServerMessageEnvelope) => Promise<void>
  ): void {
    this.messageHandlers.set(messageType, handler);
    console.log(`Registered handler for message type: ${messageType}`);
  }

  /**
   * Unregister message handler
   */
  unregisterHandler(messageType: string): void {
    this.messageHandlers.delete(messageType);
    console.log(`Unregistered handler for message type: ${messageType}`);
  }

  /**
   * Send acknowledgement
   */
  private async sendAcknowledgement(
    messageId: string,
    targetServer: string,
    status: 'received' | 'processed' | 'failed',
    error?: string
  ): Promise<void> {
    const ack: MessageAcknowledgement = {
      message_id: messageId,
      server_id: process.env.SERVER_ID || 'unknown',
      status,
      timestamp: new Date(),
      error
    };

    if (this.publisher && this.connected) {
      await this.publisher.publish(
        `shard:ack:${targetServer}`,
        JSON.stringify(ack)
      );
    }
  }

  // =====================================================
  // EVENT PERSISTENCE
  // =====================================================

  /**
   * Store event in database
   */
  private async storeEvent(envelope: ServerMessageEnvelope): Promise<void> {
    try {
      await pool.query(
        `INSERT INTO shard_events (
          event_type, source_server_id, target_server_ids,
          payload, priority, created_at
        ) VALUES ($1, $2, $3, $4, $5, $6)`,
        [
          envelope.message_type,
          envelope.source_server,
          envelope.target_servers,
          envelope.payload,
          envelope.priority,
          envelope.timestamp
        ]
      );
    } catch (error) {
      console.error('Error storing event:', error);
      // Don't throw - event storage failure shouldn't break messaging
    }
  }

  /**
   * Get event history
   */
  async getEventHistory(
    limit: number = 100,
    messageType?: string,
    serverId?: string
  ): Promise<CrossServerEvent[]> {
    let query = 'SELECT * FROM shard_events WHERE 1=1';
    const params: any[] = [];
    let paramIndex = 1;

    if (messageType) {
      query += ` AND event_type = $${paramIndex++}`;
      params.push(messageType);
    }

    if (serverId) {
      query += ` AND (source_server_id = $${paramIndex++} OR $${paramIndex++} = ANY(target_server_ids))`;
      params.push(serverId, serverId);
    }

    query += ` ORDER BY created_at DESC LIMIT $${paramIndex}`;
    params.push(limit);

    const result = await pool.query(query, params);

    return result.rows.map(row => ({
      id: row.id,
      event_type: row.event_type,
      source_server_id: row.source_server_id,
      target_server_ids: row.target_server_ids,
      payload: row.payload,
      priority: row.priority,
      requires_ack: row.requires_ack,
      created_at: row.created_at,
      processed_at: row.processed_at
    }));
  }

  // =====================================================
  // MESSAGING PATTERNS
  // =====================================================

  /**
   * Request-Response pattern
   */
  async sendRequest(
    targetServer: string,
    requestType: string,
    requestData: any,
    timeoutMs: number = 5000
  ): Promise<any> {
    return new Promise(async (resolve, reject) => {
      const requestId = this.generateMessageId();
      const responseChannel = `shard:response:${requestId}`;

      // Set up response listener with timeout
      const timeout = setTimeout(() => {
        reject(new Error(`Request timeout after ${timeoutMs}ms`));
      }, timeoutMs);

      // Subscribe to response channel
      if (this.subscriber) {
        await this.subscriber.subscribe(responseChannel, async (message) => {
          clearTimeout(timeout);
          const response = JSON.parse(message);
          resolve(response);
          
          // Cleanup
          if (this.subscriber) {
            await this.subscriber.unsubscribe(responseChannel);
          }
        });
      }

      // Send request
      const envelope: ServerMessageEnvelope = {
        message_id: requestId,
        source_server: process.env.SERVER_ID || 'unknown',
        target_servers: [targetServer],
        message_type: requestType,
        payload: {
          ...requestData,
          response_channel: responseChannel
        },
        timestamp: new Date(),
        priority: MessagePriority.HIGH
      };

      if (this.publisher && this.connected) {
        await this.publisher.publish(this.CHANNELS.SYSTEM, JSON.stringify(envelope));
      }
    });
  }

  /**
   * Publish-Subscribe pattern for real-time updates
   */
  async subscribeToTopic(
    topic: string,
    callback: (data: any) => void
  ): Promise<void> {
    const channel = `shard:topic:${topic}`;

    if (this.subscriber) {
      await this.subscriber.subscribe(channel, async (message) => {
        try {
          const data = JSON.parse(message);
          callback(data);
        } catch (error) {
          console.error(`Error in topic callback for ${topic}:`, error);
        }
      });
    }
  }

  /**
   * Publish to topic
   */
  async publishToTopic(topic: string, data: any): Promise<void> {
    const channel = `shard:topic:${topic}`;

    if (this.publisher && this.connected) {
      await this.publisher.publish(channel, JSON.stringify(data));
    }
  }

  // =====================================================
  // STATISTICS
  // =====================================================

  /**
   * Get messaging statistics
   */
  async getMessagingStatistics() {
    const result = await pool.query(`
      SELECT 
        COUNT(*) as total_events,
        COUNT(*) FILTER (WHERE created_at > NOW() - INTERVAL '1 hour') as events_last_hour,
        COUNT(*) FILTER (WHERE priority = 'critical') as critical_events,
        COUNT(DISTINCT event_type) as event_types,
        COUNT(DISTINCT source_server_id) as active_publishers
      FROM shard_events
      WHERE created_at > NOW() - INTERVAL '24 hours'
    `);

    const stats = result.rows[0];

    // Get event type breakdown
    const typeBreakdown = await pool.query(`
      SELECT 
        event_type,
        COUNT(*) as count
      FROM shard_events
      WHERE created_at > NOW() - INTERVAL '24 hours'
      GROUP BY event_type
      ORDER BY count DESC
      LIMIT 10
    `);

    return {
      total_events_24h: parseInt(stats.total_events),
      events_last_hour: parseInt(stats.events_last_hour),
      critical_events: parseInt(stats.critical_events),
      unique_event_types: parseInt(stats.event_types),
      active_publishers: parseInt(stats.active_publishers),
      events_per_minute: (parseInt(stats.events_last_hour) / 60).toFixed(2),
      top_event_types: typeBreakdown.rows.map(row => ({
        event_type: row.event_type,
        count: parseInt(row.count)
      }))
    };
  }

  // =====================================================
  // UTILITY METHODS
  // =====================================================

  private generateMessageId(): string {
    return `msg_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
  }

  /**
   * Check if connected
   */
  isConnected(): boolean {
    return this.connected;
  }

  /**
   * Get connection status
   */
  getStatus(): {
    connected: boolean;
    publisher_ready: boolean;
    subscriber_ready: boolean;
    handlers_registered: number;
  } {
    return {
      connected: this.connected,
      publisher_ready: this.publisher?.isReady || false,
      subscriber_ready: this.subscriber?.isReady || false,
      handlers_registered: this.messageHandlers.size
    };
  }
}

export default new CrossServerCommunicationService();
