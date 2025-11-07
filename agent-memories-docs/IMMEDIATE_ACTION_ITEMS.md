# Immediate Action Items - SpaceEmpire RPG

## Critical Priority (Must Complete Before Production)

### 1. Apply Database Migration
**Requirement:** Create millisecond precision combat tracking tables  
**Action:**
```bash
cd /workspace/universus-rpg
docker-compose up -d postgres
docker-compose exec postgres psql -U postgres -d universus_rpg -f /app/database/sql/migrations/003_millisecond_precision_combat.sql
```

**Verification:**
```sql
-- Verify tables were created
\dt fleet_movements_precise
\dt combats_precise
\dt combat_rounds_precise
\dt combat_events_precise
```

---

### 2. Implement Admin API Routes
**Requirement:** Backend endpoints for admin panel (currently using mock data)  
**Files to Create:** `backend/src/routes/admin.ts`

**Required Endpoints:**
```typescript
GET  /api/admin/stats              // Dashboard statistics
GET  /api/admin/users              // User list
GET  /api/admin/users/:id          // User details
POST /api/admin/users/:id/ban      // Ban user
POST /api/admin/users/:id/unban    // Unban user
GET  /api/admin/server-status      // Server metrics
GET  /api/admin/logs               // System logs
GET  /api/admin/database-stats     // Database table stats
GET  /api/admin/settings           // Server settings
PUT  /api/admin/settings           // Update settings
```

**Example Implementation:**
```typescript
// backend/src/routes/admin.ts
import { Router } from 'express';
import { authenticateToken } from '../middleware/auth';
import { pool } from '../config/database';

const router = Router();

// Middleware to verify admin status
const requireAdmin = async (req, res, next) => {
    if (!req.user.is_admin) {
        return res.status(403).json({ error: 'Admin access required' });
    }
    next();
};

// Dashboard stats
router.get('/stats', authenticateToken, requireAdmin, async (req, res) => {
    const stats = await pool.query(`
        SELECT 
            (SELECT COUNT(*) FROM users) as total_users,
            (SELECT COUNT(*) FROM users WHERE last_login > NOW() - INTERVAL '24 hours') as active_players,
            (SELECT COUNT(*) FROM planets) as total_planets
    `);
    res.json(stats.rows[0]);
});

// ... implement other endpoints

export default router;
```

**Register Routes in index.ts:**
```typescript
import adminRoutes from './routes/admin';
app.use('/api/admin', adminRoutes);
```

---

### 3. Test New UI Features
**Requirement:** Verify all new pages work correctly  

**Start Services:**
```bash
cd /workspace/universus-rpg
docker-compose up -d
```

**Manual Testing Checklist:**

#### Leaderboard (`http://localhost:3000/leaderboard.html`)
- [ ] Page loads without errors
- [ ] Player rankings display correctly
- [ ] Alliance rankings work
- [ ] Pagination functions
- [ ] Personal rank shows correctly
- [ ] Real-time updates work (if Socket.io connected)

#### Messages (`http://localhost:3000/messages.html`)
- [ ] Inbox loads
- [ ] Can compose new message
- [ ] Messages display in correct folders
- [ ] Can reply to messages
- [ ] Can delete messages
- [ ] Unread count updates

#### Admin Panel (`http://localhost:3000/admin.html`)
- [ ] Requires admin authentication
- [ ] Dashboard displays (with mock data until API implemented)
- [ ] All 6 sections accessible
- [ ] Navigation works smoothly

**Automated Testing:**
```bash
# Using test_website tool (if available)
# Test leaderboard
test_website http://localhost:3000/leaderboard.html "Verify the leaderboard page loads and displays player rankings"

# Test messages
test_website http://localhost:3000/messages.html "Check that messages inbox loads and displays folders"

# Test admin panel
test_website http://localhost:3000/admin.html "Verify admin panel loads and shows dashboard"
```

---

### 4. Run Backend Test Suite
**Requirement:** Verify test coverage meets 70% threshold  

**Action:**
```bash
cd /workspace/universus-rpg/backend
npm test -- --coverage --forceExit
```

**Expected Output:**
```
Test Suites: 2 passed, 2 total
Tests:       50+ passed, 50+ total
Coverage:    70%+ statements, branches, lines, functions
```

**If Coverage < 70%:**
1. Identify uncovered code with coverage report
2. Write additional tests for critical paths
3. Focus on services (combatService, fleetService, etc.)

---

### 5. Add Admin User to Database
**Requirement:** Grant admin access to at least one user  

