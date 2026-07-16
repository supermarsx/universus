-- Lossless, tick-less resource materialization state.

ALTER TABLE planets
    ADD COLUMN IF NOT EXISTS metal_production_remainder DOUBLE PRECISION NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS crystal_production_remainder DOUBLE PRECISION NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS deuterium_production_remainder DOUBLE PRECISION NOT NULL DEFAULT 0;

-- Energy is derived capacity, but advanced planet configurations can exceed
-- the prototype INTEGER range and the repository contract is i64 end-to-end.
ALTER TABLE planets
    ALTER COLUMN energy TYPE BIGINT USING energy::BIGINT;
UPDATE planets SET energy = 0 WHERE energy IS NULL;
ALTER TABLE planets
    ALTER COLUMN energy SET DEFAULT 0,
    ALTER COLUMN energy SET NOT NULL;

-- Legacy values were written as timezone-free timestamps. Universus defines
-- those stored wall-clock values as UTC, independent of the migration
-- session's timezone, then uses absolute instants for all future arithmetic.
DO $$
DECLARE
    timestamp_type TEXT;
BEGIN
    SELECT data_type INTO timestamp_type
    FROM information_schema.columns
    WHERE table_schema = 'public'
      AND table_name = 'planets'
      AND column_name = 'last_resource_update';

    IF timestamp_type = 'timestamp without time zone' THEN
        ALTER TABLE planets
            ALTER COLUMN last_resource_update DROP DEFAULT;
        ALTER TABLE planets
            ALTER COLUMN last_resource_update TYPE TIMESTAMPTZ
            USING last_resource_update AT TIME ZONE 'UTC';
    ELSIF timestamp_type IS DISTINCT FROM 'timestamp with time zone' THEN
        RAISE EXCEPTION 'unexpected planets.last_resource_update type: %', timestamp_type;
    END IF;
END $$;

-- Older installations allowed NULL timestamps. Keep the stockpile intact and
-- begin accrual at migration time rather than inventing an offline interval.
UPDATE planets
SET last_resource_update = CURRENT_TIMESTAMP
WHERE last_resource_update IS NULL;

ALTER TABLE planets
    ALTER COLUMN last_resource_update SET DEFAULT CURRENT_TIMESTAMP,
    ALTER COLUMN last_resource_update SET NOT NULL;

-- `value >= 0 AND value < 1` also rejects NaN and both infinities in
-- PostgreSQL, keeping every remainder safe to fold into the next snapshot.
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'planets_metal_production_remainder_valid'
          AND conrelid = 'planets'::regclass
    ) THEN
        ALTER TABLE planets
            ADD CONSTRAINT planets_metal_production_remainder_valid
            CHECK (
                metal_production_remainder >= 0
                AND metal_production_remainder < 1
            );
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'planets_crystal_production_remainder_valid'
          AND conrelid = 'planets'::regclass
    ) THEN
        ALTER TABLE planets
            ADD CONSTRAINT planets_crystal_production_remainder_valid
            CHECK (
                crystal_production_remainder >= 0
                AND crystal_production_remainder < 1
            );
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'planets_deuterium_production_remainder_valid'
          AND conrelid = 'planets'::regclass
    ) THEN
        ALTER TABLE planets
            ADD CONSTRAINT planets_deuterium_production_remainder_valid
            CHECK (
                deuterium_production_remainder >= 0
                AND deuterium_production_remainder < 1
            );
    END IF;
END $$;
