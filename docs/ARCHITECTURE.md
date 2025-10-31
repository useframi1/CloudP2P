# CloudP2P Architecture Documentation

This document provides an in-depth technical overview of the CloudP2P distributed image encryption system architecture.

## Table of Contents

- [System Overview](#system-overview)
- [Component Breakdown](#component-breakdown)
- [Data Flow Diagrams](#data-flow-diagrams)
- [Message Protocol](#message-protocol)
- [Wire Protocol Format](#wire-protocol-format)
- [Concurrency Model](#concurrency-model)
- [State Management](#state-management)
- [Fault Tolerance Mechanisms](#fault-tolerance-mechanisms)
- [Load Balancing Algorithm](#load-balancing-algorithm)
- [Task History Tracking](#task-history-tracking)

## System Overview

CloudP2P is a distributed system designed with a layered architecture that separates concerns between core functionality and distributed coordination:

```
┌─────────────────────────────────────────────────────────────┐
│                    Application Layer                        │
│  ┌──────────────────┐              ┌──────────────────┐    │
│  │  Client Binary   │              │  Server Binary   │    │
│  │   (bin/client)   │              │   (bin/server)   │    │
│  └────────┬─────────┘              └────────┬─────────┘    │
│           │                                 │               │
├───────────┼─────────────────────────────────┼───────────────┤
│           │    Coordination Layer           │               │
│  ┌────────▼─────────┐              ┌────────▼─────────┐    │
│  │ ClientMiddleware │              │ServerMiddleware  │    │
│  │  - Leader Disc.  │              │ - Elections      │    │
│  │  - Retry Logic   │              │ - Heartbeats     │    │
│  │  - Fault Tol.    │              │ - Load Balance   │    │
│  └────────┬─────────┘              └────────┬─────────┘    │
│           │                                 │               │
├───────────┼─────────────────────────────────┼───────────────┤
│           │       Core Layer                │               │
│  ┌────────▼─────────┐              ┌────────▼─────────┐    │
│  │   ClientCore     │              │   ServerCore     │    │
│  │  - Send Image    │              │ - Encrypt Image  │    │
│  │  - Recv Result   │              │ - LSB Steg       │    │
│  │  - Verify        │              │ - Save Output    │    │
│  └──────────────────┘              └──────────────────┘    │
│                                                             │
├─────────────────────────────────────────────────────────────┤
│                    Common Layer                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐     │
│  │   Messages   │  │  Connection  │  │    Config    │     │
│  │   Protocol   │  │   TCP Wrapper│  │    TOML      │     │
│  └──────────────┘  └──────────────┘  └──────────────┘     │
│                                                             │
├─────────────────────────────────────────────────────────────┤
│                 Processing Layer                            │
│  ┌──────────────────────────────────────────────────────┐  │
│  │             Steganography (LSB Algorithm)            │  │
│  │  - embed_text_bytes() / extract_text_bytes()        │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

### Design Principles

1. **Separation of Concerns**: Core functionality (encryption) is isolated from coordination logic
2. **Async-First**: Built on tokio for efficient concurrent I/O
3. **Fail-Fast**: Errors propagate quickly with anyhow::Result
4. **Type Safety**: Rust's type system prevents many classes of distributed system bugs
5. **Message Passing**: Components communicate via well-defined message types

## Component Breakdown

### ServerCore

**Location**: `src/server/server.rs`

**Responsibility**: Image encryption using LSB steganography

**Key Methods**:
```rust
pub struct ServerCore {
    server_id: u32,
}

impl ServerCore {
    pub fn new(server_id: u32) -> Self

    pub async fn encrypt_image(
        &self,
        request_id: u64,
        client_name: String,
        image_data: Vec<u8>,
        image_name: String,
        text_to_embed: String,
    ) -> Result<Vec<u8>>
}
```

**Process Flow**:
1. Receive image data and text from middleware
2. Delegate to `spawn_blocking` (CPU-bound work)
3. Call `steganography::embed_text_bytes()`
4. Save encrypted image to `user-data/outputs/`
5. Return encrypted bytes

**Why `spawn_blocking`?**
Steganography is CPU-intensive and would block the async runtime. `spawn_blocking` runs it on a dedicated thread pool.

### ServerMiddleware

**Location**: `src/server/middleware.rs`

**Responsibility**: Distributed coordination, leader election, task distribution

**State**:
```rust
pub struct ServerMiddleware {
    core: Arc<ServerCore>,                          // Encryption service
    config: ServerConfig,                           // TOML config
    metrics: ServerMetrics,                         // CPU/memory/tasks
    current_leader: Arc<RwLock<Option<u32>>>,       // Leader ID
    received_alive: Arc<RwLock<bool>>,              // Election flag
    peer_connections: Arc<RwLock<HashMap<...>>>,    // Peer channels
    last_heartbeat_times: Arc<RwLock<HashMap<...>>>,// Failure detection
    active_tasks: Arc<RwLock<HashMap<...>>>,        // Task handles
    peer_loads: Arc<RwLock<HashMap<...>>>,          // Load tracking
    task_history: Arc<RwLock<HashMap<...>>>,        // Orphan cleanup
}
```

**Concurrent Tasks**:
1. **Listener**: Accept incoming TCP connections
2. **Peer Connector**: Maintain connections to all peers
3. **Heartbeat Sender**: Broadcast load every 1 second
4. **Heartbeat Monitor**: Detect failures every 1 second
5. **Election Timer**: Trigger initial election after 3 seconds

**Message Handlers**:
- `Election`: Compare priority, respond with ALIVE if better
- `Alive`: Mark election as lost
- `Coordinator`: Acknowledge new leader
- `Heartbeat`: Update peer status and load
- `TaskAssignmentRequest`: Assign to least-loaded server
- `TaskRequest`: Process encryption task
- `HistoryAdd`/`HistoryRemove`: Track task lifecycle

### ClientCore

**Location**: `src/client/client.rs`

**Responsibility**: Direct task submission and result verification

**Key Methods**:
```rust
pub struct ClientCore {
    client_name: String,
}

impl ClientCore {
    pub fn new(client_name: String) -> Self

    pub async fn send_and_receive_encrypted_image(
        &self,
        assigned_address: &str,
        request_id: u64,
        image_data: Vec<u8>,
        image_name: &str,
        text_to_embed: &str,
        assigned_by_leader: u32,
    ) -> Result<()>
}
```

**Process Flow**:
1. Connect to assigned server
2. Send `TaskRequest` message
3. Wait for `TaskResponse`
4. Save encrypted image
5. Extract embedded text for verification
6. Compare extracted text with original

**Verification Importance**:
Ensures data integrity and that steganography worked correctly. If verification fails, the client retries.

### ClientMiddleware

**Location**: `src/client/middleware.rs`

**Responsibility**: Leader discovery, retry logic, fault tolerance

**State**:
```rust
pub struct ClientMiddleware {
    config: ClientConfig,
    core: Arc<ClientCore>,
    current_leader: Option<u32>,
}
```

**Request Workflow**:
```
For each request (up to 3 retries):
  1. Discover leader (query all servers with 2s timeout)
  2. Get assignment from leader (TaskAssignmentRequest)
  3. Execute task via ClientCore (10s timeout)
  4. On timeout/failure: wait 5s, retry
```

**Server-Side Failover Strategy**:
- **Initial assignment**: Broadcast TaskAssignmentRequest to all servers, leader responds (polls indefinitely with 2s intervals if no leader)
- **Server failure detection**: Client detects TCP connection failure
- **Reassignment polling**: Client broadcasts TaskStatusQuery to all servers (2s intervals, indefinitely)
- **Reassignment preference**: Client immediately accepts different server, retries same server after 10 polls (20s) if it keeps being returned
- **No retry limit**: Client continues indefinitely until task succeeds

This ensures resilience against server failures without hard client-side retry limits.

## Data Flow Diagrams

### End-to-End Task Flow

```
┌────────┐
│ Client │
└───┬────┘
    │
    │ 1. TaskAssignmentRequest{client:"C1", request_id:42}
    │    (Broadcast to ALL servers)
    ├─────────────┬─────────────┬─────────────┐
    ▼             ▼             ▼             │
┌─────────┐  ┌─────────┐  ┌─────────┐        │
│Server 1 │  │Server 2 │  │Server 3 │        │
│(follower│  │(LEADER) │  │(follower│        │
└─────────┘  └───┬─────┘  └─────────┘        │
                 │                            │
                 │ Check loads:               │
                 │ S1: 25.3                   │
                 │ S2: 18.7 ← lowest          │
                 │ S3: 42.1                   │
                 │                            │
                 │ 2a. HistoryAdd (broadcast) │
                 ├─────────────┬──────────────┴─────────────┐
                 ▼             ▼                            ▼
             Server 1      Server 2                     Server 3
             (tracks)      (tracks)                     (tracks)
                 │
                 │ 2b. TaskAssignmentResponse
                 │     {assigned_server_id: 2,
                 │      assigned_address: "..."}
                 │
┌────────┐       │
│ Client │◄──────┘
└───┬────┘
    │
    │ 3. TaskRequest{image_data, text_to_embed}
    └──────────────────────────────────────────┐
                                               │
                                               ▼
                                        ┌─────────────┐
                                        │  Server 2   │
                                        │             │
                                        │ ┌─────────┐ │
                                        │ │ Core    │ │
                                        │ │ Encrypt │ │
                                        │ └─────────┘ │
                                        └──────┬──────┘
                                               │
                                               │ 4a. Save to disk
                                               │ 4b. TaskResponse
                                               │     {encrypted_image_data}
                                               │
┌────────┐                                     │
│ Client │◄────────────────────────────────────┘
└───┬────┘
    │
    │ 5. Verify extraction
    │    6. Save locally
    │    7. Send TaskAck
    └──────────────────────────────────────────┐
                                               │
                                               ▼
                                        ┌─────────────┐
                                        │  Server 2   │
                                        └──────┬──────┘
                                               │
                                               │ 8. HistoryRemove (broadcast)
                                               ├─────────────┬─────────────┐
                                               ▼             ▼             ▼
                                           Server 1      Server 2      Server 3
                                           (removes)     (removes)     (removes)
    ▼
 SUCCESS
```

### Election Flow

```
Time: T0
┌─────────┐     ┌─────────┐     ┌─────────┐
│Server 1 │     │Server 2 │     │Server 3 │
│Load:25.3│     │Load:18.7│     │Load:42.1│
└────┬────┘     └────┬────┘     └────┬────┘
     │               │               │
     │ 1. Election{from:1, priority:25.3}
     ├───────────────┼───────────────►
     │               │               │
     │               │ 2a. Compare:  │
     │               │ 18.7 < 25.3   │
     │               │ (I'm better)  │
     │               │               │
     │               │ 2b. Compare:  │
     │               │ 42.1 > 25.3   │
     │               │ (Defer)       │
     │               │               │
     │ ◄─────────────┤               │
     │ 3. Alive{from:2}              │
     │               │               │
     │ (Lost)        │ 4. Election{from:2, priority:18.7}
     │               ├───────────────►
     │               │               │
     │               │               │ 5. Compare:
     │               │               │ 42.1 > 18.7
     │               │               │ (Defer)
     │               │               │
     │               │ 6. Wait 2s...│
     │               │ No ALIVE      │
     │               │ (Won!)        │
     │               │               │
     │ ◄─────────────┤               │
     │ 7. Coordinator{leader:2}      │
     │               ├───────────────►
     │               │               │
     │ Acknowledge   │   Leader!     │ Acknowledge
     │               │               │
```

### Heartbeat & Failure Detection

```
Normal Operation:
┌─────────┐                    ┌─────────┐
│Server 1 │                    │Server 2 │
└────┬────┘                    └────┬────┘
     │                              │
     │ Every 1s:                    │
     │ Heartbeat{from:1,load:25.3}  │
     ├──────────────────────────────►
     │                              │
     │ Heartbeat{from:2,load:18.7}  │
     ◄──────────────────────────────┤
     │                              │
     │ Update last_seen[2] = T      │
     │                              │ Update last_seen[1] = T
     │                              │

Server 2 Fails:
┌─────────┐                    ┌─────────┐
│Server 1 │                    │Server 2 │
│         │                    │ CRASHED │
└────┬────┘                    └─────────┘
     │
     │ T+0: last_seen[2] = T0
     │ T+1: Heartbeat expected... missing
     │ T+2: Still missing
     │ T+3: Still missing
     │
     │ Monitor detects:
     │ now - last_seen[2] > 3s
     │
     ├─► Remove peer_loads[2]
     ├─► Remove last_heartbeat[2]
     ├─► Find orphaned tasks assigned to S2
     ├─► Remove from task_history
     │
     │ If S2 was leader:
     ├─► Set current_leader = None
     └─► Initiate election
```

## Message Protocol

### Message Enumeration

All messages are defined in `src/common/messages.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    // Election Messages
    Election { from_id: u32, priority: f64 },
    Alive { from_id: u32 },
    Coordinator { leader_id: u32 },

    // Heartbeat
    Heartbeat { from_id: u32, timestamp: u64, load: f64 },

    // Client-Server Communication
    LeaderQuery,
    LeaderResponse { leader_id: u32 },
    TaskAssignmentRequest { client_name: String, request_id: u64 },
    TaskAssignmentResponse {
        request_id: u64,
        assigned_server_id: u32,
        assigned_server_address: String
    },
    TaskRequest {
        client_name: String,
        request_id: u64,
        image_data: Vec<u8>,
        image_name: String,
        text_to_embed: String,
        assigned_by_leader: u32,
    },
    TaskResponse {
        request_id: u64,
        encrypted_image_data: Vec<u8>,
        success: bool,
        error_message: Option<String>,
    },
    TaskAck {
        client_name: String,
        request_id: u64,
    },
    TaskStatusQuery {
        client_name: String,
        request_id: u64,
    },
    TaskStatusResponse {
        request_id: u64,
        assigned_server_id: u32,
        assigned_server_address: String,
    },

    // Task History (Fault Tolerance)
    HistoryAdd {
        client_name: String,
        request_id: u64,
        assigned_server_id: u32,
        timestamp: u64,
    },
    HistoryRemove { client_name: String, request_id: u64 },
}
```

### Message Serialization

Messages are serialized using `serde_json`:

```rust
impl Message {
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec(self)?)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        Ok(serde_json::from_slice(bytes)?)
    }
}
```

**Example JSON**:
```json
{
  "Election": {
    "from_id": 1,
    "priority": 25.3
  }
}

{
  "TaskRequest": {
    "client_name": "Client1",
    "request_id": 42,
    "image_data": [255, 216, 255, ...],
    "image_name": "photo.jpg",
    "text_to_embed": "username:alice,views:5",
    "assigned_by_leader": 2
  }
}
```

## Wire Protocol Format

### Connection Wrapper

**Location**: `src/common/connection.rs`

The `Connection` struct wraps a `TcpStream` and provides message-based I/O:

```rust
pub struct Connection {
    stream: TcpStream,
}

impl Connection {
    pub fn new(stream: TcpStream) -> Self

    pub async fn write_message(&mut self, msg: &Message) -> Result<()>
    pub async fn read_message(&mut self) -> Result<Option<Message>>
}
```

### Wire Format

```
┌──────────────────┬────────────────────────────────────┐
│  Length Prefix   │         JSON Message               │
│   (4 bytes)      │         (N bytes)                  │
│   Big Endian     │                                    │
└──────────────────┴────────────────────────────────────┘

Byte Layout:
[0-3]:   u32 length (big endian)
[4-N+3]: JSON bytes
```

**Write Process**:
1. Serialize message to JSON
2. Compute length `N`
3. Write 4-byte length prefix (big endian)
4. Write `N` bytes of JSON

**Read Process**:
1. Read 4-byte length prefix
2. Parse as big-endian u32
3. Read exactly `N` bytes
4. Deserialize JSON to Message

**Example**:
```
Message: {"Heartbeat":{"from_id":1,"timestamp":1234567890,"load":25.3}}
JSON Length: 63 bytes
Wire bytes: [0x00, 0x00, 0x00, 0x3F, 0x7B, 0x22, 0x48, ...]
             └─────┬─────┘  └────────┬────────────────...
                   │                 │
             Length: 63         JSON payload
```

## Concurrency Model

### Tokio Async Runtime

CloudP2P uses **tokio** for asynchronous I/O:

```rust
#[tokio::main]
async fn main() -> Result<()> {
    // All networking is async
    let listener = TcpListener::bind("127.0.0.1:8001").await?;

    loop {
        let (socket, _) = listener.accept().await?;
        tokio::spawn(handle_connection(socket));
    }
}
```

**Why Async?**
- Handles thousands of concurrent connections
- Non-blocking I/O prevents thread exhaustion
- Lightweight tasks (not OS threads)

### Thread Pools

**Tokio Runtime**: Default multi-threaded scheduler with work-stealing

**Blocking Pool**: For CPU-intensive steganography:
```rust
let result = tokio::task::spawn_blocking(move || {
    steganography::embed_text_bytes(&image_data, &text)
}).await??;
```

### Shared State Patterns

**Arc (Atomic Reference Counting)**:
```rust
let core = Arc::new(ServerCore::new(1));
let core_clone = core.clone();  // Cheap pointer copy

tokio::spawn(async move {
    core_clone.encrypt_image(...).await;
});
```

**RwLock (Reader-Writer Lock)**:
```rust
let current_leader: Arc<RwLock<Option<u32>>> = Arc::new(RwLock::new(None));

// Read (multiple concurrent readers)
let leader = *current_leader.read().await;

// Write (exclusive access)
*current_leader.write().await = Some(2);
```

**AtomicU64 (Lock-Free Counter)**:
```rust
let active_tasks: Arc<AtomicU64> = Arc::new(AtomicU64::new(0));

// Increment atomically
active_tasks.fetch_add(1, Ordering::Relaxed);

// Read atomically
let count = active_tasks.load(Ordering::Relaxed);
```

### Message Passing

**MPSC Channels (Multi-Producer, Single-Consumer)**:
```rust
let (tx, mut rx) = mpsc::channel::<Message>(100);

// Send from anywhere
tx.send(Message::Heartbeat { ... }).await?;

// Receive on dedicated task
while let Some(msg) = rx.recv().await {
    handle_message(msg).await;
}
```

**Usage in Peer Connections**:
```rust
// One channel per peer
peer_connections: HashMap<u32, mpsc::Sender<Message>>

// Broadcast to all peers
for (peer_id, tx) in peer_connections {
    tx.send(message.clone()).await?;
}
```

## State Management

### Server State

**Leader State**:
```rust
current_leader: Arc<RwLock<Option<u32>>>

States:
- None: No leader known (during startup or after leader failure)
- Some(id): Server `id` is the current leader
```

**Election State**:
```rust
received_alive: Arc<RwLock<bool>>

States:
- false: No higher-priority server responded (we won)
- true: At least one server responded with ALIVE (we lost)
```

**Peer Connection State**:
```rust
peer_connections: Arc<RwLock<HashMap<u32, mpsc::Sender<Message>>>>

States per peer:
- Not in map: No connection established
- Some(tx): Connected, can send messages via channel
```

**Heartbeat State**:
```rust
last_heartbeat_times: Arc<RwLock<HashMap<u32, u64>>>

States per peer:
- Not in map: No heartbeat received yet
- Some(timestamp): Last heartbeat at this Unix timestamp
```

**Load State**:
```rust
peer_loads: Arc<RwLock<HashMap<u32, f64>>>

States per peer:
- Not in map: Load unknown (no heartbeat yet)
- Some(load): Last reported load score
```

### Client State

**Leader Cache**:
```rust
current_leader: Option<u32>

States:
- None: No leader discovered yet
- Some(id): Server `id` is believed to be the leader (may be stale)
```

**Note**: Client rediscovers leader on every retry for freshness.

## Fault Tolerance Mechanisms

### Failure Detection

**Heartbeat-Based Detection**:
1. Each server broadcasts heartbeat every 1 second
2. Receivers update `last_heartbeat_times[peer_id] = now`
3. Monitor task checks every 1 second:
   ```rust
   if now - last_heartbeat_times[peer_id] > 3 seconds {
       // Peer failed!
   }
   ```

**Timeout Configuration**:
```toml
[election]
heartbeat_interval_secs = 1   # Send frequency
failure_timeout_secs = 3      # Detection threshold
monitor_interval_secs = 1     # Check frequency
```

**False Positive Rate**:
- Network delay < 1s: No false positives
- Network delay 1-3s: Possible detection delay
- Network delay > 3s: Permanent false positive (partition)

### Leader Failure Recovery

**Detection**:
```rust
if Some(failed_peer_id) == current_leader {
    warn!("LEADER {} failed! Starting election...", failed_peer_id);
    *current_leader.write().await = None;
    initiate_election().await;
}
```

**Timeline**:
```
T+0s: Leader stops sending heartbeats
T+3s: Peers detect leader failure
T+3s: Peers initiate elections
T+5s: New leader elected and announced
T+6s: Clients discover new leader

Total failover time: ~3-6 seconds
```

### Orphaned Task Cleanup

**Problem**: Server S2 fails while processing tasks assigned by the leader.

**Solution**: Task history tracking

**Implementation**:
```rust
// When leader assigns task:
task_history.insert(
    (client_name, request_id),
    TaskHistoryEntry {
        assigned_server_id: 2,
        timestamp: now,
    }
);

// Broadcast to all servers
broadcast(Message::HistoryAdd { ... });

// When server S2 detected as failed:
let orphaned_tasks: Vec<_> = task_history
    .iter()
    .filter(|(_, entry)| entry.assigned_server_id == 2)
    .map(|(key, _)| key.clone())
    .collect();

for key in orphaned_tasks {
    task_history.remove(&key);
    info!("Removed orphaned task: {:?}", key);
}
```

**Client Recovery**:
- Client timeout (10s) triggers retry
- Client rediscovers leader
- Client gets new assignment (possibly different server)

### Client Server-Side Failover Logic

**Configuration**:
```rust
const POLL_INTERVAL_SECS: u64 = 2;
const MAX_SAME_SERVER_POLLS: u32 = 10;  // 20 seconds of same server before retry
const CONNECTION_TIMEOUT_SECS: u64 = 2;
```

**Process**:
```
Initial Assignment:
  1. Broadcast TaskAssignmentRequest to ALL servers (2s timeout each)
  2. Leader responds with TaskAssignmentResponse
  → If no leader: poll every 2s indefinitely until leader available

Task Execution:
  1. Connect to assigned server
  2. Send TaskRequest with image data
  3. Receive TaskResponse
  4. Send TaskAck to confirm receipt
  5. Verify encryption locally

Server Failure During Execution:
  1. Detect TCP connection failure
  2. Broadcast TaskStatusQuery to ALL servers (2s timeout each, polls indefinitely)
  3. Any server responds with current TaskStatusResponse from shared history
  4. If different server: accept immediately and retry task
  5. If same server (10 consecutive polls): retry anyway (server may have recovered)
  6. Continue until task succeeds

Key Features:
- NO retry limit: client continues indefinitely
- Server-side reassignment: leader reassigns orphaned tasks to healthy servers
- At-least-once delivery: TaskAck ensures tasks not removed from history prematurely
```

**Resilience**:
- Client never gives up on tasks
- Automatic server-side reassignment
- Task history ensures no duplicate work

## Load Balancing Algorithm

### Priority Calculation

**Location**: `src/server/election.rs`

```rust
pub fn calculate_priority(&self) -> f64 {
    const W_CPU: f64 = 0.5;     // 50% weight
    const W_TASKS: f64 = 0.3;   // 30% weight
    const W_MEMORY: f64 = 0.2;  // 20% weight

    let cpu_usage = self.get_cpu_usage();  // 0-100%
    let active_tasks = self.get_active_tasks() as f64;
    let memory_available = self.get_available_memory_percent();

    // Normalize tasks: 10 concurrent = 100%
    let tasks_normalized = (active_tasks / 10.0).min(1.0) * 100.0;

    // Memory score: lower available = higher score
    let memory_score = 100.0 - memory_available;

    // Composite score (lower = better)
    W_CPU * cpu_usage + W_TASKS * tasks_normalized + W_MEMORY * memory_score
}
```

**Examples**:

| Server | CPU | Tasks | Memory Avail | Priority Score |
|--------|-----|-------|--------------|----------------|
| S1     | 10% | 0     | 90%          | 12.0 (best)    |
| S2     | 40% | 3     | 70%          | 35.0           |
| S3     | 80% | 8     | 30%          | 78.0 (worst)   |

### Task Assignment

**Leader-Based Assignment**:
```rust
async fn handle_task_assignment_request(&self, request_id: u64) {
    // Get own load
    let my_load = self.metrics.get_load();

    // Get peer loads (from heartbeats)
    let peer_loads = self.peer_loads.read().await;

    // Find minimum
    let mut lowest_load = my_load;
    let mut best_server = self.config.server.id;

    for (peer_id, peer_load) in peer_loads.iter() {
        if *peer_load < lowest_load {
            lowest_load = *peer_load;
            best_server = *peer_id;
        }
    }

    // Could assign to self!
    info!("Assigning task {} to Server {} (load: {:.2})",
          request_id, best_server, lowest_load);

    // Return assignment to client
}
```

**Load Updates**:
```rust
// When task starts
metrics.task_started();  // Increments active_tasks

// When task completes
metrics.task_finished();  // Decrements active_tasks

// Heartbeat broadcasts updated load
let load = metrics.get_load();
broadcast(Message::Heartbeat { from_id, timestamp, load });
```

**Freshness**: Load updates propagated within 1 second (heartbeat interval)

## Task History Tracking

### Purpose

Track which server is responsible for each task to enable orphaned task cleanup when servers fail.

### Data Structure

```rust
task_history: Arc<RwLock<HashMap<(String, u64), TaskHistoryEntry>>>

struct TaskHistoryEntry {
    _client_name: String,         // For logging
    _request_id: u64,             // For logging
    assigned_server_id: u32,      // Critical for cleanup
    _timestamp: u64,              // When assigned
}

// Key: (client_name, request_id)
// Value: Which server is responsible
```

### Lifecycle

**1. Task Assignment (Leader)**:
```rust
// Leader assigns task to Server 2
let entry = TaskHistoryEntry {
    _client_name: "Client1".to_string(),
    _request_id: 42,
    assigned_server_id: 2,
    _timestamp: current_timestamp(),
};

// Add to own history
task_history.insert(("Client1".to_string(), 42), entry);

// Broadcast to all servers
broadcast(Message::HistoryAdd {
    client_name: "Client1".to_string(),
    request_id: 42,
    assigned_server_id: 2,
    timestamp: now,
});
```

**2. Task Completion (Assigned Server)**:
```rust
// Server 2 finishes task
broadcast(Message::HistoryRemove {
    client_name: "Client1".to_string(),
    request_id: 42,
});

// All servers remove from history
task_history.remove(&("Client1".to_string(), 42));
```

**3. Server Failure (All Servers)**:
```rust
// Server 2 fails, monitor detects it
let orphaned_tasks: Vec<_> = task_history
    .iter()
    .filter(|(_, entry)| entry.assigned_server_id == 2)
    .map(|(key, _)| key.clone())
    .collect();

for key in orphaned_tasks {
    task_history.remove(&key);
    warn!("Removed orphaned task: {:?}", key);
}
```

### Why Broadcast History?

**Problem**: If only the leader tracks history and the leader fails, orphaned task information is lost.

**Solution**: All servers maintain identical task history via broadcast updates.

**Consistency**: Eventually consistent (within 1 RTT of broadcast)

**Trade-off**:
- **Pro**: Survives leader failure
- **Con**: Memory overhead (small: ~100 bytes per task)

---

## Summary

CloudP2P's architecture demonstrates several distributed systems patterns:

1. **Separation of Concerns**: Core vs. Middleware layers
2. **Leader Election**: Modified Bully with dynamic priority
3. **Failure Detection**: Heartbeat-based with configurable timeouts
4. **Fault Tolerance**: Task history, client retry, leader failover
5. **Load Balancing**: Real-time metrics and greedy assignment
6. **Async I/O**: Tokio for scalable concurrent connections
7. **Message Passing**: Channels and well-defined protocol

These patterns combine to create a resilient, performant distributed system for image encryption.

For algorithm details, see [`ALGORITHM.md`](/home/g03-s2025/Desktop/CloudP2P/docs/ALGORITHM.md).
