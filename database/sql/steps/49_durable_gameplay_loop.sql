-- Durable, idempotent non-fleet gameplay queues and complete planet inventory.

ALTER TABLE planets
    ADD COLUMN IF NOT EXISTS terraformer INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS space_dock INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS battlecruiser INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS solar_satellite INTEGER NOT NULL DEFAULT 0;

-- Ship counts and queued quantities are deliberately BIGINT end-to-end. This
-- avoids a silent overflow boundary between the public i64 gameplay contract
-- and the original INTEGER prototype columns.
ALTER TABLE planets
    ALTER COLUMN small_cargo TYPE BIGINT USING small_cargo::BIGINT,
    ALTER COLUMN large_cargo TYPE BIGINT USING large_cargo::BIGINT,
    ALTER COLUMN light_fighter TYPE BIGINT USING light_fighter::BIGINT,
    ALTER COLUMN heavy_fighter TYPE BIGINT USING heavy_fighter::BIGINT,
    ALTER COLUMN cruiser TYPE BIGINT USING cruiser::BIGINT,
    ALTER COLUMN battleship TYPE BIGINT USING battleship::BIGINT,
    ALTER COLUMN battlecruiser TYPE BIGINT USING battlecruiser::BIGINT,
    ALTER COLUMN bomber TYPE BIGINT USING bomber::BIGINT,
    ALTER COLUMN destroyer TYPE BIGINT USING destroyer::BIGINT,
    ALTER COLUMN deathstar TYPE BIGINT USING deathstar::BIGINT,
    ALTER COLUMN recycler TYPE BIGINT USING recycler::BIGINT,
    ALTER COLUMN espionage_probe TYPE BIGINT USING espionage_probe::BIGINT,
    ALTER COLUMN solar_satellite TYPE BIGINT USING solar_satellite::BIGINT,
    ALTER COLUMN colony_ship TYPE BIGINT USING colony_ship::BIGINT;

ALTER TABLE shipyard_queue
    ALTER COLUMN quantity TYPE BIGINT USING quantity::BIGINT;

ALTER TABLE construction_queue
    ADD COLUMN IF NOT EXISTS energy_required BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS status VARCHAR(20),
    ADD COLUMN IF NOT EXISTS completed_at TIMESTAMP,
    ADD COLUMN IF NOT EXISTS processing_started_at TIMESTAMP;

ALTER TABLE research_queue
    ADD COLUMN IF NOT EXISTS energy_required BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS status VARCHAR(20),
    ADD COLUMN IF NOT EXISTS completed_at TIMESTAMP,
    ADD COLUMN IF NOT EXISTS processing_started_at TIMESTAMP;

ALTER TABLE shipyard_queue
    ADD COLUMN IF NOT EXISTS energy_required BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS status VARCHAR(20),
    ADD COLUMN IF NOT EXISTS completed_at TIMESTAMP,
    ADD COLUMN IF NOT EXISTS processing_started_at TIMESTAMP;

-- Legacy queue rows did not record completion/cancellation state, so they
-- cannot be safely replayed or called cancelled. Preserve structurally valid
-- rows as an explicit quarantined state for an operator/domain-specific
-- reconciler; terminal-fail only malformed rows that can never be applied.
-- Rows with an existing explicit status are preserved unchanged.
UPDATE construction_queue
SET status = CASE
        WHEN level <= 0
          OR location_type NOT IN ('planet', 'moon')
          OR (location_type = 'planet' AND (planet_id IS NULL OR moon_id IS NOT NULL))
          OR (location_type = 'moon' AND (moon_id IS NULL OR planet_id IS NOT NULL))
        THEN 'failed'
        ELSE 'legacy_unclassified'
    END,
    completed_at = CASE
        WHEN level <= 0
          OR location_type NOT IN ('planet', 'moon')
          OR (location_type = 'planet' AND (planet_id IS NULL OR moon_id IS NOT NULL))
          OR (location_type = 'moon' AND (moon_id IS NULL OR planet_id IS NOT NULL))
        THEN COALESCE(completed_at, now())
        ELSE completed_at
    END
WHERE status IS NULL;

-- There was no durable lease owner in the prototype. Quarantine abandoned
-- external `processing` claims after fifteen minutes so they cannot block a
-- queue forever or be replayed as if completion were known.
UPDATE construction_queue
SET status = 'stale_processing', completed_at = COALESCE(completed_at, now())
WHERE status = 'processing'
  AND COALESCE(processing_started_at, start_time) <= now() - interval '15 minutes';
UPDATE research_queue
SET status = 'stale_processing', completed_at = COALESCE(completed_at, now())
WHERE status = 'processing'
  AND COALESCE(processing_started_at, start_time) <= now() - interval '15 minutes';
UPDATE shipyard_queue
SET status = 'stale_processing', completed_at = COALESCE(completed_at, now())
WHERE status = 'processing'
  AND COALESCE(processing_started_at, start_time) <= now() - interval '15 minutes';
UPDATE research_queue
SET status = CASE WHEN level > 0 THEN 'legacy_unclassified' ELSE 'failed' END,
    completed_at = CASE
        WHEN level > 0 THEN completed_at
        ELSE COALESCE(completed_at, now())
    END
