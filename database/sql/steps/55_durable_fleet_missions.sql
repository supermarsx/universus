-- Durable, tenant-safe fleet missions and exact-once mission processing.
--
-- The original `fleets` relation is evolved in place because ACS, realtime
-- tracking, and salvage tables already reference its identifiers. Historical
-- rows cannot be proven complete and are quarantined rather than replayed.

ALTER TABLE users
    ADD COLUMN IF NOT EXISTS vacation_mode BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS vacation_started_at TIMESTAMPTZ;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'users'::regclass
          AND conname = 'users_vacation_timestamp_valid'
    ) THEN
        ALTER TABLE users ADD CONSTRAINT users_vacation_timestamp_valid CHECK (
            vacation_mode OR vacation_started_at IS NULL
        );
    END IF;
END $$;

CREATE UNIQUE INDEX IF NOT EXISTS idx_users_universe_identity
    ON users (universe_id, id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_planets_universe_identity
    ON planets (universe_id, id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_planets_universe_owner_identity
    ON planets (universe_id, user_id, id);

UPDATE planets SET
    small_cargo = COALESCE(small_cargo, 0),
    large_cargo = COALESCE(large_cargo, 0),
    light_fighter = COALESCE(light_fighter, 0),
    heavy_fighter = COALESCE(heavy_fighter, 0),
    cruiser = COALESCE(cruiser, 0),
    battleship = COALESCE(battleship, 0),
    battlecruiser = COALESCE(battlecruiser, 0),
    bomber = COALESCE(bomber, 0),
    destroyer = COALESCE(destroyer, 0),
    deathstar = COALESCE(deathstar, 0),
    recycler = COALESCE(recycler, 0),
    espionage_probe = COALESCE(espionage_probe, 0),
    solar_satellite = COALESCE(solar_satellite, 0),
    colony_ship = COALESCE(colony_ship, 0);
ALTER TABLE planets
    ALTER COLUMN small_cargo SET DEFAULT 0, ALTER COLUMN small_cargo SET NOT NULL,
    ALTER COLUMN large_cargo SET DEFAULT 0, ALTER COLUMN large_cargo SET NOT NULL,
    ALTER COLUMN light_fighter SET DEFAULT 0, ALTER COLUMN light_fighter SET NOT NULL,
    ALTER COLUMN heavy_fighter SET DEFAULT 0, ALTER COLUMN heavy_fighter SET NOT NULL,
    ALTER COLUMN cruiser SET DEFAULT 0, ALTER COLUMN cruiser SET NOT NULL,
    ALTER COLUMN battleship SET DEFAULT 0, ALTER COLUMN battleship SET NOT NULL,
    ALTER COLUMN battlecruiser SET DEFAULT 0, ALTER COLUMN battlecruiser SET NOT NULL,
    ALTER COLUMN bomber SET DEFAULT 0, ALTER COLUMN bomber SET NOT NULL,
    ALTER COLUMN destroyer SET DEFAULT 0, ALTER COLUMN destroyer SET NOT NULL,
    ALTER COLUMN deathstar SET DEFAULT 0, ALTER COLUMN deathstar SET NOT NULL,
    ALTER COLUMN recycler SET DEFAULT 0, ALTER COLUMN recycler SET NOT NULL,
    ALTER COLUMN espionage_probe SET DEFAULT 0, ALTER COLUMN espionage_probe SET NOT NULL,
    ALTER COLUMN solar_satellite SET DEFAULT 0, ALTER COLUMN solar_satellite SET NOT NULL,
    ALTER COLUMN colony_ship SET DEFAULT 0, ALTER COLUMN colony_ship SET NOT NULL;

-- Moons are valid fleet origins and destinations. Bring their tenant and
-- inventory guarantees up to the same contract as planets.
ALTER TABLE moons
    ADD COLUMN IF NOT EXISTS universe_id BIGINT,
    ADD COLUMN IF NOT EXISTS destroyed_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS battlecruiser BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS solar_satellite BIGINT NOT NULL DEFAULT 0;

UPDATE moons AS moon
SET universe_id = planet.universe_id,
    user_id = planet.user_id
FROM planets AS planet
WHERE planet.id = moon.planet_id
  AND (moon.universe_id IS NULL OR moon.user_id IS DISTINCT FROM planet.user_id);

UPDATE moons SET
    small_cargo = COALESCE(small_cargo, 0),
    large_cargo = COALESCE(large_cargo, 0),
    light_fighter = COALESCE(light_fighter, 0),
    heavy_fighter = COALESCE(heavy_fighter, 0),
    cruiser = COALESCE(cruiser, 0),
    battleship = COALESCE(battleship, 0),
    battlecruiser = COALESCE(battlecruiser, 0),
    bomber = COALESCE(bomber, 0),
    destroyer = COALESCE(destroyer, 0),
    deathstar = COALESCE(deathstar, 0),
    recycler = COALESCE(recycler, 0),
    espionage_probe = COALESCE(espionage_probe, 0),
    solar_satellite = COALESCE(solar_satellite, 0),
    colony_ship = COALESCE(colony_ship, 0);

ALTER TABLE moons
    ALTER COLUMN universe_id SET NOT NULL,
    ALTER COLUMN small_cargo TYPE BIGINT USING small_cargo::BIGINT,
    ALTER COLUMN large_cargo TYPE BIGINT USING large_cargo::BIGINT,
    ALTER COLUMN light_fighter TYPE BIGINT USING light_fighter::BIGINT,
    ALTER COLUMN heavy_fighter TYPE BIGINT USING heavy_fighter::BIGINT,
    ALTER COLUMN cruiser TYPE BIGINT USING cruiser::BIGINT,
    ALTER COLUMN battleship TYPE BIGINT USING battleship::BIGINT,
    ALTER COLUMN colony_ship TYPE BIGINT USING colony_ship::BIGINT,
    ALTER COLUMN recycler TYPE BIGINT USING recycler::BIGINT,
    ALTER COLUMN espionage_probe TYPE BIGINT USING espionage_probe::BIGINT,
    ALTER COLUMN bomber TYPE BIGINT USING bomber::BIGINT,
    ALTER COLUMN destroyer TYPE BIGINT USING destroyer::BIGINT,
    ALTER COLUMN deathstar TYPE BIGINT USING deathstar::BIGINT,
    ALTER COLUMN small_cargo SET DEFAULT 0, ALTER COLUMN small_cargo SET NOT NULL,
    ALTER COLUMN large_cargo SET DEFAULT 0, ALTER COLUMN large_cargo SET NOT NULL,
    ALTER COLUMN light_fighter SET DEFAULT 0, ALTER COLUMN light_fighter SET NOT NULL,
    ALTER COLUMN heavy_fighter SET DEFAULT 0, ALTER COLUMN heavy_fighter SET NOT NULL,
    ALTER COLUMN cruiser SET DEFAULT 0, ALTER COLUMN cruiser SET NOT NULL,
    ALTER COLUMN battleship SET DEFAULT 0, ALTER COLUMN battleship SET NOT NULL,
    ALTER COLUMN battlecruiser SET DEFAULT 0, ALTER COLUMN battlecruiser SET NOT NULL,
    ALTER COLUMN bomber SET DEFAULT 0, ALTER COLUMN bomber SET NOT NULL,
    ALTER COLUMN destroyer SET DEFAULT 0, ALTER COLUMN destroyer SET NOT NULL,
    ALTER COLUMN deathstar SET DEFAULT 0, ALTER COLUMN deathstar SET NOT NULL,
    ALTER COLUMN recycler SET DEFAULT 0, ALTER COLUMN recycler SET NOT NULL,
    ALTER COLUMN espionage_probe SET DEFAULT 0, ALTER COLUMN espionage_probe SET NOT NULL,
    ALTER COLUMN solar_satellite SET DEFAULT 0, ALTER COLUMN solar_satellite SET NOT NULL,
    ALTER COLUMN colony_ship SET DEFAULT 0, ALTER COLUMN colony_ship SET NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_moons_universe_identity
    ON moons (universe_id, id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_moons_universe_owner_identity
    ON moons (universe_id, user_id, id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_moons_universe_planet_identity
    ON moons (universe_id, planet_id, id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_moons_universe_owner_planet_identity
    ON moons (universe_id, user_id, planet_id, id);
ALTER TABLE moons DROP CONSTRAINT IF EXISTS moons_planet_id_key;
CREATE UNIQUE INDEX IF NOT EXISTS idx_moons_active_universe_planet
    ON moons (universe_id, planet_id) WHERE destroyed_at IS NULL;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'moons'::regclass
          AND conname = 'moons_universe_id_fkey'
    ) THEN
        ALTER TABLE moons ADD CONSTRAINT moons_universe_id_fkey
            FOREIGN KEY (universe_id) REFERENCES universes(id) ON DELETE RESTRICT;
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'moons'::regclass
          AND conname = 'moons_universe_planet_fkey'
    ) THEN
        ALTER TABLE moons ADD CONSTRAINT moons_universe_planet_fkey
            FOREIGN KEY (universe_id, planet_id)
            REFERENCES planets(universe_id, id) ON DELETE CASCADE;
    END IF;
END $$;

-- Existing debris coordinates were global. They can only be attributed when
-- exactly one universe exists; otherwise preserve their evidence in an
-- explicit quarantine and keep them out of authoritative harvesting.
CREATE TABLE IF NOT EXISTS legacy_debris_fields_quarantine (
    legacy_id INTEGER PRIMARY KEY,
    galaxy INTEGER NOT NULL,
    system INTEGER NOT NULL,
    position INTEGER NOT NULL,
    metal BIGINT NOT NULL,
    crystal BIGINT NOT NULL,
    original_created_at TIMESTAMP,
    quarantine_reason TEXT NOT NULL,
    quarantined_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
ALTER TABLE debris_fields
    ADD COLUMN IF NOT EXISTS universe_id BIGINT,
    ADD COLUMN IF NOT EXISTS deuterium BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ NOT NULL DEFAULT now();
DO $$
DECLARE
    universe_count BIGINT;
    sole_universe BIGINT;
BEGIN
    SELECT COUNT(*), MIN(id) INTO universe_count, sole_universe FROM universes;
    IF universe_count = 1 THEN
        UPDATE debris_fields SET universe_id = sole_universe WHERE universe_id IS NULL;
    ELSIF universe_count > 1 THEN
        INSERT INTO legacy_debris_fields_quarantine
            (legacy_id, galaxy, system, position, metal, crystal,
             original_created_at, quarantine_reason)
        SELECT id, galaxy, system, position, COALESCE(metal, 0), COALESCE(crystal, 0),
               created_at, 'legacy debris has no provable universe tenant'
        FROM debris_fields
        WHERE universe_id IS NULL
        ON CONFLICT (legacy_id) DO NOTHING;
        DELETE FROM debris_fields WHERE universe_id IS NULL;
    ELSE
        RAISE EXCEPTION 'cannot migrate debris fields without an authoritative universe';
    END IF;
END $$;
ALTER TABLE debris_fields ALTER COLUMN universe_id SET NOT NULL;
ALTER TABLE debris_fields DROP CONSTRAINT IF EXISTS debris_fields_galaxy_system_position_key;
CREATE UNIQUE INDEX IF NOT EXISTS idx_debris_fields_universe_coordinates
    ON debris_fields (universe_id, galaxy, system, position);
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'debris_fields'::regclass
          AND conname = 'debris_fields_universe_id_fkey'
    ) THEN
        ALTER TABLE debris_fields ADD CONSTRAINT debris_fields_universe_id_fkey
            FOREIGN KEY (universe_id) REFERENCES universes(id) ON DELETE RESTRICT;
    END IF;
END $$;

-- ACS prototype rows also need an explicit tenant and durable launch link.
ALTER TABLE acs_groups
    ADD COLUMN IF NOT EXISTS universe_id BIGINT,
    ADD COLUMN IF NOT EXISTS target_kind TEXT NOT NULL DEFAULT 'planet',
    ADD COLUMN IF NOT EXISTS status TEXT NOT NULL DEFAULT 'forming',
    ADD COLUMN IF NOT EXISTS rendezvous_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS schedule_generation INTEGER NOT NULL DEFAULT 0;
UPDATE acs_groups AS acs
SET universe_id = users.universe_id
FROM users
WHERE users.id = acs.creator_id AND acs.universe_id IS NULL;
ALTER TABLE acs_groups ALTER COLUMN universe_id SET NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_acs_groups_universe_identity
    ON acs_groups (universe_id, id);
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'acs_groups'::regclass
          AND conname = 'acs_groups_universe_id_fkey'
    ) THEN
        ALTER TABLE acs_groups ADD CONSTRAINT acs_groups_universe_id_fkey
            FOREIGN KEY (universe_id) REFERENCES universes(id) ON DELETE CASCADE;
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'acs_groups'::regclass
          AND conname = 'acs_groups_target_kind_valid'
    ) THEN
        ALTER TABLE acs_groups ADD CONSTRAINT acs_groups_target_kind_valid
            CHECK (target_kind IN ('planet', 'moon'));
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'acs_groups'::regclass
          AND conname = 'acs_groups_status_valid'
    ) THEN
        ALTER TABLE acs_groups ADD CONSTRAINT acs_groups_status_valid
            CHECK (status IN ('forming', 'launched', 'arrived', 'completed', 'cancelled'));
    END IF;
