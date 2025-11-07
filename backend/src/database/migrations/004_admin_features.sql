-- Migration: Add admin features and audit logging
-- Description: Adds admin role management, audit logging, system logs, and game settings

-- Add admin-related columns to users table
ALTER TABLE users ADD COLUMN IF NOT EXISTS is_admin BOOLEAN DEFAULT false;
ALTER TABLE users ADD COLUMN IF NOT EXISTS is_banned BOOLEAN DEFAULT false;
ALTER TABLE users ADD COLUMN IF NOT EXISTS ban_reason TEXT;
ALTER TABLE users ADD COLUMN IF NOT EXISTS banned_at TIMESTAMP WITH TIME ZONE;
ALTER TABLE users ADD COLUMN IF NOT EXISTS last_login TIMESTAMP WITH TIME ZONE;

-- Create indexes for admin queries
CREATE INDEX IF NOT EXISTS idx_users_admin ON users(is_admin) WHERE is_admin = true;
CREATE INDEX IF NOT EXISTS idx_users_banned ON users(is_banned) WHERE is_banned = true;
CREATE INDEX IF NOT EXISTS idx_users_last_login ON users(last_login DESC);

-- Admin audit log table
CREATE TABLE IF NOT EXISTS admin_audit_log (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    action VARCHAR(100) NOT NULL,
    details JSONB,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

-- Index for audit log queries
CREATE INDEX IF NOT EXISTS idx_audit_log_user ON admin_audit_log(user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_audit_log_action ON admin_audit_log(action, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_audit_log_created ON admin_audit_log(created_at DESC);

-- System logs table
CREATE TABLE IF NOT EXISTS system_logs (
    id SERIAL PRIMARY KEY,
    level VARCHAR(20) NOT NULL,
    message TEXT NOT NULL,
    metadata JSONB,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT valid_log_level CHECK (level IN ('error', 'warn', 'info', 'debug'))
);

-- Index for log queries
CREATE INDEX IF NOT EXISTS idx_system_logs_level ON system_logs(level, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_system_logs_created ON system_logs(created_at DESC);

-- Game settings table
CREATE TABLE IF NOT EXISTS game_settings (
    key VARCHAR(100) PRIMARY KEY,
    value JSONB NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

-- Insert default settings
INSERT INTO game_settings (key, value) VALUES
    ('maintenanceMode', 'false'::jsonb),
    ('registrationEnabled', 'true'::jsonb),
    ('maxPlayers', '10000'::jsonb),
    ('motd', '"Welcome to SpaceEmpire!"'::jsonb)
ON CONFLICT (key) DO NOTHING;

-- Function to automatically update updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger for game_settings
DROP TRIGGER IF EXISTS update_game_settings_updated_at ON game_settings;
CREATE TRIGGER update_game_settings_updated_at
    BEFORE UPDATE ON game_settings
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- View for admin dashboard summary
CREATE OR REPLACE VIEW admin_dashboard_summary AS
SELECT 
    (SELECT COUNT(*) FROM users) as total_users,
    (SELECT COUNT(*) FROM users WHERE is_admin = true) as total_admins,
    (SELECT COUNT(*) FROM users WHERE is_banned = true) as banned_users,
    (SELECT COUNT(*) FROM users WHERE last_login > NOW() - INTERVAL '24 hours') as active_24h,
    (SELECT COUNT(*) FROM users WHERE last_login > NOW() - INTERVAL '7 days') as active_7d,
    (SELECT COUNT(*) FROM users WHERE created_at::date = CURRENT_DATE) as new_today,
    (SELECT COUNT(*) FROM planets) as total_planets,
    (SELECT COUNT(*) FROM combats_precise WHERE status = 'in_progress') as active_combats,
    (SELECT pg_database_size(current_database()) / 1024 / 1024) as db_size_mb;

-- Comments on tables
COMMENT ON TABLE admin_audit_log IS 'Audit trail for all administrative actions';
COMMENT ON TABLE system_logs IS 'System-wide log entries for debugging and monitoring';
COMMENT ON TABLE game_settings IS 'Global game configuration settings';
COMMENT ON COLUMN users.is_admin IS 'Whether the user has administrative privileges';
COMMENT ON COLUMN users.is_banned IS 'Whether the user is banned from the game';
COMMENT ON COLUMN users.ban_reason IS 'Reason for user ban';
COMMENT ON COLUMN users.banned_at IS 'Timestamp when user was banned';
COMMENT ON COLUMN users.last_login IS 'Last successful login timestamp';

-- Grant permissions (adjust as needed for your user)
-- GRANT ALL ON admin_audit_log, system_logs, game_settings TO your_app_user;
-- GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO your_app_user;