WHERE status IS NULL;
UPDATE shipyard_queue
SET status = CASE
        WHEN quantity <= 0
          OR location_type NOT IN ('planet', 'moon')
          OR (location_type = 'planet' AND (planet_id IS NULL OR moon_id IS NOT NULL))
          OR (location_type = 'moon' AND (moon_id IS NULL OR planet_id IS NOT NULL))
        THEN 'failed'
        ELSE 'legacy_unclassified'
    END,
    completed_at = CASE
        WHEN quantity <= 0
          OR location_type NOT IN ('planet', 'moon')
          OR (location_type = 'planet' AND (planet_id IS NULL OR moon_id IS NOT NULL))
          OR (location_type = 'moon' AND (moon_id IS NULL OR planet_id IS NOT NULL))
        THEN COALESCE(completed_at, now())
        ELSE completed_at
    END
WHERE status IS NULL;

ALTER TABLE construction_queue
    ALTER COLUMN status SET DEFAULT 'queued',
    ALTER COLUMN status SET NOT NULL;
ALTER TABLE research_queue
    ALTER COLUMN status SET DEFAULT 'queued',
    ALTER COLUMN status SET NOT NULL;
ALTER TABLE shipyard_queue
    ALTER COLUMN status SET DEFAULT 'queued',
    ALTER COLUMN status SET NOT NULL;

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'construction_queue_level_positive') THEN
        ALTER TABLE construction_queue
            ADD CONSTRAINT construction_queue_level_positive
            CHECK (status NOT IN ('queued', 'processing') OR level > 0);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'research_queue_level_positive') THEN
        ALTER TABLE research_queue
            ADD CONSTRAINT research_queue_level_positive
            CHECK (status NOT IN ('queued', 'processing') OR level > 0);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'shipyard_queue_quantity_positive') THEN
        ALTER TABLE shipyard_queue
            ADD CONSTRAINT shipyard_queue_quantity_positive
            CHECK (status NOT IN ('queued', 'processing') OR quantity > 0);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'construction_queue_status_valid') THEN
        ALTER TABLE construction_queue
            ADD CONSTRAINT construction_queue_status_valid
            CHECK (status IN (
                'queued', 'processing', 'completed', 'failed', 'cancelled',
                'legacy_unclassified', 'stale_processing'
            ));
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'research_queue_status_valid') THEN
        ALTER TABLE research_queue
            ADD CONSTRAINT research_queue_status_valid
            CHECK (status IN (
                'queued', 'processing', 'completed', 'failed', 'cancelled',
                'legacy_unclassified', 'stale_processing'
            ));
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'shipyard_queue_status_valid') THEN
        ALTER TABLE shipyard_queue
            ADD CONSTRAINT shipyard_queue_status_valid
            CHECK (status IN (
                'queued', 'processing', 'completed', 'failed', 'cancelled',
                'legacy_unclassified', 'stale_processing'
            ));
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'construction_queue_active_location_valid') THEN
        ALTER TABLE construction_queue
            ADD CONSTRAINT construction_queue_active_location_valid CHECK (
                status NOT IN ('queued', 'processing') OR
                (location_type = 'planet' AND planet_id IS NOT NULL AND moon_id IS NULL) OR
                (location_type = 'moon' AND moon_id IS NOT NULL AND planet_id IS NULL)
            );
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'shipyard_queue_active_location_valid') THEN
        ALTER TABLE shipyard_queue
            ADD CONSTRAINT shipyard_queue_active_location_valid CHECK (
                status NOT IN ('queued', 'processing') OR
                (location_type = 'planet' AND planet_id IS NOT NULL AND moon_id IS NULL) OR
                (location_type = 'moon' AND moon_id IS NOT NULL AND planet_id IS NULL)
            );
    END IF;
END $$;

CREATE INDEX IF NOT EXISTS idx_planets_user_id
    ON planets (user_id, id);

CREATE UNIQUE INDEX IF NOT EXISTS uq_construction_queue_active_planet
    ON construction_queue (planet_id)
    WHERE location_type = 'planet' AND planet_id IS NOT NULL
      AND status IN ('queued', 'processing');

CREATE UNIQUE INDEX IF NOT EXISTS uq_research_queue_active_user
    ON research_queue (user_id)
    WHERE status IN ('queued', 'processing');

CREATE UNIQUE INDEX IF NOT EXISTS uq_shipyard_queue_active_planet
    ON shipyard_queue (planet_id)
    WHERE location_type = 'planet' AND planet_id IS NOT NULL
      AND status IN ('queued', 'processing');

-- Moon construction remains a separate gameplay subsystem, but it receives
-- the same one-active-order invariant. The planet-only repository and worker
-- never claim these rows.
CREATE UNIQUE INDEX IF NOT EXISTS uq_construction_queue_active_moon
    ON construction_queue (moon_id)
    WHERE location_type = 'moon' AND moon_id IS NOT NULL
      AND status IN ('queued', 'processing');

CREATE UNIQUE INDEX IF NOT EXISTS uq_shipyard_queue_active_moon
    ON shipyard_queue (moon_id)
    WHERE location_type = 'moon' AND moon_id IS NOT NULL
      AND status IN ('queued', 'processing');

CREATE INDEX IF NOT EXISTS idx_construction_queue_due
    ON construction_queue (end_time, id)
    WHERE location_type = 'planet' AND planet_id IS NOT NULL AND status = 'queued';

CREATE INDEX IF NOT EXISTS idx_research_queue_due
    ON research_queue (end_time, id)
    WHERE status = 'queued';

CREATE INDEX IF NOT EXISTS idx_shipyard_queue_due
    ON shipyard_queue (end_time, id)
    WHERE location_type = 'planet' AND planet_id IS NOT NULL AND status = 'queued';
