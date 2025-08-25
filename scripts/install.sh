#!/bin/bash
# Vibe Ensemble MCP Installation Script
# Usage: curl -fsSL https://get.vibe-ensemble.dev/install.sh | bash

set -Euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
REPO="siy/vibe-ensemble-mcp"
INSTALL_DIR="/usr/local/bin"
CONFIG_DIR="$HOME/.config/vibe-ensemble"

# Functions
log() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1"
    exit 1
}

# Detect OS and architecture
detect_platform() {
    local os=$(uname -s | tr '[:upper:]' '[:lower:]')
    local arch=$(uname -m)
    
    case "$os" in
        linux*)
            OS="linux"
            ;;
        darwin*)
            OS="macos"
            ;;
        *)
            error "Unsupported operating system: $os"
            ;;
    esac
    
    case "$arch" in
        x86_64)
            ARCH="x86_64"
            ;;
        aarch64|arm64)
            ARCH="aarch64"
            ;;
        *)
            error "Unsupported architecture: $arch"
            ;;
    esac
    
    if [ "$OS" = "linux" ]; then
        if [ "$ARCH" = "aarch64" ]; then
            TARGET="aarch64-unknown-linux-gnu"
        else
            TARGET="x86_64-unknown-linux-gnu"
        fi
        ARCHIVE_EXT="tar.gz"
    else
        if [ "$ARCH" = "aarch64" ]; then
            TARGET="aarch64-apple-darwin"
        else
            TARGET="x86_64-apple-darwin"
        fi
        ARCHIVE_EXT="tar.gz"
    fi
    
    log "Detected platform: $OS-$ARCH ($TARGET)"
}

# Check if running as root
check_permissions() {
    if [ "$EUID" -eq 0 ]; then
        SUDO=""
        INSTALL_DIR="/usr/local/bin"
    else
        SUDO="sudo"
        # Check if sudo is available
        if ! command -v sudo >/dev/null 2>&1; then
            warn "sudo not available, installing to user directory"
            INSTALL_DIR="$HOME/.local/bin"
            mkdir -p "$INSTALL_DIR"
            export PATH="$INSTALL_DIR:$PATH"
        fi
    fi
}

