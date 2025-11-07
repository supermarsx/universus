# SpaceEmpire RPG - Verification and Testing Guide

**Created:** 2025-11-06  
**Purpose:** Complete verification of all new features and database migrations  
**Target Completion:** 100% production readiness

## Overview

This guide provides step-by-step instructions to verify and test all newly implemented features:
- ✅ Database migrations (003 & 004)
- ✅ Admin panel with full backend API
- ✅ Leaderboard system with real-time updates
- ✅ Messages/Inbox system
- ✅ AI-powered planet image generator
- ✅ Millisecond precision combat tracking

## Prerequisites

- Docker and Docker Compose installed
- PostgreSQL client (for direct database queries)
- Modern web browser (Chrome, Firefox, Safari)
- curl or Postman for API testing

---

## Phase 1: Start Services & Apply Migrations

### Step 1.1: Start Docker Containers

```bash
cd /workspace/ogame-rpg
docker-compose up -d
```

**Verification:**
```bash
docker-compose ps
```

Expected output: All services (postgres, redis, backend) should be "Up" and healthy.

### Step 1.2: Wait for Services to Initialize

```bash
# Wait 10 seconds for PostgreSQL to be ready
sleep 10

# Check PostgreSQL connection
docker-compose exec postgres pg_isready -U postgres
```

Expected output: `postgres:5432 - accepting connections`

### Step 1.3: Apply Migration 003 (Millisecond Precision Combat)

```bash
docker-compose exec postgres psql -U postgres -d ogame_rpg -f /app/backend/src/database/migrations/003_millisecond_precision_combat.sql
```

**Expected output:**
```
CREATE TABLE
CREATE TABLE
CREATE TABLE
CREATE TABLE
CREATE INDEX
CREATE INDEX
CREATE INDEX
CREATE INDEX
```

**Verification:**
```bash
docker-compose exec postgres psql -U postgres -d ogame_rpg -c "\dt *precise*"
```

Should show 4 tables:
- `fleet_movements_precise`
- `combats_precise`
- `combat_rounds_precise`
- `combat_events_precise`

### Step 1.4: Apply Migration 004 (Admin Features)

```bash
docker-compose exec postgres psql -U postgres -d ogame_rpg -f /app/backend/src/database/migrations/004_admin_features.sql
```

**Expected output:**
```
ALTER TABLE
CREATE TABLE
CREATE INDEX
CREATE INDEX
```

**Verification:**
```bash
docker-compose exec postgres psql -U postgres -d ogame_rpg -c "\d users"
```

Should show `is_admin` column (boolean, default false).

```bash
docker-compose exec postgres psql -U postgres -d ogame_rpg -c "\dt admin*"
```

Should show `admin_audit_log` table.

### Step 1.5: Create Admin User

```bash
# Create or update the first user to be an admin
docker-compose exec postgres psql -U postgres -d ogame_rpg -c "UPDATE users SET is_admin = true WHERE id = 1;"
```

**Verification:**
```bash
docker-compose exec postgres psql -U postgres -d ogame_rpg -c "SELECT id, username, email, is_admin FROM users WHERE is_admin = true;"
```

Should show at least one admin user.

---

## Phase 2: Backend Verification

### Step 2.1: Verify TypeScript Compilation

```bash
cd backend
pnpm run build
```

**Expected output:** Should complete with "Compiled successfully" and no errors.

**Troubleshooting:** If errors occur, run:
```bash
pnpm run type-check
```

### Step 2.2: Run Test Suite

```bash
pnpm run test
```

**Expected results:**
- All test suites pass
- Coverage >70% (target: 75%+)
- No critical failures

**Coverage report location:** `backend/coverage/lcov-report/index.html`

### Step 2.3: Test Health Endpoint

```bash
curl http://localhost:3000/api/health
```

**Expected response:**
```json
{
  "status": "ok",
  "timestamp": "2025-11-06T02:07:50.000Z"
}
```

### Step 2.4: Test Admin API Endpoints

**Get server statistics (requires admin auth):**
```bash
# First, login to get JWT token
TOKEN=$(curl -X POST http://localhost:3000/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"email":"admin@example.com","password":"adminpass"}' \
  | jq -r '.token')

# Test admin stats endpoint
curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:3000/api/admin/stats
```