END $$;
ALTER TABLE acs_groups DROP CONSTRAINT IF EXISTS acs_groups_schedule_valid;
ALTER TABLE acs_groups ADD CONSTRAINT acs_groups_schedule_valid CHECK (
    departure_window_end > departure_window_start
    AND departure_window_end <= departure_window_start + INTERVAL '24 hours'
    AND schedule_generation >= 0
    AND (rendezvous_at IS NULL OR rendezvous_at > departure_window_start)
);

ALTER TABLE acs_group_members
    ADD COLUMN IF NOT EXISTS universe_id BIGINT,
    ADD COLUMN IF NOT EXISTS fleet_id INTEGER;
UPDATE acs_group_members AS member
SET universe_id = acs.universe_id
FROM acs_groups AS acs
WHERE acs.id = member.group_id AND member.universe_id IS NULL;
ALTER TABLE acs_group_members ALTER COLUMN universe_id SET NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_acs_group_members_assigned_fleet
    ON acs_group_members (universe_id, fleet_id) WHERE fleet_id IS NOT NULL;

-- Mission header. Legacy columns remain populated for compatibility, while
-- these columns are the sole authoritative processing contract.
ALTER TABLE fleets
    ADD COLUMN IF NOT EXISTS universe_id BIGINT,
    ADD COLUMN IF NOT EXISTS command_id TEXT,
    ADD COLUMN IF NOT EXISTS request_fingerprint BYTEA,
    ADD COLUMN IF NOT EXISTS resolution_seed BYTEA,
    ADD COLUMN IF NOT EXISTS origin_kind TEXT NOT NULL DEFAULT 'planet',
    ADD COLUMN IF NOT EXISTS origin_moon_id INTEGER,
    ADD COLUMN IF NOT EXISTS origin_galaxy INTEGER,
    ADD COLUMN IF NOT EXISTS origin_system INTEGER,
    ADD COLUMN IF NOT EXISTS origin_position INTEGER,
    ADD COLUMN IF NOT EXISTS target_kind TEXT NOT NULL DEFAULT 'planet',
    ADD COLUMN IF NOT EXISTS target_planet_id INTEGER,
    ADD COLUMN IF NOT EXISTS target_moon_id INTEGER,
    ADD COLUMN IF NOT EXISTS departed_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS unadjusted_arrives_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS arrives_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS returns_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS phase_due_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS distance INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS fleet_speed BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS duration_seconds BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS hold_seconds BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS movement_fuel_consumed BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS holding_fuel_consumed BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS fuel_consumed BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS cargo_capacity BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS launched_cargo_metal BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS launched_cargo_crystal BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS launched_cargo_deuterium BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS applied_universe_speed INTEGER NOT NULL DEFAULT 1,
    ADD COLUMN IF NOT EXISTS applied_speed_percent INTEGER NOT NULL DEFAULT 100,
    ADD COLUMN IF NOT EXISTS applied_fuel_multiplier_milli INTEGER NOT NULL DEFAULT 1000,
    ADD COLUMN IF NOT EXISTS applied_cargo_multiplier_milli INTEGER NOT NULL DEFAULT 1000,
    ADD COLUMN IF NOT EXISTS applied_max_galaxies INTEGER NOT NULL DEFAULT 9,
    ADD COLUMN IF NOT EXISTS applied_max_systems INTEGER NOT NULL DEFAULT 499,
    ADD COLUMN IF NOT EXISTS applied_max_positions INTEGER NOT NULL DEFAULT 15,
    ADD COLUMN IF NOT EXISTS recalled_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS arrival_resolved_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS hold_resolved_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS return_resolved_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS terminal_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS acs_schedule_generation INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS phase_generation INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS claim_attempt BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS resolution_owner TEXT,
    ADD COLUMN IF NOT EXISTS resolution_expires_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS result JSONB NOT NULL DEFAULT '{}'::jsonb;

