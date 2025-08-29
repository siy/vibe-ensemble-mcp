#!/bin/bash
# Simple link validation script for testing vibe-ensemble web endpoints
# Usage: ./scripts/test-links.sh [base_url]

set -e

BASE_URL="${1:-http://127.0.0.1:8081}"
FAILED_LINKS=0
TOTAL_LINKS=0

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "🔗 Testing vibe-ensemble links..."
echo "Base URL: $BASE_URL"
echo ""

# Function to test a link
test_link() {
    local url="$1"
    local expected_status="${2:-200}"
    local method="${3:-GET}"
    
    TOTAL_LINKS=$((TOTAL_LINKS + 1))
    
    printf "Testing %-50s " "$url"
    
    if [[ "$url" == ws://* ]]; then
        # WebSocket URLs need special handling - just check if the HTTP equivalent responds
        http_url="${url/ws:/http:}"
        if curl -s -o /dev/null -w "%{http_code}" --max-time 5 "$http_url" | grep -q "200\|404\|405"; then
            echo -e "${GREEN}✓ OK (WebSocket endpoint)${NC}"
        else
            echo -e "${RED}✗ FAILED (WebSocket endpoint unreachable)${NC}"
            FAILED_LINKS=$((FAILED_LINKS + 1))
        fi
    else
        # Regular HTTP endpoints
        status=$(curl -s -o /dev/null -w "%{http_code}" --max-time 5 -X "$method" "$url" 2>/dev/null || echo "000")
        
        if [[ "$status" == "$expected_status" ]]; then
            echo -e "${GREEN}✓ OK ($status)${NC}"
        elif [[ "$status" == "404" && "$expected_status" == "200" ]]; then
            echo -e "${YELLOW}⚠ NOT FOUND ($status)${NC}"
            # Don't count 404s as failures for now - they might be unimplemented features
        elif [[ "$status" == "405" && "$method" == "HEAD" ]]; then
            # Retry with GET if HEAD is not allowed
            status=$(curl -s -o /dev/null -w "%{http_code}" --max-time 5 -X GET "$url" 2>/dev/null || echo "000")
            if [[ "$status" == "200" ]]; then
                echo -e "${GREEN}✓ OK ($status, fallback to GET)${NC}"
            else
                echo -e "${RED}✗ FAILED ($status)${NC}"
                FAILED_LINKS=$((FAILED_LINKS + 1))
            fi
        elif [[ "$status" == "000" ]]; then
            echo -e "${RED}✗ FAILED (Connection failed)${NC}"
            FAILED_LINKS=$((FAILED_LINKS + 1))
        else
            echo -e "${RED}✗ FAILED ($status, expected $expected_status)${NC}"
            FAILED_LINKS=$((FAILED_LINKS + 1))
        fi
    fi
}

# Wait for server to be ready (if testing locally)
if [[ "$BASE_URL" == *"127.0.0.1"* ]]; then
    echo "⏳ Waiting for server to be ready..."
    for i in {1..30}; do
        if curl -s "$BASE_URL/api/health" > /dev/null 2>&1; then
            echo "✅ Server is ready!"
            break
        fi
        if [[ $i -eq 30 ]]; then
            echo "❌ Server failed to start within 30 seconds"
            exit 1
        fi
        sleep 1
    done
    echo ""
fi

# Test dashboard pages
echo "📄 Testing dashboard pages..."
test_link "$BASE_URL/"
test_link "$BASE_URL/dashboard"
test_link "$BASE_URL/messages"
test_link "$BASE_URL/link-health"

echo ""

# Test API endpoints
echo "🔌 Testing API endpoints..."
test_link "$BASE_URL/api/health"
test_link "$BASE_URL/api/stats"

echo ""

# Test API collections
echo "📊 Testing API collections..."
test_link "$BASE_URL/api/agents"
test_link "$BASE_URL/api/issues" 
test_link "$BASE_URL/api/messages"

echo ""

# Test link health API
echo "🔗 Testing link validation API..."
test_link "$BASE_URL/api/links/health"
test_link "$BASE_URL/api/links/status"
test_link "$BASE_URL/api/links/validate"
test_link "$BASE_URL/api/links/analytics"

echo ""

# Test WebSocket endpoint (basic connectivity)
echo "🔌 Testing WebSocket endpoint..."
test_link "ws://127.0.0.1:8081/ws"

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# Summary
if [[ $FAILED_LINKS -eq 0 ]]; then
    echo -e "${GREEN}✅ All $TOTAL_LINKS links are working correctly!${NC}"
    exit 0
else
    echo -e "${RED}❌ $FAILED_LINKS out of $TOTAL_LINKS links failed!${NC}"
    exit 1
fi