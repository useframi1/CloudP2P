#!/bin/bash

# Start Web Servers for CloudP2P Image Sharing
# This script starts 3 web servers on different ports for 3 clients

echo "========================================"
echo "Starting CloudP2P Web Servers"
echo "========================================"
echo ""

# Export Firebase URL
export FIREBASE_DATABASE_URL="https://distributed-p2p-default-rtdb.europe-west1.firebasedatabase.app/"

# Kill any existing web servers
echo "Stopping any existing web servers..."
pkill -f "web_server.*port.*3000" 2>/dev/null || true
pkill -f "web_server.*port.*3001" 2>/dev/null || true
pkill -f "web_server.*port.*3002" 2>/dev/null || true
sleep 2

# Build the project
echo "Building project..."
cargo build --release --bin web_server
if [ $? -ne 0 ]; then
    echo "Build failed!"
    exit 1
fi

echo ""
echo "Starting web servers..."
echo ""

# Start Web Server 1 (Client1) on port 3000
echo "Starting Web Server 1 on port 3000 (Client1)..."
RUST_LOG=info cargo run --release --bin web_server -- \
    --config config/client1.toml \
    --port 3000 \
    > logs/webserver1.log 2>&1 &
WEB1_PID=$!
echo "Web Server 1 PID: $WEB1_PID"
echo "Access at: http://localhost:3000"

# Start Web Server 2 (Client2) on port 3001
echo "Starting Web Server 2 on port 3001 (Client2)..."
RUST_LOG=info cargo run --release --bin web_server -- \
    --config config/client2.toml \
    --port 3001 \
    > logs/webserver2.log 2>&1 &
WEB2_PID=$!
echo "Web Server 2 PID: $WEB2_PID"
echo "Access at: http://localhost:3001"

# Start Web Server 3 (Client3) on port 3002
echo "Starting Web Server 3 on port 3002 (Client3)..."
RUST_LOG=info cargo run --release --bin web_server -- \
    --config config/client3.toml \
    --port 3002 \
    > logs/webserver3.log 2>&1 &
WEB3_PID=$!
echo "Web Server 3 PID: $WEB3_PID"
echo "Access at: http://localhost:3002"

echo ""
echo "========================================"
echo "All Web Servers Started!"
echo "========================================"
echo ""
echo "Web Server 1: http://localhost:3000 (Client1)"
echo "Web Server 2: http://localhost:3001 (Client2)"
echo "Web Server 3: http://localhost:3002 (Client3)"
echo ""
echo "PIDs: $WEB1_PID, $WEB2_PID, $WEB3_PID"
echo ""
echo "To view logs:"
echo "  tail -f logs/webserver1.log"
echo "  tail -f logs/webserver2.log"
echo "  tail -f logs/webserver3.log"
echo ""
echo "To stop all servers:"
echo "  pkill -f web_server"
echo ""
