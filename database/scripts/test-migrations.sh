#!/bin/sh
set -eu

# End-to-end migration durability gate. It targets a running PostgreSQL 16
# instance and exercises the exact runner used by Docker initialization,
# Compose upgrades, and CI.

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/../.." && pwd)
RUNNER="$SCRIPT_DIR/migrate-db.sh"
STEPS_DIR="$REPO_ROOT/database/sql/steps"
FIXTURES_DIR="$REPO_ROOT/database/tests/fixtures"
ADMIN_DATABASE=${MIGRATION_TEST_ADMIN_DATABASE:-postgres}
ADMIN_USER=${PGUSER:-postgres}
ADMIN_PASSWORD=${PGPASSWORD:-}
PREFIX="universus_migration_test_$$"
ROLE_NAME="${PREFIX}_role"
ROLE_PASSWORD=credential-leak-probe-secret
TMP_ROOT=$(mktemp -d)
BACKGROUND_PIDS=

FRESH_DB="${PREFIX}_fresh"
LEGACY_DB="${PREFIX}_legacy"
CONCURRENT_DB="${PREFIX}_concurrent"
TIMEOUT_DB="${PREFIX}_timeout"
CRASH_DB="${PREFIX}_crash"
METADATA_DB="${PREFIX}_metadata"
REDACTION_DB="${PREFIX}_redaction"
DATABASES="$FRESH_DB $LEGACY_DB $CONCURRENT_DB $TIMEOUT_DB $CRASH_DB $METADATA_DB $REDACTION_DB"

fail() {
    echo "migration test failed: $*" >&2
    exit 1
}

admin_psql() {
    PGUSER=$ADMIN_USER PGPASSWORD=$ADMIN_PASSWORD PGDATABASE=$ADMIN_DATABASE \
        psql -X -v ON_ERROR_STOP=1 "$@"
}

database_psql() {
    database=$1
    shift
    PGUSER=$ADMIN_USER PGPASSWORD=$ADMIN_PASSWORD PGDATABASE=$database \
        psql -X -v ON_ERROR_STOP=1 "$@"
}

query() {
    database=$1
    statement=$2
    database_psql "$database" -Atq -c "$statement"
}

create_database() {
    database=$1
    owner=${2:-$ADMIN_USER}
    admin_psql -q -c "CREATE DATABASE $database OWNER $owner;"
}

run_migrations() {
    database=$1
    runner_id=$2
    sql_dir=${3:-$STEPS_DIR}
    PGDATABASE=$database \
    MIGRATION_RUNNER_ID=$runner_id \
    MIGRATION_SQL_DIR=$sql_dir \
        "$RUNNER"
}

wait_for_value() {
    database=$1
    statement=$2
    expected=$3
    attempts=${4:-40}
    value=
    count=0
    while [ "$count" -lt "$attempts" ]; do
        value=$(query "$database" "$statement" 2>/dev/null || true)
        [ "$value" = "$expected" ] && return 0
        count=$((count + 1))
        sleep 0.25
    done
    echo "last observed value: $value" >&2
    return 1
}

cleanup() {
    for pid in $BACKGROUND_PIDS; do
        kill "$pid" 2>/dev/null || true
        wait "$pid" 2>/dev/null || true
    done
    for database in $DATABASES; do
        PGUSER=$ADMIN_USER PGPASSWORD=$ADMIN_PASSWORD PGDATABASE=$ADMIN_DATABASE \
            dropdb --if-exists --force "$database" >/dev/null 2>&1 || true
    done
    admin_psql -q -c "DROP ROLE IF EXISTS $ROLE_NAME;" >/dev/null 2>&1 || true
    case "$TMP_ROOT" in
        /tmp/*|/var/tmp/*) rm -rf -- "$TMP_ROOT" ;;
        *) echo "refusing to remove unexpected temp path: $TMP_ROOT" >&2 ;;
    esac
}
trap cleanup EXIT HUP INT TERM

for command in psql dropdb awk sort paste cp; do
    command -v "$command" >/dev/null 2>&1 || fail "$command is required"
done
[ -x "$RUNNER" ] || fail "canonical runner is not executable: $RUNNER"
[ -f "$FIXTURES_DIR/001_atomic_probe.sql" ] || fail "atomic probe fixture is missing"
[ -f "$FIXTURES_DIR/001_failure_probe.sql" ] || fail "failure probe fixture is missing"

expected_versions=$(
    for file in "$STEPS_DIR"/*.sql; do
        name=${file##*/}
        version=${name%%_*}
        version=$(printf '%s' "$version" | sed 's/^0*//')
        [ -n "$version" ] || version=0
        printf '%s\n' "$version"
    done | sort -n | paste -sd, -
)
expected_count=$(printf '%s' "$expected_versions" | awk -F, '{print NF}')

