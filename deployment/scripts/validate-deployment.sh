#!/bin/bash
# Comprehensive deployment validation script for Vibe Ensemble MCP Server
# Validates production deployment health, security, and functionality

set -euo pipefail

# Configuration
NAMESPACE="${NAMESPACE:-vibe-ensemble}"
TIMEOUT="${TIMEOUT:-300}"
DOMAIN="${DOMAIN:-vibe-ensemble.example.com}"
STAGING_DOMAIN="${STAGING_DOMAIN:-staging.vibe-ensemble.example.com}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test results tracking
TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0
FAILED_TESTS=()

# Logging functions
log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_debug() {
    echo -e "${BLUE}[DEBUG]${NC} $1"
}

log_test_start() {
    echo -e "${BLUE}[TEST]${NC} $1"
    TESTS_RUN=$((TESTS_RUN + 1))
}

log_test_pass() {
    echo -e "${GREEN}[PASS]${NC} $1"
    TESTS_PASSED=$((TESTS_PASSED + 1))
}

log_test_fail() {
    echo -e "${RED}[FAIL]${NC} $1"
    TESTS_FAILED=$((TESTS_FAILED + 1))
    FAILED_TESTS+=("$1")
}

# Help function
show_help() {
    cat << EOF
Deployment Validation Script for Vibe Ensemble MCP Server

Usage: $0 [OPTIONS]

Options:
    -n, --namespace NAMESPACE    Kubernetes namespace (default: vibe-ensemble)
    -t, --timeout TIMEOUT        Timeout in seconds (default: 300)
    -d, --domain DOMAIN          Production domain (default: vibe-ensemble.example.com)
    -s, --staging DOMAIN         Staging domain (default: staging.vibe-ensemble.example.com)
    --skip-network              Skip network connectivity tests
    --skip-security             Skip security validation tests
    --skip-performance          Skip performance tests
    -v, --verbose               Verbose output
    -h, --help                  Show this help message

Examples:
    $0                                          # Validate default deployment
    $0 --namespace production                   # Validate specific namespace
    $0 --skip-performance --verbose            # Skip perf tests with verbose output
EOF
}

# Parse command line arguments
SKIP_NETWORK=false
SKIP_SECURITY=false
SKIP_PERFORMANCE=false
VERBOSE=false

while [[ $# -gt 0 ]]; do
    case $1 in
        -n|--namespace)
            NAMESPACE="$2"
            shift 2
            ;;
        -t|--timeout)
            TIMEOUT="$2"
            shift 2
            ;;
        -d|--domain)
            DOMAIN="$2"
            shift 2
            ;;
        -s|--staging)
            STAGING_DOMAIN="$2"
            shift 2
            ;;
        --skip-network)
            SKIP_NETWORK=true
            shift
            ;;
        --skip-security)
            SKIP_SECURITY=true
            shift
            ;;
        --skip-performance)
            SKIP_PERFORMANCE=true
            shift
            ;;
        -v|--verbose)
            VERBOSE=true
            shift
            ;;
        -h|--help)
            show_help
            exit 0
            ;;
        *)
            log_error "Unknown option: $1"
            show_help
            exit 1
            ;;
    esac
done

# Utility functions
run_test() {
    local test_name="$1"
    local test_command="$2"
    local expected_result="${3:-0}"
    
    log_test_start "$test_name"
    
    if [[ "$VERBOSE" == "true" ]]; then
        log_debug "Running: $test_command"
    fi
    
    if eval "$test_command" >/dev/null 2>&1; then
        local result=$?
        if [[ $result -eq $expected_result ]]; then
            log_test_pass "$test_name"
            return 0
        else
            log_test_fail "$test_name (exit code: $result, expected: $expected_result)"
            return 1
        fi
    else
        log_test_fail "$test_name (command failed)"
        return 1
    fi
}

wait_for_condition() {
    local description="$1"
    local condition="$2"
    local timeout="$3"
    
    log_info "Waiting for: $description"
    
    local elapsed=0
    while ! eval "$condition" >/dev/null 2>&1; do
        if [[ $elapsed -ge $timeout ]]; then
            log_error "Timeout waiting for: $description"
            return 1
        fi
        sleep 5
        elapsed=$((elapsed + 5))
        if [[ "$VERBOSE" == "true" ]]; then
            echo -n "."
        fi
    done
    
    if [[ "$VERBOSE" == "true" ]]; then
        echo
    fi
    
    log_info "Condition met: $description"
    return 0
}

# Test functions
test_prerequisites() {
    log_info "Testing prerequisites..."
    
    run_test "kubectl available" "command -v kubectl"
    run_test "curl available" "command -v curl"
    run_test "kubectl cluster connection" "kubectl cluster-info"
    run_test "namespace exists" "kubectl get namespace $NAMESPACE"
}

