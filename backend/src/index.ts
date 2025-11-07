import express from 'express';
import http from 'http';
import cors from 'cors';
import dotenv from 'dotenv';
import path from 'path';

// Load environment variables
dotenv.config();

// Import template configuration
import { configureTemplateEngine } from './config/templateConfig';

// Import routes
import authRoutes from './routes/auth';
import planetRoutes from './routes/planets';
import userRoutes from './routes/users';
import shipyardRoutes from './routes/shipyard';
import fleetRoutes from './routes/fleet';
import researchRoutes from './routes/research';
import galaxyRoutes from './routes/galaxy';
import leaderboardRoutes from './routes/leaderboard';
import messagesRoutes from './routes/messages';
import shopRoutes from './routes/shop';
import adminRoutes from './routes/admin';
import adminApiRoutes from './routes/adminRoutes';
import botRoutes from './routes/bots';
import templateRoutes from './routes/templates';
import debrisRoutes from './routes/debrisRoutes';
import universeRoutes from './routes/universeRoutes';
import shardingRoutes from './routes/shardingRoutes';
import realtimeRoutes from './routes/realtimeRoutes';
import configRoutes from './routes/configRoutes';
import themeRoutes from './routes/themeRoutes';
import accountRoutes from './routes/accountRoutes';
import enhancedShopRoutes from './routes/enhancedShopRoutes';
import allianceRoutes from './routes/allianceRoutes';

// Import services
import { GameLoopService } from './services/gameLoopService';
import { BotAIService } from './services/botAIService';
import { initializeSocket } from './socket';
import { startMonitoring, autoExpireBlocks } from './services/adminMonitoringService';
import debrisService from './services/debrisService';
import serverDiscoveryService from './services/serverDiscoveryService';
import crossServerCommunication from './services/crossServerCommunicationService';
import globalLeaderboardService from './services/globalLeaderboardService';
import chatService from './services/chatService';
import notificationService from './services/notificationService';
import { themeScheduler } from './services/themeScheduler';

// Initialize Express app
const app = express();
const PORT = process.env.PORT || 3000;

// Configure Nunjucks templating engine
configureTemplateEngine(app);

// Middleware
app.use(cors());
app.use(express.json());
app.use(express.urlencoded({ extended: true }));

// Serve static files (frontend)
app.use(express.static(path.join(__dirname, '../../frontend')));

// Template Routes (must be before static files)
app.use('/', templateRoutes);

// API Routes
app.use('/api/auth', authRoutes);
app.use('/api/planets', planetRoutes);
app.use('/api/users', userRoutes);
app.use('/api/shipyard', shipyardRoutes);
app.use('/api/fleet', fleetRoutes);
app.use('/api/research', researchRoutes);
app.use('/api/galaxy', galaxyRoutes);
app.use('/api/leaderboard', leaderboardRoutes);
app.use('/api/messages', messagesRoutes);
app.use('/api/shop', shopRoutes);
app.use('/api/admin', adminRoutes);
app.use('/api/admin', adminApiRoutes); // New comprehensive admin API
app.use('/api/admin/bots', botRoutes);
app.use('/api/debris', debrisRoutes); // Debris & Salvage System
app.use('/api/universe', universeRoutes); // Universe Seeding System
app.use('/api/shards', shardingRoutes); // Phase 5: Server Sharding System
app.use('/api/realtime', realtimeRoutes); // Phase 6: Real-time Communication System
app.use('/api/config', configRoutes); // Phase 7: Configuration System
app.use('/api/themes', themeRoutes); // Phase 8: Seasonal Theme System
app.use('/api/account', accountRoutes); // Phase 9: Advanced Account Management System
app.use('/api/shop-enhanced', enhancedShopRoutes); // Phase 10: Enhanced Shop & Matrix Theme
app.use('/api/alliances', allianceRoutes); // Phase 11: Enhanced Alliance Management System

// Health check endpoint
app.get('/api/health', (req, res) => {
  res.json({ status: 'ok', timestamp: new Date().toISOString() });
});

