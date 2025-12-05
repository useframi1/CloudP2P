#!/bin/bash
# Start complete CloudP2P system: DoS + 3 Servers + 3 Clients

set -e

echo "=========================================="
echo "  CloudP2P Full Demo"
echo "  1 DoS + 3 Servers + 3 Clients"
echo "=========================================="

# Set Firebase URL
export FIREBASE_DATABASE_URL="https://distributed-p2p-default-rtdb.europe-west1.firebasedatabase.app/"

# Kill any existing processes
echo "Cleaning up old processes..."
pkill -f dos_server 2>/dev/null || true
pkill -f "cargo run --bin server" 2>/dev/null || true
pkill -f "cargo run --bin client" 2>/dev/null || true
sleep 1

# Clear Firebase
echo "Clearing Firebase database..."
curl -s -X DELETE "${FIREBASE_DATABASE_URL}.json" > /dev/null
sleep 1

# Create logs directory
mkdir -p logs

echo "=========================================="
echo "Starting components..."
echo "=========================================="

# Start DoS Server
echo "[1/7] Starting DoS Server (port 9000)..."
cargo run --bin dos_server > logs/dos_server.log 2>&1 &
DOS_PID=$!
sleep 3

if ! ps -p $DOS_PID > /dev/null; then
    echo "❌ Failed to start DoS server"
    cat logs/dos_server.log
    exit 1
fi
echo "✅ DoS Server running (PID: $DOS_PID)"

# Start 3 Processing Servers
echo "[2/7] Starting Server 1 (port 8001)..."
cargo run --bin server -- --config config/server1.toml > logs/server1.log 2>&1 &
SERVER1_PID=$!
sleep 1

echo "[3/7] Starting Server 2 (port 8002)..."
cargo run --bin server -- --config config/server2.toml > logs/server2.log 2>&1 &
SERVER2_PID=$!
sleep 1

echo "[4/7] Starting Server 3 (port 8003)..."
cargo run --bin server -- --config config/server3.toml > logs/server3.log 2>&1 &
SERVER3_PID=$!
sleep 3

echo "✅ Server 1 running (PID: $SERVER1_PID)"
echo "✅ Server 2 running (PID: $SERVER2_PID)"
echo "✅ Server 3 running (PID: $SERVER3_PID)"

# Wait for leader election
echo "⏳ Waiting for leader election..."
sleep 3

# Start 3 Clients
echo "[5/7] Starting Client 1..."
cargo run --bin client -- --config config/client1.toml --client-id 1 > logs/client1.log 2>&1 &
CLIENT1_PID=$!
sleep 2

echo "[6/7] Starting Client 2..."
cargo run --bin client -- --config config/client2.toml --client-id 2 > logs/client2.log 2>&1 &
CLIENT2_PID=$!
sleep 2

echo "[7/7] Starting Client 3..."
cargo run --bin client -- --config config/client3.toml --client-id 3 > logs/client3.log 2>&1 &
CLIENT3_PID=$!
sleep 2

echo "✅ Client 1 running (PID: $CLIENT1_PID)"
echo "✅ Client 2 running (PID: $CLIENT2_PID)"
echo "✅ Client 3 running (PID: $CLIENT3_PID)"

echo ""
echo "=========================================="
echo "  ✅ Full System Running!"
echo "=========================================="
echo ""
echo "Architecture:"
echo "  DoS Server (coordination):"
echo "    • Port 9000 - Client registry & access control"
echo ""
echo "  Processing Servers (your 3 servers):"
echo "    • Server 1: 127.0.0.1:8001"
echo "    • Server 2: 127.0.0.1:8002"
echo "    • Server 3: 127.0.0.1:8003"
echo ""
echo "  Clients (your 3 clients):"
echo "    • Client 1: Registered as client_1"
echo "    • Client 2: Registered as client_2"
echo "    • Client 3: Registered as client_3"
echo ""
echo "Logs:"
echo "  tail -f logs/dos_server.log   # See client registrations"
echo "  tail -f logs/server1.log      # See leader election & processing"
echo "  tail -f logs/client1.log      # See client activity"
echo ""
echo "Check Firebase:"
echo "  curl -s \"\${FIREBASE_DATABASE_URL}/clients.json\" | python3 -m json.tool"
echo ""
echo "Stop everything:"
echo "  ./stop_all.sh"
echo ""
echo "Press Ctrl+C to stop monitoring (processes will continue in background)"
echo ""

# Monitor logs in foreground
echo "=========================================="
echo "  Live Logs (Ctrl+C to stop monitoring)"
echo "=========================================="
tail -f logs/dos_server.log
