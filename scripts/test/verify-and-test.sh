#!/bin/bash

###############################################################################
# SpaceEmpire RPG - Automated Verification and Testing Script
# 
# Purpose: Automatically verify all new features and apply migrations
# Created: 2025-11-06
# Usage: ./scripts/test/verify-and-test.sh
###############################################################################

set -e  # Exit on error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Counters
PASSED=0
FAILED=0
WARNINGS=0

# Logging functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[✓]${NC} $1"
    ((PASSED++))
}

log_error() {
    echo -e "${RED}[✗]${NC} $1"
    ((FAILED++))
}

log_warning() {
    echo -e "${YELLOW}[!]${NC} $1"
    ((WARNINGS++))
}

# Print header
print_header() {
    echo ""
    echo "═══════════════════════════════════════════════════════════════"
    echo "  $1"
    echo "═══════════════════════════════════════════════════════════════"
    echo ""
}

###############################################################################
# PHASE 1: Start Services & Apply Migrations
###############################################################################

print_header "PHASE 1: Starting Services & Applying Migrations"

log_info "Starting Docker containers..."
docker-compose up -d

log_info "Waiting for services to initialize (10 seconds)..."
sleep 10

# Verify PostgreSQL is ready
log_info "Checking PostgreSQL connection..."
if docker-compose exec -T database pg_isready -U postgres > /dev/null 2>&1; then
    log_success "PostgreSQL is ready"
else
    log_error "PostgreSQL is not ready"
    exit 1
fi

# Check if migration 003 has been applied
log_info "Checking if migration 003 needs to be applied..."
if docker-compose exec -T database psql -U postgres -d universus_rpg -c "\dt fleet_movements_precise" 2>&1 | grep -q "fleet_movements_precise"; then
    log_warning "Migration 003 already applied, skipping..."
else
    log_info "Applying migration 003 (Millisecond Precision Combat)..."
    docker-compose exec -T database psql -U postgres -d universus_rpg < database/sql/migrations/003_millisecond_precision_combat.sql
    
    # Verify
    if docker-compose exec -T database psql -U postgres -d universus_rpg -c "\dt fleet_movements_precise" 2>&1 | grep -q "fleet_movements_precise"; then
        log_success "Migration 003 applied successfully"
    else
        log_error "Migration 003 failed"
    fi
fi

# Check if migration 004 has been applied
log_info "Checking if migration 004 needs to be applied..."
if docker-compose exec -T database psql -U postgres -d universus_rpg -c "\d users" 2>&1 | grep -q "is_admin"; then
    log_warning "Migration 004 already applied, skipping..."
else
    log_info "Applying migration 004 (Admin Features)..."
    docker-compose exec -T database psql -U postgres -d universus_rpg < database/sql/migrations/004_admin_features.sql
    
    # Verify
    if docker-compose exec -T database psql -U postgres -d universus_rpg -c "\d users" 2>&1 | grep -q "is_admin"; then
        log_success "Migration 004 applied successfully"
    else
        log_error "Migration 004 failed"
    fi
fi

# Create admin user
log_info "Creating admin user..."
docker-compose exec -T database psql -U postgres -d universus_rpg -c "UPDATE users SET is_admin = true WHERE id = 1;" > /dev/null 2>&1
ADMIN_COUNT=$(docker-compose exec -T database psql -U postgres -d universus_rpg -t -c "SELECT COUNT(*) FROM users WHERE is_admin = true;" | xargs)
if [ "$ADMIN_COUNT" -gt 0 ]; then
    log_success "Admin user created/verified (count: $ADMIN_COUNT)"
else
    log_warning "No admin users found. You may need to create one manually."
fi

###############################################################################
# PHASE 2: Backend Verification
###############################################################################

print_header "PHASE 2: Rust API Gateway Verification"

log_info "Running cargo check across workspace..."
if cargo check --workspace >/tmp/cargo-check.log 2>&1; then
    log_success "Cargo check completed"
else
    log_error "Cargo check failed (see /tmp/cargo-check.log)"
    cat /tmp/cargo-check.log
fi

log_info "Running targeted Rust parity tests..."
if cargo test -p app-api-gateway notifications_high_volume_create_flow_stays_consistent sharding_registration_churn_keeps_routing_stats_coherent -- --nocapture >/tmp/cargo-parity.log 2>&1; then
    log_success "Parity tests succeeded"
else
    log_error "Parity tests failed (see /tmp/cargo-parity.log)"
    cat /tmp/cargo-parity.log
fi

