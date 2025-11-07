# PHASE 11: DEPLOYMENT AND TESTING GUIDE

## Critical Status: Infrastructure Required

**Environment Limitation:** The current sandbox does not have PostgreSQL or Redis installed/running.
**Impact:** Cannot deploy database schema or test backend services in this environment.
**Solution:** This guide provides complete deployment steps for production infrastructure.

---

## Prerequisites Checklist

### Required Services:
- [ ] PostgreSQL 15+ installed and running
- [ ] Redis 7+ installed and running
- [ ] Node.js 18+ installed
- [ ] npm or pnpm package manager

### Required Credentials:
- [ ] PostgreSQL connection string
- [ ] Redis connection string
- [ ] JWT secret key for authentication
- [ ] Stripe API keys (for Phase 10 testing)

### Environment Variables (.env file):
```bash
# Database
DATABASE_URL=postgresql://user:password@localhost:5432/universus
REDIS_URL=redis://localhost:6379

# Authentication
JWT_SECRET=your-secure-jwt-secret-key-here
JWT_EXPIRES_IN=7d

# Server
PORT=3000
NODE_ENV=production

# Stripe (Phase 10)
STRIPE_SECRET_KEY=sk_test_...
STRIPE_WEBHOOK_SECRET=whsec_...
STRIPE_PUBLISHABLE_KEY=pk_test_...

# CORS
ALLOWED_ORIGINS=http://localhost:3000,https://your-domain.com
```

---

## Phase 11: Database Schema Deployment

### Step 1: Locate Schema File
File location: `/workspace/universus-rpg/database/sql/phase11_alliance_management_schema.sql`

### Step 2: Deploy to PostgreSQL

**Option A: Using psql command line**
```bash
cd /workspace/universus-rpg
psql postgresql://user:password@localhost:5432/universus -f database/sql/phase11_alliance_management_schema.sql
```

**Option B: Using Node.js script**
```bash
cd /workspace/universus-rpg/backend
node -e "
const { Pool } = require('pg');
const fs = require('fs');
const pool = new Pool({ connectionString: process.env.DATABASE_URL });

async function deploy() {
  const sql = fs.readFileSync('database/sql/phase11_alliance_management_schema.sql', 'utf8');
  try {
    await pool.query(sql);
    console.log('✅ Phase 11 schema deployed successfully');
  } catch (error) {
    console.error('❌ Deployment failed:', error.message);
    process.exit(1);
  } finally {
    await pool.end();
  }
}

deploy();
"
```

### Step 3: Verify Schema Deployment

```sql
-- Check tables were created (should return 22 tables)
SELECT COUNT(*) FROM information_schema.tables 
WHERE table_schema = 'public' 
AND table_name LIKE 'alliance%' OR table_name LIKE 'war_%' OR table_name LIKE 'diplomatic_%';

-- Verify specific tables exist
SELECT table_name FROM information_schema.tables 
WHERE table_schema = 'public' 
AND (
  table_name LIKE 'alliance%' OR 
  table_name LIKE 'war_%' OR 
  table_name LIKE 'diplomatic_%'
)
ORDER BY table_name;

-- Expected tables:
-- alliances
-- alliance_members
-- alliance_rank_permissions
-- alliance_applications
-- alliance_invitations
-- alliance_wars
-- war_battles
-- war_participants
-- diplomatic_relations
-- diplomatic_proposals
-- alliance_contributions
-- alliance_research
-- alliance_territories
-- territory_control_log
-- alliance_messages
-- alliance_message_reactions
-- alliance_events
-- alliance_event_participation
-- alliance_achievements
-- alliance_history
-- v_alliance_leaderboard (view)
-- v_alliance_member_activity (view)
-- v_active_wars_summary (view)

-- Check indexes were created
SELECT indexname FROM pg_indexes 
WHERE tablename LIKE 'alliance%' OR tablename LIKE 'war_%' OR tablename LIKE 'diplomatic_%'
ORDER BY indexname;

-- Check functions were created
SELECT routine_name FROM information_schema.routines 
WHERE routine_schema = 'public' 
AND routine_name LIKE '%alliance%'
ORDER BY routine_name;
```

