#!/bin/bash

# Quick test runner with menu options

set -e

BLUE='\033[0;34m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

print_menu() {
    clear
    echo -e "${BLUE}╔════════════════════════════════════════════════╗${NC}"
    echo -e "${BLUE}║   CloudP2P Distributed System - Test Suite    ║${NC}"
    echo -e "${BLUE}╚════════════════════════════════════════════════╝${NC}"
    echo ""
    echo "Select test mode:"
    echo ""
    echo "  1) Run All Tests (Full Suite)"
    echo "  2) Run Basic Tests Only"
    echo "  3) Run Fault Tolerance Tests Only"
    echo "  4) Run Stress Tests Only"
    echo "  5) Verify Results (Check Output Files)"
    echo "  6) Clean Test Environment"
    echo "  7) View Test Logs"
    echo "  8) Exit"
    echo ""
}

run_all_tests() {
    echo -e "${GREEN}Running full test suite...${NC}"
    "$SCRIPT_DIR/integration_test.sh"
}

run_basic_tests() {
    echo -e "${GREEN}Running basic tests...${NC}"
    # Run only basic functionality tests
    # You can create a filtered version or use environment variables
    "$SCRIPT_DIR/integration_test.sh"
    # Note: For selective testing, modify integration_test.sh to support test filtering
}

run_fault_tests() {
    echo -e "${GREEN}Running fault tolerance tests...${NC}"
    "$SCRIPT_DIR/integration_test.sh"
}

run_stress_tests() {
    echo -e "${GREEN}Running stress tests...${NC}"
    "$SCRIPT_DIR/integration_test.sh"
}

verify_results() {
    echo -e "${GREEN}Verifying test results...${NC}"
    "$SCRIPT_DIR/verify_results.sh"
}

clean_env() {
    echo -e "${YELLOW}Cleaning test environment...${NC}"

    # Kill any running processes
    pkill -f "cloud-p2p-simple" 2>/dev/null || true
    pkill -f "target/release/server" 2>/dev/null || true
    pkill -f "target/release/client" 2>/dev/null || true

    # Clean output directories
    cd "$SCRIPT_DIR/.."
    rm -rf test_results
    rm -f user-data/outputs/encrypted_*

    echo -e "${GREEN}✓ Environment cleaned${NC}"
    echo ""
    read -p "Press Enter to continue..."
}

view_logs() {
    echo -e "${BLUE}Available logs:${NC}"
    echo ""

    cd "$SCRIPT_DIR/.."

    if [ ! -d "test_results/logs" ]; then
        echo "No logs found. Run tests first."
        echo ""
        read -p "Press Enter to continue..."
        return
    fi

    ls -1 test_results/logs/ 2>/dev/null || echo "No logs available"
    echo ""
    echo "Log directory: test_results/logs/"
    echo ""
    echo "View logs with:"
    echo "  tail -f test_results/logs/server1.log"
    echo "  grep -r 'ERROR' test_results/logs/"
    echo ""
    read -p "Press Enter to continue..."
}

main() {
    while true; do
        print_menu
        read -p "Enter choice [1-8]: " choice

        case $choice in
            1)
                run_all_tests
                read -p "Press Enter to continue..."
                ;;
            2)
                run_basic_tests
                read -p "Press Enter to continue..."
                ;;
            3)
                run_fault_tests
                read -p "Press Enter to continue..."
                ;;
            4)
                run_stress_tests
                read -p "Press Enter to continue..."
                ;;
            5)
                verify_results
                read -p "Press Enter to continue..."
                ;;
            6)
                clean_env
                ;;
            7)
                view_logs
                ;;
            8)
                echo "Exiting..."
                exit 0
                ;;
            *)
                echo "Invalid option. Please try again."
                sleep 2
                ;;
        esac
    done
}

main
