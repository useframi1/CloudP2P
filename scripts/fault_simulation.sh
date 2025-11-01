#!/bin/bash

# CloudP2P Fault Simulation Script
#
# Implements a ring algorithm to sequentially kill and restart servers
# to simulate fault conditions during stress testing.
#
# Usage:
#   ./scripts/fault_simulation.sh [config_file]
#
# Default config: ./scripts/config/fault_sim.conf
#
# Prerequisites:
#   - SSH key-based authentication configured for all server hosts
#   - Server binary and config files deployed on server machines

set -e

# Default configuration file
CONFIG_FILE="${1:-./scripts/config/fault_sim.conf}"

# Check if config file exists
if [ ! -f "$CONFIG_FILE" ]; then
    echo "Error: Configuration file not found: $CONFIG_FILE"
    echo "Usage: $0 [config_file]"
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
declare -a SERVER_HOSTS
declare -a SERVER_CONFIGS
declare -a SERVER_WORK_DIRS
declare -a SERVER_BINARIES
declare -a SERVER_IDS

# Parse server configurations
for i in {1..10}; do
    HOST_VAR="SERVER_${i}_HOST"
    CONFIG_VAR="SERVER_${i}_CONFIG"
    WORK_DIR_VAR="SERVER_${i}_WORK_DIR"
    BINARY_VAR="SERVER_${i}_BINARY"

    HOST="${!HOST_VAR}"
    CONFIG="${!CONFIG_VAR}"
    WORK_DIR="${!WORK_DIR_VAR}"
    BINARY="${!BINARY_VAR}"

    if [ -n "$HOST" ]; then
        SERVER_HOSTS+=("$HOST")
        SERVER_CONFIGS+=("$CONFIG")
        SERVER_WORK_DIRS+=("${WORK_DIR:-/root/CloudP2P}")
        SERVER_BINARIES+=("${BINARY:-./target/release/server}")
        SERVER_IDS+=("$i")
    fi
done