-- Quarantine only rows that predate this contract. A repeated migration must
-- never quarantine a mission created by the authoritative repository.
UPDATE fleets
SET status = 'legacy_unclassified'
WHERE request_fingerprint IS NULL;

UPDATE fleets AS fleet
SET universe_id = users.universe_id
FROM users
WHERE users.id = fleet.user_id AND fleet.universe_id IS NULL;

UPDATE fleets AS fleet
SET origin_galaxy = planet.galaxy,
    origin_system = planet.system,
    origin_position = planet.position
FROM planets AS planet
WHERE planet.id = fleet.origin_planet_id
  AND (fleet.origin_galaxy IS NULL OR fleet.origin_system IS NULL OR fleet.origin_position IS NULL);

UPDATE fleets
SET command_id = COALESCE(command_id, 'legacy-' || id::TEXT),
    request_fingerprint = COALESCE(
        request_fingerprint,
        decode(md5('legacy-request:' || id::TEXT) || md5('legacy-request-2:' || id::TEXT), 'hex')
    ),
    resolution_seed = COALESCE(
        resolution_seed,
        decode(md5('legacy-seed:' || id::TEXT) || md5('legacy-seed-2:' || id::TEXT), 'hex')
    ),
    departed_at = COALESCE(departed_at, departure_time AT TIME ZONE 'UTC'),
    unadjusted_arrives_at = COALESCE(
        unadjusted_arrives_at,
        arrival_time AT TIME ZONE 'UTC'
    ),
    arrives_at = COALESCE(arrives_at, arrival_time AT TIME ZONE 'UTC'),
    returns_at = COALESCE(
        returns_at,
        COALESCE(return_time, arrival_time) AT TIME ZONE 'UTC'
    ),
    phase_due_at = COALESCE(
        phase_due_at,
        COALESCE(return_time, arrival_time) AT TIME ZONE 'UTC'
    ),
    movement_fuel_consumed = CASE
        WHEN request_fingerprint IS NULL THEN COALESCE(fuel_consumed, 0)
        ELSE movement_fuel_consumed
    END,
    launched_cargo_metal = COALESCE(cargo_metal, 0),
    launched_cargo_crystal = COALESCE(cargo_crystal, 0),
    launched_cargo_deuterium = COALESCE(cargo_deuterium, 0)
