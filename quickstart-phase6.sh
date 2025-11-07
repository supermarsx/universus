#!/bin/bash

# Phase 6 Quick Start Script
# Automated deployment and testing of Phase 6 Real-time Communication Systems

set -e

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo "=============================================="
echo "Phase 6 Real-time Systems - Quick Start"
echo "=============================================="
echo

# Function to check if a command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Function to check service status
check_service() {
    local service=$1
    local check_command=$2
    
    echo -n "Checking $service... "
    if eval "$check_command" >/dev/null 2>&1; then
        echo -e "${GREEN}✓ Running${NC}"
        return 0
    else
        echo -e "${RED}✗ Not running${NC}"
        return 1
    fi
}

# Step 1: Check prerequisites
echo "Step 1: Checking Prerequisites"
echo "--------------------------------"

PREREQ_FAILED=0

if command_exists node; then
    NODE_VERSION=$(node --version)
    echo -e "${GREEN}✓ Node.js installed: $NODE_VERSION${NC}"
else
    echo -e "${RED}✗ Node.js not found${NC}"
    ((PREREQ_FAILED++))
fi

if command_exists psql; then
    echo -e "${GREEN}✓ PostgreSQL client installed${NC}"
else
    echo -e "${YELLOW}⚠ PostgreSQL client not found${NC}"
    echo "  Attempting to install..."
    sudo apt-get install -y postgresql-client >/dev/null 2>&1 || true
fi

if command_exists redis-cli; then
    echo -e "${GREEN}✓ Redis client installed${NC}"
else
    echo -e "${YELLOW}⚠ Redis client not found${NC}"
fi

echo

# Step 2: Start services
echo "Step 2: Starting Services"
echo "-------------------------"

# Try to start PostgreSQL
echo "Starting PostgreSQL..."
if sudo service postgresql start >/dev/null 2>&1; then
    echo -e "${GREEN}✓ PostgreSQL service started${NC}"
elif sudo /etc/init.d/postgresql start >/dev/null 2>&1; then
    echo -e "${GREEN}✓ PostgreSQL service started (init.d)${NC}"
else
    echo -e "${YELLOW}⚠ Could not start PostgreSQL service${NC}"
    echo "  You may need to start it manually"
fi

sleep 2

# Try to start Redis
echo "Starting Redis..."
if sudo service redis-server start >/dev/null 2>&1; then
    echo -e "${GREEN}✓ Redis service started${NC}"
elif sudo /etc/init.d/redis-server start >/dev/null 2>&1; then
    echo -e "${GREEN}✓ Redis service started (init.d)${NC}"
elif redis-server --daemonize yes >/dev/null 2>&1; then
    echo -e "${GREEN}✓ Redis started in background${NC}"
else
    echo -e "${YELLOW}⚠ Could not start Redis service${NC}"
    echo "  You may need to start it manually"
fi

sleep 2

# Check service status
echo
check_service "PostgreSQL" "pg_isready -h 127.0.0.1 -p 5432 -U postgres"
POSTGRES_OK=$?

check_service "Redis" "redis-cli -h 127.0.0.1 ping"
REDIS_OK=$?

echo

if [ $POSTGRES_OK -ne 0 ] || [ $REDIS_OK -ne 0 ]; then
    echo -e "${RED}✗ Services not ready${NC}"
    echo
    echo "Please start the services manually:"
    echo "  PostgreSQL: sudo service postgresql start"
    echo "  Redis: sudo service redis-server start"
    echo
    echo "Then run this script again, or run the deployment manually:"
    echo "  ./deploy-phase6-schema.sh"
    exit 1
fi

# Step 3: Deploy schema
echo "Step 3: Deploying Database Schema"
echo "----------------------------------"

if [ -f "./deploy-phase6-schema.sh" ]; then
    echo "Running deployment script..."
    ./deploy-phase6-schema.sh
    DEPLOY_STATUS=$?
    
    if [ $DEPLOY_STATUS -eq 0 ]; then
        echo -e "${GREEN}✓ Schema deployed successfully${NC}"
    else
        echo -e "${RED}✗ Schema deployment failed${NC}"
        exit 1
    fi
else
    echo -e "${RED}✗ Deployment script not found${NC}"
    exit 1
fi

echo

# Step 4: Check if backend is running
echo "Step 4: Checking Backend Server"
echo "--------------------------------"

if curl -s http://localhost:3000/api/health >/dev/null 2>&1; then
    echo -e "${GREEN}✓ Backend server is already running${NC}"
    BACKEND_RUNNING=1
else
    echo -e "${YELLOW}⚠ Backend server not running${NC}"
    echo "Starting backend server..."
    
    cd backend
    
    # Check if node_modules exists
    if [ ! -d "node_modules" ]; then
        echo "Installing dependencies..."
        npm install
    fi
    
    # Start backend in background
    echo "Starting server..."
    npm start > /tmp/backend.log 2>&1 &
    BACKEND_PID=$!
    
    echo "Backend started (PID: $BACKEND_PID)"
    echo "Waiting for server to be ready..."
    
    # Wait for server to start (max 30 seconds)
    for i in {1..30}; do
        if curl -s http://localhost:3000/api/health >/dev/null 2>&1; then
            echo -e "${GREEN}✓ Backend server is running${NC}"
            BACKEND_RUNNING=1
            break
        fi
        sleep 1
        echo -n "."
    done
    echo
    
    if [ $BACKEND_RUNNING -ne 1 ]; then
        echo -e "${RED}✗ Backend server failed to start${NC}"
        echo "Check logs: tail -f /tmp/backend.log"
        exit 1
    fi
    
    cd ..
fi

echo

# Step 5: Run tests
echo "Step 5: Running Comprehensive Tests"
echo "------------------------------------"

if [ -f "./test-phase6-realtime.sh" ]; then
    echo "Running test suite..."
    ./test-phase6-realtime.sh
    TEST_STATUS=$?
    
    if [ $TEST_STATUS -eq 0 ]; then
        echo -e "${GREEN}✓ All tests passed${NC}"
    else
        echo -e "${YELLOW}⚠ Some tests failed (see above)${NC}"
    fi
else
    echo -e "${YELLOW}⚠ Test script not found, skipping tests${NC}"
fi

echo

# Final summary
echo "=============================================="
echo "Phase 6 Deployment Complete"
echo "=============================================="
echo
echo "Services:"
echo -e "  PostgreSQL: ${GREEN}Running${NC}"
echo -e "  Redis: ${GREEN}Running${NC}"
echo -e "  Backend: ${GREEN}Running on http://localhost:3000${NC}"
echo
echo "Next steps:"
echo "  1. Access chat interface: http://localhost:3000/chat"
echo "  2. Test real-time features"
echo "  3. Review logs: tail -f /tmp/backend.log"
echo
echo "Documentation:"
echo "  - PHASE6_DEPLOYMENT_STATUS_REPORT.md - Complete status report"
echo "  - PHASE6_DEPLOYMENT_TESTING_GUIDE.md - Detailed guide"
echo "  - PHASE6_REALTIME_IMPLEMENTATION_COMPLETE.md - Technical docs"
echo
echo -e "${GREEN}✓ Phase 6 Real-time Communication Systems ready!${NC}"
