# Bot System Quick Reference

## Quick Start

### Deploy Bot System
```bash
cd /workspace/ogame-rpg
./deploy-bot-system.sh
```

This script will:
1. Start PostgreSQL and Redis
2. Apply migration 005 (bot system tables)
3. Build and start backend server
4. Run automated tests
5. Provide access URLs

### Manual Deployment

#### 1. Start Services
```bash
# PostgreSQL
sudo service postgresql start

# Redis
sudo service redis-server start
```

#### 2. Apply Migration
```bash
cd /workspace/ogame-rpg
PGPASSWORD=postgres psql -h 127.0.0.1 -U postgres -d ogame_rpg \
    -f backend/src/database/migrations/005_bot_system.sql
```

#### 3. Build and Start Backend
```bash
cd backend
npm run build
npm start
```

#### 4. Access Bot Management
Open: `http://localhost:3000/admin/bots.html`
Login: `admin@example.com` / `admin123`

## Bot Management UI

### Access Points
- **Bot Management:** `/admin/bots.html`
- **Main Admin Panel:** `/admin/admin.html`
- **Backend API:** `http://localhost:3000/api/admin/bots`

### Features
1. **Dashboard** - Summary statistics (total bots, active bots, attacks, plunder)
2. **Bot Grid** - Visual cards showing all bots with stats
3. **Create Bot** - Modal form with personality selection
4. **Edit Bot** - Modify bot configuration
5. **Bulk Actions** - Process, activate, or deactivate all bots
6. **Filters** - Filter by personality, status, or search username

### Creating a Bot

1. Click "Create Bot" button
2. Fill in details:
   - Username (e.g., `bot_warrior_001`)
   - Email (e.g., `bot001@example.com`)
   - Personality Type (8 options)
   - Difficulty Level (1-10 slider)
   - Behavior Parameters (0-100 sliders):
     - Aggression Level
     - Economy Focus
     - Military Focus
     - Research Focus
   - Think Interval (minutes)
3. Click "Save Bot"

### Bot Operations

#### Individual Bot Actions
- **Edit** - Modify bot configuration
- **Activate/Pause** - Toggle bot active status
- **Think** - Force bot to execute decision cycle
- **Delete** - Remove bot permanently

#### Bulk Operations
- **Process All Bots** - Execute think cycle for all active bots
- **Activate All** - Activate all inactive bots
- **Deactivate All** - Pause all active bots

## Bot Personalities

### 1. Aggressive Conqueror
- **Focus:** Military expansion, frequent attacks
- **Parameters:** Aggression 90, Military 85, Economy 30
- **Behavior:** Rapid fleet building, resource plundering

### 2. Strategic Builder
- **Focus:** Infrastructure, balanced development
- **Parameters:** Economy 80, Research 70, Aggression 40
- **Behavior:** Long-term planning, defensive strategies

### 3. Diplomatic Negotiator
- **Focus:** Alliances, trade, peaceful expansion
- **Parameters:** Diplomacy 85, Aggression 20
- **Behavior:** Cooperation over conflict

### 4. Resource Hoarder
- **Focus:** Maximum resource gathering
- **Parameters:** Economy 95, Risk 15
- **Behavior:** Conservative, long-term planning

### 5. Speed Rusher
- **Focus:** Early aggression, rapid tech
- **Parameters:** Aggression 85, Research 80
- **Behavior:** Timing attacks, high-risk

### 6. Tech Enthusiast
- **Focus:** Research, advanced technology
- **Parameters:** Research 95, Economy 55
- **Behavior:** Scientific approach to warfare

### 7. Alliance-Focused
- **Focus:** Team play, coordination
- **Parameters:** Alliance 90, Diplomacy 75
- **Behavior:** Supports allies, resource sharing

### 8. Solo Survivor
- **Focus:** Self-sufficiency, defense
- **Parameters:** Independence 90, Economy 70
- **Behavior:** Minimal diplomacy, strong defenses

## API Endpoints

### List All Bots
```bash
GET /api/admin/bots
Authorization: Bearer {admin_token}

Response: {
  "bots": [...]
}
```

### Create Bot
```bash
POST /api/admin/bots
Authorization: Bearer {admin_token}
Content-Type: application/json

{
  "username": "bot_name",
  "email": "bot@example.com",
  "personality_type": "aggressive_conqueror",
  "difficulty_level": 7,
  "aggression_level": 90,
  "economy_focus": 30,
  "military_focus": 85,
  "research_focus": 40,
  "think_interval_minutes": 15
}
```

### Update Bot
```bash
PUT /api/admin/bots/{id}
Authorization: Bearer {admin_token}
Content-Type: application/json

{
  "is_active": true,
  "difficulty_level": 8
}
```

### Delete Bot
```bash
DELETE /api/admin/bots/{id}
Authorization: Bearer {admin_token}
```

