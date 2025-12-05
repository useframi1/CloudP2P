#!/bin/bash

# Complete Demo Setup for CloudP2P
# Starts DoS server, 3 backend servers, and 3 web servers

echo "========================================"
echo "CloudP2P Complete Demo Setup"
echo "========================================"
echo ""

# Export Firebase URL
export FIREBASE_DATABASE_URL="https://distributed-p2p-default-rtdb.europe-west1.firebasedatabase.app/"

# Create logs directory
mkdir -p logs

# Kill any existing processes
echo "Stopping any existing processes..."
pkill -f "dos_server" 2>/dev/null || true
pkill -f "target.*server" 2>/dev/null || true
pkill -f "web_server" 2>/dev/null || true
sleep 3

# Build the project
echo "Building project..."
cargo build --release
if [ $? -ne 0 ]; then
    echo "Build failed!"
    exit 1
fi

echo ""
echo "========================================"
echo "Starting DoS Server"
echo "========================================"

# Start DoS Server
echo "Starting DoS Server on 127.0.0.1:9000..."
RUST_LOG=info cargo run --release --bin dos_server -- \
    --firebase-url "$FIREBASE_DATABASE_URL" \
    --bind-address "127.0.0.1:9000" \
    > logs/dos_server.log 2>&1 &
DOS_PID=$!
echo "DoS Server PID: $DOS_PID"
sleep 2

echo ""
echo "========================================"
echo "Starting Backend Servers (3)"
echo "========================================"

# Start Server 1
echo "Starting Server 1 on port 8001..."
RUST_LOG=info cargo run --release --bin server -- \
    --config config/server1.toml \
    > logs/server1.log 2>&1 &
SERVER1_PID=$!
echo "Server 1 PID: $SERVER1_PID"
sleep 1

# Start Server 2
echo "Starting Server 2 on port 8002..."
RUST_LOG=info cargo run --release --bin server -- \
    --config config/server2.toml \
    > logs/server2.log 2>&1 &
SERVER2_PID=$!
echo "Server 2 PID: $SERVER2_PID"
sleep 1

# Start Server 3
echo "Starting Server 3 on port 8003..."
RUST_LOG=info cargo run --release --bin server -- \
    --config config/server3.toml \
    > logs/server3.log 2>&1 &
SERVER3_PID=$!
echo "Server 3 PID: $SERVER3_PID"
sleep 2

echo ""
echo "========================================"
echo "Starting Web Servers (3)"
echo "========================================"

# Start Web Server 1
echo "Starting Web Server 1 on port 3000 (Client1)..."
RUST_LOG=info cargo run --release --bin web_server -- \
    --config config/client1.toml \
    --port 3000 \
    > logs/webserver1.log 2>&1 &
WEB1_PID=$!
echo "Web Server 1 PID: $WEB1_PID"
sleep 1

# Start Web Server 2
echo "Starting Web Server 2 on port 3001 (Client2)..."
RUST_LOG=info cargo run --release --bin web_server -- \
    --config config/client2.toml \
    --port 3001 \
    > logs/webserver2.log 2>&1 &
WEB2_PID=$!
echo "Web Server 2 PID: $WEB2_PID"
sleep 1

# Start Web Server 3
echo "Starting Web Server 3 on port 3002 (Client3)..."
RUST_LOG=info cargo run --release --bin web_server -- \
    --config config/client3.toml \
    --port 3002 \
    > logs/webserver3.log 2>&1 &
WEB3_PID=$!
echo "Web Server 3 PID: $WEB3_PID"

echo ""
echo "========================================"
echo "System Startup Complete!"
echo "========================================"
echo ""
echo "DoS Server: Running on 127.0.0.1:9000 (PID: $DOS_PID)"
echo ""
echo "Backend Servers:"
echo "  Server 1: 127.0.0.1:8001 (PID: $SERVER1_PID)"
echo "  Server 2: 127.0.0.1:8002 (PID: $SERVER2_PID)"
echo "  Server 3: 127.0.0.1:8003 (PID: $SERVER3_PID)"
echo ""
echo "Web Interfaces:"
echo "  Client 1: http://localhost:3000 (PID: $WEB1_PID)"
echo "  Client 2: http://localhost:3001 (PID: $WEB2_PID)"
echo "  Client 3: http://localhost:3002 (PID: $WEB3_PID)"
echo ""
echo "========================================"
echo "How to Use:"
echo "========================================"
echo ""
echo "1. Open http://localhost:3000 in your browser for Client 1"
echo "2. Open http://localhost:3001 in another tab for Client 2"
echo "3. Open http://localhost:3002 in another tab for Client 3"
echo ""
echo "4. Sign up or sign in with a client ID"
echo "5. Use the 'Encrypt Image' tab to encrypt images"
echo "6. Use 'Request DoS' to see online peers and their images"
echo "7. Request access to images from other clients"
echo "8. Approve/deny access requests in 'Pending Requests'"
echo ""
echo "========================================"
echo "Monitoring:"
echo "========================================"
echo ""
echo "View logs with:"
echo "  tail -f logs/dos_server.log"
echo "  tail -f logs/server1.log"
echo "  tail -f logs/webserver1.log"
echo ""
echo "Stop all services:"
echo "  ./stop_all.sh"
echo ""
echo "Press Ctrl+C to stop monitoring..."
echo ""

# Monitor logs
tail -f logs/dos_server.log
