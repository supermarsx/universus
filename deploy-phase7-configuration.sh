#!/bin/bash

##############################################################################
# PHASE 7: CONFIGURATION SYSTEM - DEPLOYMENT SCRIPT
# Deploys the comprehensive configuration management system
# - Applies database schema
# - Verifies all tables, views, and functions
# - Seeds initial configuration data
# - Tests API endpoints
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
SCHEMA_FILE="database/sql/phase7_config_schema.sql"

##############################################################################
# Helper Functions
##############################################################################

print_header() {
    echo -e "\n${BLUE}========================================${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}========================================${NC}\n"
}

print_success() {
    echo -e "${GREEN}✓${NC} $1"
}

print_error() {
    echo -e "${RED}✗${NC} $1"
}

print_info() {
    echo -e "${BLUE}ℹ${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}⚠${NC} $1"
}

##############################################################################
# Database Connection Test
##############################################################################

test_database_connection() {
    print_header "DATABASE CONNECTION TEST"
    
    if PGPASSWORD=$DB_PASSWORD psql -h $DB_HOST -p $DB_PORT -U $DB_USER -d $DB_NAME -c "SELECT 1" > /dev/null 2>&1; then
        print_success "Connected to database: $DB_NAME"
        return 0
    else
        print_error "Failed to connect to database: $DB_NAME"
        print_info "Host: $DB_HOST:$DB_PORT"
        print_info "User: $DB_USER"
        print_info "Database: $DB_NAME"
        return 1
    fi
}

##############################################################################
# Schema Deployment
##############################################################################

deploy_schema() {
    print_header "DEPLOYING PHASE 7 SCHEMA"
    
    if [ ! -f "$SCHEMA_FILE" ]; then
        print_error "Schema file not found: $SCHEMA_FILE"
        exit 1
    fi
    
    print_info "Applying schema from: $SCHEMA_FILE"
    
    if PGPASSWORD=$DB_PASSWORD psql -h $DB_HOST -p $DB_PORT -U $DB_USER -d $DB_NAME -f "$SCHEMA_FILE" > /tmp/phase7_deploy.log 2>&1; then
        print_success "Schema applied successfully"
        
        # Check for any warnings or notices
        if grep -i "notice\|warning" /tmp/phase7_deploy.log > /dev/null 2>&1; then
            print_warning "Schema deployment had some notices/warnings:"
            grep -i "notice\|warning" /tmp/phase7_deploy.log | head -5
        fi
    else
        print_error "Failed to apply schema"
        print_info "Check log file: /tmp/phase7_deploy.log"
        cat /tmp/phase7_deploy.log
        exit 1
    fi
}

##############################################################################
# Verification
##############################################################################

verify_tables() {
    print_header "VERIFYING TABLES"
    
    TABLES=(
        "config_categories"
        "config_parameters"
        "config_change_history"
        "config_templates"
        "config_template_items"
        "config_cache"
        "config_locks"
    )
    
    ALL_EXIST=true
    
    for table in "${TABLES[@]}"; do
        if PGPASSWORD=$DB_PASSWORD psql -h $DB_HOST -p $DB_PORT -U $DB_USER -d $DB_NAME -c "\dt $table" 2>/dev/null | grep -q $table; then
            print_success "Table: $table"
        else
            print_error "Table missing: $table"
            ALL_EXIST=false
        fi
    done
    
    if [ "$ALL_EXIST" = true ]; then
        print_success "All 7 tables verified"
        return 0
    else
        print_error "Some tables are missing"
        return 1
    fi
}

verify_views() {
    print_header "VERIFYING VIEWS"
    
    VIEWS=(
        "v_active_config"
        "v_recent_config_changes"
        "v_config_statistics"
    )
    
    ALL_EXIST=true
    
    for view in "${VIEWS[@]}"; do
        if PGPASSWORD=$DB_PASSWORD psql -h $DB_HOST -p $DB_PORT -U $DB_USER -d $DB_NAME -c "\dv $view" 2>/dev/null | grep -q $view; then
            print_success "View: $view"
        else
            print_error "View missing: $view"
            ALL_EXIST=false
        fi
    done
    
    if [ "$ALL_EXIST" = true ]; then
        print_success "All 3 views verified"
        return 0
    else
        print_error "Some views are missing"
        return 1
    fi
}

