#!/bin/bash

##############################################################################
# PHASE 7: CONFIGURATION SYSTEM - COMPREHENSIVE TEST SUITE
# Tests all configuration management features including:
# - Database schema and tables
# - REST API endpoints
# - Configuration CRUD operations
# - Validation and rollback
# - Template management
# - Import/Export functionality
# - Real-time Socket.io updates
# - Integration with game systems
##############################################################################

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
DB_HOST="${DB_HOST:-localhost}"
DB_PORT="${DB_PORT:-5432}"
DB_NAME="${DB_NAME:-universus_rpg}"
DB_USER="${DB_USER:-postgres}"
DB_PASSWORD="${DB_PASSWORD:-postgres}"
API_URL="${API_URL:-http://localhost:3000}"

# Test counters
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0

# Admin token (will be obtained via login)
ADMIN_TOKEN=""

##############################################################################
# Helper Functions
##############################################################################

print_header() {
    echo -e "\n${BLUE}========================================${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}========================================${NC}\n"
}

print_test() {
    echo -e "${YELLOW}TEST $((TOTAL_TESTS + 1)):${NC} $1"
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
}

print_success() {
    echo -e "${GREEN}✓ PASS:${NC} $1"
    PASSED_TESTS=$((PASSED_TESTS + 1))
}

print_failure() {
    echo -e "${RED}✗ FAIL:${NC} $1"
    FAILED_TESTS=$((FAILED_TESTS + 1))
}

print_info() {
    echo -e "${BLUE}ℹ INFO:${NC} $1"
}

##############################################################################
# Database Tests
##############################################################################

test_database_tables() {
    print_header "DATABASE SCHEMA TESTS"
    
    print_test "Verify configuration tables exist"
    TABLES=(
        "config_categories"
        "config_parameters"
        "config_change_history"
        "config_templates"
        "config_template_items"
        "config_cache"
        "config_locks"
    )
    
    for table in "${TABLES[@]}"; do
        if PGPASSWORD=$DB_PASSWORD psql -h $DB_HOST -p $DB_PORT -U $DB_USER -d $DB_NAME -c "\dt $table" | grep -q $table; then
            print_success "Table '$table' exists"
        else
            print_failure "Table '$table' missing"
        fi
    done
    
    print_test "Verify configuration views exist"
    VIEWS=(
        "v_active_config"
        "v_recent_config_changes"
        "v_config_statistics"
    )
    
    for view in "${VIEWS[@]}"; do
        if PGPASSWORD=$DB_PASSWORD psql -h $DB_HOST -p $DB_PORT -U $DB_USER -d $DB_NAME -c "\dv $view" | grep -q $view; then
            print_success "View '$view' exists"
        else
            print_failure "View '$view' missing"
        fi
    done
    
    print_test "Verify configuration functions exist"
    FUNCTIONS=(
        "get_config_value"
        "update_config_value"
        "rollback_config_change"
        "export_config_snapshot"
        "apply_config_template"
    )
    
    for func in "${FUNCTIONS[@]}"; do
        if PGPASSWORD=$DB_PASSWORD psql -h $DB_HOST -p $DB_PORT -U $DB_USER -d $DB_NAME -c "\df $func" | grep -q $func; then
            print_success "Function '$func' exists"
        else
            print_failure "Function '$func' missing"
        fi
    done
    
    print_test "Verify seeded categories"
    CATEGORY_COUNT=$(PGPASSWORD=$DB_PASSWORD psql -h $DB_HOST -p $DB_PORT -U $DB_USER -d $DB_NAME -t -c "SELECT COUNT(*) FROM config_categories WHERE is_active = TRUE")
    if [ "$CATEGORY_COUNT" -ge 10 ]; then
        print_success "Found $CATEGORY_COUNT configuration categories"
    else
        print_failure "Expected at least 10 categories, found $CATEGORY_COUNT"
    fi
    
    print_test "Verify seeded parameters"
    PARAM_COUNT=$(PGPASSWORD=$DB_PASSWORD psql -h $DB_HOST -p $DB_PORT -U $DB_USER -d $DB_NAME -t -c "SELECT COUNT(*) FROM config_parameters WHERE is_editable = TRUE")
    if [ "$PARAM_COUNT" -ge 30 ]; then
        print_success "Found $PARAM_COUNT configuration parameters"
    else
        print_failure "Expected at least 30 parameters, found $PARAM_COUNT"
    fi
}

##############################################################################
# Authentication Tests
##############################################################################

