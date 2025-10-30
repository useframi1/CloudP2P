#!/bin/bash

# CloudP2P Distributed Multi-Machine Integration Test
# This script orchestrates tests across multiple physical machines via SSH

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# ============================================================================
# CONFIGURATION - EDIT THESE FOR YOUR MACHINES
# ============================================================================

# Server machines configuration
# Format: "username@ip:project_path"
declare -A SERVER_MACHINES=(
    [1]="user@192.168.1.10:/home/user/CloudP2P"
    [2]="user@192.168.1.11:/home/user/CloudP2P"
    [3]="user@192.168.1.12:/home/user/CloudP2P"
)

# Client machines configuration
declare -A CLIENT_MACHINES=(
    [1]="user@192.168.1.20:/home/user/CloudP2P"
    [2]="user@192.168.1.21:/home/user/CloudP2P"
    [3]="user@192.168.1.22:/home/user/CloudP2P"
)

# SSH key path (leave empty to use default)
SSH_KEY=""

# Local project directory (for generating configs)
PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# ============================================================================
# UTILITY FUNCTIONS
# ============================================================================

print_header() {
    echo ""
    echo -e "${BLUE}========================================${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}========================================${NC}"
    echo ""
}

print_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

print_error() {
    echo -e "${RED}✗ $1${NC}"
}

print_info() {
    echo -e "${BLUE}ℹ $1${NC}"
}

# SSH wrapper function
run_ssh() {
    local host=$1
    local command=$2

    if [ -n "$SSH_KEY" ]; then
        ssh -i "$SSH_KEY" -o StrictHostKeyChecking=no "$host" "$command"
    else
        ssh -o StrictHostKeyChecking=no "$host" "$command"
    fi
}

# SCP wrapper function
run_scp() {
    local src=$1
    local dest=$2

    if [ -n "$SSH_KEY" ]; then
        scp -i "$SSH_KEY" -o StrictHostKeyChecking=no "$src" "$dest"
    else
        scp -o StrictHostKeyChecking=no "$src" "$dest"
    fi
}

# ============================================================================
# MACHINE MANAGEMENT
# ============================================================================

deploy_to_server() {
    local server_id=$1
    local machine_info="${SERVER_MACHINES[$server_id]}"
    local host="${machine_info%%:*}"
    local path="${machine_info##*:}"

    print_info "Deploying to Server $server_id ($host)..."

    # Create directories
    run_ssh "$host" "mkdir -p $path/user-data/uploads $path/user-data/outputs $path/logs"

    # Copy config file
    run_scp "$PROJECT_DIR/config/server${server_id}.toml" "$host:$path/config/server${server_id}.toml"

    # Build on remote (assumes code is already there)
    run_ssh "$host" "cd $path && cargo build --release"

    print_success "Server $server_id deployed"
}

deploy_to_client() {
    local client_id=$1
    local machine_info="${CLIENT_MACHINES[$client_id]}"
    local host="${machine_info%%:*}"
    local path="${machine_info##*:}"

    print_info "Deploying to Client $client_id ($host)..."

    # Create directories
    run_ssh "$host" "mkdir -p $path/user-data/uploads $path/user-data/outputs $path/logs"

    # Copy config file
    run_scp "$PROJECT_DIR/config/client${client_id}.toml" "$host:$path/config/client${client_id}.toml"

    # Copy test image
    if [ -f "$PROJECT_DIR/user-data/uploads/test_image.jpg" ]; then
        run_scp "$PROJECT_DIR/user-data/uploads/test_image.jpg" "$host:$path/user-data/uploads/"
    fi

    # Build on remote (assumes code is already there)
    run_ssh "$host" "cd $path && cargo build --release"

    print_success "Client $client_id deployed"
}

start_server() {
    local server_id=$1
    local machine_info="${SERVER_MACHINES[$server_id]}"
    local host="${machine_info%%:*}"
    local path="${machine_info##*:}"

    print_info "Starting Server $server_id on $host..."

    # Start server in background, redirect output to log file
    run_ssh "$host" "cd $path && nohup ./target/release/server -c config/server${server_id}.toml > logs/server${server_id}.log 2>&1 &"

    sleep 2

    # Verify it's running
    if run_ssh "$host" "pgrep -f 'server -c config/server${server_id}.toml'" > /dev/null; then
        print_success "Server $server_id started"
        return 0
    else
        print_error "Server $server_id failed to start"
        return 1
    fi
}

