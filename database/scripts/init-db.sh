#!/bin/sh
set -eu

# The official Postgres entrypoint invokes this only for an empty data volume.
# Delegate to the same durable runner used by upgrades and CI.
export PGUSER=${PGUSER:-${POSTGRES_USER:-postgres}}
export PGDATABASE=${PGDATABASE:-${POSTGRES_DB:-postgres}}
export PGPASSWORD=${PGPASSWORD:-${POSTGRES_PASSWORD:-}}
export MIGRATION_SQL_DIR=${MIGRATION_SQL_DIR:-/opt/universus/database/sql/steps}
exec /opt/universus/database/scripts/migrate-db.sh