---

## Backend Server Deployment

### Step 1: Install Dependencies
```bash
cd /workspace/universus-rpg/backend
npm install
# or
pnpm install
```

### Step 2: Compile TypeScript
```bash
npm run build
# Expected: dist/ directory created with compiled JavaScript
```

### Step 3: Verify Compilation
```bash
# Check for compilation errors
npm run build 2>&1 | grep -i error

# Expected output: No errors, exit code 0
```

### Step 4: Start Backend Server
```bash
# Development mode
npm run dev

# Production mode
npm start

# Background mode (production)
npm start &

# With PM2 (recommended for production)
pm2 start dist/index.js --name "universus-backend"
```

### Step 5: Verify Server is Running
```bash
# Check process
ps aux | grep node | grep universus

# Test health endpoint
curl http://localhost:3000/api/health

# Expected response:
# {"status":"ok","timestamp":"2025-11-07T00:34:21.000Z"}

# Test alliance endpoint (requires authentication)
curl http://localhost:3000/api/alliances/leaderboard/rankings

# Expected: JSON response or 401 Unauthorized (which proves endpoint exists)
```

---

## Phase 11: Frontend Testing Checklist

### Test Environment Setup:
1. Ensure backend server is running
2. Ensure PostgreSQL and Redis are accessible
3. Create test user accounts (admin, alliance founder, regular member)
4. Open browser to http://localhost:3000

### Test 1: Alliance Dashboard (/alliance/dashboard)

**Preconditions:**
- User is logged in
- User is NOT in an alliance

**Test Steps:**
1. Navigate to /alliance/dashboard
2. Verify "No Alliance State" displays correctly
3. Click "Create Alliance" button
4. Fill out form:
   - Tag: TEST (3-6 uppercase characters)
   - Name: Test Alliance
   - Description: Test alliance for Phase 11
5. Submit form
6. **Expected:** Alliance created successfully, redirected to dashboard
7. Verify:
   - Alliance header shows tag, name, description
   - Statistics cards display (6 cards with zero values initially)
   - Members list shows founder (you)
   - Activity feed shows "Alliance created" event
   - Quick actions show "Invite Member", "Make Announcement", "Manage" buttons

**Test Cases:**
- [ ] Create alliance form validation (tag format, name length)
- [ ] Alliance statistics display correctly
- [ ] Member search and filtering works
- [ ] Activity feed updates in real-time (Socket.io)
- [ ] Invite member modal opens and submits
- [ ] Announcement modal opens and submits
- [ ] Member cards show correct role badges
- [ ] Online/offline status indicators work
- [ ] Responsive design on mobile (resize browser)

### Test 2: Alliance Wars (/alliance/wars)

**Preconditions:**
- User is in an alliance
- User has "DECLARE_WAR" permission (Founder/Leader/Officer)

**Test Steps:**
1. Navigate to /alliance/wars
2. Verify page displays 3 sections: Active Wars, Declare War, War History
3. Click "Declare War" button
4. Fill out form:
   - Target Alliance: Enter another alliance tag
   - War Type: Select "Conquest"
   - Objective: Select "Domination"
   - Terms: Enter war declaration terms
5. Submit declaration
6. **Expected:** War proposal sent, shows in "Pending Wars"
7. (As target alliance leader) Accept war
8. **Expected:** War becomes "Active"
9. Record a battle:
   - Battle Type: "Major Battle"
   - Outcome: "Victory"
   - Attacker Ships Lost: 100
   - Defender Ships Lost: 150
10. **Expected:** Battle recorded, war score updated
11. Verify:
    - War cards show correct status
    - Battle timeline displays all battles
    - War statistics update (score, battles count)
    - Real-time updates work (Socket.io)

**Test Cases:**
- [ ] War declaration form validation
- [ ] War status displays correctly (Pending, Active, Completed)
- [ ] Battle recording updates scores
- [ ] Peace proposal workflow
- [ ] War history filtering
- [ ] Real-time war updates (Socket.io)
- [ ] War details modal shows complete information
- [ ] Responsive design on mobile