start_client() {
    local client_id=$1
    local machine_info="${CLIENT_MACHINES[$client_id]}"
    local host="${machine_info%%:*}"
    local path="${machine_info##*:}"

    print_info "Starting Client $client_id on $host..."

    # Start client in background
    run_ssh "$host" "cd $path && nohup ./target/release/client -c config/client${client_id}.toml > logs/client${client_id}.log 2>&1 &"

    print_success "Client $client_id started"
}

stop_server() {
    local server_id=$1
    local machine_info="${SERVER_MACHINES[$server_id]}"
    local host="${machine_info%%:*}"
    local path="${machine_info##*:}"

    print_info "Stopping Server $server_id on $host..."

    run_ssh "$host" "pkill -f 'server -c config/server${server_id}.toml' || true"

    print_success "Server $server_id stopped"
}

stop_client() {
    local client_id=$1
    local machine_info="${CLIENT_MACHINES[$client_id]}"
    local host="${machine_info%%:*}"
    local path="${machine_info##*:}"

    print_info "Stopping Client $client_id on $host..."

    run_ssh "$host" "pkill -f 'client -c config/client${client_id}.toml' || true"

    print_success "Client $client_id stopped"
}

stop_all() {
    print_info "Stopping all servers and clients..."

    for id in "${!SERVER_MACHINES[@]}"; do
        stop_server "$id" 2>/dev/null || true
    done

    for id in "${!CLIENT_MACHINES[@]}"; do
        stop_client "$id" 2>/dev/null || true
    done
}

# ============================================================================
# LOG COLLECTION AND VERIFICATION
# ============================================================================

collect_logs() {
    local dest_dir="$PROJECT_DIR/test_results/distributed_logs"
    mkdir -p "$dest_dir"

    print_info "Collecting logs from all machines..."

    # Collect server logs
    for id in "${!SERVER_MACHINES[@]}"; do
        local machine_info="${SERVER_MACHINES[$id]}"
        local host="${machine_info%%:*}"
        local path="${machine_info##*:}"

        run_scp "$host:$path/logs/server${id}.log" "$dest_dir/" 2>/dev/null || \
            print_error "Could not collect logs from Server $id"
    done

    # Collect client logs
    for id in "${!CLIENT_MACHINES[@]}"; do
        local machine_info="${CLIENT_MACHINES[$id]}"
        local host="${machine_info%%:*}"
        local path="${machine_info##*:}"

        run_scp "$host:$path/logs/client${id}.log" "$dest_dir/" 2>/dev/null || \
            print_error "Could not collect logs from Client $id"
    done

    print_success "Logs collected in $dest_dir"
}

check_leader_elected() {
    local dest_dir="$PROJECT_DIR/test_results/distributed_logs"

    for id in "${!SERVER_MACHINES[@]}"; do
        if grep -q "won election" "$dest_dir/server${id}.log" 2>/dev/null; then
            print_success "Leader election completed (Server $id is leader)"
            return 0
        fi
    done

    print_error "No leader elected"
    return 1
}

check_tasks_completed() {
    local dest_dir="$PROJECT_DIR/test_results/distributed_logs"
    local total_completed=0

    for id in "${!CLIENT_MACHINES[@]}"; do
        local completed=$(grep -c "completed successfully" "$dest_dir/client${id}.log" 2>/dev/null || echo "0")
        total_completed=$((total_completed + completed))
        print_info "Client $id: $completed tasks completed"
    done

    if [ "$total_completed" -gt 0 ]; then
        print_success "Total tasks completed: $total_completed"
        return 0
    else
        print_error "No tasks completed"
        return 1
    fi
}

# ============================================================================
# TEST SCENARIOS
# ============================================================================

test_basic_distributed() {
    print_header "Test: Basic Distributed Operation"

    # Start all servers
    for id in "${!SERVER_MACHINES[@]}"; do
        start_server "$id"
    done

    # Wait for leader election
    print_info "Waiting for leader election..."
    sleep 10

    # Start all clients
    for id in "${!CLIENT_MACHINES[@]}"; do
        start_client "$id"
    done

    # Let clients run for a while
    print_info "Running test for 60 seconds..."
    sleep 60

    # Stop everything
    stop_all
    sleep 5

    # Collect and verify logs
    collect_logs

    local success=0
    if check_leader_elected; then
        ((success++))
    fi

    if check_tasks_completed; then
        ((success++))
    fi

    if [ $success -eq 2 ]; then
        print_success "TEST PASSED: Basic Distributed Operation"
        return 0
    else
        print_error "TEST FAILED: Basic Distributed Operation"
        return 1
    fi
}

