#!/bin/bash

# Deployment script for cloud P2P image sharing system

set -e

echo "=== Cloud P2P Image Sharing Deployment Script ==="

# Build the project
echo "Building the project..."
cargo build --release

# Create necessary directories
echo "Creating necessary directories..."
mkdir -p images/server1
mkdir -p images/server2
mkdir -p images/server3
mkdir -p images/test
mkdir -p logs

# Check if sample image exists
if [ ! -f images/test/sample.png ]; then
    echo "Warning: No sample image found at images/test/sample.png"
    echo "You may want to add a test image for verification"
fi

# Set executable permissions for scripts
echo "Setting executable permissions..."
chmod +x scripts/start_servers.sh
chmod +x scripts/stop_servers.sh

echo ""
echo "=== Deployment Complete ==="
echo ""
echo "To start the servers, run: ./scripts/start_servers.sh"
echo "To stop the servers, run: ./scripts/stop_servers.sh"
echo ""
echo "Server configurations:"
echo "  - Server 1: 127.0.0.1:8001 (Priority: 3)"
echo "  - Server 2: 127.0.0.1:8002 (Priority: 2)"
echo "  - Server 3: 127.0.0.1:8003 (Priority: 1)"