**Action:**
```sql
-- Connect to database
docker-compose exec postgres psql -U postgres -d universus_rpg

-- Add is_admin column if not exists
ALTER TABLE users ADD COLUMN IF NOT EXISTS is_admin BOOLEAN DEFAULT false;

-- Grant admin access to specific user
UPDATE users SET is_admin = true WHERE username = 'your_username';

-- Or grant to user ID
UPDATE users SET is_admin = true WHERE id = 1;

-- Verify
SELECT id, username, email, is_admin FROM users WHERE is_admin = true;
```

---

## High Priority (Complete Within Next Session)

### 6. Create Backend Admin Service
**Requirement:** Centralize admin logic  
**File:** `backend/src/services/adminService.ts`

```typescript
import { pool } from '../config/database';
import os from 'os';

export class AdminService {
    static async getDashboardStats() {
        const userStats = await pool.query(`
            SELECT 
                COUNT(*) as total,
                COUNT(CASE WHEN last_login > NOW() - INTERVAL '24 hours' THEN 1 END) as active_24h,
                COUNT(CASE WHEN created_at::date = CURRENT_DATE THEN 1 END) as today
            FROM users
        `);

        const planetCount = await pool.query('SELECT COUNT(*) FROM planets');
        const combatCount = await pool.query("SELECT COUNT(*) FROM combats_precise WHERE status = 'in_progress'");

        return {
            totalUsers: parseInt(userStats.rows[0].total),
            activePlayers: parseInt(userStats.rows[0].active_24h),
            usersToday: parseInt(userStats.rows[0].today),
            totalPlanets: parseInt(planetCount.rows[0].count),
            activeCombats: parseInt(combatCount.rows[0].count),
            serverUptime: Math.floor(process.uptime() / 3600),
            dbSize: await this.getDatabaseSize()
        };
    }

    static async getDatabaseSize(): Promise<number> {
        const result = await pool.query(`
            SELECT pg_database_size(current_database()) / 1024 / 1024 as size_mb
        `);
        return Math.round(result.rows[0].size_mb);
    }

    static async getServerStatus() {
        return {
            cpu: Math.round(os.loadavg()[0] * 100),
            memory: Math.round(process.memoryUsage().heapUsed / 1024 / 1024),
            connections: 0, // Implement connection pool stats
            requestsPerMin: 0, // Implement request counter
            services: [
                { name: 'PostgreSQL', status: 'running', uptime: Math.floor(process.uptime() / 3600) },
                { name: 'Redis', status: 'running', uptime: Math.floor(process.uptime() / 3600) },
                { name: 'WebSocket', status: 'running', uptime: Math.floor(process.uptime() / 3600) }
            ]
        };
    }

    static async getUsers(filter: string = 'all') {
        let query = 'SELECT id, username, email, created_at, last_login, is_admin FROM users';
        
        if (filter === 'admin') {
            query += ' WHERE is_admin = true';
        } else if (filter === 'banned') {
            query += ' WHERE is_banned = true'; // Add is_banned column if needed
        }
        
        query += ' ORDER BY created_at DESC LIMIT 1000';
        
        const result = await pool.query(query);
        return result.rows;
    }

    static async getUserDetails(userId: number) {
        const user = await pool.query(`
            SELECT u.*, 
                COUNT(DISTINCT p.id) as planet_count,
                COALESCE(SUM(p.total_score), 0) as total_score
            FROM users u
            LEFT JOIN planets p ON u.id = p.user_id
            WHERE u.id = $1
            GROUP BY u.id
        `, [userId]);

        return user.rows[0];
    }

    static async banUser(userId: number): Promise<void> {
        await pool.query('UPDATE users SET is_banned = true WHERE id = $1', [userId]);
    }

    static async unbanUser(userId: number): Promise<void> {
        await pool.query('UPDATE users SET is_banned = false WHERE id = $1', [userId]);
    }
}
```

---

### 7. Enhance Database Schema
**Requirement:** Add missing columns for admin features  

```sql
-- Add admin-related columns
ALTER TABLE users ADD COLUMN IF NOT EXISTS is_admin BOOLEAN DEFAULT false;
ALTER TABLE users ADD COLUMN IF NOT EXISTS is_banned BOOLEAN DEFAULT false;
ALTER TABLE users ADD COLUMN IF NOT EXISTS ban_reason TEXT;
ALTER TABLE users ADD COLUMN IF NOT EXISTS banned_at TIMESTAMP;
ALTER TABLE users ADD COLUMN IF NOT EXISTS last_login TIMESTAMP;

-- Create indexes for admin queries
CREATE INDEX IF NOT EXISTS idx_users_admin ON users(is_admin) WHERE is_admin = true;
CREATE INDEX IF NOT EXISTS idx_users_banned ON users(is_banned) WHERE is_banned = true;
CREATE INDEX IF NOT EXISTS idx_users_last_login ON users(last_login DESC);
```

