#!/bin/bash
# Secure Docker build script for Vibe Ensemble MCP Server
# Implements security scanning and multi-architecture builds

set -euo pipefail

# Configuration
REGISTRY="${REGISTRY:-ghcr.io/siy/vibe-ensemble-mcp}"
VERSION="${VERSION:-$(git rev-parse --short HEAD)}"
PLATFORMS="${PLATFORMS:-linux/amd64,linux/arm64}"
DOCKERFILE="${DOCKERFILE:-Dockerfile.production}"
TARGET_STAGE="${TARGET_STAGE:-production}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
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

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."
    
    if ! command -v docker &> /dev/null; then
        log_error "Docker is not installed"
        exit 1
    fi
    
    if ! command -v buildx &> /dev/null && ! docker buildx version &> /dev/null; then
        log_error "Docker Buildx is not available"
        exit 1
    fi
    
    if ! command -v trivy &> /dev/null; then
        log_warn "Trivy is not installed - security scanning will be skipped"
    fi
    
    if ! command -v hadolint &> /dev/null; then
        log_warn "Hadolint is not installed - Dockerfile linting will be skipped"
    fi
    
    log_info "Prerequisites check completed"
}

# Lint Dockerfile
lint_dockerfile() {
    if command -v hadolint &> /dev/null; then
        log_info "Linting Dockerfile..."
        hadolint "${DOCKERFILE}" || {
            log_error "Dockerfile linting failed"
            exit 1
        }
        log_info "Dockerfile linting passed"
    else
        log_warn "Skipping Dockerfile linting (hadolint not installed)"
    fi
}

# Build and scan image
build_and_scan() {
    local stage="$1"
    local tag="${REGISTRY}:${VERSION}-${stage}"
    
    log_info "Building ${stage} image: ${tag}"
    
    # Build for local platform first for security scanning
    docker build \
        --target "${stage}" \
        --tag "${tag}" \
        --file "${DOCKERFILE}" \
        --build-arg VERSION="${VERSION}" \
        --build-arg BUILD_DATE="$(date -u +'%Y-%m-%dT%H:%M:%SZ')" \
        --build-arg VCS_REF="$(git rev-parse HEAD)" \
        .
    
    # Security scan with Trivy
    if command -v trivy &> /dev/null; then
        log_info "Running security scan on ${tag}..."
        
        # Scan for critical and high vulnerabilities
        trivy image \
            --exit-code 1 \
            --severity HIGH,CRITICAL \
            --format table \
            "${tag}" || {
            log_error "Security scan failed for ${tag}"
            exit 1
        }
        
        # Generate detailed report
        trivy image \
            --format json \
            --output "security-report-${stage}-${VERSION}.json" \
            "${tag}"
        
        log_info "Security scan passed for ${tag}"
    else
        log_warn "Skipping security scan (trivy not installed)"
    fi
    
    return 0
}

# Multi-platform build
multi_platform_build() {
    local stage="$1"
    local tag="${REGISTRY}:${VERSION}-${stage}"
    local latest_tag="${REGISTRY}:${stage}"
    
    log_info "Building multi-platform image for ${stage}..."
    
    # Create and use buildx builder
    docker buildx create --name vibe-ensemble-builder --use --bootstrap || true
    
    # Build and push multi-platform image
    docker buildx build \
        --platform "${PLATFORMS}" \
        --target "${stage}" \
        --tag "${tag}" \
        --tag "${latest_tag}" \
        --file "${DOCKERFILE}" \
        --build-arg VERSION="${VERSION}" \
        --build-arg BUILD_DATE="$(date -u +'%Y-%m-%dT%H:%M:%SZ')" \
        --build-arg VCS_REF="$(git rev-parse HEAD)" \
        --push \
        .
    
    log_info "Multi-platform build completed for ${stage}"
}

# Generate SBOM (Software Bill of Materials)
generate_sbom() {
    local tag="${REGISTRY}:${VERSION}-${TARGET_STAGE}"
    
    if command -v syft &> /dev/null; then
        log_info "Generating SBOM..."
        syft "${tag}" -o spdx-json > "sbom-${VERSION}.json"
        syft "${tag}" -o table > "sbom-${VERSION}.txt"
        log_info "SBOM generated"
    else
        log_warn "Skipping SBOM generation (syft not installed)"
    fi
}

# Sign image with cosign
sign_image() {
    local tag="${REGISTRY}:${VERSION}-${TARGET_STAGE}"
    
    if command -v cosign &> /dev/null && [[ -n "${COSIGN_PRIVATE_KEY:-}" ]]; then
        log_info "Signing image with cosign..."
        cosign sign --key "${COSIGN_PRIVATE_KEY}" "${tag}"
        log_info "Image signed successfully"
    else
        log_warn "Skipping image signing (cosign not configured)"
    fi
}

# Test image functionality
test_image() {
    local tag="${REGISTRY}:${VERSION}-${TARGET_STAGE}"
    
    log_info "Testing image functionality..."
    
    # Basic functionality test
    docker run --rm --entrypoint="" "${tag}" vibe-ensemble-server --help > /dev/null || {
        log_error "Image functionality test failed"
        exit 1
    }
    
    # Health check test
    container_id=$(docker run -d -p 8080:8080 "${tag}")
    
    # Wait for startup
    sleep 10
    
    # Test health endpoint
    if curl -f http://localhost:8080/api/health > /dev/null 2>&1; then
        log_info "Health check test passed"
    else
        log_error "Health check test failed"
        docker logs "${container_id}"
        docker stop "${container_id}"
        exit 1
    fi
    
    docker stop "${container_id}"
    log_info "Image functionality tests passed"
}

# Cleanup
cleanup() {
    log_info "Cleaning up..."
    
    # Remove builder if it exists
    docker buildx rm vibe-ensemble-builder 2>/dev/null || true
    
    # Clean up dangling images
    docker image prune -f
    
    log_info "Cleanup completed"
}

# Main execution
main() {
    log_info "Starting secure build process for Vibe Ensemble MCP Server"
    log_info "Registry: ${REGISTRY}"
    log_info "Version: ${VERSION}"
    log_info "Target Stage: ${TARGET_STAGE}"
    log_info "Platforms: ${PLATFORMS}"
    
    # Set up cleanup trap
    trap cleanup EXIT
    
    # Execute build pipeline
    check_prerequisites
    lint_dockerfile
    build_and_scan "${TARGET_STAGE}"
    test_image
    
    # Only push if not in CI or if explicitly requested
    if [[ "${PUSH_IMAGE:-false}" == "true" ]] || [[ "${CI:-false}" == "true" ]]; then
        multi_platform_build "${TARGET_STAGE}"
        generate_sbom
        sign_image
    else
        log_info "Skipping push (set PUSH_IMAGE=true to push)"
    fi
    
    log_info "Secure build process completed successfully"
}

# Run main function
main "$@"