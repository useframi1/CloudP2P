#!/bin/bash

# CloudP2P Local Fault Simulation Script
#
# Simplified version for local testing that kills/restarts server processes
# without using SSH (for localhost testing).
#
# Usage:
#   ./scripts/fault_simulation_local.sh [config_file]
#
# Default config: ./scripts/config/fault_sim_local.conf

set -e

# Default configuration file
CONFIG_FILE="${1:-./scripts/config/fault_sim_local.conf}"

# Check if config file exists, if not use the regular one
if [ ! -f "$CONFIG_FILE" ]; then
    echo "Local config not found, using: ./scripts/config/fault_sim.conf"
    CONFIG_FILE="./scripts/config/fault_sim.conf"
fi

if [ ! -f "$CONFIG_FILE" ]; then
    echo "Error: Configuration file not found: $CONFIG_FILE"
    exit 1
fi

# Load configuration
echo "Loading configuration from: $CONFIG_FILE"
source "$CONFIG_FILE"

# Validate required parameters
if [ -z "$FAULT_INTERVAL_SECS" ] || [ -z "$RESTART_DELAY_SECS" ] || [ -z "$NUM_CYCLES" ]; then
    echo "Error: Missing required configuration parameters"
    echo "Required: FAULT_INTERVAL_SECS, RESTART_DELAY_SECS, NUM_CYCLES"
    exit 1
fi

# Build server list from configuration
declare -a SERVER_CONFIGS
declare -a SERVER_WORK_DIRS
declare -a SERVER_BINARIES
declare -a SERVER_IDS

# Parse server configurations (only localhost servers)
for i in {1..10}; do
    HOST_VAR="SERVER_${i}_HOST"
    CONFIG_VAR="SERVER_${i}_CONFIG"
    WORK_DIR_VAR="SERVER_${i}_WORK_DIR"
    BINARY_VAR="SERVER_${i}_BINARY"

    HOST="${!HOST_VAR}"
    CONFIG="${!CONFIG_VAR}"
    WORK_DIR="${!WORK_DIR_VAR}"
    BINARY="${!BINARY_VAR}"

    # Only process localhost servers
    if [ -n "$HOST" ] && [[ "$HOST" == "localhost" || "$HOST" == "127.0.0.1" ]]; then
        SERVER_CONFIGS+=("$CONFIG")
        SERVER_WORK_DIRS+=("${WORK_DIR:-.}")
        SERVER_BINARIES+=("${BINARY:-./target/release/server}")
        SERVER_IDS+=("$i")
    fi
done