WHERE command_id IS NULL
   OR request_fingerprint IS NULL
   OR resolution_seed IS NULL
   OR departed_at IS NULL
   OR arrives_at IS NULL
   OR returns_at IS NULL
   OR phase_due_at IS NULL;

ALTER TABLE fleets
    ALTER COLUMN universe_id SET NOT NULL,
    ALTER COLUMN command_id SET NOT NULL,
    ALTER COLUMN request_fingerprint SET NOT NULL,
    ALTER COLUMN resolution_seed SET NOT NULL,
    ALTER COLUMN origin_galaxy SET NOT NULL,
    ALTER COLUMN origin_system SET NOT NULL,
    ALTER COLUMN origin_position SET NOT NULL,
    ALTER COLUMN departed_at SET NOT NULL,
    ALTER COLUMN unadjusted_arrives_at SET NOT NULL,
    ALTER COLUMN arrives_at SET NOT NULL,
    ALTER COLUMN returns_at SET NOT NULL,
    ALTER COLUMN phase_due_at SET NOT NULL,
    ALTER COLUMN cargo_metal SET DEFAULT 0,
    ALTER COLUMN cargo_crystal SET DEFAULT 0,
    ALTER COLUMN cargo_deuterium SET DEFAULT 0,
    ALTER COLUMN status SET DEFAULT 'outbound',
    ALTER COLUMN status SET NOT NULL;

UPDATE fleets
SET cargo_metal = COALESCE(cargo_metal, 0),
    cargo_crystal = COALESCE(cargo_crystal, 0),
    cargo_deuterium = COALESCE(cargo_deuterium, 0);
ALTER TABLE fleets
    ALTER COLUMN cargo_metal SET NOT NULL,
    ALTER COLUMN cargo_crystal SET NOT NULL,
    ALTER COLUMN cargo_deuterium SET NOT NULL;

ALTER TABLE fleets DROP CONSTRAINT IF EXISTS fleets_mission_type_check;
ALTER TABLE fleets DROP CONSTRAINT IF EXISTS fleets_status_valid;
ALTER TABLE fleets DROP CONSTRAINT IF EXISTS fleets_source_target_valid;
ALTER TABLE fleets DROP CONSTRAINT IF EXISTS fleets_launch_facts_valid;
ALTER TABLE fleets DROP CONSTRAINT IF EXISTS fleets_fingerprint_seed_valid;