verify_functions() {
    print_header "VERIFYING FUNCTIONS"
    
    FUNCTIONS=(
        "get_config_value"
        "update_config_value"
        "rollback_config_change"
        "export_config_snapshot"
        "apply_config_template"
    )
    
    ALL_EXIST=true
    
    for func in "${FUNCTIONS[@]}"; do
        if PGPASSWORD=$DB_PASSWORD psql -h $DB_HOST -p $DB_PORT -U $DB_USER -d $DB_NAME -c "\df $func" 2>/dev/null | grep -q $func; then
            print_success "Function: $func"
        else
            print_error "Function missing: $func"
            ALL_EXIST=false
        fi
    done
    
    if [ "$ALL_EXIST" = true ]; then
        print_success "All 5 functions verified"
        return 0
    else
        print_error "Some functions are missing"
        return 1
    fi
}

verify_seeded_data() {
    print_header "VERIFYING SEEDED DATA"
    
    # Check categories
    CATEGORY_COUNT=$(PGPASSWORD=$DB_PASSWORD psql -h $DB_HOST -p $DB_PORT -U $DB_USER -d $DB_NAME -t -c "SELECT COUNT(*) FROM config_categories WHERE is_active = TRUE" | tr -d ' ')
    
    if [ "$CATEGORY_COUNT" -ge 10 ]; then
        print_success "Configuration categories: $CATEGORY_COUNT (expected >= 10)"
    else
        print_error "Configuration categories: $CATEGORY_COUNT (expected >= 10)"
    fi
    
    # Check parameters
    PARAM_COUNT=$(PGPASSWORD=$DB_PASSWORD psql -h $DB_HOST -p $DB_PORT -U $DB_USER -d $DB_NAME -t -c "SELECT COUNT(*) FROM config_parameters WHERE is_editable = TRUE" | tr -d ' ')
    
    if [ "$PARAM_COUNT" -ge 30 ]; then
        print_success "Configuration parameters: $PARAM_COUNT (expected >= 30)"
    else
        print_error "Configuration parameters: $PARAM_COUNT (expected >= 30)"
    fi
    
    # Show category breakdown
    print_info "Category breakdown:"
    PGPASSWORD=$DB_PASSWORD psql -h $DB_HOST -p $DB_PORT -U $DB_USER -d $DB_NAME -c "
        SELECT 
            cc.category_name,
            COUNT(cp.parameter_id) as param_count
        FROM config_categories cc
        LEFT JOIN config_parameters cp ON cc.category_id = cp.category_id
        WHERE cc.is_active = TRUE
        GROUP BY cc.category_name
        ORDER BY cc.display_order
    " | grep -v "rows)" | tail -n +3
}

verify_indexes() {
    print_header "VERIFYING INDEXES"
    
    INDEX_COUNT=$(PGPASSWORD=$DB_PASSWORD psql -h $DB_HOST -p $DB_PORT -U $DB_USER -d $DB_NAME -t -c "
        SELECT COUNT(*) 
        FROM pg_indexes 
        WHERE tablename IN (
            'config_categories',
            'config_parameters',
            'config_change_history',
            'config_templates',
            'config_template_items',
            'config_cache',
            'config_locks'
        )
    " | tr -d ' ')
    
    print_success "Found $INDEX_COUNT indexes on configuration tables"
    
    # Show index details
    print_info "Index breakdown:"
    PGPASSWORD=$DB_PASSWORD psql -h $DB_HOST -p $DB_PORT -U $DB_USER -d $DB_NAME -c "
        SELECT 
            tablename,
            indexname
        FROM pg_indexes 
        WHERE tablename IN (
            'config_categories',
            'config_parameters',
            'config_change_history',
            'config_templates',
            'config_template_items',
            'config_cache',
            'config_locks'
        )
        ORDER BY tablename, indexname
    " | grep -v "rows)" | tail -n +3
}

##############################################################################
# API Verification
##############################################################################

verify_api_endpoints() {
    print_header "VERIFYING API ENDPOINTS"
    
    print_info "Checking if backend server is running..."
    
    if curl -s http://localhost:3000/api/health > /dev/null 2>&1; then
        print_success "Backend server is running"
        
        print_info "Note: API endpoint testing requires admin authentication"
        print_info "Run './test-phase7-configuration.sh' for full API tests"
    else
        print_warning "Backend server is not running"
        print_info "Start the server with: cd backend && npm run dev"
    fi
}

##############################################################################
# Configuration System Information
##############################################################################

show_configuration_info() {
    print_header "CONFIGURATION SYSTEM INFORMATION"
    
    echo -e "${BLUE}Configuration Categories:${NC}"
    PGPASSWORD=$DB_PASSWORD psql -h $DB_HOST -p $DB_PORT -U $DB_USER -d $DB_NAME -c "
        SELECT 
            category_name,
            description,
            display_order
        FROM config_categories
        WHERE is_active = TRUE
        ORDER BY display_order
    " | grep -v "rows)" | tail -n +3
    
    echo -e "\n${BLUE}Sample Configuration Parameters:${NC}"
    PGPASSWORD=$DB_PASSWORD psql -h $DB_HOST -p $DB_PORT -U $DB_USER -d $DB_NAME -c "
        SELECT 
            cc.category_name,
            cp.parameter_key,
            cp.current_value,
            cp.data_type
        FROM config_parameters cp
        JOIN config_categories cc ON cp.category_id = cc.category_id
        WHERE cp.is_editable = TRUE
        LIMIT 10
    " | grep -v "rows)" | tail -n +3
}

