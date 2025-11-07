# PHASE 5 SHARDING - QUICK REFERENCE GUIDE

## Server Setup

### Environment Variables
```bash
# Enable sharding
ENABLE_SHARDING=true

# Server identification
SERVER_ID=game-us-east-1
SERVER_NAME=US East Server 1
SERVER_REGION=us-east
SERVER_HOST=game1.universus.com
SERVER_CAPACITY=1000

# Ports
PORT=3000
WS_PORT=3001

# Redis
REDIS_URL=redis://localhost:6379
```

### Start Server with Sharding
```typescript
// Automatic initialization when ENABLE_SHARDING=true
npm start

// Logs will show:
// - Cross-server communication initialized
// - Server health monitoring started
// - Global leaderboard updates started
// - Server registered: game-us-east-1
```

## API Usage Examples

### 1. Route Player to Server
```bash
curl -X POST http://localhost:3000/api/shards/routing/calculate \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{
    "user_id": 12345,
    "preferred_region": "us-east"
  }'

# Response:
{
  "success": true,
  "data": {
    "server_id": "game-us-east-1",
    "server_name": "US East Server 1",
    "host_address": "game1.universus.com",
    "port": 3000,
    "websocket_port": 3001,
    "region": "us-east",
    "estimated_latency": 45,
    "routing_algorithm": "geographic"
  }
}
```

### 2. Get Server Health
```bash
curl http://localhost:3000/api/shards/servers/game-us-east-1/health \
  -H "Authorization: Bearer <admin_token>"

# Response:
{
  "success": true,
  "data": {
    "server_id": "game-us-east-1",
    "status": "online",
    "health_score": 95,
    "checks": {
      "api_responsive": true,
      "database_connected": true,
      "redis_connected": true,
      "websocket_active": true,
      "disk_space_available": true
    },
    "metrics": {
      "cpu_usage": 45.2,
      "memory_usage": 62.8,
      "response_time": 87,
      "active_connections": 423
    }
  }
}
```

### 3. Broadcast Message to All Servers
```bash
curl -X POST http://localhost:3000/api/shards/messages/broadcast \
  -H "Authorization: Bearer <admin_token>" \
  -H "Content-Type: application/json" \
  -d '{
    "message_type": "system_announcement",
    "payload": {
      "title": "Server Maintenance",
      "message": "Scheduled maintenance in 30 minutes",
      "duration": "2 hours"
    },
    "priority": "high"
  }'
```

### 4. Get Global Leaderboard
```bash
curl "http://localhost:3000/api/shards/leaderboards/total_points?limit=50&offset=0"

# Response:
{
  "success": true,
  "data": {
    "data": [
      {
        "rank": 1,
        "user_id": 42,
        "username": "SpaceCommander",
        "server_id": "game-us-east-1",
        "score": 1500000,
        "rank_change": 2,
        "alliance_name": "Galactic Empire"
      },
      // ... more entries
    ],
    "total": 1000,
    "page": 1,
    "per_page": 50,
    "total_pages": 20
  }
}
```

### 5. Migrate Player
```bash
curl -X POST http://localhost:3000/api/shards/routing/migrate \
  -H "Authorization: Bearer <admin_token>" \
  -H "Content-Type: application/json" \
  -d '{
    "user_id": 12345,
    "from_server_id": "game-us-east-1",
    "to_server_id": "game-us-west-1",
    "reason": "load_balancing",
    "preserve_session": true
  }'
```

## Service Integration

### Using ServerDiscoveryService
```typescript
import serverDiscoveryService from './services/serverDiscoveryService';

// Register a new server
await serverDiscoveryService.registerServer({
  server_id: 'game-eu-west-1',
  server_name: 'EU West Server 1',
  server_type: 'game',
  region: 'eu-west',
  host_address: 'game-eu-1.universus.com',
  port: 3000,
  websocket_port: 3001,
  capacity: 1000
});

// Update server health
await serverDiscoveryService.updateServerHealth({
  server_id: 'game-eu-west-1',
  cpu_usage: 45.2,
  memory_usage: 62.8,
  response_time_ms: 87,
  current_load: 423,
  health_score: 95
});

// Get healthy servers
const servers = await serverDiscoveryService.getHealthyServers();
```

### Using PlayerRoutingService
```typescript
import playerRoutingService from './services/playerRoutingService';

// Route player
const routing = await playerRoutingService.routePlayer({
  user_id: 12345,
  preferred_region: 'us-east'
});

// Migrate player
await playerRoutingService.migratePlayer({
  user_id: 12345,
  from_server_id: 'game-us-east-1',
  to_server_id: 'game-us-west-1',
  reason: 'load_balancing',
  preserve_session: true
});

// Auto-balance
const migratedCount = await playerRoutingService.autoBalancePlayers();
```

