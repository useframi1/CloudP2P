# Local Testing Guide

This guide shows how to run stress tests locally on your machine using multiple terminals.

## Quick Setup for Local Testing

### 1. Build the Project

```bash
cargo build --release
```

### 2. Create Test Images

```bash
mkdir -p test_images

# Create dummy test images (or copy real ones)
# For testing, you can create simple images:
convert -size 800x600 xc:red test_images/small.png      # ~100KB
convert -size 1920x1080 xc:green test_images/medium.png # ~1MB
convert -size 3840x2160 xc:blue test_images/large.png   # ~5MB

# Or if you don't have ImageMagick, just copy any images:
cp /path/to/some/image1.jpg test_images/small.jpg
cp /path/to/some/image2.png test_images/medium.png
cp /path/to/some/image3.png test_images/large.png
```

### 3. Use Existing Server Configs

Your existing server configs should already use localhost:

```bash
# config/server1.toml - Already configured
# config/server2.toml - Already configured
# config/server3.toml - Already configured
```

### 4. Create Local Client Stress Config

```bash
cp config/client1.toml config/client_stress_local.toml
```

Edit `config/client_stress_local.toml`:

```toml
[client]
name = "StressTestClient"
server_addresses = [
    "127.0.0.1:8001",
    "127.0.0.1:8002",
    "127.0.0.1:8003"
]

[requests]
rate_per_second = 2.0       # Lower rate for local testing
duration_seconds = 60.0     # Shorter duration for quick test
```

### 5. Create Local Stress Test Config

Edit `scripts/config/stress_test.conf`:

```bash
# Machine identifier
MACHINE_ID=1

# Reduced numbers for local testing
NUM_CLIENTS=10              # Instead of 100
REQUESTS_PER_CLIENT=50      # Instead of 1000

# Random delay range
MIN_DELAY_MS=100
MAX_DELAY_MS=2000

# Local paths
IMAGE_DIRECTORY="./test_images"
CLIENT_CONFIG="./config/client_stress_local.toml"
METRICS_OUTPUT_DIR="./metrics"
CLIENT_BINARY="./target/release/client"
```

### 6. Create Local Fault Simulation Config

Edit `scripts/config/fault_sim.conf`:

```bash
# Timing parameters (shorter for local testing)
FAULT_INTERVAL_SECS=20      # Reduced from 30
RESTART_DELAY_SECS=5        # Reduced from 10
NUM_CYCLES=2                # Reduced from 3

LOG_FILE="./fault_events.log"

# Local servers (use localhost instead of SSH)
# For local testing, we'll use kill/restart without SSH
SERVER_1_HOST="localhost"
SERVER_1_CONFIG="$(pwd)/config/server1.toml"
SERVER_1_WORK_DIR="$(pwd)"
SERVER_1_BINARY="./target/release/server"

SERVER_2_HOST="localhost"
SERVER_2_CONFIG="$(pwd)/config/server2.toml"
SERVER_2_WORK_DIR="$(pwd)"
SERVER_2_BINARY="./target/release/server"

SERVER_3_HOST="localhost"
SERVER_3_CONFIG="$(pwd)/config/server3.toml"
SERVER_3_WORK_DIR="$(pwd)"
SERVER_3_BINARY="./target/release/server"
```

### 7. Create Local Fault Simulation Script

Since SSH won't work for localhost, create a local version:

```bash
cp scripts/fault_simulation.sh scripts/fault_simulation_local.sh
```

I'll create a simpler local version below.

## Running the Test

### Terminal Layout

You'll need **5 terminals**:

```
┌─────────────┬─────────────┬─────────────┐
│  Terminal 1 │  Terminal 2 │  Terminal 3 │
│   Server 1  │   Server 2  │   Server 3  │
└─────────────┴─────────────┴─────────────┘
┌──────────────────────┬──────────────────┐
│     Terminal 4       │   Terminal 5     │
│   Stress Test        │ Fault Simulation │
└──────────────────────┴──────────────────┘
```

### Step-by-Step Execution

**Terminal 1 - Server 1:**
```bash
cd /Users/youssef/Documents/Fall-2025/Distributed-Systems/CloudP2P
./target/release/server --config config/server1.toml
```

