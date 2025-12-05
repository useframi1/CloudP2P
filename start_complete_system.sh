#!/bin/bash
# Start the complete CloudP2P system with all components

echo "=========================================="
echo "  CloudP2P Complete System Launcher"
echo "=========================================="
echo ""

# Set Firebase URL
export FIREBASE_DATABASE_URL="https://distributed-p2p-default-rtdb.europe-west1.firebasedatabase.app/"

# Cleanup
echo "[1/4] Cleaning up old processes..."
pkill -f dos_server 2>/dev/null || true
pkill -f "cargo run --bin server" 2>/dev/null || true
pkill -f "cargo run --bin web_server" 2>/dev/null || true
sleep 1

# Clear Firebase
echo "[2/4] Clearing Firebase database..."
curl -s -X DELETE "${FIREBASE_DATABASE_URL}.json" > /dev/null
sleep 2

# Create logs directory
mkdir -p logs

echo "[3/4] Starting all components..."
echo ""

# Start DoS Server
echo "  Starting DoS Server (port 9000)..."
cargo run --bin dos_server > logs/dos_server.log 2>&1 &
DOS_PID=$!
sleep 3

# Start 3 Processing Servers
echo "  Starting Server 1 (port 8001)..."
cargo run --bin server -- --config config/server1.toml > logs/server1.log 2>&1 &
SERVER1_PID=$!
sleep 1

echo "  Starting Server 2 (port 8002)..."
cargo run --bin server -- --config config/server2.toml > logs/server2.log 2>&1 &
SERVER2_PID=$!
sleep 1

echo "  Starting Server 3 (port 8003)..."
cargo run --bin server -- --config config/server3.toml > logs/server3.log 2>&1 &
SERVER3_PID=$!
sleep 4

# Start 3 Web Clients
echo "  Starting Web Client 1 (port 3001)..."
cargo run --bin web_server -- --config config/client1.toml --port 3001 > logs/web_client1.log 2>&1 &
CLIENT1_PID=$!
sleep 2

echo "  Starting Web Client 2 (port 3002)..."
cargo run --bin web_server -- --config config/client2.toml --port 3002 > logs/web_client2.log 2>&1 &
CLIENT2_PID=$!
sleep 2

echo "  Starting Web Client 3 (port 3003)..."
cargo run --bin web_server -- --config config/client3.toml --port 3003 > logs/web_client3.log 2>&1 &
CLIENT3_PID=$!
sleep 3

echo ""
echo "=========================================="
echo "  ✅ System Started Successfully!"
echo "=========================================="
echo ""
echo "Components Running:"
echo "  DoS Server:      http://127.0.0.1:9000  (PID: $DOS_PID)"
echo "  Server 1:        http://127.0.0.1:8001  (PID: $SERVER1_PID)"
echo "  Server 2:        http://127.0.0.1:8002  (PID: $SERVER2_PID)"
echo "  Server 3:        http://127.0.0.1:8003  (PID: $SERVER3_PID)"
echo "  Web Client 1:    http://127.0.0.1:3001  (PID: $CLIENT1_PID)"
echo "  Web Client 2:    http://127.0.0.1:3002  (PID: $CLIENT2_PID)"
echo "  Web Client 3:    http://127.0.0.1:3003  (PID: $CLIENT3_PID)"
echo ""
echo "Logs Directory: logs/"
echo "  tail -f logs/dos_server.log     # DoS activity"
echo "  tail -f logs/server1.log        # Server 1 activity"
echo "  tail -f logs/web_client1.log    # Client 1 activity"
echo ""
echo "[4/4] Running automated tests in 5 seconds..."
sleep 5
echo ""
./demo_test.sh
echo ""
echo "=========================================="
echo "  Testing Complete!"
echo "=========================================="
echo ""
echo "Next Steps:"
echo "  1. View logs:        tail -f logs/dos_server.log"
echo "  2. Test manually:    curl http://127.0.0.1:3001/api/status"
echo "  3. Check Firebase:   curl -s \"\${FIREBASE_DATABASE_URL}/clients.json\" | python3 -m json.tool"
echo "  4. Stop all:         ./stop_all.sh"
echo ""
echo "API Endpoints:"
echo "  Client 1: http://127.0.0.1:3001/api/*"
echo "  Client 2: http://127.0.0.1:3002/api/*"
echo "  Client 3: http://127.0.0.1:3003/api/*"
echo ""