test_admin_authentication() {
    print_header "AUTHENTICATION TESTS"
    
    print_test "Admin login"
    LOGIN_RESPONSE=$(curl -s -X POST "$API_URL/api/auth/login" \
        -H "Content-Type: application/json" \
        -d '{"username":"admin","password":"admin123"}')
    
    ADMIN_TOKEN=$(echo $LOGIN_RESPONSE | grep -o '"token":"[^"]*' | cut -d'"' -f4)
    
    if [ -n "$ADMIN_TOKEN" ]; then
        print_success "Admin authentication successful"
    else
        print_failure "Admin authentication failed"
        echo "Response: $LOGIN_RESPONSE"
        exit 1
    fi
}

##############################################################################
# Configuration API Tests
##############################################################################

test_configuration_categories() {
    print_header "CONFIGURATION CATEGORY TESTS"
    
    print_test "GET /api/config/categories - List all categories"
    RESPONSE=$(curl -s -X GET "$API_URL/api/config/categories" \
        -H "Authorization: Bearer $ADMIN_TOKEN")
    
    if echo $RESPONSE | grep -q "category_id"; then
        print_success "Retrieved configuration categories"
    else
        print_failure "Failed to retrieve categories"
    fi
}

test_configuration_parameters() {
    print_header "CONFIGURATION PARAMETER TESTS"
    
    print_test "GET /api/config/parameters - List all parameters"
    RESPONSE=$(curl -s -X GET "$API_URL/api/config/parameters" \
        -H "Authorization: Bearer $ADMIN_TOKEN")
    
    if echo $RESPONSE | grep -q "parameter_key"; then
        print_success "Retrieved configuration parameters"
    else
        print_failure "Failed to retrieve parameters"
    fi
    
    print_test "GET /api/config/parameters?category=combat - Filter by category"
    RESPONSE=$(curl -s -X GET "$API_URL/api/config/parameters?category=combat" \
        -H "Authorization: Bearer $ADMIN_TOKEN")
    
    if echo $RESPONSE | grep -q "combat"; then
        print_success "Retrieved combat parameters"
    else
        print_failure "Failed to filter by category"
    fi
}

test_configuration_crud() {
    print_header "CONFIGURATION CRUD TESTS"
    
    print_test "GET /api/config/config/combat.max_rounds - Get single parameter"
    RESPONSE=$(curl -s -X GET "$API_URL/api/config/config/combat.max_rounds" \
        -H "Authorization: Bearer $ADMIN_TOKEN")
    
    if echo $RESPONSE | grep -q "current_value"; then
        print_success "Retrieved single configuration parameter"
    else
        print_failure "Failed to retrieve parameter"
    fi
    
    print_test "PUT /api/config/parameters/combat.max_rounds - Update parameter"
    RESPONSE=$(curl -s -X PUT "$API_URL/api/config/parameters/combat.max_rounds" \
        -H "Authorization: Bearer $ADMIN_TOKEN" \
        -H "Content-Type: application/json" \
        -d '{"value":10,"reason":"Testing configuration update"}')
    
    if echo $RESPONSE | grep -q "success.*true"; then
        print_success "Updated configuration parameter"
    else
        print_failure "Failed to update parameter"
    fi
    
    print_test "POST /api/config/bulk-update - Bulk update parameters"
    RESPONSE=$(curl -s -X POST "$API_URL/api/config/bulk-update" \
        -H "Authorization: Bearer $ADMIN_TOKEN" \
        -H "Content-Type: application/json" \
        -d '{
            "updates":[
                {"parameter_key":"combat.max_rounds","value":6},
                {"parameter_key":"resources.metal_production_base","value":30}
            ],
            "change_reason":"Bulk testing"
        }')
    
    if echo $RESPONSE | grep -q "updated_count"; then
        print_success "Bulk updated configuration parameters"
    else
        print_failure "Failed to bulk update"
    fi
}

test_configuration_history() {
    print_header "CONFIGURATION HISTORY TESTS"
    
    print_test "GET /api/config/history - Get change history"
    RESPONSE=$(curl -s -X GET "$API_URL/api/config/history?limit=10" \
        -H "Authorization: Bearer $ADMIN_TOKEN")
    
    if echo $RESPONSE | grep -q "change_id"; then
        print_success "Retrieved configuration history"
    else
        print_failure "Failed to retrieve history"
    fi
    
    print_test "GET /api/config/history/combat.max_rounds - Get parameter history"
    RESPONSE=$(curl -s -X GET "$API_URL/api/config/history/combat.max_rounds" \
        -H "Authorization: Bearer $ADMIN_TOKEN")
    
    if echo $RESPONSE | grep -q "combat.max_rounds"; then
        print_success "Retrieved parameter-specific history"
    else
        print_failure "Failed to retrieve parameter history"
    fi
}

