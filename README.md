# CloudP2P - Distributed Image Encryption Service

A fault-tolerant, load-balanced distributed system for image encryption using steganography. The system implements a Modified Bully Algorithm for leader election and provides automatic failover capabilities.

## Table of Contents

- [Features](#features)
- [Architecture Overview](#architecture-overview)
- [Quick Start](#quick-start)
- [Configuration](#configuration)
- [How It Works](#how-it-works)
- [File Structure](#file-structure)
- [Technical Details](#technical-details)
- [Testing](#testing)
- [Troubleshooting](#troubleshooting)
- [Performance Notes](#performance-notes)

## Features

- **Distributed Leader Election**: Modified Bully Algorithm based on dynamic server load
- **Load Balancing**: Automatic task assignment to least-loaded servers
- **Fault Tolerance**: Automatic failure detection and recovery
- **Steganography**: LSB (Least Significant Bit) text embedding in images
- **Heartbeat Monitoring**: Continuous health checking of all servers
- **Task History Tracking**: Orphaned task cleanup when servers fail
- **Server-Side Failover**: Automatic server reassignment with indefinite client polling
- **Concurrent Processing**: Asynchronous task handling with tokio

## Architecture Overview

```
                         CloudP2P System

   +----------+      +----------+      +----------+
   | Client 1 |      | Client 2 |      | Client N |
   +-----+----+      +-----+----+      +-----+----+
         |                 |                 |
         |    1. Discover Leader             |
         +----------------+------------------+
                          |
                          v
                  +-------+--------+
                  | LEADER SERVER  |
                  +-------+--------+
                  | - Modified     |
                  |   Bully        |
                  | - Load         |
                  |   Balancing    |
                  | - Task         |
                  |   Assignment   |
                  +-------+--------+
                          |
                          | 2. Get Assignment
                          v
         +----------------+------------------+
         |                |                  |
    +----+----+      +----+----+      +-----+-----+
    | Server1 |      | Server2 |      | Server 3  |
    | Load:   |      | Load:   |      | Load:     |
    | 25.3    |      | 18.7    |      | 42.1      |
    +----+----+      +----+----+      +-----+-----+
         |                |                  |
         <-------Heartbeats (1s)------------->
         |                |                  |
         |       3. Execute Task             |
         v                                   |
    ServerCore:                              |
    Steganography                            |
    - Embed text in image LSBs               |
    - Save encrypted image                   |
         |                                   |
         |       4. Return Result            |
         +-----------------------------------+
                          |
                          v
                     +--------+
                     | Client |
                     +--------+
```

**Key Components:**
- **Clients**: Submit tasks, discover leader, receive encrypted images
- **Leader**: Assigns tasks based on server loads, maintains cluster state
- **Servers**: Process encryption tasks, participate in elections, send heartbeats
- **Modified Bully Algorithm**: Elects least-loaded server as leader

## Quick Start

### Installation

1. **Prerequisites**:
   ```bash
   # Install Rust (if not already installed)
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

   # Verify installation
   rustc --version
   cargo --version
   ```

2. **Build the project**:
   ```bash
   cd /home/g03-s2025/Desktop/CloudP2P
   cargo build --release
   ```

3. **Prepare directories**:
   ```bash
   # Ensure user-data directories exist
   mkdir -p user-data/uploads
   mkdir -p user-data/outputs

   # Add a test image
   cp your_image.jpg user-data/uploads/test_image.jpg
   ```

### Running Servers

Start three servers in separate terminals:

**Terminal 1 - Server 1:**
```bash
RUST_LOG=info cargo run --bin server -- --config config/server1.toml
```

**Terminal 2 - Server 2:**
```bash
RUST_LOG=info cargo run --bin server -- --config config/server2.toml
```

**Terminal 3 - Server 3:**
```bash
RUST_LOG=info cargo run --bin server -- --config config/server3.toml
```

You should see election messages and one server becoming the leader:
```
[INFO] Server 1 initiating election
[INFO] Server 1 priority: 15.23 (CPU: 5.1%, Tasks: 0, Memory: 85.3% available)
[INFO] Server 1 won election! (lowest priority score: 15.23)
[INFO] Server 2 acknowledges 1 as LEADER
```

### Running Clients

Start a client in a new terminal:

**Terminal 4 - Client 1:**
```bash
RUST_LOG=info cargo run --bin client -- --config config/client1.toml
```

The client will:
1. Broadcast assignment request (leader responds with server assignment)
2. Send image to assigned server
3. Receive encrypted image
4. Send acknowledgment (TaskAck)
5. Verify encryption

## Configuration

### Server Configuration

Example `config/server1.toml`:

```toml
[server]
id = 1
address = "127.0.0.1:8001"

[peers]
peers = [
    { id = 2, address = "127.0.0.1:8002" },
    { id = 3, address = "127.0.0.1:8003" }
]

[election]
heartbeat_interval_secs = 1      # Send heartbeat every 1 second
election_timeout_secs = 2        # Wait 2 seconds for election responses
failure_timeout_secs = 3         # Consider peer failed after 3 seconds
monitor_interval_secs = 1        # Check for failures every 1 second

[encryption]
thread_pool_size = 4             # Number of worker threads
max_queue_size = 100             # Maximum queued tasks
```

**Configuration Parameters:**
- `server.id`: Unique server identifier (1, 2, 3, ...)
- `server.address`: IP:port for this server
- `peers`: List of other servers in the cluster
- `heartbeat_interval_secs`: How often to send heartbeats
- `election_timeout_secs`: How long to wait for election responses
- `failure_timeout_secs`: No heartbeat = server failed
- `monitor_interval_secs`: How often to check for failures

### Client Configuration

Example `config/client1.toml`:

```toml
[client]
name = "Client1"
server_addresses = [
    "127.0.0.1:8001",
    "127.0.0.1:8002",
    "127.0.0.1:8003"
]

[requests]
rate_per_second = 0.1            # Send 0.1 requests per second (1 every 10s)
duration_seconds = 60.0          # Run for 60 seconds
request_processing_ms = 50000    # Simulated processing time
load_per_request = 0.1           # Simulated load per request
```

**Configuration Parameters:**
- `client.name`: Unique client identifier
- `server_addresses`: List of servers to query for leader
- `rate_per_second`: Request rate (requests/second)
- `duration_seconds`: How long to send requests
- `request_processing_ms`: Simulated processing delay
- `load_per_request`: Simulated load value

## How It Works

### Modified Bully Algorithm

Unlike the classic Bully Algorithm that uses static server IDs, CloudP2P uses **dynamic load-based priority**:

**Priority Formula:**
```
priority = 0.5 * CPU_usage + 0.3 * normalized_tasks + 0.2 * memory_used

Where:
- CPU_usage: 0-100% from system metrics
- normalized_tasks: (active_tasks / 10) * 100, capped at 100%
- memory_used: 100% - available_memory_percent
```

**Lower scores indicate better candidates** (less loaded servers).

**Election Process:**
1. Server initiates election, broadcasts priority
2. Servers with lower priority respond with ALIVE
3. If no ALIVE received, server wins and broadcasts COORDINATOR
4. If ALIVE received, server defers to the better candidate
5. All servers acknowledge the new leader

**Example:**
```
Server 1: CPU 20%, Tasks 2, Memory 80% available -> priority = 20.0
Server 2: CPU 40%, Tasks 5, Memory 60% available -> priority = 43.0
Server 3: CPU 10%, Tasks 0, Memory 90% available -> priority = 12.0

Winner: Server 3 (lowest score = least loaded)
```

### Task Processing Flow

```
1. Client -> All Servers (broadcast): TaskAssignmentRequest(task ID: 42)
   Leader checks load of all servers:
   - Server 1: load = 25.3
   - Server 2: load = 18.7 (lowest)
   - Server 3: load = 42.1
   Leader -> Client: TaskAssignmentResponse(Server 2, 127.0.0.1:8002)
   Leader -> All Servers (broadcast): HistoryAdd(task assigned to Server 2)

2. Client -> Server 2: TaskRequest(image_data, text_to_embed)
   Server 2 -> ServerCore: encrypt_image()
   - Embeds text into image LSBs
   - Saves encrypted image to disk
   Server 2 -> Client: TaskResponse(encrypted_image_data)

3. Client -> Server 2: TaskAck(task ID: 42)
   Server 2 -> All Servers (broadcast): HistoryRemove(task 42 completed)

4. Client verifies encryption by extracting text
   If text matches -> Success
```

### Fault Tolerance Mechanisms

**Failure Detection:**
- Servers send heartbeats every 1 second
- If no heartbeat for 3 seconds -> server considered failed
- Leader failure triggers immediate re-election

**Orphaned Task Reassignment:**
1. All servers track task assignments via shared history: `(client, task_id) -> server_id`
2. When server fails, surviving servers detect timeout (3+ seconds without heartbeat)
3. New leader (or existing leader) automatically reassigns orphaned tasks to healthy servers
4. Clients poll for updated assignment using TaskStatusQuery (broadcast to all servers)

**Client Failover Logic:**
- Client broadcasts TaskAssignmentRequest, waits for leader response (polls indefinitely with 2s intervals if no leader)
- If assigned server fails during task execution, client polls all servers for reassignment (2s intervals, indefinitely)
- Client preferentially accepts reassignment to different server
- If same server keeps being returned after 10 polls (20s), client retries (server may have recovered)
- No hard failure limit - client continues indefinitely until task succeeds

### Load Balancing Algorithm

The leader maintains a real-time view of cluster load:

```rust
// Updated via heartbeats
peer_loads: HashMap<server_id, load_score>

// On task assignment:
fn assign_task() -> server_id {
    let mut lowest_load = my_load;
    let mut best_server = my_id;

    for (peer_id, peer_load) in peer_loads {
        if peer_load < lowest_load {
            lowest_load = peer_load;
            best_server = peer_id;
        }
    }

    return best_server;  // Could be self!
}
```

**Load updates:**
- Incremented when task starts
- Decremented when task completes
- Broadcast via heartbeats every 1 second

## File Structure

```
CloudP2P/
├── Cargo.toml                  # Project dependencies and metadata
├── README.md                   # This file
│
├── src/
│   ├── lib.rs                  # Library root
│   ├── bin/
│   │   ├── server.rs           # Server binary entry point
│   │   └── client.rs           # Client binary entry point
│   │
│   ├── server/
│   │   ├── mod.rs              # Server module exports
│   │   ├── server.rs           # ServerCore: image encryption
│   │   ├── middleware.rs       # ServerMiddleware: coordination
│   │   └── election.rs         # ServerMetrics: priority calculation
│   │
│   ├── client/
│   │   ├── mod.rs              # Client module exports
│   │   ├── client.rs           # ClientCore: task submission
│   │   └── middleware.rs       # ClientMiddleware: retry & discovery
│   │
│   ├── common/
│   │   ├── mod.rs              # Common module exports
│   │   ├── messages.rs         # Message protocol definitions
│   │   ├── connection.rs       # TCP connection wrapper
│   │   └── config.rs           # Configuration structures
│   │
│   └── processing/
│       ├── mod.rs              # Processing module exports
│       └── steganography.rs    # LSB steganography implementation
│
├── config/
│   ├── server1.toml            # Server 1 configuration
│   ├── server2.toml            # Server 2 configuration
│   ├── server3.toml            # Server 3 configuration
│   ├── client1.toml            # Client 1 configuration
│   ├── client2.toml            # Client 2 configuration
│   └── test/                   # Test configurations
│
├── tests/
│   ├── README.md               # Test documentation
│   ├── smoke_test.sh           # Basic functionality test
│   ├── integration_test.sh     # Full integration test
│   └── distributed_test.sh     # Multi-node test
│
├── docs/
│   ├── ARCHITECTURE.md         # Detailed architecture docs
│   └── ALGORITHM.md            # Algorithm explanation
│
└── user-data/
    ├── uploads/                # Input images
    └── outputs/                # Encrypted images
```

## Technical Details

### Dependencies

```toml
tokio = { version = "1.35", features = ["full"] }  # Async runtime
serde = { version = "1.0", features = ["derive"] }  # Serialization
serde_json = "1.0"                                  # JSON encoding
toml = "0.8"                                        # Config parsing
image = "0.24"                                      # Image processing
sysinfo = "0.32"                                    # System metrics
uuid = { version = "1.6", features = ["v4"] }       # Unique IDs
anyhow = "1.0"                                      # Error handling
```

### Message Protocol

Messages are JSON-serialized with a 4-byte length prefix:

```
Wire Format:
+------------+------------------+
| Length (4) | JSON Message (N) |
| bytes      | bytes            |
+------------+------------------+

Example:
Length: [0x00, 0x00, 0x00, 0x2A] (42 bytes)
Message: {"Election":{"from_id":1,"priority":25.3}}
```

**Message Types:**
- `Election`: Start election with priority
- `Alive`: Response to election
- `Coordinator`: Announce new leader
- `Heartbeat`: Periodic health check with load
- `LeaderQuery`: Request current leader (optional, not used in current implementation)
- `LeaderResponse`: Return leader ID
- `TaskAssignmentRequest`: Request server assignment (broadcast to all servers)
- `TaskAssignmentResponse`: Return assigned server (leader responds)
- `TaskRequest`: Submit encryption task
- `TaskResponse`: Return encrypted image
- `TaskAck`: Client acknowledges receipt of TaskResponse
- `TaskStatusQuery`: Query current server assignment for a task (broadcast)
- `TaskStatusResponse`: Return current server assignment (any server can respond)
- `HistoryAdd`: Track task assignment (broadcast to all servers)
- `HistoryRemove`: Remove completed task (broadcast to all servers)

### Steganography Implementation

**LSB (Least Significant Bit) Algorithm:**

1. **Encoding**:
   - Prefix text with 4-byte length
   - For each bit of data:
     - Get next pixel RGB channel
     - Clear LSB: `channel & 0xFE`
     - Set LSB to data bit: `channel | bit`
   - Output as PNG format

2. **Decoding**:
   - Extract first 32 bits (length)
   - Extract next N bits (text data)
   - Convert bits -> bytes -> UTF-8 string

3. **Capacity**:
   - 3 bits per pixel (RGB channels)
   - Example: 800x600 image = 1,440,000 bits = 180 KB capacity

### Concurrency Model

**Tokio Async Runtime:**
- All I/O operations are async
- Non-blocking TCP connections
- Concurrent task processing

**Shared State:**
- `Arc<RwLock<T>>`: Thread-safe shared state
- `Arc<AtomicU64>`: Lock-free counters
- `mpsc::channel`: Message passing between tasks

**Task Spawning:**
- `spawn_blocking`: CPU-intensive steganography
- `tokio::spawn`: Async I/O tasks

## Testing

CloudP2P includes comprehensive test suites. See `tests/README.md` for detailed testing instructions.

**Quick Test:**
```bash
cd tests
./smoke_test.sh
```

**Full Integration Test:**
```bash
cd tests
./integration_test.sh
```

**Test Scenarios:**
- Basic connectivity
- Leader election
- Task processing
- Server failure and recovery
- Load balancing
- Concurrent clients
- Network partitions

## Troubleshooting

### Server won't start

**Problem:** `Address already in use`
```
[ERROR] Failed to bind to 127.0.0.1:8001: Address already in use
```

**Solution:**
```bash
# Find process using the port
lsof -i :8001

# Kill the process
kill -9 <PID>

# Or use a different port in config
```

### No leader elected

**Problem:** Servers running but no leader
```
[WARN] No leader found for task #1 on attempt 1/3
```

**Solution:**
1. Check all servers are running and connected
2. Check firewall isn't blocking connections
3. Verify peer addresses in config files
4. Look for election timeout messages in server logs

### Client can't connect

**Problem:** Client times out
```
[WARN] Task #1 timed out after 10s on attempt 1/3
```

**Solution:**
1. Verify servers are running: `ps aux | grep server`
2. Check server addresses in client config
3. Increase timeout in client code if network is slow
4. Check server logs for errors

### Encryption fails

**Problem:** Image too small
```
[ERROR] Image too small for this text: need 512 bits but only have 480 bits available
```

**Solution:**
1. Use a larger image (minimum ~100x100 pixels)
2. Reduce text to embed
3. Image capacity = (width * height * 3) / 8 bytes

### Build errors

**Problem:** Compilation fails
```
error[E0433]: failed to resolve: use of undeclared crate or module
```

**Solution:**
```bash
# Update dependencies
cargo update

# Clean and rebuild
cargo clean
cargo build --release
```

## Performance Notes

### Typical Performance

**Single Server:**
- Image processing: ~50-100ms per 800x600 image
- Throughput: ~10-20 images/second

**3-Server Cluster:**
- Load balanced throughput: ~30-50 images/second
- Election time: ~2-3 seconds
- Failover time: ~3-5 seconds (heartbeat timeout)

### Optimization Tips

1. **Increase thread pool**: Edit `config/server.toml`
   ```toml
   [encryption]
   thread_pool_size = 8  # Increase for more parallelism
   ```

2. **Tune heartbeat intervals**: Balance responsiveness vs overhead
   ```toml
   [election]
   heartbeat_interval_secs = 2  # Reduce network traffic
   ```

3. **Adjust client rate**: Control request rate
   ```toml
   [requests]
   rate_per_second = 5.0  # Increase for stress testing
   ```

### Scalability

- **Tested**: Up to 10 servers, 20 concurrent clients
- **Bottleneck**: Leader task assignment (sequential)
- **Future improvement**: Distributed hash table for server selection

---

## Additional Resources

- **Architecture Details**: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
- **Algorithm Explanation**: [docs/ALGORITHM.md](docs/ALGORITHM.md)
- **Testing Guide**: [tests/README.md](tests/README.md)

## License

This project is for educational purposes.

## Contributors

G03-S2025 Team
