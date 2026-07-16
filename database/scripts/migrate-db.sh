#!/bin/sh
set -eu

# Canonical Universus PostgreSQL migration runner.
#
# This script is used by Docker initialization, Compose upgrades, and CI. It
# deliberately keeps ordering, locking, checksums, and history in one place so
# every environment exercises the same contract.

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
SQL_DIR=${MIGRATION_SQL_DIR:-"$SCRIPT_DIR/../sql/steps"}
PGUSER=${PGUSER:-${POSTGRES_USER:-postgres}}
PGDATABASE=${PGDATABASE:-${POSTGRES_DB:-postgres}}
PGPASSWORD=${PGPASSWORD:-${POSTGRES_PASSWORD:-}}
MIGRATION_WAIT_SECONDS=${MIGRATION_WAIT_SECONDS:-60}
MIGRATION_LOCK_TIMEOUT_SECONDS=${MIGRATION_LOCK_TIMEOUT_SECONDS:-120}
MIGRATION_ADVISORY_LOCK_KEY=${MIGRATION_ADVISORY_LOCK_KEY:-7614882119}
MIGRATION_RUNNER_ID=${MIGRATION_RUNNER_ID:-"$(date -u +%Y%m%dT%H%M%SZ)-$$"}
PGAPPNAME=${PGAPPNAME:-"universus-migrate:$MIGRATION_RUNNER_ID"}

export PGUSER PGDATABASE PGPASSWORD PGAPPNAME

case "$MIGRATION_WAIT_SECONDS" in
    ''|*[!0-9]*) echo "MIGRATION_WAIT_SECONDS must be a non-negative integer" >&2; exit 2 ;;
esac
case "$MIGRATION_LOCK_TIMEOUT_SECONDS" in
    ''|*[!0-9]*) echo "MIGRATION_LOCK_TIMEOUT_SECONDS must be a non-negative integer" >&2; exit 2 ;;
esac
lock_key_digits=$MIGRATION_ADVISORY_LOCK_KEY
case "$lock_key_digits" in -*) lock_key_digits=${lock_key_digits#-} ;; esac
case "$lock_key_digits" in
    ''|*[!0-9]*) echo "MIGRATION_ADVISORY_LOCK_KEY must be an integer" >&2; exit 2 ;;
esac

if [ ! -d "$SQL_DIR" ]; then
    echo "Migration directory does not exist: $SQL_DIR" >&2
    exit 2
fi
if ! command -v psql >/dev/null 2>&1; then
    echo "psql is required to run database migrations" >&2
    exit 2
fi
if ! command -v pg_isready >/dev/null 2>&1; then
    echo "pg_isready is required to run database migrations" >&2
    exit 2
fi

psql_base() {
    if [ -n "${PGHOST:-}" ] && [ -n "${PGPORT:-}" ]; then
        psql -X -v ON_ERROR_STOP=1 -h "$PGHOST" -p "$PGPORT" -U "$PGUSER" -d "$PGDATABASE" "$@"
    elif [ -n "${PGHOST:-}" ]; then
        psql -X -v ON_ERROR_STOP=1 -h "$PGHOST" -U "$PGUSER" -d "$PGDATABASE" "$@"
    elif [ -n "${PGPORT:-}" ]; then
        psql -X -v ON_ERROR_STOP=1 -p "$PGPORT" -U "$PGUSER" -d "$PGDATABASE" "$@"
    else
        psql -X -v ON_ERROR_STOP=1 -U "$PGUSER" -d "$PGDATABASE" "$@"
    fi
}

ready() {
    if [ -n "${PGHOST:-}" ] && [ -n "${PGPORT:-}" ]; then
        pg_isready -q -h "$PGHOST" -p "$PGPORT" -U "$PGUSER" -d "$PGDATABASE"
    elif [ -n "${PGHOST:-}" ]; then
        pg_isready -q -h "$PGHOST" -U "$PGUSER" -d "$PGDATABASE"
    elif [ -n "${PGPORT:-}" ]; then
        pg_isready -q -p "$PGPORT" -U "$PGUSER" -d "$PGDATABASE"
    else
        pg_isready -q -U "$PGUSER" -d "$PGDATABASE"
    fi
}

checksum_file() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$1" | awk '{print $1}'
    else
        echo "sha256sum or shasum is required to checksum migrations" >&2
        return 1
    fi
}

