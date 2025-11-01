#!/bin/bash

# CloudP2P Server Status Script
#
# Usage:
#   ./scripts/status_server.sh [server_id]
#
# Examples:
#   ./scripts/status_server.sh      # Shows status of all servers
#   ./scripts/status_server.sh 1    # Shows status of server 1

LOG_DIR="logs"

# Function to check single server status
check_server_status() {
    local server_id=$1
    local pid_file="${LOG_DIR}/server_${server_id}.pid"
    local log_file="${LOG_DIR}/server_${server_id}.log"

    echo "=== Server $server_id ==="

    if [ ! -f "$pid_file" ]; then
        echo "Status: ❌ NOT RUNNING (no PID file)"
    else
        local pid=$(cat "$pid_file")
        if ps -p "$pid" > /dev/null 2>&1; then
            echo "Status: ✅ RUNNING"
            echo "PID:    $pid"

            # Show process info
            local process_info=$(ps -o pid,ppid,etime,command -p "$pid" | tail -1)
            echo "Info:   $process_info"

            # Show last few log lines
            if [ -f "$log_file" ]; then
                echo ""
                echo "Recent logs (last 5 lines):"
                tail -5 "$log_file" | sed 's/^/  /'
            fi
        else
            echo "Status: ❌ NOT RUNNING (stale PID: $pid)"
            rm -f "$pid_file"
        fi
    fi

    # Check for orphaned processes
    local orphaned=$(pgrep -f "server.*config.*server${server_id}" || echo "")
    if [ -n "$orphaned" ] && [ ! -f "$pid_file" ]; then
        echo "Warning: Found orphaned server processes: $orphaned"
        echo "         Run: kill -9 $orphaned"
    fi

    echo ""
}

# Main logic
if [ $# -eq 0 ]; then
    # Check all servers (1-10)
    echo "========================================="
    echo "CloudP2P Server Status"
    echo "========================================="
    echo ""

    for i in {1..10}; do
        if [ -f "config/server${i}.toml" ]; then
            check_server_status "$i"
        fi
    done

    echo "========================================="
else
    # Check specific server
    SERVER_ID=$1
    check_server_status "$SERVER_ID"
fi
