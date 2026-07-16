-- Migration 101: Moon destruction mission scheduling/logging
CREATE TABLE IF NOT EXISTS rip_attack (
    id SERIAL PRIMARY KEY,
    attacker_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    source_moon_id INTEGER NOT NULL REFERENCES moons(id) ON DELETE CASCADE,
    target_moon_id INTEGER NOT NULL REFERENCES moons(id) ON DELETE CASCADE,
    num_rips INTEGER NOT NULL CHECK (num_rips > 0),
    p_destroy NUMERIC(8,4),
    p_lose NUMERIC(8,4),
    deathstars_lost INTEGER DEFAULT 0,
    success BOOLEAN,
    status VARCHAR(20) NOT NULL DEFAULT 'scheduled',
    scheduled_for TIMESTAMP NOT NULL,
    resolved_ts TIMESTAMP,
    error_message TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT rip_attack_status_check CHECK (status IN ('scheduled', 'resolved', 'failed'))
);

CREATE INDEX IF NOT EXISTS idx_rip_attack_status_eta ON rip_attack(status, scheduled_for);
CREATE INDEX IF NOT EXISTS idx_rip_attack_attacker ON rip_attack(attacker_id, created_at DESC);
