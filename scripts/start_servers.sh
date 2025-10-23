#!/bin/bash

echo "Starting Cloud P2P Image Sharing Servers..."

# Start Server 1
cargo run --bin server -- --config config/server1.toml &
SERVER1_PID=$!
echo "Started Server 1 (PID: $SERVER1_PID)"

# Start Server 2
cargo run --bin server -- --config config/server2.toml &
SERVER2_PID=$!
echo "Started Server 2 (PID: $SERVER2_PID)"

# Start Server 3
cargo run --bin server -- --config config/server3.toml &
SERVER3_PID=$!
echo "Started Server 3 (PID: $SERVER3_PID)"

echo "All servers started!"
echo "Server PIDs: $SERVER1_PID, $SERVER2_PID, $SERVER3_PID"

# Save PIDs for cleanup
echo $SERVER1_PID > /tmp/server1.pid
echo $SERVER2_PID > /tmp/server2.pid
echo $SERVER3_PID > /tmp/server3.pid