test_configuration_rollback() {
    print_header "CONFIGURATION ROLLBACK TESTS"
    
    # First, make a change
    print_test "Make configuration change for rollback test"
    curl -s -X PUT "$API_URL/api/config/parameters/combat.max_rounds" \
        -H "Authorization: Bearer $ADMIN_TOKEN" \
        -H "Content-Type: application/json" \
        -d '{"value":99,"reason":"Testing rollback"}' > /dev/null
    
    # Get the change ID
    CHANGE_ID=$(curl -s -X GET "$API_URL/api/config/history/combat.max_rounds?limit=1" \
        -H "Authorization: Bearer $ADMIN_TOKEN" | grep -o '"change_id":[0-9]*' | head -1 | cut -d':' -f2)
    
    if [ -n "$CHANGE_ID" ]; then
        print_test "POST /api/config/rollback - Rollback configuration change"
        RESPONSE=$(curl -s -X POST "$API_URL/api/config/rollback" \
            -H "Authorization: Bearer $ADMIN_TOKEN" \
            -H "Content-Type: application/json" \
            -d "{\"change_id\":$CHANGE_ID,\"reason\":\"Testing rollback\"}")
        
        if echo $RESPONSE | grep -q "success.*true"; then
            print_success "Successfully rolled back configuration change"
        else
            print_failure "Failed to rollback change"
        fi
    else
        print_failure "Could not find change ID for rollback test"
    fi
}

test_configuration_templates() {
    print_header "CONFIGURATION TEMPLATE TESTS"
    
    print_test "POST /api/config/templates - Create template"
    TEMPLATE_RESPONSE=$(curl -s -X POST "$API_URL/api/config/templates" \
        -H "Authorization: Bearer $ADMIN_TOKEN" \
        -H "Content-Type: application/json" \
        -d '{
            "template_name":"Test Speed Server",
            "description":"2x speed configuration for testing",
            "parameters":[
                {"parameter_key":"resources.metal_production_base","value":60},
                {"parameter_key":"resources.crystal_production_base","value":30}
            ]
        }')
    
    TEMPLATE_ID=$(echo $TEMPLATE_RESPONSE | grep -o '"template_id":[0-9]*' | cut -d':' -f2)
    
    if [ -n "$TEMPLATE_ID" ]; then
        print_success "Created configuration template (ID: $TEMPLATE_ID)"
    else
        print_failure "Failed to create template"
    fi
    
    print_test "GET /api/config/templates - List all templates"
    RESPONSE=$(curl -s -X GET "$API_URL/api/config/templates" \
        -H "Authorization: Bearer $ADMIN_TOKEN")
    
    if echo $RESPONSE | grep -q "template_id"; then
        print_success "Retrieved configuration templates"
    else
        print_failure "Failed to retrieve templates"
    fi
    
    if [ -n "$TEMPLATE_ID" ]; then
        print_test "POST /api/config/templates/$TEMPLATE_ID/apply - Apply template"
        RESPONSE=$(curl -s -X POST "$API_URL/api/config/templates/$TEMPLATE_ID/apply" \
            -H "Authorization: Bearer $ADMIN_TOKEN" \
            -H "Content-Type: application/json" \
            -d '{"reason":"Testing template application"}')
        
        if echo $RESPONSE | grep -q "success.*true"; then
            print_success "Applied configuration template"
        else
            print_failure "Failed to apply template"
        fi
        
        print_test "DELETE /api/config/templates/$TEMPLATE_ID - Delete template"
        RESPONSE=$(curl -s -X DELETE "$API_URL/api/config/templates/$TEMPLATE_ID" \
            -H "Authorization: Bearer $ADMIN_TOKEN")
        
        if echo $RESPONSE | grep -q "success.*true"; then
            print_success "Deleted configuration template"
        else
            print_failure "Failed to delete template"
        fi
    fi
}

test_configuration_import_export() {
    print_header "CONFIGURATION IMPORT/EXPORT TESTS"
    
    print_test "GET /api/config/export - Export configuration"
    EXPORT_DATA=$(curl -s -X GET "$API_URL/api/config/export" \
        -H "Authorization: Bearer $ADMIN_TOKEN")
    
    if echo $EXPORT_DATA | grep -q "parameters"; then
        print_success "Exported configuration"
        
        # Save export for import test
        echo $EXPORT_DATA > /tmp/config_export_test.json
    else
        print_failure "Failed to export configuration"
    fi
    
    if [ -f /tmp/config_export_test.json ]; then
        print_test "POST /api/config/import - Import configuration"
        RESPONSE=$(curl -s -X POST "$API_URL/api/config/import" \
            -H "Authorization: Bearer $ADMIN_TOKEN" \
            -H "Content-Type: application/json" \
            -d "@/tmp/config_export_test.json")
        
        if echo $RESPONSE | grep -q "success.*true"; then
            print_success "Imported configuration"
        else
            print_failure "Failed to import configuration"
        fi
        
        rm -f /tmp/config_export_test.json
    fi
}