// Catch-all route for frontend (commented out temporarily for Express 5.x compatibility)
// Will serve static files through express.static middleware above
// app.get('/*', (req, res, next) => {
//   if (!req.path.startsWith('/api')) {
//     res.sendFile(path.join(__dirname, '../../frontend/index.html'));
//   } else {
//     next();
//   }
// });

// Error handling middleware
app.use((err: any, req: express.Request, res: express.Response, next: express.NextFunction) => {
  console.error('Error:', err);
  res.status(err.status || 500).json({
    error: err.message || 'Internal server error',
  });
});

// Create HTTP server
const server = http.createServer(app);

// Initialize Socket.io
const io = initializeSocket(server);

// Start game loop
GameLoopService.start();

// Start admin monitoring service (collect metrics every minute)
startMonitoring(60000);
console.log('Admin monitoring service started');

// Auto-expire user blocks every 5 minutes
setInterval(() => {
  autoExpireBlocks().catch(console.error);
}, 300000);
console.log('Block expiration scheduler started');

// Start debris cleanup service (auto-decay and cleanup every hour)
debrisService.startAutomaticCleanup(60);
console.log('Debris cleanup service started');

// Phase 6: Start chat and notification cleanup services
setInterval(() => {
  chatService.autoExpireRestrictions().catch(console.error);
  notificationService.performScheduledCleanup().catch(console.error);
}, 3600000); // Run every hour
console.log('Chat and notification cleanup services started');

// Phase 8: Start theme scheduler (check every minute)
themeScheduler.start();
console.log('Theme scheduler started');

// Phase 5: Initialize sharding services
if (process.env.ENABLE_SHARDING === 'true') {
  // Initialize cross-server communication
  crossServerCommunication.initialize()
    .then(() => {
      console.log('Cross-server communication initialized');
      
      // Start server health monitoring
      serverDiscoveryService.startHealthMonitoring();
      console.log('Server health monitoring started');
      
      // Start automatic leaderboard updates
      globalLeaderboardService.startAutomaticUpdates();
      console.log('Global leaderboard updates started');
      
      // Register this server if SERVER_ID is set
      if (process.env.SERVER_ID) {
        serverDiscoveryService.registerServer({
          server_id: process.env.SERVER_ID,
          server_name: process.env.SERVER_NAME || `Universus Server ${process.env.SERVER_ID}`,
          server_type: 'game' as any,
          region: (process.env.SERVER_REGION as any) || 'us-east',
          host_address: process.env.SERVER_HOST || 'localhost',
          port: parseInt(process.env.PORT || '3000'),
          websocket_port: parseInt(process.env.WS_PORT || '3001'),
          capacity: parseInt(process.env.SERVER_CAPACITY || '1000')
        }).then(() => {
          console.log(`Server registered: ${process.env.SERVER_ID}`);
        }).catch(err => {
          console.error('Failed to register server:', err);
        });
      }
    })
    .catch(err => {
      console.error('Failed to initialize sharding services:', err);
      console.log('Sharding services disabled');
    });
} else {
  console.log('Sharding disabled (set ENABLE_SHARDING=true to enable)');
}

// Graceful shutdown
process.on('SIGTERM', () => {
  console.log('SIGTERM received, shutting down gracefully');
  GameLoopService.stop();
  themeScheduler.stop();
  
  // Shutdown sharding services
  if (process.env.ENABLE_SHARDING === 'true') {
    serverDiscoveryService.stopHealthMonitoring();
    globalLeaderboardService.stopAutomaticUpdates();
    crossServerCommunication.disconnect().catch(console.error);
  }
  
  server.close(() => {
    console.log('Server closed');
    process.exit(0);
  });
});

process.on('SIGINT', () => {
  console.log('SIGINT received, shutting down gracefully');
  GameLoopService.stop();
  themeScheduler.stop();
  
  // Shutdown sharding services
  if (process.env.ENABLE_SHARDING === 'true') {
    serverDiscoveryService.stopHealthMonitoring();
    globalLeaderboardService.stopAutomaticUpdates();
    crossServerCommunication.disconnect().catch(console.error);
  }
  
  server.close(() => {
    console.log('Server closed');
    process.exit(0);
  });
});

export { io };
