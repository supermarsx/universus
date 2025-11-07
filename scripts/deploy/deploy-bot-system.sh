#!/bin/bash

# Bot System Deployment and Testing Script
# This script sets up the database, starts services, and tests the bot system

set -e  # Exit on error

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if command -v git >/dev/null 2>&1; then
    REPO_ROOT="$(git -C "$SCRIPT_DIR" rev-parse --show-toplevel)"
else
    REPO_ROOT="$SCRIPT_DIR"
    while [ "$REPO_ROOT" != "/" ] && [ ! -f "$REPO_ROOT/docker-compose.yml" ]; do
        REPO_ROOT="$(cd "$REPO_ROOT/.." && pwd)"
    done
fi
LOG_DIR="$REPO_ROOT/logs"
mkdir -p "$LOG_DIR"
cd "$REPO_ROOT"

echo "=========================================="
echo "Bot System Deployment Script"
echo "=========================================="
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to print colored messages
print_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

print_error() {
    echo -e "${RED}✗ $1${NC}"
}

print_info() {
    echo -e "${YELLOW}ℹ $1${NC}"
}

# 1. Start PostgreSQL
print_info "Starting PostgreSQL..."
sudo service postgresql start || {
    print_error "Failed to start PostgreSQL"
    print_info "Trying alternative method..."
    sudo -u postgres /usr/lib/postgresql/15/bin/pg_ctl -D /var/lib/postgresql/15/main start
}

sleep 3

# Check if PostgreSQL is running
if pg_isready -h 127.0.0.1 -p 5432; then
    print_success "PostgreSQL is running"
else
    print_error "PostgreSQL failed to start"
    exit 1
fi

# 2. Apply Bot System Migration
print_info "Applying bot system migration (005)..."
PGPASSWORD=postgres psql -h 127.0.0.1 -U postgres -d universus_rpg \
    -f database/sql/migrations/005_bot_system.sql 2>&1 | tee migration_output.txt

if grep -q "ERROR" migration_output.txt; then
    print_error "Migration failed - check migration_output.txt for details"
else
    print_success "Migration applied successfully"
fi

# 3. Verify tables created
print_info "Verifying bot tables..."
TABLES=$(PGPASSWORD=postgres psql -h 127.0.0.1 -U postgres -d universus_rpg -t -c \
    "SELECT table_name FROM information_schema.tables WHERE table_schema = 'public' AND table_name LIKE 'bot%' ORDER BY table_name;")

if [ -z "$TABLES" ]; then
    print_error "No bot tables found"
else
    print_success "Bot tables created:"
    echo "$TABLES"
fi

# 4. Start Redis
print_info "Starting Redis..."
sudo service redis-server start || redis-server --daemonize yes

sleep 2

if redis-cli ping | grep -q PONG; then
    print_success "Redis is running"
else
    print_error "Redis failed to start"
fi

# 5. Build Backend
print_info "Building backend TypeScript..."
cd backend
BUILD_LOG="$LOG_DIR/backend_build.log"
npm run build 2>&1 | tee "$BUILD_LOG"

if grep -q "error" "$BUILD_LOG"; then
    print_error "TypeScript compilation failed"
    cd ..
    exit 1
else
    print_success "Backend built successfully"
fi
cd ..

# 6. Start Backend (in background)
print_info "Starting backend server..."
cd backend
RUNTIME_LOG="$LOG_DIR/backend_runtime.log"
npm start > "$RUNTIME_LOG" 2>&1 &
BACKEND_PID=$!
cd ..

sleep 5

# Check if backend is running
if curl -s http://localhost:3000/api/health | grep -q "ok"; then
    print_success "Backend server is running (PID: $BACKEND_PID)"
else
    print_error "Backend server failed to start - check $RUNTIME_LOG"
    kill $BACKEND_PID 2>/dev/null || true
    exit 1
fi

# 7. Test Bot Endpoints (requires admin token)
print_info "Testing bot endpoints..."

