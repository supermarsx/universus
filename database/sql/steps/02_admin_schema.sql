-- ========================================
-- UNIVERSUS ADMIN SYSTEM DATABASE SCHEMA
-- Phase 2: Advanced Admin Capabilities
-- ========================================

-- Admin Users Table (Multi-level admin system)
CREATE TABLE IF NOT EXISTS admin_users (
  id SERIAL PRIMARY KEY,
  user_id INTEGER REFERENCES users(id) ON DELETE CASCADE,
  admin_level VARCHAR(50) NOT NULL CHECK (admin_level IN ('super_admin', 'game_admin', 'moderator', 'support')),
  permissions TEXT[] DEFAULT '{}',
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  created_by INTEGER REFERENCES users(id),
  two_factor_enabled BOOLEAN DEFAULT FALSE,
  two_factor_secret VARCHAR(255),
  ip_whitelist TEXT[] DEFAULT '{}',
  last_login TIMESTAMP,
  is_active BOOLEAN DEFAULT TRUE,
  notes TEXT,
  UNIQUE(user_id)
);

-- Admin Audit Logs (Comprehensive action tracking)
CREATE TABLE IF NOT EXISTS admin_audit_logs (
  id SERIAL PRIMARY KEY,
  admin_id INTEGER REFERENCES users(id) ON DELETE SET NULL,
  admin_username VARCHAR(255) NOT NULL,
  action_type VARCHAR(100) NOT NULL,
  action_category VARCHAR(50) NOT NULL CHECK (action_category IN ('user_management', 'game_config', 'server_control', 'data_modification', 'security', 'monitoring')),
  target_type VARCHAR(50),
  target_id INTEGER,
  target_identifier VARCHAR(255),
  action_details JSONB,
  before_state JSONB,
  after_state JSONB,
  ip_address VARCHAR(45),
  user_agent TEXT,
  timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  severity VARCHAR(20) CHECK (severity IN ('low', 'medium', 'high', 'critical')),
  success BOOLEAN DEFAULT TRUE,
  error_message TEXT
);

CREATE INDEX idx_audit_admin_id ON admin_audit_logs(admin_id);
CREATE INDEX idx_audit_action_type ON admin_audit_logs(action_type);
CREATE INDEX idx_audit_timestamp ON admin_audit_logs(timestamp DESC);
CREATE INDEX idx_audit_target ON admin_audit_logs(target_type, target_id);

-- Admin Settings (Global game configuration)
CREATE TABLE IF NOT EXISTS admin_settings (
  id SERIAL PRIMARY KEY,
  setting_key VARCHAR(100) UNIQUE NOT NULL,
  setting_value JSONB NOT NULL,
  setting_category VARCHAR(50) NOT NULL,
  description TEXT,
  data_type VARCHAR(20) CHECK (data_type IN ('string', 'number', 'boolean', 'json', 'array')),
  is_public BOOLEAN DEFAULT FALSE,
  requires_restart BOOLEAN DEFAULT FALSE,
  last_modified TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  modified_by INTEGER REFERENCES users(id),
  version INTEGER DEFAULT 1,
  previous_value JSONB
);

CREATE INDEX idx_settings_category ON admin_settings(setting_category);
CREATE INDEX idx_settings_key ON admin_settings(setting_key);

-- User Blocks (Player blocking/muting system)
CREATE TABLE IF NOT EXISTS user_blocks (
  id SERIAL PRIMARY KEY,
  user_id INTEGER REFERENCES users(id) ON DELETE CASCADE,
  block_type VARCHAR(20) NOT NULL CHECK (block_type IN ('ban', 'mute', 'restrict', 'warning')),
  reason TEXT NOT NULL,
  duration_minutes INTEGER,
  start_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  end_time TIMESTAMP,
  is_permanent BOOLEAN DEFAULT FALSE,
  is_active BOOLEAN DEFAULT TRUE,
  blocked_by INTEGER REFERENCES users(id),
  unblocked_by INTEGER,
  unblock_time TIMESTAMP,
  unblock_reason TEXT,
  appeal_status VARCHAR(20) CHECK (appeal_status IN ('pending', 'approved', 'rejected', 'none')) DEFAULT 'none',
  notes TEXT,
  severity_level INTEGER CHECK (severity_level BETWEEN 1 AND 5),
  CONSTRAINT check_duration CHECK (
    (is_permanent = TRUE AND duration_minutes IS NULL) OR
    (is_permanent = FALSE AND duration_minutes IS NOT NULL) OR
    (is_permanent = FALSE AND duration_minutes IS NULL)
  )
);

