#!/bin/bash

# Link Validation Script for Vibe Ensemble
# This script validates navigation links and can be used in CI/CD pipelines or as a pre-commit hook

set -e

# Configuration
BASE_URL=${BASE_URL:-"http://127.0.0.1:8081"}
HEALTH_THRESHOLD=${HEALTH_THRESHOLD:-80}
TIMEOUT=${TIMEOUT:-30}
RETRY_ATTEMPTS=${RETRY_ATTEMPTS:-3}
RETRY_DELAY=${RETRY_DELAY:-5}

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if server is running
check_server() {
    log_info "Checking if server is running at $BASE_URL..."
    
    for attempt in $(seq 1 $RETRY_ATTEMPTS); do
        if curl -f -s --max-time $TIMEOUT "$BASE_URL/api/health" > /dev/null 2>&1; then
            log_success "Server is running and healthy"
            return 0
        fi
        
        if [ $attempt -lt $RETRY_ATTEMPTS ]; then
            log_warning "Server not ready, retrying in ${RETRY_DELAY}s... (attempt $attempt/$RETRY_ATTEMPTS)"
            sleep $RETRY_DELAY
        fi
    done
    
    log_error "Server is not running or not healthy at $BASE_URL"
    log_error "Please start the server with: cargo run --bin vibe-ensemble"
    return 1
}

