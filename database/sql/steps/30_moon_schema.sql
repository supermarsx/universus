-- Migration 30: Initial moon schema and queue adjustments

CREATE TABLE IF NOT EXISTS moons (
    id SERIAL PRIMARY KEY,
    planet_id INTEGER UNIQUE NOT NULL REFERENCES planets(id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name VARCHAR(100) NOT NULL,
    diameter INTEGER NOT NULL,
    total_fields INTEGER NOT NULL,
    used_fields INTEGER NOT NULL DEFAULT 0,
    metal BIGINT NOT NULL DEFAULT 0,
    crystal BIGINT NOT NULL DEFAULT 0,
    deuterium BIGINT NOT NULL DEFAULT 0,
    lunar_base INTEGER DEFAULT 0,
    sensor_phalanx INTEGER DEFAULT 0,
    jump_gate INTEGER DEFAULT 0,
    moon_robotics_factory INTEGER DEFAULT 0,
    moon_shipyard INTEGER DEFAULT 0,
    moon_nanite_factory INTEGER DEFAULT 0,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

ALTER TABLE construction_queue
    ADD COLUMN IF NOT EXISTS location_type VARCHAR(10) NOT NULL DEFAULT 'planet',
    ADD COLUMN IF NOT EXISTS moon_id INTEGER REFERENCES moons(id) ON DELETE CASCADE;

ALTER TABLE shipyard_queue
    ADD COLUMN IF NOT EXISTS location_type VARCHAR(10) NOT NULL DEFAULT 'planet',
    ADD COLUMN IF NOT EXISTS moon_id INTEGER REFERENCES moons(id) ON DELETE CASCADE;

ALTER TABLE construction_queue ALTER COLUMN planet_id DROP NOT NULL;
ALTER TABLE shipyard_queue ALTER COLUMN planet_id DROP NOT NULL;

CREATE INDEX IF NOT EXISTS idx_construction_queue_moon ON construction_queue(moon_id);
CREATE INDEX IF NOT EXISTS idx_shipyard_queue_moon ON shipyard_queue(moon_id);

CREATE INDEX IF NOT EXISTS idx_moons_user ON moons(user_id);