CREATE INDEX idx_user_blocks_user ON user_blocks(user_id);
CREATE INDEX idx_user_blocks_active ON user_blocks(is_active);
CREATE INDEX idx_user_blocks_type ON user_blocks(block_type);
CREATE INDEX idx_user_blocks_end_time ON user_blocks(end_time);

-- Admin Player Tags (Categorization system)
CREATE TABLE IF NOT EXISTS admin_player_tags (
  id SERIAL PRIMARY KEY,
  user_id INTEGER REFERENCES users(id) ON DELETE CASCADE,
  tag_name VARCHAR(50) NOT NULL,
  tag_category VARCHAR(50) NOT NULL CHECK (tag_category IN ('behavior', 'payment', 'skill', 'special', 'support', 'custom')),
  tag_color VARCHAR(7),
  description TEXT,
  added_by INTEGER REFERENCES users(id),
  added_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  expires_at TIMESTAMP,
  is_active BOOLEAN DEFAULT TRUE,
  metadata JSONB,
  CONSTRAINT unique_user_tag UNIQUE(user_id, tag_name)
);

CREATE INDEX idx_player_tags_user ON admin_player_tags(user_id);
CREATE INDEX idx_player_tags_category ON admin_player_tags(tag_category);
CREATE INDEX idx_player_tags_name ON admin_player_tags(tag_name);
CREATE INDEX idx_player_tags_active ON admin_player_tags(is_active);

-- Admin Notifications (Real-time admin alerts)
CREATE TABLE IF NOT EXISTS admin_notifications (
  id SERIAL PRIMARY KEY,
  notification_type VARCHAR(50) NOT NULL,
  priority VARCHAR(20) CHECK (priority IN ('low', 'medium', 'high', 'critical')) DEFAULT 'medium',
  title VARCHAR(255) NOT NULL,
  message TEXT NOT NULL,
  data JSONB,
  target_admin_level VARCHAR(50),
  target_admin_ids INTEGER[],
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  expires_at TIMESTAMP,
  is_read BOOLEAN DEFAULT FALSE,
  read_by INTEGER[],
  action_url VARCHAR(500),
  requires_acknowledgment BOOLEAN DEFAULT FALSE,
  acknowledged_by INTEGER[],
  auto_dismiss BOOLEAN DEFAULT TRUE
);

CREATE INDEX idx_admin_notif_type ON admin_notifications(notification_type);
CREATE INDEX idx_admin_notif_priority ON admin_notifications(priority);
CREATE INDEX idx_admin_notif_created ON admin_notifications(created_at DESC);
CREATE INDEX idx_admin_notif_read ON admin_notifications(is_read);

-- Server Monitoring (Performance metrics and health)
CREATE TABLE IF NOT EXISTS server_monitoring (
  id SERIAL PRIMARY KEY,
  metric_type VARCHAR(50) NOT NULL,
  metric_name VARCHAR(100) NOT NULL,
  metric_value NUMERIC NOT NULL,
  metric_unit VARCHAR(20),
  timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  server_instance VARCHAR(100),
  metadata JSONB,
  threshold_exceeded BOOLEAN DEFAULT FALSE,
  alert_sent BOOLEAN DEFAULT FALSE
);

CREATE INDEX idx_server_mon_type ON server_monitoring(metric_type);
CREATE INDEX idx_server_mon_timestamp ON server_monitoring(timestamp DESC);
CREATE INDEX idx_server_mon_threshold ON server_monitoring(threshold_exceeded);

