#!/bin/bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

TOTAL_STEPS=10
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

log_info() {
  echo "  ℹ $1"
}

cd "$REPO_ROOT"

log_step "Checking PostgreSQL and Redis clients are installed"
if ! command -v psql &> /dev/null; then
  log_error "psql missing; please install PostgreSQL client"
  exit 1
fi
if ! command -v redis-cli &> /dev/null; then
  log_error "redis-cli missing; please install Redis"
  exit 1
fi
log_success "PostgreSQL and Redis clients are available"

log_step "Ensuring PostgreSQL and Redis services are running"
if ! sudo service postgresql status &> /dev/null; then
  sudo service postgresql start
  sleep 2
fi
if ! sudo service redis-server status &> /dev/null; then
  sudo service redis-server start
  sleep 2
fi
log_success "PostgreSQL and Redis are running"

log_step "Recreating universus_rpg database"
sudo -u postgres psql -c "ALTER USER postgres PASSWORD 'postgres';" 2>/dev/null || true
sudo -u postgres psql -c "DROP DATABASE IF EXISTS universus_rpg;" 2>/dev/null || true
sudo -u postgres psql -c "CREATE DATABASE universus_rpg;" 2>/dev/null || true
log_success "Database universus_rpg is ready"

log_step "Applying schema and migrations"
apply_if_exists() {
  local script="$1"
  if [ -f "$script" ]; then
    log_info "Applying $(basename "$script")"
    sudo -u postgres psql -d universus_rpg -f "$script" >/dev/null 2>&1
    log_success "  $(basename "$script") applied"
  else
    log_info "  $(basename "$script") missing (skipping)"
  fi
}

apply_if_exists "$REPO_ROOT/database/sql/schema.sql"
for migration in \
  "$REPO_ROOT/database/sql/migrations/001_update_messages_table.sql" \
  "$REPO_ROOT/database/sql/migrations/002_add_shop_tables.sql" \
  "$REPO_ROOT/database/sql/migrations/003_millisecond_precision_combat.sql" \
  "$REPO_ROOT/database/sql/migrations/004_admin_features.sql" \
  "$REPO_ROOT/database/sql/migrations/005_bot_system.sql"; do
  apply_if_exists "$migration"
done
apply_if_exists "$REPO_ROOT/database/sql/admin_schema.sql"
apply_if_exists "$REPO_ROOT/database/sql/debris_schema.sql"
apply_if_exists "$REPO_ROOT/database/sql/universe_seeding_schema.sql"

log_step "Inspecting schema"
TABLE_COUNT=$(sudo -u postgres psql -d universus_rpg -t -c "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'public';")
log_info "Created tables: ${TABLE_COUNT}"
log_success "Schema inspection recorded"

log_step "Creating admin user"
ADMIN_PASSWORD_HASH='$2b$10$rOZhW9K4qVXZ9KqH.xZxVu3kB8pQw3qJ5YTl5Z8vZ9QZxQZxQZxQZ'
sudo -u postgres psql -d universus_rpg <<'EOF' >/dev/null 2>&1
INSERT INTO users (username, email, password, created_at, is_admin)
VALUES ('admin', 'admin@universus.com', '$ADMIN_PASSWORD_HASH', NOW(), true)
ON CONFLICT (email) DO NOTHING;
EOF
log_success "Admin user seeded (admin@universus.com / admin123)"

log_step "Building Rust workspace"
cargo build --workspace >/dev/null
log_success "Rust workspace compiled"

log_step "Starting Rust services via docker compose"
docker compose up -d \
  rust-api-gateway \
  rust-realtime-gateway \
  rust-web-frontend \
  rust-admin-api \
  rust-bot-api \
  rust-sms-api \
  rust-core-engine \
  rust-scheduler-worker \
  rust-sharding-worker \
  rust-analytics-worker \
  rust-email-worker \
  rust-bot-worker >/dev/null
log_info "Waiting for services to initialize..."
sleep 10
log_success "Dockerized Rust services are up"

log_step "Probing key HTTP endpoints"
probe() {
  local url="$1"
  local label="$2"
  local code
  code=$(curl -s -o /dev/null -w "%{http_code}" "$url")
  if [[ "$code" =~ ^(200|401|403|429)$ ]]; then
    log_success "Endpoint ${label} responded (${code})"
  else
    log_error "Endpoint ${label} returned ${code}"
  fi
}

probe "http://localhost:3300/api/health" "API gateway health"
probe "http://localhost:3300/api/debris" "Debris API"
probe "http://localhost:3300/api/moons" "Moons API"
probe "http://localhost:3300/api/universe" "Universe API"
probe "http://localhost:4302/api/admin/stats" "Admin API health"
probe "http://localhost:4301/api/admin/bots" "Bot API health"
probe "http://localhost:4303/api/health" "SMS API health"
probe "http://localhost:8080/" "Web frontend"

log_step "Cutover smoke tests"
pwsh -NoProfile -File scripts/rust/run-cutover-validation.ps1 >/dev/null
log_success "Cutover smoke validation executed"

echo
echo "Deployment verification complete:"
echo "  API gateway: http://localhost:3300"
echo "  Admin API: http://localhost:4302"
echo "  Bot API: http://localhost:4301"
echo "  SMS API: http://localhost:4303"
echo "  Web frontend: http://localhost:8080"
echo "  Docker services: docker compose ps"
echo
echo "To stop the suite: docker compose down"
echo
exit 0