# Get latest release version
get_latest_version() {
    if command -v curl >/dev/null 2>&1; then
        VERSION=$(curl -sSf "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/' || echo "")
    elif command -v wget >/dev/null 2>&1; then
        VERSION=$(wget -qO- --https-only "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/' || echo "")
    else
        error "Neither curl nor wget found. Please install one of them."
    fi
    
    if [ -z "$VERSION" ]; then
        error "Failed to get latest version"
    fi
}

# Download and install binaries
install_binaries() {
    local tmpdir=$(mktemp -d)
    local filename="vibe-ensemble-mcp-$VERSION-$OS-$TARGET.$ARCHIVE_EXT"
    local url="https://github.com/$REPO/releases/download/$VERSION/$filename"
    
    log "Downloading $url..."
    
    cd "$tmpdir"
    if command -v curl >/dev/null 2>&1; then
        curl -sSfL -o "$filename" "$url"
    else
        wget --https-only -O "$filename" "$url"
    fi
    
    log "Extracting archive..."
    if [ "$ARCHIVE_EXT" = "tar.gz" ]; then
        tar -xzf "$filename"
    else
        unzip "$filename"
    fi
    
    log "Installing binaries to $INSTALL_DIR..."
    # Verify binaries exist before installation
    if [ ! -f "vibe-ensemble" ]; then
        error "Expected binaries not found in archive"
    fi

    $SUDO install -m 755 "vibe-ensemble" "$INSTALL_DIR/"
    
    # Cleanup
    cd /
    rm -rf "$tmpdir"
    
    log "Binaries installed successfully"
}

# Create configuration directory and files
setup_config() {
    log "Setting up configuration..."
    
    mkdir -p "$CONFIG_DIR"
    
    if [ ! -f "$CONFIG_DIR/config.toml" ]; then
        cat > "$CONFIG_DIR/config.toml" << EOF
[server]
host = "127.0.0.1"
port = 8080

[database]
url = "sqlite:./vibe_ensemble.db"
migrate_on_startup = true

[web]
enabled = true
host = "127.0.0.1"
port = 8081

[logging]
level = "info"
format = "pretty"
EOF
        log "Created default configuration at $CONFIG_DIR/config.toml"
    else
        log "Configuration file already exists at $CONFIG_DIR/config.toml"
    fi
}

# Install systemd service (Linux only)
install_service() {
    if [ "$OS" != "linux" ] || [ -z "$SUDO" ]; then
        return 0
    fi
    
    log "Installing systemd service..."
    
    $SUDO tee /etc/systemd/system/vibe-ensemble.service > /dev/null << EOF
[Unit]
Description=Vibe Ensemble MCP Server
After=network.target

[Service]
Type=simple
User=$USER
Group=$USER
WorkingDirectory=$HOME
ExecStart=$INSTALL_DIR/vibe-ensemble --config $CONFIG_DIR/config.toml
Restart=always
RestartSec=5
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
EOF
    
    $SUDO systemctl daemon-reload
    log "Systemd service installed. You can enable it with: sudo systemctl enable vibe-ensemble"
}

# Verify installation
verify_installation() {
    log "Verifying installation..."
    
    if ! command -v vibe-ensemble >/dev/null 2>&1; then
        if [ "$INSTALL_DIR" != "/usr/local/bin" ]; then
            warn "Binary not in PATH. You may need to add $INSTALL_DIR to your PATH"
            echo "export PATH=\"$INSTALL_DIR:\$PATH\"" >> "$HOME/.bashrc"
            echo "export PATH=\"$INSTALL_DIR:\$PATH\"" >> "$HOME/.zshrc" 2>/dev/null || true
        else
            error "Installation verification failed"
        fi
    fi
    
    log "Installation verified successfully!"
    
    # Show version
    if command -v vibe-ensemble >/dev/null 2>&1; then
        vibe-ensemble --version 2>/dev/null || log "Installed vibe-ensemble"
    fi
}

# Show post-install instructions
show_instructions() {
    echo
    echo -e "${BLUE}=== Installation Complete ===${NC}"
    echo
    echo "Vibe Ensemble MCP has been installed successfully!"
    echo
    echo "Next steps:"
    echo "1. Start the server:"
    echo "   vibe-ensemble"
    echo
    echo "2. Or run as a service (Linux with systemd):"
    echo "   sudo systemctl enable vibe-ensemble"
    echo "   sudo systemctl start vibe-ensemble"
    echo
    echo "3. Access the web dashboard:"
    echo "   http://localhost:8081"
    echo
    echo "4. Add to Claude Code (choose one):"
    echo "   # Local scope (current project only)"
    echo '   claude mcp add vibe-ensemble "vibe-ensemble --mcp-only --transport=stdio" --transport=stdio'
    echo
    echo "   # User scope (all projects)"
    echo '   claude mcp add vibe-ensemble "vibe-ensemble --mcp-only --transport=stdio" --transport=stdio -s user'
    echo
    echo "   # Project scope (shared with team)"
    echo '   claude mcp add vibe-ensemble "vibe-ensemble --mcp-only --transport=stdio" --transport=stdio -s project'
    echo
    echo "5. Check the API:"
    echo "   curl http://localhost:8080/health"
    echo
    echo "Configuration: $CONFIG_DIR/config.toml"
    echo "Documentation: https://github.com/$REPO/blob/main/docs/installation.md"
    echo
}

# Main installation process
main() {
    echo -e "${BLUE}=== Vibe Ensemble MCP Installer ===${NC}"
    echo
    
    detect_platform
    check_permissions
    log "Fetching latest release version..."
    get_latest_version
    log "Latest version: $VERSION"
    install_binaries
    setup_config
    install_service
    verify_installation
    show_instructions
}

# Handle script interruption
trap 'error "Installation interrupted"' INT TERM

# Run main installation
main "$@"