semantic_step_list() {
    found=
    for file in "$SQL_DIR"/*.sql; do
        if [ -f "$file" ]; then
            found=1
            break
        fi
    done
    [ -n "$found" ] || {
        echo "No migration files found in $SQL_DIR" >&2
        return 1
    }
    for file in "$SQL_DIR"/*.sql; do
        [ -f "$file" ] || continue
        base=${file##*/}
        version=${base%%_*}
        case "$base" in
            [0-9]*_*.sql) ;;
            *) echo "Invalid migration filename (expected <number>_<name>.sql): $base" >&2; return 1 ;;
        esac
        case "$version" in
            ''|*[!0-9]*) echo "Invalid numeric migration version in: $base" >&2; return 1 ;;
        esac
        # The first field is compared numerically; the filename is a stable tie
        # breaker so duplicate versions can be diagnosed deterministically.
        printf '%s\t%s\n' "$version" "$file"
    done | sort -n -k1,1 -k2,2
}

escape_sql_literal() {
    # Migration filenames are validated below, but runner IDs may be supplied
    # by orchestration and still need normal SQL literal escaping.
    printf '%s' "$1" | sed "s/'/''/g"
}

redact_credentials() {
    # psql does not normally print PGPASSWORD, but migration error text can
    # contain arbitrary server output. Never persist or echo the connection
    # credential if a server-side error happens to include it.
    awk '
        BEGIN { secret = ENVIRON["PGPASSWORD"] }
        {
            line = $0
            while (secret != "" && (position = index(line, secret)) != 0) {
                printf "%s[REDACTED]", substr(line, 1, position - 1)
                line = substr(line, position + length(secret))
            }
            print line
        }
    '
}

echo "Waiting for PostgreSQL database $PGDATABASE as $PGUSER${PGHOST:+ at $PGHOST}${PGPORT:+:$PGPORT}"
waited=0
until ready; do
    if [ "$waited" -ge "$MIGRATION_WAIT_SECONDS" ]; then
        echo "PostgreSQL did not become ready within ${MIGRATION_WAIT_SECONDS}s" >&2
        exit 1
    fi
    waited=$((waited + 1))
    sleep 1
done

step_list=$(mktemp)
master_sql=$(mktemp)
runner_log=$(mktemp)
cleanup() {
    rm -f "$step_list" "$master_sql" "$runner_log"
}
trap cleanup EXIT HUP INT TERM

semantic_step_list >"$step_list"

previous_version=
while IFS="$(printf '\t')" read -r raw_version file; do
    version=$(printf '%s' "$raw_version" | sed 's/^0*//')
    [ -n "$version" ] || version=0
    if [ -n "$previous_version" ] && [ "$version" = "$previous_version" ]; then
        echo "Duplicate migration version $version: ${previous_file##*/} and ${file##*/}" >&2
        exit 2
    fi
    previous_version=$version
    previous_file=$file
done <"$step_list"

runner_id_sql=$(escape_sql_literal "$MIGRATION_RUNNER_ID")
host_sql=$(escape_sql_literal "$(hostname 2>/dev/null || printf unknown)")

# Metadata creation is safe before the advisory lock and lets a waiting or
# failed runner remain observable even if it never acquires the migration lock.
# The catalog assertions are intentionally strict: IF NOT EXISTS alone would
# silently accept an older or manually-created table with an incompatible shape.
psql_base -q <<SQL
CREATE TABLE IF NOT EXISTS universus_schema_migrations (
    version BIGINT PRIMARY KEY,
    filename TEXT NOT NULL UNIQUE,
    checksum_sha256 CHAR(64) NOT NULL,
    applied_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    runner_id TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS universus_schema_migration_runs (
    runner_id TEXT PRIMARY KEY,
    host TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('waiting', 'running', 'applied', 'failed')),
    started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    finished_at TIMESTAMPTZ,
    error_message TEXT
);
CREATE TABLE IF NOT EXISTS universus_schema_migration_attempts (
    id BIGSERIAL PRIMARY KEY,
    runner_id TEXT NOT NULL REFERENCES universus_schema_migration_runs(runner_id) ON DELETE CASCADE,
    version BIGINT NOT NULL,
    filename TEXT NOT NULL,
    checksum_sha256 CHAR(64) NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('running', 'applied', 'failed')),
    started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    finished_at TIMESTAMPTZ,
    error_message TEXT
);
CREATE INDEX IF NOT EXISTS idx_universus_migration_attempts_version
    ON universus_schema_migration_attempts(version, started_at DESC);
DO \$metadata_validation\$
DECLARE
    actual_columns TEXT[];
