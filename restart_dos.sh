#!/bin/bash
# Script to cleanly restart the DoS system

echo "=============================================="
echo "  Restarting DoS System"
echo "=============================================="

# Step 1: Kill any running dos_server processes
echo "Step 1: Killing existing DoS servers..."
pkill -f dos_server
sleep 1

# Step 2: Set Firebase URL
echo "Step 2: Setting Firebase URL..."
export FIREBASE_DATABASE_URL="https://distributed-p2p-default-rtdb.europe-west1.firebasedatabase.app/"

# Step 3: Clear Firebase database
echo "Step 3: Clearing Firebase database..."
curl -s -X DELETE "${FIREBASE_DATABASE_URL}.json" > /dev/null
echo "✓ Firebase cleared"

# Step 4: Wait for Firebase to process
echo "Step 4: Waiting for Firebase..."
sleep 2

# Step 5: Start DoS server
echo "Step 5: Starting DoS server..."
echo "=============================================="
cargo run --bin dos_server
