#!/bin/bash

# Quick script to check Firebase state
FIREBASE_URL="https://distributed-p2p-default-rtdb.europe-west1.firebasedatabase.app"

echo "================================================"
echo "  Firebase State Check"
echo "================================================"
echo ""

echo "Pending Requests:"
echo "-----------------"
curl -s "${FIREBASE_URL}/pending_requests.json" | jq '.'
echo ""

echo "Clients:"
echo "--------"
curl -s "${FIREBASE_URL}/clients.json" | jq 'to_entries | map({client_id: .key, status: .value.status, p2p_port: .value.p2p_port})'
echo ""