-- Partition by month for better performance
CREATE INDEX idx_server_mon_month ON server_monitoring((DATE_TRUNC('month', timestamp)));

-- Game Events (Admin-triggered events and announcements)
CREATE TABLE IF NOT EXISTS game_events (
  id SERIAL PRIMARY KEY,
  event_type VARCHAR(50) NOT NULL CHECK (event_type IN ('announcement', 'maintenance', 'tournament', 'bonus', 'special_event', 'emergency')),
  event_name VARCHAR(255) NOT NULL,
  event_description TEXT,
  event_data JSONB,
  start_time TIMESTAMP NOT NULL,
  end_time TIMESTAMP,
  is_active BOOLEAN DEFAULT FALSE,
  is_recurring BOOLEAN DEFAULT FALSE,
  recurrence_pattern VARCHAR(100),
  target_scope VARCHAR(20) CHECK (target_scope IN ('all', 'alliance', 'user', 'galaxy', 'custom')) DEFAULT 'all',
  target_ids INTEGER[],
  created_by INTEGER REFERENCES users(id),
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  modified_at TIMESTAMP,
  priority INTEGER CHECK (priority BETWEEN 1 AND 10) DEFAULT 5,
  visibility VARCHAR(20) CHECK (visibility IN ('public', 'hidden', 'admin_only')) DEFAULT 'public',
  requires_participation BOOLEAN DEFAULT FALSE,
  participation_count INTEGER DEFAULT 0,
  rewards JSONB,
  conditions JSONB
);

CREATE INDEX idx_game_events_type ON game_events(event_type);
CREATE INDEX idx_game_events_active ON game_events(is_active);
CREATE INDEX idx_game_events_start ON game_events(start_time);
CREATE INDEX idx_game_events_scope ON game_events(target_scope);

-- ========================================
-- ENHANCED EXISTING TABLES
-- ========================================

-- Add admin fields to users table
ALTER TABLE users ADD COLUMN IF NOT EXISTS admin_notes TEXT;
ALTER TABLE users ADD COLUMN IF NOT EXISTS admin_flags TEXT[];
ALTER TABLE users ADD COLUMN IF NOT EXISTS account_status VARCHAR(20) DEFAULT 'active' CHECK (account_status IN ('active', 'suspended', 'banned', 'deleted', 'review'));
ALTER TABLE users ADD COLUMN IF NOT EXISTS risk_score INTEGER DEFAULT 0 CHECK (risk_score BETWEEN 0 AND 100);
ALTER TABLE users ADD COLUMN IF NOT EXISTS lifetime_value NUMERIC DEFAULT 0;
ALTER TABLE users ADD COLUMN IF NOT EXISTS last_ip VARCHAR(45);
ALTER TABLE users ADD COLUMN IF NOT EXISTS country_code VARCHAR(3);
ALTER TABLE users ADD COLUMN IF NOT EXISTS referral_source VARCHAR(100);

-- Add admin fields to alliances table (if exists)
DO $$ 
BEGIN
  IF EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'alliances') THEN
    ALTER TABLE alliances ADD COLUMN IF NOT EXISTS admin_notes TEXT;
    ALTER TABLE alliances ADD COLUMN IF NOT EXISTS monitoring_status VARCHAR(20) DEFAULT 'normal' CHECK (monitoring_status IN ('normal', 'watch', 'alert', 'suspended'));
    ALTER TABLE alliances ADD COLUMN IF NOT EXISTS verification_status VARCHAR(20) DEFAULT 'unverified' CHECK (verification_status IN ('unverified', 'verified', 'official', 'partner'));
  END IF;
END $$;

-- Add admin fields to planets table
ALTER TABLE planets ADD COLUMN IF NOT EXISTS admin_flags TEXT[];
ALTER TABLE planets ADD COLUMN IF NOT EXISTS special_status VARCHAR(50);
ALTER TABLE planets ADD COLUMN IF NOT EXISTS is_protected BOOLEAN DEFAULT FALSE;

-- ========================================
-- ADMIN PERMISSION TEMPLATES
-- ========================================

-- Super Admin: Full system access
-- Permissions: ['*']