### Test 3: Alliance Diplomacy (/alliance/diplomacy)

**Preconditions:**
- User is in an alliance
- User has "MANAGE_DIPLOMACY" permission

**Test Steps:**
1. Navigate to /alliance/diplomacy
2. Verify page displays 3 sections: Current Relations, Pending Proposals, History
3. Click "Propose Treaty" button
4. Fill out form:
   - Target Alliance: Enter another alliance tag
   - Relation Type: Select "NAP" (Non-Aggression Pact)
   - Duration: 30 days
   - Terms: Enter treaty terms
5. Submit proposal
6. **Expected:** Proposal sent, shows in "Pending Proposals" (Outgoing)
7. (As target alliance leader) View incoming proposal
8. Accept proposal
9. **Expected:** Treaty established, shows in "Current Relations"
10. Verify:
    - Relation cards show correct type with color coding
    - Treaty expiration date displays
    - Diplomatic history timeline updates
    - Real-time updates work (Socket.io)

**Test Cases:**
- [ ] Treaty proposal form validation
- [ ] Relation type colors display correctly (7 types)
- [ ] Proposal acceptance/rejection workflow
- [ ] Treaty breaking with confirmation
- [ ] Diplomatic history timeline
- [ ] Real-time diplomatic updates (Socket.io)
- [ ] Relation details modal
- [ ] Filter relations by type
- [ ] Responsive design on mobile

### Test 4: Alliance Management (/alliance/manage)

**Preconditions:**
- User is in an alliance
- User is Founder or Leader
- User has "EDIT_SETTINGS" permission

**Test Steps:**
1. Navigate to /alliance/manage
2. Verify 4 tabs display: Settings, Ranks, Treasury, Members

**Settings Tab:**
3. Update alliance settings:
   - Change name (test validation: 4-50 characters)
   - Update description
   - Change join type to "Approval Required"
   - Set minimum rank requirement: 100
   - Toggle public visibility
4. Submit form
5. **Expected:** Settings updated successfully
6. Verify changes reflected on alliance dashboard

**Ranks Tab:**
7. Click "Create Custom Rank"
8. Fill out form:
   - Rank Name: "Elite Officer"
   - Permissions: Select 4-5 permissions
9. Submit
10. **Expected:** Rank created, displays in ranks list
11. Edit rank (change permissions)
12. Delete rank
13. **Expected:** Confirmation modal, rank deleted

**Treasury Tab:**
14. Verify treasury displays resource totals (Metal, Crystal, Deuterium)
15. Verify "Recent Contributions" list displays
16. Verify "Top Contributors" leaderboard displays

**Members Tab:**
17. Search for member by username
18. Filter members by role
19. Click "Manage" on a member
20. Promote member to Officer
21. **Expected:** Member role updated
22. Demote member back to Member
23. **Expected:** Member role updated
24. (Optional) Kick member with confirmation

**Test Cases:**
- [ ] Tab switching works correctly
- [ ] Settings form validation (all fields)
- [ ] Settings save successfully
- [ ] Custom rank creation with permissions
- [ ] Rank editing updates permissions
- [ ] Rank deletion with confirmation
- [ ] Treasury display shows correct totals
- [ ] Contributions list with recent activity
- [ ] Contributors leaderboard sorting
- [ ] Member search functionality
- [ ] Member role filtering
- [ ] Member promotion/demotion
- [ ] Member kick with confirmation
- [ ] Real-time updates (Socket.io) for all tabs
- [ ] Responsive design on all tabs

---

## Phase 10: Stripe Payment Testing

### Prerequisites:
- [ ] Stripe test account created
- [ ] Test API keys configured in .env
- [ ] Webhook endpoint configured and tested

### Test Environment:
Use Stripe test mode with test credit cards:
- **Success:** 4242 4242 4242 4242
- **Decline:** 4000 0000 0000 0002
- **Insufficient Funds:** 4000 0000 0000 9995

### Test 1: Purchase Dark Matter

