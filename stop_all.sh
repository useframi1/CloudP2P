#!/bin/bash
# Stop all CloudP2P processes

echo "Stopping all CloudP2P processes..."

pkill -f dos_server
pkill -f "cargo run --bin server"
pkill -f "cargo run --bin client"
pkill -f web_server

sleep 1

echo "✅ All processes stopped"
