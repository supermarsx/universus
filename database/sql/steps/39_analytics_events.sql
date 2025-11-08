-- Migration 39: Analytics events + usage tables

CREATE TABLE IF NOT EXISTS analytics_events (
    id BIGSERIAL PRIMARY KEY,
    user_id INTEGER REFERENCES users(id) ON DELETE SET NULL,
    session_id UUID,
    event_type VARCHAR(100) NOT NULL,
    event_properties JSONB DEFAULT '{}'::jsonb,
    user_agent TEXT,
    ip_address INET,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_analytics_events_type ON analytics_events(event_type);
CREATE INDEX IF NOT EXISTS idx_analytics_events_created_at ON analytics_events(created_at);

CREATE TABLE IF NOT EXISTS analytics_daily_usage (
    event_date DATE NOT NULL,
    event_type VARCHAR(100) NOT NULL,
    total_events INTEGER NOT NULL DEFAULT 0,
    unique_users INTEGER NOT NULL DEFAULT 0,
    last_calculated TIMESTAMPTZ DEFAULT NOW(),
    PRIMARY KEY (event_date, event_type)
);
