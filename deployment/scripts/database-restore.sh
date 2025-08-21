#!/bin/bash
# Database restore script for Vibe Ensemble MCP Server
# Provides safe database restoration with verification and rollback capabilities

set -euo pipefail

# Configuration
NAMESPACE="${NAMESPACE:-vibe-ensemble}"
POSTGRES_SERVICE="${POSTGRES_SERVICE:-postgres-service}"
BACKUP_BUCKET="${BACKUP_BUCKET:-vibe-ensemble-backups}"
KUBECONFIG="${KUBECONFIG:-}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

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

# Help function
show_help() {
    cat << EOF
Database Restore Script for Vibe Ensemble MCP Server

Usage: $0 [OPTIONS] BACKUP_FILE

Options:
    -n, --namespace NAMESPACE    Kubernetes namespace (default: vibe-ensemble)
    -s, --service SERVICE        PostgreSQL service name (default: postgres-service)
    -b, --bucket BUCKET          S3 backup bucket (default: vibe-ensemble-backups)
    -f, --force                  Skip confirmation prompts
    -v, --verify                 Verify backup integrity before restore
    -d, --dry-run               Show what would be done without executing
    -h, --help                  Show this help message

Examples:
    $0 vibe_ensemble_backup_20240101_120000.sql.gz
    $0 --verify --force backup_file.sql
    $0 --namespace production --dry-run latest_backup.sql.gz

Environment Variables:
    KUBECONFIG                  Path to kubectl config file
    BACKUP_BUCKET              S3 bucket for backups
    POSTGRES_PASSWORD          PostgreSQL password (if not using secrets)
EOF
}

# Parse command line arguments
FORCE=false
VERIFY=false
DRY_RUN=false
BACKUP_FILE=""

while [[ $# -gt 0 ]]; do
    case $1 in
        -n|--namespace)
            NAMESPACE="$2"
            shift 2
            ;;
        -s|--service)
            POSTGRES_SERVICE="$2"
            shift 2
            ;;
        -b|--bucket)
            BACKUP_BUCKET="$2"
            shift 2
            ;;
        -f|--force)
            FORCE=true
            shift
            ;;
        -v|--verify)
            VERIFY=true
            shift
            ;;
        -d|--dry-run)
            DRY_RUN=true
            shift
            ;;
        -h|--help)
            show_help
            exit 0
            ;;
        -*)
            log_error "Unknown option $1"
            show_help
            exit 1
            ;;
        *)
            if [[ -z "$BACKUP_FILE" ]]; then
                BACKUP_FILE="$1"
            else
                log_error "Multiple backup files specified"
                exit 1
            fi
            shift
            ;;
    esac
done

# Validate inputs
if [[ -z "$BACKUP_FILE" ]]; then
    log_error "Backup file not specified"
    show_help
    exit 1
fi

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."
    
    if ! command -v kubectl &> /dev/null; then
        log_error "kubectl is not installed"
        exit 1
    fi
    
    if ! kubectl get namespace "$NAMESPACE" &> /dev/null; then
        log_error "Namespace '$NAMESPACE' does not exist"
        exit 1
    fi
    
    if ! kubectl get service "$POSTGRES_SERVICE" -n "$NAMESPACE" &> /dev/null; then
        log_error "PostgreSQL service '$POSTGRES_SERVICE' not found in namespace '$NAMESPACE'"
        exit 1
    fi
    
    log_info "Prerequisites check passed"
}

