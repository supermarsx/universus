#!/bin/sh
set -eu

# Backward-compatible entry point retained for CI and local scripts. All
# ordering/history/locking behavior lives in the canonical runner.
SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
PGHOST=${PGHOST:-localhost}
PGPORT=${PGPORT:-5432}
PGUSER=${PGUSER:-postgres}
PGPASSWORD=${PGPASSWORD:-postgres}
PGDATABASE=${PGDATABASE:-testdb}
export PGHOST PGPORT PGUSER PGPASSWORD PGDATABASE
exec "$SCRIPT_DIR/migrate-db.sh" "$@"
