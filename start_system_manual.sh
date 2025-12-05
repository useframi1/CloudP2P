#!/bin/bash
# Start CloudP2P system for MANUAL testing (no automated tests)

echo "=========================================="
echo "  CloudP2P System - Manual Testing Mode"
echo "=========================================="
echo ""

# Set Firebase URL
export FIREBASE_DATABASE_URL="https://distributed-p2p-default-rtdb.europe-west1.firebasedatabase.app/"

# Cleanup
echo "[1/3] Cleaning up old processes..."
pkill -9 -f dos_server 2>/dev/null || true
pkill -9 -f "cargo run --bin server" 2>/dev/null || true
pkill -9 -f "cargo run --bin web_server" 2>/dev/null || true
sleep 2

# Clear Firebase
echo "[2/3] Clearing Firebase database..."
curl -s -X DELETE "${FIREBASE_DATABASE_URL}.json" > /dev/null
sleep 2

# Create logs directory
mkdir -p logs

echo "[3/3] Starting all components..."
echo ""

# Start DoS Server
echo "  ✓ DoS Server starting on port 9000..."
cargo run --bin dos_server > logs/dos_server.log 2>&1 &
DOS_PID=$!
sleep 3

# Start 3 Processing Servers
echo "  ✓ Server 1 starting on port 8001..."
cargo run --bin server -- --config config/server1.toml > logs/server1.log 2>&1 &
SERVER1_PID=$!
sleep 1

echo "  ✓ Server 2 starting on port 8002..."
cargo run --bin server -- --config config/server2.toml > logs/server2.log 2>&1 &
SERVER2_PID=$!
sleep 1

echo "  ✓ Server 3 starting on port 8003..."
cargo run --bin server -- --config config/server3.toml > logs/server3.log 2>&1 &
SERVER3_PID=$!
sleep 4

echo "  ✓ Waiting for leader election..."
sleep 2

# Start 3 Web Clients
echo "  ✓ Web Client 1 starting on port 3001..."
cargo run --bin web_server -- --config config/client1.toml --port 3001 > logs/web_client1.log 2>&1 &
CLIENT1_PID=$!
sleep 2

echo "  ✓ Web Client 2 starting on port 3002..."
cargo run --bin web_server -- --config config/client2.toml --port 3002 > logs/web_client2.log 2>&1 &
CLIENT2_PID=$!
sleep 2

echo "  ✓ Web Client 3 starting on port 3003..."
cargo run --bin web_server -- --config config/client3.toml --port 3003 > logs/web_client3.log 2>&1 &
CLIENT3_PID=$!
sleep 3

echo ""
echo "=========================================="
echo "  ✅ ALL SYSTEMS RUNNING!"
echo "=========================================="
echo ""
echo "COMPONENTS:"
echo "  • DoS Server:    http://127.0.0.1:9000  [PID: $DOS_PID]"
echo "  • Server 1:      http://127.0.0.1:8001  [PID: $SERVER1_PID]"
echo "  • Server 2:      http://127.0.0.1:8002  [PID: $SERVER2_PID]"
echo "  • Server 3:      http://127.0.0.1:8003  [PID: $SERVER3_PID]"
echo "  • Web Client 1:  http://127.0.0.1:3001  [PID: $CLIENT1_PID]"
echo "  • Web Client 2:  http://127.0.0.1:3002  [PID: $CLIENT2_PID]"
echo "  • Web Client 3:  http://127.0.0.1:3003  [PID: $CLIENT3_PID]"
echo ""
echo "=========================================="
echo "  MANUAL TESTING"
echo "=========================================="
echo ""
echo "SIGN UP CLIENTS:"
echo "  curl -X POST http://127.0.0.1:3001/api/signup"
echo "  curl -X POST http://127.0.0.1:3002/api/signup"
echo "  curl -X POST http://127.0.0.1:3003/api/signup"
echo ""
echo "VIEW ONLINE PEERS:"
echo "  curl http://127.0.0.1:3001/api/online-peers | python3 -m json.tool"
echo ""
echo "ENCRYPT IMAGE:"
echo "  curl -X POST http://127.0.0.1:3001/api/encrypt-image/image1.jpg | python3 -m json.tool"
echo ""
echo "VIEW LOGS (open new terminals):"
echo "  tail -f logs/dos_server.log"
echo "  tail -f logs/server1.log"
echo "  tail -f logs/web_client1.log"
echo ""
echo "STOP ALL:"
echo "  ./stop_all.sh"
echo ""
echo "=========================================="
echo ""
echo "Press Ctrl+C to stop monitoring (processes continue in background)"
echo "Showing DoS Server logs..."
echo ""

# Show DoS logs in foreground
tail -f logs/dos_server.log
