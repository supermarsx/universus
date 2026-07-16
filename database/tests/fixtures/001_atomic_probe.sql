CREATE TABLE migration_atomic_probe_before (
    id INTEGER PRIMARY KEY
);

SELECT pg_sleep(
    COALESCE(
        NULLIF(
            current_setting('universus.migration_test_pause_seconds', true),
            ''
        )::DOUBLE PRECISION,
        0
    )
);

CREATE TABLE migration_atomic_probe_after (
    id INTEGER PRIMARY KEY
);