echo "[1/7] fresh PostgreSQL migration and repeat"
create_database "$FRESH_DB"
run_migrations "$FRESH_DB" fresh-proof >"$TMP_ROOT/fresh.log" 2>&1
actual_versions=$(query "$FRESH_DB" "SELECT string_agg(version::TEXT, ',' ORDER BY version) FROM universus_schema_migrations;")
[ "$actual_versions" = "$expected_versions" ] || fail "fresh migration versions were not complete or numeric"
run_migrations "$FRESH_DB" repeat-proof >"$TMP_ROOT/repeat.log" 2>&1
[ "$(query "$FRESH_DB" "SELECT count(*) FROM universus_schema_migrations;")" = "$expected_count" ] || fail "repeat changed migration history"
[ "$(query "$FRESH_DB" "SELECT count(*) FROM universus_schema_migration_runs WHERE status = 'applied';")" = 2 ] || fail "repeat run was not recorded"

echo "[2/7] checksum drift and metadata shape rejection"
drift_dir="$TMP_ROOT/drift-steps"
cp -R "$STEPS_DIR" "$drift_dir"
printf '\n-- checksum drift probe\n' >>"$drift_dir/01_core_schema.sql"
if run_migrations "$FRESH_DB" drift-proof "$drift_dir" >"$TMP_ROOT/drift.log" 2>&1; then
    fail "checksum drift was accepted"
fi
grep -q 'drift detected' "$TMP_ROOT/drift.log" || fail "checksum drift was not visible"
[ "$(query "$FRESH_DB" "SELECT status FROM universus_schema_migration_runs WHERE runner_id = 'drift-proof';")" = failed ] || fail "drift run was not recorded as failed"

create_database "$METADATA_DB"
database_psql "$METADATA_DB" -q -c 'CREATE TABLE universus_schema_migrations(version BIGINT PRIMARY KEY);'
if run_migrations "$METADATA_DB" metadata-proof >"$TMP_ROOT/metadata.log" 2>&1; then
    fail "malformed metadata table was accepted"
fi
grep -q 'invalid universus_schema_migrations columns' "$TMP_ROOT/metadata.log" || fail "metadata mismatch was not visible"

echo "[3/7] no-history existing-volume upgrade and data preservation"
create_database "$LEGACY_DB"
database_psql "$LEGACY_DB" -q -f "$STEPS_DIR/01_core_schema.sql"
database_psql "$LEGACY_DB" -q -c "INSERT INTO users (username, email, password_hash) VALUES ('legacy_keeper', 'legacy@example.test', 'legacy-password-hash');"
run_migrations "$LEGACY_DB" legacy-upgrade-proof >"$TMP_ROOT/legacy.log" 2>&1
run_migrations "$LEGACY_DB" legacy-repeat-proof >"$TMP_ROOT/legacy-repeat.log" 2>&1
[ "$(query "$LEGACY_DB" "SELECT count(*) FROM users WHERE username = 'legacy_keeper';")" = 1 ] || fail "legacy data was not preserved"
[ "$(query "$LEGACY_DB" "SELECT count(*) FROM universus_schema_migrations;")" = "$expected_count" ] || fail "legacy upgrade did not record the complete chain"

