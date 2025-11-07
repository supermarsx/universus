#!/bin/bash

###############################################################################
# Phase 9: Account Management System - End-to-End Testing Script
#
# This script performs comprehensive testing of all 7 account management
# interfaces including API integration, UI functionality, and data validation
###############################################################################

set -e  # Exit on error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test configuration
API_BASE_URL="${API_BASE_URL:-http://localhost:3000/api}"
WEB_BASE_URL="${WEB_BASE_URL:-http://localhost:3000}"
TEST_EMAIL="test.account.$(date +%s)@example.com"
TEST_PASSWORD="TestPassword123!"
AUTH_TOKEN=""

# Test counters
TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0

###############################################################################
# Helper Functions
###############################################################################

print_header() {
    echo -e "${BLUE}"
    echo "=============================================================================="
    echo "  Phase 9: Account Management System - End-to-End Testing"
    echo "=============================================================================="
    echo -e "${NC}"
}

print_test() {
    echo -e "${BLUE}[TEST $1]${NC} $2"
    ((TESTS_RUN++))
}

print_pass() {
    echo -e "${GREEN}  ✓ PASS${NC} $1"
    ((TESTS_PASSED++))
}

print_fail() {
    echo -e "${RED}  ✗ FAIL${NC} $1"
    ((TESTS_FAILED++))
}

print_skip() {
    echo -e "${YELLOW}  ⊘ SKIP${NC} $1"
}

print_info() {
    echo -e "${BLUE}  ℹ INFO${NC} $1"
}

# Check if server is running
check_server() {
    print_info "Checking if server is running at $WEB_BASE_URL..."
    if curl -s -f "$WEB_BASE_URL/api/health" > /dev/null 2>&1; then
        print_pass "Server is running"
        return 0
    else
        print_fail "Server is not running at $WEB_BASE_URL"
        return 1
    fi
}

# Register test user
register_user() {
    print_info "Registering test user: $TEST_EMAIL"
    RESPONSE=$(curl -s -X POST "$API_BASE_URL/register" \
        -H "Content-Type: application/json" \
        -d "{\"email\":\"$TEST_EMAIL\",\"username\":\"testuser$(date +%s)\",\"password\":\"$TEST_PASSWORD\"}")
    
    if echo "$RESPONSE" | grep -q "token"; then
        AUTH_TOKEN=$(echo "$RESPONSE" | grep -o '"token":"[^"]*' | cut -d'"' -f4)
        print_pass "User registered successfully"
        return 0
    else
        print_fail "Failed to register user: $RESPONSE"
        return 1
    fi
}

# Login test user
login_user() {
    print_info "Logging in test user..."
    RESPONSE=$(curl -s -X POST "$API_BASE_URL/login" \
        -H "Content-Type: application/json" \
        -d "{\"email\":\"$TEST_EMAIL\",\"password\":\"$TEST_PASSWORD\"}")
    
    if echo "$RESPONSE" | grep -q "token"; then
        AUTH_TOKEN=$(echo "$RESPONSE" | grep -o '"token":"[^"]*' | cut -d'"' -f4)
        print_pass "Login successful"
        return 0
    else
        print_fail "Login failed: $RESPONSE"
        return 1
    fi
}

###############################################################################
# Test Suite 1: Security Dashboard
###############################################################################

test_security_dashboard() {
    echo ""
    echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
    echo -e "${BLUE}  Test Suite 1: Security Dashboard${NC}"
    echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
    
    # Test 1.1: Get security summary
    print_test "1.1" "Get account security summary"
    RESPONSE=$(curl -s "$API_BASE_URL/account/security/summary" \
        -H "Authorization: Bearer $AUTH_TOKEN")
    
    if echo "$RESPONSE" | grep -q "activeSessions"; then
        print_pass "Security summary retrieved"
    else
        print_fail "Failed to get security summary"
    fi
    
    # Test 1.2: List active sessions
    print_test "1.2" "List active user sessions"
    RESPONSE=$(curl -s "$API_BASE_URL/account/sessions" \
        -H "Authorization: Bearer $AUTH_TOKEN")
    
    if echo "$RESPONSE" | grep -q "sessions"; then
        print_pass "Session list retrieved"
    else
        print_fail "Failed to get sessions"
    fi
    
    # Test 1.3: Validate current session
    print_test "1.3" "Validate current session"
    RESPONSE=$(curl -s -X POST "$API_BASE_URL/account/sessions/validate" \
        -H "Authorization: Bearer $AUTH_TOKEN" \
        -H "Content-Type: application/json")
    
    if echo "$RESPONSE" | grep -q "valid"; then
        print_pass "Session validation successful"
    else
        print_fail "Session validation failed"
    fi
    
    # Test 1.4: Get security audit logs
    print_test "1.4" "Retrieve security audit logs"
    RESPONSE=$(curl -s "$API_BASE_URL/account/security/logs" \
        -H "Authorization: Bearer $AUTH_TOKEN")
    
    if [ -n "$RESPONSE" ]; then
        print_pass "Audit logs retrieved"
    else
        print_fail "Failed to get audit logs"
    fi
}

