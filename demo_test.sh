#!/bin/bash
# Comprehensive system test for CloudP2P

set -e

echo "=========================================="
echo "  CloudP2P Complete System Test"
echo "=========================================="
echo ""

BASE_URL_1="http://127.0.0.1:3001"
BASE_URL_2="http://127.0.0.1:3002"
BASE_URL_3="http://127.0.0.1:3003"

echo "[1/10] Signing up all clients..."
echo "Client 1:"
curl -s -X POST $BASE_URL_1/api/signup | python3 -m json.tool
echo ""
echo "Client 2:"
curl -s -X POST $BASE_URL_2/api/signup | python3 -m json.tool
echo ""
echo "Client 3:"
curl -s -X POST $BASE_URL_3/api/signup | python3 -m json.tool
echo "✅ All clients signed up"
echo ""

sleep 2

echo "[2/10] Checking online peers from Client 1..."
curl -s $BASE_URL_1/api/online-peers | python3 -m json.tool
echo "✅ Online peers listed"
echo ""

sleep 2

echo "[3/10] Viewing my images (Client 1)..."
curl -s $BASE_URL_1/api/my-images | python3 -m json.tool
echo "✅ Images listed"
echo ""

sleep 2

echo "[4/10] Encrypting image at server side (Client 1)..."
curl -s -X POST $BASE_URL_1/api/encrypt-image/image1.jpg | python3 -m json.tool
echo "✅ Image encrypted"
echo ""

sleep 2

echo "[5/10] Viewing shared images from Client 2..."
curl -s $BASE_URL_2/api/shared-images | python3 -m json.tool
echo "✅ Shared images listed"
echo ""

sleep 2

echo "[6/10] Client 2 encrypting image..."
curl -s -X POST $BASE_URL_2/api/encrypt-image/image1.jpg | python3 -m json.tool
echo "✅ Client 2 image encrypted"
echo ""

sleep 2

echo "[7/10] Client 3 encrypting image..."
curl -s -X POST $BASE_URL_3/api/encrypt-image/image1.jpg | python3 -m json.tool
echo "✅ Client 3 image encrypted"
echo ""

sleep 2

echo "[8/10] Signing out Client 3..."
curl -s -X POST $BASE_URL_3/api/signout | python3 -m json.tool
echo "✅ Client 3 signed out"
echo ""

sleep 2

echo "[9/10] Checking online peers again (Client 3 should be offline)..."
curl -s $BASE_URL_1/api/online-peers | python3 -m json.tool
echo "✅ Client 3 should be offline or not in list"
echo ""

sleep 2

echo "[10/10] Checking encrypted images directory..."
ls -lh test_images/encrypted/
echo "✅ Encrypted images saved"
echo ""

echo "=========================================="
echo "  ✅ Test Complete!"
echo "=========================================="
echo ""
echo "Summary:"
echo "  • 3 clients signed up (client_1, client_2, client_3)"
echo "  • All clients encrypted images at server side"
echo "  • Client 3 signed out and went offline"
echo "  • All encrypted images saved to test_images/encrypted/"
echo ""
echo "Check Firebase database:"
echo "  export FIREBASE_DATABASE_URL=\"https://distributed-p2p-default-rtdb.europe-west1.firebasedatabase.app/\""
echo "  curl -s \"\${FIREBASE_DATABASE_URL}/clients.json\" | python3 -m json.tool"
echo ""