ALTER TABLE fleets ADD CONSTRAINT fleets_status_valid CHECK (
    status IN ('outbound', 'holding', 'returning', 'completed', 'destroyed', 'legacy_unclassified')
);
ALTER TABLE fleets ADD CONSTRAINT fleets_source_target_valid CHECK (
    status = 'legacy_unclassified' OR (
        origin_kind IN ('planet', 'moon')
        AND (
            (origin_kind = 'planet' AND origin_moon_id IS NULL)
            OR (origin_kind = 'moon' AND origin_moon_id IS NOT NULL)
        )
        AND target_kind IN ('planet', 'moon', 'debris', 'empty_coordinate', 'expedition_slot')
        AND (
            (target_kind = 'planet' AND target_planet_id IS NOT NULL AND target_moon_id IS NULL)
            OR (target_kind = 'moon' AND target_planet_id IS NOT NULL AND target_moon_id IS NOT NULL)
            OR (target_kind IN ('debris', 'empty_coordinate', 'expedition_slot')
                AND target_planet_id IS NULL AND target_moon_id IS NULL)
        )
        AND (
            (mission_type IN ('attack', 'espionage', 'acs_attack', 'acs_defend', 'acs_join')
                AND target_kind IN ('planet', 'moon'))
            OR (mission_type IN ('transport', 'deploy') AND target_kind IN ('planet', 'moon'))
            OR (mission_type = 'colonize' AND target_kind = 'empty_coordinate')
            OR (mission_type = 'harvest' AND target_kind = 'debris')
            OR (mission_type = 'expedition' AND target_kind = 'expedition_slot')
            OR (mission_type = 'destroy' AND target_kind = 'moon')
        )
    )
);
ALTER TABLE fleets ADD CONSTRAINT fleets_launch_facts_valid CHECK (
    status = 'legacy_unclassified' OR (
        distance > 0 AND fleet_speed > 0 AND duration_seconds > 0
        AND hold_seconds >= 0
        AND movement_fuel_consumed >= 0 AND holding_fuel_consumed >= 0
        AND fuel_consumed = movement_fuel_consumed + holding_fuel_consumed
        AND cargo_capacity >= 0
        AND cargo_metal >= 0 AND cargo_crystal >= 0 AND cargo_deuterium >= 0
        AND launched_cargo_metal >= 0
        AND launched_cargo_crystal >= 0
        AND launched_cargo_deuterium >= 0
        AND applied_universe_speed BETWEEN 1 AND 1000
        AND applied_speed_percent BETWEEN 10 AND 100
        AND applied_fuel_multiplier_milli BETWEEN 1 AND 100000
        AND applied_cargo_multiplier_milli BETWEEN 1 AND 100000
        AND (
            (mission_type = 'acs_defend' AND hold_seconds BETWEEN 60 AND 172800)
            OR (mission_type <> 'acs_defend' AND hold_seconds = 0)
        )
        AND origin_galaxy BETWEEN 1 AND applied_max_galaxies
        AND target_galaxy BETWEEN 1 AND applied_max_galaxies
        AND origin_system BETWEEN 1 AND applied_max_systems
        AND target_system BETWEEN 1 AND applied_max_systems
        AND origin_position BETWEEN 1 AND applied_max_positions
        AND target_position BETWEEN 1 AND applied_max_positions + 1
        AND arrives_at > departed_at
        AND unadjusted_arrives_at > departed_at
        AND arrives_at >= unadjusted_arrives_at
        AND returns_at >= arrives_at
        AND phase_due_at >= departed_at
        AND phase_generation >= 0 AND claim_attempt >= 0
    )
);
ALTER TABLE fleets ADD CONSTRAINT fleets_fingerprint_seed_valid CHECK (
    octet_length(request_fingerprint) = 32 AND octet_length(resolution_seed) = 32
);
ALTER TABLE fleets DROP CONSTRAINT IF EXISTS fleets_bounded_payload_valid;
ALTER TABLE fleets DROP CONSTRAINT IF EXISTS fleets_transition_shape_valid;
ALTER TABLE fleets ADD CONSTRAINT fleets_bounded_payload_valid CHECK (
    char_length(command_id) BETWEEN 1 AND 128
    AND (resolution_owner IS NULL OR char_length(resolution_owner) BETWEEN 1 AND 160)
    AND jsonb_typeof(ships) = 'object'
    AND jsonb_typeof(result) = 'object'
);
ALTER TABLE fleets ADD CONSTRAINT fleets_transition_shape_valid CHECK (
    status = 'legacy_unclassified' OR (
        (status IN ('completed', 'destroyed')) = (terminal_at IS NOT NULL)
        AND (return_resolved_at IS NULL OR status = 'completed')
        AND (hold_resolved_at IS NULL
             OR (arrival_resolved_at IS NOT NULL AND status IN ('returning', 'completed', 'destroyed')))
        AND (recalled_at IS NULL OR status IN ('returning', 'completed', 'destroyed'))
        AND (status <> 'outbound'
             OR (arrival_resolved_at IS NULL AND return_resolved_at IS NULL
                 AND terminal_at IS NULL AND recalled_at IS NULL))
        AND (status <> 'returning'
             OR (arrival_resolved_at IS NOT NULL OR recalled_at IS NOT NULL))
        AND (status <> 'holding'
             OR (mission_type = 'acs_defend' AND hold_seconds > 0
                 AND arrival_resolved_at IS NOT NULL AND hold_resolved_at IS NULL
                 AND return_resolved_at IS NULL AND terminal_at IS NULL
                 AND recalled_at IS NULL))
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_fleets_universe_command
    ON fleets (universe_id, user_id, command_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_fleets_universe_identity
    ON fleets (universe_id, id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_fleets_universe_owner_identity
    ON fleets (universe_id, user_id, id);
CREATE INDEX IF NOT EXISTS idx_fleets_user_active
    ON fleets (universe_id, user_id, origin_planet_id, status, id)
    WHERE status IN ('outbound', 'holding', 'returning');
CREATE INDEX IF NOT EXISTS idx_fleets_due_phase
    ON fleets (phase_due_at, id)
    WHERE status IN ('outbound', 'holding', 'returning');
CREATE INDEX IF NOT EXISTS idx_fleets_expired_resolution_lease
    ON fleets (resolution_expires_at, phase_due_at, id)
    WHERE status IN ('outbound', 'holding', 'returning');

ALTER TABLE fleets DROP CONSTRAINT IF EXISTS fleets_universe_origin_moon_fkey;
ALTER TABLE fleets DROP CONSTRAINT IF EXISTS fleets_universe_target_moon_fkey;
ALTER TABLE fleets DROP CONSTRAINT IF EXISTS fleets_universe_acs_group_fkey;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'fleets'::regclass AND conname = 'fleets_universe_user_fkey'
    ) THEN
        ALTER TABLE fleets ADD CONSTRAINT fleets_universe_user_fkey
            FOREIGN KEY (universe_id, user_id)
            REFERENCES users(universe_id, id) ON DELETE CASCADE;
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'fleets'::regclass AND conname = 'fleets_universe_origin_fkey'
    ) THEN
        ALTER TABLE fleets ADD CONSTRAINT fleets_universe_origin_fkey
            FOREIGN KEY (universe_id, user_id, origin_planet_id)
            REFERENCES planets(universe_id, user_id, id) ON DELETE RESTRICT;
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'fleets'::regclass AND conname = 'fleets_universe_origin_moon_fkey'
    ) THEN
        ALTER TABLE fleets ADD CONSTRAINT fleets_universe_origin_moon_fkey
            FOREIGN KEY (universe_id, user_id, origin_planet_id, origin_moon_id)
            REFERENCES moons(universe_id, user_id, planet_id, id) ON DELETE RESTRICT;
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'fleets'::regclass AND conname = 'fleets_universe_target_planet_fkey'
    ) THEN
        ALTER TABLE fleets ADD CONSTRAINT fleets_universe_target_planet_fkey
            FOREIGN KEY (universe_id, target_planet_id)
            REFERENCES planets(universe_id, id) ON DELETE RESTRICT;
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'fleets'::regclass AND conname = 'fleets_universe_target_moon_fkey'
    ) THEN
        ALTER TABLE fleets ADD CONSTRAINT fleets_universe_target_moon_fkey
            FOREIGN KEY (universe_id, target_planet_id, target_moon_id)
            REFERENCES moons(universe_id, planet_id, id) ON DELETE RESTRICT;
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'fleets'::regclass AND conname = 'fleets_universe_acs_group_fkey'
    ) THEN
        ALTER TABLE fleets ADD CONSTRAINT fleets_universe_acs_group_fkey
            FOREIGN KEY (universe_id, acs_group_id)
            REFERENCES acs_groups(universe_id, id) ON DELETE RESTRICT;
    END IF;
END $$;

CREATE TABLE IF NOT EXISTS fleet_mission_ships (
    fleet_id INTEGER NOT NULL REFERENCES fleets(id) ON DELETE CASCADE,
    ship_type TEXT NOT NULL,
    initial_count BIGINT NOT NULL CHECK (initial_count > 0 AND initial_count <= 1000000000),
    current_count BIGINT NOT NULL CHECK (current_count >= 0 AND current_count <= initial_count),
    PRIMARY KEY (fleet_id, ship_type),
    CONSTRAINT fleet_mission_ships_type_valid CHECK (ship_type IN (
        'small_cargo', 'large_cargo', 'light_fighter', 'heavy_fighter',
        'cruiser', 'battleship', 'battlecruiser', 'bomber', 'destroyer',
        'deathstar', 'recycler', 'espionage_probe', 'colony_ship'
    ))
);