**Expected response:**
```json
{
  "success": true,
  "data": {
    "totalUsers": 1,
    "activeUsers": 1,
    "totalPlanets": 1,
    "totalFleets": 0,
    "totalAlliances": 0,
    "serverUptime": "2h 15m"
  }
}
```

**Test other admin endpoints:**
```bash
# Get all users
curl -H "Authorization: Bearer $TOKEN" http://localhost:3000/api/admin/users

# Get server status
curl -H "Authorization: Bearer $TOKEN" http://localhost:3000/api/admin/server-status

# Get database stats
curl -H "Authorization: Bearer $TOKEN" http://localhost:3000/api/admin/database-stats
```

---

## Phase 3: Frontend UI Testing

### Step 3.1: Test Leaderboard Page

1. Open browser: `http://localhost:3000/leaderboard.html`

**Verification checklist:**
- [ ] Page loads without errors
- [ ] Player leaderboard displays with rankings
- [ ] Alliance leaderboard tab switches correctly
- [ ] Personal stats section shows (if logged in)
- [ ] Pagination works (if >10 players)
- [ ] Real-time updates work (Socket.io connected)
- [ ] No console errors in browser DevTools

**Console verification:**
```javascript
// Open browser DevTools console
console.log("Socket connected:", window.socket?.connected);
```

### Step 3.2: Test Messages/Inbox Page

1. Open browser: `http://localhost:3000/messages.html`

**Verification checklist:**
- [ ] Page loads without errors
- [ ] Inbox folder displays messages
- [ ] Sent folder shows sent messages
- [ ] Combat reports render correctly (if any)
- [ ] Compose modal opens and closes
- [ ] Send message functionality works
- [ ] Reply to message works
- [ ] Message deletion works
- [ ] Real-time notifications work
- [ ] Unread count updates correctly
- [ ] No console errors

**Test sending a message:**
1. Click "Compose Message"
2. Enter recipient, subject, content
3. Click "Send"
4. Verify message appears in "Sent" folder
5. Recipient should see it in inbox (if testing with 2 accounts)

### Step 3.3: Test Admin Panel

1. Login with admin account
2. Open browser: `http://localhost:3000/admin.html`

**Verification checklist:**
- [ ] Page loads (or redirects if not admin)
- [ ] Dashboard shows real-time statistics
- [ ] User management section lists all users
- [ ] Ban/Unban user buttons work
- [ ] Server status shows metrics
- [ ] System logs display correctly
- [ ] Database statistics show table sizes
- [ ] Settings management loads
- [ ] Auto-refresh works (30-second intervals)
- [ ] No console errors

**Test user management:**
1. Navigate to "User Management" tab
2. Click "View Details" on a user
3. Click "Ban User" (or "Unban" if already banned)
4. Verify ban status updates in database:
```bash
docker-compose exec postgres psql -U postgres -d ogame_rpg -c "SELECT id, username, is_banned FROM users WHERE id = 2;"
```

### Step 3.4: Test AI Planet Image Generator

1. Open browser: `http://localhost:3000/galaxy.html`
2. Navigate to any galaxy/system

**Verification checklist:**
- [ ] Planet images generate and display
- [ ] Images are unique per position
- [ ] 7 different planet types render correctly:
  - [ ] Terrestrial (with continents)
  - [ ] Gas Giant (with bands)
  - [ ] Ice World (with ice cracks)
  - [ ] Desert (sandy with dunes)
  - [ ] Lava (red with lava flows)
  - [ ] Metal (metallic sheen)
  - [ ] Artificial (hexagonal patterns)
- [ ] Rings display on gas giants
- [ ] Same coordinates always produce same image
- [ ] No console errors

**Test deterministic generation:**
```javascript
// In browser console
const img1 = planetImageGenerator.generatePlanetImage(1, 1, 1);
const img2 = planetImageGenerator.generatePlanetImage(1, 1, 1);
console.log("Images match:", img1.src === img2.src);
```

### Step 3.5: Test Millisecond Combat Tracking

**Create a fleet and initiate combat:**
1. Go to shipyard, build some ships
2. Go to fleet page, dispatch attack mission
3. Wait for combat to occur
4. Check combat report in messages

