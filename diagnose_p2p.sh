#!/bin/bash

# Diagnostic script for P2P issues

echo "========================================"
echo "  P2P Diagnostic Tool"
echo "========================================"
echo ""

DOS_URL="http://localhost:9000"
CLIENT2_URL="http://localhost:3002"
CLIENT3_URL="http://localhost:3003"

# Check if servers are running
echo "1. Checking if servers are accessible..."
echo "----------------------------------------"

echo -n "DoS Server (port 9000): "
if curl -s --max-time 2 "${DOS_URL}" >/dev/null 2>&1; then
    echo "✅ RUNNING"
else
    echo "❌ NOT ACCESSIBLE"
fi

echo -n "Client 2 (port 3002): "
if curl -s --max-time 2 "${CLIENT2_URL}/api/online-peers" >/dev/null 2>&1; then
    echo "✅ RUNNING"
else
    echo "❌ NOT ACCESSIBLE"
fi

echo -n "Client 3 (port 3003): "
if curl -s --max-time 2 "${CLIENT3_URL}/api/online-peers" >/dev/null 2>&1; then
    echo "✅ RUNNING"
else
    echo "❌ NOT ACCESSIBLE"
fi

echo ""

# Check P2P ports
echo "2. Checking P2P ports..."
echo "------------------------"

echo -n "Client 2 P2P (port 7002): "
if timeout 1 bash -c "echo > /dev/tcp/127.0.0.1/7002" 2>/dev/null; then
    echo "✅ LISTENING"
else
    echo "❌ NOT LISTENING"
fi

echo -n "Client 3 P2P (port 7003): "
if timeout 1 bash -c "echo > /dev/tcp/127.0.0.1/7003" 2>/dev/null; then
    echo "✅ LISTENING"
else
    echo "❌ NOT LISTENING"
fi

echo ""

# Check Firebase state
echo "3. Checking Firebase state..."
echo "------------------------------"

# Use environment variable if set, otherwise use default
if [ -n "$FIREBASE_DATABASE_URL" ]; then
    FIREBASE_URL="${FIREBASE_DATABASE_URL%/}"  # Remove trailing slash
    echo "Using Firebase URL from environment: $FIREBASE_URL"
else
    FIREBASE_URL="https://distributed-p2p-default-rtdb.europe-west1.firebasedatabase.app"
    echo "Using default Firebase URL: $FIREBASE_URL"
fi
echo ""

echo "Client 2 status:"
curl -s "${FIREBASE_URL}/clients/client2.json" | jq '{status, p2p_port, ip_address}'

echo ""
echo "Client 3 status:"
curl -s "${FIREBASE_URL}/clients/client3.json" | jq '{status, p2p_port, ip_address}'

echo ""
echo "Client 3 images:"
curl -s "${FIREBASE_URL}/clients/client3/images.json" | jq 'to_entries | map({image_id: .key, name: .value.name, access_rights: .value.access_rights | keys})'

echo ""

# Check image files
echo "4. Checking image files..."
echo "--------------------------"

echo -n "Client 3 claude.png: "
if [ -f "test_images/client3/claude.png" ]; then
    SIZE=$(stat -c%s "test_images/client3/claude.png" 2>/dev/null || stat -f%z "test_images/client3/claude.png" 2>/dev/null)
    echo "✅ EXISTS ($SIZE bytes)"
else
    echo "❌ NOT FOUND"
fi

echo ""
echo "========================================"
echo "Diagnostic complete!"
echo "========================================"