test_pod_health() {
    log_info "Testing pod health..."
    
    # Wait for all pods to be ready
    wait_for_condition "all pods ready" \
        "kubectl get pods -n $NAMESPACE --no-headers | grep -v Completed | awk '{print \$2}' | grep -v '1/1' | wc -l | grep -q '^0$'" \
        "$TIMEOUT"
    
    # Test individual pod health
    run_test "vibe-ensemble pods running" \
        "kubectl get pods -n $NAMESPACE -l app.kubernetes.io/name=vibe-ensemble | grep -q Running"
    
    run_test "postgres pods running" \
        "kubectl get pods -n $NAMESPACE -l app.kubernetes.io/name=postgres | grep -q Running"
    
    run_test "prometheus pods running" \
        "kubectl get pods -n $NAMESPACE -l app.kubernetes.io/name=prometheus | grep -q Running"
    
    # Check pod restart counts
    local max_restarts=5
    local restart_count=$(kubectl get pods -n "$NAMESPACE" --no-headers | awk '{print $4}' | sort -nr | head -1)
    if [[ ${restart_count:-0} -gt $max_restarts ]]; then
        log_test_fail "High pod restart count: $restart_count (max: $max_restarts)"
    else
        log_test_pass "Pod restart counts acceptable"
    fi
}

test_service_connectivity() {
    log_info "Testing service connectivity..."
    
    run_test "vibe-ensemble service exists" \
        "kubectl get service vibe-ensemble-service -n $NAMESPACE"
    
    run_test "postgres service exists" \
        "kubectl get service postgres-service -n $NAMESPACE"
    
    run_test "prometheus service exists" \
        "kubectl get service prometheus-service -n $NAMESPACE"
    
    # Test internal service connectivity
    local test_pod=$(kubectl get pods -n "$NAMESPACE" -l app.kubernetes.io/name=vibe-ensemble -o jsonpath='{.items[0].metadata.name}')
    
    if [[ -n "$test_pod" ]]; then
        run_test "internal database connectivity" \
            "kubectl exec $test_pod -n $NAMESPACE -- curl -f http://postgres-service:5432 || true"
        
        run_test "internal prometheus connectivity" \
            "kubectl exec $test_pod -n $NAMESPACE -- curl -f http://prometheus-service:9090/metrics"
    fi
}

test_health_endpoints() {
    log_info "Testing health endpoints..."
    
    # Wait for ingress to be ready
    wait_for_condition "ingress ready" \
        "kubectl get ingress -n $NAMESPACE | grep -q ADDRESS" \
        "$TIMEOUT"
    
    # Test health endpoints
    if [[ "$SKIP_NETWORK" != "true" ]]; then
        run_test "main health endpoint" \
            "curl -f https://$DOMAIN/api/health"
        
        run_test "metrics endpoint" \
            "curl -f https://$DOMAIN/metrics"
        
        run_test "staging health endpoint" \
            "curl -f https://$STAGING_DOMAIN/api/health"
    fi
    
    # Test internal health endpoints
    local app_pod=$(kubectl get pods -n "$NAMESPACE" -l app.kubernetes.io/name=vibe-ensemble -o jsonpath='{.items[0].metadata.name}')
    
    if [[ -n "$app_pod" ]]; then
        run_test "internal health endpoint" \
            "kubectl exec $app_pod -n $NAMESPACE -- curl -f http://localhost:8080/api/health"
        
        run_test "internal metrics endpoint" \
            "kubectl exec $app_pod -n $NAMESPACE -- curl -f http://localhost:9090/metrics"
    fi
}

test_database_functionality() {
    log_info "Testing database functionality..."
    
    local postgres_pod=$(kubectl get pods -n "$NAMESPACE" -l app.kubernetes.io/name=postgres -o jsonpath='{.items[0].metadata.name}')
    
    if [[ -n "$postgres_pod" ]]; then
        run_test "database connection" \
            "kubectl exec $postgres_pod -n $NAMESPACE -- psql -U vibe_ensemble -d vibe_ensemble -c 'SELECT 1;'"
        
        run_test "database tables exist" \
            "kubectl exec $postgres_pod -n $NAMESPACE -- psql -U vibe_ensemble -d vibe_ensemble -c 'SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = \"public\";' | grep -q '[1-9]'"
        
        run_test "database write test" \
            "kubectl exec $postgres_pod -n $NAMESPACE -- psql -U vibe_ensemble -d vibe_ensemble -c 'INSERT INTO agents (id, name, status, capabilities, created_at, updated_at) VALUES (\"test-validation-$(date +%s)\", \"validation-agent\", \"active\", \"{}\", NOW(), NOW()) ON CONFLICT (id) DO NOTHING;'"
        
        run_test "database read test" \
            "kubectl exec $postgres_pod -n $NAMESPACE -- psql -U vibe_ensemble -d vibe_ensemble -c 'SELECT COUNT(*) FROM agents;' | grep -q '[0-9]'"
    fi
}

