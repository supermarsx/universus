-- Migration: Add millisecond precision combat tracking tables
-- Description: Enables microsecond-level timing for combat events, fleet movements, and coordinated attacks

-- Fleet movements with microsecond precision
CREATE TABLE IF NOT EXISTS fleet_movements_precise (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    from_planet_id INTEGER NOT NULL REFERENCES planets(id) ON DELETE CASCADE,
    to_planet_id INTEGER NOT NULL REFERENCES planets(id) ON DELETE CASCADE,
    ships JSONB NOT NULL,
    departure_time_micros BIGINT NOT NULL,
    arrival_time_micros BIGINT NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'in_transit',
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT valid_status CHECK (status IN ('in_transit', 'arrived', 'cancelled'))
);

-- Index for efficient queries on arrival time windows
CREATE INDEX IF NOT EXISTS idx_fleet_movements_arrival 
ON fleet_movements_precise(to_planet_id, arrival_time_micros, status)
WHERE status = 'in_transit';

-- Index for user's active fleets
CREATE INDEX IF NOT EXISTS idx_fleet_movements_user 
ON fleet_movements_precise(user_id, status);

-- Combats with microsecond precision tracking
CREATE TABLE IF NOT EXISTS combats_precise (
    id SERIAL PRIMARY KEY,
    planet_id INTEGER NOT NULL REFERENCES planets(id) ON DELETE CASCADE,
    start_time_micros BIGINT NOT NULL,
    end_time_micros BIGINT,
    winner VARCHAR(20),
    final_data JSONB,
    status VARCHAR(20) NOT NULL DEFAULT 'in_progress',
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT valid_winner CHECK (winner IN ('attacker', 'defender', 'draw') OR winner IS NULL),
    CONSTRAINT valid_combat_status CHECK (status IN ('in_progress', 'completed', 'cancelled'))
);

-- Index for active combats
CREATE INDEX IF NOT EXISTS idx_combats_status 
ON combats_precise(status, planet_id);

-- Combat rounds with microsecond precision
CREATE TABLE IF NOT EXISTS combat_rounds_precise (
    id SERIAL PRIMARY KEY,
    combat_id INTEGER NOT NULL REFERENCES combats_precise(id) ON DELETE CASCADE,
    round_number INTEGER NOT NULL,
    round_time_micros BIGINT NOT NULL,
    attacker_ships_remaining JSONB NOT NULL,
    defender_ships_remaining JSONB NOT NULL,
    damage_dealt_attacker INTEGER NOT NULL DEFAULT 0,
    damage_dealt_defender INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT unique_combat_round UNIQUE (combat_id, round_number)
);

-- Index for retrieving rounds by combat
CREATE INDEX IF NOT EXISTS idx_combat_rounds_combat 
ON combat_rounds_precise(combat_id, round_number);

-- Combat events log with microsecond precision
CREATE TABLE IF NOT EXISTS combat_events_precise (
    id SERIAL PRIMARY KEY,
    event_id VARCHAR(100) NOT NULL UNIQUE,
    event_type VARCHAR(50) NOT NULL,
    combat_id INTEGER,
    timestamp_micros BIGINT NOT NULL,
    event_data JSONB,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT valid_event_type CHECK (event_type IN (
        'fleet_departure', 
        'fleet_arrival', 
        'attack_started', 
        'round_executed', 
        'attack_completed'
    ))
);

-- Index for event retrieval by combat
CREATE INDEX IF NOT EXISTS idx_combat_events_combat 
ON combat_events_precise(combat_id, timestamp_micros);

-- Index for event retrieval by timestamp
CREATE INDEX IF NOT EXISTS idx_combat_events_timestamp 
ON combat_events_precise(timestamp_micros DESC);

-- Index for event type queries
CREATE INDEX IF NOT EXISTS idx_combat_events_type 
ON combat_events_precise(event_type, timestamp_micros);

-- Function to convert microseconds to timestamp
CREATE OR REPLACE FUNCTION micros_to_timestamp(micros BIGINT)
RETURNS TIMESTAMP WITH TIME ZONE AS $$
BEGIN
    RETURN TO_TIMESTAMP(micros / 1000000.0);
END;
$$ LANGUAGE plpgsql IMMUTABLE;

-- View for human-readable combat timing
CREATE OR REPLACE VIEW combat_timing_view AS
SELECT 
    c.id AS combat_id,
    c.planet_id,
    micros_to_timestamp(c.start_time_micros) AS start_time,
    micros_to_timestamp(c.end_time_micros) AS end_time,
    CASE 
        WHEN c.end_time_micros IS NOT NULL THEN 
            (c.end_time_micros - c.start_time_micros) / 1000.0
        ELSE NULL
    END AS duration_ms,
    c.winner,
    c.status,
    COUNT(r.id) AS total_rounds,
    c.created_at
FROM combats_precise c
LEFT JOIN combat_rounds_precise r ON c.id = r.combat_id
GROUP BY c.id;

-- View for fleet movement timing
CREATE OR REPLACE VIEW fleet_timing_view AS
SELECT 
    f.id AS fleet_id,
    f.user_id,
    f.from_planet_id,
    f.to_planet_id,
    micros_to_timestamp(f.departure_time_micros) AS departure_time,
    micros_to_timestamp(f.arrival_time_micros) AS arrival_time,
    (f.arrival_time_micros - f.departure_time_micros) / 1000000.0 AS travel_time_seconds,
    f.status,
    f.ships
FROM fleet_movements_precise f;

-- Comment on tables
COMMENT ON TABLE fleet_movements_precise IS 'Tracks fleet movements with microsecond precision for accurate arrival timing';
COMMENT ON TABLE combats_precise IS 'Tracks combat instances with microsecond-level start and end times';
COMMENT ON TABLE combat_rounds_precise IS 'Individual combat rounds with precise timing for each round execution';
COMMENT ON TABLE combat_events_precise IS 'Event log for all combat-related events with microsecond timestamps';

-- Grant permissions (adjust as needed for your user)
-- GRANT ALL ON fleet_movements_precise, combats_precise, combat_rounds_precise, combat_events_precise TO your_app_user;
-- GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO your_app_user;
