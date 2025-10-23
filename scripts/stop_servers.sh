#!/bin/bash

echo "Stopping servers..."

if [ -f /tmp/server1.pid ]; then
    kill $(cat /tmp/server1.pid)
    rm /tmp/server1.pid
fi

if [ -f /tmp/server2.pid ]; then
    kill $(cat /tmp/server2.pid)
    rm /tmp/server2.pid
fi

if [ -f /tmp/server3.pid ]; then
    kill $(cat /tmp/server3.pid)
    rm /tmp/server3.pid
fi

echo "All servers stopped!"