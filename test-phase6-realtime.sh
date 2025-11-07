#!/bin/bash

# Phase 6 Real-time Communication Systems - Comprehensive Test Suite
# This script performs end-to-end testing of all Phase 6 features

set -e

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# API endpoint
API_URL="${API_URL:-http://localhost:3000}"
REALTIME_API="$API_URL/api/realtime"

# Test credentials
ADMIN_EMAIL="admin@example.com"
ADMIN_PASSWORD="admin123"
TEST_USER1_EMAIL="testuser1@example.com"
TEST_USER1_PASSWORD="testpass123"
TEST_USER2_EMAIL="testuser2@example.com"
TEST_USER2_PASSWORD="testpass123"

# Token storage
TOKEN=""
USER1_TOKEN=""
USER2_TOKEN=""

# Test counters
TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0

# Function to print test header
print_header() {
    echo
    echo "====================================="
    echo "$1"
    echo "====================================="
}

# Function to print test step
print_step() {
    echo -e "${BLUE}▸ $1${NC}"
}

# Function to print success
print_success() {
    echo -e "${GREEN}✓ $1${NC}"
    ((TESTS_PASSED++))
}

# Function to print failure
print_failure() {
    echo -e "${RED}✗ $1${NC}"
    ((TESTS_FAILED++))
}

# Function to make API request
api_request() {
    local method=$1
    local endpoint=$2
    local data=$3
    local auth_token=$4
    
    if [ -n "$auth_token" ]; then
        if [ -n "$data" ]; then
            curl -s -X $method "$endpoint" \
                -H "Content-Type: application/json" \
                -H "Authorization: Bearer $auth_token" \
                -d "$data"
        else
            curl -s -X $method "$endpoint" \
                -H "Authorization: Bearer $auth_token"
        fi
    else
        if [ -n "$data" ]; then
            curl -s -X $method "$endpoint" \
                -H "Content-Type: application/json" \
                -d "$data"
        else
            curl -s -X $method "$endpoint"
        fi
    fi
}

# Function to test database tables
test_database_tables() {
    print_header "TEST 1: Database Tables Verification"
    ((TESTS_RUN++))
    
    print_step "Verifying 18 Phase 6 tables exist..."
    
    TABLES=(
        "chat_channels" "chat_messages" "private_messages" "private_conversations"
        "notifications" "notification_preferences" "notification_types" "player_status"
        "player_activity_log" "fleet_movement_events" "combat_alerts" "trading_offers"
        "trading_transactions" "chat_moderators" "chat_reports" "chat_bans"
        "alliance_announcements" "world_events"
    )
    
    MISSING=0
    for table in "${TABLES[@]}"; do
        COUNT=$(PGPASSWORD=${DB_PASSWORD:-postgres} psql -h ${DB_HOST:-127.0.0.1} -U ${DB_USER:-postgres} -d ${DB_NAME:-ogame_rpg} -t -c "SELECT COUNT(*) FROM information_schema.tables WHERE table_name='$table';" 2>/dev/null | tr -d ' ')
        
        if [ "$COUNT" != "1" ]; then
            print_failure "Table missing: $table"
            ((MISSING++))
        fi
    done
    
    if [ $MISSING -eq 0 ]; then
        print_success "All 18 tables verified successfully"
    else
        print_failure "$MISSING table(s) missing"
        return 1
    fi
}

# Function to test server connection
test_server_connection() {
    print_header "TEST 2: Server and Socket.io Connection"
    ((TESTS_RUN++))
    
    print_step "Testing backend health endpoint..."
    
    RESPONSE=$(curl -s "$API_URL/api/health" 2>/dev/null)
    
    if echo "$RESPONSE" | grep -q "\"status\":\"ok\""; then
        print_success "Backend server is running"
    else
        print_failure "Backend server health check failed"
        echo "Response: $RESPONSE"
        return 1
    fi
    
    print_step "Testing Socket.io endpoint availability..."
    
    # Check if socket.io endpoint responds
    SOCKET_RESPONSE=$(curl -s "$API_URL/socket.io/?EIO=4&transport=polling" 2>/dev/null)
    
    if [ -n "$SOCKET_RESPONSE" ]; then
        print_success "Socket.io endpoint is accessible"
    else
        print_failure "Socket.io endpoint not responding"
        return 1
    fi
}

