import { Server as SocketIOServer, Socket } from 'socket.io';
import { Server as HTTPServer } from 'http';
import jwt from 'jsonwebtoken';
import { pool } from '../config/database';
import redis from '../config/redis';
import RealtimeSocketHandler from './realtimeHandler';

let realtimeHandler: RealtimeSocketHandler;

export function initializeSocket(httpServer: HTTPServer): SocketIOServer {
  const io = new SocketIOServer(httpServer, {
    cors: {
      origin: '*',
      methods: ['GET', 'POST'],
    },
  });

  // Authentication middleware
  io.use(async (socket: Socket, next) => {
    try {
      const token = socket.handshake.auth.token;
      
      if (!token) {
        return next(new Error('Authentication error'));
      }

      const secret = process.env.JWT_SECRET || 'your_super_secret_jwt_key';
      const decoded = jwt.verify(token, secret) as { userId: number };

      const result = await pool.query(
        'SELECT id, username, is_banned FROM users WHERE id = $1',
        [decoded.userId]
      );

      if (result.rows.length === 0 || result.rows[0].is_banned) {
        return next(new Error('User not found or banned'));
      }

      (socket as any).userId = decoded.userId;
      (socket as any).username = result.rows[0].username;
      
      next();
    } catch (error) {
      next(new Error('Authentication error'));
    }
  });

  // Initialize Phase 6 Realtime Handler
  realtimeHandler = new RealtimeSocketHandler(io);

  // Legacy connection handler (for backwards compatibility)
  io.on('connection', (socket: Socket) => {
    const userId = (socket as any).userId;
    const username = (socket as any).username;
    
    console.log(`User connected: ${username} (${userId})`);

    // Join user's personal room
    socket.join(`user:${userId}`);

    // Store online status in Redis (already handled by realtimeHandler)
    // redis.sadd('online_users', userId.toString());

    // Handle planet subscription
    socket.on('subscribe:planet', (planetId: number) => {
      socket.join(`planet:${planetId}`);
      console.log(`User ${username} subscribed to planet ${planetId}`);
    });

    socket.on('unsubscribe:planet', (planetId: number) => {
      socket.leave(`planet:${planetId}`);
    });

    // Handle alliance chat subscription (legacy - replaced by Phase 6)
    socket.on('subscribe:alliance', (allianceId: number) => {
      socket.join(`alliance:${allianceId}`);
    });

    socket.on('alliance:message', async (data: { allianceId: number; message: string }) => {
      try {
        // Verify user is in alliance
        const result = await pool.query(
          'SELECT 1 FROM alliance_members WHERE alliance_id = $1 AND user_id = $2',
          [data.allianceId, userId]
        );

        if (result.rows.length > 0) {
          // Save message
          await pool.query(
            'INSERT INTO alliance_chat (alliance_id, user_id, message) VALUES ($1, $2, $3)',
            [data.allianceId, userId, data.message]
          );

          // Broadcast to alliance
          io.to(`alliance:${data.allianceId}`).emit('alliance:new_message', {
            username,
            message: data.message,
            timestamp: new Date(),
          });
        }
      } catch (error) {
        console.error('Alliance chat error:', error);
      }
    });

    // Handle disconnect (Phase 6 realtimeHandler also handles this)
    socket.on('disconnect', () => {
      console.log(`User disconnected: ${username} (${userId})`);
      // redis.srem('online_users', userId.toString());
    });
  });

  return io;
}

// Helper function to emit to specific users
export async function emitToUser(
  io: SocketIOServer,
  userId: number,
  event: string,
  data: any
): Promise<void> {
  io.to(`user:${userId}`).emit(event, data);
}

// Helper function to emit to planet subscribers
export async function emitToPlanet(
  io: SocketIOServer,
  planetId: number,
  event: string,
  data: any
): Promise<void> {
  io.to(`planet:${planetId}`).emit(event, data);
}

// Export realtime handler instance for use in services
export function getRealtimeHandler(): RealtimeSocketHandler {
  return realtimeHandler;
}
