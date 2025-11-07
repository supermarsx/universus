# Quick Start Guide - New Features

## Overview

This guide covers the newly implemented Leaderboard and Messaging systems, along with the development infrastructure setup.

## New Features Implemented

### 1. Leaderboard System

#### API Endpoints

**Get Top Players**
```bash
GET /api/leaderboard/players?limit=100&offset=0
Authorization: Bearer <token>

Response:
{
  "success": true,
  "data": [
    {
      "userId": 1,
      "username": "Player1",
      "totalScore": 125000,
      "rank": 1,
      "allianceTag": "ELITE"
    },
    ...
  ],
  "pagination": {
    "limit": 100,
    "offset": 0,
    "total": 100
  }
}
```

**Get Top Alliances**
```bash
GET /api/leaderboard/alliances?limit=50&offset=0
Authorization: Bearer <token>

Response:
{
  "success": true,
  "data": [
    {
      "allianceId": 1,
      "allianceName": "Elite Alliance",
      "allianceTag": "ELITE",
      "totalScore": 5000000,
      "memberCount": 25,
      "averageScore": 200000,
      "rank": 1
    },
    ...
  ]
}
```

**Get My Rank**
```bash
GET /api/leaderboard/me?range=5
Authorization: Bearer <token>

Response:
{
  "success": true,
  "data": {
    "player": {
      "userId": 123,
      "username": "MyPlayer",
      "totalScore": 125000,
      "buildingScore": 50000,
      "researchScore": 35000,
      "fleetScore": 30000,
      "defenseScore": 10000,
      "rank": 42
    },
    "neighbors": [
      // 5 players above and 5 below
    ]
  }
}
```

**Manual Update (Admin)**
```bash
POST /api/leaderboard/update
Authorization: Bearer <token>

Response:
{
  "success": true,
  "data": {
    "playersUpdated": 150,
    "alliancesUpdated": 12
  }
}
```

### 2. Messaging System

#### API Endpoints

**Get Inbox**
```bash
GET /api/messages/inbox?limit=50&offset=0&type=player_message
Authorization: Bearer <token>

Response:
{
  "success": true,
  "data": [
    {
      "id": 1,
      "fromUserId": 2,
      "toUserId": 1,
      "fromUsername": "Sender",
      "toUsername": "Recipient",
      "subject": "Hello!",
      "content": "Message content here",
      "messageType": "player_message",
      "isRead": false,
      "createdAt": "2025-11-06T01:00:00.000Z"
    },
    ...
  ]
}
```

**Send Message**
```bash
POST /api/messages/send
Authorization: Bearer <token>
Content-Type: application/json

{
  "toUserId": 2,
  "subject": "Trade Offer",
  "content": "Would you like to trade resources?"
}

Response:
{
  "success": true,
  "data": {
    "id": 15,
    "fromUserId": 1,
    "toUserId": 2,
    "subject": "Trade Offer",
    "content": "Would you like to trade resources?",
    "messageType": "player_message",
    "isRead": false,
    "createdAt": "2025-11-06T01:00:00.000Z"
  },
  "message": "Message sent successfully"
}
```

**Get Unread Count**
```bash
GET /api/messages/unread-count
Authorization: Bearer <token>

Response:
{
  "success": true,
  "data": {
    "count": 5
  }
}
```

**Mark as Read**
```bash
PUT /api/messages/15/read
Authorization: Bearer <token>

Response:
{
  "success": true,
  "message": "Message marked as read"
}
```

**Alliance Circular (Broadcast)**
```bash
POST /api/messages/alliance-circular
Authorization: Bearer <token>
Content-Type: application/json

{
  "subject": "Alliance Meeting",
  "content": "Emergency alliance meeting tonight at 8 PM!"
}

Response:
{
  "success": true,
  "data": {
    "sentCount": 24
  },
  "message": "Message sent to 24 alliance members"
}
```

#### Message Types

