#!/bin/bash
# Start the complete CloudP2P system with DoS integration

set -e

echo "=========================================="
echo "  Starting CloudP2P Full System"
echo "=========================================="

# Set Firebase URL
export FIREBASE_DATABASE_URL="https://distributed-p2p-default-rtdb.europe-west1.firebasedatabase.app/"

# Kill any existing processes
echo "Cleaning up old processes..."
pkill -f dos_server 2>/dev/null || true
pkill -f "cargo run --bin server" 2>/dev/null || true
pkill -f "cargo run --bin client" 2>/dev/null || true
pkill -f web_server 2>/dev/null || true
sleep 1

# Clear Firebase
echo "Clearing Firebase..."
curl -s -X DELETE "${FIREBASE_DATABASE_URL}.json" > /dev/null
sleep 1

# Build if needed
if [ ! -f "target/debug/dos_server" ]; then
    echo "Building project..."
    cargo build
fi

echo "=========================================="
echo "Starting components..."
echo "=========================================="

# Start DoS Server
echo "[1/4] Starting DoS Server on port 9000..."
cargo run --bin dos_server > logs/dos_server.log 2>&1 &
DOS_PID=$!
sleep 2

if ! ps -p $DOS_PID > /dev/null; then
    echo "❌ Failed to start DoS server"
    cat logs/dos_server.log
    exit 1
fi
echo "✅ DoS Server running (PID: $DOS_PID)"

# Start Servers
echo "[2/4] Starting Processing Servers..."
cargo run --bin server -- --config config/server1.toml > logs/server1.log 2>&1 &
SERVER1_PID=$!
sleep 1

cargo run --bin server -- --config config/server2.toml > logs/server2.log 2>&1 &
SERVER2_PID=$!
sleep 1

cargo run --bin server -- --config config/server3.toml > logs/server3.log 2>&1 &
SERVER3_PID=$!
sleep 2

echo "✅ Server 1 running (PID: $SERVER1_PID)"
echo "✅ Server 2 running (PID: $SERVER2_PID)"
echo "✅ Server 3 running (PID: $SERVER3_PID)"

# Wait for leader election
echo "[3/4] Waiting for leader election..."
sleep 3

# Start Web Server (optional)
if [ "$1" == "--with-web" ]; then
    echo "[4/4] Starting Web Server on port 3000..."
    cargo run --bin web_server > logs/web_server.log 2>&1 &
    WEB_PID=$!
    sleep 2
    echo "✅ Web Server running (PID: $WEB_PID)"
    echo ""
    echo "🌐 Web UI: http://127.0.0.1:3000"
else
    echo "[4/4] Skipping Web Server (use --with-web to enable)"
fi

echo ""
echo "=========================================="
echo "  ✅ System Started Successfully!"
echo "=========================================="
echo ""
echo "Components running:"
echo "  • DoS Server:      127.0.0.1:9000"
echo "  • Processing Servers:"
echo "    - Server 1:      127.0.0.1:8001"
echo "    - Server 2:      127.0.0.1:8002"
echo "    - Server 3:      127.0.0.1:8003"
if [ "$1" == "--with-web" ]; then
    echo "  • Web Server:      127.0.0.1:3000"
fi
echo ""
echo "Logs:"
echo "  • DoS:     logs/dos_server.log"
echo "  • Server1: logs/server1.log"
echo "  • Server2: logs/server2.log"
echo "  • Server3: logs/server3.log"
if [ "$1" == "--with-web" ]; then
    echo "  • Web:     logs/web_server.log"
fi
echo ""
echo "Test with client:"
echo "  cargo run --bin client -- --config config/client1.toml"
echo ""
echo "Monitor logs:"
echo "  tail -f logs/dos_server.log"
echo ""
echo "Stop all:"
echo "  ./stop_all.sh"
echo ""