test_configuration_validation() {
    print_header "CONFIGURATION VALIDATION TESTS"
    
    print_test "POST /api/config/validate - Validate configuration"
    RESPONSE=$(curl -s -X POST "$API_URL/api/config/validate" \
        -H "Authorization: Bearer $ADMIN_TOKEN" \
        -H "Content-Type: application/json" \
        -d '{"parameter_key":"combat.max_rounds","value":6}')
    
    if echo $RESPONSE | grep -q "is_valid"; then
        print_success "Validated configuration value"
    else
        print_failure "Failed to validate configuration"
    fi
    
    print_test "Validate with invalid value (negative number)"
    RESPONSE=$(curl -s -X POST "$API_URL/api/config/validate" \
        -H "Authorization: Bearer $ADMIN_TOKEN" \
        -H "Content-Type: application/json" \
        -d '{"parameter_key":"combat.max_rounds","value":-5}')
    
    if echo $RESPONSE | grep -q "is_valid.*false"; then
        print_success "Correctly rejected invalid value"
    else
        print_failure "Failed to reject invalid value"
    fi
}

test_configuration_search() {
    print_header "CONFIGURATION SEARCH TESTS"
    
    print_test "GET /api/config/search?query=combat - Search configuration"
    RESPONSE=$(curl -s -X GET "$API_URL/api/config/search?query=combat" \
        -H "Authorization: Bearer $ADMIN_TOKEN")
    
    if echo $RESPONSE | grep -q "combat"; then
        print_success "Searched configuration successfully"
    else
        print_failure "Failed to search configuration"
    fi
}

test_configuration_statistics() {
    print_header "CONFIGURATION STATISTICS TESTS"
    
    print_test "GET /api/config/stats - Get configuration statistics"
    RESPONSE=$(curl -s -X GET "$API_URL/api/config/stats" \
        -H "Authorization: Bearer $ADMIN_TOKEN")
    
    if echo $RESPONSE | grep -q "total_parameters"; then
        print_success "Retrieved configuration statistics"
    else
        print_failure "Failed to retrieve statistics"
    fi
}

##############################################################################
# Real-time Update Tests
##############################################################################

test_realtime_updates() {
    print_header "REAL-TIME UPDATE TESTS"
    
    print_info "Real-time Socket.io tests require manual verification"
    print_info "Connect to Socket.io server and subscribe to 'config:updates' channel"
    print_info "Make configuration changes and verify events are received"
    print_info "Expected events: config:changed, config:bulk_update, config:reload"
}

##############################################################################
# Integration Tests
##############################################################################

test_configuration_integration() {
    print_header "CONFIGURATION INTEGRATION TESTS"
    
    print_info "Integration tests verify configuration is used by game systems"
    print_info "These tests require the game to be running with active services"
    
    print_test "Verify combat system uses configuration"
    print_info "Manual: Start a combat and verify max_rounds is respected"
    
    print_test "Verify resource system uses configuration"
    print_info "Manual: Check resource production rates match configuration"
    
    print_test "Verify building system uses configuration"
    print_info "Manual: Verify building costs and times use configuration"
    
    print_test "Verify research system uses configuration"
    print_info "Manual: Check research speeds match configuration"
    
    print_test "Verify fleet system uses configuration"
    print_info "Manual: Verify ship costs and speeds use configuration"
}

##############################################################################
# Main Test Execution
##############################################################################

main() {
    print_header "PHASE 7: CONFIGURATION SYSTEM TEST SUITE"
    print_info "Starting comprehensive configuration system tests"
    print_info "API URL: $API_URL"
    print_info "Database: $DB_HOST:$DB_PORT/$DB_NAME"
    
    # Run test suites
    test_database_tables
    test_admin_authentication
    test_configuration_categories
    test_configuration_parameters
    test_configuration_crud
    test_configuration_history
    test_configuration_rollback
    test_configuration_templates
    test_configuration_import_export
    test_configuration_validation
    test_configuration_search
    test_configuration_statistics
    test_realtime_updates
    test_configuration_integration
    
    # Print summary
    print_header "TEST SUMMARY"
    echo -e "Total Tests:  ${BLUE}$TOTAL_TESTS${NC}"
    echo -e "Passed:       ${GREEN}$PASSED_TESTS${NC}"
    echo -e "Failed:       ${RED}$FAILED_TESTS${NC}"
    
    if [ $FAILED_TESTS -eq 0 ]; then
        echo -e "\n${GREEN}✓ ALL TESTS PASSED${NC}\n"
        exit 0
    else
        echo -e "\n${RED}✗ SOME TESTS FAILED${NC}\n"
        exit 1
    fi
}

# Run main function
main
