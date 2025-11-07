-- Phase 9: Advanced Account Management System Schema
-- Comprehensive account security, session management, email verification,
-- password recovery, 2FA, GDPR compliance, and audit logging

-- =====================================================
-- TABLE: account_suspensions
-- Purpose: Track account suspension history and status
-- =====================================================
CREATE TABLE IF NOT EXISTS account_suspensions (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    reason VARCHAR(100) NOT NULL,
    suspended_by INTEGER NOT NULL REFERENCES users(id),
    suspended_at TIMESTAMP NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMP,
    lifted_at TIMESTAMP,
    lifted_by INTEGER REFERENCES users(id),
    notes TEXT,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_account_suspensions_user_id ON account_suspensions(user_id);
CREATE INDEX idx_account_suspensions_active ON account_suspensions(is_active) WHERE is_active = TRUE;
CREATE INDEX idx_account_suspensions_expires ON account_suspensions(expires_at) WHERE expires_at IS NOT NULL;

-- =====================================================
-- TABLE: account_transfers
-- Purpose: Handle account ownership transfers
-- =====================================================
CREATE TABLE IF NOT EXISTS account_transfers (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    from_email VARCHAR(255) NOT NULL,
    to_email VARCHAR(255) NOT NULL,
    verification_token VARCHAR(255) NOT NULL UNIQUE,
    status VARCHAR(50) NOT NULL DEFAULT 'pending',
    initiated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    verified_at TIMESTAMP,
    completed_at TIMESTAMP,
    cancelled_at TIMESTAMP,
    expires_at TIMESTAMP NOT NULL,
    ip_address VARCHAR(45),
    user_agent TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_account_transfers_user_id ON account_transfers(user_id);
CREATE INDEX idx_account_transfers_status ON account_transfers(status);
CREATE INDEX idx_account_transfers_token ON account_transfers(verification_token);
CREATE INDEX idx_account_transfers_expires ON account_transfers(expires_at);

-- =====================================================
-- TABLE: email_verifications
-- Purpose: Email verification tokens and status
-- =====================================================
CREATE TABLE IF NOT EXISTS email_verifications (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    email VARCHAR(255) NOT NULL,
    verification_token VARCHAR(255) NOT NULL UNIQUE,
    status VARCHAR(50) NOT NULL DEFAULT 'pending',
    sent_at TIMESTAMP NOT NULL DEFAULT NOW(),
    verified_at TIMESTAMP,
    expires_at TIMESTAMP NOT NULL,
    attempts INTEGER NOT NULL DEFAULT 0,
    ip_address VARCHAR(45),
    user_agent TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_email_verifications_user_id ON email_verifications(user_id);
CREATE INDEX idx_email_verifications_token ON email_verifications(verification_token);
CREATE INDEX idx_email_verifications_status ON email_verifications(status);
CREATE INDEX idx_email_verifications_expires ON email_verifications(expires_at);

-- =====================================================
-- TABLE: password_resets
-- Purpose: Password recovery tokens and history
-- =====================================================
CREATE TABLE IF NOT EXISTS password_resets (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    reset_token VARCHAR(255) NOT NULL UNIQUE,
    status VARCHAR(50) NOT NULL DEFAULT 'pending',
    initiated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    validated_at TIMESTAMP,
    completed_at TIMESTAMP,
    expires_at TIMESTAMP NOT NULL,
    ip_address VARCHAR(45),
    user_agent TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_password_resets_user_id ON password_resets(user_id);
CREATE INDEX idx_password_resets_token ON password_resets(reset_token);
CREATE INDEX idx_password_resets_status ON password_resets(status);
CREATE INDEX idx_password_resets_expires ON password_resets(expires_at);

-- =====================================================
-- TABLE: two_factor_auth
-- Purpose: 2FA settings and backup codes
-- =====================================================
CREATE TABLE IF NOT EXISTS two_factor_auth (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL UNIQUE REFERENCES users(id) ON DELETE CASCADE,
    method VARCHAR(50) NOT NULL DEFAULT 'totp',
    secret VARCHAR(255) NOT NULL,
    is_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    verified_at TIMESTAMP,
    backup_codes JSONB,
    recovery_email VARCHAR(255),
    last_used_at TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_two_factor_auth_user_id ON two_factor_auth(user_id);
CREATE INDEX idx_two_factor_auth_enabled ON two_factor_auth(is_enabled) WHERE is_enabled = TRUE;

-- =====================================================
-- TABLE: user_sessions
-- Purpose: Track active user sessions and devices
-- =====================================================
CREATE TABLE IF NOT EXISTS user_sessions (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    session_token VARCHAR(255) NOT NULL UNIQUE,
    device_fingerprint VARCHAR(255),
    device_name VARCHAR(255),
    device_type VARCHAR(50),
    browser VARCHAR(100),
    os VARCHAR(100),
    ip_address VARCHAR(45) NOT NULL,
    location VARCHAR(255),
    latitude DECIMAL(10, 8),
    longitude DECIMAL(11, 8),
    status VARCHAR(50) NOT NULL DEFAULT 'active',
    is_trusted BOOLEAN NOT NULL DEFAULT FALSE,
    last_activity TIMESTAMP NOT NULL DEFAULT NOW(),
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMP NOT NULL
);

CREATE INDEX idx_user_sessions_user_id ON user_sessions(user_id);
CREATE INDEX idx_user_sessions_token ON user_sessions(session_token);
CREATE INDEX idx_user_sessions_status ON user_sessions(status);
CREATE INDEX idx_user_sessions_expires ON user_sessions(expires_at);
CREATE INDEX idx_user_sessions_device ON user_sessions(device_fingerprint);

-- =====================================================
-- TABLE: security_audit_logs
-- Purpose: Comprehensive security event logging
-- =====================================================
CREATE TABLE IF NOT EXISTS security_audit_logs (
    id SERIAL PRIMARY KEY,
    user_id INTEGER REFERENCES users(id) ON DELETE SET NULL,
    event_type VARCHAR(100) NOT NULL,
    event_description TEXT NOT NULL,
    severity VARCHAR(50) NOT NULL DEFAULT 'info',
    ip_address VARCHAR(45),
    user_agent TEXT,
    metadata JSONB,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_security_audit_logs_user_id ON security_audit_logs(user_id);
CREATE INDEX idx_security_audit_logs_event_type ON security_audit_logs(event_type);
CREATE INDEX idx_security_audit_logs_severity ON security_audit_logs(severity);
CREATE INDEX idx_security_audit_logs_created_at ON security_audit_logs(created_at DESC);

-- =====================================================
-- TABLE: gdpr_requests
-- Purpose: GDPR compliance requests (export, delete, etc.)
-- =====================================================
CREATE TABLE IF NOT EXISTS gdpr_requests (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    request_type VARCHAR(50) NOT NULL,
    status VARCHAR(50) NOT NULL DEFAULT 'pending',
    requested_at TIMESTAMP NOT NULL DEFAULT NOW(),
    processed_at TIMESTAMP,
    completed_at TIMESTAMP,
    data_url VARCHAR(500),
    expires_at TIMESTAMP,
    notes TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_gdpr_requests_user_id ON gdpr_requests(user_id);
CREATE INDEX idx_gdpr_requests_type ON gdpr_requests(request_type);
CREATE INDEX idx_gdpr_requests_status ON gdpr_requests(status);
CREATE INDEX idx_gdpr_requests_created_at ON gdpr_requests(created_at DESC);

-- =====================================================
-- TABLE: user_blocks
-- Purpose: User blocking and muting functionality
-- =====================================================
CREATE TABLE IF NOT EXISTS user_blocks (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    blocked_user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    block_type VARCHAR(50) NOT NULL DEFAULT 'full',
    reason TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    UNIQUE(user_id, blocked_user_id)
);

CREATE INDEX idx_user_blocks_user_id ON user_blocks(user_id);
CREATE INDEX idx_user_blocks_blocked_user_id ON user_blocks(blocked_user_id);
CREATE INDEX idx_user_blocks_type ON user_blocks(block_type);

-- =====================================================
-- TABLE: user_activity_logs
-- Purpose: Track user activity for monitoring
-- =====================================================
CREATE TABLE IF NOT EXISTS user_activity_logs (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    activity_type VARCHAR(100) NOT NULL,
    description TEXT,
    ip_address VARCHAR(45),
    metadata JSONB,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_user_activity_logs_user_id ON user_activity_logs(user_id);
CREATE INDEX idx_user_activity_logs_type ON user_activity_logs(activity_type);
CREATE INDEX idx_user_activity_logs_created_at ON user_activity_logs(created_at DESC);

-- =====================================================
-- TABLE: account_data_backups
-- Purpose: Store account backup data for recovery
-- =====================================================
CREATE TABLE IF NOT EXISTS account_data_backups (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    backup_data JSONB NOT NULL,
    backup_size INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMP
);

CREATE INDEX idx_account_data_backups_user_id ON account_data_backups(user_id);
CREATE INDEX idx_account_data_backups_created_at ON account_data_backups(created_at DESC);

-- =====================================================
-- TABLE: backup_verification_codes
-- Purpose: Verification codes for backup downloads
-- =====================================================
CREATE TABLE IF NOT EXISTS backup_verification_codes (
    id SERIAL PRIMARY KEY,
    backup_id INTEGER NOT NULL REFERENCES account_data_backups(id) ON DELETE CASCADE,
    verification_code VARCHAR(100) NOT NULL UNIQUE,
    is_used BOOLEAN NOT NULL DEFAULT FALSE,
    used_at TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMP NOT NULL
);

CREATE INDEX idx_backup_verification_codes_backup_id ON backup_verification_codes(backup_id);
CREATE INDEX idx_backup_verification_codes_code ON backup_verification_codes(verification_code);

-- =====================================================
-- VIEW: active_user_sessions_view
-- Purpose: Quick access to active sessions per user
-- =====================================================
CREATE OR REPLACE VIEW active_user_sessions_view AS
SELECT 
    user_id,
    COUNT(*) as active_session_count,
    MAX(last_activity) as most_recent_activity,
    ARRAY_AGG(
        jsonb_build_object(
            'session_id', id,
            'device_name', device_name,
            'ip_address', ip_address,
            'location', location,
            'last_activity', last_activity
        ) ORDER BY last_activity DESC
    ) as sessions
FROM user_sessions
WHERE status = 'active' AND expires_at > NOW()
GROUP BY user_id;

-- =====================================================
-- VIEW: security_risk_assessment_view
-- Purpose: Identify users with security risks
-- =====================================================
CREATE OR REPLACE VIEW security_risk_assessment_view AS
SELECT 
    u.id as user_id,
    u.username,
    u.email,
    CASE
        WHEN COUNT(DISTINCT s.ip_address) > 10 THEN 'high'
        WHEN COUNT(DISTINCT s.ip_address) > 5 THEN 'medium'
        ELSE 'low'
    END as risk_level,
    COUNT(DISTINCT s.ip_address) as unique_ip_count,
    COUNT(DISTINCT s.location) as unique_location_count,
    (SELECT COUNT(*) FROM security_audit_logs sal 
     WHERE sal.user_id = u.id AND sal.severity IN ('high', 'critical') 
     AND sal.created_at > NOW() - INTERVAL '30 days') as recent_security_events,
    (SELECT is_enabled FROM two_factor_auth tfa WHERE tfa.user_id = u.id) as has_2fa
FROM users u
LEFT JOIN user_sessions s ON s.user_id = u.id
GROUP BY u.id, u.username, u.email;

-- =====================================================
-- VIEW: gdpr_compliance_status_view
-- Purpose: Track GDPR compliance status per user
-- =====================================================
CREATE OR REPLACE VIEW gdpr_compliance_status_view AS
SELECT 
    u.id as user_id,
    u.username,
    u.email,
    (SELECT COUNT(*) FROM gdpr_requests gr 
     WHERE gr.user_id = u.id AND gr.request_type = 'export_data') as data_export_requests,
    (SELECT COUNT(*) FROM gdpr_requests gr 
     WHERE gr.user_id = u.id AND gr.request_type = 'delete_data') as deletion_requests,
    (SELECT MAX(created_at) FROM gdpr_requests gr 
     WHERE gr.user_id = u.id) as last_request_date,
    u.created_at as account_created_at
FROM users u;

-- =====================================================
-- FUNCTION: check_account_access
-- Purpose: Check if account can access system
-- =====================================================
CREATE OR REPLACE FUNCTION check_account_access(p_user_id INTEGER)
RETURNS TABLE(
    can_access BOOLEAN,
    reason VARCHAR(255),
    suspension_expires TIMESTAMP
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        CASE
            WHEN EXISTS (
                SELECT 1 FROM account_suspensions 
                WHERE user_id = p_user_id 
                AND is_active = TRUE 
                AND (expires_at IS NULL OR expires_at > NOW())
            ) THEN FALSE
            ELSE TRUE
        END as can_access,
        CASE
            WHEN EXISTS (
                SELECT 1 FROM account_suspensions 
                WHERE user_id = p_user_id 
                AND is_active = TRUE 
                AND (expires_at IS NULL OR expires_at > NOW())
            ) THEN (
                SELECT reason FROM account_suspensions 
                WHERE user_id = p_user_id 
                AND is_active = TRUE 
                ORDER BY created_at DESC LIMIT 1
            )
            ELSE NULL
        END as reason,
        (
            SELECT expires_at FROM account_suspensions 
            WHERE user_id = p_user_id 
            AND is_active = TRUE 
            ORDER BY created_at DESC LIMIT 1
        ) as suspension_expires;
END;
$$ LANGUAGE plpgsql;

-- =====================================================
-- FUNCTION: log_security_event
-- Purpose: Log security events consistently
-- =====================================================
CREATE OR REPLACE FUNCTION log_security_event(
    p_user_id INTEGER,
    p_event_type VARCHAR(100),
    p_description TEXT,
    p_severity VARCHAR(50),
    p_ip_address VARCHAR(45),
    p_metadata JSONB DEFAULT NULL
) RETURNS INTEGER AS $$
DECLARE
    v_log_id INTEGER;
BEGIN
    INSERT INTO security_audit_logs (
        user_id, event_type, event_description, severity, 
        ip_address, metadata
    ) VALUES (
        p_user_id, p_event_type, p_description, p_severity,
        p_ip_address, p_metadata
    ) RETURNING id INTO v_log_id;
    
    RETURN v_log_id;
END;
$$ LANGUAGE plpgsql;

-- =====================================================
-- FUNCTION: cleanup_expired_sessions
-- Purpose: Remove expired sessions (called by scheduler)
-- =====================================================
CREATE OR REPLACE FUNCTION cleanup_expired_sessions()
RETURNS INTEGER AS $$
DECLARE
    v_deleted_count INTEGER;
BEGIN
    UPDATE user_sessions
    SET status = 'expired'
    WHERE status = 'active' AND expires_at < NOW();
    
    GET DIAGNOSTICS v_deleted_count = ROW_COUNT;
    
    RETURN v_deleted_count;
END;
$$ LANGUAGE plpgsql;

-- =====================================================
-- FUNCTION: generate_backup_codes
-- Purpose: Generate backup codes for 2FA
-- =====================================================
CREATE OR REPLACE FUNCTION generate_backup_codes()
RETURNS TEXT[] AS $$
DECLARE
    codes TEXT[];
    i INTEGER;
BEGIN
    codes := ARRAY[]::TEXT[];
    
    FOR i IN 1..10 LOOP
        codes := array_append(codes, 
            upper(substring(md5(random()::text) from 1 for 8))
        );
    END LOOP;
    
    RETURN codes;
END;
$$ LANGUAGE plpgsql;

-- =====================================================
-- FUNCTION: validate_2fa_code
-- Purpose: Validate TOTP or backup code
-- =====================================================
CREATE OR REPLACE FUNCTION validate_2fa_code(
    p_user_id INTEGER,
    p_code VARCHAR(10)
) RETURNS BOOLEAN AS $$
DECLARE
    v_backup_codes JSONB;
    v_code_exists BOOLEAN;
BEGIN
    -- Check if it's a backup code
    SELECT backup_codes INTO v_backup_codes
    FROM two_factor_auth
    WHERE user_id = p_user_id AND is_enabled = TRUE;
    
    IF v_backup_codes IS NOT NULL THEN
        v_code_exists := v_backup_codes ? p_code;
        
        IF v_code_exists THEN
            -- Remove used backup code
            UPDATE two_factor_auth
            SET backup_codes = backup_codes - p_code,
                last_used_at = NOW()
            WHERE user_id = p_user_id;
            
            RETURN TRUE;
        END IF;
    END IF;
    
    -- TOTP validation handled in application layer
    RETURN FALSE;
END;
$$ LANGUAGE plpgsql;

-- =====================================================
-- Add account management columns to users table
-- =====================================================
ALTER TABLE users ADD COLUMN IF NOT EXISTS account_status VARCHAR(50) DEFAULT 'active';
ALTER TABLE users ADD COLUMN IF NOT EXISTS is_locked BOOLEAN DEFAULT FALSE;
ALTER TABLE users ADD COLUMN IF NOT EXISTS locked_at TIMESTAMP;
ALTER TABLE users ADD COLUMN IF NOT EXISTS locked_reason TEXT;
ALTER TABLE users ADD COLUMN IF NOT EXISTS email_verified BOOLEAN DEFAULT FALSE;
ALTER TABLE users ADD COLUMN IF NOT EXISTS email_verified_at TIMESTAMP;
ALTER TABLE users ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMP;
ALTER TABLE users ADD COLUMN IF NOT EXISTS deletion_reason TEXT;
ALTER TABLE users ADD COLUMN IF NOT EXISTS last_login_at TIMESTAMP;
ALTER TABLE users ADD COLUMN IF NOT EXISTS last_login_ip VARCHAR(45);
ALTER TABLE users ADD COLUMN IF NOT EXISTS failed_login_attempts INTEGER DEFAULT 0;
ALTER TABLE users ADD COLUMN IF NOT EXISTS locked_until TIMESTAMP;

CREATE INDEX IF NOT EXISTS idx_users_account_status ON users(account_status);
CREATE INDEX IF NOT EXISTS idx_users_is_locked ON users(is_locked) WHERE is_locked = TRUE;
CREATE INDEX IF NOT EXISTS idx_users_email_verified ON users(email_verified);

-- =====================================================
-- End of Phase 9 Schema
-- =====================================================
