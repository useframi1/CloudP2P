# Modified Bully Algorithm - CloudP2P Implementation

This document provides a comprehensive explanation of the Modified Bully Algorithm used for leader election in CloudP2P, including how it differs from the classic algorithm, detailed examples, and edge case handling.

## Table of Contents

- [Algorithm Overview](#algorithm-overview)
- [Differences from Classic Bully Algorithm](#differences-from-classic-bully-algorithm)
- [Priority Calculation Formula](#priority-calculation-formula)
- [Election Flow Step-by-Step](#election-flow-step-by-step)
- [Heartbeat Protocol](#heartbeat-protocol)
- [Failure Detection Mechanism](#failure-detection-mechanism)
- [Re-election Triggers](#re-election-triggers)
- [Edge Cases and Handling](#edge-cases-and-handling)
- [Performance Characteristics](#performance-characteristics)
- [Example Scenarios](#example-scenarios)

## Algorithm Overview

The **Modified Bully Algorithm** is a leader election algorithm that selects the most suitable server to coordinate cluster operations. Unlike the classic Bully Algorithm which uses static server IDs, CloudP2P's implementation uses **dynamic load-based priority** to elect the **least-loaded server** as the leader.

### Key Properties

- **Safety**: At most one leader at any time
- **Liveness**: Eventually elects a leader if at least one server is alive
- **Optimality**: Elects the least-loaded server (lowest priority score)
- **Fault Tolerance**: Automatically re-elects if leader fails

### Why Modified?

The classic Bully Algorithm always elects the highest-ID server, which may be overloaded. Our modification elects the server with the **lowest load**, improving cluster performance through intelligent load distribution.

## Differences from Classic Bully Algorithm

### Classic Bully Algorithm

```
Servers: S1, S2, S3 (IDs: 1, 2, 3)
Election Rule: Highest ID wins

Process:
1. S1 starts election
2. S2 and S3 respond "I have higher ID"
3. S3 wins (highest ID)
4. S3 announces itself as coordinator

Problem: S3 might be overloaded!
```

### Modified Bully Algorithm (CloudP2P)

```
Servers: S1, S2, S3
Priority: Based on current load (CPU, tasks, memory)

Process:
1. S1 calculates priority: 25.3
2. S1 broadcasts Election{from:1, priority:25.3}
3. S2 (priority:18.7) responds Alive (better than 25.3)
4. S3 (priority:42.1) defers (worse than 25.3)
5. S2 starts its own election
6. S2 wins (lowest priority = least loaded)
7. S2 announces Coordinator{leader:2}

Benefit: Least-loaded server becomes leader!
```

### Comparison Table

| Feature | Classic Bully | Modified Bully (CloudP2P) |
|---------|---------------|---------------------------|
| **Election Criteria** | Highest static ID | Lowest dynamic load |
| **Priority** | Server ID (fixed) | CPU + Tasks + Memory (dynamic) |
| **Comparison** | ID1 > ID2 | Load1 < Load2 (lower is better) |
| **Winner** | Highest ID | Lowest load |
| **Adaptability** | Static | Adapts to current conditions |
| **Load Balancing** | None | Built-in |

### Key Insight

> **Classic**: "The server with ID 3 is always the leader"
>
> **Modified**: "The server with the least load becomes the leader"

This ensures the coordinator role is assigned to the server best equipped to handle task assignment decisions.

## Priority Calculation Formula

### Formula

```
priority = 0.5 × CPU_usage + 0.3 × normalized_tasks + 0.2 × memory_used
```

### Components

1. **CPU Usage** (50% weight):
   - Source: `sysinfo` crate, system CPU percentage
   - Range: 0.0 to 100.0
   - Formula: `system.global_cpu_usage()`
   - Interpretation: 0 = idle, 100 = fully utilized

2. **Active Tasks** (30% weight):
   - Source: Atomic counter, incremented/decremented on task start/finish
   - Range: 0 to infinity
   - Normalization: `(active_tasks / 10) × 100`, capped at 100
   - Interpretation: 10 concurrent tasks = 100% load

3. **Memory Used** (20% weight):
   - Source: `sysinfo` crate, system memory statistics
   - Range: 0.0 to 100.0
   - Formula: `100 - (available_memory / total_memory × 100)`
   - Interpretation: 0 = all memory available, 100 = no memory available

### Implementation

```rust
pub fn calculate_priority(&self) -> f64 {
    const W_CPU: f64 = 0.5;     // Weight for CPU usage
    const W_TASKS: f64 = 0.3;   // Weight for active tasks
    const W_MEMORY: f64 = 0.2;  // Weight for memory

    let cpu_usage = self.get_cpu_usage();
    let active_tasks = self.get_active_tasks() as f64;
    let memory_available = self.get_available_memory_percent();

    // Normalize active tasks (10 tasks = full load)
    let tasks_normalized = (active_tasks / 10.0).min(1.0) * 100.0;

    // Memory score: lower available = higher score
    let memory_score = 100.0 - memory_available;

    // Composite score (LOWER = BETTER)
    W_CPU * cpu_usage + W_TASKS * tasks_normalized + W_MEMORY * memory_score
}
```

### Examples with Calculations

**Example 1: Idle Server**
```
CPU: 0%
Active Tasks: 0
Memory Available: 100%

Calculation:
  CPU component:    0.5 × 0   = 0.0
  Task component:   0.3 × 0   = 0.0
  Memory component: 0.2 × 0   = 0.0
  ─────────────────────────────
  Priority:                0.0  ← Best possible score
```

**Example 2: Moderately Loaded Server**
```
CPU: 40%
Active Tasks: 5
Memory Available: 60%

Calculation:
  Tasks normalized: (5 / 10) × 100 = 50%
  Memory used:      100 - 60 = 40%

  CPU component:    0.5 × 40  = 20.0
  Task component:   0.3 × 50  = 15.0
  Memory component: 0.2 × 40  = 8.0
  ─────────────────────────────
  Priority:                43.0
```

**Example 3: Heavily Loaded Server**
```
CPU: 80%
Active Tasks: 12 (normalized to 100%)
Memory Available: 20%

Calculation:
  Tasks normalized: (12 / 10) × 100 = 120%, capped at 100%
  Memory used:      100 - 20 = 80%

  CPU component:    0.5 × 80  = 40.0
  Task component:   0.3 × 100 = 30.0
  Memory component: 0.2 × 80  = 16.0
  ─────────────────────────────
  Priority:                86.0  ← Poor score
```

### Weight Rationale

- **CPU (50%)**: Most indicative of processing capacity
- **Tasks (30%)**: Direct measure of current workload
- **Memory (20%)**: Less critical for image encryption (CPU-bound)

These weights can be tuned based on workload characteristics.

## Election Flow Step-by-Step

### State Diagram

```
                    ┌──────────────┐
                    │  No Leader   │
                    └──────┬───────┘
                           │
                           │ Startup or
                           │ Leader Failure
                           ▼
                    ┌──────────────┐
              ┌─────┤ Initiate     │
              │     │ Election     │
              │     └──────┬───────┘
              │            │
              │            │ Calculate Priority
              │            │ Broadcast Election
              │            ▼
              │     ┌──────────────┐
              │     │ Wait for     │
              │     │ Responses    │◄──────┐
              │     │ (2 seconds)  │       │
              │     └──────┬───────┘       │
              │            │                │
              │     ┌──────▼────────┐       │
              │     │ Received      │       │
              │  ┌──┤ ALIVE?        │       │
              │  │  └───────────────┘       │
              │  │                          │
              │  │ No (Won)      Yes (Lost) │
              │  │                          │
              ▼  ▼                          │
       ┌──────────────┐                    │
       │ Broadcast    │              Defer, wait
       │ Coordinator  │              for winner
       └──────┬───────┘                    │
              │                            │
              ▼                            │
       ┌──────────────┐                    │
       │  I am Leader │                    │
       └──────────────┘                    │
              │                            │
              │                            │
              ▼                            │
       ┌──────────────┐             ┌─────▼──────┐
       │ Send         │             │ Receive    │
       │ Heartbeats   │             │ Coordinator│
       └──────────────┘             └─────┬──────┘
                                          │
                                          ▼
                                   ┌──────────────┐
                                   │ Acknowledge  │
                                   │ Leader       │
                                   └──────────────┘
```

### Detailed Steps

#### 1. Election Initiation

**Trigger**: Startup timer (3s + random 100-500ms) or leader failure detected

**Process**:
```rust
async fn initiate_election(&self) {
    // Reset election state
    *self.received_alive.write().await = false;

    // Calculate current priority
    let my_priority = self.metrics.calculate_priority();

    info!("Server {} initiating election with priority {:.2}",
          self.config.server.id, my_priority);

    // Broadcast to all peers
    let election_msg = Message::Election {
        from_id: self.config.server.id,
        priority: my_priority,
    };
    self.broadcast(election_msg).await;

    // Wait for responses
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Check result
    if !*self.received_alive.read().await {
        self.announce_victory().await;
    }
}
```

#### 2. Receiving Election Message

**Receiver Logic**:
```rust
Message::Election { from_id, priority } => {
    let my_priority = self.metrics.calculate_priority();

    if my_priority < priority {
        // I'm better! Respond and start my own election
        let alive_msg = Message::Alive {
            from_id: self.config.server.id,
        };
        self.send_to_peer(from_id, alive_msg).await;

        // Start own election after small delay
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            self.initiate_election().await;
        });
    } else {
        // They're better, defer
        info!("Deferring to Server {} (their priority {:.2} < mine {:.2})",
              from_id, priority, my_priority);
    }
}
```

#### 3. Receiving Alive Response

**Sender Logic**:
```rust
Message::Alive { from_id } => {
    info!("Received ALIVE from {} (I lost)", from_id);
    *self.received_alive.write().await = true;
}
```

**Effect**: Marks the election as lost, prevents broadcasting Coordinator

#### 4. Victory Announcement

**Winner Logic**:
```rust
async fn announce_victory(&self) {
    info!("Server {} won election! Announcing...", self.config.server.id);

    // Update own state
    *self.current_leader.write().await = Some(self.config.server.id);

    // Broadcast to all peers
    let coordinator_msg = Message::Coordinator {
        leader_id: self.config.server.id,
    };
    self.broadcast(coordinator_msg).await;
}
```

#### 5. Acknowledging New Leader

**Non-winner Logic**:
```rust
Message::Coordinator { leader_id } => {
    info!("Acknowledging Server {} as LEADER", leader_id);
    *self.current_leader.write().await = Some(leader_id);
}
```

### ASCII Sequence Diagram

```
Server 1           Server 2           Server 3
(Load:25.3)        (Load:18.7)        (Load:42.1)
────┬────          ────┬────          ────┬────
    │                  │                  │
    │ Election{1,25.3} │                  │
    ├──────────────────┼──────────────────►
    │                  │                  │
    │                  │ Compare:         │
    │                  │ 18.7 < 25.3      │
    │                  │ (I win)          │
    │                  │                  │ Compare:
    │                  │                  │ 42.1 > 25.3
    │                  │                  │ (Defer)
    │     Alive{2}     │                  │
    │◄─────────────────┤                  │
    │                  │                  │
    │ (Lost)           │ Election{2,18.7} │
    │                  ├──────────────────►
    │                  │                  │
    │                  │                  │ Compare:
    │                  │                  │ 42.1 > 18.7
    │                  │                  │ (Defer)
    │                  │                  │
    │                  │ Wait 2s...       │
    │                  │ No ALIVE         │
    │                  │ (Victory!)       │
    │                  │                  │
    │ Coordinator{2}   │                  │
    │◄─────────────────┼──────────────────┤
    │                  │                  │
    │ ACK              │   LEADER         │ ACK
    ▼                  ▼                  ▼
```

## Heartbeat Protocol

### Purpose

1. **Liveness Indication**: Prove server is still alive
2. **Load Sharing**: Broadcast current load for task assignment
3. **Failure Detection**: Basis for detecting crashed servers

### Heartbeat Message

```rust
Message::Heartbeat {
    from_id: u32,       // Sender's ID
    timestamp: u64,     // Unix timestamp (seconds)
    load: f64,          // Current priority score
}
```

### Sending Heartbeats

**Frequency**: Every 1 second (configurable)

```rust
async fn start_heartbeat(&self) {
    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;

        let heartbeat = Message::Heartbeat {
            from_id: self.config.server.id,
            timestamp: current_timestamp(),
            load: self.metrics.get_load(),
        };

        self.broadcast(heartbeat).await;
    }
}
```

### Receiving Heartbeats

```rust
Message::Heartbeat { from_id, timestamp, load } => {
    // Update last seen time (for failure detection)
    self.last_heartbeat_times.write().await.insert(from_id, timestamp);

    // Update load (for task assignment)
    self.peer_loads.write().await.insert(from_id, load);

    debug!("Heartbeat from Server {} (load: {:.2})", from_id, load);
}
```

### Configuration

```toml
[election]
heartbeat_interval_secs = 1
```

**Considerations**:
- **Lower interval** (e.g., 0.5s): Faster failure detection, more network traffic
- **Higher interval** (e.g., 2s): Less overhead, slower failure detection

## Failure Detection Mechanism

### Timeout-Based Detection

**Rule**: If no heartbeat received for `failure_timeout_secs`, consider server failed.

```toml
[election]
failure_timeout_secs = 3
monitor_interval_secs = 1
```

### Monitor Task

Runs every 1 second, checks for timed-out peers:

```rust
async fn monitor_heartbeats(&self) {
    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;

        let now = current_timestamp();
        let timeout = 3;  // seconds

        // Find timed-out peers
        let heartbeats = self.last_heartbeat_times.read().await;
        let timed_out_peers: Vec<u32> = heartbeats
            .iter()
            .filter_map(|(peer_id, last_seen)| {
                if now - last_seen > timeout {
                    Some(*peer_id)
                } else {
                    None
                }
            })
            .collect();

        // Handle each failure
        for peer_id in timed_out_peers {
            self.handle_peer_failure(peer_id).await;
        }
    }
}
```

### Failure Handling

```rust
async fn handle_peer_failure(&self, peer_id: u32) {
    warn!("Server {} detected peer {} failed", self.config.server.id, peer_id);

    // Clean up state
    self.peer_loads.write().await.remove(&peer_id);
    self.last_heartbeat_times.write().await.remove(&peer_id);

    // Find orphaned tasks
    let orphaned_tasks = self.find_orphaned_tasks(peer_id).await;
    for task in orphaned_tasks {
        self.task_history.write().await.remove(&task);
    }

    // Check if leader failed
    let current_leader = *self.current_leader.read().await;
    if Some(peer_id) == current_leader {
        warn!("Leader {} failed! Initiating re-election...", peer_id);
        *self.current_leader.write().await = None;
        self.initiate_election().await;
    }
}
```

### Detection Timeline

```
T+0s:  Server 2 crashes (stops sending heartbeats)
T+1s:  Peers receive last heartbeat from S2
T+2s:  Peers notice no new heartbeat (suspicious)
T+3s:  Peers notice no heartbeat for 3s (threshold exceeded)
T+3s:  Peers declare S2 failed
T+3s:  If S2 was leader, start new election
T+5s:  New leader elected
```

**Total failover time**: ~3-5 seconds

## Re-election Triggers

### 1. System Startup

**When**: Server starts for the first time

**Delay**: 3 seconds + random 100-500ms

**Reason**: Allow all servers to start and connect

```rust
tokio::spawn(async move {
    tokio::time::sleep(Duration::from_secs(3) + random_delay).await;
    self.initiate_election().await;
});
```

### 2. Leader Failure

**When**: Leader's heartbeat times out (3+ seconds)

**Trigger**: All surviving servers detect failure and start elections

```rust
if Some(failed_peer_id) == current_leader {
    *self.current_leader.write().await = None;
    self.initiate_election().await;
}
```

### 3. Manual Trigger (Optional)

Not currently implemented, but could add:

```rust
// Via admin command or signal
pub async fn force_election(&self) {
    self.initiate_election().await;
}
```

## Edge Cases and Handling

### 1. Simultaneous Elections

**Scenario**: Multiple servers start elections at the same time

**Example**:
```
T+0s: S1 and S3 both start elections
T+0s: S1 broadcasts Election{1, 25.3}
T+0s: S3 broadcasts Election{3, 42.1}
```

**Handling**:
1. S1 receives S3's election: compares 25.3 vs 42.1 → S1 is better → S1 sends Alive, continues
2. S3 receives S1's election: compares 42.1 vs 25.3 → S1 is better → S3 sends Alive, defers
3. S2 receives both: sends Alive to both (S2 has priority 18.7, best of all)
4. S2 starts own election, wins

**Result**: Lowest-priority server (S2) wins despite concurrent elections

### 2. Network Partition

**Scenario**: Servers split into two groups that can't communicate

**Example**:
```
Group A: S1, S2 (can communicate)
Group B: S3 (isolated)

Result:
- Group A elects a leader (S1 or S2)
- Group B elects itself (S3) as leader
- Split-brain: 2 leaders!
```

**Current Handling**: Not handled (requires consensus algorithm like Raft/Paxos)

**Mitigation**: Deploy on reliable network, monitor connectivity

### 3. Leader Becomes Overloaded

**Scenario**: Leader's load increases significantly after election

**Example**:
```
T+0s:  S2 elected (load: 18.7)
T+10s: S2 processes many tasks (load: 65.3)
T+10s: S1 has load: 15.2 (now better candidate)
```

**Current Handling**: No automatic re-election

**Possible Enhancement**: Periodic re-election or leader stepping down

**Workaround**: Leader still assigns tasks to least-loaded server (could be another server)

### 4. All Servers Have Same Load

**Scenario**: Tie in priority scores

**Example**:
```
S1: priority = 20.0
S2: priority = 20.0
S3: priority = 20.0
```

**Handling**: First to respond wins (non-deterministic but safe)

**Actual Behavior**:
1. S1 starts election with priority 20.0
2. S2 and S3 both have same priority → neither responds with Alive
3. S1 waits 2 seconds, no response → S1 wins

**Alternative**: Add server ID as tiebreaker (not currently implemented)

### 5. Message Loss

**Scenario**: Election or Coordinator message lost in network

**Example**:
```
S2 broadcasts Coordinator{2}, but S3 never receives it
```

**Handling**:
- S3 will timeout on leader heartbeat (no heartbeats from S2 as leader)
- S3 starts new election
- S2 participates, wins again
- S3 receives new Coordinator message

**Recovery Time**: Up to 3 seconds (heartbeat timeout)

### 6. Server Joins Mid-Election

**Scenario**: New server S4 starts while election is in progress

**Handling**:
1. S4 starts own election after 3-second timer
2. By this time, existing election has completed
3. S4 receives Coordinator message from current leader
4. S4 acknowledges leader, joins cluster

**Result**: S4 integrates smoothly without disrupting ongoing election

### 7. Crashed Server Restarts

**Scenario**: Server crashes and restarts during operation

**Timeline**:
```
T+0s:  S2 crashes
T+3s:  Others detect failure, elect new leader (S1)
T+10s: S2 restarts
T+13s: S2 starts election
```

**Handling**:
1. S2 calculates priority (might be low due to restart)
2. S2 broadcasts election
3. S1 (current leader) responds with Alive if better
4. S2 receives Coordinator from S1, acknowledges
5. S2 joins cluster as follower

**Result**: Seamless recovery and integration

## Performance Characteristics

### Time Complexity

**Election Process**:
- Message broadcasts: O(N) where N = number of servers
- Priority comparison: O(1)
- Overall: O(N) per election

**Task Assignment**:
- Load comparison: O(N) to find minimum
- Per assignment: O(N)

### Message Complexity

**Normal Operation** (per second):
- Heartbeats: N × (N - 1) messages (each server to all peers)
- Example (3 servers): 3 × 2 = 6 messages/second

**Election**:
- Election messages: N broadcasts
- Alive responses: Up to N messages
- Coordinator: 1 broadcast
- Total: ~3N messages per election

**Example (3 servers)**:
- Election broadcasts: 3
- Alive responses: ~2
- Coordinator: 1
- Total: ~6 messages for complete election

### Latency

**Election Latency**:
```
Minimum: Election timeout (2 seconds)
Average: 2-3 seconds
Maximum: 2 × election timeout (4 seconds) if cascading elections
```

**Failure Detection Latency**:
```
Minimum: failure_timeout (3 seconds)
Average: 3-4 seconds
Maximum: failure_timeout + monitor_interval (4 seconds)
```

**Total Failover Latency**:
```
Detection: 3-4 seconds
Election:  2-3 seconds
Total:     5-7 seconds
```

### Scalability

**Current Configuration**: Tested with up to 10 servers

**Bottlenecks**:
1. **Heartbeat overhead**: O(N²) messages/second
   - 10 servers: 90 messages/second (acceptable)
   - 100 servers: 9,900 messages/second (high)

2. **Election coordination**: O(N) messages, but serialized
   - More servers = longer cascade of elections

**Optimization Strategies**:
1. Increase heartbeat interval (trade-off with detection latency)
2. Use multicast for broadcasts (reduce network load)
3. Implement hierarchical election (split into groups)

## Example Scenarios

### Scenario 1: Normal Election with 3 Servers

**Initial State**:
```
Server 1: CPU=20%, Tasks=2, Memory=80% available
          Priority = 0.5×20 + 0.3×20 + 0.2×20 = 20.0

Server 2: CPU=40%, Tasks=5, Memory=60% available
          Priority = 0.5×40 + 0.3×50 + 0.2×40 = 43.0

Server 3: CPU=10%, Tasks=0, Memory=90% available
          Priority = 0.5×10 + 0.3×0 + 0.2×10 = 7.0
```

**Election Timeline**:

```
T+0s: All servers start
T+3s: Server 1 timer expires → initiates election

Server 1 → All: Election{from:1, priority:20.0}

T+3.1s:
  Server 2 receives:
    Compare 43.0 vs 20.0 → S1 is better → Defer

  Server 3 receives:
    Compare 7.0 vs 20.0 → S3 is better! → Respond

Server 3 → Server 1: Alive{from:3}
Server 3 → All: Election{from:3, priority:7.0}

T+3.2s:
  Server 1 receives Alive → marks election as lost

  Server 2 receives Election from S3:
    Compare 43.0 vs 7.0 → S3 is better → Defer

T+5.2s: Server 3 election timeout (2s elapsed)
  No Alive received → Server 3 won!

Server 3 → All: Coordinator{leader:3}

T+5.3s:
  Server 1: Acknowledges leader 3
  Server 2: Acknowledges leader 3

RESULT: Server 3 is the leader (lowest load)
```

### Scenario 2: Leader Failure and Re-election

**Initial State**:
```
Leader: Server 3 (priority: 7.0)
Followers: Server 1 (priority: 20.0), Server 2 (priority: 43.0)
All servers exchanging heartbeats
```

**Failure Timeline**:

```
T+0s: Server 3 crashes (stops sending heartbeats)

T+0s-T+3s:
  Server 1 and Server 2 continue sending heartbeats
  Last heartbeat from S3: T+0s

T+3s: Monitor tasks on S1 and S2 detect failure
  now - last_heartbeat[3] = 3s > threshold

  Server 1:
    Detects S3 failed
    S3 was leader → Set leader = None
    Initiate election

  Server 2:
    Detects S3 failed
    S3 was leader → Set leader = None
    Initiate election

T+3s: Concurrent elections!
  Server 1 → All: Election{from:1, priority:20.0}
  Server 2 → All: Election{from:2, priority:43.0}

T+3.1s:
  S1 receives S2's election:
    Compare 20.0 vs 43.0 → S1 is better!
    S1 → S2: Alive{from:1}
    S1 continues election

  S2 receives S1's election:
    Compare 43.0 vs 20.0 → S1 is better
    S2 → S1: Alive{from:2}  (wait, this is wrong!)
    Actually: S2 defers

  Result: S1 receives NO Alive (S2 deferred)

T+5s: S1 election timeout
  No Alive → S1 wins
  S1 → All: Coordinator{leader:1}

T+5.1s:
  S2 acknowledges S1 as leader

RESULT: Server 1 is the new leader
FAILOVER TIME: 5 seconds (3s detection + 2s election)
```

### Scenario 3: Load Changes During Operation

**Initial State**:
```
Leader: Server 2 (priority: 18.7)
Server 1: priority: 25.3
Server 3: priority: 42.1
```

**Operation**:
```
T+0s: Leader S2 assigns tasks

Client requests arrive:
  Task 1 → S2 assigns to S1 (lowest load: 25.3)
  Task 2 → S2 assigns to S1 (still lowest: 27.5)
  Task 3 → S2 assigns to S1 (still lowest: 29.8)

T+10s: Loads have changed
  S1: priority: 35.2 (processing 3 tasks)
  S2: priority: 18.7 (just coordinating)
  S3: priority: 42.1 (idle)

  Task 4 → S2 assigns to S2 (self, lowest: 18.7)
  Task 5 → S2 assigns to S2 (still lowest: 22.3)

T+20s: All tasks complete
  S1: priority: 25.3
  S2: priority: 18.7
  S3: priority: 42.1

RESULT: Load balancing adapts to current conditions
        Leader coordinates but can also process tasks
```

**Key Insight**: Even though leader's priority increases, it doesn't trigger re-election. Leader continues coordinating and can assign tasks to itself if it's the least loaded.

---

## Summary

The Modified Bully Algorithm in CloudP2P provides:

1. **Dynamic Leadership**: Elects least-loaded server, not highest ID
2. **Fault Tolerance**: Automatic re-election on leader failure
3. **Load Awareness**: Real-time metrics via heartbeat protocol
4. **Simplicity**: Straightforward implementation, easy to reason about
5. **Efficiency**: O(N) message complexity, fast elections (2-3s)

**Trade-offs**:
- **Split-brain risk**: Network partitions can create multiple leaders
- **No re-election**: Overloaded leader doesn't automatically step down
- **Message overhead**: O(N²) heartbeat messages

**Best For**: Small to medium clusters (3-10 servers) with reliable networking

**Not Best For**: Large clusters (100+ servers) or networks with frequent partitions

For architecture details, see [`ARCHITECTURE.md`](/home/g03-s2025/Desktop/CloudP2P/docs/ARCHITECTURE.md).
