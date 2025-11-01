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
#   - start_server.sh and stop_server.sh scripts deployed on server machines

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
declare -a SERVER_WORK_DIRS
declare -a SERVER_IDS

# Parse server configurations
for i in {1..10}; do
    HOST_VAR="SERVER_${i}_HOST"
    WORK_DIR_VAR="SERVER_${i}_WORK_DIR"

    HOST="${!HOST_VAR}"
    WORK_DIR="${!WORK_DIR_VAR}"

    if [ -n "$HOST" ]; then
        SERVER_HOSTS+=("$HOST")
        SERVER_WORK_DIRS+=("${WORK_DIR:-/root/CloudP2P}")
        SERVER_IDS+=("$i")
    fi
done

NUM_SERVERS=${#SERVER_HOSTS[@]}

if [ "$NUM_SERVERS" -eq 0 ]; then
    echo "Error: No servers configured"
    echo "Please configure at least SERVER_1_HOST and SERVER_1_WORK_DIR"
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

# Function to stop server via SSH using stop_server.sh script
stop_server() {
    local host="$1"
    local work_dir="$2"
    local server_id="$3"

    log_event "Stopping Server $server_id on $host using stop_server.sh"

    # Check if stop script exists
    local script_exists=$(ssh "$host" "test -f $work_dir/scripts/stop_server.sh && echo 'yes' || echo 'no'")

    if [ "$script_exists" != "yes" ]; then
        log_event "ERROR: stop_server.sh not found at $work_dir/scripts/stop_server.sh on $host"
        return 1
    fi

    # Run the stop script via SSH (exactly as if you ran it locally)
    local output=$(ssh "$host" "cd $work_dir && ./scripts/stop_server.sh $server_id 2>&1")
    local exit_code=$?

    # Log the output
    echo "$output" | while IFS= read -r line; do
        log_event "  [Server $server_id] $line"
    done

    if [ $exit_code -eq 0 ]; then
        log_event "SUCCESS: Server $server_id stopped on $host"
        return 0
    else
        log_event "ERROR: Failed to stop Server $server_id on $host (exit code: $exit_code)"
        return 1
    fi
}

# Function to start server via SSH using start_server.sh script
start_server() {
    local host="$1"
    local work_dir="$2"
    local server_id="$3"

    log_event "Starting Server $server_id on $host using start_server.sh"

    # Check if start script exists
    local script_exists=$(ssh "$host" "test -f $work_dir/scripts/start_server.sh && echo 'yes' || echo 'no'")

    if [ "$script_exists" != "yes" ]; then
        log_event "ERROR: start_server.sh not found at $work_dir/scripts/start_server.sh on $host"
        return 1
    fi

    # Run the start script via SSH (exactly as if you ran it locally)
    local output=$(ssh "$host" "cd $work_dir && ./scripts/start_server.sh $server_id 2>&1")
    local exit_code=$?

    # Log the output
    echo "$output" | while IFS= read -r line; do
        log_event "  [Server $server_id] $line"
    done

    if [ $exit_code -eq 0 ]; then
        log_event "SUCCESS: Server $server_id started on $host"
        return 0
    else
        log_event "ERROR: Failed to start Server $server_id on $host (exit code: $exit_code)"
        return 1
    fi
}

# Function to check server status via SSH using status_server.sh script
check_server_status() {
    local host="$1"
    local work_dir="$2"
    local server_id="$3"

    # Check if status script exists
    local script_exists=$(ssh "$host" "test -f $work_dir/scripts/status_server.sh && echo 'yes' || echo 'no'")

    if [ "$script_exists" = "yes" ]; then
        ssh "$host" "cd $work_dir && ./scripts/status_server.sh $server_id 2>&1" || true
    else
        echo "Status script not found"
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
    echo "  Work Dir: ${SERVER_WORK_DIRS[$i]}"
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

# Verify management scripts exist on all servers
log_event "Verifying management scripts exist on all servers..."
SCRIPTS_OK=true
for i in "${!SERVER_HOSTS[@]}"; do
    SERVER_ID="${SERVER_IDS[$i]}"
    SERVER_HOST="${SERVER_HOSTS[$i]}"
    SERVER_WORK_DIR="${SERVER_WORK_DIRS[$i]}"

    echo "Checking Server ${SERVER_ID} (${SERVER_HOST})..."

    start_exists=$(ssh "$SERVER_HOST" "test -f $SERVER_WORK_DIR/scripts/start_server.sh && echo 'yes' || echo 'no'")
    stop_exists=$(ssh "$SERVER_HOST" "test -f $SERVER_WORK_DIR/scripts/stop_server.sh && echo 'yes' || echo 'no'")

    if [ "$start_exists" != "yes" ] || [ "$stop_exists" != "yes" ]; then
        log_event "ERROR: Management scripts not found on Server $SERVER_ID"
        echo "  start_server.sh: $start_exists"
        echo "  stop_server.sh: $stop_exists"
        SCRIPTS_OK=false
    else
        log_event "Management scripts found on Server $SERVER_ID"
    fi
done

if [ "$SCRIPTS_OK" = false ]; then
    log_event "ERROR: Management scripts missing on one or more servers"
    echo ""
    echo "Please deploy scripts/start_server.sh and scripts/stop_server.sh to all server machines"
    exit 1
fi

log_event "All management scripts verified"
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
        SERVER_WORK_DIR="${SERVER_WORK_DIRS[$i]}"

        log_event "Ring Algorithm: Processing Server $SERVER_ID"

        # Step 1: Stop server using stop_server.sh
        if stop_server "$SERVER_HOST" "$SERVER_WORK_DIR" "$SERVER_ID"; then
            log_event "Server $SERVER_ID is now DOWN"
        else
            log_event "WARNING: Server $SERVER_ID stop operation had issues"
        fi

        # Step 2: Wait for restart delay (server stays down)
        log_event "Waiting ${RESTART_DELAY_SECS}s before restarting Server $SERVER_ID..."
        sleep "$RESTART_DELAY_SECS"

        # Step 3: Start server using start_server.sh
        if start_server "$SERVER_HOST" "$SERVER_WORK_DIR" "$SERVER_ID"; then
            log_event "Server $SERVER_ID is now UP"
        else
            log_event "WARNING: Server $SERVER_ID start operation had issues"
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
