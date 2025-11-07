#!/bin/bash

###############################################################################
# Phase 9: Advanced Account Management System - Database Deployment Script
# 
# This script deploys the Phase 9 database schema for account management
# Features: Security, Sessions, 2FA, Email Verification, Password Recovery,
#           GDPR Compliance, Account Transfer
###############################################################################

set -e  # Exit on error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Database connection parameters
DB_HOST="${DB_HOST:-localhost}"
DB_PORT="${DB_PORT:-5432}"
DB_NAME="${DB_NAME:-universus_db}"
DB_USER="${DB_USER:-postgres}"
DB_PASSWORD="${DB_PASSWORD:-}"

# Script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCHEMA_FILE="${SCRIPT_DIR}/database/sql/phase9_account_management_schema.sql"

###############################################################################
# Functions
###############################################################################

print_header() {
    echo -e "${BLUE}"
    echo "=================================================="
    echo "  Phase 9: Account Management Schema Deployment"
    echo "=================================================="
    echo -e "${NC}"
}

print_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

print_error() {
    echo -e "${RED}✗ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠ $1${NC}"
}

print_info() {
    echo -e "${BLUE}ℹ $1${NC}"
}

check_prerequisites() {
    print_info "Checking prerequisites..."
    
    # Check if psql is installed
    if ! command -v psql &> /dev/null; then
        print_error "PostgreSQL client (psql) is not installed"
        exit 1
    fi
    print_success "PostgreSQL client found"
    
    # Check if schema file exists
    if [ ! -f "$SCHEMA_FILE" ]; then
        print_error "Schema file not found: $SCHEMA_FILE"
        exit 1
    fi
    print_success "Schema file found"
}

test_connection() {
    print_info "Testing database connection..."
    
    if [ -n "$DB_PASSWORD" ]; then
        export PGPASSWORD="$DB_PASSWORD"
    fi
    
    if psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -c "SELECT 1;" &> /dev/null; then
        print_success "Database connection successful"
    else
        print_error "Failed to connect to database"
        print_info "Connection details:"
        echo "  Host: $DB_HOST"
        echo "  Port: $DB_PORT"
        echo "  Database: $DB_NAME"
        echo "  User: $DB_USER"
        exit 1
    fi
}

