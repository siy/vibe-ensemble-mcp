#!/bin/bash

set -e

# Vibe Ensemble MCP Server Installation Script
# Usage: curl -fsSL https://vibeensemble.dev/install.sh | bash

GITHUB_REPO="siy/vibe-ensemble-mcp"
INSTALL_DIR="/usr/local/bin"
TEMP_DIR="/tmp/vibe-ensemble-install"

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

# Detect OS and architecture
detect_platform() {
    local os arch
    
    case "$(uname -s)" in
        Darwin*)
            os="apple-darwin"
            ;;
        Linux*)
            os="unknown-linux-gnu"
            ;;
        *)
            log_error "Unsupported operating system: $(uname -s)"
            exit 1
            ;;
    esac
    
    case "$(uname -m)" in
        x86_64|amd64)
            arch="x86_64"
            ;;
        arm64|aarch64)
            arch="aarch64"
            ;;
        *)
            log_error "Unsupported architecture: $(uname -m)"
            exit 1
            ;;
    esac
    
    echo "${arch}-${os}"
}

# Check if running as root
check_root() {
    if [[ $EUID -eq 0 ]]; then
        log_warning "Running as root. Consider running as a regular user with sudo for installation."
    fi
}

# Check dependencies
check_dependencies() {
    local deps=("curl" "tar")
    local missing=()
    
    for dep in "${deps[@]}"; do
        if ! command -v "$dep" &> /dev/null; then
            missing+=("$dep")
        fi
    done
    
    if [[ ${#missing[@]} -ne 0 ]]; then
        log_error "Missing required dependencies: ${missing[*]}"
        log_info "Please install them and try again."
        exit 1
    fi
}

# Get latest release version
get_latest_version() {
    curl -s "https://api.github.com/repos/$GITHUB_REPO/releases/latest" | \
        grep '"tag_name":' | \
        sed -E 's/.*"([^"]+)".*/\1/'
}

# Download and extract binaries
download_and_extract() {
    local version="$1"
    local platform="$2"
    local filename
    
    if [[ "$platform" == *"apple-darwin"* ]]; then
        filename="vibe-ensemble-${version}-macos-${platform}.tar.gz"
    else
        filename="vibe-ensemble-${version}-linux-${platform}.tar.gz"
    fi
    
    local download_url="https://github.com/$GITHUB_REPO/releases/download/$version/$filename"
    
    log_info "Downloading Vibe Ensemble MCP Server $version for $platform..."
    
    # Create temp directory
    mkdir -p "$TEMP_DIR"
    cd "$TEMP_DIR"
    
    # Download with progress bar
    if curl -L --progress-bar "$download_url" -o "$filename"; then
        log_success "Download completed"
    else
        log_error "Failed to download from $download_url"
        exit 1
    fi
    
    # Extract
    log_info "Extracting binaries..."
    if tar -xzf "$filename"; then
        log_success "Extraction completed"
    else
        log_error "Failed to extract $filename"
        exit 1
    fi
}

# Install binaries
install_binaries() {
    log_info "Installing binaries to $INSTALL_DIR..."
    
    # Check if install directory exists and is writable
    if [[ ! -d "$INSTALL_DIR" ]]; then
        log_info "Creating install directory $INSTALL_DIR..."
        sudo mkdir -p "$INSTALL_DIR"
    fi
    
    # Install binary
    local binary="vibe-ensemble"
    
    if [[ -f "$binary" ]]; then
        log_info "Installing $binary..."
        if sudo cp "$binary" "$INSTALL_DIR/" && sudo chmod +x "$INSTALL_DIR/$binary"; then
            log_success "$binary installed successfully"
        else
            log_error "Failed to install $binary"
            exit 1
        fi
    else
        log_error "$binary not found in archive"
        exit 1
    fi
}

# Verify installation
verify_installation() {
    log_info "Verifying installation..."
    
    if command -v vibe-ensemble &> /dev/null; then
        local version
        version=$(vibe-ensemble --version 2>/dev/null || echo "unknown")
        log_success "vibe-ensemble installed: $version"
    else
        log_error "vibe-ensemble not found in PATH"
        exit 1
    fi
    
}

# Cleanup
cleanup() {
    log_info "Cleaning up temporary files..."
    rm -rf "$TEMP_DIR"
    log_success "Cleanup completed"
}

# Print next steps
print_next_steps() {
    echo
    log_success "🎉 Vibe Ensemble MCP Server installed successfully!"
    echo
    echo -e "${BLUE}Next steps:${NC}"
    echo "1. Start the server:"
    echo -e "   ${GREEN}vibe-ensemble${NC}"
    echo
    echo "2. Add to Claude Code (choose one):"
    echo -e "   ${GREEN}# Local scope (current project only)${NC}"
    echo '   claude mcp add vibe-ensemble -- vibe-ensemble --mcp-only --transport=stdio'
    echo
    echo -e "   ${GREEN}# User scope (all projects)${NC}"
    echo '   claude mcp add -s user vibe-ensemble -- vibe-ensemble --mcp-only --transport=stdio'
    echo
    echo -e "   ${GREEN}# Project scope (shared with team)${NC}"
    echo '   claude mcp add -s project vibe-ensemble -- vibe-ensemble --mcp-only --transport=stdio'
    echo
    echo -e "   ${GREEN}# HTTP transport (server already running on 8080)${NC}"
    echo '   claude mcp add --transport http vibe-ensemble http://localhost:8080/mcp'
    echo
    echo -e "   ${GREEN}# SSE transport (event stream monitoring)${NC}"
    echo '   claude mcp add --transport sse vibe-ensemble http://localhost:8080/mcp/events'
    echo
    echo "3. Access the web dashboard: http://127.0.0.1:8081"
    echo "4. Check health: http://127.0.0.1:8080/api/health"
    echo
    echo -e "${BLUE}Documentation:${NC} https://vibeensemble.dev"
    echo -e "${BLUE}GitHub:${NC} https://github.com/$GITHUB_REPO"
    echo
}

# Main installation function
main() {
    echo -e "${BLUE}Vibe Ensemble MCP Server Installer${NC}"
    echo "============================================"
    echo
    
    check_root
    check_dependencies
    
    local platform
    platform=$(detect_platform)
    log_info "Detected platform: $platform"
    
    local version
    log_info "Fetching latest release version..."
    version=$(get_latest_version)
    if [[ -z "$version" ]]; then
        log_error "Failed to fetch latest version"
        exit 1
    fi
    log_info "Latest version: $version"
    
    download_and_extract "$version" "$platform"
    install_binaries
    verify_installation
    cleanup
    print_next_steps
}

# Handle interrupts
trap 'log_error "Installation interrupted"; cleanup; exit 1' INT TERM

# Run main function
main "$@"