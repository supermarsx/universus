-- Phase 10: SMS / WhatsApp Validation Schema
-- Extends the account management system with phone verification support
-- via SMS, WhatsApp or custom API providers.

-- =====================================================
-- USERS TABLE ENHANCEMENTS
-- =====================================================
ALTER TABLE users
    ADD COLUMN IF NOT EXISTS phone_number VARCHAR(32),
    ADD COLUMN IF NOT EXISTS phone_verified BOOLEAN DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS phone_verified_at TIMESTAMP;

CREATE INDEX IF NOT EXISTS idx_users_phone_verified
    ON users(phone_verified)
    WHERE phone_verified IS TRUE;

-- =====================================================
-- TABLE: sms_verifications
-- Purpose: Track SMS / WhatsApp verification codes
-- =====================================================
CREATE TABLE IF NOT EXISTS sms_verifications (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    phone_number VARCHAR(32) NOT NULL,
    channel VARCHAR(32) NOT NULL DEFAULT 'sms',
    verification_code VARCHAR(16) NOT NULL,
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

CREATE INDEX IF NOT EXISTS idx_sms_verifications_user_id ON sms_verifications(user_id);
CREATE INDEX IF NOT EXISTS idx_sms_verifications_phone ON sms_verifications(phone_number);
CREATE INDEX IF NOT EXISTS idx_sms_verifications_status ON sms_verifications(status);
CREATE INDEX IF NOT EXISTS idx_sms_verifications_expires ON sms_verifications(expires_at);

-- =====================================================
-- TABLE: sms_service_settings
-- Purpose: Admin-configurable SMS service connection/settings
-- =====================================================
CREATE TABLE IF NOT EXISTS sms_service_settings (
    id SERIAL PRIMARY KEY,
    service_url TEXT NOT NULL,
    api_key TEXT,
    default_channel VARCHAR(64) NOT NULL DEFAULT 'sms_twilio',
    fallback_channels TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
    updated_by INTEGER REFERENCES users(id),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW()
);

INSERT INTO sms_service_settings (service_url, api_key, default_channel, fallback_channels)
SELECT 'http://localhost:4700', NULL, 'sms_twilio', ARRAY['telegram','custom_http']
WHERE NOT EXISTS (SELECT 1 FROM sms_service_settings);

-- =====================================================
-- PERMISSIONS: SMS service configuration
-- =====================================================
INSERT INTO permissions (name, description) VALUES
    ('notifications:sms:read', 'Read SMS service configuration'),
    ('notifications:sms:write', 'Modify SMS service configuration')
ON CONFLICT (name) DO NOTHING;

INSERT INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r, permissions p 
WHERE r.name = 'super_game_master' AND p.name IN ('notifications:sms:read', 'notifications:sms:write')
ON CONFLICT DO NOTHING;

INSERT INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r, permissions p 
WHERE r.name = 'game_master' AND p.name IN ('notifications:sms:read')
ON CONFLICT DO NOTHING;