NUM_SERVERS=${#SERVER_CONFIGS[@]}

if [ "$NUM_SERVERS" -eq 0 ]; then
    echo "Error: No localhost servers configured"
    echo "Please set SERVER_N_HOST to 'localhost' or '127.0.0.1'"
    exit 1
fi

# Log file for fault events
LOG_FILE="${LOG_FILE:-./fault_events_local.log}"
mkdir -p "$(dirname "$LOG_FILE")"

# Function to log with timestamp
log_event() {
    local message="$1"
    local timestamp=$(date '+%Y-%m-%d %H:%M:%S')
    echo "[$timestamp] $message" | tee -a "$LOG_FILE"
}

# Function to get server PID locally
get_server_pid() {
    local config_path="$1"
    # Extract just the config filename for more flexible matching
    local config_file=$(basename "$config_path")
    pgrep -f "server.*$config_file" | head -1 || echo ""
}

# Function to kill server locally
kill_server() {
    local config_path="$1"
    local server_id="$2"

    log_event "Attempting to kill Server $server_id (local)"

    local pid=$(get_server_pid "$config_path")

    if [ -z "$pid" ]; then
        log_event "WARNING: Server $server_id not running (no PID found)"
        return 1
    fi

    log_event "Found Server $server_id PID: $pid"

    # Kill the server process
    if kill -9 "$pid" 2>/dev/null; then
        sleep 1  # Give process time to die
        log_event "SUCCESS: Killed Server $server_id (PID $pid)"
        return 0
    else
        log_event "ERROR: Failed to kill Server $server_id"
        return 1
    fi
}

# Function to restart server locally
restart_server() {
    local config_path="$1"
    local work_dir="$2"
    local binary="$3"
    local server_id="$4"

    log_event "Attempting to restart Server $server_id (local)"

    # Check if server is already running
    local pid=$(get_server_pid "$config_path")
    if [ -n "$pid" ]; then
        log_event "WARNING: Server $server_id already running (PID $pid)"
        return 0
    fi

    # Start server in background
    (cd "$work_dir" && nohup "$binary" --config "$config_path" > /dev/null 2>&1 &)

    sleep 2  # Give server time to start

    # Verify server started
    pid=$(get_server_pid "$config_path")
    if [ -n "$pid" ]; then
        log_event "SUCCESS: Restarted Server $server_id (PID $pid)"
        return 0
    else
        log_event "ERROR: Server $server_id failed to start"
        return 1
    fi
}

echo "========================================="
echo "CloudP2P Local Fault Simulation - Ring Algorithm"
echo "========================================="
echo "Number of Servers:   $NUM_SERVERS"
echo "Fault Interval:      ${FAULT_INTERVAL_SECS}s"
echo "Restart Delay:       ${RESTART_DELAY_SECS}s"
echo "Number of Cycles:    $NUM_CYCLES"
echo "Log File:            $LOG_FILE"
echo "========================================="
echo ""

# Display server configuration
for i in "${!SERVER_CONFIGS[@]}"; do
    echo "Server ${SERVER_IDS[$i]}: localhost"
    echo "  Config:   ${SERVER_CONFIGS[$i]}"
    echo "  Work Dir: ${SERVER_WORK_DIRS[$i]}"
    echo "  Binary:   ${SERVER_BINARIES[$i]}"
done

echo ""
echo "========================================="
echo ""

# Check if servers are running
log_event "Checking if servers are running..."
log_event "Debug: Looking for server processes with config files:"
for i in "${!SERVER_CONFIGS[@]}"; do
    log_event "  - $(basename ${SERVER_CONFIGS[$i]})"
done

ALL_RUNNING=true
for i in "${!SERVER_CONFIGS[@]}"; do
    pid=$(get_server_pid "${SERVER_CONFIGS[$i]}")
    if [ -n "$pid" ]; then
        log_event "Server ${SERVER_IDS[$i]} is running (PID $pid)"
    else
        config_file=$(basename "${SERVER_CONFIGS[$i]}")
        log_event "WARNING: Server ${SERVER_IDS[$i]} is not running"
        log_event "  (Searched for: server.*$config_file)"
        log_event "  Running servers: $(pgrep -f 'server' | tr '\n' ' ')"
        ALL_RUNNING=false
    fi
done

if [ "$ALL_RUNNING" = false ]; then
    log_event "WARNING: Not all servers are running. Continuing anyway..."
fi

echo ""

# Main fault simulation loop
log_event "Starting fault simulation with ring algorithm"
log_event "Ring order: Server ${SERVER_IDS[*]}"

START_TIME=$(date +%s)

for ((cycle=1; cycle<=NUM_CYCLES; cycle++)); do
    log_event "===== CYCLE $cycle/$NUM_CYCLES ====="

    # Iterate through servers in ring order
    for i in "${!SERVER_CONFIGS[@]}"; do
        SERVER_ID="${SERVER_IDS[$i]}"
        SERVER_CONFIG="${SERVER_CONFIGS[$i]}"
        SERVER_WORK_DIR="${SERVER_WORK_DIRS[$i]}"
        SERVER_BINARY="${SERVER_BINARIES[$i]}"

        log_event "Ring Algorithm: Processing Server $SERVER_ID"

        # Step 1: Kill server
        if kill_server "$SERVER_CONFIG" "$SERVER_ID"; then
            log_event "Server $SERVER_ID is now DOWN"
        else
            log_event "WARNING: Server $SERVER_ID kill operation had issues"
        fi

        # Step 2: Wait for restart delay
        log_event "Waiting ${RESTART_DELAY_SECS}s before restarting Server $SERVER_ID..."
        sleep "$RESTART_DELAY_SECS"

        # Step 3: Restart server
        if restart_server "$SERVER_CONFIG" "$SERVER_WORK_DIR" "$SERVER_BINARY" "$SERVER_ID"; then
            log_event "Server $SERVER_ID is now UP"
        else
            log_event "WARNING: Server $SERVER_ID restart operation had issues"
        fi

        # Step 4: Wait for fault interval before moving to next server
        log_event "Waiting ${FAULT_INTERVAL_SECS}s before moving to next server in ring..."
        sleep "$FAULT_INTERVAL_SECS"
    done

    log_event "===== CYCLE $cycle COMPLETED ====="
    echo ""
done

END_TIME=$(date +%s)
TOTAL_TIME=$((END_TIME - START_TIME))

log_event "========================================="
log_event "Fault Simulation Completed!"
log_event "========================================="
log_event "Total Cycles:       $NUM_CYCLES"
log_event "Servers Processed:  $NUM_SERVERS"
log_event "Total Faults:       $((NUM_CYCLES * NUM_SERVERS))"
log_event "Total Time:         ${TOTAL_TIME}s"
log_event "========================================="

echo ""
echo "Fault simulation completed successfully."
echo "Log file: $LOG_FILE"
echo ""
echo "To view the log:"
echo "  cat $LOG_FILE"
