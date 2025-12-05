#!/bin/bash

echo "Stopping all web servers..."

pkill -f "web_server.*port.*3000" 2>/dev/null || true
pkill -f "web_server.*port.*3001" 2>/dev/null || true
pkill -f "web_server.*port.*3002" 2>/dev/null || true

sleep 2

echo "All web servers stopped!"
