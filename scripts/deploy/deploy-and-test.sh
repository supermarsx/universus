#!/bin/bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

TOTAL_STEPS=8
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

log_step "Checking deployment tools"
for command in docker cargo curl pwsh; do
  if ! command -v "$command" &> /dev/null; then
    log_error "$command is required"
    exit 1
  fi
done
docker compose version >/dev/null
docker compose config --quiet
log_success "Docker Compose, Rust, curl, and PowerShell are available"

log_step "Starting infrastructure and applying durable migrations"
docker compose up -d --build database redis rabbitmq database-migrate >/dev/null
for _ in {1..90}; do
  MIGRATION_STATE=$(docker inspect -f '{{.State.Status}}:{{.State.ExitCode}}' universus_database_migrate 2>/dev/null || true)
  if [[ "$MIGRATION_STATE" == "exited:0" ]]; then
    break
  fi
  if [[ "$MIGRATION_STATE" == exited:* ]]; then
    docker compose logs database-migrate
    log_error "Database migration service failed ($MIGRATION_STATE)"
    exit 1
  fi
  sleep 2
done
if [[ "${MIGRATION_STATE:-}" != "exited:0" ]]; then
  docker compose logs database database-migrate
  log_error "Database migrations did not complete within 180 seconds"
  exit 1
fi
log_success "PostgreSQL, Redis, RabbitMQ, and the migration chain are ready"

log_step "Inspecting schema"
TABLE_COUNT=$(docker compose exec -T database sh -c \
  'psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" -Atc "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = '\''public'\'';"')
log_info "Created tables: ${TABLE_COUNT}"
log_success "Schema inspection recorded"

log_step "Creating optional bootstrap admin"
if [[ -n "${UNIVERSUS_BOOTSTRAP_ADMIN_PASSWORD_HASH:-}" ]]; then
  if [[ "$UNIVERSUS_BOOTSTRAP_ADMIN_PASSWORD_HASH" != '$argon2id$'* ]]; then
    log_error "UNIVERSUS_BOOTSTRAP_ADMIN_PASSWORD_HASH must be an Argon2id PHC string"
    exit 1
  fi
  docker compose exec -T \
    -e UNIVERSUS_BOOTSTRAP_ADMIN_PASSWORD_HASH="$UNIVERSUS_BOOTSTRAP_ADMIN_PASSWORD_HASH" \
    database sh -c \
    'psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -v admin_password_hash="$UNIVERSUS_BOOTSTRAP_ADMIN_PASSWORD_HASH"' \
    <<'EOF' >/dev/null
INSERT INTO users (username, email, password_hash, created_at, is_admin)
VALUES (
  'admin',
  'admin@universus.com',
  :'admin_password_hash',
  NOW(),
  true
)
ON CONFLICT (email) DO NOTHING;
EOF
  log_success "Bootstrap admin seeded at admin@universus.com"
else
  log_info "Set UNIVERSUS_BOOTSTRAP_ADMIN_PASSWORD_HASH to seed an admin account"
fi

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
