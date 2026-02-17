#!/bin/bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

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

log_step "Ensuring PostgreSQL and Redis are online"
sudo service postgresql start >/dev/null 2>&1 || true
sudo service redis-server start >/dev/null 2>&1 || true
log_success "PostgreSQL and Redis services started"

log_step "Applying bot system migration (#005)"
PGPASSWORD=postgres psql -h 127.0.0.1 -U postgres -d universus_rpg \
  -f database/sql/migrations/005_bot_system.sql >/dev/null
log_success "Bot migration applied"

log_step "Creating bot tables snapshot"
TABLES=$(PGPASSWORD=postgres psql -h 127.0.0.1 -U postgres -d universus_rpg -t -c \
  "SELECT table_name FROM information_schema.tables WHERE table_schema = 'public' AND table_name LIKE 'bot%';")
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
ADMIN_TOKEN=$(curl -s -X POST http://localhost:3300/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"email":"admin@example.com","password":"admin123"}' | jq -r '.token')
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