CREATE TABLE IF NOT EXISTS fleet_mission_events (
    id BIGSERIAL PRIMARY KEY,
    universe_id BIGINT NOT NULL REFERENCES universes(id) ON DELETE RESTRICT,
    fleet_id INTEGER NOT NULL REFERENCES fleets(id) ON DELETE CASCADE,
    sequence INTEGER NOT NULL CHECK (sequence > 0),
    event_key TEXT NOT NULL,
    event_type TEXT NOT NULL,
    phase_generation INTEGER NOT NULL CHECK (phase_generation >= 0),
    actor_user_id INTEGER,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (fleet_id, sequence),
    UNIQUE (fleet_id, event_key)
);
ALTER TABLE fleet_mission_events DROP CONSTRAINT IF EXISTS fleet_mission_events_bounded_valid;
ALTER TABLE fleet_mission_events ADD CONSTRAINT fleet_mission_events_bounded_valid CHECK (
    char_length(event_key) BETWEEN 1 AND 160
    AND char_length(event_type) BETWEEN 1 AND 80
    AND jsonb_typeof(payload) = 'object'
);
CREATE INDEX IF NOT EXISTS idx_fleet_mission_events_tenant_fleet
    ON fleet_mission_events (universe_id, fleet_id, sequence);
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'fleet_mission_events'::regclass
          AND conname = 'fleet_mission_events_universe_fleet_fkey'
    ) THEN
        ALTER TABLE fleet_mission_events
            ADD CONSTRAINT fleet_mission_events_universe_fleet_fkey
            FOREIGN KEY (universe_id, fleet_id)
            REFERENCES fleets(universe_id, id) ON DELETE CASCADE;
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'fleet_mission_events'::regclass
          AND conname = 'fleet_mission_events_universe_actor_fkey'
    ) THEN
        ALTER TABLE fleet_mission_events
            ADD CONSTRAINT fleet_mission_events_universe_actor_fkey
            FOREIGN KEY (universe_id, actor_user_id)
            REFERENCES users(universe_id, id) ON DELETE SET NULL (actor_user_id);
    END IF;
END $$;

CREATE OR REPLACE FUNCTION reject_fleet_mission_event_mutation()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'fleet mission events are append-only';
END $$;
DROP TRIGGER IF EXISTS fleet_mission_events_append_only ON fleet_mission_events;
CREATE TRIGGER fleet_mission_events_append_only
BEFORE UPDATE OR DELETE ON fleet_mission_events
FOR EACH ROW EXECUTE FUNCTION reject_fleet_mission_event_mutation();

CREATE OR REPLACE FUNCTION protect_fleet_launch_facts()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.universe_id IS DISTINCT FROM NEW.universe_id
       OR OLD.user_id IS DISTINCT FROM NEW.user_id
       OR OLD.command_id IS DISTINCT FROM NEW.command_id
       OR OLD.request_fingerprint IS DISTINCT FROM NEW.request_fingerprint
       OR OLD.resolution_seed IS DISTINCT FROM NEW.resolution_seed
       OR OLD.mission_type IS DISTINCT FROM NEW.mission_type
       OR OLD.ships IS DISTINCT FROM NEW.ships
       OR OLD.origin_kind IS DISTINCT FROM NEW.origin_kind
       OR OLD.origin_planet_id IS DISTINCT FROM NEW.origin_planet_id
       OR OLD.origin_moon_id IS DISTINCT FROM NEW.origin_moon_id
       OR OLD.origin_galaxy IS DISTINCT FROM NEW.origin_galaxy
       OR OLD.origin_system IS DISTINCT FROM NEW.origin_system
       OR OLD.origin_position IS DISTINCT FROM NEW.origin_position
       OR OLD.target_kind IS DISTINCT FROM NEW.target_kind
       OR OLD.target_planet_id IS DISTINCT FROM NEW.target_planet_id
       OR OLD.target_moon_id IS DISTINCT FROM NEW.target_moon_id
       OR OLD.target_galaxy IS DISTINCT FROM NEW.target_galaxy
       OR OLD.target_system IS DISTINCT FROM NEW.target_system
       OR OLD.target_position IS DISTINCT FROM NEW.target_position
       OR OLD.acs_group_id IS DISTINCT FROM NEW.acs_group_id
       OR OLD.distance IS DISTINCT FROM NEW.distance
       OR OLD.fleet_speed IS DISTINCT FROM NEW.fleet_speed
       OR OLD.duration_seconds IS DISTINCT FROM NEW.duration_seconds
       OR OLD.hold_seconds IS DISTINCT FROM NEW.hold_seconds
       OR OLD.movement_fuel_consumed IS DISTINCT FROM NEW.movement_fuel_consumed
       OR OLD.holding_fuel_consumed IS DISTINCT FROM NEW.holding_fuel_consumed
       OR OLD.fuel_consumed IS DISTINCT FROM NEW.fuel_consumed
       OR OLD.cargo_capacity IS DISTINCT FROM NEW.cargo_capacity
       OR OLD.launched_cargo_metal IS DISTINCT FROM NEW.launched_cargo_metal
       OR OLD.launched_cargo_crystal IS DISTINCT FROM NEW.launched_cargo_crystal
       OR OLD.launched_cargo_deuterium IS DISTINCT FROM NEW.launched_cargo_deuterium
       OR OLD.departed_at IS DISTINCT FROM NEW.departed_at
       OR OLD.unadjusted_arrives_at IS DISTINCT FROM NEW.unadjusted_arrives_at
       OR OLD.applied_universe_speed IS DISTINCT FROM NEW.applied_universe_speed
       OR OLD.applied_speed_percent IS DISTINCT FROM NEW.applied_speed_percent
       OR OLD.applied_fuel_multiplier_milli IS DISTINCT FROM NEW.applied_fuel_multiplier_milli
       OR OLD.applied_cargo_multiplier_milli IS DISTINCT FROM NEW.applied_cargo_multiplier_milli
       OR OLD.applied_max_galaxies IS DISTINCT FROM NEW.applied_max_galaxies
       OR OLD.applied_max_systems IS DISTINCT FROM NEW.applied_max_systems
       OR OLD.applied_max_positions IS DISTINCT FROM NEW.applied_max_positions
    THEN
        RAISE EXCEPTION 'fleet launch facts are immutable';
    END IF;
    IF OLD.arrives_at IS DISTINCT FROM NEW.arrives_at THEN
        IF OLD.status <> 'outbound'
           OR OLD.mission_type NOT IN ('acs_attack', 'acs_join')
           OR OLD.acs_group_id IS NULL
           OR OLD.arrival_resolved_at IS NOT NULL
           OR OLD.recalled_at IS NOT NULL
           OR NEW.arrives_at < OLD.arrives_at
           OR NEW.arrival_time IS DISTINCT FROM (NEW.arrives_at AT TIME ZONE 'UTC')
           OR NEW.phase_due_at IS DISTINCT FROM NEW.arrives_at
           OR NEW.returns_at IS DISTINCT FROM
                (NEW.arrives_at + make_interval(secs => NEW.duration_seconds::DOUBLE PRECISION))
           OR NEW.return_time IS DISTINCT FROM (NEW.returns_at AT TIME ZONE 'UTC')
           OR NOT EXISTS (
                SELECT 1 FROM acs_groups AS acs
                WHERE acs.universe_id = OLD.universe_id
                  AND acs.id = OLD.acs_group_id
                  AND acs.status = 'forming'
                  AND acs.rendezvous_at = NEW.arrives_at
                  AND acs.schedule_generation = NEW.acs_schedule_generation
           )
        THEN
            RAISE EXCEPTION 'ACS rendezvous may only move forward to the locked group schedule';
        END IF;
    END IF;
    IF NEW.phase_generation < OLD.phase_generation
       OR NEW.claim_attempt < OLD.claim_attempt
       OR NEW.acs_schedule_generation < OLD.acs_schedule_generation
       OR (OLD.arrival_resolved_at IS NOT NULL
           AND OLD.arrival_resolved_at IS DISTINCT FROM NEW.arrival_resolved_at)
       OR (OLD.hold_resolved_at IS NOT NULL
           AND OLD.hold_resolved_at IS DISTINCT FROM NEW.hold_resolved_at)
       OR (OLD.return_resolved_at IS NOT NULL
           AND OLD.return_resolved_at IS DISTINCT FROM NEW.return_resolved_at)
       OR (OLD.recalled_at IS NOT NULL
           AND OLD.recalled_at IS DISTINCT FROM NEW.recalled_at)
       OR (OLD.terminal_at IS NOT NULL
           AND OLD.terminal_at IS DISTINCT FROM NEW.terminal_at)
       OR (OLD.status = 'outbound'
           AND NEW.status NOT IN ('outbound', 'holding', 'returning', 'completed', 'destroyed'))
       OR (OLD.status = 'holding'
           AND NEW.status NOT IN ('holding', 'returning', 'completed', 'destroyed'))
       OR (OLD.status = 'returning'
           AND NEW.status NOT IN ('returning', 'completed', 'destroyed'))
       OR (OLD.status IN ('completed', 'destroyed', 'legacy_unclassified')
           AND OLD.status IS DISTINCT FROM NEW.status)
    THEN
        RAISE EXCEPTION 'fleet lifecycle state cannot regress';
    END IF;
    RETURN NEW;