-- Game Admin: Game management, user management, monitoring
-- Permissions: ['user:read', 'user:write', 'user:ban', 'game:config', 'game:events', 'monitoring:read', 'reports:read']

-- Moderator: User moderation, content management
-- Permissions: ['user:read', 'user:mute', 'user:warn', 'content:moderate', 'reports:read']

-- Support: Basic user assistance
-- Permissions: ['user:read', 'user:assist', 'tickets:manage']

-- ========================================
-- ADMIN VIEWS FOR ANALYTICS
-- ========================================

-- Active Admin Users View
CREATE OR REPLACE VIEW v_active_admins AS
SELECT 
  au.id,
  u.username,
  u.email,
  au.admin_level,
  au.is_active,
  au.last_login,
  au.created_at,
  COALESCE(
    (SELECT COUNT(*) FROM admin_audit_logs WHERE admin_id = u.id AND timestamp > NOW() - INTERVAL '24 hours'),
    0
  ) as actions_today
FROM admin_users au
JOIN users u ON au.user_id = u.id
WHERE au.is_active = TRUE;

-- User Block Statistics View
CREATE OR REPLACE VIEW v_user_block_stats AS
SELECT 
  user_id,
  u.username,
  COUNT(*) as total_blocks,
  SUM(CASE WHEN is_active THEN 1 ELSE 0 END) as active_blocks,
  MAX(start_time) as last_block_date,
  array_agg(DISTINCT block_type) as block_types
FROM user_blocks ub
JOIN users u ON ub.user_id = u.id
GROUP BY user_id, u.username;

-- Admin Action Summary View
CREATE OR REPLACE VIEW v_admin_action_summary AS
SELECT 
  admin_username,
  action_category,
  COUNT(*) as action_count,
  DATE_TRUNC('day', timestamp) as action_date
FROM admin_audit_logs
WHERE timestamp > NOW() - INTERVAL '30 days'
GROUP BY admin_username, action_category, DATE_TRUNC('day', timestamp);

-- Server Health Summary View
CREATE OR REPLACE VIEW v_server_health AS
SELECT 
  metric_type,
  metric_name,
  AVG(metric_value) as avg_value,
  MAX(metric_value) as max_value,
  MIN(metric_value) as min_value,
  metric_unit,
  DATE_TRUNC('hour', timestamp) as time_bucket
FROM server_monitoring
WHERE timestamp > NOW() - INTERVAL '24 hours'
GROUP BY metric_type, metric_name, metric_unit, DATE_TRUNC('hour', timestamp);

-- ========================================
-- ADMIN HELPER FUNCTIONS
-- ========================================

-- Function to log admin actions
CREATE OR REPLACE FUNCTION log_admin_action(
  p_admin_id INTEGER,
  p_admin_username VARCHAR,
  p_action_type VARCHAR,
  p_action_category VARCHAR,
  p_target_type VARCHAR DEFAULT NULL,
  p_target_id INTEGER DEFAULT NULL,
  p_action_details JSONB DEFAULT NULL,
  p_severity VARCHAR DEFAULT 'medium'
) RETURNS INTEGER AS $$
DECLARE
  v_log_id INTEGER;
BEGIN
  INSERT INTO admin_audit_logs (
    admin_id, admin_username, action_type, action_category,
    target_type, target_id, action_details, severity
  )
  VALUES (
    p_admin_id, p_admin_username, p_action_type, p_action_category,
    p_target_type, p_target_id, p_action_details, p_severity
  )
  RETURNING id INTO v_log_id;
  
  RETURN v_log_id;
END;
$$ LANGUAGE plpgsql;

-- Function to check user block status
CREATE OR REPLACE FUNCTION is_user_blocked(p_user_id INTEGER) RETURNS TABLE(
  is_blocked BOOLEAN,
  block_type VARCHAR,
  reason TEXT,
  end_time TIMESTAMP
) AS $$
BEGIN
  RETURN QUERY
  SELECT 
    TRUE as is_blocked,
    ub.block_type,
    ub.reason,
    ub.end_time
  FROM user_blocks ub
  WHERE ub.user_id = p_user_id
    AND ub.is_active = TRUE
    AND (ub.end_time IS NULL OR ub.end_time > NOW())
  ORDER BY ub.start_time DESC
  LIMIT 1;
  
  IF NOT FOUND THEN
    RETURN QUERY SELECT FALSE, NULL::VARCHAR, NULL::TEXT, NULL::TIMESTAMP;
  END IF;
