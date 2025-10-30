#!/bin/bash

# Quick smoke test - validates basic functionality in < 30 seconds
# Run this before committing changes

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEST_CONFIG_DIR="$PROJECT_DIR/config/test"

print_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

print_error() {
    echo -e "${RED}✗ $1${NC}"
}

print_info() {
    echo -e "${BLUE}ℹ $1${NC}"
}

cleanup() {
    print_info "Cleaning up..."
    pkill -f "cloud-p2p-simple" 2>/dev/null || true
    pkill -f "target/release/server" 2>/dev/null || true
    pkill -f "target/release/client" 2>/dev/null || true
    sleep 1
}

# Trap to ensure cleanup on exit
trap cleanup EXIT

echo ""
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}  CloudP2P - Quick Smoke Test${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""

# Clean start
cleanup

# Check if project is built
if [ ! -f "$PROJECT_DIR/target/release/server" ]; then
    print_error "Project not built. Building..."
    cd "$PROJECT_DIR"
    cargo build --release
fi

# Check for test image
if [ ! -f "$PROJECT_DIR/user-data/uploads/test_image.jpg" ]; then
    print_error "Test image not found"
    exit 1
fi

print_info "Starting smoke test..."
echo ""

# Test 1: Can servers start?
print_info "[1/5] Testing server startup..."
"$PROJECT_DIR/target/release/server" -c "$TEST_CONFIG_DIR/server1.toml" > /dev/null 2>&1 &
PID1=$!
sleep 1

if ps -p $PID1 > /dev/null; then
    print_success "Server 1 started"
else
    print_error "Server 1 failed to start"
    exit 1
fi

"$PROJECT_DIR/target/release/server" -c "$TEST_CONFIG_DIR/server2.toml" > /dev/null 2>&1 &
PID2=$!
"$PROJECT_DIR/target/release/server" -c "$TEST_CONFIG_DIR/server3.toml" > /dev/null 2>&1 &
PID3=$!
sleep 2

print_success "All servers started"
echo ""

# Test 2: Leader election
print_info "[2/5] Testing leader election..."
sleep 5

# Check if any server became leader (very basic check - just see if they're still running)
if ps -p $PID1 > /dev/null && ps -p $PID2 > /dev/null && ps -p $PID3 > /dev/null; then
    print_success "Servers running, election likely completed"
else
    print_error "One or more servers crashed"
    exit 1
fi
echo ""

# Test 3: Can client start?
print_info "[3/5] Testing client startup..."

# Create a minimal test client config
cat > /tmp/smoke_client.toml << EOF
[client]
name = "SmokeTest"
server_addresses = [
    "127.0.0.1:9001",
    "127.0.0.1:9002",
    "127.0.0.1:9003"
]

[requests]
rate_per_second = 1.0
duration_seconds = 3.0
request_processing_ms = 2000
load_per_request = 0.1
EOF

"$PROJECT_DIR/target/release/client" -c /tmp/smoke_client.toml > /tmp/smoke_client.log 2>&1 &
CLIENT_PID=$!

print_success "Client started"
echo ""

# Test 4: Wait for client to complete
print_info "[4/5] Testing task processing..."
sleep 8

# Check if client completed
if grep -q "completed successfully" /tmp/smoke_client.log 2>/dev/null; then
    COMPLETED=$(grep -c "completed successfully" /tmp/smoke_client.log)
    print_success "Tasks completed: $COMPLETED"
else
    print_error "No tasks completed - check /tmp/smoke_client.log"
    cat /tmp/smoke_client.log
    exit 1
fi
echo ""

# Test 5: Check outputs
print_info "[5/5] Checking encrypted outputs..."
OUTPUT_COUNT=$(ls -1 "$PROJECT_DIR/user-data/outputs/encrypted_SmokeTest"* 2>/dev/null | wc -l)

if [ "$OUTPUT_COUNT" -gt 0 ]; then
    print_success "Found $OUTPUT_COUNT encrypted file(s)"
else
    print_error "No encrypted files generated"
    exit 1
fi
echo ""

# Final summary
echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${GREEN}✓ SMOKE TEST PASSED${NC}"
echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""
echo "System is functioning correctly!"
echo ""
echo "Next steps:"
echo "  - Run full test suite: ./tests/integration_test.sh"
echo "  - Check logs: tail -f test_results/logs/*"
echo ""

exit 0
