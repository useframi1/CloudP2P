#!/bin/bash

# CloudP2P Server Stop Script
#
# Usage:
#   ./scripts/stop_server.sh <server_id>
#
# Example:
#   ./scripts/stop_server.sh 1    # Stops server 1
#   ./scripts/stop_server.sh 2    # Stops server 2

set -e

# Check arguments
if [ $# -ne 1 ]; then
    echo "Usage: $0 <server_id>"
    echo "Example: $0 1"
    exit 1
fi

SERVER_ID=$1
LOG_DIR="logs"
PID_FILE="${LOG_DIR}/server_${SERVER_ID}.pid"

# Check if PID file exists
if [ ! -f "$PID_FILE" ]; then
    echo "Error: Server $SERVER_ID is not running (no PID file found: $PID_FILE)"
    echo ""
    echo "Checking for any running server processes..."
    RUNNING_PIDS=$(pgrep -f "server.*config.*server${SERVER_ID}" || echo "")
    if [ -n "$RUNNING_PIDS" ]; then
        echo "Found running server processes: $RUNNING_PIDS"
        echo "To kill manually: kill -9 $RUNNING_PIDS"
    else
        echo "No running server $SERVER_ID processes found"
    fi
    exit 1
fi

# Read PID
SERVER_PID=$(cat "$PID_FILE")

# Check if process is running
if ! ps -p "$SERVER_PID" > /dev/null 2>&1; then
    echo "Warning: Server $SERVER_ID (PID: $SERVER_PID) is not running"
    echo "Removing stale PID file: $PID_FILE"
    rm -f "$PID_FILE"
    exit 0
fi

# Stop the server
echo "Stopping Server $SERVER_ID (PID: $SERVER_PID)..."

# Try graceful shutdown first (SIGTERM)
if kill "$SERVER_PID" 2>/dev/null; then
    echo "Sent SIGTERM to Server $SERVER_ID, waiting for graceful shutdown..."

    # Wait up to 5 seconds for graceful shutdown
    for i in {1..5}; do
        if ! ps -p "$SERVER_PID" > /dev/null 2>&1; then
            echo "✅ Server $SERVER_ID stopped gracefully"
            rm -f "$PID_FILE"
            exit 0
        fi
        sleep 1
    done

    # If still running, force kill
    echo "Server $SERVER_ID did not stop gracefully, sending SIGKILL..."
    if kill -9 "$SERVER_PID" 2>/dev/null; then
        echo "✅ Server $SERVER_ID force stopped"
        rm -f "$PID_FILE"
        exit 0
    fi
fi

# Final check
if ps -p "$SERVER_PID" > /dev/null 2>&1; then
    echo "❌ Error: Failed to stop Server $SERVER_ID (PID: $SERVER_PID)"
    exit 1
else
    echo "✅ Server $SERVER_ID stopped"
    rm -f "$PID_FILE"
    exit 0
fi