check_existing_tables() {
    print_info "Checking for existing Phase 9 tables..."
    
    EXISTING_TABLES=$(psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -t -c "
        SELECT COUNT(*) FROM information_schema.tables 
        WHERE table_schema = 'public' 
        AND table_name IN (
            'account_suspensions',
            'account_transfers',
            'email_verifications',
            'password_resets',
            'two_factor_auth',
            'user_sessions',
            'security_audit_logs',
            'gdpr_requests',
            'user_blocks',
            'user_activity_logs',
            'account_data_backups',
            'backup_verification_codes'
        );
    " | tr -d ' ')
    
    if [ "$EXISTING_TABLES" -gt 0 ]; then
        print_warning "Found $EXISTING_TABLES existing Phase 9 tables"
        read -p "Do you want to drop and recreate them? (y/N) " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            print_info "Dropping existing tables..."
            psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -c "
                DROP TABLE IF EXISTS backup_verification_codes CASCADE;
                DROP TABLE IF EXISTS account_data_backups CASCADE;
                DROP TABLE IF EXISTS user_activity_logs CASCADE;
                DROP TABLE IF EXISTS user_blocks CASCADE;
                DROP TABLE IF EXISTS gdpr_requests CASCADE;
                DROP TABLE IF EXISTS security_audit_logs CASCADE;
                DROP TABLE IF EXISTS user_sessions CASCADE;
                DROP TABLE IF EXISTS two_factor_auth CASCADE;
                DROP TABLE IF EXISTS password_resets CASCADE;
                DROP TABLE IF EXISTS email_verifications CASCADE;
                DROP TABLE IF EXISTS account_transfers CASCADE;
                DROP TABLE IF EXISTS account_suspensions CASCADE;
            " &> /dev/null
            print_success "Existing tables dropped"
        else
            print_warning "Deployment cancelled by user"
            exit 0
        fi
    else
        print_success "No existing Phase 9 tables found"
    fi
}

deploy_schema() {
    print_info "Deploying Phase 9 schema..."
    
    if psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -f "$SCHEMA_FILE" &> /tmp/phase9_deploy.log; then
        print_success "Schema deployed successfully"
    else
        print_error "Schema deployment failed"
        print_info "Check log file for details: /tmp/phase9_deploy.log"
        cat /tmp/phase9_deploy.log
        exit 1
    fi
}

verify_deployment() {
    print_info "Verifying deployment..."
    
    # Verify tables
    TABLES_COUNT=$(psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -t -c "
        SELECT COUNT(*) FROM information_schema.tables 
        WHERE table_schema = 'public' 
        AND table_name IN (
            'account_suspensions',
            'account_transfers',
            'email_verifications',
            'password_resets',
            'two_factor_auth',
            'user_sessions',
            'security_audit_logs',
            'gdpr_requests',
            'user_blocks',
            'user_activity_logs',
            'account_data_backups',
            'backup_verification_codes'
        );
    " | tr -d ' ')
    
    if [ "$TABLES_COUNT" -eq 12 ]; then
        print_success "All 12 tables created successfully"
    else
        print_error "Expected 12 tables, found $TABLES_COUNT"
        exit 1
    fi
    
    # Verify views
    VIEWS_COUNT=$(psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -t -c "
        SELECT COUNT(*) FROM information_schema.views 
        WHERE table_schema = 'public' 
        AND table_name IN (
            'active_user_sessions_view',
            'security_risk_assessment_view',
            'gdpr_compliance_status_view'
        );
    " | tr -d ' ')
    
    if [ "$VIEWS_COUNT" -eq 3 ]; then
        print_success "All 3 views created successfully"
    else
        print_error "Expected 3 views, found $VIEWS_COUNT"
        exit 1
    fi
    
    # Verify functions
    FUNCTIONS_COUNT=$(psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -t -c "
        SELECT COUNT(*) FROM pg_proc 
        WHERE proname IN (
            'check_account_access',
            'log_security_event',
            'cleanup_expired_sessions',
            'generate_backup_codes',
            'validate_2fa_code'
        );
    " | tr -d ' ')
    
    if [ "$FUNCTIONS_COUNT" -eq 5 ]; then
        print_success "All 5 functions created successfully"
    else
        print_warning "Expected 5 functions, found $FUNCTIONS_COUNT (may include overloads)"
    fi
    
    # Verify user table enhancements
    USER_COLUMNS=$(psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -t -c "
        SELECT COUNT(*) FROM information_schema.columns 
        WHERE table_name = 'users' 
        AND column_name IN (
            'email_verified',
            'email_verified_at',
            'two_factor_enabled',
            'failed_login_attempts',
            'account_locked_until',
            'account_status',
            'last_password_change',
            'password_reset_required'
        );
    " | tr -d ' ')
    
    if [ "$USER_COLUMNS" -eq 8 ]; then
        print_success "User table enhanced with 8 new columns"
    else
        print_warning "Expected 8 new user columns, found $USER_COLUMNS"
    fi
}

print_summary() {
    echo ""
    print_header
    print_success "Phase 9 deployment completed successfully!"
    echo ""
    echo "Deployed components:"
    echo "  - 12 new tables for account management"
    echo "  - 3 analytical views"
    echo "  - 5 utility functions"
    echo "  - 8 enhanced user table columns"
    echo ""
    echo "Features enabled:"
    echo "  ✓ Account suspension and deletion"
    echo "  ✓ Multi-session management"
    echo "  ✓ Two-factor authentication (TOTP)"
    echo "  ✓ Email verification"
    echo "  ✓ Password recovery"
    echo "  ✓ GDPR compliance (data export/deletion)"
    echo "  ✓ Account ownership transfer"
    echo "  ✓ Security audit logging"
    echo ""
    print_info "Backend API routes available at: /api/account/*"
    print_info "Frontend interfaces available at: /account/*"
    echo ""
}

###############################################################################
# Main Execution
###############################################################################

main() {
    print_header
    
    check_prerequisites
    test_connection
    check_existing_tables
    deploy_schema
    verify_deployment
    print_summary
    
    # Clean up
    unset PGPASSWORD
}

# Run main function
main "$@"
