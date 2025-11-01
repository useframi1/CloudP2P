# CloudP2P Stress Testing Guide

This guide explains how to run comprehensive stress tests on the CloudP2P distributed system with fault simulation and metrics collection.

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Prerequisites](#prerequisites)
4. [Setup](#setup)
5. [Running Stress Tests](#running-stress-tests)
6. [Fault Simulation](#fault-simulation)
7. [Metrics Aggregation](#metrics-aggregation)
8. [Troubleshooting](#troubleshooting)

---

## Overview

The stress testing framework provides:

- **Concurrent Client Load**: Spawn hundreds of clients on multiple machines, each sending thousands of requests
- **Random Delays**: Configurable random delays between requests to simulate realistic traffic patterns
- **Fault Simulation**: Ring algorithm-based server failures to test fault tolerance
- **Comprehensive Metrics**: Track request latency, load balancing distribution, and failure rates
- **Automated Reporting**: Aggregate metrics from all machines into a single comprehensive report

### Metrics Collected

1. **Request Latency**: Min, Max, Avg, P50, P95, P99 for successful requests
2. **Load Balancing**: Distribution of requests across servers
3. **Failure Analysis**: Total failures, failure rate percentage, failure reasons

---

## Architecture

### Components

```
┌─────────────────┐
│ Controller      │
│ Machine         │
│                 │
│ - Fault         │
│   Simulation    │
│ - Metrics       │
│   Aggregation   │
└─────────────────┘
        │
        │ SSH (fault control)
        ├─────────────┬─────────────┬─────────────┐
        ▼             ▼             ▼
┌─────────────┐ ┌─────────────┐ ┌─────────────┐
│ Server 1    │ │ Server 2    │ │ Server 3    │
│             │ │             │ │             │
│ CloudP2P    │ │ CloudP2P    │ │ CloudP2P    │
│ Server      │ │ Server      │ │ Server      │
└─────────────┘ └─────────────┘ └─────────────┘
        ▲             ▲             ▲
        │             │             │
        │   TCP Requests            │
        ├─────────────┴─────────────┤
        │                           │
┌─────────────┐ ┌─────────────┐ ┌─────────────┐
│ Client      │ │ Client      │ │ Client      │
│ Machine 1   │ │ Machine 2   │ │ Machine 3   │
│             │ │             │ │             │
│ 100 clients │ │ 100 clients │ │ 100 clients │
│ x 1000 req  │ │ x 1000 req  │ │ x 1000 req  │
└─────────────┘ └─────────────┘ └─────────────┘
```

### Client Naming Convention

Clients are named: `Machine_{n}_Client_{i}`

- `n`: Machine ID (1, 2, 3, ...)
- `i`: Client ID on that machine (1, 2, 3, ..., NUM_CLIENTS)

Example: `Machine_1_Client_42` is the 42nd client on Machine 1.

---

## Prerequisites

### Software Requirements

All machines:
- Rust (for building CloudP2P)
- SSH server running
- Python 3 (for metrics aggregation)

Client machines:
- Bash shell
- `bc` command (for calculations)

Controller machine:
- SSH client
- SCP (for collecting metrics)
- Python 3 (for aggregation)

### SSH Key-Based Authentication

You must configure passwordless SSH authentication from:
- **Controller → All servers** (for fault simulation)
- **Controller → All clients** (for metrics collection)

#### Setup SSH Keys

On the controller machine:

```bash
# Generate SSH key if you don't have one
ssh-keygen -t rsa -b 4096

# Copy key to all server machines
ssh-copy-id user@server1.example.com
ssh-copy-id user@server2.example.com
ssh-copy-id user@server3.example.com

# Copy key to all client machines
ssh-copy-id user@client1.example.com
ssh-copy-id user@client2.example.com
ssh-copy-id user@client3.example.com

# Test connectivity
ssh user@server1.example.com "echo 'SSH OK'"
```

### Test Images

Prepare 3 test images of different sizes in the `test_images/` directory:

```bash
mkdir -p test_images

# Example sizes:
# - small.png:  ~100KB
# - medium.jpg: ~1MB
# - large.png:  ~5MB
```

The client will randomly select from these images for each request.

---

## Setup

### 1. Build the Project

On all machines (servers and clients), build the release binary:

```bash
cd /path/to/CloudP2P
cargo build --release
```

### 2. Configure Servers

On each server machine, create a configuration file:

**Server 1** (`config/server1.toml`):
```toml
[server]
id = 1
address = "0.0.0.0:8001"

[peers]
peers = [
    { id = 2, address = "server2.example.com:8001" },
    { id = 3, address = "server3.example.com:8001" }
]

[election]
heartbeat_interval_secs = 1
monitor_interval_secs = 1
failure_timeout_secs = 3
election_timeout_secs = 2
```

Repeat for Server 2 and Server 3 with appropriate IDs and addresses.

### 3. Configure Clients

On each client machine:

**Create client configuration** (`config/client_stress.toml`):

```bash
cp config/client_stress_template.toml config/client_stress.toml
```

Edit the server addresses:

```toml
[client]
name = "StressTestClient"
server_addresses = [
    "server1.example.com:8001",
    "server2.example.com:8001",
    "server3.example.com:8001"
]

[requests]
rate_per_second = 1.0
duration_seconds = 300.0
```

**Configure stress test parameters** (`scripts/config/stress_test.conf`):

```bash
# Unique for each machine!
MACHINE_ID=1  # Change to 2, 3, etc. on other machines

NUM_CLIENTS=100
REQUESTS_PER_CLIENT=1000
MIN_DELAY_MS=100
MAX_DELAY_MS=2000
IMAGE_DIRECTORY="./test_images"
CLIENT_CONFIG="./config/client_stress.toml"
METRICS_OUTPUT_DIR="./metrics"
CLIENT_BINARY="./target/release/client"
```

### 4. Configure Fault Simulation

On the controller machine, edit `scripts/config/fault_sim.conf`:

```bash
# Timing
FAULT_INTERVAL_SECS=30    # Time between each server failure
RESTART_DELAY_SECS=10     # How long server stays down
NUM_CYCLES=3              # Number of times to cycle through all servers

# Server 1
SERVER_1_HOST="user@server1.example.com"
SERVER_1_CONFIG="/path/to/CloudP2P/config/server1.toml"
SERVER_1_WORK_DIR="/path/to/CloudP2P"
SERVER_1_BINARY="./target/release/server"

# Server 2
SERVER_2_HOST="user@server2.example.com"
SERVER_2_CONFIG="/path/to/CloudP2P/config/server2.toml"
SERVER_2_WORK_DIR="/path/to/CloudP2P"
SERVER_2_BINARY="./target/release/server"

# Server 3
SERVER_3_HOST="user@server3.example.com"
SERVER_3_CONFIG="/path/to/CloudP2P/config/server3.toml"
SERVER_3_WORK_DIR="/path/to/CloudP2P"
SERVER_3_BINARY="./target/release/server"
```

### 5. Configure Metrics Aggregation

On the controller machine, edit `scripts/config/aggregate.conf`:

```bash
OUTPUT_DIR="./aggregated_metrics"

CLIENT_MACHINES=(
    "user@client1.example.com:/path/to/CloudP2P/metrics"
    "user@client2.example.com:/path/to/CloudP2P/metrics"
    "user@client3.example.com:/path/to/CloudP2P/metrics"
)
```

---

## Running Stress Tests

### Execution Workflow

```
┌──────────────────┐
│ 1. Start Servers │
└────────┬─────────┘
         │
┌────────▼─────────┐
│ 2. Start Clients │ (on each client machine)
└────────┬─────────┘
         │
┌────────▼─────────────┐
│ 3. Start Fault Sim   │ (parallel)
└────────┬─────────────┘
         │
┌────────▼─────────────┐
│ 4. Wait for Complete │
└────────┬─────────────┘
         │
┌────────▼─────────────┐
│ 5. Aggregate Metrics │
└──────────────────────┘
```

### Step 1: Start Servers

On each server machine:

```bash
cd /path/to/CloudP2P

# Start server
./target/release/server --config config/server1.toml
```

Verify servers are running and have elected a leader (check logs).

### Step 2: Start Stress Tests on Client Machines

On **each client machine** (Machine 1, 2, 3), run:

```bash
cd /path/to/CloudP2P

# Make sure MACHINE_ID is unique in stress_test.conf!
# Machine 1: MACHINE_ID=1
# Machine 2: MACHINE_ID=2
# Machine 3: MACHINE_ID=3

./scripts/stress_test.sh
```

The script will:
- Spawn NUM_CLIENTS client processes in parallel
- Each client sends REQUESTS_PER_CLIENT requests
- Random delays between MIN_DELAY_MS and MAX_DELAY_MS
- Write metrics to `metrics/machine_{n}_client_{i}.json`
- Write logs to `metrics/machine_{n}_client_{i}.log`

**Expected output:**

```
==================================
CloudP2P Stress Test Configuration
==================================
Machine ID:         1
Number of Clients:  100
Requests/Client:    1000
Min Delay (ms):     100
Max Delay (ms):     2000
Image Directory:    ./test_images
Client Config:      ./config/client_stress.toml
Metrics Output:     ./metrics
Test Duration:      ~1500 seconds
==================================

Starting 100 clients...

[2025-10-31 10:00:00] Starting client 1/100: Machine_1_Client_1
[2025-10-31 10:00:00] Starting client 2/100: Machine_1_Client_2
...
```

### Step 3: Start Fault Simulation (Parallel)

On the controller machine, **while stress tests are running**:

```bash
cd /path/to/CloudP2P

./scripts/fault_simulation.sh
```

The script will:
- Use SSH to connect to each server
- Kill server processes sequentially (ring algorithm)
- Wait RESTART_DELAY_SECS, then restart the server
- Wait FAULT_INTERVAL_SECS before moving to the next server
- Repeat for NUM_CYCLES

**Expected output:**

```
=========================================
CloudP2P Fault Simulation - Ring Algorithm
=========================================
Number of Servers:   3
Fault Interval:      30s
Restart Delay:       10s
Number of Cycles:    3
Log File:            ./fault_events.log
=========================================

Server 1: user@server1.example.com
  Config:   /path/to/CloudP2P/config/server1.toml
  Work Dir: /path/to/CloudP2P
  Binary:   ./target/release/server
...

[2025-10-31 10:01:00] ===== CYCLE 1/3 =====
[2025-10-31 10:01:00] Ring Algorithm: Processing Server 1
[2025-10-31 10:01:00] Attempting to kill Server 1 on user@server1.example.com
[2025-10-31 10:01:00] Found Server 1 PID: 12345 on user@server1.example.com
[2025-10-31 10:01:00] SUCCESS: Killed Server 1 (PID 12345) on user@server1.example.com
[2025-10-31 10:01:00] Server 1 is now DOWN
[2025-10-31 10:01:00] Waiting 10s before restarting Server 1...
[2025-10-31 10:01:10] Attempting to restart Server 1 on user@server1.example.com
[2025-10-31 10:01:12] SUCCESS: Restarted Server 1 on user@server1.example.com (PID 12389)
[2025-10-31 10:01:12] Server 1 is now UP
[2025-10-31 10:01:12] Waiting 30s before moving to next server in ring...
```

### Step 4: Wait for Completion

Monitor the stress tests on client machines:

```bash
# Check active clients
watch -n 5 'pgrep -f "client --config" | wc -l'

# Watch specific client log
tail -f metrics/machine_1_client_1.log
```

Wait for all clients to complete before proceeding to aggregation.

### Step 5: Aggregate Metrics

On the controller machine, after all clients finish:

```bash
cd /path/to/CloudP2P

./scripts/aggregate_metrics.sh
```

The script will:
- Collect metrics JSON files from all client machines via SCP
- Aggregate all metrics into a single report
- Generate `final_report.json` and `final_report.txt`

**Expected output:**

```
============================================
CloudP2P Metrics Aggregation
============================================
Client Machines: 3
Output Directory: ./aggregated_metrics
============================================

Step 1: Collecting metrics from client machines...

[1/3] Collecting from: user@client1.example.com:/path/to/CloudP2P/metrics
  ✓ Successfully collected metrics from machine 1
[2/3] Collecting from: user@client2.example.com:/path/to/CloudP2P/metrics
  ✓ Successfully collected metrics from machine 2
[3/3] Collecting from: user@client3.example.com:/path/to/CloudP2P/metrics
  ✓ Successfully collected metrics from machine 3

Step 2: Aggregating metrics...

Loading metrics from: ./aggregated_metrics/collected
Loaded 300 client metrics files
Aggregating metrics...
✓ Saved JSON report: ./aggregated_metrics/final_report.json
✓ Saved text report: ./aggregated_metrics/final_report.txt

============================================
Metrics Aggregation Complete!
============================================
```

---

## Results

### View the Report

```bash
cat aggregated_metrics/final_report.txt
```

**Example report:**

```
============================================================
CloudP2P Stress Test - Aggregated Metrics Report
============================================================

OVERALL STATISTICS
------------------------------------------------------------
Total Requests:       300,000
Successful Requests:  297,450
Failed Requests:      2,550
Failure Rate:         0.85%

REQUEST LATENCY (Successful Requests)
------------------------------------------------------------
Minimum:              45.23 ms
Maximum:              8,234.56 ms
Average:              234.67 ms
Median (P50):         198.45 ms
95th Percentile:      456.78 ms
99th Percentile:      789.12 ms

LOAD BALANCING - Server Distribution
------------------------------------------------------------
Server  1:   99,234 requests (33.41%) ████████████████
Server  2:   98,456 requests (33.15%) ████████████████
Server  3:   99,760 requests (33.59%) ████████████████

FAILURE ANALYSIS
------------------------------------------------------------
Connection refused                                :  1,234 ( 48.37%)
Request timeout after 10s                         :    890 ( 34.90%)
Server unavailable during reassignment polling    :    426 ( 16.71%)

============================================================
End of Report
============================================================
```

### Interpreting Results

**Request Latency:**
- Low P50/P95/P99 = Good performance
- High max latency = Potential issues during server failures

**Load Balancing:**
- Even distribution (~33% each for 3 servers) = Good load balancing
- Uneven distribution = Potential leader election or assignment issues

**Failure Rate:**
- <1% = Excellent fault tolerance
- 1-5% = Acceptable with fault simulation
- >5% = Investigate failure reasons

---

## Troubleshooting

### Clients Can't Connect to Servers

**Symptoms:** All requests fail with "Connection refused"

**Solutions:**
1. Verify servers are running: `ssh server1 "pgrep -f server"`
2. Check server addresses in `config/client_stress.toml`
3. Verify firewall allows connections on port 8001
4. Check server logs for errors

### SSH Authentication Fails

**Symptoms:** Fault simulation or metrics collection fails with permission errors

**Solutions:**
1. Verify SSH key is copied: `ssh-copy-id user@host`
2. Test SSH manually: `ssh user@host "echo OK"`
3. Check SSH agent: `ssh-add -l`
4. Ensure correct username in config files

### Metrics Not Generated

**Symptoms:** No JSON files in `metrics/` directory

**Solutions:**
1. Check client logs: `tail metrics/machine_1_client_1.log`
2. Verify `--metrics-output` argument is passed
3. Check disk space: `df -h`
4. Ensure directory permissions: `chmod 755 metrics/`

### Python Aggregation Fails

**Symptoms:** `aggregate_metrics.sh` errors

**Solutions:**
1. Verify Python 3 installed: `python3 --version`
2. Check JSON files are valid: `cat metrics/machine_1_client_1.json | python3 -m json.tool`
3. Ensure collected metrics exist: `ls -la aggregated_metrics/collected/`

### High Failure Rate

**Symptoms:** Failure rate >10%

**Possible causes:**
1. **Servers overloaded**: Reduce NUM_CLIENTS or increase delays
2. **Network issues**: Check network connectivity between machines
3. **Fault simulation too aggressive**: Increase FAULT_INTERVAL_SECS or RESTART_DELAY_SECS
4. **Resource exhaustion**: Monitor CPU/memory on servers

**Debug steps:**
```bash
# Check server resource usage
ssh server1 "top -bn1 | head -20"

# Check network connectivity
ping server1.example.com

# Review client logs for specific errors
grep "ERROR" metrics/machine_1_client_*.log | sort | uniq -c
```

---

## Advanced Configuration

### Adjusting Load

To increase load:
- Increase `NUM_CLIENTS`
- Increase `REQUESTS_PER_CLIENT`
- Decrease `MIN_DELAY_MS` and `MAX_DELAY_MS`

To decrease load:
- Decrease `NUM_CLIENTS`
- Decrease `REQUESTS_PER_CLIENT`
- Increase `MIN_DELAY_MS` and `MAX_DELAY_MS`

### Fault Simulation Patterns

**Current: Ring Algorithm**
- Servers fail sequentially: 1 → 2 → 3 → 1
- Fixed timing

**To implement other patterns**, edit `scripts/fault_simulation.sh`:

**Random failures:**
```bash
# Instead of iterating in order, shuffle the array
SERVER_ORDER=($(shuf -e "${!SERVER_HOSTS[@]}"))
for i in "${SERVER_ORDER[@]}"; do
    # ... kill and restart
done
```

**Simultaneous failures:**
```bash
# Kill multiple servers at once
kill_server "${SERVER_HOSTS[0]}" "${SERVER_CONFIGS[0]}" "${SERVER_IDS[0]}" &
kill_server "${SERVER_HOSTS[1]}" "${SERVER_CONFIGS[1]}" "${SERVER_IDS[1]}" &
wait
```

### Custom Metrics

To track additional metrics, edit `src/client/metrics.rs`:

```rust
pub struct RequestMetric {
    pub request_id: u64,
    pub latency_ms: u64,
    pub success: bool,
    // Add custom fields:
    pub retry_count: u32,
    pub bytes_sent: usize,
    pub bytes_received: usize,
}
```

---

## Summary

You now have a complete stress testing framework for CloudP2P:

1. ✅ Concurrent client load generation
2. ✅ Random delay simulation
3. ✅ Fault injection with ring algorithm
4. ✅ Comprehensive metrics collection
5. ✅ Automated report generation

**Next Steps:**
- Run longer tests (increase duration_seconds)
- Experiment with different failure patterns
- Monitor system resources during tests
- Tune server configuration based on results

For questions or issues, refer to the main CloudP2P documentation.