NUM_SERVERS=${#SERVER_HOSTS[@]}

if [ "$NUM_SERVERS" -eq 0 ]; then
    echo "Error: No servers configured"
    echo "Please configure at least SERVER_1_HOST and SERVER_1_CONFIG"
    exit 1
fi

# Log file for fault events
LOG_FILE="${LOG_FILE:-./fault_events.log}"
mkdir -p "$(dirname "$LOG_FILE")"

# Function to log with timestamp
log_event() {
    local message="$1"
    local timestamp=$(date '+%Y-%m-%d %H:%M:%S')
    echo "[$timestamp] $message" | tee -a "$LOG_FILE"
}

# Function to get server PID via SSH
get_server_pid() {
    local host="$1"
    local config_path="$2"

    ssh -o ConnectTimeout=5 "$host" "pgrep -f 'server --config $config_path' | head -1" 2>/dev/null || echo ""
}

# Function to kill server via SSH
kill_server() {
    local host="$1"
    local config_path="$2"
    local server_id="$3"

    log_event "Attempting to kill Server $server_id on $host"

    local pid=$(get_server_pid "$host" "$config_path")

    if [ -z "$pid" ]; then
        log_event "WARNING: Server $server_id not running on $host (no PID found)"
        return 1
    fi

    log_event "Found Server $server_id PID: $pid on $host"

    # Kill the server process
    if ssh "$host" "kill -9 $pid" 2>/dev/null; then
        log_event "SUCCESS: Killed Server $server_id (PID $pid) on $host"
        return 0
    else
        log_event "ERROR: Failed to kill Server $server_id on $host"
        return 1
    fi
}

# Function to restart server via SSH
restart_server() {
    local host="$1"
    local config_path="$2"
    local work_dir="$3"
    local binary="$4"
    local server_id="$5"

    log_event "Attempting to restart Server $server_id on $host"

    # Check if server is already running
    local pid=$(get_server_pid "$host" "$config_path")
    if [ -n "$pid" ]; then
        log_event "WARNING: Server $server_id already running on $host (PID $pid)"
        return 0
    fi

    # Start server in background using nohup
    if ssh "$host" "cd $work_dir && nohup $binary --config $config_path > /dev/null 2>&1 &"; then
        sleep 2  # Give server time to start

        # Verify server started
        pid=$(get_server_pid "$host" "$config_path")
        if [ -n "$pid" ]; then
            log_event "SUCCESS: Restarted Server $server_id on $host (PID $pid)"
            return 0
        else
            log_event "ERROR: Server $server_id failed to start on $host"
            return 1
        fi
    else
        log_event "ERROR: Failed to execute restart command for Server $server_id on $host"
        return 1
    fi
}

# Function to check SSH connectivity
check_ssh_connectivity() {
    local host="$1"
    local server_id="$2"

    if ssh "$host" "echo 'SSH OK'" > /dev/null 2>&1; then
        log_event "SSH connectivity OK for Server $server_id ($host)"
        return 0
    else
        log_event "ERROR: Cannot connect to Server $server_id ($host) via SSH"
        return 1
    fi
}

echo "========================================="
echo "CloudP2P Fault Simulation - Ring Algorithm"
echo "========================================="
echo "Number of Servers:   $NUM_SERVERS"
echo "Fault Interval:      ${FAULT_INTERVAL_SECS}s"
echo "Restart Delay:       ${RESTART_DELAY_SECS}s"
echo "Number of Cycles:    $NUM_CYCLES"
echo "Log File:            $LOG_FILE"
echo "========================================="
echo ""

# Display server configuration
for i in "${!SERVER_HOSTS[@]}"; do
    echo "Server ${SERVER_IDS[$i]}: ${SERVER_HOSTS[$i]}"
    echo "  Config:   ${SERVER_CONFIGS[$i]}"
    echo "  Work Dir: ${SERVER_WORK_DIRS[$i]}"
    echo "  Binary:   ${SERVER_BINARIES[$i]}"
done

echo ""
echo "========================================="
echo ""

# Check SSH connectivity to all servers
log_event "Checking SSH connectivity to all servers..."
ALL_SSH_OK=true
for i in "${!SERVER_HOSTS[@]}"; do
    echo "Checking Server ${SERVER_IDS[$i]} (${SERVER_HOSTS[$i]})..."
    if ! check_ssh_connectivity "${SERVER_HOSTS[$i]}" "${SERVER_IDS[$i]}"; then
        ALL_SSH_OK=false
    fi
done

if [ "$ALL_SSH_OK" = false ]; then
    log_event "ERROR: SSH connectivity check failed for one or more servers"
    echo ""
    echo "Please ensure:"
    echo "  1. SSH key-based authentication is configured"
    echo "  2. All server hosts are reachable"
    echo "  3. SSH keys are added to ssh-agent (ssh-add)"
    exit 1
fi

log_event "All SSH connectivity checks passed"
echo ""

# Main fault simulation loop
log_event "Starting fault simulation with ring algorithm"
log_event "Ring order: Server ${SERVER_IDS[*]}"

START_TIME=$(date +%s)

for ((cycle=1; cycle<=NUM_CYCLES; cycle++)); do
    log_event "===== CYCLE $cycle/$NUM_CYCLES ====="

    # Iterate through servers in ring order
    for i in "${!SERVER_HOSTS[@]}"; do
        SERVER_ID="${SERVER_IDS[$i]}"
        SERVER_HOST="${SERVER_HOSTS[$i]}"
        SERVER_CONFIG="${SERVER_CONFIGS[$i]}"
        SERVER_WORK_DIR="${SERVER_WORK_DIRS[$i]}"
        SERVER_BINARY="${SERVER_BINARIES[$i]}"

        log_event "Ring Algorithm: Processing Server $SERVER_ID"

        # Step 1: Kill server
        if kill_server "$SERVER_HOST" "$SERVER_CONFIG" "$SERVER_ID"; then
            log_event "Server $SERVER_ID is now DOWN"
        else
            log_event "WARNING: Server $SERVER_ID kill operation had issues"
        fi

        # Step 2: Wait for restart delay
        log_event "Waiting ${RESTART_DELAY_SECS}s before restarting Server $SERVER_ID..."
        sleep "$RESTART_DELAY_SECS"

        # Step 3: Restart server
        if restart_server "$SERVER_HOST" "$SERVER_CONFIG" "$SERVER_WORK_DIR" "$SERVER_BINARY" "$SERVER_ID"; then
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
