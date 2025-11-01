#!/bin/bash

# Script to find what's auto-restarting the CloudP2P server

CONFIG_FILE="${1:-./scripts/config/fault_sim.conf}"

if [ ! -f "$CONFIG_FILE" ]; then
    echo "Error: Configuration file not found: $CONFIG_FILE"
    exit 1
fi

source "$CONFIG_FILE"

echo "========================================="
echo "Finding What's Restarting Servers"
echo "========================================="
echo ""

# Function to diagnose a server
diagnose_server() {
    local host="$1"
    local server_id="$2"
    local work_dir="$3"

    echo "=== Server $server_id ($host) ==="
    echo ""

    echo "1. Current server process:"
    ssh "$host" "ps aux | grep 'target/release/server' | grep -v grep" || echo "   No server process found"
    echo ""

    echo "2. Process tree (with parents):"
    ssh "$host" "pgrep -f 'target/release/server' | xargs -I {} ps -o pid,ppid,user,cmd -p {} 2>/dev/null" || echo "   No process found"
    echo ""

    echo "3. Systemd user services:"
    ssh "$host" "systemctl --user list-units --type=service --all 2>/dev/null | grep -i 'server\|cloud' || echo '   None found'"
    echo ""

    echo "4. System-wide systemd services:"
    ssh "$host" "systemctl list-units --type=service 2>/dev/null | grep -i 'cloud\|p2p' || echo '   None found'"
    echo ""

    echo "5. Cron jobs:"
    ssh "$host" "crontab -l 2>/dev/null || echo '   No crontab'"
    echo ""

    echo "6. Running screen/tmux sessions:"
    ssh "$host" "screen -ls 2>/dev/null || echo '   No screen sessions'"
    ssh "$host" "tmux ls 2>/dev/null || echo '   No tmux sessions'"
    echo ""

    echo "7. Checking for supervisor/monitoring processes:"
    ssh "$host" "ps aux | grep -E 'supervisor|monit|upstart|systemd.*spawn' | grep -v grep || echo '   None found'"
    echo ""

    echo "8. Checking shell scripts in background:"
    ssh "$host" "ps aux | grep -E 'bash.*server|sh.*server' | grep -v grep || echo '   None found'"
    echo ""

    echo "9. Checking if server binary has auto-restart compiled in:"
    ssh "$host" "cd $work_dir && strings target/release/server | grep -i restart | head -5 || echo '   No restart strings found'"
    echo ""

    echo "10. Checking for inotify/file watchers:"
    ssh "$host" "ps aux | grep -E 'inotify|fswatch|watchdog' | grep -v grep || echo '   None found'"
    echo ""

    echo "----------------------------------------"
    echo ""
}

# Check each configured server
if [ -n "$SERVER_1_HOST" ]; then
    diagnose_server "$SERVER_1_HOST" 1 "$SERVER_1_WORK_DIR"
fi

if [ -n "$SERVER_2_HOST" ]; then
    diagnose_server "$SERVER_2_HOST" 2 "$SERVER_2_WORK_DIR"
fi

if [ -n "$SERVER_3_HOST" ]; then
    diagnose_server "$SERVER_3_HOST" 3 "$SERVER_3_WORK_DIR"
fi

echo "========================================="
echo "Diagnosis Complete"
echo ""
echo "If you found something restarting the servers:"
echo "  - Systemd service: systemctl --user stop <service>"
echo "  - Cron job: crontab -e (then remove the line)"
echo "  - Screen/tmux: screen -r or tmux attach (then Ctrl+C)"
echo "========================================="