BEGIN
    SELECT array_agg(
        format('%s:%s:%s', column_name, udt_name, is_nullable)
        ORDER BY ordinal_position
    ) INTO actual_columns
    FROM information_schema.columns
    WHERE table_schema = 'public' AND table_name = 'universus_schema_migrations';
    IF actual_columns IS DISTINCT FROM ARRAY[
        'version:int8:NO', 'filename:text:NO', 'checksum_sha256:bpchar:NO',
        'applied_at:timestamptz:NO', 'runner_id:text:NO'
    ] THEN
        RAISE EXCEPTION 'invalid universus_schema_migrations columns: %', actual_columns;
    END IF;

    SELECT array_agg(
        format('%s:%s:%s', column_name, udt_name, is_nullable)
        ORDER BY ordinal_position
    ) INTO actual_columns
    FROM information_schema.columns
    WHERE table_schema = 'public' AND table_name = 'universus_schema_migration_runs';
    IF actual_columns IS DISTINCT FROM ARRAY[
        'runner_id:text:NO', 'host:text:NO', 'status:text:NO',
        'started_at:timestamptz:NO', 'finished_at:timestamptz:YES',
        'error_message:text:YES'
    ] THEN
        RAISE EXCEPTION 'invalid universus_schema_migration_runs columns: %', actual_columns;
    END IF;

    SELECT array_agg(
        format('%s:%s:%s', column_name, udt_name, is_nullable)
        ORDER BY ordinal_position
    ) INTO actual_columns
    FROM information_schema.columns
    WHERE table_schema = 'public' AND table_name = 'universus_schema_migration_attempts';
    IF actual_columns IS DISTINCT FROM ARRAY[
        'id:int8:NO', 'runner_id:text:NO', 'version:int8:NO', 'filename:text:NO',
        'checksum_sha256:bpchar:NO', 'status:text:NO',
        'started_at:timestamptz:NO', 'finished_at:timestamptz:YES',
        'error_message:text:YES'
    ] THEN
        RAISE EXCEPTION 'invalid universus_schema_migration_attempts columns: %', actual_columns;
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = 'universus_schema_migrations'
          AND column_name = 'checksum_sha256'
          AND character_maximum_length = 64
    ) OR NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = 'universus_schema_migrations'
          AND column_name = 'applied_at'
          AND column_default = 'now()'
    ) OR NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = 'universus_schema_migration_runs'
          AND column_name = 'started_at'
          AND column_default = 'now()'
    ) OR NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = 'universus_schema_migration_attempts'
          AND column_name = 'id'
          AND column_default LIKE 'nextval(%'
    ) OR NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = 'universus_schema_migration_attempts'
          AND column_name = 'checksum_sha256'
          AND character_maximum_length = 64
    ) OR NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = 'universus_schema_migration_attempts'
          AND column_name = 'started_at'
          AND column_default = 'now()'
    ) THEN
        RAISE EXCEPTION 'invalid migration metadata lengths, defaults, or sequence';
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'public.universus_schema_migrations'::regclass
          AND contype = 'p' AND pg_get_constraintdef(oid) = 'PRIMARY KEY (version)'
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'public.universus_schema_migrations'::regclass
          AND contype = 'u' AND pg_get_constraintdef(oid) = 'UNIQUE (filename)'
    ) THEN
        RAISE EXCEPTION 'invalid universus_schema_migrations key constraints';
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'public.universus_schema_migration_runs'::regclass
          AND contype = 'p' AND pg_get_constraintdef(oid) = 'PRIMARY KEY (runner_id)'
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'public.universus_schema_migration_runs'::regclass
          AND contype = 'c' AND pg_get_constraintdef(oid) LIKE '%waiting%running%applied%failed%'
    ) THEN
        RAISE EXCEPTION 'invalid universus_schema_migration_runs constraints';
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'public.universus_schema_migration_attempts'::regclass
          AND contype = 'p' AND pg_get_constraintdef(oid) = 'PRIMARY KEY (id)'
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'public.universus_schema_migration_attempts'::regclass
          AND contype = 'f'
          AND confrelid = 'public.universus_schema_migration_runs'::regclass
          AND pg_get_constraintdef(oid) LIKE 'FOREIGN KEY (runner_id)%ON DELETE CASCADE'
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'public.universus_schema_migration_attempts'::regclass
          AND contype = 'c' AND pg_get_constraintdef(oid) LIKE '%running%applied%failed%'
    ) THEN
        RAISE EXCEPTION 'invalid universus_schema_migration_attempts constraints';
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_indexes
        WHERE schemaname = 'public'
          AND tablename = 'universus_schema_migration_attempts'
          AND indexname = 'idx_universus_migration_attempts_version'
          AND indexdef LIKE '%USING btree (version, started_at DESC)'
    ) THEN
        RAISE EXCEPTION 'invalid universus migration-attempt lookup index';
    END IF;
END
\$metadata_validation\$;
INSERT INTO universus_schema_migration_runs (runner_id, host, status)
VALUES ('$runner_id_sql', '$host_sql', 'waiting')
ON CONFLICT (runner_id) DO UPDATE SET
    host = EXCLUDED.host,
    status = 'waiting',
    started_at = now(),
    finished_at = NULL,
    error_message = NULL;