**Test Steps:**
1. Navigate to /matrix-shop (Phase 10 shop)
2. Select "1,000 Dark Matter" package (price: $4.99)
3. Click "Purchase" button
4. **Expected:** Stripe checkout modal opens
5. Enter test card: 4242 4242 4242 4242
6. Enter expiry: Any future date (12/25)
7. Enter CVC: Any 3 digits (123)
8. Submit payment
9. **Expected:** Payment success, redirected to shop
10. Verify:
    - Dark Matter balance increased by 1,000
    - Purchase appears in purchase history
    - Database record created in shop_purchases_enhanced

**Test Cases:**
- [ ] Checkout modal opens correctly
- [ ] Stripe Elements form displays
- [ ] Test card payment succeeds
- [ ] Dark Matter credited to account
- [ ] Purchase history updated
- [ ] Database transaction recorded
- [ ] Declined card shows error message
- [ ] Insufficient funds card shows error message
- [ ] Webhook received and processed

### Test 2: Purchase Cosmetic Item

**Test Steps:**
1. Select a cosmetic item (e.g., ship skin)
2. Click "Purchase"
3. Complete Stripe checkout
4. **Expected:** Item added to inventory
5. Click "Equip" on purchased item
6. **Expected:** Item equipped, visual change applies
7. Verify:
    - Item shows in "My Cosmetics"
    - Equipped status displays
    - Dark Matter deducted if using DM payment

**Test Cases:**
- [ ] Cosmetic purchase with Dark Matter
- [ ] Cosmetic purchase with real money (Stripe)
- [ ] Item appears in inventory
- [ ] Equip/unequip functionality
- [ ] Multiple items equipped simultaneously
- [ ] Cosmetic effects apply to game

### Test 3: Webhook Verification

**Test Steps:**
1. Make a test purchase
2. Monitor webhook endpoint logs
3. **Expected:** Webhook event received
4. Verify:
    - Event type: `checkout.session.completed`
    - Payment status: `paid`
    - Purchase record created
    - User balance updated
    - Webhook signature validated

**Webhook Testing Command:**
```bash
# Using Stripe CLI
stripe listen --forward-to localhost:3000/api/shop-enhanced/webhook/stripe

# Make test payment
stripe trigger checkout.session.completed
```

**Test Cases:**
- [ ] Webhook endpoint receives events
- [ ] Signature verification passes
- [ ] Payment success event processed
- [ ] Payment failure event handled
- [ ] Refund event processed correctly
- [ ] Duplicate events ignored (idempotency)
- [ ] Error logging for failed webhooks

---

## Socket.io Real-time Testing

### Test Setup:
1. Open two browser windows/tabs
2. Log in as different users in each
3. Have both users in the same alliance

### Test Scenarios:

**Alliance Dashboard:**
- [ ] User A invites User B → User B sees invitation notification
- [ ] User A posts announcement → User B sees announcement appear
- [ ] User C joins alliance → Both users see activity feed update
- [ ] Member online/offline status updates in real-time

**Alliance Wars:**
- [ ] User A declares war → User B (target alliance) sees notification
- [ ] User A records battle → War score updates for all viewers
- [ ] User A proposes peace → User B sees peace terms proposal
- [ ] War status changes reflected immediately for all users

**Alliance Diplomacy:**
- [ ] User A proposes treaty → User B sees incoming proposal
- [ ] User A accepts treaty → Relation established for both alliances
- [ ] User A breaks treaty → User B sees relation terminated

**Alliance Management:**
- [ ] User A updates settings → Changes visible to all members
- [ ] User A promotes User B → User B's interface updates with new permissions
- [ ] User A creates custom rank → Rank appears for all leaders/officers

---

## Performance Testing

### Load Testing Endpoints:
```bash
# Using Apache Bench (ab)
# Test alliance leaderboard (should handle 100 concurrent requests)
ab -n 1000 -c 100 http://localhost:3000/api/alliances/leaderboard/rankings

# Test alliance search (with authentication token)
ab -n 500 -c 50 -H "Authorization: Bearer YOUR_TOKEN" http://localhost:3000/api/alliances/search/query?q=test

# Expected: 95%+ requests succeed, <500ms average response time
```

