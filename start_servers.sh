#!/bin/bash

# Script to start all servers for P2P testing
# Usage: ./start_servers.sh
# Then run test_p2p.sh in another terminal

echo "================================================"
echo "  Starting P2P Test Environment"
echo "================================================"
echo ""

# Check if Firebase URL is set
if [ -z "$FIREBASE_DATABASE_URL" ]; then
    echo "Setting Firebase URL..."
    export FIREBASE_DATABASE_URL="https://distributed-p2p-default-rtdb.europe-west1.firebasedatabase.app/"
fi

echo "Firebase URL: $FIREBASE_DATABASE_URL"
echo ""

# Kill any existing servers
echo "Cleaning up old servers..."
pkill -f dos_server 2>/dev/null
pkill -f web_server 2>/dev/null
sleep 1

# Start DoS server
echo "Starting DoS server on port 9000..."
cargo run --release --bin dos_server -- \
    --firebase-url "$FIREBASE_DATABASE_URL" \
    --bind-address "127.0.0.1:9000" &
DOS_PID=$!

sleep 2

# Start Client 2 web server
echo "Starting Client 2 web server (HTTP: 3002, P2P: 7002)..."
cargo run --release --bin web_server -- \
    --config config/client2.toml \
    --port 3002 \
    --p2p-port 7002 &
CLIENT2_PID=$!

sleep 1

# Start Client 3 web server
echo "Starting Client 3 web server (HTTP: 3003, P2P: 7003)..."
cargo run --release --bin web_server -- \
    --config config/client3.toml \
    --port 3003 \
    --p2p-port 7003 &
CLIENT3_PID=$!

sleep 2

echo ""
echo "================================================"
echo "  All servers started!"
echo "================================================"
echo "DoS Server:    PID $DOS_PID (port 9000)"
echo "Client 2:      PID $CLIENT2_PID (HTTP: 3002, P2P: 7002)"
echo "Client 3:      PID $CLIENT3_PID (HTTP: 3003, P2P: 7003)"
echo ""
echo "To run the test: bash test_p2p.sh"
echo "To stop servers: pkill -f dos_server && pkill -f web_server"
echo "================================================"
echo ""

# Wait for all background jobs
wait
