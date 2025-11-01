#!/bin/bash

# CloudP2P Stress Test Script
#
# Runs multiple client instances using a single TOML config.
# Each client gets a unique ID appended to the machine name.
#
# Usage:
#   ./scripts/stress_test.sh <machine_id> [num_clients] [config_file]
#
# Examples:
#   ./scripts/stress_test.sh 1              # Machine 1, 10 clients, default config
#   ./scripts/stress_test.sh 2 20           # Machine 2, 20 clients, default config
#   ./scripts/stress_test.sh 1 100 config/custom.toml  # Machine 1, 100 clients, custom config

set -e

# Machine ID (required)
if [ -z "$1" ]; then
    echo "Error: Machine ID is required"
    echo "Usage: $0 <machine_id> [num_clients] [config_file]"
    echo "Example: $0 1 10"
    exit 1
fi

MACHINE_ID=$1

# Number of clients to run (default: 10)
NUM_CLIENTS=${2:-10}

# Configuration (can be overridden via argument)
STRESS_CONFIG="${3:-./config/client_stress.toml}"
METRICS_DIR="./metrics"
CLIENT_BINARY="./target/release/client"

echo "========================================"
echo "CloudP2P Stress Test"
echo "========================================"
echo "Machine ID:        $MACHINE_ID"
echo "Number of Clients: $NUM_CLIENTS"
echo "Config File:       $STRESS_CONFIG"
echo "Metrics Directory: $METRICS_DIR"
echo "========================================"
echo ""

# Check prerequisites
if [ ! -f "$CLIENT_BINARY" ]; then
    echo "Error: Client binary not found at $CLIENT_BINARY"
    echo "Please build first: cargo build --release"
    exit 1
fi

if [ ! -f "$STRESS_CONFIG" ]; then
    echo "Error: Config not found at $STRESS_CONFIG"
    exit 1
fi

# Create a temporary config file with the correct machine name
TEMP_CONFIG=$(mktemp /tmp/client_stress_machine${MACHINE_ID}.XXXXXX.toml)
echo "Creating temporary config with Machine_${MACHINE_ID}..."

# Replace the machine name in the config
sed "s/name = \"Machine_[0-9]*\"/name = \"Machine_${MACHINE_ID}\"/" "$STRESS_CONFIG" > "$TEMP_CONFIG"

# Verify the replacement worked
if ! grep -q "name = \"Machine_${MACHINE_ID}\"" "$TEMP_CONFIG"; then
    echo "Error: Failed to set machine name in config"
    rm -f "$TEMP_CONFIG"
    exit 1
fi

echo "Using config with name: Machine_${MACHINE_ID}"
echo ""

# Clean up old metrics
echo "Cleaning up old metrics..."
rm -rf "$METRICS_DIR"
mkdir -p "$METRICS_DIR"

# Array to store PIDs
declare -a CLIENT_PIDS

# Function to cleanup on exit
cleanup() {
    echo ""
    echo "Cleaning up client processes..."
    for pid in "${CLIENT_PIDS[@]}"; do
        if kill -0 "$pid" 2>/dev/null; then
            kill "$pid" 2>/dev/null || true
        fi
    done
    wait
    echo "All clients terminated."

    # Remove temporary config file
    if [ -f "$TEMP_CONFIG" ]; then
        rm -f "$TEMP_CONFIG"
        echo "Removed temporary config file."
    fi
}

# Register cleanup function
trap cleanup EXIT INT TERM

# Start timestamp
START_TIME=$(date +%s)

echo "Starting clients..."
echo ""

# Start clients using the same config with different client IDs
for ((i=1; i<=NUM_CLIENTS; i++)); do
    METRICS_FILE="$METRICS_DIR/client_$i.json"
    LOG_FILE="$METRICS_DIR/client_$i.log"

    echo "[$(date '+%Y-%m-%d %H:%M:%S')] Starting client $i/$NUM_CLIENTS"

    # Run client in background with --client-id
    "$CLIENT_BINARY" \
        --config "$TEMP_CONFIG" \
        --client-id "$i" \
        --metrics-output "$METRICS_FILE" \
        > "$LOG_FILE" 2>&1 &

    CLIENT_PID=$!
    CLIENT_PIDS+=("$CLIENT_PID")

    # Small delay between spawns
    sleep 0.1
done

echo ""
echo "All $NUM_CLIENTS clients started!"
echo "PIDs: ${CLIENT_PIDS[*]}"
echo ""
echo "Waiting for clients to complete..."
echo "(Press Ctrl+C to stop all clients)"
echo ""

# Monitor client processes
ACTIVE_CLIENTS=$NUM_CLIENTS
while [ $ACTIVE_CLIENTS -gt 0 ]; do
    ACTIVE_CLIENTS=0
    for pid in "${CLIENT_PIDS[@]}"; do
        if kill -0 "$pid" 2>/dev/null; then
            ACTIVE_CLIENTS=$((ACTIVE_CLIENTS + 1))
        fi
    done

    # Show progress every 5 seconds
    if [ $(($(date +%s) % 5)) -eq 0 ]; then
        ELAPSED=$(($(date +%s) - START_TIME))
        echo "[$(date '+%Y-%m-%d %H:%M:%S')] Active clients: $ACTIVE_CLIENTS/$NUM_CLIENTS (Elapsed: ${ELAPSED}s)"
    fi

    sleep 1
done

# Calculate total time
END_TIME=$(date +%s)
TOTAL_TIME=$((END_TIME - START_TIME))

echo ""
echo "========================================"
echo "Stress Test Completed!"
echo "========================================"
echo "Total Time:    ${TOTAL_TIME}s"
echo "Clients Run:   $NUM_CLIENTS"
echo "Metrics Dir:   $METRICS_DIR"
echo "========================================"
echo ""

# Show generated files
echo "Metrics files:"
ls -lh "$METRICS_DIR"/*.json 2>/dev/null || echo "  (No metrics files found)"
echo ""

echo "Logs:"
ls -lh "$METRICS_DIR"/*.log 2>/dev/null || echo "  (No log files found)"
echo ""

echo "Quick stats:"
echo "  Total requests sent:"
if command -v jq &> /dev/null; then
    jq -s 'map(.aggregated_stats.total_requests) | add' "$METRICS_DIR"/*.json 2>/dev/null || echo "  (Unable to calculate)"
else
    echo "  (Install 'jq' to see stats)"
fi

echo ""
echo "To view a specific client log:"
echo "  tail -f $METRICS_DIR/client_1.log"
echo ""
echo "To view metrics:"
echo "  cat $METRICS_DIR/client_1.json | jq"