The system supports 6 message types:
- `player_message` - Direct player-to-player messages
- `combat_report` - Automated combat reports
- `espionage_report` - Intelligence gathering reports
- `system_notification` - System announcements
- `alliance_message` - Alliance communications
- `alliance_circular` - Broadcast to all alliance members

## Development Tools

### Running Tests

```bash
cd backend

# Run all tests with coverage
pnpm run test

# Watch mode for development
pnpm run test:watch

# Run only unit tests
pnpm run test:unit

# Run only integration tests
pnpm run test:integration
```

### Code Quality

```bash
# Lint code
pnpm run lint

# Lint and auto-fix issues
pnpm run lint:fix

# Format code
pnpm run format

# Check formatting
pnpm run format:check

# Type check without building
pnpm run type-check

# Run all validations
pnpm run validate
```

### Building

```bash
# Build TypeScript to JavaScript
pnpm run build

# Run development server with auto-reload
pnpm run dev

# Run production build
pnpm run start
```

## Database Migration

Before using the messaging system, run the migration:

```bash
cd backend
psql -U postgres -d ogame_rpg -f src/database/migrations/001_update_messages_table.sql
```

This updates the messages table structure to support the new messaging service.

## Testing the Features

### 1. Start the Server

```bash
cd /workspace/ogame-rpg
docker-compose up -d
```

### 2. Login and Get Token

```bash
# Register or login
curl -X POST http://localhost:3000/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"email":"test@example.com","password":"password123"}'

# Save the token from response
export TOKEN="your_jwt_token_here"
```

### 3. Test Leaderboard

```bash
# Get top players
curl -X GET "http://localhost:3000/api/leaderboard/players?limit=10" \
  -H "Authorization: Bearer $TOKEN"

# Get my rank
curl -X GET "http://localhost:3000/api/leaderboard/me" \
  -H "Authorization: Bearer $TOKEN"

# Update leaderboard manually
curl -X POST http://localhost:3000/api/leaderboard/update \
  -H "Authorization: Bearer $TOKEN"
```

### 4. Test Messaging

```bash
# Check unread count
curl -X GET http://localhost:3000/api/messages/unread-count \
  -H "Authorization: Bearer $TOKEN"

# Get inbox
curl -X GET "http://localhost:3000/api/messages/inbox?limit=20" \
  -H "Authorization: Bearer $TOKEN"

# Send a message (requires valid toUserId)
curl -X POST http://localhost:3000/api/messages/send \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"toUserId":2,"subject":"Test","content":"Hello!"}'
```

## Score Calculation

Player scores are automatically calculated based on:

### Building Score
- Metal Mine: 60 * 1.5^(level-1)
- Crystal Mine: 48 * 1.6^(level-1)
- Deuterium Synthesizer: 225 * 1.5^(level-1)
- Other buildings: Various formulas

### Research Score
- Based on technology costs * 2^(level-1)

### Fleet Score
- Small Cargo: 4,000 per unit
- Large Cargo: 12,000 per unit
- Light Fighter: 3,000 per unit
- Battleship: 45,000 per unit
- Deathstar: 5,000,000 per unit

### Defense Score
- Rocket Launcher: 2,000 per unit
- Plasma Turret: 50,000 per unit
- Large Shield Dome: 50,000 per unit

## Performance Notes

- Leaderboard data is cached in Redis for 5 minutes
- Leaderboard updates should be run periodically (every 5-10 minutes)
- Consider setting up a cron job for automatic updates:
  ```bash
  */5 * * * * curl -X POST http://localhost:3000/api/leaderboard/update \
    -H "Authorization: Bearer $ADMIN_TOKEN"
  ```

## Next Steps

1. **Create UI pages** for leaderboard and messaging
2. **Write tests** for the new services
3. **Implement automated leaderboard updates** in the game loop
4. **Add combat/espionage report generation** integration
5. **Create admin panel** for managing messages and leaderboards

## Documentation

- Full API documentation: See `PRODUCTION_ENHANCEMENT_PROGRESS.md`
- Code documentation: Check JSDoc comments in source files
- Architecture: See inline comments in service files

---

For questions or issues, refer to the comprehensive documentation in the project root.
