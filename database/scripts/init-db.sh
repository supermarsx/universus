#!/bin/sh
set -e

run_sql() {
  local file="$1"
  if [ -f "$file" ]; then
    echo "Applying schema file: $(basename "$file")"
    psql -v ON_ERROR_STOP=1 -U "$POSTGRES_USER" -d "$POSTGRES_DB" -f "$file"
  fi
}

SQL_DIR="/docker-entrypoint-initdb.d/sql"

run_sql "$SQL_DIR/schema.sql"

find "$SQL_DIR" -maxdepth 1 -type f ! -name 'schema.sql' -name '*.sql' | sort | while read -r file; do
  run_sql "$file"
done

if [ -d "$SQL_DIR/migrations" ]; then
  find "$SQL_DIR/migrations" -type f -name '*.sql' | sort | while read -r file; do
    run_sql "$file"
  done
fi
