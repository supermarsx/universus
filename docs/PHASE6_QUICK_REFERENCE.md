# PHASE 6: REAL-TIME COMMUNICATION - QUICK REFERENCE

**Version:** 1.0  
**Date:** 2025-11-06  
**Status:** Production Ready ✅

---

## Quick Start

### 1. Database Setup
```bash
psql -U postgres -d universus_db -f database/sql/phase6_realtime_schema.sql
```

### 2. Compile TypeScript
```bash
cd backend && npx tsc
```

### 3. Start Server
```bash
npm start
```

---

## API Endpoints Cheat Sheet

### Chat
```http
GET    /api/realtime/chat/channels                    # List channels
GET    /api/realtime/chat/channels/:id/messages       # Get history
POST   /api/realtime/chat/channels/:id/messages       # Send message
PUT    /api/realtime/chat/messages/:id                # Edit message
DELETE /api/realtime/chat/messages/:id                # Delete message
```

### Private Messages
```http
GET    /api/realtime/chat/conversations               # List conversations
GET    /api/realtime/chat/conversations/:id/messages  # Get messages
POST   /api/realtime/chat/private                     # Send PM
```

### Notifications
```http
GET    /api/realtime/notifications                    # List notifications
PUT    /api/realtime/notifications/:id/read           # Mark as read
PUT    /api/realtime/notifications/read/all           # Mark all read
GET    /api/realtime/notifications/unread/count       # Unread count
GET    /api/realtime/notifications/preferences        # Get preferences
```

### Players
```http
GET    /api/realtime/players/online                   # Online players
GET    /api/realtime/players/:id/status               # Player status
```

### Trading
```http
GET    /api/realtime/trade/offers                     # List offers
POST   /api/realtime/trade/offers                     # Create offer
POST   /api/realtime/trade/offers/:id/accept          # Accept offer
DELETE /api/realtime/trade/offers/:id                 # Cancel offer
GET    /api/realtime/trade/history                    # Trade history
```

---

## WebSocket Events

### Chat Events
```javascript
// Subscribe to channel
socket.emit('chat:subscribe', channelId);

// Send message
socket.emit('chat:message', { channelId, message });

// Listen for new messages
socket.on('chat:new_message', (data) => {
  console.log(data.username, ':', data.message);
});

// Listen for edits/deletes
socket.on('chat:message_edited', (data) => { });
socket.on('chat:message_deleted', (data) => { });
```

### Private Messages
```javascript
// Send PM
socket.emit('pm:send', { receiverId, message });

// Listen for new PMs
socket.on('pm:new_message', (data) => {
  console.log('PM from', data.senderUsername, ':', data.message);
});

// Typing indicator
socket.emit('pm:typing', { conversationId, receiverId });
socket.on('pm:user_typing', (data) => {
  console.log(data.username, 'is typing...');
});
```

### Notifications
```javascript
// Listen for notifications
socket.on('notification:new', (notification) => {
  console.log(notification.title, ':', notification.message);
});

// Mark as read
socket.emit('notification:mark_read', notificationId);

// Get unread count
socket.emit('notification:get_unread_count');
socket.on('notification:unread_count', (data) => {
  console.log('Unread:', data.count);
});
```

### Player Status
```javascript
// Update status
socket.emit('status:update', { 
  status: 'away', 
  statusMessage: 'AFK for 10 mins' 
});

// Listen for status changes
socket.on('player:status_change', (data) => {
  console.log(data.username, 'is now', data.status);
});

// Get online players
socket.emit('status:get_online_players');
socket.on('status:online_players', (data) => {
  console.log('Online:', data.players.length);
});
```

### Fleet Tracking
```javascript
// Watch fleet
socket.emit('fleet:subscribe', fleetId);

// Listen for updates
socket.on('fleet:movement', (event) => {
  console.log('Fleet', event.fleetId, ':', 
    event.progressPercent, '%');
});
```

### Trading
```javascript
// Subscribe to trade updates
socket.emit('trade:subscribe');

// Listen for new offers
socket.on('trade:new_offer', (offer) => {
  console.log('New offer:', offer.resourceOffered, 
    offer.amountOffered);
});
```

---

## Service Usage

### ChatService