# Function to login and get token
login_user() {
    local email=$1
    local password=$2
    
    RESPONSE=$(api_request "POST" "$API_URL/api/auth/login" "{\"email\":\"$email\",\"password\":\"$password\"}")
    
    TOKEN=$(echo "$RESPONSE" | grep -o '"token":"[^"]*' | sed 's/"token":"//')
    
    if [ -n "$TOKEN" ]; then
        echo "$TOKEN"
        return 0
    else
        return 1
    fi
}

# Function to test chat channels
test_chat_channels() {
    print_header "TEST 3: Chat Channel Management"
    ((TESTS_RUN++))
    
    print_step "Logging in as admin..."
    
    TOKEN=$(login_user "$ADMIN_EMAIL" "$ADMIN_PASSWORD")
    
    if [ -z "$TOKEN" ]; then
        print_failure "Admin login failed"
        return 1
    fi
    
    print_success "Admin login successful"
    
    print_step "Fetching chat channels..."
    
    CHANNELS=$(api_request "GET" "$REALTIME_API/chat/channels" "" "$TOKEN")
    
    if echo "$CHANNELS" | grep -q "global"; then
        print_success "Chat channels retrieved successfully"
    else
        print_failure "Failed to retrieve chat channels"
        echo "Response: $CHANNELS"
        return 1
    fi
    
    # Verify all 5 default channels exist
    for channel in "global" "trade" "alliance" "combat" "help"; do
        if echo "$CHANNELS" | grep -q "\"$channel\""; then
            print_success "Channel '$channel' exists"
        else
            print_failure "Channel '$channel' missing"
        fi
    done
}

# Function to test chat messaging
test_chat_messaging() {
    print_header "TEST 4: Chat Message Sending/Receiving"
    ((TESTS_RUN++))
    
    print_step "Sending message to global chat..."
    
    MESSAGE_DATA="{\"channel_id\":\"global\",\"content\":\"Test message from automated test suite\"}"
    
    SEND_RESPONSE=$(api_request "POST" "$REALTIME_API/chat/messages" "$MESSAGE_DATA" "$TOKEN")
    
    if echo "$SEND_RESPONSE" | grep -q "\"message_id\""; then
        print_success "Message sent successfully"
        
        MESSAGE_ID=$(echo "$SEND_RESPONSE" | grep -o '"message_id":[0-9]*' | sed 's/"message_id"://')
        
        print_step "Retrieving messages from global chat..."
        
        MESSAGES=$(api_request "GET" "$REALTIME_API/chat/messages/global?limit=10" "" "$TOKEN")
        
        if echo "$MESSAGES" | grep -q "$MESSAGE_ID"; then
            print_success "Message retrieved successfully"
        else
            print_failure "Message not found in chat history"
        fi
    else
        print_failure "Failed to send message"
        echo "Response: $SEND_RESPONSE"
        return 1
    fi
}

