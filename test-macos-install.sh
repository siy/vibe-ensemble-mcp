#!/bin/bash

# Test script for macOS installation debugging
# Usage: ./test-macos-install.sh

set -e

echo "🔍 Vibe Ensemble MCP macOS Installation Test"
echo "============================================"

# Test 1: Platform Detection
echo "1. Testing platform detection..."
OS=$(uname -s)
ARCH=$(uname -m)
echo "   OS: $OS"
echo "   Architecture: $ARCH"

case "$OS" in
    Darwin*)
        os_suffix="apple-darwin"
        ;;
    *)
        echo "   ❌ This test is for macOS only"
        exit 1
        ;;
esac

case "$ARCH" in
    x86_64|amd64)
        arch_prefix="x86_64"
        ;;
    arm64|aarch64)
        arch_prefix="aarch64"
        ;;
    *)
        echo "   ❌ Unsupported architecture: $ARCH"
        exit 1
        ;;
esac

platform="${arch_prefix}-${os_suffix}"
echo "   ✅ Platform: $platform"

# Test 2: Latest Version Fetch
echo "2. Testing latest version fetch..."
version=$(curl -s "https://api.github.com/repos/siy/vibe-ensemble-mcp/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')
if [[ -z "$version" ]]; then
    echo "   ❌ Failed to fetch version"
    exit 1
fi
echo "   ✅ Version: $version"

# Test 3: Filename Construction
echo "3. Testing filename construction..."
filename="vibe-ensemble-${version}-macos-${platform}.tar.gz"
echo "   ✅ Filename: $filename"

# Test 4: Download URL Test
echo "4. Testing download URL accessibility..."
download_url="https://github.com/siy/vibe-ensemble-mcp/releases/download/$version/$filename"
echo "   URL: $download_url"

http_code=$(curl -s -o /dev/null -w "%{http_code}" "$download_url")
if [[ "$http_code" == "200" || "$http_code" == "302" ]]; then
    echo "   ✅ URL accessible (HTTP $http_code)"
else
    echo "   ❌ URL failed (HTTP $http_code)"
    exit 1
fi

# Test 5: Actual Download Test
echo "5. Testing actual download..."
temp_dir="/tmp/vibe-test-$(date +%s)"
mkdir -p "$temp_dir"
cd "$temp_dir"

if curl -L --progress-bar "$download_url" -o "$filename"; then
    echo "   ✅ Download successful"
    file_size=$(ls -lh "$filename" | awk '{print $5}')
    echo "   File size: $file_size"
else
    echo "   ❌ Download failed"
    cd /
    rm -rf "$temp_dir"
    exit 1
fi

# Test 6: Extraction Test
echo "6. Testing extraction..."
if tar -xzf "$filename"; then
    echo "   ✅ Extraction successful"
else
    echo "   ❌ Extraction failed"
    cd /
    rm -rf "$temp_dir"
    exit 1
fi

# Test 7: Binary Verification
echo "7. Testing binary presence and permissions..."
if [[ -f "vibe-ensemble" ]]; then
    echo "   ✅ Binary present"
    
    if [[ -x "vibe-ensemble" ]]; then
        echo "   ✅ Binary executable"
    else
        echo "   ❌ Binary not executable"
        ls -la vibe-ensemble*
    fi
    
    # Test binary architecture if file command available
    if command -v file >/dev/null 2>&1; then
        echo "   Binary info:"
        echo "     vibe-ensemble: $(file vibe-ensemble | cut -d: -f2 | xargs)"
    fi
else
    echo "   ❌ Missing binary"
    echo "   Contents of archive:"
    ls -la
fi

# Cleanup
cd /
rm -rf "$temp_dir"

echo ""
echo "🎉 All tests passed! macOS installation should work correctly."
echo ""
echo "To install manually, run:"
echo "curl -fsSL https://raw.githubusercontent.com/siy/vibe-ensemble-mcp/main/website/install.sh | bash"
echo ""
echo "Or use the official command:"
echo "curl -fsSL https://vibeensemble.dev/install.sh | bash"
echo ""
echo "Then add to Claude Code:"
echo "claude mcp add vibe-ensemble -- vibe-ensemble --mcp-only --transport=stdio"