test_security_configuration() {
    if [[ "$SKIP_SECURITY" == "true" ]]; then
        log_info "Skipping security tests"
        return 0
    fi
    
    log_info "Testing security configuration..."
    
    # Test RBAC
    run_test "service account exists" \
        "kubectl get serviceaccount vibe-ensemble-sa -n $NAMESPACE"
    
    run_test "role binding exists" \
        "kubectl get rolebinding vibe-ensemble-rolebinding -n $NAMESPACE"
    
    # Test network policies
    run_test "network policies configured" \
        "kubectl get networkpolicy -n $NAMESPACE | grep -q vibe-ensemble"
    
    # Test pod security
    local pod_security_context=$(kubectl get pods -n "$NAMESPACE" -l app.kubernetes.io/name=vibe-ensemble -o jsonpath='{.items[0].spec.securityContext.runAsNonRoot}')
    if [[ "$pod_security_context" == "true" ]]; then
        log_test_pass "pods run as non-root"
    else
        log_test_fail "pods not configured to run as non-root"
    fi
    
    # Test TLS configuration
    if [[ "$SKIP_NETWORK" != "true" ]]; then
        run_test "TLS certificate valid" \
            "curl -I https://$DOMAIN | grep -q 'HTTP/2 200'"
        
        run_test "TLS security headers" \
            "curl -I https://$DOMAIN | grep -q 'Strict-Transport-Security'"
    fi
    
    # Test secrets
    run_test "secrets properly configured" \
        "kubectl get secret vibe-ensemble-secrets -n $NAMESPACE -o jsonpath='{.data}' | grep -q jwt-secret"
}

test_monitoring_configuration() {
    log_info "Testing monitoring configuration..."
    
    # Test Prometheus
    run_test "prometheus deployment ready" \
        "kubectl get deployment prometheus -n $NAMESPACE -o jsonpath='{.status.readyReplicas}' | grep -q '[1-9]'"
    
    # Test metrics collection
    local prometheus_pod=$(kubectl get pods -n "$NAMESPACE" -l app.kubernetes.io/name=prometheus -o jsonpath='{.items[0].metadata.name}')
    
    if [[ -n "$prometheus_pod" ]]; then
        run_test "prometheus metrics endpoint" \
            "kubectl exec $prometheus_pod -n $NAMESPACE -- curl -f http://localhost:9090/metrics"
        
        run_test "application metrics in prometheus" \
            "kubectl exec $prometheus_pod -n $NAMESPACE -- curl -s http://localhost:9090/api/v1/query?query=up | grep -q vibe-ensemble"
    fi
    
    # Test Grafana
    if kubectl get deployment grafana -n "$NAMESPACE" >/dev/null 2>&1; then
        run_test "grafana deployment ready" \
            "kubectl get deployment grafana -n $NAMESPACE -o jsonpath='{.status.readyReplicas}' | grep -q '[1-9]'"
    fi
}

test_backup_configuration() {
    log_info "Testing backup configuration..."
    
    # Test backup CronJob
    run_test "backup cronjob configured" \
        "kubectl get cronjob postgres-backup -n $NAMESPACE"
    
    run_test "backup service account configured" \
        "kubectl get serviceaccount vibe-ensemble-backup-sa -n $NAMESPACE"
    
    # Test backup script accessibility
    run_test "backup script executable" \
        "test -x deployment/scripts/database-restore.sh"
    
    # Test recent backup existence (if backups have been running)
    if kubectl get jobs -n "$NAMESPACE" | grep -q backup; then
        local latest_backup_job=$(kubectl get jobs -n "$NAMESPACE" --sort-by=.metadata.creationTimestamp | grep backup | tail -1 | awk '{print $1}')
        if [[ -n "$latest_backup_job" ]]; then
            run_test "recent backup job successful" \
                "kubectl get job $latest_backup_job -n $NAMESPACE -o jsonpath='{.status.succeeded}' | grep -q '1'"
        fi
    fi
}