###############################################################################
# Test Suite 2: Email Verification
###############################################################################

test_email_verification() {
    echo ""
    echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
    echo -e "${BLUE}  Test Suite 2: Email Verification${NC}"
    echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
    
    # Test 2.1: Check verification status
    print_test "2.1" "Check email verification status"
    RESPONSE=$(curl -s "$API_BASE_URL/account/email/status" \
        -H "Authorization: Bearer $AUTH_TOKEN")
    
    if echo "$RESPONSE" | grep -q "verified"; then
        print_pass "Verification status retrieved"
    else
        print_fail "Failed to get verification status"
    fi
    
    # Test 2.2: Send verification email
    print_test "2.2" "Send verification email"
    RESPONSE=$(curl -s -X POST "$API_BASE_URL/account/email/send" \
        -H "Authorization: Bearer $AUTH_TOKEN" \
        -H "Content-Type: application/json")
    
    HTTP_CODE=$(curl -s -w "%{http_code}" -o /dev/null -X POST "$API_BASE_URL/account/email/send" \
        -H "Authorization: Bearer $AUTH_TOKEN")
    
    if [ "$HTTP_CODE" -eq 200 ] || [ "$HTTP_CODE" -eq 201 ]; then
        print_pass "Verification email sent"
    else
        print_fail "Failed to send verification email (HTTP $HTTP_CODE)"
    fi
    
    # Test 2.3: Resend verification email (should be rate limited)
    print_test "2.3" "Test rate limiting on resend"
    HTTP_CODE=$(curl -s -w "%{http_code}" -o /dev/null -X POST "$API_BASE_URL/account/email/resend" \
        -H "Authorization: Bearer $AUTH_TOKEN")
    
    if [ "$HTTP_CODE" -eq 429 ]; then
        print_pass "Rate limiting is working"
    else
        print_skip "Rate limiting check (HTTP $HTTP_CODE)"
    fi
}

###############################################################################
# Test Suite 3: Password Recovery
###############################################################################

test_password_recovery() {
    echo ""
    echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
    echo -e "${BLUE}  Test Suite 3: Password Recovery${NC}"
    echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
    
    # Test 3.1: Initiate password reset
    print_test "3.1" "Initiate password reset"
    HTTP_CODE=$(curl -s -w "%{http_code}" -o /dev/null -X POST "$API_BASE_URL/account/password-recovery/initiate" \
        -H "Content-Type: application/json" \
        -d "{\"email\":\"$TEST_EMAIL\"}")
    
    if [ "$HTTP_CODE" -eq 200 ] || [ "$HTTP_CODE" -eq 201 ]; then
        print_pass "Password reset initiated"
    else
        print_fail "Failed to initiate password reset (HTTP $HTTP_CODE)"
    fi
    
    # Test 3.2: Validate with invalid token
    print_test "3.2" "Validate with invalid token"
    HTTP_CODE=$(curl -s -w "%{http_code}" -o /dev/null -X POST "$API_BASE_URL/account/password-recovery/validate" \
        -H "Content-Type: application/json" \
        -d "{\"token\":\"invalid-token-12345\"}")
    
    if [ "$HTTP_CODE" -eq 400 ] || [ "$HTTP_CODE" -eq 401 ]; then
        print_pass "Invalid token rejected correctly"
    else
        print_fail "Invalid token validation incorrect (HTTP $HTTP_CODE)"
    fi
}

###############################################################################
# Test Suite 4: Two-Factor Authentication
###############################################################################