### Using CrossServerCommunication
```typescript
import crossServerCommunication from './services/crossServerCommunicationService';

// Initialize (done automatically in index.ts)
await crossServerCommunication.initialize();

// Broadcast to all servers
await crossServerCommunication.broadcastToAllServers(
  'game_event',
  { type: 'galaxy_conquest', galaxy_id: 42 },
  'high'
);

// Send to specific servers
await crossServerCommunication.sendToServers(
  ['game-us-east-1', 'game-us-west-1'],
  'region_alert',
  { message: 'High activity detected' },
  'normal'
);

// Register message handler
crossServerCommunication.registerHandler('custom_event', async (message) => {
  console.log('Received:', message.payload);
  // Process message
});

// Publish to topic
await crossServerCommunication.publishToTopic('player_updates', {
  user_id: 123,
  action: 'level_up'
});
```

### Using GlobalLeaderboardService
```typescript
import globalLeaderboardService from './services/globalLeaderboardService';

// Update player score
await globalLeaderboardService.updatePlayerEntry(
  12345,                    // user_id
  'game-us-east-1',        // server_id
  'total_points',          // category
  1500000,                 // score
  42                       // alliance_id
);

// Get leaderboard
const leaderboard = await globalLeaderboardService.getGlobalLeaderboard({
  category: 'total_points',
  limit: 50,
  offset: 0
});

// Get player rank
const rank = await globalLeaderboardService.getPlayerRank(
  12345,
  'total_points'
);
```

## Load Balancing Algorithms

### Configure Algorithm
```typescript
import playerRoutingService from './services/playerRoutingService';

playerRoutingService.updateConfig({
  algorithm: 'weighted',           // or 'round_robin', 'least_connections', etc.
  max_server_load: 0.85,          // 85% capacity
  failover_enabled: true,
  weighted_factors: {
    cpu_weight: 0.3,
    memory_weight: 0.2,
    latency_weight: 0.3,
    load_weight: 0.2
  }
});
```

## Monitoring

### Health Check Endpoint
```bash
curl http://localhost:3000/api/shards/health/overview

# Response includes:
# - Server statistics
# - Routing statistics
# - Messaging statistics
# - Timestamp
```

### Server Statistics
```bash
curl http://localhost:3000/api/shards/servers/stats \
  -H "Authorization: Bearer <admin_token>"

# Returns:
# - total_servers
# - online_servers
# - total_capacity
# - total_load
# - average_health_score
# - average_cpu_usage
# - average_memory_usage
```

## Troubleshooting

### Server Not Registering
```typescript
// Check Redis connection
const status = crossServerCommunication.getStatus();
console.log(status); // Should show connected: true

// Manually register
await serverDiscoveryService.registerServer({...});
```

### Players Not Routing
```typescript
// Check available servers
const servers = await serverDiscoveryService.getHealthyServers();
console.log(`${servers.length} servers available`);

// Get player assignment
const assignment = await playerRoutingService.getPlayerAssignment(userId);
console.log(assignment);
```

### Cross-Server Messages Not Received
```typescript
// Check message handler registration
crossServerCommunication.registerHandler('your_message_type', async (msg) => {
  console.log('Handler called:', msg);
});

// Check connection status
const status = crossServerCommunication.getStatus();
console.log('Subscriber ready:', status.subscriber_ready);
```

## Best Practices

### 1. Server Registration
- Always use unique SERVER_ID
- Set appropriate capacity based on hardware
- Include geographic region for optimal routing
- Update health metrics regularly (30s intervals)

### 2. Load Balancing
- Use 'geographic' for latency-sensitive games
- Use 'weighted' for balanced resource utilization
- Enable failover for production environments
- Monitor server load and trigger rebalancing at 80%+

### 3. Cross-Server Communication
- Use appropriate priority levels
- Set reasonable TTL for messages
- Register handlers before initializing
- Handle message errors gracefully

### 4. Leaderboards
- Update scores in real-time or batch
- Create daily snapshots for historical data
- Use pagination for large leaderboards
- Cache top players for quick access

### 5. Health Monitoring
- Set appropriate thresholds for alerts
- Monitor CPU, memory, and response times
- Trigger failover at health score < 30
- Log all health state changes

## Production Checklist

- [ ] Redis cluster configured for HA
- [ ] Load balancer (HAProxy/NGINX) set up
- [ ] All servers registered with unique IDs
- [ ] Health monitoring enabled
- [ ] Leaderboard updates scheduled
- [ ] Cross-server messaging tested
- [ ] Failover tested and verified
- [ ] Performance benchmarks met
- [ ] Monitoring dashboards configured
- [ ] Alert rules set up
- [ ] Backup and disaster recovery plan
- [ ] Documentation updated

---

**Quick Reference Version:** 1.0  
**Last Updated:** 2025-11-06  
**For:** Universus Phase 5 Sharding System