atomic_dir="$TMP_ROOT/atomic-steps"
mkdir "$atomic_dir"
cp "$FIXTURES_DIR/001_atomic_probe.sql" "$atomic_dir/001_atomic_probe.sql"

echo "[4/7] concurrent runners serialize; waiter skips"
create_database "$CONCURRENT_DB"
(
    PGOPTIONS='-c universus.migration_test_pause_seconds=3' \
        run_migrations "$CONCURRENT_DB" concurrent-applier "$atomic_dir"
) >"$TMP_ROOT/concurrent-a.log" 2>&1 &
concurrent_a=$!
BACKGROUND_PIDS="$BACKGROUND_PIDS $concurrent_a"
wait_for_value "$CONCURRENT_DB" "SELECT status FROM universus_schema_migration_runs WHERE runner_id = 'concurrent-applier';" running || fail "concurrent applier did not start"
run_migrations "$CONCURRENT_DB" concurrent-waiter "$atomic_dir" >"$TMP_ROOT/concurrent-b.log" 2>&1 &
concurrent_b=$!
BACKGROUND_PIDS="$BACKGROUND_PIDS $concurrent_b"
wait_for_value "$CONCURRENT_DB" "SELECT status FROM universus_schema_migration_runs WHERE runner_id = 'concurrent-waiter';" waiting || fail "concurrent waiter was not visible"
wait "$concurrent_a" || fail "concurrent applier failed"
wait "$concurrent_b" || fail "concurrent waiter failed"
[ "$(query "$CONCURRENT_DB" "SELECT count(*) FROM universus_schema_migrations;")" = 1 ] || fail "concurrent runners applied twice"
[ "$(query "$CONCURRENT_DB" "SELECT count(*) FROM universus_schema_migration_attempts;")" = 1 ] || fail "waiter created a duplicate attempt"
[ "$(query "$CONCURRENT_DB" "SELECT count(*) FROM universus_schema_migration_runs WHERE status = 'applied';")" = 2 ] || fail "concurrent runs were not both successful"

echo "[5/7] advisory-lock timeout is visible and recoverable"
create_database "$TIMEOUT_DB"
(
    PGOPTIONS='-c universus.migration_test_pause_seconds=5' \
        run_migrations "$TIMEOUT_DB" timeout-holder "$atomic_dir"
) >"$TMP_ROOT/timeout-holder.log" 2>&1 &
timeout_holder=$!
BACKGROUND_PIDS="$BACKGROUND_PIDS $timeout_holder"
wait_for_value "$TIMEOUT_DB" "SELECT status FROM universus_schema_migration_runs WHERE runner_id = 'timeout-holder';" running || fail "timeout holder did not start"
if MIGRATION_LOCK_TIMEOUT_SECONDS=1 run_migrations "$TIMEOUT_DB" timeout-waiter "$atomic_dir" >"$TMP_ROOT/timeout-waiter.log" 2>&1; then
    fail "advisory-lock timeout unexpectedly succeeded"
fi
[ "$(query "$TIMEOUT_DB" "SELECT status FROM universus_schema_migration_runs WHERE runner_id = 'timeout-waiter';")" = failed ] || fail "lock timeout was not recorded"
grep -qi 'lock timeout' "$TMP_ROOT/timeout-waiter.log" || fail "lock timeout reason was not visible"
wait "$timeout_holder" || fail "timeout holder failed"
run_migrations "$TIMEOUT_DB" timeout-recovery "$atomic_dir" >"$TMP_ROOT/timeout-recovery.log" 2>&1
[ "$(query "$TIMEOUT_DB" "SELECT status FROM universus_schema_migration_runs WHERE runner_id = 'timeout-recovery';")" = applied ] || fail "post-timeout recovery failed"

