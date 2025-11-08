-- Migration 33: Alliance announcements and shared communications
-- Adds persistence for alliance announcements and improves circular messaging

CREATE TABLE IF NOT EXISTS alliance_announcements (
    id SERIAL PRIMARY KEY,
    alliance_id INTEGER NOT NULL REFERENCES alliances(id) ON DELETE CASCADE,
    title VARCHAR(100) NOT NULL,
    content TEXT NOT NULL,
    is_pinned BOOLEAN DEFAULT FALSE,
    created_by INTEGER REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    pinned_at TIMESTAMP,
    metadata JSONB
);

CREATE INDEX IF NOT EXISTS idx_alliance_announcements_alliance
    ON alliance_announcements (alliance_id, is_pinned DESC, created_at DESC);

ALTER TABLE alliances
    ADD COLUMN IF NOT EXISTS auto_accept_min_score INTEGER,
    ADD COLUMN IF NOT EXISTS auto_reject_below_score INTEGER,
    ADD COLUMN IF NOT EXISTS auto_application_notes TEXT;
