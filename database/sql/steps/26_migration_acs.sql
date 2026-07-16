-- Migration 26: Alliance Combat System (ACS) support

CREATE TABLE IF NOT EXISTS acs_groups (
    id SERIAL PRIMARY KEY,
    alliance_id INTEGER REFERENCES alliances(id) ON DELETE CASCADE,
    creator_id INTEGER REFERENCES users(id) ON DELETE CASCADE,
    mission_type VARCHAR(20) NOT NULL DEFAULT 'attack',
    target_galaxy INTEGER NOT NULL,
    target_system INTEGER NOT NULL,
    target_position INTEGER NOT NULL,
    departure_window_start TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    departure_window_end TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP + INTERVAL '1 hour'),
    notes TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS acs_group_members (
    id SERIAL PRIMARY KEY,
    group_id INTEGER NOT NULL REFERENCES acs_groups(id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    planet_id INTEGER REFERENCES planets(id) ON DELETE SET NULL,
    joined_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(group_id, user_id)
);

ALTER TABLE fleets
    ADD COLUMN IF NOT EXISTS acs_group_id INTEGER REFERENCES acs_groups(id);

CREATE INDEX IF NOT EXISTS idx_fleets_acs_group ON fleets(acs_group_id);