# Test basic navigation endpoints
test_navigation_endpoints() {
    log_info "Testing navigation endpoints..."
    
    local failed_endpoints=()
    local endpoints=(
        "/"
        "/dashboard"
        "/link-health"
    )
    
    for endpoint in "${endpoints[@]}"; do
        local url="$BASE_URL$endpoint"
        if curl -f -s --max-time $TIMEOUT "$url" > /dev/null 2>&1; then
            log_success "✓ $endpoint"
        else
            log_error "✗ $endpoint"
            failed_endpoints+=("$endpoint")
        fi
    done
    
    if [ ${#failed_endpoints[@]} -gt 0 ]; then
        log_error "Failed navigation endpoints: ${failed_endpoints[*]}"
        return 1
    fi
    
    return 0
}

# Test API endpoints
test_api_endpoints() {
    log_info "Testing API endpoints..."
    
    local failed_endpoints=()
    local endpoints=(
        "/api/health"
        "/api/stats"
        "/api/agents"
        "/api/issues"
        "/api/links/health"
        "/api/links/status"
        "/api/links/analytics"
    )
    
    for endpoint in "${endpoints[@]}"; do
        local url="$BASE_URL$endpoint"
        if curl -f -s --max-time $TIMEOUT "$url" > /dev/null 2>&1; then
            log_success "✓ $endpoint"
        else
            log_error "✗ $endpoint"
            failed_endpoints+=("$endpoint")
        fi
    done
    
    if [ ${#failed_endpoints[@]} -gt 0 ]; then
        log_error "Failed API endpoints: ${failed_endpoints[*]}"
        return 1
    fi
    
    return 0
}

# Run automated link validation
run_link_validation() {
    log_info "Running automated link validation..."
    
    # Trigger validation
    local validation_response
    validation_response=$(curl -s --max-time $TIMEOUT "$BASE_URL/api/links/validate" 2>/dev/null)
    
    if [ $? -eq 0 ]; then
        log_success "Link validation triggered successfully"
    else
        log_warning "Could not trigger link validation (API might not be available)"
    fi
    
    # Get health summary
    local health_response
    health_response=$(curl -s --max-time $TIMEOUT "$BASE_URL/api/links/health" 2>/dev/null)
    
    if [ $? -eq 0 ] && [ -n "$health_response" ]; then
        # Parse health score (requires jq for JSON parsing)
        if command -v jq >/dev/null 2>&1; then
            local health_score
            health_score=$(echo "$health_response" | jq -r '.health_score // 0' 2>/dev/null)
            
            if [ -n "$health_score" ] && [ "$health_score" != "null" ]; then
                local score_int
                score_int=$(printf "%.0f" "$health_score")
                
                log_info "Current health score: ${health_score}%"
                
                if [ "$score_int" -ge "$HEALTH_THRESHOLD" ]; then
                    log_success "Health score (${health_score}%) meets threshold (${HEALTH_THRESHOLD}%)"
                    return 0
                else
                    log_error "Health score (${health_score}%) below threshold (${HEALTH_THRESHOLD}%)"
                    return 1
                fi
            else
                log_warning "Could not parse health score from response"
            fi
        else
            log_warning "jq not available - skipping health score validation"
            log_info "Health response: $health_response"
        fi
    else
        log_warning "Could not get health summary from link validation API"
        return 1
    fi
    
    return 0
}

# Generate validation report
generate_report() {
    log_info "Generating validation report..."
    
    local report_dir="link-validation-reports"
    mkdir -p "$report_dir"
    
    # Get link status
    curl -s --max-time $TIMEOUT "$BASE_URL/api/links/status" > "$report_dir/status.json" 2>/dev/null || true
    
    # Get health summary
    curl -s --max-time $TIMEOUT "$BASE_URL/api/links/health" > "$report_dir/health.json" 2>/dev/null || true
    
    # Get analytics
    curl -s --max-time $TIMEOUT "$BASE_URL/api/links/analytics" > "$report_dir/analytics.json" 2>/dev/null || true
    
    log_success "Reports generated in $report_dir/"
    
    # Display summary if jq is available
    if command -v jq >/dev/null 2>&1 && [ -f "$report_dir/health.json" ]; then
        echo
        log_info "=== VALIDATION SUMMARY ==="
        echo "Health Score: $(jq -r '.health_score // "N/A"' "$report_dir/health.json")%"
        echo "Total Links: $(jq -r '.total_links // "N/A"' "$report_dir/health.json")"
        echo "Healthy Links: $(jq -r '.healthy_links // "N/A"' "$report_dir/health.json")"
        echo "Broken Links: $(jq -r '.broken_links // "N/A"' "$report_dir/health.json")"
        echo "Warning Links: $(jq -r '.warning_links // "N/A"' "$report_dir/health.json")"
    fi
}

# Main function
main() {
    echo "===================================="
    echo "    Link Validation for Vibe Ensemble"
    echo "===================================="
    echo
    
    log_info "Configuration:"
    echo "  Base URL: $BASE_URL"
    echo "  Health Threshold: ${HEALTH_THRESHOLD}%"
    echo "  Timeout: ${TIMEOUT}s"
    echo
    
    # Check if server is running
    if ! check_server; then
        exit 1
    fi
    
    echo
    
    # Test navigation endpoints
    local nav_result=0
    test_navigation_endpoints || nav_result=$?
    
    echo
    
    # Test API endpoints
    local api_result=0
    test_api_endpoints || api_result=$?
    
    echo
    
    # Run link validation
    local validation_result=0
    run_link_validation || validation_result=$?
    
    echo
    
    # Generate report
    generate_report
    
    echo
    
    # Final results
    if [ $nav_result -eq 0 ] && [ $api_result -eq 0 ] && [ $validation_result -eq 0 ]; then
        log_success "✅ All link validation checks passed!"
        exit 0
    else
        log_error "❌ Link validation checks failed!"
        echo
        echo "Failed checks:"
        [ $nav_result -ne 0 ] && echo "  - Navigation endpoints"
        [ $api_result -ne 0 ] && echo "  - API endpoints"
        [ $validation_result -ne 0 ] && echo "  - Link health validation"
        exit 1
    fi
}

# Handle script arguments
case "${1:-}" in
    --help|-h)
        echo "Usage: $0 [options]"
        echo
        echo "Options:"
        echo "  --help, -h          Show this help message"
        echo "  --server-only       Only check if server is running"
        echo "  --nav-only          Only test navigation endpoints"
        echo "  --api-only          Only test API endpoints"
        echo "  --validate-only     Only run link validation"
        echo "  --report-only       Only generate validation report"
        echo
        echo "Environment Variables:"
        echo "  BASE_URL            Server base URL (default: http://127.0.0.1:8081)"
        echo "  HEALTH_THRESHOLD    Minimum health score percentage (default: 80)"
        echo "  TIMEOUT             Request timeout in seconds (default: 30)"
        echo "  RETRY_ATTEMPTS      Number of retry attempts (default: 3)"
        echo "  RETRY_DELAY         Delay between retries in seconds (default: 5)"
        exit 0
        ;;
    --server-only)
        check_server
        exit $?
        ;;
    --nav-only)
        check_server && test_navigation_endpoints
        exit $?
        ;;
    --api-only)
        check_server && test_api_endpoints
        exit $?
        ;;
    --validate-only)
        check_server && run_link_validation
        exit $?
        ;;
    --report-only)
        check_server && generate_report
        exit $?
        ;;
    "")
        main
        ;;
    *)
        log_error "Unknown option: $1"
        echo "Use --help for usage information"
        exit 1
        ;;
esac