### Database Query Performance:
```sql
-- Test alliance leaderboard query performance
EXPLAIN ANALYZE 
SELECT * FROM v_alliance_leaderboard 
ORDER BY total_power DESC 
LIMIT 100;
-- Expected: <100ms execution time

-- Test member list query performance
EXPLAIN ANALYZE
SELECT * FROM alliance_members 
WHERE alliance_id = 1;
-- Expected: <50ms execution time

-- Test diplomatic relations query performance
EXPLAIN ANALYZE
SELECT * FROM diplomatic_relations 
WHERE source_alliance_id = 1 OR target_alliance_id = 1;
-- Expected: <50ms execution time
```

---

## Known Issues and Troubleshooting

### Issue 1: Alliance Creation Fails
**Symptoms:** 500 error when creating alliance  
**Possible Causes:**
- Database connection lost
- Duplicate alliance tag
- User already in alliance
**Solution:** Check server logs, verify database connection, check alliance_members table

### Issue 2: Socket.io Not Connecting
**Symptoms:** Real-time updates not working  
**Possible Causes:**
- CORS configuration incorrect
- Redis connection failed
- Socket.io client version mismatch
**Solution:** Check browser console for errors, verify Redis connection, update socket.io client

### Issue 3: Stripe Webhook Not Receiving Events
**Symptoms:** Payments succeed but DM not credited  
**Possible Causes:**
- Webhook URL incorrect
- Webhook secret mismatch
- Firewall blocking webhooks
**Solution:** Use Stripe CLI for local testing, verify webhook endpoint in Stripe dashboard, check webhook signature validation

### Issue 4: Permission Denied Errors
**Symptoms:** Users cannot perform actions  
**Possible Causes:**
- User role not set correctly
- Permission check logic error
- Session expired
**Solution:** Verify user role in database, check permission middleware, refresh authentication token

---

## Deployment Checklist

### Pre-Deployment:
- [ ] Database schema deployed (Phase 11)
- [ ] All environment variables configured
- [ ] Backend compiled with zero TypeScript errors
- [ ] Frontend assets copied to public directory
- [ ] Redis connection tested
- [ ] PostgreSQL connection tested

### Deployment:
- [ ] Backend server started (PM2 or Docker)
- [ ] Health endpoint responding (/api/health)
- [ ] Socket.io server connected
- [ ] CORS configured for production domain
- [ ] SSL certificates installed (HTTPS)
- [ ] Firewall rules configured

### Post-Deployment:
- [ ] All Phase 11 frontend tests completed
- [ ] All Phase 10 Stripe tests completed
- [ ] Socket.io real-time tests completed
- [ ] Performance testing completed
- [ ] Error monitoring configured (e.g., Sentry)
- [ ] Backup strategy implemented
- [ ] Rollback plan documented

---

## Success Criteria

Phase 11 is considered **fully deployed and tested** when:

✅ All 22 database tables exist and have data  
✅ All 40+ API endpoints respond correctly  
✅ All 4 frontend interfaces load without errors  
✅ Real-time Socket.io updates work across all interfaces  
✅ Alliance creation/management workflows complete successfully  
✅ War system fully functional (declare, battle, end)  
✅ Diplomacy system fully functional (propose, accept, reject)  
✅ Member administration works (promote, demote, kick)  
✅ All Stripe payment flows complete successfully  
✅ Webhooks receive and process events correctly  
✅ Performance benchmarks met (response times < 500ms)  
✅ No critical errors in server logs  
✅ Cross-browser testing completed (Chrome, Firefox, Safari)  
✅ Mobile responsive design verified  

---

## Contact Information for Production Deployment

When ready for production deployment, provide:
1. Database connection credentials
2. Server access (SSH or deployment platform)
3. Domain name for frontend
4. Stripe production API keys
5. Any additional environment-specific requirements

This guide will ensure successful deployment and comprehensive testing of Phase 11 and Phase 10 features.

---

**Document Version:** 1.0  
**Last Updated:** 2025-11-07 00:34:21  
**Status:** Ready for Production Infrastructure