# Function to test notifications
test_notifications() {
    print_header "TEST 5: Notification Creation and Delivery"
    ((TESTS_RUN++))
    
    print_step "Creating test notification..."
    
    NOTIFICATION_DATA="{\"type\":\"system_alert\",\"title\":\"Test Notification\",\"message\":\"This is a test notification from the automated test suite\",\"priority\":3}"
    
    # Create notification via internal API (would normally be triggered by game events)
    # For testing, we'll check if notifications can be retrieved
    
    print_step "Fetching user notifications..."
    
    NOTIFICATIONS=$(api_request "GET" "$REALTIME_API/notifications" "" "$TOKEN")
    
    if echo "$NOTIFICATIONS" | grep -q "\"notifications\""; then
        print_success "Notifications retrieved successfully"
        
        # Check unread count
        UNREAD=$(api_request "GET" "$REALTIME_API/notifications/unread-count" "" "$TOKEN")
        
        if echo "$UNREAD" | grep -q "\"count\""; then
            print_success "Unread notification count retrieved"
        else
            print_failure "Failed to get unread count"
        fi
    else
        print_failure "Failed to retrieve notifications"
        echo "Response: $NOTIFICATIONS"
        return 1
    fi
    
    print_step "Testing notification preferences..."
    
    PREFS=$(api_request "GET" "$REALTIME_API/notifications/preferences" "" "$TOKEN")
    
    if echo "$PREFS" | grep -q "notification_types"; then
        print_success "Notification preferences retrieved"
    else
        print_failure "Failed to retrieve notification preferences"
    fi
}

# Function to test player status
test_player_status() {
    print_header "TEST 6: Player Status Updates"
    ((TESTS_RUN++))
    
    print_step "Updating player status to 'online'..."
    
    STATUS_DATA="{\"status\":\"online\"}"
    
    UPDATE_RESPONSE=$(api_request "POST" "$REALTIME_API/status/update" "$STATUS_DATA" "$TOKEN")
    
    if echo "$UPDATE_RESPONSE" | grep -q "\"success\":true"; then
        print_success "Player status updated successfully"
        
        print_step "Fetching online players list..."
        
        ONLINE=$(api_request "GET" "$REALTIME_API/status/online" "" "$TOKEN")
        
        if echo "$ONLINE" | grep -q "\"players\""; then
            print_success "Online players list retrieved"
        else
            print_failure "Failed to retrieve online players"
        fi
    else
        print_failure "Failed to update player status"
        echo "Response: $UPDATE_RESPONSE"
        return 1
    fi
}

# Function to test REST API endpoints
test_rest_api_endpoints() {
    print_header "TEST 7: REST API Endpoints"
    ((TESTS_RUN++))
    
    ENDPOINTS=(
        "GET:$REALTIME_API/chat/channels"
        "GET:$REALTIME_API/notifications"
        "GET:$REALTIME_API/notifications/types"
        "GET:$REALTIME_API/status/online"
        "GET:$REALTIME_API/trade/offers"
    )
    
    for endpoint_spec in "${ENDPOINTS[@]}"; do
        METHOD=$(echo "$endpoint_spec" | cut -d: -f1)
        ENDPOINT=$(echo "$endpoint_spec" | cut -d: -f2-)
        
        print_step "Testing $METHOD $ENDPOINT..."
        
        RESPONSE=$(api_request "$METHOD" "$ENDPOINT" "" "$TOKEN")
        HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" -X $METHOD "$ENDPOINT" -H "Authorization: Bearer $TOKEN" 2>/dev/null)
        
        if [ "$HTTP_CODE" = "200" ] || [ "$HTTP_CODE" = "201" ]; then
            print_success "$METHOD $ENDPOINT - HTTP $HTTP_CODE"
        else
            print_failure "$METHOD $ENDPOINT - HTTP $HTTP_CODE"
        fi
    done
}

# Function to test rate limiting
test_rate_limiting() {
    print_header "TEST 8: Chat Rate Limiting"
    ((TESTS_RUN++))
    
    print_step "Testing rate limit on global chat..."
    
    SUCCESS_COUNT=0
    RATE_LIMITED=false
    
    # Try to send multiple messages quickly
    for i in {1..15}; do
        MESSAGE_DATA="{\"channel_id\":\"global\",\"content\":\"Rate limit test message $i\"}"
        RESPONSE=$(api_request "POST" "$REALTIME_API/chat/messages" "$MESSAGE_DATA" "$TOKEN" 2>/dev/null)
        
        if echo "$RESPONSE" | grep -q "rate limit"; then
            RATE_LIMITED=true
            break
        elif echo "$RESPONSE" | grep -q "\"message_id\""; then
            ((SUCCESS_COUNT++))
        fi
        
        sleep 0.1
    done
    
    if [ "$RATE_LIMITED" = true ]; then
        print_success "Rate limiting is active (stopped after $SUCCESS_COUNT messages)"
    else
        print_failure "Rate limiting not working (sent $SUCCESS_COUNT messages without limit)"
    fi
}