END $$;
DROP TRIGGER IF EXISTS fleets_launch_facts_immutable ON fleets;
CREATE TRIGGER fleets_launch_facts_immutable
BEFORE UPDATE ON fleets
FOR EACH ROW EXECUTE FUNCTION protect_fleet_launch_facts();

CREATE OR REPLACE FUNCTION protect_fleet_ship_identity()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.fleet_id IS DISTINCT FROM NEW.fleet_id
       OR OLD.ship_type IS DISTINCT FROM NEW.ship_type
       OR OLD.initial_count IS DISTINCT FROM NEW.initial_count
       OR NEW.current_count > OLD.current_count
    THEN
        RAISE EXCEPTION 'fleet launch composition is immutable';
    END IF;
    RETURN NEW;
END $$;
DROP TRIGGER IF EXISTS fleet_mission_ships_identity_immutable ON fleet_mission_ships;
CREATE TRIGGER fleet_mission_ships_identity_immutable
BEFORE UPDATE ON fleet_mission_ships
FOR EACH ROW EXECUTE FUNCTION protect_fleet_ship_identity();

CREATE OR REPLACE FUNCTION protect_acs_member_assignment()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.universe_id IS DISTINCT FROM NEW.universe_id
       OR OLD.group_id IS DISTINCT FROM NEW.group_id
       OR OLD.user_id IS DISTINCT FROM NEW.user_id
       OR (OLD.fleet_id IS NOT NULL AND OLD.fleet_id IS DISTINCT FROM NEW.fleet_id)
    THEN
        RAISE EXCEPTION 'ACS membership launch assignment is immutable';
    END IF;
    RETURN NEW;
END $$;
DROP TRIGGER IF EXISTS acs_group_member_assignment_immutable ON acs_group_members;
CREATE TRIGGER acs_group_member_assignment_immutable
BEFORE UPDATE ON acs_group_members
FOR EACH ROW EXECUTE FUNCTION protect_acs_member_assignment();

ALTER TABLE acs_group_members DROP CONSTRAINT IF EXISTS acs_group_members_fleet_id_fkey;
ALTER TABLE acs_group_members DROP CONSTRAINT IF EXISTS acs_group_members_universe_group_fkey;
ALTER TABLE acs_group_members DROP CONSTRAINT IF EXISTS acs_group_members_universe_user_fleet_fkey;
CREATE UNIQUE INDEX IF NOT EXISTS idx_acs_group_members_universe_identity
    ON acs_group_members (universe_id, id);
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'acs_group_members'::regclass
          AND conname = 'acs_group_members_universe_group_fkey'
    ) THEN
        ALTER TABLE acs_group_members
            ADD CONSTRAINT acs_group_members_universe_group_fkey
            FOREIGN KEY (universe_id, group_id)
            REFERENCES acs_groups(universe_id, id) ON DELETE CASCADE;
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'acs_group_members'::regclass
          AND conname = 'acs_group_members_universe_user_fleet_fkey'
    ) THEN
        ALTER TABLE acs_group_members
            ADD CONSTRAINT acs_group_members_universe_user_fleet_fkey
            FOREIGN KEY (universe_id, user_id, fleet_id)
            REFERENCES fleets(universe_id, user_id, id) ON DELETE SET NULL (fleet_id);
    END IF;
END $$;

ALTER TABLE combat_reports
    ADD COLUMN IF NOT EXISTS universe_id BIGINT,
    ADD COLUMN IF NOT EXISTS fleet_id INTEGER,
    ADD COLUMN IF NOT EXISTS target_kind TEXT,
    ADD COLUMN IF NOT EXISTS target_planet_id INTEGER,
    ADD COLUMN IF NOT EXISTS target_moon_id INTEGER;