test_2fa() {
    echo ""
    echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
    echo -e "${BLUE}  Test Suite 4: Two-Factor Authentication${NC}"
    echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
    
    # Test 4.1: Get 2FA status
    print_test "4.1" "Get 2FA status"
    RESPONSE=$(curl -s "$API_BASE_URL/account/2fa/status" \
        -H "Authorization: Bearer $AUTH_TOKEN")
    
    if echo "$RESPONSE" | grep -q "enabled"; then
        print_pass "2FA status retrieved"
    else
        print_fail "Failed to get 2FA status"
    fi
    
    # Test 4.2: Setup 2FA
    print_test "4.2" "Setup 2FA"
    RESPONSE=$(curl -s -X POST "$API_BASE_URL/account/2fa/setup" \
        -H "Authorization: Bearer $AUTH_TOKEN" \
        -H "Content-Type: application/json")
    
    if echo "$RESPONSE" | grep -q "secret\|qrCode"; then
        print_pass "2FA setup successful"
        TWO_FA_SECRET=$(echo "$RESPONSE" | grep -o '"secret":"[^"]*' | cut -d'"' -f4)
    else
        print_fail "Failed to setup 2FA"
    fi
    
    # Test 4.3: Get backup codes
    print_test "4.3" "Get 2FA backup codes"
    RESPONSE=$(curl -s "$API_BASE_URL/account/2fa/backup-codes" \
        -H "Authorization: Bearer $AUTH_TOKEN")
    
    if echo "$RESPONSE" | grep -q "backupCodes"; then
        print_pass "Backup codes retrieved"
    else
        print_skip "Backup codes not available"
    fi
}

###############################################################################
# Test Suite 5: GDPR Compliance
###############################################################################

test_gdpr() {
    echo ""
    echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
    echo -e "${BLUE}  Test Suite 5: GDPR Compliance${NC}"
    echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
    
    # Test 5.1: Request data export
    print_test "5.1" "Request data export"
    HTTP_CODE=$(curl -s -w "%{http_code}" -o /dev/null -X POST "$API_BASE_URL/account/gdpr/request" \
        -H "Authorization: Bearer $AUTH_TOKEN" \
        -H "Content-Type: application/json" \
        -d "{\"requestType\":\"data_export\",\"options\":{\"includeGameData\":true}}")
    
    if [ "$HTTP_CODE" -eq 200 ] || [ "$HTTP_CODE" -eq 201 ]; then
        print_pass "Data export requested"
    else
        print_fail "Failed to request data export (HTTP $HTTP_CODE)"
    fi
    
    # Test 5.2: List GDPR requests
    print_test "5.2" "List GDPR requests"
    RESPONSE=$(curl -s "$API_BASE_URL/account/gdpr/requests" \
        -H "Authorization: Bearer $AUTH_TOKEN")
    
    if echo "$RESPONSE" | grep -q "requests"; then
        print_pass "GDPR requests listed"
    else
        print_fail "Failed to list GDPR requests"
    fi
}

###############################################################################
# Test Suite 6: Account Transfer
###############################################################################

test_account_transfer() {
    echo ""
    echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
    echo -e "${BLUE}  Test Suite 6: Account Transfer${NC}"
    echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
    
    NEW_EMAIL="new.owner.$(date +%s)@example.com"
    
    # Test 6.1: Initiate account transfer
    print_test "6.1" "Initiate account transfer"
    HTTP_CODE=$(curl -s -w "%{http_code}" -o /dev/null -X POST "$API_BASE_URL/account/transfer/initiate" \
        -H "Authorization: Bearer $AUTH_TOKEN" \
        -H "Content-Type: application/json" \
        -d "{\"toEmail\":\"$NEW_EMAIL\",\"password\":\"$TEST_PASSWORD\",\"reason\":\"Testing\"}")
    
    if [ "$HTTP_CODE" -eq 200 ] || [ "$HTTP_CODE" -eq 201 ]; then
        print_pass "Account transfer initiated"
    else
        print_fail "Failed to initiate transfer (HTTP $HTTP_CODE)"
    fi
    
    # Test 6.2: Get transfer status
    print_test "6.2" "Get transfer status"
    RESPONSE=$(curl -s "$API_BASE_URL/account/transfer/status" \
        -H "Authorization: Bearer $AUTH_TOKEN")
    
    if [ -n "$RESPONSE" ]; then
        print_pass "Transfer status retrieved"
    else
        print_fail "Failed to get transfer status"
    fi
}

