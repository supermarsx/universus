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
STEPS_DIR="$SQL_DIR/steps"

if [ -d "$STEPS_DIR" ]; then
  echo "Applying ordered schema steps..."
  find "$STEPS_DIR" -maxdepth 1 -type f -name '*.sql' | sort | while read -r file; do
    run_sql "$file"
  done
else
  echo "No steps directory found; skipping structured schema application."
fi

echo "Finished applying schema steps."