### Force Bot Think
```bash
POST /api/admin/bots/{id}/think
Authorization: Bearer {admin_token}
```

### Process All Bots
```bash
POST /api/admin/bots/process/all
Authorization: Bearer {admin_token}
```

### Get Bot Actions
```bash
GET /api/admin/bots/{id}/actions?limit=50
Authorization: Bearer {admin_token}
```

### Get Personality List
```bash
GET /api/admin/bots/personalities/list
Authorization: Bearer {admin_token}
```

### Bulk Create Bots
```bash
POST /api/admin/bots/bulk
Authorization: Bearer {admin_token}
Content-Type: application/json

{
  "count": 5,
  "personality_type": "aggressive_conqueror",
  "difficulty_range": [3, 7]
}
```

## Database Tables

### bot_profiles
- Bot configuration and statistics
- 34 columns including personality, behavior parameters, metrics

### bot_actions_log
- Complete audit trail of bot decisions
- Stores action type, details, success status, resources

### bot_stats
- Daily aggregated performance metrics
- Economic, military, development statistics

### bot_decision_queue
- Async processing queue for bot decisions
- Priority-based scheduling

### bot_targets
- Target tracking and attack planning
- Threat assessment, resource potential

## Testing Bot Behavior

### Test Scenario 1: Create and Monitor
1. Create a bot with "Aggressive Conqueror" personality
2. Activate the bot
3. Force think cycle (click "Think" button)
4. Check action logs in database
5. Monitor bot statistics updates

### Test Scenario 2: Multiple Personalities
1. Create 8 bots (one per personality type)
2. Activate all
3. Process all bots
4. Compare decision patterns
5. Monitor resource changes

### Test Scenario 3: Difficulty Levels
1. Create 3 bots with same personality
2. Set different difficulty levels (1, 5, 10)
3. Observe decision differences
4. Compare performance metrics

### Database Queries for Testing

```sql
-- View all bots
SELECT * FROM bot_profiles;

-- View bot actions
SELECT * FROM bot_actions_log ORDER BY created_at DESC LIMIT 20;

-- View bot statistics
SELECT * FROM bot_stats WHERE bot_id = 1;

-- View bot leaderboard
SELECT * FROM bot_leaderboard;

-- Count active bots
SELECT COUNT(*) FROM bot_profiles WHERE is_active = true;
```

## Troubleshooting

### PostgreSQL Not Starting
```bash
# Check status
pg_isready -h 127.0.0.1 -p 5432

# View logs
sudo tail -f /var/log/postgresql/postgresql-15-main.log

# Restart
sudo service postgresql restart
```

### Migration Errors
```bash
# Check existing tables
psql -h 127.0.0.1 -U postgres -d ogame_rpg -c "\dt bot*"

# Re-run migration
PGPASSWORD=postgres psql -h 127.0.0.1 -U postgres -d ogame_rpg \
    -f backend/src/database/migrations/005_bot_system.sql
```

### Backend Not Starting
```bash
# Check logs
tail -f backend.log

# Check if port 3000 is in use
lsof -i :3000

# Rebuild
cd backend && npm run build
```

### Bot Actions Not Appearing
1. Check bot is active: `SELECT is_active FROM bot_profiles WHERE id = ?`
2. Check think interval: `SELECT next_think_at FROM bot_profiles WHERE id = ?`
3. Check action logs: `SELECT * FROM bot_actions_log WHERE bot_id = ?`
4. Force think cycle manually via API

## Performance Notes

- **Think Interval:** Default 15 minutes (configurable per bot)
- **Batch Processing:** Up to 10 bots can be created at once
- **Auto-refresh:** UI updates every 30 seconds
- **Database Indexes:** 9 indexes optimize bot queries

## Integration Points

Bot system integrates with:
- User authentication
- Planet management
- Building construction
- Research system
- Fleet operations
- Combat engine
- Resource management
- Alliance system

## Files Reference

### Backend
- `backend/src/database/migrations/005_bot_system.sql` - Database schema
- `backend/src/services/botService.ts` - Bot CRUD operations
- `backend/src/services/botAIService.ts` - AI decision engine
- `backend/src/routes/bots.ts` - API endpoints
- `backend/src/services/gameLoopService.ts` - Bot processing integration

### Frontend
- `frontend/admin/bots.html` - Bot management UI
- `frontend/js/bots.js` - Bot management logic

### Documentation
- `BOT_SYSTEM_COMPLETE.md` - Complete implementation report
- `BOT_SYSTEM_QUICK_REFERENCE.md` - This file
- `deploy-bot-system.sh` - Automated deployment script

## Support

For issues or questions:
1. Check logs: `backend.log`, `migration_output.txt`
2. Verify database tables exist
3. Ensure admin user exists (email: admin@example.com)
4. Check backend is running on port 3000
5. Verify Redis is running for session management