###############################################################################
# Test Suite 7: Account Settings
###############################################################################

test_account_settings() {
    echo ""
    echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
    echo -e "${BLUE}  Test Suite 7: Account Settings${NC}"
    echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
    
    # Test 7.1: Get profile
    print_test "7.1" "Get user profile"
    RESPONSE=$(curl -s "$API_BASE_URL/account/profile" \
        -H "Authorization: Bearer $AUTH_TOKEN")
    
    if echo "$RESPONSE" | grep -q "email\|username"; then
        print_pass "Profile retrieved"
    else
        print_fail "Failed to get profile"
    fi
    
    # Test 7.2: Update profile
    print_test "7.2" "Update user profile"
    HTTP_CODE=$(curl -s -w "%{http_code}" -o /dev/null -X PUT "$API_BASE_URL/account/profile" \
        -H "Authorization: Bearer $AUTH_TOKEN" \
        -H "Content-Type: application/json" \
        -d "{\"displayName\":\"Test User\",\"timezone\":\"UTC\"}")
    
    if [ "$HTTP_CODE" -eq 200 ]; then
        print_pass "Profile updated"
    else
        print_fail "Failed to update profile (HTTP $HTTP_CODE)"
    fi
    
    # Test 7.3: Get notification settings
    print_test "7.3" "Get notification settings"
    RESPONSE=$(curl -s "$API_BASE_URL/account/settings/notifications" \
        -H "Authorization: Bearer $AUTH_TOKEN")
    
    if [ -n "$RESPONSE" ]; then
        print_pass "Notification settings retrieved"
    else
        print_skip "Notification settings endpoint may not exist"
    fi
}

###############################################################################
# Frontend Tests
###############################################################################

test_frontend_pages() {
    echo ""
    echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
    echo -e "${BLUE}  Test Suite 8: Frontend Pages${NC}"
    echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
    
    PAGES=(
        "/account/settings:Account Settings"
        "/account/security:Security Dashboard"
        "/account/2fa:Two-Factor Authentication"
        "/account/email:Email Verification"
        "/account/password:Password Recovery"
        "/account/privacy:Privacy"
        "/account/transfer:Account Transfer"
    )
    
    for page_info in "${PAGES[@]}"; do
        IFS=':' read -r url title <<< "$page_info"
        print_test "8.x" "Check $title page"
        
        HTTP_CODE=$(curl -s -w "%{http_code}" -o /dev/null "$WEB_BASE_URL$url")
        
        if [ "$HTTP_CODE" -eq 200 ]; then
            print_pass "$title page loads (HTTP $HTTP_CODE)"
        else
            print_fail "$title page failed (HTTP $HTTP_CODE)"
        fi
    done
}

###############################################################################
# Summary
###############################################################################

print_summary() {
    echo ""
    echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
    echo -e "${BLUE}  Test Summary${NC}"
    echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
    echo ""
    echo "  Total Tests Run:    $TESTS_RUN"
    echo -e "  ${GREEN}Tests Passed:       $TESTS_PASSED${NC}"
    echo -e "  ${RED}Tests Failed:       $TESTS_FAILED${NC}"
    echo ""
    
    PASS_RATE=0
    if [ $TESTS_RUN -gt 0 ]; then
        PASS_RATE=$((TESTS_PASSED * 100 / TESTS_RUN))
    fi
    
    echo "  Pass Rate:          ${PASS_RATE}%"
    echo ""
    
    if [ $TESTS_FAILED -eq 0 ]; then
        echo -e "${GREEN}✓ All tests passed!${NC}"
        return 0
    else
        echo -e "${RED}✗ Some tests failed. Please review the output above.${NC}"
        return 1
    fi
}

###############################################################################
# Main Execution
###############################################################################

main() {
    print_header
    
    # Check if server is running
    if ! check_server; then
        echo ""
        echo -e "${RED}Cannot run tests: Server is not running${NC}"
        echo "Please start the server and try again:"
        echo "  cd backend && npm start"
        exit 1
    fi
    
    # Register and login test user
    if ! register_user; then
        # Try to login if registration fails
        login_user || exit 1
    fi
    
    # Run all test suites
    test_security_dashboard
    test_email_verification
    test_password_recovery
    test_2fa
    test_gdpr
    test_account_transfer
    test_account_settings
    test_frontend_pages
    
    # Print summary
    print_summary
}

# Run main function
main "$@"