SQL

cat >"$master_sql" <<SQL
\set ON_ERROR_STOP on
SET lock_timeout TO '${MIGRATION_LOCK_TIMEOUT_SECONDS}s';
SELECT pg_advisory_lock(${MIGRATION_ADVISORY_LOCK_KEY});
UPDATE universus_schema_migration_attempts
SET status = 'failed', finished_at = now(),
    error_message = 'migration backend terminated before transaction completion'
WHERE status = 'running' AND runner_id <> '$runner_id_sql';
UPDATE universus_schema_migration_runs
SET status = 'failed', finished_at = now(),
    error_message = 'migration backend terminated while holding the advisory lock'
WHERE status = 'running' AND runner_id <> '$runner_id_sql';
UPDATE universus_schema_migration_runs
SET status = 'running'
WHERE runner_id = '$runner_id_sql';
SQL

while IFS="$(printf '\t')" read -r raw_version file; do
    version=$(printf '%s' "$raw_version" | sed 's/^0*//')
    [ -n "$version" ] || version=0
    base=${file##*/}
    case "$base" in
        *[!A-Za-z0-9._-]*) echo "Unsafe migration filename: $base" >&2; exit 2 ;;
    esac
    checksum=$(checksum_file "$file")
    case "$checksum" in
        [0-9a-fA-F][0-9a-fA-F]*) ;;
        *) echo "Invalid checksum for $base" >&2; exit 2 ;;
    esac
    escaped_file=$(escape_sql_literal "$file")
    cat >>"$master_sql" <<SQL

\echo 'checking migration $version $base'
SELECT
    NOT EXISTS (
        SELECT 1 FROM universus_schema_migrations WHERE version = $version
    ) AS migration_apply,
    EXISTS (
        SELECT 1 FROM universus_schema_migrations
        WHERE version = $version
          AND filename = '$base'
          AND checksum_sha256 = '$checksum'
    ) AS migration_match,
    EXISTS (
        SELECT 1 FROM universus_schema_migrations
        WHERE version = $version
          AND (filename <> '$base' OR checksum_sha256 <> '$checksum')
    ) OR EXISTS (
        SELECT 1 FROM universus_schema_migrations
        WHERE filename = '$base' AND version <> $version
    ) AS migration_drift
\gset
\if :migration_drift
    \echo 'checksum or filename drift detected for migration $version $base'
    SELECT 1 / 0 AS migration_drift_is_fatal;
\endif
\if :migration_apply
    \echo 'applying migration $version $base'
    INSERT INTO universus_schema_migration_attempts
        (runner_id, version, filename, checksum_sha256, status)
    VALUES ('$runner_id_sql', $version, '$base', '$checksum', 'running')
    RETURNING id AS migration_attempt_id
    \gset
    BEGIN;
    \i '$escaped_file'
    INSERT INTO universus_schema_migrations
        (version, filename, checksum_sha256, runner_id)
    VALUES ($version, '$base', '$checksum', '$runner_id_sql');
    UPDATE universus_schema_migration_attempts
    SET status = 'applied', finished_at = now()
    WHERE id = :migration_attempt_id;
    COMMIT;
\else
    \if :migration_match
        \echo 'already applied migration $version $base'
    \endif
\endif
SQL
done <"$step_list"

cat >>"$master_sql" <<SQL

UPDATE universus_schema_migration_runs
SET status = 'applied', finished_at = now(), error_message = NULL
WHERE runner_id = '$runner_id_sql';
SELECT pg_advisory_unlock(${MIGRATION_ADVISORY_LOCK_KEY});
SQL

set +e
psql_base -f "$master_sql" >"$runner_log" 2>&1
result=$?
set -e
redact_credentials <"$runner_log"

if [ "$result" -ne 0 ]; then
    migration_error=$(tail -c 4000 "$runner_log" | redact_credentials)
    psql_base -q -v migration_error="$migration_error" <<SQL || true
UPDATE universus_schema_migration_attempts
SET status = 'failed', finished_at = now(),
    error_message = :'migration_error'
WHERE runner_id = '$runner_id_sql' AND status = 'running';
UPDATE universus_schema_migration_runs
SET status = 'failed', finished_at = now(),
    error_message = :'migration_error'
WHERE runner_id = '$runner_id_sql';
SQL
    echo "Database migrations failed for runner $MIGRATION_RUNNER_ID" >&2
    exit "$result"
fi

psql_base -q -c "SELECT COUNT(*) FROM universus_schema_migrations;" >/dev/null
echo "Database migrations are current for $PGDATABASE (runner $MIGRATION_RUNNER_ID)"