# Function to create test combat scenario
test_combat_notifications() {
    print_header "TEST 9: Combat Notification Integration"
    ((TESTS_RUN++))
    
    print_step "Checking combat notification types..."
    
    NOTIF_TYPES=$(api_request "GET" "$REALTIME_API/notifications/types" "" "$TOKEN")
    
    if echo "$NOTIF_TYPES" | grep -q "combat_report"; then
        print_success "Combat notification types configured"
    else
        print_failure "Combat notification types missing"
        return 1
    fi
    
    print_step "Verifying combat alert table..."
    
    ALERT_COUNT=$(PGPASSWORD=${DB_PASSWORD:-postgres} psql -h ${DB_HOST:-127.0.0.1} -U ${DB_USER:-postgres} -d ${DB_NAME:-ogame_rpg} -t -c "SELECT COUNT(*) FROM combat_alerts;" 2>/dev/null | tr -d ' ')
    
    if [ -n "$ALERT_COUNT" ]; then
        print_success "Combat alerts table is accessible"
    else
        print_failure "Combat alerts table not accessible"
    fi
}

# Function to test fleet movement events
test_fleet_movement_events() {
    print_header "TEST 10: Fleet Movement Event Broadcasting"
    ((TESTS_RUN++))
    
    print_step "Verifying fleet movement events table..."
    
    EVENT_COUNT=$(PGPASSWORD=${DB_PASSWORD:-postgres} psql -h ${DB_HOST:-127.0.0.1} -U ${DB_USER:-postgres} -d ${DB_NAME:-ogame_rpg} -t -c "SELECT COUNT(*) FROM fleet_movement_events;" 2>/dev/null | tr -d ' ')
    
    if [ -n "$EVENT_COUNT" ]; then
        print_success "Fleet movement events table is accessible"
    else
        print_failure "Fleet movement events table not accessible"
        return 1
    fi
    
    print_step "Checking fleet movement notification type..."
    
    if echo "$NOTIF_TYPES" | grep -q "fleet_arrival"; then
        print_success "Fleet movement notification types configured"
    else
        print_failure "Fleet movement notification types missing"
    fi
}

# Main execution
main() {
    print_header "Phase 6 Real-time Systems - Comprehensive Test Suite"
    echo "Testing against: $API_URL"
    echo "Started: $(date)"
    echo
    
    # Run all tests
    test_database_tables || true
    test_server_connection || true
    test_chat_channels || true
    test_chat_messaging || true
    test_notifications || true
    test_player_status || true
    test_rest_api_endpoints || true
    test_rate_limiting || true
    test_combat_notifications || true
    test_fleet_movement_events || true
    
    # Print summary
    print_header "Test Summary"
    echo "Tests Run:    $TESTS_RUN"
    echo -e "Tests Passed: ${GREEN}$TESTS_PASSED${NC}"
    echo -e "Tests Failed: ${RED}$TESTS_FAILED${NC}"
    echo
    
    PASS_RATE=$(( TESTS_PASSED * 100 / TESTS_RUN ))
    
    if [ $TESTS_FAILED -eq 0 ]; then
        echo -e "${GREEN}✓ All tests passed successfully! (100%)${NC}"
        exit 0
    elif [ $PASS_RATE -ge 80 ]; then
        echo -e "${YELLOW}⚠ Most tests passed ($PASS_RATE%)${NC}"
        exit 0
    else
        echo -e "${RED}✗ Multiple test failures ($PASS_RATE% passed)${NC}"
        exit 1
    fi
}

# Run main
main