END;
$$ LANGUAGE plpgsql;

-- Function to create admin notification
CREATE OR REPLACE FUNCTION create_admin_notification(
  p_type VARCHAR,
  p_priority VARCHAR,
  p_title VARCHAR,
  p_message TEXT,
  p_data JSONB DEFAULT NULL,
  p_target_level VARCHAR DEFAULT NULL
) RETURNS INTEGER AS $$
DECLARE
  v_notif_id INTEGER;
BEGIN
  INSERT INTO admin_notifications (
    notification_type, priority, title, message, data, target_admin_level
  )
  VALUES (
    p_type, p_priority, p_title, p_message, p_data, p_target_level
  )
  RETURNING id INTO v_notif_id;
  
  RETURN v_notif_id;
END;
$$ LANGUAGE plpgsql;

-- Function to auto-expire user blocks
CREATE OR REPLACE FUNCTION auto_expire_blocks() RETURNS INTEGER AS $$
DECLARE
  v_expired_count INTEGER;
BEGIN
  UPDATE user_blocks
  SET is_active = FALSE,
      unblock_time = NOW(),
      unblock_reason = 'Auto-expired'
  WHERE is_active = TRUE
    AND is_permanent = FALSE
    AND end_time IS NOT NULL
    AND end_time < NOW();
    
  GET DIAGNOSTICS v_expired_count = ROW_COUNT;
  RETURN v_expired_count;
END;
$$ LANGUAGE plpgsql;

-- ========================================
-- ADMIN TRIGGERS
-- ========================================

-- Trigger to update admin settings version
CREATE OR REPLACE FUNCTION update_setting_version() RETURNS TRIGGER AS $$
BEGIN
  NEW.version := OLD.version + 1;
  NEW.previous_value := OLD.setting_value;
  NEW.last_modified := NOW();
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_update_setting_version
BEFORE UPDATE ON admin_settings
FOR EACH ROW
WHEN (OLD.setting_value IS DISTINCT FROM NEW.setting_value)
EXECUTE FUNCTION update_setting_version();

-- ========================================
-- INITIAL DATA - DEFAULT SETTINGS
-- ========================================

INSERT INTO admin_settings (setting_key, setting_value, setting_category, description, data_type) VALUES
('game.speed_multiplier', '1', 'game_mechanics', 'Global game speed multiplier', 'number'),
('game.resource_production_rate', '1.0', 'economy', 'Resource production rate multiplier', 'number'),
('game.combat_damage_multiplier', '1.0', 'combat', 'Combat damage multiplier', 'number'),
('game.max_planets_per_user', '9', 'limits', 'Maximum planets per user', 'number'),
('game.max_fleet_slots', '10', 'limits', 'Maximum concurrent fleet missions', 'number'),
('server.maintenance_mode', 'false', 'server', 'Enable maintenance mode', 'boolean'),
('server.registration_enabled', 'true', 'server', 'Allow new user registration', 'boolean'),
('security.max_login_attempts', '5', 'security', 'Maximum login attempts before lockout', 'number'),
('security.session_timeout_minutes', '120', 'security', 'User session timeout in minutes', 'number'),
('features.alliance_system_enabled', 'true', 'features', 'Enable alliance system', 'boolean'),
('features.marketplace_enabled', 'true', 'features', 'Enable player marketplace', 'boolean')
ON CONFLICT (setting_key) DO NOTHING;

-- ========================================
-- ADMIN SCHEMA COMPLETE
-- ========================================
-- Total tables created: 8 new tables + enhanced 3 existing tables
-- Total views: 4 analytical views
-- Total functions: 5 helper functions
-- Total triggers: 1 automatic trigger
-- ========================================