**Verify combat tracking in database:**
```bash
docker-compose exec postgres psql -U postgres -d ogame_rpg -c "SELECT * FROM combats_precise ORDER BY created_at DESC LIMIT 1;"
```

Should show microsecond precision timestamps.

```bash
docker-compose exec postgres psql -U postgres -d ogame_rpg -c "SELECT * FROM combat_events_precise WHERE combat_id = <COMBAT_ID> ORDER BY event_timestamp;"
```

Should show detailed combat events with microsecond timing.

---

## Phase 4: Integration Testing

### Step 4.1: End-to-End User Flow

**Test complete user journey:**
1. Register new account
2. Login
3. View overview page
4. Build buildings
5. Research technologies
6. Build ships in shipyard
7. Dispatch fleet
8. View galaxy
9. Check leaderboard position
10. Send message to another player
11. View combat report (if combat occurred)
12. Check shop for purchases

**Verification:** All features work seamlessly without errors.

### Step 4.2: Real-time Updates Test

**Open 2 browser windows (side-by-side):**
1. Window 1: Login as User A, go to overview
2. Window 2: Login as User B, go to overview
3. User A: Send message to User B
4. User B: Should see notification instantly
5. User A: Start building
6. User B: Check leaderboard, should see User A's score update

**Verification:** Socket.io real-time updates work across sessions.

### Step 4.3: Admin Actions Test

**Admin performs actions:**
1. Login as admin
2. Go to admin panel
3. Ban a user
4. Check audit log:
```bash
docker-compose exec postgres psql -U postgres -d ogame_rpg -c "SELECT * FROM admin_audit_log ORDER BY performed_at DESC LIMIT 5;"
```

**Verification:** All admin actions are logged with timestamps.

---

## Phase 5: Performance Testing

### Step 5.1: Load Testing

**Test concurrent users:**
```bash
# Install artillery (if not installed)
npm install -g artillery

# Create load test config
cat > artillery-config.yml << 'EOF'
config:
  target: 'http://localhost:3000'
  phases:
    - duration: 60
      arrivalRate: 10
      name: "Warm up"
    - duration: 120
      arrivalRate: 50
      name: "Load test"
scenarios:
  - flow:
    - get:
        url: "/api/health"
    - post:
        url: "/api/auth/login"
        json:
          email: "test@example.com"
          password: "testpass"
EOF

# Run test
artillery run artillery-config.yml
```

**Expected results:**
- Response times <500ms (p95)
- No 500 errors
- Successful connection rate >99%

### Step 5.2: Database Query Performance

```bash
# Check slow queries
docker-compose exec postgres psql -U postgres -d ogame_rpg -c "
SELECT query, calls, mean_exec_time, max_exec_time 
FROM pg_stat_statements 
WHERE mean_exec_time > 100 
ORDER BY mean_exec_time DESC 
LIMIT 10;"
```

**Verification:** No queries with mean execution time >1000ms.

---

## Phase 6: Security Verification

### Step 6.1: Test Authentication

**Try accessing admin endpoint without token:**
```bash
curl http://localhost:3000/api/admin/stats
```

**Expected response:** 401 Unauthorized

**Try accessing with non-admin token:**
```bash
# Login as regular user
TOKEN=$(curl -X POST http://localhost:3000/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"email":"user@example.com","password":"userpass"}' \
  | jq -r '.token')

curl -H "Authorization: Bearer $TOKEN" http://localhost:3000/api/admin/stats
```

**Expected response:** 403 Forbidden

### Step 6.2: SQL Injection Test

**Test input sanitization:**
```bash
curl -X POST http://localhost:3000/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"email":"admin@example.com OR 1=1--","password":"anything"}'
```

**Expected response:** Login failure (not SQL error).

### Step 6.3: XSS Protection Test

**Test message XSS:**
1. Login to messages page
2. Send message with content: `<script>alert('XSS')</script>`
3. View message in inbox

**Verification:** Script should be displayed as text, not executed.

---

## Phase 7: Final Checklist

### Code Quality
- [ ] TypeScript compiles with 0 errors
- [ ] ESLint passes with 0 errors
- [ ] Prettier formatting applied
- [ ] Test coverage >70%
- [ ] All tests pass

