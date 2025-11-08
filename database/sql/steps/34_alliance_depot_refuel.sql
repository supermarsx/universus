-- Migration 34: Alliance depot docking & refuel support

CREATE TABLE IF NOT EXISTS alliance_depot_sessions (
    id SERIAL PRIMARY KEY,
    alliance_id INTEGER NOT NULL REFERENCES alliances(id) ON DELETE CASCADE,
    host_planet_id INTEGER NOT NULL REFERENCES planets(id) ON DELETE CASCADE,
    fleet_id INTEGER REFERENCES fleets(id) ON DELETE SET NULL,
    guest_user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    status VARCHAR(20) NOT NULL DEFAULT 'active',
    remaining_duration INTEGER NOT NULL DEFAULT 0,
    deuterium_consumed BIGINT DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMP,
    metadata JSONB
);

CREATE INDEX IF NOT EXISTS idx_alliance_depot_sessions_alliance
    ON alliance_depot_sessions (alliance_id, status, expires_at);