UPDATE combat_reports AS report
SET universe_id = users.universe_id
FROM users
WHERE users.id = report.attacker_id AND report.universe_id IS NULL;
ALTER TABLE combat_reports ALTER COLUMN universe_id SET NOT NULL;
CREATE INDEX IF NOT EXISTS idx_combat_reports_universe_fleet
    ON combat_reports (universe_id, fleet_id, battle_time DESC);
ALTER TABLE combat_reports DROP CONSTRAINT IF EXISTS combat_reports_fleet_id_fkey;
ALTER TABLE combat_reports DROP CONSTRAINT IF EXISTS combat_reports_universe_target_moon_fkey;
ALTER TABLE combat_reports DROP CONSTRAINT IF EXISTS combat_reports_target_valid;
ALTER TABLE combat_reports ADD CONSTRAINT combat_reports_target_valid CHECK (
    (target_kind IS NULL AND target_planet_id IS NULL AND target_moon_id IS NULL)
    OR (target_kind = 'planet' AND target_planet_id IS NOT NULL AND target_moon_id IS NULL)
    OR (target_kind = 'moon' AND target_planet_id IS NOT NULL AND target_moon_id IS NOT NULL)
);
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'combat_reports'::regclass
          AND conname = 'combat_reports_universe_id_fkey'
    ) THEN
        ALTER TABLE combat_reports ADD CONSTRAINT combat_reports_universe_id_fkey
            FOREIGN KEY (universe_id) REFERENCES universes(id) ON DELETE RESTRICT;
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'combat_reports'::regclass
          AND conname = 'combat_reports_fleet_id_fkey'
    ) THEN
        ALTER TABLE combat_reports ADD CONSTRAINT combat_reports_fleet_id_fkey
            FOREIGN KEY (universe_id, fleet_id)
            REFERENCES fleets(universe_id, id) ON DELETE SET NULL (fleet_id);
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'combat_reports'::regclass
          AND conname = 'combat_reports_universe_attacker_fkey'
    ) THEN
        ALTER TABLE combat_reports ADD CONSTRAINT combat_reports_universe_attacker_fkey
            FOREIGN KEY (universe_id, attacker_id)
            REFERENCES users(universe_id, id) ON DELETE RESTRICT;
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'combat_reports'::regclass
          AND conname = 'combat_reports_universe_defender_fkey'
    ) THEN
        ALTER TABLE combat_reports ADD CONSTRAINT combat_reports_universe_defender_fkey
            FOREIGN KEY (universe_id, defender_id)
            REFERENCES users(universe_id, id) ON DELETE RESTRICT;
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'combat_reports'::regclass
          AND conname = 'combat_reports_universe_target_planet_fkey'
    ) THEN
        ALTER TABLE combat_reports ADD CONSTRAINT combat_reports_universe_target_planet_fkey
            FOREIGN KEY (universe_id, target_planet_id)
            REFERENCES planets(universe_id, id) ON DELETE RESTRICT;
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'combat_reports'::regclass
          AND conname = 'combat_reports_universe_target_moon_fkey'
    ) THEN
        ALTER TABLE combat_reports ADD CONSTRAINT combat_reports_universe_target_moon_fkey
            FOREIGN KEY (universe_id, target_planet_id, target_moon_id)
            REFERENCES moons(universe_id, planet_id, id) ON DELETE RESTRICT;
    END IF;
END $$;

-- Persisted settings consumed by the repository. Existing operator values win.
INSERT INTO config_parameters
    (category_id, parameter_key, parameter_name, description, data_type,
     current_value, default_value, min_value, max_value)
SELECT category_id, 'fleet.max_active_per_planet', 'Active fleet slots per planet',
       'Maximum simultaneous outbound/returning fleets from one planet or moon',
       'number', '16', '16', 1, 1000
FROM config_categories WHERE category_name = 'ships'
ON CONFLICT (parameter_key) DO NOTHING;

INSERT INTO config_parameters
    (category_id, parameter_key, parameter_name, description, data_type,
     current_value, default_value, min_value, max_value)
SELECT category_id, 'combat.noob_protection_enabled', 'New-player protection',
       'Prevent attacks outside the configured player-score ratio',
       'boolean', 'true', 'true', NULL, NULL
FROM config_categories WHERE category_name = 'combat'
ON CONFLICT (parameter_key) DO NOTHING;

INSERT INTO config_parameters
    (category_id, parameter_key, parameter_name, description, data_type,
     current_value, default_value, min_value, max_value)
SELECT category_id, 'combat.noob_protection_points', 'New-player protection points',
       'Score threshold for new-player protection',
       'number', '5000', '5000', 0, 1000000000000
FROM config_categories WHERE category_name = 'combat'
ON CONFLICT (parameter_key) DO NOTHING;

INSERT INTO config_parameters
    (category_id, parameter_key, parameter_name, description, data_type,
     current_value, default_value, min_value, max_value)
SELECT category_id, 'combat.noob_protection_multiplier', 'New-player protection ratio',
       'Maximum protected score ratio between attacker and defender',
       'number', '5.0', '5.0', 1, 100
FROM config_categories WHERE category_name = 'combat'
ON CONFLICT (parameter_key) DO NOTHING;

INSERT INTO notification_types
    (type_name, category, description, default_priority, icon)
VALUES
    ('fleet_arrived', 'fleet', 'Your fleet has arrived at destination', 2, 'fleet'),
    ('fleet_returned', 'fleet', 'Your fleet has returned home', 2, 'fleet'),
    ('under_attack', 'combat', 'Your planet or moon is under attack', 5, 'alert'),
    ('combat_report', 'combat', 'Combat report available', 3, 'combat')
ON CONFLICT (type_name) DO NOTHING;

DO $$
DECLARE
    missing_keys TEXT;
BEGIN
    SELECT string_agg(required.key, ', ' ORDER BY required.key)
    INTO missing_keys
    FROM (VALUES
        ('fleet.max_active_per_planet'),
        ('combat.noob_protection_enabled'),
        ('combat.noob_protection_points'),
        ('combat.noob_protection_multiplier')
    ) AS required(key)
    WHERE NOT EXISTS (
        SELECT 1 FROM config_parameters WHERE parameter_key = required.key
    );
    IF missing_keys IS NOT NULL THEN
        RAISE EXCEPTION 'fleet configuration categories are incomplete; missing keys: %', missing_keys;
    END IF;
END $$;
