#!/bin/bash

# CloudP2P Server Startup Script
#
# Usage:
#   ./scripts/start_server.sh <server_id>
#
# Example:
#   ./scripts/start_server.sh 1    # Starts server 1 with config/server1.toml
#   ./scripts/start_server.sh 2    # Starts server 2 with config/server2.toml
#
# This script:
#   - Starts the server with the specified configuration
#   - Redirects output to logs/server_<id>.log
#   - Saves the PID to logs/server_<id>.pid
#   - Runs in the background using nohup

set -e

# Check arguments
if [ $# -ne 1 ]; then
    echo "Usage: $0 <server_id>"
    echo "Example: $0 1"
    exit 1
fi

SERVER_ID=$1
CONFIG_FILE="config/server${SERVER_ID}.toml"
LOG_DIR="logs"
LOG_FILE="${LOG_DIR}/server_${SERVER_ID}.log"
PID_FILE="${LOG_DIR}/server_${SERVER_ID}.pid"
SERVER_BINARY="./target/release/server"

# Validate inputs
if [ ! -f "$CONFIG_FILE" ]; then
    echo "Error: Configuration file not found: $CONFIG_FILE"
    exit 1
fi

if [ ! -f "$SERVER_BINARY" ]; then
    echo "Error: Server binary not found: $SERVER_BINARY"
    echo "Please build the project first: cargo build --release"
    exit 1
fi

# Create logs directory
mkdir -p "$LOG_DIR"

# Check if server is already running
if [ -f "$PID_FILE" ]; then
    OLD_PID=$(cat "$PID_FILE")
    if ps -p "$OLD_PID" > /dev/null 2>&1; then
        echo "Error: Server $SERVER_ID is already running (PID: $OLD_PID)"
        echo "To stop it first, run: ./scripts/stop_server.sh $SERVER_ID"
        exit 1
    else
        echo "Removing stale PID file (process $OLD_PID no longer exists)"
        rm -f "$PID_FILE"
    fi
fi

# Start the server
echo "Starting Server $SERVER_ID..."
echo "  Config: $CONFIG_FILE"
echo "  Log:    $LOG_FILE"
echo "  PID:    $PID_FILE"
echo ""

# Start server in background with nohup
nohup "$SERVER_BINARY" --config "$CONFIG_FILE" > "$LOG_FILE" 2>&1 &
SERVER_PID=$!

# Save PID to file
echo "$SERVER_PID" > "$PID_FILE"

# Wait a moment and verify it started
sleep 2

if ps -p "$SERVER_PID" > /dev/null 2>&1; then
    echo "✅ Server $SERVER_ID started successfully (PID: $SERVER_PID)"
    echo ""
    echo "To view logs in real-time:"
    echo "  tail -f $LOG_FILE"
    echo ""
    echo "To stop the server:"
    echo "  ./scripts/stop_server.sh $SERVER_ID"
else
    echo "❌ Error: Server $SERVER_ID failed to start"
    echo "Check the log file for details: $LOG_FILE"
    rm -f "$PID_FILE"
    exit 1
fi
