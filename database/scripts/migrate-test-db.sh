#!/bin/sh
set -e

# migrate-test-db.sh
# Apply SQL migration steps to a running Postgres DB using psql.
# Expects the following environment variables:
# - PGHOST (default: localhost)
# - PGPORT (default: 5432)
# - PGUSER (default: postgres)
# - PGPASSWORD
# - PGDATABASE (defaults to testdb)

PGHOST=${PGHOST:-localhost}
PGPORT=${PGPORT:-5432}
PGUSER=${PGUSER:-postgres}
PGPASSWORD=${PGPASSWORD:-postgres}
PGDATABASE=${PGDATABASE:-testdb}

export PGPASSWORD

SQL_DIR="$(dirname "$0")/../sql/steps"

echo "Applying SQL files from $SQL_DIR to $PGDATABASE@$PGHOST:$PGPORT"

# Simple retry wrapper for psql commands
run_psql() {
  local attempt=0
  local max_attempts=3
  local delay=2
  local cmd="$1"
  while [ $attempt -lt $max_attempts ]; do
    if sh -c "$cmd"; then
      return 0
    fi
    attempt=$((attempt + 1))
    echo "psql command failed (attempt $attempt/$max_attempts). Retrying in $delay seconds..."
    sleep $delay
  done
  echo "psql command failed after $max_attempts attempts. Exiting."
  return 1
}

for f in "$SQL_DIR"/*.sql; do
  echo "Applying $(basename "$f")"
  run_psql "psql -v ON_ERROR_STOP=1 -h \"$PGHOST\" -p \"$PGPORT\" -U \"$PGUSER\" -d \"$PGDATABASE\" -f \"$f\""
done

echo "Applying schema completed. Running sanity checks..."

# Sanity checks: ensure core tables exist by querying their row counts (0 is ok)
check_table() {
  local tbl=$1
  echo -n "Checking table $tbl: "
  run_psql "psql -h \"$PGHOST\" -p \"$PGPORT\" -U \"$PGUSER\" -d \"$PGDATABASE\" -c \"SELECT COUNT(*) FROM $tbl;\" -t"
}

check_table users || { echo "Sanity check failed: users table missing"; exit 1; }
check_table planets || { echo "Sanity check failed: planets table missing"; exit 1; }
check_table fleets || { echo "Sanity check failed: fleets table missing"; exit 1; }

# Optionally, list a few rows from key tables for debugging
if [ "$VERBOSE_MIGRATE" = "true" ]; then
  echo "Listing sample rows from users, planets, fleets for verification:"
  psql -h "$PGHOST" -p "$PGPORT" -U "$PGUSER" -d "$PGDATABASE" -c "SELECT * FROM users LIMIT 3;"
  psql -h "$PGHOST" -p "$PGPORT" -U "$PGUSER" -d "$PGDATABASE" -c "SELECT * FROM planets LIMIT 3;"
  psql -h "$PGHOST" -p "$PGPORT" -U "$PGUSER" -d "$PGDATABASE" -c "SELECT * FROM fleets LIMIT 3;"
fi

echo "Sanity checks passed."