echo "[6/7] terminated backend rolls back schema and recovers"
create_database "$CRASH_DB"
(
    PGOPTIONS='-c universus.migration_test_pause_seconds=30' \
        run_migrations "$CRASH_DB" crash-proof "$atomic_dir"
) >"$TMP_ROOT/crash.log" 2>&1 &
crash_pid=$!
BACKGROUND_PIDS="$BACKGROUND_PIDS $crash_pid"
wait_for_value "$CRASH_DB" "SELECT status FROM universus_schema_migration_runs WHERE runner_id = 'crash-proof';" running || fail "crash probe did not start"
backend_pid=
attempt=0
while [ "$attempt" -lt 40 ]; do
    backend_pid=$(query "$CRASH_DB" "SELECT pid FROM pg_stat_activity WHERE application_name = 'universus-migrate:crash-proof' AND query LIKE '%pg_sleep%' LIMIT 1;" || true)
    [ -n "$backend_pid" ] && break
    attempt=$((attempt + 1))
    sleep 0.25
done
[ -n "$backend_pid" ] || fail "sleeping migration backend was not found"
[ "$(query "$CRASH_DB" "SELECT pg_terminate_backend($backend_pid);")" = t ] || fail "migration backend was not terminated"
if wait "$crash_pid"; then
    fail "terminated migration runner unexpectedly succeeded"
fi
wait_for_value "$CRASH_DB" "SELECT status FROM universus_schema_migration_runs WHERE runner_id = 'crash-proof';" failed || fail "terminated run was not marked failed"
[ "$(query "$CRASH_DB" "SELECT count(*) FROM universus_schema_migrations;")" = 0 ] || fail "terminated transaction wrote migration history"
[ "$(query "$CRASH_DB" "SELECT count(*) FROM pg_class WHERE oid IN (to_regclass('migration_atomic_probe_before'), to_regclass('migration_atomic_probe_after'));")" = 0 ] || fail "terminated transaction left partial DDL"
run_migrations "$CRASH_DB" crash-recovery "$atomic_dir" >"$TMP_ROOT/crash-recovery.log" 2>&1
run_migrations "$CRASH_DB" crash-repeat "$atomic_dir" >"$TMP_ROOT/crash-repeat.log" 2>&1
[ "$(query "$CRASH_DB" "SELECT count(*) FROM universus_schema_migrations;")" = 1 ] || fail "crash recovery did not apply exactly once"

echo "[7/7] failure visibility redacts the connection credential"
admin_psql -q -c "CREATE ROLE $ROLE_NAME LOGIN PASSWORD '$ROLE_PASSWORD';"
create_database "$REDACTION_DB" "$ROLE_NAME"
failure_dir="$TMP_ROOT/failure-steps"
mkdir "$failure_dir"
cp "$FIXTURES_DIR/001_failure_probe.sql" "$failure_dir/001_failure_probe.sql"
if redaction_output=$(
    PGUSER=$ROLE_NAME \
    PGPASSWORD=$ROLE_PASSWORD \
    PGDATABASE=$REDACTION_DB \
    MIGRATION_RUNNER_ID=redaction-proof \
    MIGRATION_SQL_DIR=$failure_dir \
        "$RUNNER" 2>&1
); then
    fail "failure probe unexpectedly succeeded"
fi
case "$redaction_output" in
    *"$ROLE_PASSWORD"*) fail "connection credential was printed" ;;
esac
printf '%s\n' "$redaction_output" | grep -q '\[REDACTED\]' || fail "redaction marker was absent"
[ "$(query "$REDACTION_DB" "SELECT count(*) FROM universus_schema_migration_runs WHERE error_message LIKE '%$ROLE_PASSWORD%';")" = 0 ] || fail "credential persisted in run history"
[ "$(query "$REDACTION_DB" "SELECT count(*) FROM universus_schema_migration_attempts WHERE error_message LIKE '%$ROLE_PASSWORD%';")" = 0 ] || fail "credential persisted in attempt history"

echo "All migration durability tests passed ($expected_count migrations)."