**Terminal 2 - Server 2:**
```bash
cd /Users/youssef/Documents/Fall-2025/Distributed-Systems/CloudP2P
./target/release/server --config config/server2.toml
```

**Terminal 3 - Server 3:**
```bash
cd /Users/youssef/Documents/Fall-2025/Distributed-Systems/CloudP2P
./target/release/server --config config/server3.toml
```

Wait a few seconds for leader election to complete. You should see log messages about the election.

**Terminal 4 - Stress Test:**
```bash
cd /Users/youssef/Documents/Fall-2025/Distributed-Systems/CloudP2P
./scripts/stress_test.sh
```

This will spawn 10 clients (instead of 100), each sending 50 requests (instead of 1000).

**Terminal 5 - Fault Simulation (wait ~10 seconds after starting stress test):**
```bash
cd /Users/youssef/Documents/Fall-2025/Distributed-Systems/CloudP2P
./scripts/fault_simulation_local.sh
```

### View Results

After the stress test completes (should take 2-3 minutes with the reduced settings):

```bash
# View metrics for a specific client
cat metrics/machine_1_client_1.json | python3 -m json.tool

# Quick stats
echo "Total clients run:"
ls metrics/machine_1_client_*.json | wc -l

echo "Total requests:"
cat metrics/machine_1_client_*.json | jq '.aggregated_stats.total_requests' | awk '{s+=$1} END {print s}'

echo "Average latency:"
cat metrics/machine_1_client_*.json | jq '.aggregated_stats.latency_avg_ms' | awk '{s+=$1; c++} END {print s/c " ms"}'

echo "Total failures:"
cat metrics/machine_1_client_*.json | jq '.aggregated_stats.failed_requests' | awk '{s+=$1} END {print s}'
```

## Simplified Commands

The local configuration files are already created for you:
- `config/client_stress_local.toml` - Client config using localhost
- `scripts/config/stress_test_local.conf` - 10 clients, 50 requests each
- `scripts/config/fault_sim_local.conf` - Localhost fault simulation
- `scripts/fault_simulation_local.sh` - Local fault script (no SSH)

### Quick Start

**Step 1-3: Start servers** (3 terminals)
```bash
./target/release/server --config config/server1.toml
./target/release/server --config config/server2.toml
./target/release/server --config config/server3.toml
```

**Step 4: Run stress test** (terminal 4)
```bash
./scripts/stress_test.sh scripts/config/stress_test_local.conf
```

**Step 5: Run fault simulation** (terminal 5, wait 10s after step 4)
```bash
./scripts/fault_simulation_local.sh
```

### Quick Analysis After Test

```bash
# Total requests
jq -s 'map(.aggregated_stats.total_requests) | add' metrics/machine_1_client_*.json

# Failure rate
jq -s 'map(.aggregated_stats | [.total_requests, .failed_requests]) |
       transpose | map(add) | .[1] / .[0] * 100' metrics/machine_1_client_*.json

# Average latency
jq -s 'map(.aggregated_stats.latency_avg_ms) | add / length' metrics/machine_1_client_*.json

# Server distribution
jq -s 'map(.aggregated_stats.server_distribution) |
       reduce .[] as $item ({};. + $item)' metrics/machine_1_client_*.json
```

## Troubleshooting

### "Address already in use"
```bash
pkill -f "server --config"
# Or kill specific ports
lsof -ti:8001 | xargs kill -9
lsof -ti:8002 | xargs kill -9
lsof -ti:8003 | xargs kill -9
```

### "No such file: test_images"
```bash
mkdir -p test_images
# Copy any 3 images
cp ~/Pictures/*.{jpg,png} test_images/
# Rename to small.jpg, medium.png, large.png
```

### Clean Up
```bash
pkill -f "server --config"
pkill -f "client --config"
rm -rf metrics/
rm -f fault_events_local.log
```

## Expected Results

With local config (10 clients × 50 requests = 500 total):
- **Duration**: 2-3 minutes
- **Latency P50**: < 100ms (localhost)
- **Failure rate**: < 2%
- **Load balancing**: ~33% per server