```typescript
import chatService from './services/chatService';

// Send message
const message = await chatService.sendMessage(userId, {
  channelId: 1,
  message: 'Hello, world!'
});

// Get chat history
const history = await chatService.getChatHistory({
  channelId: 1,
  limit: 50
});

// Send private message
const pm = await chatService.sendPrivateMessage(senderId, {
  receiverId: targetUserId,
  message: 'Hello!'
});

// Get conversations
const conversations = await chatService.getPrivateConversations(
  userId, 
  20, // limit
  0   // offset
);
```

### NotificationService

```typescript
import notificationService from './services/notificationService';

// Create notification
await notificationService.createNotification({
  userId,
  notificationTypeId: 3, // under_attack
  title: 'Under Attack!',
  message: 'Your planet is being attacked',
  priority: 5,
  actionUrl: '/combat/123',
  actionLabel: 'View Battle'
});

// Quick notification methods
await notificationService.notifyUnderAttack(
  userId, 
  'AttackerName', 
  'PlanetName', 
  combatId
);

await notificationService.notifyFleetArrived(
  userId, 
  fleetId, 
  'Galaxy 1:2:3'
);

// Get notifications
const result = await notificationService.getUserNotifications({
  userId,
  unreadOnly: true,
  category: 'combat',
  limit: 50
});

// Mark as read
await notificationService.markAsRead(notificationId, userId);
await notificationService.markAllAsRead(userId);
```

### RealtimeSocketHandler

```typescript
import { getRealtimeHandler } from './socket';

const handler = getRealtimeHandler();

// Broadcast notification
await handler.broadcastNotification(userId, {
  notificationId: 123,
  userId,
  type: 'under_attack',
  category: 'combat',
  title: 'Under Attack!',
  message: 'Your planet is being attacked',
  priority: 5,
  timestamp: new Date()
});

// Broadcast fleet movement
await handler.broadcastFleetMovement(fleetId, {
  fleetId,
  ownerId: userId,
  eventType: 'moving',
  progressPercent: 45,
  estimatedArrival: new Date(),
  currentLocation: { galaxy: 1, system: 2, position: 3 }
});

// Broadcast combat alert
await handler.broadcastCombatAlert({
  combatId,
  alertType: 'combat_started',
  attackerId,
  attackerUsername: 'Attacker',
  defenderId,
  defenderUsername: 'Defender',
  severity: 5,
  data: {},
  timestamp: new Date()
});
```

---

## Database Tables Reference

### chat_channels
- `id` - Channel ID
- `channel_name` - Display name
- `channel_type` - Type (global, trade, alliance, etc.)
- `max_message_length` - Max characters
- `rate_limit_seconds` - Seconds between messages

### chat_messages
- `id` - Message ID
- `channel_id` - Channel reference
- `user_id` - Sender
- `message` - Content
- `is_deleted` - Soft delete flag
- `is_flagged` - Moderation flag

### notifications
- `id` - Notification ID
- `user_id` - Recipient
- `notification_type_id` - Type reference
- `title` - Notification title
- `message` - Notification content
- `priority` - 1-5 (5 = urgent)
- `is_read` - Read status
- `action_url` - Click action

### player_status
- `user_id` - Player ID
- `status` - online/offline/away/busy/in_combat
- `last_activity` - Timestamp
- `socket_id` - Current socket
- `session_count` - Login count

### trade_offers
- `id` - Offer ID
- `seller_id` - Seller
- `resource_offered` - Resource type
- `amount_offered` - Amount
- `resource_wanted` - Wanted resource
- `amount_wanted` - Wanted amount
- `status` - active/completed/cancelled/expired
- `expires_at` - Expiration timestamp

---

## Common Tasks

### 1. Add New Notification Type

```sql
INSERT INTO notification_types 
(type_name, category, description, default_priority, icon)
VALUES 
('new_event', 'system', 'New game event', 3, 'event');
```

### 2. Create Custom Chat Channel

```sql
INSERT INTO chat_channels 
(channel_name, channel_type, description, max_message_length, rate_limit_seconds)
VALUES 
('Spanish Chat', 'global', 'Spanish language chat', 500, 3);
```

### 3. Moderate User

```typescript
// Mute user for 24 hours
await chatService.restrictUser(
  userId, 
  channelId, 
  'mute', 
  'Spam', 
  adminId, 
  1440 // minutes
);

// Ban user globally
await chatService.restrictUser(
  userId, 
  null,  // null = all channels
  'ban', 
  'Inappropriate behavior', 
  adminId, 
  null   // null = permanent
);

// Remove restriction
await chatService.removeRestriction(
  userId, 
  channelId, 
  'mute'
);
```

