#!/bin/bash

set -e

# Vibe Ensemble MCP Server Installer
# This script automatically downloads and installs the latest release

REPO="siy/vibe-ensemble-mcp"
INSTALL_DIR="${HOME}/.local/bin"
BINARY_NAME="vibe-ensemble-mcp"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Detect architecture
detect_arch() {
    local arch
    case "$(uname -m)" in
        x86_64|amd64)
            arch="x86_64"
            ;;
        arm64|aarch64)
            arch="aarch64"
            ;;
        *)
            print_error "Unsupported architecture: $(uname -m)"
            exit 1
            ;;
    esac
    echo "$arch"
}

# Detect platform
detect_platform() {
    case "$(uname -s)" in
        Darwin)
            echo "apple-darwin"
            ;;
        Linux)
            echo "unknown-linux-gnu"
            ;;
        *)
            print_error "Unsupported platform: $(uname -s)"
            exit 1
            ;;
    esac
}

# Get latest release version
get_latest_version() {
    local latest_url="https://api.github.com/repos/${REPO}/releases/latest"

    if command -v curl >/dev/null 2>&1; then
        curl -s "$latest_url" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/'
    elif command -v wget >/dev/null 2>&1; then
        wget -qO- "$latest_url" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/'
    else
        print_error "Neither curl nor wget is available. Please install one of them."
        exit 1
    fi
}

# Download and install
install_binary() {
    local version="$1"
    local arch="$2"
    local platform="$3"
    
    local target="${arch}-${platform}"
    local archive_name="vibe-ensemble-mcp-${target}.tar.gz"
    local download_url="https://github.com/${REPO}/releases/download/${version}/${archive_name}"
    
    print_status "Downloading Vibe Ensemble MCP Server ${version} for ${target}..."
    
    # Create temporary directory
    local temp_dir=$(mktemp -d)
    cd "$temp_dir"
    
    # Download archive
    if command -v curl >/dev/null 2>&1; then
        curl -L -o "$archive_name" "$download_url"
    elif command -v wget >/dev/null 2>&1; then
        wget -O "$archive_name" "$download_url"
    fi
    
    if [ ! -f "$archive_name" ]; then
        print_error "Failed to download $archive_name"
        exit 1
    fi
    
    print_status "Extracting archive..."
    tar -xzf "$archive_name"
    
    # Find the binary
    local binary_path=""
    if [ -f "${BINARY_NAME}" ]; then
        binary_path="${BINARY_NAME}"
    elif [ -f "vibe-ensemble-mcp-${target}/${BINARY_NAME}" ]; then
        binary_path="vibe-ensemble-mcp-${target}/${BINARY_NAME}"
    else
        print_error "Binary not found in archive"
        exit 1
    fi
    
    # Create install directory
    mkdir -p "$INSTALL_DIR"
    
    # Install binary
    print_status "Installing to ${INSTALL_DIR}/${BINARY_NAME}..."
    cp "$binary_path" "${INSTALL_DIR}/${BINARY_NAME}"
    chmod +x "${INSTALL_DIR}/${BINARY_NAME}"
    
    # Cleanup
    cd /
    rm -rf "$temp_dir"
    
    print_success "Vibe Ensemble MCP Server installed successfully!"
}

# Check if binary is in PATH
check_path() {
    if ! echo "$PATH" | grep -q "$INSTALL_DIR"; then
        print_warning "Install directory ${INSTALL_DIR} is not in your PATH."
        print_warning "Add the following line to your shell profile (~/.bashrc, ~/.zshrc, etc.):"
        echo ""
        echo "    export PATH=\"${INSTALL_DIR}:\$PATH\""
        echo ""
        print_warning "Or run the following command to add it to your current session:"
        echo ""
        echo "    export PATH=\"${INSTALL_DIR}:\$PATH\""
        echo ""
    fi
}

# Main installation process
main() {
    echo ""
    print_status "Vibe Ensemble MCP Server Installer"
    echo ""
    
    # Check for required tools
    if ! command -v tar >/dev/null 2>&1; then
        print_error "tar is required but not installed"
        exit 1
    fi
    
    if ! command -v curl >/dev/null 2>&1 && ! command -v wget >/dev/null 2>&1; then
        print_error "Either curl or wget is required but neither is installed"
        exit 1
    fi
    
    # Detect system
    local arch=$(detect_arch)
    local platform=$(detect_platform)

    print_status "Detected platform: ${arch}-${platform}"
    print_status "Fetching latest release information..."
    local version=$(get_latest_version)

    if [ -z "$version" ]; then
        print_error "Failed to get latest version information"
        exit 1
    fi

    print_status "Latest version: ${version}"
    
    # Install
    install_binary "$version" "$arch" "$platform"
    
    # Check PATH
    check_path
    
    # Success message
    echo ""
    print_success "Installation complete!"
    print_status "You can now run: ${BINARY_NAME} --help"
    print_status "To start the server: ${BINARY_NAME} --port 3000"
    echo ""
    print_status "Documentation: https://github.com/${REPO}"
    echo ""
}

# Run main function
main "$@"