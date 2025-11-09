#!/bin/bash
# Automated backup script for PostgreSQL and Redis (Dockerized)
# Usage: ./backup.sh
# Requires: pg_dump, docker, access to running containers

set -e

# Configurable variables
BACKUP_DIR="$(dirname "$0")/../../backups"
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")

# PostgreSQL
POSTGRES_CONTAINER="universus-db"  # Change to your actual container name
POSTGRES_DB="universus"           # Change to your actual DB name
POSTGRES_USER="postgres"          # Change if needed
POSTGRES_PASSWORD="postgres"      # Change if needed

# Redis
REDIS_CONTAINER="universus-redis"  # Change to your actual container name

mkdir -p "$BACKUP_DIR"

# Backup PostgreSQL
echo "[INFO] Backing up PostgreSQL..."
docker exec -e PGPASSWORD="$POSTGRES_PASSWORD" "$POSTGRES_CONTAINER" pg_dump -U "$POSTGRES_USER" "$POSTGRES_DB" > "$BACKUP_DIR/postgres_${TIMESTAMP}.sql"
echo "[INFO] PostgreSQL backup complete: $BACKUP_DIR/postgres_${TIMESTAMP}.sql"

# Backup Redis
echo "[INFO] Backing up Redis..."
docker exec "$REDIS_CONTAINER" redis-cli SAVE
docker cp "$REDIS_CONTAINER":/data/dump.rdb "$BACKUP_DIR/redis_${TIMESTAMP}.rdb"
echo "[INFO] Redis backup complete: $BACKUP_DIR/redis_${TIMESTAMP}.rdb"

# (Optional) Upload to S3 or other remote storage here
# Example:
# aws s3 cp "$BACKUP_DIR/postgres_${TIMESTAMP}.sql" s3://your-bucket/backups/
# aws s3 cp "$BACKUP_DIR/redis_${TIMESTAMP}.rdb" s3://your-bucket/backups/

# Cleanup old backups (keep last 7 days)
find "$BACKUP_DIR" -type f -mtime +7 -delete
