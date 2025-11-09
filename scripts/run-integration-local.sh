#!/bin/sh
# run-integration-local.sh
# Starts a local Postgres docker container, applies migrations, runs integration tests, then cleans up.

CONTAINER_NAME=universus-test-db
IMAGE=postgres:15
PGUSER=postgres
PGPASSWORD=postgres
PGDATABASE=testdb
PGPORT=5432

set -e

echo "Starting Postgres container ($CONTAINER_NAME)..."
docker run --name $CONTAINER_NAME -e POSTGRES_PASSWORD=$PGPASSWORD -e POSTGRES_USER=$PGUSER -e POSTGRES_DB=$PGDATABASE -p $PGPORT:5432 -d $IMAGE

echo "Waiting for Postgres to become ready..."
# wait for pg_isready
until docker exec $CONTAINER_NAME pg_isready -U $PGUSER >/dev/null 2>&1; do
  echo "Waiting..."
  sleep 1
done

echo "Applying migrations..."
PGHOST=localhost PGPORT=$PGPORT PGUSER=$PGUSER PGPASSWORD=$PGPASSWORD PGDATABASE=$PGDATABASE scripts/../database/scripts/migrate-test-db.sh

echo "Running backend integration tests..."
DATABASE_URL=postgres://$PGUSER:$PGPASSWORD@localhost:$PGPORT/$PGDATABASE RUN_INTEGRATION=true pnpm --filter ./backend... run test:integration

STATUS=$?

echo "Stopping and removing Postgres container..."
docker rm -f $CONTAINER_NAME || true

exit $STATUS