### Database
- [ ] Migration 003 applied successfully
- [ ] Migration 004 applied successfully
- [ ] At least 1 admin user exists
- [ ] All tables have proper indexes
- [ ] Foreign keys properly configured

### Backend API
- [ ] Health endpoint responds
- [ ] All admin endpoints work
- [ ] Authentication middleware works
- [ ] Admin authorization works
- [ ] Audit logging works
- [ ] Error handling works

### Frontend UI
- [ ] Leaderboard page works
- [ ] Messages page works
- [ ] Admin panel works
- [ ] Planet images generate
- [ ] Real-time updates work
- [ ] No console errors

### Integration
- [ ] User registration works
- [ ] Login/logout works
- [ ] All gameplay features work
- [ ] Socket.io connections stable
- [ ] Cross-page navigation works

### Performance
- [ ] Page load <3 seconds
- [ ] API response <500ms (p95)
- [ ] No memory leaks
- [ ] Database queries optimized

### Security
- [ ] Authentication required for protected routes
- [ ] Admin authorization enforced
- [ ] SQL injection prevented
- [ ] XSS protection works
- [ ] CORS configured properly

---

## Troubleshooting

### Issue: Migrations fail with "relation already exists"

**Solution:**
```bash
# Drop and recreate specific tables
docker-compose exec postgres psql -U postgres -d ogame_rpg -c "
DROP TABLE IF EXISTS combat_events_precise CASCADE;
DROP TABLE IF EXISTS combat_rounds_precise CASCADE;
DROP TABLE IF EXISTS combats_precise CASCADE;
DROP TABLE IF EXISTS fleet_movements_precise CASCADE;
"

# Re-run migration 003
docker-compose exec postgres psql -U postgres -d ogame_rpg -f /app/backend/src/database/migrations/003_millisecond_precision_combat.sql
```

### Issue: Admin panel shows 403 Forbidden

**Solution:**
```bash
# Verify user is admin
docker-compose exec postgres psql -U postgres -d ogame_rpg -c "SELECT id, username, is_admin FROM users WHERE id = 1;"

# Set admin flag if false
docker-compose exec postgres psql -U postgres -d ogame_rpg -c "UPDATE users SET is_admin = true WHERE id = 1;"
```

### Issue: Socket.io not connecting

**Solution:**
1. Check browser console for WebSocket errors
2. Verify backend is running: `docker-compose logs backend`
3. Check CORS configuration in backend/src/index.ts
4. Restart services: `docker-compose restart`

### Issue: Planet images not showing

**Solution:**
1. Check browser console for JavaScript errors
2. Verify planetImageGenerator.js is loaded:
   - Open galaxy.html source
   - Verify `<script src="js/planetImageGenerator.js"></script>` exists
3. Clear browser cache and reload

### Issue: Tests fail

**Solution:**
```bash
# Clear coverage and node_modules
cd backend
rm -rf coverage node_modules

# Reinstall dependencies
pnpm install

# Run tests with verbose output
pnpm run test -- --verbose
```

---

## Success Criteria

✅ **All migrations applied successfully**  
✅ **TypeScript compiles with 0 errors**  
✅ **Test suite passes with >70% coverage**  
✅ **All UI pages load without errors**  
✅ **Real-time features work (Socket.io)**  
✅ **Admin panel accessible and functional**  
✅ **Combat tracking works with microsecond precision**  
✅ **Planet generator creates unique images**  
✅ **Security measures in place and tested**  
✅ **Performance meets targets (<500ms API, <3s page load)**

---

## Next Steps After Verification

1. **Create production build:**
   ```bash
   cd backend
   pnpm run build
   ```

2. **Run E2E tests** (if implemented):
   ```bash
   pnpm run test:e2e
   ```

3. **Deploy to production environment**

4. **Set up monitoring and logging**

5. **Create backup strategy**

6. **Document API for external integrators**

---

## Support

For issues or questions:
1. Check logs: `docker-compose logs backend`
2. Check database: `docker-compose exec postgres psql -U postgres -d ogame_rpg`
3. Review code in `/workspace/ogame-rpg/backend/src/`
4. Check frontend console errors in browser DevTools

**Project Status:** 90% complete → Target: 100% production ready
**Last Updated:** 2025-11-06 02:07:50