# Download backup from cloud storage
download_backup() {
    local backup_file="$1"
    local local_path="/tmp/$(basename "$backup_file")"
    
    log_info "Downloading backup: $backup_file"
    
    if [[ "$backup_file" =~ ^s3:// ]]; then
        if ! command -v aws &> /dev/null; then
            log_error "AWS CLI is not installed"
            exit 1
        fi
        
        aws s3 cp "$backup_file" "$local_path"
    elif [[ "$backup_file" =~ ^gs:// ]]; then
        if ! command -v gsutil &> /dev/null; then
            log_error "Google Cloud SDK is not installed"
            exit 1
        fi
        
        gsutil cp "$backup_file" "$local_path"
    elif [[ -f "$backup_file" ]]; then
        cp "$backup_file" "$local_path"
    else
        # Try to download from default S3 bucket
        if command -v aws &> /dev/null; then
            aws s3 cp "s3://${BACKUP_BUCKET}/postgresql/$backup_file" "$local_path"
        else
            log_error "Backup file not found and no cloud CLI available"
            exit 1
        fi
    fi
    
    if [[ ! -f "$local_path" ]]; then
        log_error "Failed to download backup file"
        exit 1
    fi
    
    echo "$local_path"
}

# Verify backup integrity
verify_backup() {
    local backup_path="$1"
    
    log_info "Verifying backup integrity..."
    
    # Check if file is compressed
    if [[ "$backup_path" =~ \.gz$ ]]; then
        if ! gzip -t "$backup_path"; then
            log_error "Backup file is corrupted (gzip test failed)"
            exit 1
        fi
        
        # Extract for further verification
        local extracted_path="${backup_path%.gz}"
        gunzip -c "$backup_path" > "$extracted_path"
        backup_path="$extracted_path"
    fi
    
    # Verify PostgreSQL dump format
    if file "$backup_path" | grep -q "PostgreSQL custom database dump"; then
        log_info "Backup is a PostgreSQL custom format dump"
        
        # Use pg_restore to list contents without restoring
        if ! pg_restore --list "$backup_path" > /dev/null; then
            log_error "Backup file is corrupted (pg_restore list failed)"
            exit 1
        fi
    elif file "$backup_path" | grep -q "ASCII text"; then
        log_info "Backup is a SQL text dump"
        
        # Basic SQL syntax check
        if ! grep -q "PostgreSQL database dump" "$backup_path"; then
            log_warn "Backup may not be a PostgreSQL dump"
        fi
    else
        log_error "Unknown backup format"
        exit 1
    fi
    
    log_info "Backup integrity verification passed"
}

# Create pre-restore backup
create_pre_restore_backup() {
    log_info "Creating pre-restore backup..."
    
    local timestamp=$(date +%Y%m%d_%H%M%S)
    local backup_name="pre_restore_backup_${timestamp}.sql"
    
    kubectl exec -n "$NAMESPACE" deployment/postgres -- \
        pg_dump \
        -U vibe_ensemble \
        -d vibe_ensemble \
        --no-password \
        --clean \
        --if-exists \
        --create \
        --format=custom \
        --compress=9 > "/tmp/$backup_name"
    
    if [[ $? -eq 0 ]]; then
        log_info "Pre-restore backup created: /tmp/$backup_name"
        echo "/tmp/$backup_name"
    else
        log_error "Failed to create pre-restore backup"
        exit 1
    fi
}

# Stop application pods
stop_application() {
    log_info "Scaling down application deployment..."
    
    # Scale down the main application
    kubectl scale deployment vibe-ensemble-server --replicas=0 -n "$NAMESPACE"
    
    # Wait for pods to terminate
    local timeout=300
    local elapsed=0
    while kubectl get pods -n "$NAMESPACE" -l app.kubernetes.io/name=vibe-ensemble | grep -q Running; do
        if [[ $elapsed -ge $timeout ]]; then
            log_error "Timeout waiting for pods to terminate"
            exit 1
        fi
        sleep 5
        elapsed=$((elapsed + 5))
    done
    
    log_info "Application pods stopped"
}

# Start application pods
start_application() {
    log_info "Scaling up application deployment..."
    
    # Scale up the main application
    kubectl scale deployment vibe-ensemble-server --replicas=3 -n "$NAMESPACE"
    
    # Wait for pods to be ready
    kubectl rollout status deployment/vibe-ensemble-server -n "$NAMESPACE" --timeout=600s
    
    log_info "Application pods started"
}

# Restore database
restore_database() {
    local backup_path="$1"
    
    log_info "Starting database restore from: $(basename "$backup_path")"
    
    # Copy backup file to postgres pod
    local postgres_pod=$(kubectl get pods -n "$NAMESPACE" -l app.kubernetes.io/name=postgres -o jsonpath='{.items[0].metadata.name}')
    
    if [[ -z "$postgres_pod" ]]; then
        log_error "No postgres pod found"
        exit 1
    fi
    
    log_info "Copying backup to postgres pod: $postgres_pod"
    kubectl cp "$backup_path" "$NAMESPACE/$postgres_pod:/tmp/restore_backup.sql"
    
    # Restore database
    log_info "Restoring database..."
    
    if [[ "$backup_path" =~ \.sql$ ]]; then
        # Text format restore
        kubectl exec -n "$NAMESPACE" "$postgres_pod" -- \
            psql -U vibe_ensemble -d postgres -c "DROP DATABASE IF EXISTS vibe_ensemble;"
        
        kubectl exec -n "$NAMESPACE" "$postgres_pod" -- \
            psql -U vibe_ensemble -d postgres -f /tmp/restore_backup.sql
    else
        # Custom format restore
        kubectl exec -n "$NAMESPACE" "$postgres_pod" -- \
            dropdb -U vibe_ensemble vibe_ensemble || true
        
        kubectl exec -n "$NAMESPACE" "$postgres_pod" -- \
            pg_restore \
            -U vibe_ensemble \
            -d postgres \
            --clean \
            --if-exists \
            --create \
            --exit-on-error \
            --verbose \
            /tmp/restore_backup.sql
    fi
    
    if [[ $? -eq 0 ]]; then
        log_info "Database restore completed successfully"
    else
        log_error "Database restore failed"
        exit 1
    fi
    
    # Cleanup backup file from pod
    kubectl exec -n "$NAMESPACE" "$postgres_pod" -- rm -f /tmp/restore_backup.sql
}

# Verify restored database
verify_restoration() {
    log_info "Verifying database restoration..."
    
    local postgres_pod=$(kubectl get pods -n "$NAMESPACE" -l app.kubernetes.io/name=postgres -o jsonpath='{.items[0].metadata.name}')
    
    # Check database connectivity
    if ! kubectl exec -n "$NAMESPACE" "$postgres_pod" -- \
        psql -U vibe_ensemble -d vibe_ensemble -c "SELECT 1;" > /dev/null; then
        log_error "Cannot connect to restored database"
        return 1
    fi
    
    # Check table existence
    local table_count=$(kubectl exec -n "$NAMESPACE" "$postgres_pod" -- \
        psql -U vibe_ensemble -d vibe_ensemble -t -c "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'public';" | tr -d ' ')
    
    if [[ "$table_count" -lt 5 ]]; then
        log_error "Restored database has insufficient tables (found: $table_count)"
        return 1
    fi
    
    log_info "Database restoration verification passed (tables: $table_count)"
    return 0
}

# Rollback restoration
rollback_restoration() {
    local pre_restore_backup="$1"
    
    log_warn "Rolling back database restoration..."
    
    if [[ -f "$pre_restore_backup" ]]; then
        restore_database "$pre_restore_backup"
        
        if verify_restoration; then
            log_info "Rollback completed successfully"
        else
            log_error "Rollback failed - manual intervention required"
            exit 1
        fi
    else
        log_error "Pre-restore backup not found - cannot rollback"
        exit 1
    fi
}

# Main execution
main() {
    log_info "Starting database restore process"
    log_info "Namespace: $NAMESPACE"
    log_info "Service: $POSTGRES_SERVICE"
    log_info "Backup file: $BACKUP_FILE"
    log_info "Force: $FORCE"
    log_info "Verify: $VERIFY"
    log_info "Dry run: $DRY_RUN"
    
    # Check prerequisites
    check_prerequisites
    
    # Download backup
    local backup_path
    backup_path=$(download_backup "$BACKUP_FILE")
    
    # Verify backup if requested
    if [[ "$VERIFY" == "true" ]]; then
        verify_backup "$backup_path"
    fi
    
    if [[ "$DRY_RUN" == "true" ]]; then
        log_info "Dry run completed - no changes made"
        rm -f "$backup_path"
        return 0
    fi
    
    # Confirmation prompt
    if [[ "$FORCE" != "true" ]]; then
        echo
        log_warn "This will restore the database from backup and may cause data loss!"
        log_warn "Backup file: $(basename "$backup_path")"
        log_warn "Target namespace: $NAMESPACE"
        echo
        read -p "Are you sure you want to continue? (yes/no): " -r
        if [[ ! $REPLY =~ ^[Yy][Ee][Ss]$ ]]; then
            log_info "Restore cancelled by user"
            rm -f "$backup_path"
            exit 0
        fi
    fi
    
    # Create pre-restore backup
    local pre_restore_backup
    pre_restore_backup=$(create_pre_restore_backup)
    
    # Stop application
    stop_application
    
    # Restore database
    if restore_database "$backup_path"; then
        # Verify restoration
        if verify_restoration; then
            # Start application
            start_application
            
            log_info "Database restore completed successfully"
            log_info "Pre-restore backup saved: $pre_restore_backup"
        else
            log_error "Database verification failed - rolling back"
            rollback_restoration "$pre_restore_backup"
        fi
    else
        log_error "Database restore failed - rolling back"
        rollback_restoration "$pre_restore_backup"
    fi
    
    # Cleanup
    rm -f "$backup_path"
    
    log_info "Restore process completed"
}

# Run main function
main "$@"