test_leader_failure_distributed() {
    print_header "Test: Distributed Leader Failure"

    # Start all servers
    for id in "${!SERVER_MACHINES[@]}"; do
        start_server "$id"
    done

    sleep 10

    # Start clients
    for id in "${!CLIENT_MACHINES[@]}"; do
        start_client "$id"
    done

    sleep 15

    # Collect logs to find leader
    collect_logs

    local leader_id=""
    for id in "${!SERVER_MACHINES[@]}"; do
        if grep -q "won election" "$PROJECT_DIR/test_results/distributed_logs/server${id}.log" 2>/dev/null; then
            leader_id=$id
            break
        fi
    done

    if [ -n "$leader_id" ]; then
        print_info "Killing leader (Server $leader_id)..."
        stop_server "$leader_id"

        # Wait for re-election
        sleep 15
    fi

    # Let system continue
    sleep 30

    # Stop everything
    stop_all
    sleep 5

    # Collect final logs
    collect_logs

    # Check for re-election
    if check_leader_elected && check_tasks_completed; then
        print_success "TEST PASSED: Distributed Leader Failure"
        return 0
    else
        print_error "TEST FAILED: Distributed Leader Failure"
        return 1
    fi
}

# ============================================================================
# MAIN EXECUTION
# ============================================================================

main() {
    print_header "CloudP2P Distributed Multi-Machine Test"

    # Verify SSH connectivity
    print_info "Verifying SSH connectivity to all machines..."

    for id in "${!SERVER_MACHINES[@]}"; do
        local host="${SERVER_MACHINES[$id]%%:*}"
        if ! run_ssh "$host" "echo 'Connected'" > /dev/null 2>&1; then
            print_error "Cannot connect to Server $id ($host)"
            exit 1
        fi
        print_success "Connected to Server $id ($host)"
    done

    for id in "${!CLIENT_MACHINES[@]}"; do
        local host="${CLIENT_MACHINES[$id]%%:*}"
        if ! run_ssh "$host" "echo 'Connected'" > /dev/null 2>&1; then
            print_error "Cannot connect to Client $id ($host)"
            exit 1
        fi
        print_success "Connected to Client $id ($host)"
    done

    # Deploy to all machines
    print_header "Deploying to All Machines"

    for id in "${!SERVER_MACHINES[@]}"; do
        deploy_to_server "$id"
    done

    for id in "${!CLIENT_MACHINES[@]}"; do
        deploy_to_client "$id"
    done

    # Run tests
    print_header "Running Distributed Tests"

    test_basic_distributed

    # Uncomment to run more tests
    # test_leader_failure_distributed

    print_header "Test Complete"
    print_info "Logs available in: $PROJECT_DIR/test_results/distributed_logs"
}

# Cleanup on exit
trap stop_all EXIT

# Parse command line arguments
case "${1:-}" in
    deploy)
        print_header "Deploying to All Machines"
        for id in "${!SERVER_MACHINES[@]}"; do
            deploy_to_server "$id"
        done
        for id in "${!CLIENT_MACHINES[@]}"; do
            deploy_to_client "$id"
        done
        ;;
    start)
        print_header "Starting All Servers and Clients"
        for id in "${!SERVER_MACHINES[@]}"; do
            start_server "$id"
        done
        sleep 10
        for id in "${!CLIENT_MACHINES[@]}"; do
            start_client "$id"
        done
        ;;
    stop)
        stop_all
        ;;
    logs)
        collect_logs
        ;;
    test)
        main
        ;;
    *)
        echo "Usage: $0 {deploy|start|stop|logs|test}"
        echo ""
        echo "Commands:"
        echo "  deploy  - Deploy code and configs to all machines"
        echo "  start   - Start all servers and clients"
        echo "  stop    - Stop all servers and clients"
        echo "  logs    - Collect logs from all machines"
        echo "  test    - Run full distributed test suite"
        exit 1
        ;;
esac
