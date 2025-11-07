#!/bin/bash

# Phase 6 Real-time Database Schema Deployment Script
# This script deploys the Phase 6 real-time communication systems schema

set -e

echo "====================================="
echo "Phase 6 Schema Deployment"
echo "====================================="
echo

# Database connection parameters
DB_HOST="${DB_HOST:-127.0.0.1}"
DB_PORT="${DB_PORT:-5432}"
DB_NAME="${DB_NAME:-universus_rpg}"
DB_USER="${DB_USER:-postgres}"
DB_PASSWORD="${DB_PASSWORD:-postgres}"

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to execute SQL
execute_sql() {
    local sql_file=$1
    echo -e "${YELLOW}Executing: $sql_file${NC}"
    PGPASSWORD=$DB_PASSWORD psql -h $DB_HOST -p $DB_PORT -U $DB_USER -d $DB_NAME -f "$sql_file" 2>&1
    if [ $? -eq 0 ]; then
        echo -e "${GREEN}✓ Successfully executed $sql_file${NC}"
        return 0
    else
        echo -e "${RED}✗ Failed to execute $sql_file${NC}"
        return 1
    fi
}

# Check PostgreSQL connection
echo "Checking database connection..."
PGPASSWORD=$DB_PASSWORD psql -h $DB_HOST -p $DB_PORT -U $DB_USER -d $DB_NAME -c "SELECT version();" > /dev/null 2>&1
if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ Database connection successful${NC}"
else
    echo -e "${RED}✗ Cannot connect to database${NC}"
    echo "Please ensure PostgreSQL is running and credentials are correct"
    exit 1
fi

# Deploy Phase 6 schema
echo
echo "Deploying Phase 6 Real-time Communication Systems schema..."
echo

SCHEMA_FILE="database/sql/phase6_realtime_schema.sql"

if [ ! -f "$SCHEMA_FILE" ]; then
    echo -e "${RED}✗ Schema file not found: $SCHEMA_FILE${NC}"
    exit 1
fi

execute_sql "$SCHEMA_FILE"

# Verify tables created
echo
echo "Verifying Phase 6 tables..."
echo

TABLES=(
    "chat_channels"
    "chat_messages"
    "private_messages"
    "private_conversations"
    "notifications"
    "notification_preferences"
    "notification_types"
    "player_status"
    "player_activity_log"
    "fleet_movement_events"
    "combat_alerts"
    "trading_offers"
    "trading_transactions"
    "chat_moderators"
    "chat_reports"
    "chat_bans"
    "alliance_announcements"
    "world_events"
)

MISSING_TABLES=0

for table in "${TABLES[@]}"; do
    COUNT=$(PGPASSWORD=$DB_PASSWORD psql -h $DB_HOST -p $DB_PORT -U $DB_USER -d $DB_NAME -t -c "SELECT COUNT(*) FROM information_schema.tables WHERE table_name='$table';" 2>/dev/null | tr -d ' ')
    
    if [ "$COUNT" = "1" ]; then
        echo -e "${GREEN}✓ Table exists: $table${NC}"
    else
        echo -e "${RED}✗ Table missing: $table${NC}"
        ((MISSING_TABLES++))
    fi
done

# Verify views
echo
echo "Verifying views..."

VIEWS=(
    "chat_activity_summary"
    "notification_statistics"
    "player_activity_summary"
    "fleet_movement_summary"
)

for view in "${VIEWS[@]}"; do
    COUNT=$(PGPASSWORD=$DB_PASSWORD psql -h $DB_HOST -p $DB_PORT -U $DB_USER -d $DB_NAME -t -c "SELECT COUNT(*) FROM information_schema.views WHERE table_name='$view';" 2>/dev/null | tr -d ' ')
    
    if [ "$COUNT" = "1" ]; then
        echo -e "${GREEN}✓ View exists: $view${NC}"
    else
        echo -e "${YELLOW}⚠ View missing: $view${NC}"
    fi
done

# Verify functions
echo
echo "Verifying functions..."

FUNCTIONS=(
    "mark_notification_as_read"
    "get_unread_notification_count"
    "update_player_last_activity"
    "log_player_activity"
)

for func in "${FUNCTIONS[@]}"; do
    COUNT=$(PGPASSWORD=$DB_PASSWORD psql -h $DB_HOST -p $DB_PORT -U $DB_USER -d $DB_NAME -t -c "SELECT COUNT(*) FROM information_schema.routines WHERE routine_name='$func';" 2>/dev/null | tr -d ' ')
    
    if [ "$COUNT" != "0" ]; then
        echo -e "${GREEN}✓ Function exists: $func${NC}"
    else
        echo -e "${YELLOW}⚠ Function missing: $func${NC}"
    fi
done

# Summary
echo
echo "====================================="
echo "Deployment Summary"
echo "====================================="
echo

if [ $MISSING_TABLES -eq 0 ]; then
    echo -e "${GREEN}✓ All 18 tables created successfully${NC}"
    echo -e "${GREEN}✓ Phase 6 schema deployment COMPLETE${NC}"
    echo
    echo "Next steps:"
    echo "  1. Start the backend server: cd backend && npm start"
    echo "  2. Run the comprehensive test suite: ./test-phase6-realtime.sh"
    exit 0
else
    echo -e "${RED}✗ $MISSING_TABLES table(s) missing${NC}"
    echo "Please review the error messages above and try again"
    exit 1
fi
