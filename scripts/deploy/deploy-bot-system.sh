#!/bin/bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

TOTAL_STEPS=6
CURRENT_STEP=0

log_step() {
  CURRENT_STEP=$((CURRENT_STEP + 1))
  echo "[${CURRENT_STEP}/${TOTAL_STEPS}] $1"
}

log_success() {
  echo "✓ $1"
}

log_error() {
  echo "✗ $1"
}

log_warning() {
  echo "⚠ $1"
}

log_info() {
  echo "  ℹ $1"
}

cd "$REPO_ROOT"

log_step "Starting Compose infrastructure"
docker compose config --quiet
docker compose up -d --build database redis rabbitmq database-migrate >/dev/null
for _ in {1..90}; do
  MIGRATION_STATE=$(docker inspect -f '{{.State.Status}}:{{.State.ExitCode}}' universus_database_migrate 2>/dev/null || true)
  [[ "$MIGRATION_STATE" == "exited:0" ]] && break
  if [[ "$MIGRATION_STATE" == exited:* ]]; then
    docker compose logs database-migrate
    log_error "Database migration service failed ($MIGRATION_STATE)"
    exit 1
  fi
  sleep 2
done
[[ "${MIGRATION_STATE:-}" == "exited:0" ]] || { log_error "Database migrations timed out"; exit 1; }
log_success "PostgreSQL, Redis, RabbitMQ, and migrations are ready"

log_step "Verifying bot migration history"
BOT_MIGRATION_COUNT=$(docker compose exec -T database sh -c \
  'psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" -Atc "SELECT COUNT(*) FROM universus_schema_migrations WHERE version = 24;"')
[[ "$BOT_MIGRATION_COUNT" == "1" ]] || { log_error "Bot migration 24 is not recorded"; exit 1; }
log_success "Bot migration is current"

log_step "Creating bot tables snapshot"
TABLES=$(docker compose exec -T database sh -c \
  'psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" -Atc "SELECT table_name FROM information_schema.tables WHERE table_schema = '\''public'\'' AND table_name LIKE '\''bot%'\'';"')
if [ -n "$TABLES" ]; then
  log_success "Bot tables present"
else
  log_warning() { echo "⚠ $1"; }
  log_warning "No bot tables were found"
fi

log_step "Starting Rust bot stack"
docker compose up -d rust-api-gateway rust-bot-api rust-bot-worker >/dev/null
sleep 10
if curl -s http://localhost:3300/api/health >/dev/null 2>&1; then
  log_success "Rust API gateway ready"
else
  log_error "Rust API gateway did not respond"
  docker compose logs rust-api-gateway
  exit 1
fi

log_step "Testing bot control endpoints"
if [[ -z "${UNIVERSUS_ADMIN_EMAIL:-}" || -z "${UNIVERSUS_ADMIN_PASSWORD:-}" ]]; then
  log_error "Set UNIVERSUS_ADMIN_EMAIL and UNIVERSUS_ADMIN_PASSWORD for the endpoint smoke test"
  exit 1
fi
LOGIN_PAYLOAD=$(jq -n \
  --arg email "$UNIVERSUS_ADMIN_EMAIL" \
  --arg password "$UNIVERSUS_ADMIN_PASSWORD" \
  '{email: $email, password: $password}')
ADMIN_TOKEN=$(curl -s -X POST http://localhost:3300/api/auth/login \
  -H "Content-Type: application/json" \
  -d "$LOGIN_PAYLOAD" | jq -r '.token')
if [ "$ADMIN_TOKEN" == "null" ] || [ -z "$ADMIN_TOKEN" ]; then
  log_error "Unable to obtain admin token"
  exit 1
fi
log_success "Admin token retrieved"

curl -s -H "Authorization: Bearer $ADMIN_TOKEN" http://localhost:3300/api/admin/bots >/dev/null \
  && log_success "Bot list endpoint reachable" \
  || log_error "Bot list endpoint failed"

curl -s -H "Authorization: Bearer $ADMIN_TOKEN" http://localhost:3300/api/admin/bots/personalities/list >/dev/null \
  && log_success "Bot personalities endpoint reachable" \
  || log_error "Bot personalities endpoint failed"

log_step "Summary"
echo
echo "Rust bot stack running via docker compose"
echo "API gateway: http://localhost:3300"
echo "Bot admin endpoints: http://localhost:3300/api/admin/bots"
echo "Logs: docker compose logs rust-api-gateway"
echo "To tear down: docker compose down"
