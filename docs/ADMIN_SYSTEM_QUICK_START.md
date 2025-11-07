# Admin System Quick Start Guide

## Setup Instructions

### 1. Database Migration

Run the admin schema to create all necessary tables:

```bash
cd /workspace/universus-rpg/backend
psql -U postgres -d universus_rpg -f database/sql/admin_schema.sql
```

### 2. Create First Admin User

After running the migration, create your first super admin:

```sql
-- Connect to database
psql -U postgres -d universus_rpg

-- Create super admin (replace user_id with your user's ID)
INSERT INTO admin_users (user_id, admin_level, permissions, is_active)
VALUES (1, 'super_admin', ARRAY['*'], TRUE);
```

### 3. Compile and Start Server

```bash
cd /workspace/universus-rpg/backend
npm run build
npm start
```

The admin monitoring service and block expiration scheduler will start automatically.

## Accessing the Admin Panel

### Admin Dashboard
- URL: http://localhost:3000/admin/dashboard
- Login required with admin privileges
- Auto-refreshes every 30 seconds

### Admin Pages
- Dashboard: `/admin/dashboard`
- User Management: `/admin/users`
- Server Monitoring: `/admin/monitoring`
- Settings: `/admin/settings`
- Events: `/admin/events`
- Analytics: `/admin/analytics`
- Audit Logs: `/admin/audit`

## API Endpoints

### Dashboard Data
```bash
GET /api/admin/dashboard
Authorization: Bearer <token>
```

### User Management
```bash
# List all users
GET /api/admin/users?page=1&limit=50
Authorization: Bearer <token>

# Get user details
GET /api/admin/users/:id
Authorization: Bearer <token>

# Block a user
POST /api/admin/users/:id/block
Authorization: Bearer <token>
Content-Type: application/json
{
  "block_type": "ban",
  "reason": "Violating terms of service",
  "duration_minutes": 1440,
  "is_permanent": false
}

# Tag a user
POST /api/admin/users/:id/tag
Authorization: Bearer <token>
Content-Type: application/json
{
  "tag_name": "VIP",
  "tag_category": "special",
  "tag_color": "#10b981",
  "description": "High-value player"
}
```

### Server Monitoring
```bash
# Get server health
GET /api/admin/monitoring/health
Authorization: Bearer <token>

# Get metrics history
GET /api/admin/monitoring/metrics/cpu_usage?hours=24
Authorization: Bearer <token>

# Get player activity
GET /api/admin/monitoring/activity
Authorization: Bearer <token>
```

### Settings Management
```bash
# Get all settings
GET /api/admin/settings
Authorization: Bearer <token>

# Update a setting
PUT /api/admin/settings/game.speed_multiplier
Authorization: Bearer <token>
Content-Type: application/json
{
  "value": 2
}
```

### Game Events
```bash
# Create an event
POST /api/admin/events
Authorization: Bearer <token>
Content-Type: application/json
{
  "event_type": "bonus",
  "event_name": "Double Resources Weekend",
  "event_description": "All resource production doubled",
  "start_time": "2025-11-08T00:00:00Z",
  "end_time": "2025-11-10T23:59:59Z",
  "target_scope": "all",
  "priority": 8
}

# Activate event
POST /api/admin/events/:id/activate
Authorization: Bearer <token>
```

## Admin Levels & Permissions

### Super Admin (Full Access)
- Permissions: `['*']`
- Can do everything

### Game Admin
- Permissions: `['user:read', 'user:write', 'user:ban', 'game:config', 'monitoring:read', ...]`
- User management, game configuration, monitoring

### Moderator
- Permissions: `['user:read', 'user:mute', 'user:warn', 'content:moderate', ...]`
- User moderation, content management

### Support
- Permissions: `['user:read', 'user:assist', 'tickets:manage']`
- Basic user assistance

## Testing the System

### 1. Access Dashboard
Navigate to http://localhost:3000/admin/dashboard after logging in as an admin user.

### 2. Test User Management
```bash
# Get your token first (login)
TOKEN="your_jwt_token_here"

# List users
curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:3000/api/admin/users

# Get server health
curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:3000/api/admin/monitoring/health
```

### 3. Monitor Server Metrics
Server metrics are automatically collected every 60 seconds. View them via:
- Dashboard UI
- `/api/admin/monitoring/metrics/:name` endpoint

### 4. Check Audit Logs
All admin actions are automatically logged. View them via:
```bash
curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:3000/api/admin/audit-logs?page=1&limit=20
```

## Automatic Services

### Metric Collection
- Runs every 60 seconds
- Collects: CPU, memory, database connections, active players
- Stores in `server_monitoring` table
- Triggers alerts on threshold exceeded

### Block Expiration
- Runs every 5 minutes
- Auto-expires temporary blocks
- Updates user account status
- Creates notifications

## Troubleshooting

### Cannot Access Admin Dashboard
1. Ensure you have an admin_users record
2. Check that is_active = TRUE
3. Verify your user is not blocked
4. Check JWT token is valid

### Monitoring Not Working
1. Check server logs for errors
2. Verify PostgreSQL is running
3. Check database connections
4. Review monitoring service startup logs

### API Returns 403 Forbidden
1. Verify admin_level is sufficient
2. Check required permissions
3. Review audit logs for failed attempts
4. Ensure IP whitelist (if configured)

## Security Best Practices

1. **Enable Two-Factor Authentication** (ready for implementation)
2. **Use IP Whitelisting** for super admins
3. **Regularly Review Audit Logs**
4. **Set Strong Admin Passwords**
5. **Limit Super Admin Accounts**
6. **Monitor Failed Login Attempts**

## Database Maintenance

### Clean Old Metrics
```sql
SELECT admin_monitoring_service.cleanup_old_metrics(30); -- Keep 30 days
```

### View Admin Activity
```sql
SELECT * FROM v_admin_action_summary ORDER BY action_date DESC LIMIT 100;
```

### Check Server Health
```sql
SELECT * FROM v_server_health ORDER BY time_bucket DESC LIMIT 24;
```

## What's Next?

The backend is fully functional. To complete the admin system:

1. **User Management UI** - Build complete interface with filters and bulk actions
2. **Monitoring Dashboard** - Add real-time charts and graphs
3. **Settings UI** - Create form-based configuration editor
4. **Events UI** - Build event creation and management interface
5. **Analytics UI** - Implement charts and visualizations
6. **Audit Viewer** - Create searchable log interface

All API endpoints are ready and fully functional. The frontend templates just need to be connected to the APIs.

---

**Need Help?** Check the ADMIN_SYSTEM_IMPLEMENTATION_REPORT.md for detailed documentation.