log_info "Testing Rust API gateway health endpoint..."
HEALTH_RESPONSE=$(curl -s http://localhost:3300/api/health)
if echo "$HEALTH_RESPONSE" | grep -q "\"status\":\"ok\""; then
    log_success "Rust API gateway is healthy"
else
    log_error "Rust API gateway is not responding"
fi

log_info "Checking admin and bot API health"
if curl -s http://localhost:4302/api/admin/dashboard | grep -q '"success"'; then
    log_success "Admin API responsive"
else
    log_warning "Admin API may require auth to respond"
fi
if curl -s http://localhost:4301/api/admin/bots | grep -q '"success"'; then
    log_success "Bot API responsive"
else
    log_warning "Bot API may require auth to respond"
fi

###############################################################################
# PHASE 3: Frontend File Verification
###############################################################################

print_header "PHASE 3: Frontend File Verification"

# Check if all new UI files exist
log_info "Checking UI files..."

FILES=(
    "frontend/views/pages/leaderboard.njk"
    "frontend/views/pages/messages.njk"
    "frontend/views/pages/admin.njk"
    "frontend/js/leaderboard.js"
    "frontend/js/messages.js"
    "frontend/js/admin.js"
    "frontend/js/planetImageGenerator.js"
)

for FILE in "${FILES[@]}"; do
    if [ -f "$FILE" ]; then
        log_success "$FILE exists"
    else
        log_error "$FILE not found"
    fi
done

# Check file sizes (should not be empty)
log_info "Checking file sizes..."
for FILE in "${FILES[@]}"; do
    if [ -f "$FILE" ]; then
        SIZE=$(wc -l < "$FILE")
        if [ "$SIZE" -gt 50 ]; then
            log_success "$FILE has $SIZE lines"
        else
            log_warning "$FILE seems small ($SIZE lines)"
        fi
    fi
done

###############################################################################
# PHASE 4: Database Verification
###############################################################################

print_header "PHASE 4: Database Verification"

log_info "Checking database tables..."

TABLES=(
    "fleet_movements_precise"
    "combats_precise"
    "combat_rounds_precise"
    "combat_events_precise"
    "admin_audit_log"
)

for TABLE in "${TABLES[@]}"; do
    if docker-compose exec -T database psql -U postgres -d universus_rpg -c "\dt $TABLE" 2>&1 | grep -q "$TABLE"; then
        log_success "Table $TABLE exists"
    else
        log_error "Table $TABLE not found"
    fi
done

# Check indexes
log_info "Checking database indexes..."
INDEX_COUNT=$(docker-compose exec -T database psql -U postgres -d universus_rpg -t -c "SELECT COUNT(*) FROM pg_indexes WHERE tablename LIKE '%precise%' OR tablename = 'admin_audit_log';" | xargs)
if [ "$INDEX_COUNT" -gt 5 ]; then
    log_success "Found $INDEX_COUNT indexes on new tables"
else
    log_warning "Expected more indexes (found: $INDEX_COUNT)"
fi

###############################################################################
# PHASE 5: Integration Checks
###############################################################################

print_header "PHASE 5: Integration Checks"

log_info "Checking Rust API gateway service status..."
if docker compose ps rust-api-gateway | grep -q "Up"; then
    log_success "rust-api-gateway container is running"
else
    log_error "rust-api-gateway container is not running"
fi

log_info "Checking Redis connection..."
if docker compose exec -T redis redis-cli ping 2>&1 | grep -q "PONG"; then
    log_success "Redis is responding"
else
    log_error "Redis is not responding"
fi

log_info "Verifying realtime REST endpoints"
if curl -s http://localhost:3300/api/realtime/chat/channels | grep -q "\"channels\""; then
    log_success "Realtime chat channels endpoint returns data"
else
    log_warning "Realtime endpoints may require auth"
fi

###############################################################################
# PHASE 6: Summary Report
###############################################################################

print_header "VERIFICATION SUMMARY"

echo ""
echo "Test Results:"
echo "  ✓ Passed:   $PASSED"
echo "  ✗ Failed:   $FAILED"
echo "  ! Warnings: $WARNINGS"
echo ""

if [ "$FAILED" -eq 0 ]; then
    echo -e "${GREEN}═══════════════════════════════════════════════════════════════${NC}"
    echo -e "${GREEN}  ✓ ALL AUTOMATED CHECKS PASSED!${NC}"
    echo -e "${GREEN}═══════════════════════════════════════════════════════════════${NC}"
    echo ""
    echo "Next steps:"
    echo "  1. Open http://localhost:8080 in your browser"
    echo "  2. Browse the Rust API gateway at http://localhost:3300/api"
    echo "  3. Tail the gateway logs: docker compose logs -f rust-api-gateway"
    echo "  4. Review the validation harness: scripts/rust/run-cutover-validation.ps1"
    echo ""
    exit 0
else
    echo -e "${RED}═══════════════════════════════════════════════════════════════${NC}"
    echo -e "${RED}  ✗ SOME CHECKS FAILED${NC}"
    echo -e "${RED}═══════════════════════════════════════════════════════════════${NC}"
    echo ""
    echo "Please review the errors above and check the following logs:"
        echo "  - Rust API gateway logs: docker compose logs rust-api-gateway"
        echo "  - PostgreSQL logs: docker compose logs postgres"
        echo "  - Cargo check log: /tmp/cargo-check.log"
        echo "  - Parity test log: /tmp/cargo-parity.log"
    echo ""
    echo "For detailed troubleshooting, see: VERIFICATION_AND_TESTING_GUIDE.md"
    echo ""
    exit 1
fi