### 4. Send Batch Notifications

```typescript
// Notify all alliance members
const memberIds = [1, 2, 3, 4, 5];
await notificationService.createBatchNotifications({
  userIds: memberIds,
  notificationTypeId: 8, // alliance_message
  title: 'Alliance Meeting',
  message: 'Meeting at 20:00 UTC',
  priority: 3
});
```

### 5. Clean Up Old Data

```typescript
// Clean old chat messages (30 days)
const deleted = await chatService.cleanupOldMessages(30);

// Clean old notifications (30 days)
await notificationService.cleanupOldNotifications(30);

// Expire old trade offers
await pool.query('SELECT auto_expire_trades()');
```

---

## Monitoring

### Check Online Players
```sql
SELECT COUNT(*) 
FROM player_status 
WHERE status = 'online' 
  AND last_activity > NOW() - INTERVAL '5 minutes';
```

### Chat Activity
```sql
SELECT * FROM v_chat_activity;
```

### Unread Notifications
```sql
SELECT * FROM v_user_unread_notifications 
WHERE unread_count > 0
ORDER BY urgent_count DESC;
```

### Active Trades
```sql
SELECT * FROM v_active_trades 
ORDER BY created_at DESC;
```

### Recent Player Activity
```sql
SELECT 
  u.username,
  pal.activity_type,
  pal.created_at
FROM player_activity_log pal
JOIN users u ON pal.user_id = u.id
ORDER BY pal.created_at DESC
LIMIT 50;
```

---

## Troubleshooting

### Socket Not Connecting
1. Check JWT token is valid
2. Verify user is not banned
3. Check CORS settings
4. Verify Socket.io server is running

### Messages Not Sending
1. Check rate limiting (`chat:ratelimit` in Redis)
2. Verify user is not restricted (muted/banned)
3. Check message length limits
4. Verify channel exists and is active

### Notifications Not Appearing
1. Check user preferences
2. Verify notification type is active
3. Check priority filtering
4. Verify Redis connection for unread counts

### Trade Offers Not Showing
1. Check offer status (must be 'active')
2. Verify expiration date
3. Check alliance-only restrictions
4. Verify resource filters

---

## Performance Tips

### 1. Use Redis Caching
```typescript
// Check Redis before database
const cached = await redis.get('key');
if (cached) return JSON.parse(cached);

// Query database and cache result
const result = await db.query(...);
await redis.setex('key', 300, JSON.stringify(result));
```

### 2. Limit Query Results
```typescript
// Always use LIMIT and OFFSET
const messages = await chatService.getChatHistory({
  channelId,
  limit: 50,  // Don't fetch everything
  before: lastMessageDate
});
```

### 3. Use Database Views
```sql
-- Use pre-built views for complex queries
SELECT * FROM v_active_players;
SELECT * FROM v_chat_activity;
```

### 4. Index Important Columns
```sql
-- Already created in schema, but verify:
EXPLAIN ANALYZE 
SELECT * FROM chat_messages 
WHERE channel_id = 1 
ORDER BY created_at DESC 
LIMIT 50;
```

---

## Security Checklist

- [x] JWT authentication on all endpoints
- [x] User ownership verification
- [x] Rate limiting per channel
- [x] Input validation (message length, amounts)
- [x] SQL injection prevention (parameterized queries)
- [x] XSS prevention (sanitize user input on frontend)
- [x] CSRF protection (stateless JWT)
- [x] Admin-only operations protected
- [x] User blocking/muting system
- [x] Audit logging (player_activity_log)

---

## Files Overview

| File | Purpose | Lines |
|------|---------|-------|
| `phase6_realtime_schema.sql` | Database schema | 561 |
| `types/realtime.ts` | TypeScript types | 679 |
| `services/chatService.ts` | Chat logic | 562 |
| `services/notificationService.ts` | Notifications | 562 |
| `socket/realtimeHandler.ts` | Socket.io events | 491 |
| `socket/index.ts` | Socket integration | 139 |
| `routes/realtimeRoutes.ts` | REST API | 611 |

**Total:** 3,605 lines

---

## Support

For issues or questions:
1. Check error logs in console
2. Verify database migration completed
3. Check Redis connection
4. Review API documentation in full implementation report

---

**Phase 6: Real-Time Communication Systems**  
**Status:** Complete ✅  
**Ready for Production Deployment**
