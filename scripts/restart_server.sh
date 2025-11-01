#!/bin/bash

# CloudP2P Server Restart Script
#
# Usage:
#   ./scripts/restart_server.sh <server_id>
#
# Example:
#   ./scripts/restart_server.sh 1    # Restarts server 1

set -e

# Check arguments
if [ $# -ne 1 ]; then
    echo "Usage: $0 <server_id>"
    echo "Example: $0 1"
    exit 1
fi

SERVER_ID=$1

echo "========================================="
echo "Restarting Server $SERVER_ID"
echo "========================================="
echo ""

# Stop the server if running
if [ -f "logs/server_${SERVER_ID}.pid" ]; then
    echo "Stopping Server $SERVER_ID..."
    ./scripts/stop_server.sh "$SERVER_ID"
    echo ""
    sleep 1
else
    echo "Server $SERVER_ID is not running, starting fresh..."
    echo ""
fi

# Start the server
./scripts/start_server.sh "$SERVER_ID"

echo ""
echo "========================================="
echo "Server $SERVER_ID restarted successfully"
echo "========================================="