test_autoscaling() {
    log_info "Testing autoscaling configuration..."
    
    # Test HPA
    run_test "horizontal pod autoscaler configured" \
        "kubectl get hpa vibe-ensemble-hpa -n $NAMESPACE"
    
    # Test resource metrics
    run_test "resource metrics available" \
        "kubectl top pods -n $NAMESPACE | grep -q vibe-ensemble"
    
    # Test pod disruption budget
    run_test "pod disruption budget configured" \
        "kubectl get pdb vibe-ensemble-pdb -n $NAMESPACE"
}

test_performance() {
    if [[ "$SKIP_PERFORMANCE" == "true" ]]; then
        log_info "Skipping performance tests"
        return 0
    fi
    
    log_info "Testing performance..."
    
    if [[ "$SKIP_NETWORK" != "true" ]]; then
        # Test response time
        local response_time=$(curl -o /dev/null -s -w '%{time_total}' https://$DOMAIN/api/health)
        local max_response_time=2.0
        
        if (( $(echo "$response_time < $max_response_time" | bc -l) )); then
            log_test_pass "response time acceptable: ${response_time}s"
        else
            log_test_fail "response time too high: ${response_time}s (max: ${max_response_time}s)"
        fi
        
        # Test concurrent requests
        log_test_start "concurrent request handling"
        local concurrent_success=true
        for i in {1..10}; do
            if ! curl -f https://$DOMAIN/api/health >/dev/null 2>&1 & then
                concurrent_success=false
                break
            fi
        done
        wait
        
        if [[ "$concurrent_success" == "true" ]]; then
            log_test_pass "concurrent request handling"
        else
            log_test_fail "concurrent request handling"
        fi
    fi
    
    # Test resource usage
    local cpu_usage=$(kubectl top pods -n "$NAMESPACE" --no-headers | grep vibe-ensemble | awk '{print $2}' | sed 's/m//' | head -1)
    local memory_usage=$(kubectl top pods -n "$NAMESPACE" --no-headers | grep vibe-ensemble | awk '{print $3}' | sed 's/Mi//' | head -1)
    
    if [[ ${cpu_usage:-0} -lt 1000 ]]; then
        log_test_pass "CPU usage reasonable: ${cpu_usage}m"
    else
        log_test_fail "CPU usage high: ${cpu_usage}m"
    fi
    
    if [[ ${memory_usage:-0} -lt 2048 ]]; then
        log_test_pass "Memory usage reasonable: ${memory_usage}Mi"
    else
        log_test_fail "Memory usage high: ${memory_usage}Mi"
    fi
}

test_disaster_recovery() {
    log_info "Testing disaster recovery capabilities..."
    
    # Test backup restore script
    run_test "backup restore script exists" \
        "test -f deployment/scripts/database-restore.sh"
    
    run_test "backup restore script executable" \
        "test -x deployment/scripts/database-restore.sh"
    
    # Test rollback capability
    run_test "deployment rollback capability" \
        "kubectl rollout history deployment/vibe-ensemble-server -n $NAMESPACE | grep -q REVISION"
    
    # Test configuration recovery
    run_test "configuration backup exists" \
        "kubectl get configmap vibe-ensemble-config-production -n $NAMESPACE"
}

# Generate test report
generate_report() {
    echo
    echo "========================================"
    echo "     DEPLOYMENT VALIDATION REPORT"
    echo "========================================"
    echo "Namespace: $NAMESPACE"
    echo "Domain: $DOMAIN"
    echo "Timestamp: $(date)"
    echo
    echo "Test Results:"
    echo "  Total Tests: $TESTS_RUN"
    echo "  Passed: $TESTS_PASSED"
    echo "  Failed: $TESTS_FAILED"
    echo "  Success Rate: $(( (TESTS_PASSED * 100) / TESTS_RUN ))%"
    echo
    
    if [[ $TESTS_FAILED -gt 0 ]]; then
        echo "Failed Tests:"
        for test in "${FAILED_TESTS[@]}"; do
            echo "  - $test"
        done
        echo
    fi
    
    if [[ $TESTS_FAILED -eq 0 ]]; then
        echo -e "${GREEN}✅ All tests passed! Deployment is healthy.${NC}"
        return 0
    else
        echo -e "${RED}❌ Some tests failed. Please review and fix issues.${NC}"
        return 1
    fi
}

# Main execution
main() {
    log_info "Starting deployment validation for Vibe Ensemble MCP Server"
    log_info "Namespace: $NAMESPACE"
    log_info "Timeout: $TIMEOUT seconds"
    
    # Run test suites
    test_prerequisites
    test_pod_health
    test_service_connectivity
    test_health_endpoints
    test_database_functionality
    test_security_configuration
    test_monitoring_configuration
    test_backup_configuration
    test_autoscaling
    test_performance
    test_disaster_recovery
    
    # Generate and display report
    generate_report
}

# Run main function
main "$@"