---

### 8. Implement System Logging
**Requirement:** Store and retrieve system logs  

**Create Log Table:**
```sql
CREATE TABLE IF NOT EXISTS system_logs (
    id SERIAL PRIMARY KEY,
    level VARCHAR(20) NOT NULL,
    message TEXT NOT NULL,
    metadata JSONB,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_system_logs_level ON system_logs(level, created_at DESC);
CREATE INDEX idx_system_logs_created ON system_logs(created_at DESC);
```

**Logger Service:**
```typescript
// backend/src/services/loggerService.ts
import { pool } from '../config/database';

export class LoggerService {
    static async log(level: 'info' | 'warn' | 'error', message: string, metadata?: any) {
        await pool.query(
            'INSERT INTO system_logs (level, message, metadata) VALUES ($1, $2, $3)',
            [level, message, JSON.stringify(metadata || {})]
        );
        console.log(`[${level.toUpperCase()}] ${message}`, metadata);
    }

    static async getLogs(level?: string, limit: number = 100) {
        let query = 'SELECT * FROM system_logs';
        const params: any[] = [];

        if (level && level !== 'all') {
            query += ' WHERE level = $1';
            params.push(level);
        }

        query += ' ORDER BY created_at DESC LIMIT $' + (params.length + 1);
        params.push(limit);

        const result = await pool.query(query, params);
        return result.rows;
    }
}
```

---

## Medium Priority (Complete Before Production Launch)

### 9. Frontend TypeScript Conversion
Convert remaining JavaScript files to TypeScript:
- `frontend/js/game.js` → `game.ts`
- `frontend/js/api.js` → `api.ts`
- `frontend/js/overview.js` → `overview.ts`
- `frontend/js/buildings.js` → `buildings.ts`
- `frontend/js/fleet.js` → `fleet.ts`
- `frontend/js/shipyard.js` → `shipyard.ts`
- `frontend/js/research.js` → `research.ts`

### 10. Write Integration Tests
Create integration tests for API endpoints:
- Auth flow (register, login, logout)
- Planet operations (create, update, query)
- Fleet operations (create, dispatch, arrival)
- Combat simulation
- Leaderboard updates
- Message sending/receiving

### 11. Performance Optimization
- Implement Redis caching for frequently accessed data
- Optimize database queries (add indexes, use EXPLAIN ANALYZE)
- Minify and compress frontend assets
- Implement CDN for static assets
- Add database connection pooling configuration

### 12. Security Audit
- Implement rate limiting on all endpoints
- Add CAPTCHA to registration
- Verify SQL injection prevention
- Check XSS protection
- Implement CSRF tokens
- Set up security headers (helmet.js)

---

## Quick Start Command Summary

```bash
# 1. Start all services
cd /workspace/universus-rpg
docker-compose up -d

# 2. Apply migration
docker-compose exec postgres psql -U postgres -d universus_rpg -f /app/database/sql/migrations/003_millisecond_precision_combat.sql

# 3. Grant admin access
docker-compose exec postgres psql -U postgres -d universus_rpg -c "ALTER TABLE users ADD COLUMN IF NOT EXISTS is_admin BOOLEAN DEFAULT false; UPDATE users SET is_admin = true WHERE id = 1;"

# 4. Run tests
cd backend && npm test -- --coverage

# 5. Check services
docker-compose ps
curl http://localhost:3000/api/health

# 6. Test new pages
# Open in browser:
# - http://localhost:3000/leaderboard.html
# - http://localhost:3000/messages.html
# - http://localhost:3000/admin.html
```

---

## Completion Checklist

Use this checklist to track completion of critical items:

### Critical (Session 3 Completion)
- [ ] Database migration applied
- [ ] Admin API routes implemented
- [ ] Admin user created in database
- [ ] New UI features manually tested
- [ ] Backend test suite runs successfully
- [ ] Test coverage verified (70%+)

### High Priority (Session 4)
- [ ] Admin service created
- [ ] System logging implemented
- [ ] Database schema enhanced (admin columns)
- [ ] All admin panel features functional

### Medium Priority (Pre-Production)
- [ ] Frontend TypeScript conversion
- [ ] Integration tests written
- [ ] E2E tests implemented
- [ ] Performance optimization completed
- [ ] Security audit passed

### Pre-Launch
- [ ] SSL/TLS configured
- [ ] Production environment variables set
- [ ] Monitoring and alerting configured
- [ ] Backup strategy implemented
- [ ] Load testing completed
- [ ] Documentation finalized

---

**Status:** 85% Complete  
**Remaining Work:** ~15% (primarily testing, API completion, optimization)  
**Estimated Time to Production:** 2-3 additional sessions
