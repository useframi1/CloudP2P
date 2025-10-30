#!/bin/bash

# CloudP2P Distributed System Integration Tests
# Tests fault tolerance, leader election, and system reliability

set -e  # Exit on error (except in test functions)

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test tracking
TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0
FAILED_TESTS=()

# Project paths
PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEST_CONFIG_DIR="$PROJECT_DIR/config/test"
TEST_OUTPUT_DIR="$PROJECT_DIR/test_results"
LOGS_DIR="$TEST_OUTPUT_DIR/logs"

# Server/Client PIDs
declare -a SERVER_PIDS
declare -a CLIENT_PIDS

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

print_test() {
    echo -e "${YELLOW}[TEST $1]${NC} $2"
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

# ============================================================================
# SETUP AND TEARDOWN
# ============================================================================

setup_test_env() {
    print_info "Setting up test environment..."

    # Create test output directories
    rm -rf "$TEST_OUTPUT_DIR"
    mkdir -p "$LOGS_DIR"
    mkdir -p "$PROJECT_DIR/user-data/uploads"
    mkdir -p "$PROJECT_DIR/user-data/outputs"

    # Check if test image exists, create if needed
    if [ ! -f "$PROJECT_DIR/user-data/uploads/test_image.jpg" ]; then
        print_info "Creating test image..."
        # Create a simple test image using ImageMagick if available
        if command -v convert &> /dev/null; then
            convert -size 800x600 xc:blue "$PROJECT_DIR/user-data/uploads/test_image.jpg"
            print_success "Test image created"
        else
            print_error "ImageMagick not found. Please place a test_image.jpg in user-data/uploads/"
            print_info "You can download any JPG image and place it at: user-data/uploads/test_image.jpg"
            exit 1
        fi
    else
        print_success "Test image found ($(du -h "$PROJECT_DIR/user-data/uploads/test_image.jpg" | cut -f1))"
    fi

    # Build the project
    print_info "Building project..."
    cd "$PROJECT_DIR"
    cargo build --release 2>&1 | tee "$LOGS_DIR/build.log"

    if [ ${PIPESTATUS[0]} -ne 0 ]; then
        print_error "Build failed! Check $LOGS_DIR/build.log"
        exit 1
    fi

    print_success "Test environment ready"
}

cleanup() {
    print_info "Cleaning up..."
    stop_all_servers
    stop_all_clients

    # Aggressively kill any remaining server/client processes
    pkill -9 -f "target/release/server" 2>/dev/null || true
    pkill -9 -f "target/release/client" 2>/dev/null || true

    # Wait longer for TCP ports to be released
    sleep 3
}

# ============================================================================
# SERVER/CLIENT MANAGEMENT
# ============================================================================

start_server() {
    local server_id=$1
    local config_file="$TEST_CONFIG_DIR/server${server_id}.toml"
    local log_file="$LOGS_DIR/server${server_id}.log"

    print_info "Starting Server $server_id..."
    (cd "$PROJECT_DIR" && "$PROJECT_DIR/target/release/server" -c "$config_file" > "$log_file" 2>&1) &
    local pid=$!
    SERVER_PIDS[$server_id]=$pid

    # Wait a moment for server to start
    sleep 3

    if ps -p $pid > /dev/null; then
        print_success "Server $server_id started (PID: $pid)"
        return 0
    else
        print_error "Server $server_id failed to start"
        return 1
    fi
}

stop_server() {
    local server_id=$1
    local pid=${SERVER_PIDS[$server_id]}

    if [ -n "$pid" ] && ps -p $pid > /dev/null; then
        print_info "Stopping Server $server_id (PID: $pid)..."
        kill $pid 2>/dev/null || true
        sleep 1

        # Force kill if still running
        if ps -p $pid > /dev/null; then
            kill -9 $pid 2>/dev/null || true
        fi

        print_success "Server $server_id stopped"
    fi

    SERVER_PIDS[$server_id]=""
}

start_all_servers() {
    print_info "Starting all servers..."
    for i in 1 2 3; do
        start_server $i
    done

    # Wait for leader election
    print_info "Waiting for leader election..."
    sleep 5
}

stop_all_servers() {
    for i in 1 2 3; do
        stop_server $i
    done
    # Give extra time for ports to be released
    sleep 2
}

start_client() {
    local client_name=$1
    local config_file=$2
    local log_file="$LOGS_DIR/${client_name}.log"

    print_info "Starting $client_name..."
    (cd "$PROJECT_DIR" && "$PROJECT_DIR/target/release/client" -c "$config_file" > "$log_file" 2>&1) &
    local pid=$!
    CLIENT_PIDS+=($pid)

    print_success "$client_name started (PID: $pid)"
}

stop_all_clients() {
    for pid in "${CLIENT_PIDS[@]}"; do
        if [ -n "$pid" ] && ps -p $pid > /dev/null; then
            kill $pid 2>/dev/null || true
        fi
    done
    CLIENT_PIDS=()
}

wait_for_clients() {
    print_info "Waiting for clients to complete..."
    for pid in "${CLIENT_PIDS[@]}"; do
        if [ -n "$pid" ]; then
            wait $pid 2>/dev/null || true
        fi
    done
    CLIENT_PIDS=()
}

# ============================================================================
# TEST VERIFICATION FUNCTIONS
# ============================================================================

check_leader_elected() {
    print_info "Checking if a leader was elected..."

    for i in 1 2 3; do
        local log_file="$LOGS_DIR/server${i}.log"
        if grep -q "won election" "$log_file" || grep -q "acknowledges .* as LEADER" "$log_file"; then
            print_success "Leader election completed"
            return 0
        fi
    done

    print_error "No leader elected"
    return 1
}

check_server_running() {
    local server_id=$1
    local pid=${SERVER_PIDS[$server_id]}

    if [ -n "$pid" ] && ps -p $pid > /dev/null; then
        return 0
    else
        return 1
    fi
}

check_tasks_completed() {
    local client_name=$1
    local expected_count=$2
    local log_file="$LOGS_DIR/${client_name}.log"

    local completed=$(grep -c "completed successfully" "$log_file" 2>/dev/null || echo "0")

    if [ "$completed" -ge "$expected_count" ]; then
        print_success "$client_name: $completed/$expected_count tasks completed"
        return 0
    else
        print_error "$client_name: Only $completed/$expected_count tasks completed"
        return 1
    fi
}

check_encrypted_files() {
    local count=$(ls -1 "$PROJECT_DIR/user-data/outputs/encrypted_"* 2>/dev/null | wc -l)

    if [ "$count" -gt 0 ]; then
        print_success "Found $count encrypted output files"
        return 0
    else
        print_error "No encrypted output files found"
        return 1
    fi
}

check_no_errors() {
    local log_pattern=$1
    local description=$2

    if grep -r "FAILED after" $LOGS_DIR/$log_pattern 2>/dev/null; then
        print_error "$description: Found task failures"
        return 1
    fi

    print_success "$description: No task failures"
    return 0
}

check_reelection() {
    print_info "Checking if re-election occurred..."

    for i in 1 2 3; do
        local log_file="$LOGS_DIR/server${i}.log"
        if grep -q "LEADER .* appears to have failed" "$log_file"; then
            print_success "Detected leader failure"

            # Check if new election happened
            if grep -q "initiating election" "$log_file" | tail -n 5; then
                print_success "Re-election initiated"
                return 0
            fi
        fi
    done

    print_error "No re-election detected"
    return 1
}

# ============================================================================
# TEST EXECUTION FRAMEWORK
# ============================================================================

run_test() {
    local test_name="$1"
    local test_function="$2"

    TESTS_RUN=$((TESTS_RUN + 1))
    print_test "$TESTS_RUN" "$test_name"

    # Clean up before test
    cleanup
    sleep 2

    # Run test
    if $test_function; then
        TESTS_PASSED=$((TESTS_PASSED + 1))
        print_success "TEST PASSED: $test_name"
    else
        TESTS_FAILED=$((TESTS_FAILED + 1))
        FAILED_TESTS+=("$test_name")
        print_error "TEST FAILED: $test_name"
    fi

    echo ""
}

# ============================================================================
# TEST CASES
# ============================================================================

test_basic_leader_election() {
    print_info "Test: Basic leader election with 3 servers"

    start_all_servers

    # Wait for election to complete
    sleep 5

    # Verify leader was elected
    if check_leader_elected; then
        cleanup
        return 0
    else
        cleanup
        return 1
    fi
}

test_basic_task_processing() {
    print_info "Test: Basic task processing with one client"

    start_all_servers
    sleep 5

    # Start one client with 5 requests
    start_client "TestClient1" "$TEST_CONFIG_DIR/client_test1.toml"

    # Wait for client to finish
    wait_for_clients

    # Verify tasks completed
    local success=0
    if check_tasks_completed "TestClient1" 3; then
        ((success++))
    fi

    if check_encrypted_files; then
        ((success++))
    fi

    if check_no_errors "TestClient1.log" "Client"; then
        ((success++))
    fi

    cleanup

    [ $success -eq 3 ]
}

test_concurrent_clients() {
    print_info "Test: Multiple concurrent clients"

    start_all_servers
    sleep 5

    # Start two clients simultaneously
    start_client "TestClient1" "$TEST_CONFIG_DIR/client_test1.toml"
    start_client "TestClient2" "$TEST_CONFIG_DIR/client_test2.toml"

    # Wait for both clients
    wait_for_clients

    # Verify both clients completed tasks
    local success=0
    if check_tasks_completed "TestClient1" 3; then
        ((success++))
    fi

    if check_tasks_completed "TestClient2" 3; then
        ((success++))
    fi

    if check_encrypted_files; then
        ((success++))
    fi

    cleanup

    [ $success -eq 3 ]
}

test_leader_failure() {
    print_info "Test: Leader failure and re-election"

    start_all_servers
    sleep 5

    # Identify the leader
    local leader_id=""
    for i in 1 2 3; do
        if grep -q "won election" "$LOGS_DIR/server${i}.log"; then
            leader_id=$i
            break
        fi
    done

    if [ -z "$leader_id" ]; then
        print_error "Could not identify leader"
        cleanup
        return 1
    fi

    print_info "Leader is Server $leader_id"

    # Start a client
    start_client "TestClient1" "$TEST_CONFIG_DIR/client_test1.toml" &
    local client_pid=$!

    # Wait a bit for some tasks to start
    sleep 3

    # Kill the leader
    print_info "Killing leader (Server $leader_id)..."
    stop_server $leader_id

    # Wait for client to finish (should retry and succeed)
    wait $client_pid 2>/dev/null || true

    # Verify re-election happened
    local success=0

    # Give time for re-election
    sleep 5

    if check_reelection; then
        ((success++))
    fi

    # Check if new leader was elected
    if check_leader_elected; then
        ((success++))
    fi

    # Check that client recovered and completed some tasks
    if check_tasks_completed "TestClient1" 1; then
        ((success++))
    fi

    cleanup

    [ $success -ge 2 ]
}

test_worker_server_failure() {
    print_info "Test: Non-leader server failure"

    start_all_servers
    sleep 5

    # Identify a non-leader server
    local leader_id=""
    for i in 1 2 3; do
        if grep -q "won election" "$LOGS_DIR/server${i}.log"; then
            leader_id=$i
            break
        fi
    done

    local worker_id
    if [ "$leader_id" = "1" ]; then
        worker_id=2
    else
        worker_id=1
    fi

    print_info "Killing worker Server $worker_id..."
    stop_server $worker_id

    # Start client after killing worker
    sleep 2
    start_client "TestClient1" "$TEST_CONFIG_DIR/client_test1.toml"

    # Wait for client
    wait_for_clients

    # System should continue working with remaining servers
    local success=0

    if check_tasks_completed "TestClient1" 2; then
        ((success++))
    fi

    # Leader should still be running
    if check_server_running $leader_id; then
        print_success "Leader still running"
        ((success++))
    fi

    cleanup

    [ $success -eq 2 ]
}

test_multiple_server_failures() {
    print_info "Test: Multiple server failures (only 1 server remains)"

    start_all_servers
    sleep 5

    # Kill two servers, leave one alive
    print_info "Killing Server 1 and Server 2..."
    stop_server 1
    stop_server 2

    sleep 3

    # Start client with only Server 3 available
    start_client "TestClient1" "$TEST_CONFIG_DIR/client_test1.toml"

    # Wait for client
    wait_for_clients

    # Verify at least some tasks completed
    local success=0

    if check_tasks_completed "TestClient1" 1; then
        ((success++))
    fi

    if check_server_running 3; then
        print_success "Server 3 still running"
        ((success++))
    fi

    cleanup

    [ $success -eq 2 ]
}

test_server_recovery() {
    print_info "Test: Server recovery after failure"

    start_all_servers
    sleep 5

    # Stop Server 2
    print_info "Stopping Server 2..."
    stop_server 2

    sleep 3

    # Restart Server 2
    print_info "Restarting Server 2..."
    start_server 2

    sleep 5

    # Verify Server 2 rejoined
    local success=0

    if check_server_running 2; then
        print_success "Server 2 is running"
        ((success++))
    fi

    # Check if it reconnected to peers (proves it rejoined the cluster)
    if grep -q "connected to peer" "$LOGS_DIR/server2.log"; then
        print_success "Server 2 reconnected to peers"
        ((success++))
    fi

    # Start a client to verify system works
    start_client "TestClient1" "$TEST_CONFIG_DIR/client_test1.toml"
    wait_for_clients

    if check_tasks_completed "TestClient1" 2; then
        ((success++))
    fi

    cleanup

    [ $success -eq 3 ]
}

test_rapid_leader_changes() {
    print_info "Test: Rapid leader election cycles"

    start_all_servers
    sleep 5

    # Start a long-running client
    start_client "TestClient1" "$TEST_CONFIG_DIR/client_test1.toml" &
    local client_pid=$!

    # Kill and restart leader twice
    for cycle in 1 2; do
        sleep 3

        # Find current leader
        local leader_id=""
        for i in 1 2 3; do
            if check_server_running $i && grep -q "won election\|acknowledges .* as LEADER" "$LOGS_DIR/server${i}.log" | tail -1 | grep -q "won election"; then
                leader_id=$i
                break
            fi
        done

        if [ -n "$leader_id" ]; then
            print_info "Cycle $cycle: Killing leader Server $leader_id"
            stop_server $leader_id
            sleep 6

            print_info "Cycle $cycle: Restarting Server $leader_id"
            start_server $leader_id
            sleep 4
        fi
    done

    # Wait for client to finish
    wait $client_pid 2>/dev/null || true

    # Check if client completed at least some tasks despite chaos
    local success=0

    if check_tasks_completed "TestClient1" 1; then
        ((success++))
    fi

    cleanup

    [ $success -ge 1 ]
}

test_high_concurrent_load() {
    print_info "Test: High concurrent load with stress client"

    start_all_servers
    sleep 5

    # Start stress client (higher request rate)
    start_client "StressClient" "$TEST_CONFIG_DIR/client_stress.toml"

    # Wait for client
    wait_for_clients

    # Verify tasks completed
    local success=0

    if check_tasks_completed "StressClient" 10; then
        ((success++))
    fi

    if check_encrypted_files; then
        ((success++))
    fi

    # Check all servers are still running
    local running_count=0
    for i in 1 2 3; do
        if check_server_running $i; then
            ((running_count++))
        fi
    done

    if [ $running_count -eq 3 ]; then
        print_success "All servers still running after stress test"
        ((success++))
    fi

    cleanup

    [ $success -eq 3 ]
}

test_client_retry_mechanism() {
    print_info "Test: Client retry mechanism"

    # Start only Server 3
    start_server 3
    sleep 3

    # Start client (will need to discover Server 3)
    start_client "TestClient1" "$TEST_CONFIG_DIR/client_test1.toml"

    # Wait a bit, then start other servers
    sleep 5
    start_server 1
    start_server 2

    # Wait for client
    wait_for_clients

    # Verify client completed tasks
    local success=0

    if check_tasks_completed "TestClient1" 2; then
        ((success++))
    fi

    # Check logs for retry attempts
    if grep -q "Retry attempt" "$LOGS_DIR/TestClient1.log"; then
        print_success "Client performed retries as expected"
        ((success++))
    fi

    cleanup

    [ $success -eq 2 ]
}

# ============================================================================
# MAIN TEST EXECUTION
# ============================================================================

main() {
    print_header "CloudP2P Distributed System - Integration Tests"

    # Setup
    setup_test_env

    # Run all tests
    print_header "Running Tests"

    run_test "Basic Leader Election" test_basic_leader_election
    run_test "Basic Task Processing" test_basic_task_processing
    run_test "Concurrent Clients" test_concurrent_clients
    run_test "Leader Failure & Re-election" test_leader_failure
    run_test "Worker Server Failure" test_worker_server_failure
    run_test "Multiple Server Failures" test_multiple_server_failures
    run_test "Server Recovery" test_server_recovery
    run_test "Rapid Leader Changes" test_rapid_leader_changes
    run_test "High Concurrent Load" test_high_concurrent_load
    run_test "Client Retry Mechanism" test_client_retry_mechanism

    # Final cleanup
    cleanup

    # Print results
    print_header "Test Results"
    echo "Total Tests:  $TESTS_RUN"
    echo -e "${GREEN}Passed:       $TESTS_PASSED${NC}"
    echo -e "${RED}Failed:       $TESTS_FAILED${NC}"
    echo ""

    if [ $TESTS_FAILED -gt 0 ]; then
        echo -e "${RED}Failed Tests:${NC}"
        for test in "${FAILED_TESTS[@]}"; do
            echo -e "${RED}  - $test${NC}"
        done
        echo ""
        echo "Check logs in: $LOGS_DIR"
        exit 1
    else
        print_success "All tests passed!"
        echo ""
        echo "Logs available in: $LOGS_DIR"
        exit 0
    fi
}

# Run main if script is executed directly
if [ "${BASH_SOURCE[0]}" = "${0}" ]; then
    main "$@"
fi