##############################################################################
# Rollback Function
##############################################################################

rollback_deployment() {
    print_header "ROLLING BACK DEPLOYMENT"
    print_warning "This will drop all Phase 7 configuration tables"
    
    read -p "Are you sure you want to rollback? (yes/no): " confirm
    
    if [ "$confirm" = "yes" ]; then
        print_info "Dropping configuration tables..."
        
        PGPASSWORD=$DB_PASSWORD psql -h $DB_HOST -p $DB_PORT -U $DB_USER -d $DB_NAME << EOF
        DROP VIEW IF EXISTS v_config_statistics CASCADE;
        DROP VIEW IF EXISTS v_recent_config_changes CASCADE;
        DROP VIEW IF EXISTS v_active_config CASCADE;
        
        DROP FUNCTION IF EXISTS apply_config_template(INTEGER, INTEGER, TEXT) CASCADE;
        DROP FUNCTION IF EXISTS export_config_snapshot() CASCADE;
        DROP FUNCTION IF EXISTS rollback_config_change(INTEGER, INTEGER, TEXT) CASCADE;
        DROP FUNCTION IF EXISTS update_config_value(TEXT, TEXT, INTEGER, TEXT) CASCADE;
        DROP FUNCTION IF EXISTS get_config_value(TEXT) CASCADE;
        
        DROP TABLE IF EXISTS config_locks CASCADE;
        DROP TABLE IF EXISTS config_cache CASCADE;
        DROP TABLE IF EXISTS config_template_items CASCADE;
        DROP TABLE IF EXISTS config_templates CASCADE;
        DROP TABLE IF EXISTS config_change_history CASCADE;
        DROP TABLE IF EXISTS config_parameters CASCADE;
        DROP TABLE IF EXISTS config_categories CASCADE;
EOF
        
        print_success "Rollback completed"
    else
        print_info "Rollback cancelled"
    fi
}

##############################################################################
# Main Execution
##############################################################################

main() {
    print_header "PHASE 7: CONFIGURATION SYSTEM DEPLOYMENT"
    
    # Parse command line arguments
    case "${1:-}" in
        rollback)
            rollback_deployment
            exit 0
            ;;
        verify)
            print_info "Running verification only (no deployment)"
            test_database_connection || exit 1
            verify_tables || exit 1
            verify_views || exit 1
            verify_functions || exit 1
            verify_seeded_data
            verify_indexes
            show_configuration_info
            exit 0
            ;;
        *)
            # Normal deployment
            print_info "Starting Phase 7 deployment..."
            print_info "Database: $DB_NAME @ $DB_HOST:$DB_PORT"
            print_info "Schema file: $SCHEMA_FILE"
            echo ""
            
            # Execute deployment steps
            test_database_connection || exit 1
            deploy_schema
            verify_tables || exit 1
            verify_views || exit 1
            verify_functions || exit 1
            verify_seeded_data
            verify_indexes
            verify_api_endpoints
            show_configuration_info
            
            print_header "DEPLOYMENT COMPLETE"
            print_success "Phase 7: Configuration System deployed successfully"
            print_info "Next steps:"
            echo "  1. Start/restart the backend server"
            echo "  2. Run tests: ./test-phase7-configuration.sh"
            echo "  3. Access admin config UI: http://localhost:3000/admin/config"
            echo ""
            ;;
    esac
}

# Show usage if --help
if [ "${1:-}" = "--help" ] || [ "${1:-}" = "-h" ]; then
    echo "Phase 7 Configuration System Deployment Script"
    echo ""
    echo "Usage: $0 [command]"
    echo ""
    echo "Commands:"
    echo "  (none)     Deploy Phase 7 schema and verify installation"
    echo "  verify     Verify existing installation without deploying"
    echo "  rollback   Remove Phase 7 tables and functions"
    echo "  --help     Show this help message"
    echo ""
    echo "Environment Variables:"
    echo "  DB_HOST     Database host (default: localhost)"
    echo "  DB_PORT     Database port (default: 5432)"
    echo "  DB_NAME     Database name (default: universus_rpg)"
    echo "  DB_USER     Database user (default: postgres)"
    echo "  DB_PASSWORD Database password (default: postgres)"
    echo ""
    exit 0
fi

# Run main function
main "$@"
