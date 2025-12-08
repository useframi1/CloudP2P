#!/bin/bash

# Test P2P Image Transfer Script
# This script tests direct peer-to-peer image transfer between Client 2 and Client 3

echo "================================================"
echo "  P2P Image Transfer Test"
echo "================================================"
echo ""

# Configuration
DOS_URL="http://localhost:9000"
CLIENT2_URL="http://localhost:3002"
CLIENT3_URL="http://localhost:3003"

echo "Step 1: Sign in Client 3..."
echo "----------------------------"
curl -X POST "$CLIENT3_URL/api/signin" \
  -H "Content-Type: application/json" \
  -d '{"client_id":"client3"}' \
  -w "\nHTTP Status: %{http_code}\n\n"

sleep 1

echo "Step 2: Sign in Client 2..."
echo "----------------------------"
curl -X POST "$CLIENT2_URL/api/signin" \
  -H "Content-Type: application/json" \
  -d '{"client_id":"client2"}' \
  -w "\nHTTP Status: %{http_code}\n\n"

sleep 1

echo "Step 3: Client 2 requests access to claude_png from Client 3..."
echo "----------------------------------------------------------------"
curl -X POST "$CLIENT2_URL/api/request-access" \
  -H "Content-Type: application/json" \
  -d '{"owner_id":"client3","image_id":"claude_png"}' \
  -w "\nHTTP Status: %{http_code}\n\n"

sleep 1

echo "Step 4: Get pending requests for Client 3..."
echo "---------------------------------------------"
REQUESTS=$(curl -s -X GET "$CLIENT3_URL/api/pending-requests")
echo "$REQUESTS" | jq '.'
echo ""

# Extract request_id from the response
REQUEST_ID=$(echo "$REQUESTS" | jq -r '.requests[0].request_id // empty')

if [ -z "$REQUEST_ID" ]; then
    echo "❌ No pending request found. Make sure images are registered."
    echo "   Run: cargo run --bin register_images -- client3 test_images/client3/"
    exit 1
fi

echo "Found request ID: $REQUEST_ID"
echo ""

sleep 1

echo "Step 5: Client 3 approves access request with view limit 5..."
echo "--------------------------------------------------------------"
curl -X POST "$CLIENT3_URL/api/respond-request" \
  -H "Content-Type: application/json" \
  -d "{\"request_id\":\"$REQUEST_ID\",\"approved\":true,\"view_limit\":5}" \
  -w "\nHTTP Status: %{http_code}\n\n"

echo "Waiting for access rights to propagate..."
sleep 3

echo "Step 6: Client 2 views image (P2P transfer should occur)..."
echo "------------------------------------------------------------"
echo "Watch the terminal output for P2P logs!"
echo ""

RESPONSE=$(curl -s -X POST "$CLIENT2_URL/api/view-image" \
  -H "Content-Type: application/json" \
  -d '{"owner_id":"client3","image_id":"claude_png"}')

echo "$RESPONSE" | jq 'del(.carrier_image_base64) | . + {carrier_image_size: (.carrier_image_base64 // "" | length)}'
echo ""

# Check if successful
SUCCESS=$(echo "$RESPONSE" | jq -r '.success')

if [ "$SUCCESS" = "true" ]; then
    echo "================================================"
    echo "✅ TEST PASSED - P2P Transfer Successful!"
    echo "================================================"
    echo ""
    echo "Check the terminal logs for:"
    echo "  Client 2: 🔌 [P2P_REQUEST] lines showing the request"
    echo "  Client 3: 🔐 [P2P_HANDLER] lines showing the response"
    echo ""
else
    echo "================================================"
    echo "❌ TEST FAILED"
    echo "================================================"
    echo ""
    ERROR=$(echo "$RESPONSE" | jq -r '.error // "Unknown error"')
    echo "Error: $ERROR"
    echo ""
fi