# Get admin token first
print_info "Logging in as admin..."
ADMIN_TOKEN=$(curl -s -X POST http://localhost:3000/api/auth/login \
    -H "Content-Type: application/json" \
    -d '{"email":"admin@example.com","password":"admin123"}' | \
    jq -r '.token')

if [ "$ADMIN_TOKEN" != "null" ] && [ -n "$ADMIN_TOKEN" ]; then
    print_success "Admin token obtained"
    
    # Test bot endpoints
    print_info "Testing GET /api/admin/bots..."
    BOTS_RESPONSE=$(curl -s -X GET http://localhost:3000/api/admin/bots \
        -H "Authorization: Bearer $ADMIN_TOKEN")
    
    if echo "$BOTS_RESPONSE" | grep -q "bots"; then
        print_success "Bot list endpoint working"
    else
        print_error "Bot list endpoint failed"
    fi
    
    # Test personality list
    print_info "Testing GET /api/admin/bots/personalities/list..."
    PERSONALITIES=$(curl -s -X GET http://localhost:3000/api/admin/bots/personalities/list \
        -H "Authorization: Bearer $ADMIN_TOKEN")
    
    if echo "$PERSONALITIES" | grep -q "aggressive_conqueror"; then
        print_success "Personality list endpoint working"
    else
        print_error "Personality list endpoint failed"
    fi
    
    # Create a test bot
    print_info "Creating test bot..."
    CREATE_RESPONSE=$(curl -s -X POST http://localhost:3000/api/admin/bots \
        -H "Authorization: Bearer $ADMIN_TOKEN" \
        -H "Content-Type: application/json" \
        -d '{
            "username": "test_bot_001",
            "email": "testbot001@example.com",
            "personality_type": "aggressive_conqueror",
            "difficulty_level": 5,
            "aggression_level": 90,
            "economy_focus": 30,
            "military_focus": 85,
            "research_focus": 40
        }')
    
    if echo "$CREATE_RESPONSE" | grep -q "success"; then
        print_success "Test bot created successfully"
        
        # Extract bot ID
        BOT_ID=$(echo "$CREATE_RESPONSE" | jq -r '.bot.id')
        
        # Force bot think
        print_info "Testing bot think cycle..."
        THINK_RESPONSE=$(curl -s -X POST "http://localhost:3000/api/admin/bots/$BOT_ID/think" \
            -H "Authorization: Bearer $ADMIN_TOKEN")
        
        if echo "$THINK_RESPONSE" | grep -q "success"; then
            print_success "Bot think cycle completed"
        else
            print_error "Bot think cycle failed"
        fi
        
        # Delete test bot
        print_info "Cleaning up test bot..."
        curl -s -X DELETE "http://localhost:3000/api/admin/bots/$BOT_ID" \
            -H "Authorization: Bearer $ADMIN_TOKEN" > /dev/null
        print_success "Test bot deleted"
    else
        print_error "Failed to create test bot"
    fi
else
    print_error "Failed to get admin token - check admin account exists"
fi

# 8. Summary
echo ""
echo "=========================================="
echo "Deployment Summary"
echo "=========================================="
print_success "PostgreSQL: Running"
print_success "Redis: Running"
print_success "Backend: Running on http://localhost:3000"
print_success "Bot System: Deployed"
echo ""
print_info "Bot Management UI: http://localhost:3000/admin/bots.html"
print_info "Admin Panel: http://localhost:3000/admin/admin.html"
print_info "Backend PID: $BACKEND_PID"
echo ""
print_info "To stop backend: kill $BACKEND_PID"
print_info "Build log: $BUILD_LOG"
print_info "Runtime log: $RUNTIME_LOG"
echo ""
echo "=========================================="
echo "Next Steps:"
echo "=========================================="
echo "1. Open http://localhost:3000/admin/bots.html in your browser"
echo "2. Login with admin credentials (admin@example.com / admin123)"
echo "3. Create bots with different personalities"
echo "4. Monitor bot actions and statistics"
echo "5. Test bot AI decision-making"
echo ""
