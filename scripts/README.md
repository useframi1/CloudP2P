# CloudP2P Server Management Scripts

This directory contains scripts for managing CloudP2P servers during development and testing.

## Server Management Scripts

### Starting a Server

To start a server on any machine:

```bash
./scripts/start_server.sh <server_id>
```

**Example:**
```bash
# On Server 1 machine
./scripts/start_server.sh 1

# On Server 2 machine
./scripts/start_server.sh 2

# On Server 3 machine
./scripts/start_server.sh 3
```

**What it does:**
- Starts the server with `config/server<id>.toml`
- Redirects all output to `logs/server_<id>.log`
- Saves the process ID to `logs/server_<id>.pid`
- Runs in the background using `nohup`

### Stopping a Server

```bash
./scripts/stop_server.sh <server_id>
```

**Example:**
```bash
./scripts/stop_server.sh 1
```

**What it does:**
- Attempts graceful shutdown (SIGTERM) first
- Waits up to 5 seconds for graceful shutdown
- Force kills (SIGKILL) if graceful shutdown fails
- Removes the PID file

### Checking Server Status

```bash
# Check all servers
./scripts/status_server.sh

# Check specific server
./scripts/status_server.sh 1
```

**Shows:**
- Whether the server is running
- Process ID (PID)
- How long it's been running
- Last 5 lines of logs

### Restarting a Server

```bash
./scripts/restart_server.sh <server_id>
```

**Example:**
```bash
./scripts/restart_server.sh 1
```

**What it does:**
- Stops the server (if running)
- Waits 1 second
- Starts the server again

## Viewing Logs

### Real-time log viewing

```bash
# View logs for server 1
tail -f logs/server_1.log

# View logs for server 2
tail -f logs/server_2.log
```

### View recent logs

```bash
# Last 50 lines
tail -50 logs/server_1.log

# Last 100 lines
tail -100 logs/server_1.log
```

### Search logs

```bash
# Search for errors
grep ERROR logs/server_1.log

# Search for specific client
grep "Client_5" logs/server_1.log

# Search for leader election events
grep "leader" logs/server_1.log -i
```

## Fault Simulation Script

The fault simulation script uses SSH to kill and restart servers in a ring pattern to simulate failures.

### Usage

```bash
./scripts/fault_simulation.sh [config_file]
```

**Default config:** `./scripts/config/fault_sim.conf`

### Configuration

Edit `scripts/config/fault_sim.conf` to configure:
- Which servers to target
- Fault interval (time between failures)
- Restart delay (how long servers stay down)
- Number of cycles to run

**Example configuration:**
```bash
FAULT_INTERVAL_SECS=30    # Wait 30s between each server failure
RESTART_DELAY_SECS=10     # Keep servers down for 10s
NUM_CYCLES=3              # Repeat the ring 3 times

SERVER_1_HOST="user@10.40.39.41"
SERVER_1_CONFIG="/path/to/config/server1.toml"
SERVER_1_WORK_DIR="/path/to/CloudP2P"
SERVER_1_BINARY="./target/release/server"
```

### Prerequisites

1. **SSH key-based authentication** must be configured:
   ```bash
   ssh-copy-id user@server_host
   ```

2. **Server scripts** must be deployed on each machine:
   - Copy the entire `scripts/` directory to each server machine
   - Ensure scripts are executable: `chmod +x scripts/*.sh`

3. **Start servers using the management scripts** (not manually):
   ```bash
   # On each server machine
   ./scripts/start_server.sh <server_id>
   ```

### What the Fault Simulation Does

1. **Ring Algorithm**: Processes servers in order (1 → 2 → 3 → 1 → ...)
2. **For each server:**
   - Kills the server process
   - Waits `RESTART_DELAY_SECS`
   - Restarts the server
   - Waits `FAULT_INTERVAL_SECS` before moving to next server
3. **Repeats** for `NUM_CYCLES` cycles

**Example timeline** (3 servers, 30s interval, 10s delay):
```
T=0s:   Kill Server 1
T=10s:  Restart Server 1
T=40s:  Kill Server 2
T=50s:  Restart Server 2
T=80s:  Kill Server 3
T=90s:  Restart Server 3
T=120s: [Cycle 1 complete, start Cycle 2]
```

## Typical Workflow

### Initial Setup (on each server machine)

1. Build the project:
   ```bash
   cargo build --release
   ```

2. Start the server:
   ```bash
   ./scripts/start_server.sh 1  # Use appropriate server ID
   ```

3. Verify it's running:
   ```bash
   ./scripts/status_server.sh 1
   ```

### Running Stress Tests with Fault Simulation

1. **On each server machine**, start the servers:
   ```bash
   # Server 1 machine
   ./scripts/start_server.sh 1

   # Server 2 machine
   ./scripts/start_server.sh 2

   # Server 3 machine (if you have one)
   ./scripts/start_server.sh 3
   ```

2. **On your local machine**, start the fault simulation:
   ```bash
   ./scripts/fault_simulation.sh
   ```

3. **Monitor logs** on each server:
   ```bash
   tail -f logs/server_1.log
   ```

4. **View fault simulation events:**
   ```bash
   cat fault_events.log
   ```

### Stopping Everything

**On each server machine:**
```bash
./scripts/stop_server.sh 1  # Use appropriate server ID
```

## Troubleshooting

### "Server already running" error

Check if there's a stale PID file:
```bash
./scripts/status_server.sh 1
```

If the server isn't actually running, the status script will clean up the stale PID file automatically.

### Can't find the server process

Check for orphaned processes:
```bash
ps aux | grep "target/release/server"
```

Kill manually if needed:
```bash
pkill -9 -f "target/release/server"
```

### Fault simulation can't kill servers

Make sure you're using the server management scripts to start servers, not running them manually in terminals. The fault simulation needs the PID files in `logs/server_<id>.pid`.

### Logs not appearing

Make sure the `logs/` directory exists:
```bash
mkdir -p logs
```

## Directory Structure

```
CloudP2P/
├── scripts/
│   ├── start_server.sh          # Start a server
│   ├── stop_server.sh           # Stop a server
│   ├── restart_server.sh        # Restart a server
│   ├── status_server.sh         # Check server status
│   ├── fault_simulation.sh      # Fault injection script
│   └── config/
│       └── fault_sim.conf       # Fault simulation config
├── logs/
│   ├── server_1.log            # Server 1 logs
│   ├── server_1.pid            # Server 1 process ID
│   ├── server_2.log            # Server 2 logs
│   ├── server_2.pid            # Server 2 process ID
│   └── ...
├── config/
│   ├── server1.toml            # Server 1 configuration
│   ├── server2.toml            # Server 2 configuration
│   └── ...
└── target/
    └── release/
        └── server              # Compiled server binary
```
