//! # Server Middleware - Distributed System Coordination
//!
//! The middleware layer handles all distributed system concerns for the CloudP2P cluster:
//!
//! ## Core Responsibilities
//!
//! ### 1. Leader Election (Modified Bully Algorithm)
//! - Initiates elections when the leader fails or system starts
//! - Calculates priority based on CPU usage, active tasks, and available memory
//! - Broadcasts election messages and handles responses
//! - Announces leader and maintains leader state
//!
//! ### 2. Heartbeat Management
//! - Sends periodic heartbeats to all peers with current load metrics
//! - Monitors incoming heartbeats to detect failed servers
//! - Maintains last-seen timestamps for all peers
//!
//! ### 3. Task Distribution & Load Balancing
//! - Receives task assignment requests from clients
//! - Determines optimal server based on current load across cluster
//! - Routes tasks to the least-loaded server
//! - Maintains task history for fault tolerance
//!
//! ### 4. Fault Tolerance
//! - Detects when peers fail (missing heartbeats)
//! - Cleans up orphaned tasks when servers fail
//! - Initiates new elections when leader fails
//! - Removes stale state for failed servers
//!
//! ### 5. Peer Connection Management
//! - Establishes and maintains connections to all peer servers
//! - Provides message broadcasting and point-to-point communication
//! - Automatically reconnects when connections are lost
//!
//! ## Architecture
//!
//! The middleware wraps around [`ServerCore`] which performs the actual image
//! encryption. All coordination logic is isolated in this layer, allowing the
//! core to remain simple and focused.
//!
//! ## Message Flow
//!
//! ```text
//! Client -> Leader (TaskAssignmentRequest)
//! Leader -> Client (TaskAssignmentResponse with best server)
//! Client -> Assigned Server (TaskRequest)
//! Server -> ServerCore (encrypt_image)
//! Server -> Client (TaskResponse)
//! ```

use anyhow::Result;
use log::{debug, error, info, warn};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};

use crate::common::messages::*;
use crate::common::connection::Connection;
use crate::common::config::{PeersConfig, ElectionConfig};
use crate::server::election::ServerMetrics;
use crate::server::server::ServerCore;

// ============================================================================
// CONFIGURATION STRUCTURES
// ============================================================================

/// Complete server configuration loaded from TOML file.
///
/// Contains information about this server, peer servers, and election timing.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Information about THIS server (ID and address)
    pub server: ServerInfo,
    /// Information about OTHER servers in the cluster
    pub peers: PeersConfig,
    /// Election timing and timeout configuration
    pub election: ElectionConfig,
}

/// Information about this server instance.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    /// Unique identifier for this server (1, 2, 3, etc.)
    pub id: u32,
    /// Network address where this server listens (e.g., "127.0.0.1:8001")
    pub address: String,
}

#[allow(dead_code)]
impl ServerConfig {
    /// Load server configuration from a TOML file.
    ///
    /// # Arguments
    /// - `path`: Path to the TOML configuration file
    ///
    /// # Returns
    /// - `Ok(ServerConfig)`: Successfully loaded configuration
    /// - `Err`: File I/O or parsing error
    ///
    /// # Example
    /// ```ignore
    /// let config = ServerConfig::from_file("config/server1.toml")?;
    /// ```
    pub fn from_file(path: &str) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: ServerConfig = toml::from_str(&content)?;
        Ok(config)
    }
}

// ============================================================================
// TASK HISTORY - For fault tolerance tracking
// ============================================================================

/// Entry in the task history log.
///
/// Tracks which server was assigned to handle a particular client task.
/// Used for fault tolerance - if a server fails, we can identify and clean up
/// its orphaned tasks.
#[derive(Debug, Clone)]
struct TaskHistoryEntry {
    _client_name: String,
    _request_id: u64,
    assigned_server_id: u32,
    _timestamp: u64,
}

// ============================================================================
// SERVER MIDDLEWARE - Main coordination component
// ============================================================================

/// Server middleware that handles all distributed system coordination.
///
/// This struct manages:
/// - Leader election using Modified Bully Algorithm
/// - Heartbeat sending and monitoring
/// - Peer connection management
/// - Task assignment and load balancing
/// - Fault tolerance and failure recovery
///
/// The middleware delegates actual image encryption to [`ServerCore`].
///
/// # Architecture
///
/// ```text
/// ┌─────────────────────────────────────┐
/// │      ServerMiddleware               │
/// │  (Election, Heartbeats, Tasks)      │
/// │                                     │
/// │  ┌───────────────────────────────┐ │
/// │  │       ServerCore              │ │
/// │  │  (Image Encryption Only)      │ │
/// │  └───────────────────────────────┘ │
/// └─────────────────────────────────────┘
/// ```
#[allow(dead_code)]
pub struct ServerMiddleware {
    /// Core encryption service (wrapped in Arc for sharing across tasks)
    core: Arc<ServerCore>,

    /// Configuration loaded from TOML file
    config: ServerConfig,

    /// Performance metrics used to calculate priority during election
    metrics: ServerMetrics,

    /// Current leader ID (None if no leader, Some(id) if we have a leader)
    current_leader: Arc<RwLock<Option<u32>>>,

    /// Flag indicating if we received ALIVE response during election
    received_alive: Arc<RwLock<bool>>,

    /// Peer connections: peer_id -> channel to send messages to that peer
    /// We use channels so we can send messages from anywhere in the code
    peer_connections: Arc<RwLock<HashMap<u32, mpsc::Sender<Message>>>>,

    /// Last time we heard from each peer (used to detect failures)
    last_heartbeat_times: Arc<RwLock<HashMap<u32, u64>>>,

    /// Active task handles for cancellation if needed
    active_tasks: Arc<RwLock<HashMap<u64, tokio::task::JoinHandle<()>>>>,

    /// Current load values for each peer (reported via heartbeats)
    peer_loads: Arc<RwLock<HashMap<u32, f64>>>,

    /// Task history for fault tolerance: (client_name, request_id) -> entry
    task_history: Arc<RwLock<HashMap<(String, u64), TaskHistoryEntry>>>,
}

#[allow(dead_code)]
impl ServerMiddleware {
    /// Create a new server middleware instance.
    ///
    /// # Arguments
    /// - `config`: Server configuration (loaded from TOML)
    /// - `core`: The core encryption service (wrapped in Arc)
    ///
    /// # Example
    /// ```ignore
    /// let config = ServerConfig::from_file("config/server1.toml")?;
    /// let core = Arc::new(ServerCore::new(config.server.id));
    /// let middleware = ServerMiddleware::new(config, core);
    /// ```
    pub fn new(config: ServerConfig, core: Arc<ServerCore>) -> Self {
        // Initialize metrics for this server
        let metrics = ServerMetrics::new();

        Self {
            core,
            config,
            metrics,
            current_leader: Arc::new(RwLock::new(None)),
            received_alive: Arc::new(RwLock::new(false)),
            peer_connections: Arc::new(RwLock::new(HashMap::new())),
            last_heartbeat_times: Arc::new(RwLock::new(HashMap::new())),
            active_tasks: Arc::new(RwLock::new(HashMap::new())),
            peer_loads: Arc::new(RwLock::new(HashMap::new())),
            task_history: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Main entry point - starts all server tasks and runs forever.
    ///
    /// This method:
    /// 1. Starts initial election timer (3 seconds + random delay)
    /// 2. Launches listener for incoming connections
    /// 3. Connects to peer servers
    /// 4. Starts heartbeat broadcasting
    /// 5. Starts heartbeat monitoring
    ///
    /// All tasks run concurrently and indefinitely.
    pub async fn run(&self) {
        info!(
            "🚀 Server {} starting on {}",
            self.config.server.id, self.config.server.address
        );

        // After 3 seconds + random delay, start an election
        // Random delay prevents all servers from starting election simultaneously
        let server_clone = self.clone_arc();
        let mut rng = rand::thread_rng();
        let random_delay = rng.gen_range(100..500); // 100-500ms random delay
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(3) + Duration::from_millis(random_delay)).await;
            info!("⏰ Initial election timer expired, starting election...");
            server_clone.initiate_election().await;
        });

        // Start all long-running tasks
        let listener_task = self.start_listener();
        let peer_task = self.connect_to_peers();
        let heartbeat_task = self.start_heartbeat();
        let monitor_task = self.monitor_heartbeats();

        // Run all tasks concurrently - if any terminates, log an error
        tokio::select! {
            _ = listener_task => error!("❌ Listener task terminated"),
            _ = peer_task => error!("❌ Peer connection task terminated"),
            _ = heartbeat_task => error!("❌ Heartbeat task terminated"),
            _ = monitor_task => error!("❌ Monitor task terminated"),
        }
    }

    // ========================================================================
    // TASK 1: Listen for incoming connections from peers and clients
    // ========================================================================

    /// Start listening for incoming TCP connections.
    ///
    /// For each incoming connection:
    /// 1. Accept the connection
    /// 2. Spawn a new task to handle messages from that connection
    /// 3. Continue listening for more connections
    ///
    /// This runs forever in a loop.
    async fn start_listener(&self) {
        use tokio::net::TcpListener;

        // Bind to our configured address
        let listener = match TcpListener::bind(&self.config.server.address).await {
            Ok(l) => l,
            Err(e) => {
                error!("❌ Failed to bind to {}: {}", self.config.server.address, e);
                return;
            }
        };

        info!(
            "📡 Server {} listening on {}",
            self.config.server.id, self.config.server.address
        );

        // Accept connections in a loop
        loop {
            match listener.accept().await {
                Ok((socket, addr)) => {
                    debug!(
                        "🔗 Server {} accepted connection from {}",
                        self.config.server.id, addr
                    );

                    // Spawn a new task to handle this connection
                    let server = self.clone_arc();
                    tokio::spawn(async move {
                        server.handle_connection(socket).await;
                    });
                }
                Err(e) => error!("❌ Accept error: {}", e),
            }
        }
    }

    /// Handle a single TCP connection - read and process messages in a loop.
    ///
    /// # Arguments
    /// - `socket`: The TCP stream for this connection
    ///
    /// This method:
    /// 1. Wraps the socket in a Connection
    /// 2. Reads messages in a loop
    /// 3. Handles special cases (LeaderQuery)
    /// 4. Delegates to handle_message for normal messages
    /// 5. Closes connection when done
    async fn handle_connection(&self, socket: tokio::net::TcpStream) {
        let mut conn = Connection::new(socket);

        loop {
            match conn.read_message().await {
                Ok(Some(message)) => {
                    // Special case: LeaderQuery requires immediate response
                    if matches!(message, Message::LeaderQuery) {
                        let leader = *self.current_leader.read().await;
                        if let Some(leader_id) = leader {
                            let response = Message::LeaderResponse { leader_id };
                            let _ = conn.write_message(&response).await;
                        }
                        continue; // Don't process this as a normal message
                    }

                    // Normal message handling
                    self.handle_message(message, &mut conn).await;
                }
                Ok(None) => {
                    debug!("🔌 Connection closed");
                    break;
                }
                Err(e) => {
                    error!("❌ Error reading message: {}", e);
                    break;
                }
            }
        }
    }

    // ========================================================================
    // TASK 2: Connect to peer servers
    // ========================================================================

    /// Connect to all peer servers and maintain connections.
    ///
    /// For each peer:
    /// 1. Try to establish TCP connection
    /// 2. Create a channel for sending messages
    /// 3. Spawn task that reads from channel and sends to peer
    /// 4. Reconnect if connection is lost
    ///
    /// This runs forever, maintaining connections to all peers.
    async fn connect_to_peers(&self) {
        use tokio::net::TcpStream;

        // Wait a bit for servers to start
        tokio::time::sleep(Duration::from_secs(1)).await;

        // Try to connect to each peer
        for peer in &self.config.peers.peers {
            let peer_id = peer.id;
            let peer_addr = peer.address.clone();
            let server = self.clone_arc();

            // Spawn a task that keeps trying to connect to this peer
            tokio::spawn(async move {
                loop {
                    match TcpStream::connect(&peer_addr).await {
                        Ok(stream) => {
                            info!(
                                "🤝 Server {} connected to peer {}",
                                server.config.server.id, peer_id
                            );

                            // Create a channel for sending messages to this peer
                            let (tx, mut rx) = mpsc::channel::<Message>(100);
                            server.peer_connections.write().await.insert(peer_id, tx);

                            let mut conn = Connection::new(stream);

                            // Read from the channel and send messages to the peer
                            while let Some(msg) = rx.recv().await {
                                if let Err(e) = conn.write_message(&msg).await {
                                    error!("❌ Error sending to peer {}: {}", peer_id, e);
                                    break;
                                }
                            }

                            // Connection lost
                            server.peer_connections.write().await.remove(&peer_id);
                            warn!(
                                "⚠️  Server {} lost connection to peer {}",
                                server.config.server.id, peer_id
                            );
                        }
                        Err(_) => {
                            // Connection failed, will retry
                        }
                    }

                    // Wait before retrying
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            });
        }

        // Keep this task alive forever
        std::future::pending::<()>().await;
    }

    // ========================================================================
    // MESSAGE HANDLING - Process different message types
    // ========================================================================

    /// Handle incoming messages based on their type.
    ///
    /// # Arguments
    /// - `message`: The received message
    /// - `conn`: The connection to send responses on (if needed)
    ///
    /// ## Message Types
    ///
    /// - **Election**: Handle incoming election request
    /// - **Alive**: Handle response during election
    /// - **Coordinator**: Acknowledge new leader
    /// - **Heartbeat**: Update peer status
    /// - **TaskRequest**: Process encryption task
    /// - **TaskAssignmentRequest**: Assign task to best server (leader only)
    /// - **HistoryAdd**: Add task to history
    /// - **HistoryRemove**: Remove completed task from history
    async fn handle_message(&self, message: Message, conn: &mut Connection) {
        match message {
            // Someone started an election
            Message::Election { from_id, priority } => {
                info!(
                    "🗳️  Server {} received ELECTION from {} (priority: {:.2})",
                    self.config.server.id, from_id, priority
                );

                // Calculate our priority
                let my_priority = self.metrics.calculate_priority();

                // If we have higher priority (lower score), respond and start our own election
                if my_priority < priority {
                    info!(
                        "💪 Server {} has lower priority ({:.2} < {:.2}), responding with ALIVE",
                        self.config.server.id, my_priority, priority
                    );

                    // Send ALIVE message to the sender
                    let alive_msg = Message::Alive {
                        from_id: self.config.server.id,
                    };
                    self.send_to_peer(from_id, alive_msg).await;

                    // Add small delay before starting own election
                    let server = self.clone_arc();
                    tokio::spawn(async move {
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        server.initiate_election().await;
                    });
                } else {
                    info!(
                        "📊 Server {} has higher priority ({:.2} > {:.2}), deferring",
                        self.config.server.id, my_priority, priority
                    );
                }
            }

            // Someone responded to our election with "I'm alive and have higher priority"
            Message::Alive { from_id } => {
                info!(
                    "👋 Server {} received ALIVE from {} (they have lower priority)",
                    self.config.server.id, from_id
                );
                // We lost the election
                *self.received_alive.write().await = true;
            }

            // Someone won the election and is announcing themselves as leader
            Message::Coordinator { leader_id } => {
                info!(
                    "👑 Server {} acknowledges {} as LEADER",
                    self.config.server.id, leader_id
                );
                *self.current_leader.write().await = Some(leader_id);
            }

            // Received a heartbeat from a peer
            Message::Heartbeat {
                from_id,
                timestamp,
                load,
            } => {
                // Update the last time we heard from this peer
                self.last_heartbeat_times
                    .write()
                    .await
                    .insert(from_id, timestamp);

                self.peer_loads.write().await.insert(from_id, load);

                debug!(
                    "💓 Server {} received heartbeat from {} (load: {:.2})",
                    self.config.server.id, from_id, load
                );
            }

            // Client asking who the leader is
            Message::LeaderQuery => {
                let leader = *self.current_leader.read().await;
                if let Some(leader_id) = leader {
                    info!("📋 Client queried leader, responding with: {}", leader_id);
                    // Response is sent back through the same connection
                    // (handled in handle_connection)
                }
            }

            // Client sending a task
            Message::TaskRequest {
                client_name,
                request_id,
                image_data,
                image_name,
                text_to_embed,
                assigned_by_leader,
            } => {
                info!(
                    "📥 Server {} received task #{} from client '{}' (assigned by leader {})",
                    self.config.server.id, request_id, client_name, assigned_by_leader
                );

                // Create a channel for response
                let (tx, mut rx) = mpsc::channel::<Message>(1);

                // Process the task (delegates to core for encryption)
                self.process_task(
                    request_id,
                    client_name.clone(),
                    image_data,
                    image_name,
                    text_to_embed,
                    Some(tx),
                )
                .await;

                // Send response back to client
                if let Some(response) = rx.recv().await {
                    if let Err(e) = conn.write_message(&response).await {
                        error!("❌ Failed to send response to client: {}", e);
                    }
                }
            }

            // Leader receives request to assign task to best server
            Message::TaskAssignmentRequest {
                client_name,
                request_id,
            } => {
                // First, check if we're the leader
                let current_leader = *self.current_leader.read().await;
                let am_i_leader = current_leader == Some(self.config.server.id);

                if am_i_leader {
                    // We're the leader! Let's find the best server

                    // Get our own load
                    let my_load = self.metrics.get_load();

                    // Get all peer loads (from heartbeats)
                    let peer_loads = self.peer_loads.read().await;

                    // Log current state
                    info!("📊 LOAD DISTRIBUTION:");
                    info!(
                        "   Server {} (me, leader): {:.2}",
                        self.config.server.id, my_load
                    );
                    for (peer_id, peer_load) in peer_loads.iter() {
                        info!("   Server {}: {:.2}", peer_id, peer_load);
                    }

                    // Find server with lowest load (could be us!)
                    let mut lowest_load = my_load;
                    let mut best_server = self.config.server.id;

                    for (peer_id, peer_load) in peer_loads.iter() {
                        if *peer_load < lowest_load {
                            lowest_load = *peer_load;
                            best_server = *peer_id;
                        }
                    }

                    // Get the address of the chosen server
                    let assigned_address = if best_server == self.config.server.id {
                        // It's us! Use our address
                        self.config.server.address.clone()
                    } else {
                        // It's a peer, look up their address
                        self.config
                            .peers
                            .peers
                            .iter()
                            .find(|p| p.id == best_server)
                            .map(|p| p.address.clone())
                            .unwrap_or_default()
                    };

                    info!(
                        "📌 Task #{} from {} assigned to Server {} (load: {:.2})",
                        request_id, client_name, best_server, lowest_load
                    );

                    // Add to history and broadcast to all servers
                    let timestamp = current_timestamp();
                    let history_msg = Message::HistoryAdd {
                        client_name: client_name.clone(),
                        request_id,
                        assigned_server_id: best_server,
                        timestamp,
                    };

                    // Add to own history
                    let entry = TaskHistoryEntry {
                        _client_name: client_name.clone(),
                        _request_id: request_id,
                        assigned_server_id: best_server,
                        _timestamp: timestamp,
                    };
                    self.task_history
                        .write()
                        .await
                        .insert((client_name, request_id), entry);

                    // Broadcast to all peers
                    self.broadcast(history_msg).await;

                    // Send response to client
                    let response = Message::TaskAssignmentResponse {
                        request_id,
                        assigned_server_id: best_server,
                        assigned_server_address: assigned_address,
                    };

                    if let Err(e) = conn.write_message(&response).await {
                        error!("❌ Failed to send assignment response: {}", e);
                    }
                } else {
                    warn!("⚠️  Non-leader received assignment request, ignoring");
                }
            }

            // History management messages
            Message::HistoryAdd {
                client_name,
                request_id,
                assigned_server_id,
                timestamp,
            } => {
                debug!(
                    "📝 Server {} adding history entry: ({}, {}) -> Server {}",
                    self.config.server.id, client_name, request_id, assigned_server_id
                );

                let entry = TaskHistoryEntry {
                    _client_name: client_name.clone(),
                    _request_id: request_id,
                    assigned_server_id,
                    _timestamp: timestamp,
                };

                self.task_history
                    .write()
                    .await
                    .insert((client_name, request_id), entry);
            }

            Message::HistoryRemove {
                client_name,
                request_id,
            } => {
                debug!(
                    "🗑️  Server {} removing history entry: ({}, {})",
                    self.config.server.id, client_name, request_id
                );

                self.task_history
                    .write()
                    .await
                    .remove(&(client_name, request_id));
            }

            _ => {
                // Ignore other messages
            }
        }
    }

    // ========================================================================
    // TASK 3: Send heartbeats periodically
    // ========================================================================

    /// Broadcast heartbeat messages to all peers periodically.
    ///
    /// Each heartbeat contains:
    /// - Server ID
    /// - Current timestamp
    /// - Current load (priority score)
    ///
    /// This runs forever in a loop, sending heartbeats at the configured interval.
    async fn start_heartbeat(&self) {
        let interval = self.config.election.heartbeat_interval_secs;

        loop {
            tokio::time::sleep(Duration::from_secs(interval)).await;

            // Get REAL current load
            let current_load = self.metrics.get_load();
            let cpu = self.metrics.get_cpu_usage();
            let tasks = self.metrics.get_active_tasks();

            let heartbeat = Message::Heartbeat {
                from_id: self.config.server.id,
                timestamp: current_timestamp(),
                load: current_load,
            };

            debug!(
                "💓 Server {} sending heartbeat (load: {:.2}, CPU: {:.1}%, tasks: {})",
                self.config.server.id, current_load, cpu, tasks
            );

            self.broadcast(heartbeat).await;
        }
    }

    // ========================================================================
    // TASK 4: Monitor heartbeats and detect failures
    // ========================================================================

    /// Monitor peer heartbeats and detect failed servers.
    ///
    /// For each monitoring interval:
    /// 1. Check last heartbeat time for each peer
    /// 2. Identify peers that haven't sent heartbeat in timeout period
    /// 3. Clean up state for failed peers
    /// 4. Clean up orphaned tasks assigned to failed peers
    /// 5. If leader failed, initiate new election
    ///
    /// This runs forever in a loop, checking at the configured interval.
    async fn monitor_heartbeats(&self) {
        loop {
            tokio::time::sleep(Duration::from_secs(
                self.config.election.monitor_interval_secs,
            ))
            .await;

            let now = current_timestamp();
            let timeout = self.config.election.failure_timeout_secs;

            // Collect timed-out peers (only holding read lock)
            let timed_out_peers: Vec<u32> = {
                let heartbeats = self.last_heartbeat_times.read().await;
                heartbeats
                    .iter()
                    .filter_map(|(peer_id, last_seen)| {
                        if now - last_seen > timeout {
                            Some(*peer_id)
                        } else {
                            None
                        }
                    })
                    .collect()
            };

            let current_leader = *self.current_leader.read().await;

            // Now process the timed-out peers without holding the read lock
            for peer_id in timed_out_peers {
                warn!(
                    "⚠️  Server {} detected peer {} may have failed (no heartbeat for {}s)",
                    self.config.server.id, peer_id, timeout
                );

                self.peer_loads.write().await.remove(&peer_id);
                self.last_heartbeat_times.write().await.remove(&peer_id);

                // Check for orphaned tasks assigned to this failed server
                let orphaned_tasks: Vec<(String, u64)> = {
                    let history = self.task_history.read().await;
                    history
                        .iter()
                        .filter(|(_, entry)| entry.assigned_server_id == peer_id)
                        .map(|(key, _)| key.clone())
                        .collect()
                };

                if !orphaned_tasks.is_empty() {
                    warn!(
                        "🗑️  Server {} found {} orphaned task(s) assigned to failed Server {}",
                        self.config.server.id,
                        orphaned_tasks.len(),
                        peer_id
                    );

                    // Remove orphaned tasks from history
                    let mut history = self.task_history.write().await;
                    for key in &orphaned_tasks {
                        history.remove(key);
                        info!("   - Removed task: {:?}", key);
                    }
                }

                // If the leader failed, start a new election
                if Some(peer_id) == current_leader {
                    warn!(
                        "⚠️  LEADER {} appears to have failed! Starting election...",
                        peer_id
                    );
                    *self.current_leader.write().await = None;
                    self.initiate_election().await;
                }
            }
        }
    }

    // ========================================================================
    // ELECTION LOGIC
    // ========================================================================

    /// Initiate a new leader election using the Modified Bully Algorithm.
    ///
    /// # Election Process
    ///
    /// 1. Calculate our priority based on current CPU, tasks, and memory
    /// 2. Broadcast ELECTION message to all peers with our priority
    /// 3. Wait for ALIVE responses (from servers with lower priority)
    /// 4. If no ALIVE received, we won - broadcast COORDINATOR message
    /// 5. If ALIVE received, we lost - wait for winner to announce
    ///
    /// # Priority Calculation
    ///
    /// Lower priority score = better candidate (less loaded)
    /// - 50% weight: CPU usage
    /// - 30% weight: Active tasks
    /// - 20% weight: Memory usage
    async fn initiate_election(&self) {
        *self.received_alive.write().await = false;
        info!("🗳️  Server {} initiating election", self.config.server.id);

        // Calculate priority based on REAL metrics
        let my_priority = self.metrics.calculate_priority();
        let cpu = self.metrics.get_cpu_usage();
        let tasks = self.metrics.get_active_tasks();
        let memory = self.metrics.get_available_memory_percent();

        info!(
            "📊 Server {} priority: {:.2} (CPU: {:.1}%, Tasks: {}, Memory: {:.1}% available)",
            self.config.server.id, my_priority, cpu, tasks, memory
        );

        // Send election message with our priority
        let election_msg = Message::Election {
            from_id: self.config.server.id,
            priority: my_priority,
        };

        info!(
            "📤 Server {} broadcasting ELECTION message",
            self.config.server.id
        );
        self.broadcast(election_msg).await;

        // Wait for responses
        info!(
            "⏳ Server {} waiting {}s for election responses...",
            self.config.server.id, self.config.election.election_timeout_secs
        );
        tokio::time::sleep(Duration::from_secs(
            self.config.election.election_timeout_secs,
        ))
        .await;

        // Check if we won
        if !*self.received_alive.read().await {
            info!(
                "🎉 Server {} won election! (lowest priority score: {:.2})",
                self.config.server.id, my_priority
            );

            *self.current_leader.write().await = Some(self.config.server.id);

            let coordinator_msg = Message::Coordinator {
                leader_id: self.config.server.id,
            };

            info!(
                "📤 Server {} broadcasting COORDINATOR message",
                self.config.server.id
            );
            self.broadcast(coordinator_msg).await;
        } else {
            info!(
                "📊 Server {} lost election (higher load than others)",
                self.config.server.id
            );
        }
    }

    // ========================================================================
    // HELPER FUNCTIONS
    // ========================================================================

    /// Broadcast a message to all connected peers.
    ///
    /// # Arguments
    /// - `message`: The message to send (will be cloned for each peer)
    ///
    /// Messages are sent asynchronously via channels - this method returns
    /// immediately after queuing the messages.
    async fn broadcast(&self, message: Message) {
        let connections = self.peer_connections.read().await;
        for (peer_id, tx) in connections.iter() {
            match tx.send(message.clone()).await {
                Ok(_) => {
                    debug!("📤 Sent message to peer {}", peer_id);
                }
                Err(e) => {
                    debug!("❌ Failed to send to peer {}: {}", peer_id, e);
                }
            }
        }
    }

    /// Send a message to a specific peer.
    ///
    /// # Arguments
    /// - `peer_id`: The ID of the target peer
    /// - `message`: The message to send
    ///
    /// If the peer is not connected, the message is silently dropped.
    async fn send_to_peer(&self, peer_id: u32, message: Message) {
        let connections = self.peer_connections.read().await;
        if let Some(tx) = connections.get(&peer_id) {
            match tx.send(message).await {
                Ok(_) => {
                    debug!("📤 Sent message to peer {}", peer_id);
                }
                Err(e) => {
                    debug!("❌ Failed to send to peer {}: {}", peer_id, e);
                }
            }
        } else {
            debug!("❌ No connection to peer {}", peer_id);
        }
    }

    /// Create an Arc-wrapped clone of this server.
    ///
    /// Needed because we need to pass the server to async tasks.
    /// All fields are already Arc/RwLock wrapped, so this is cheap.
    fn clone_arc(&self) -> Arc<Self> {
        Arc::new(Self {
            core: self.core.clone(),
            config: self.config.clone(),
            metrics: self.metrics.clone(),
            current_leader: self.current_leader.clone(),
            received_alive: self.received_alive.clone(),
            peer_connections: self.peer_connections.clone(),
            last_heartbeat_times: self.last_heartbeat_times.clone(),
            active_tasks: self.active_tasks.clone(),
            peer_loads: self.peer_loads.clone(),
            task_history: self.task_history.clone(),
        })
    }

    /// Process an encryption task by delegating to ServerCore.
    ///
    /// # Arguments
    /// - `request_id`: Unique identifier for this task
    /// - `client_name`: Name of the client that submitted this task
    /// - `image_data`: Raw image bytes
    /// - `image_name`: Original filename
    /// - `text_to_embed`: Text to hide in the image
    /// - `response_tx`: Optional channel to send response on
    ///
    /// # Process
    ///
    /// 1. Increment active task counter (for load calculation)
    /// 2. Spawn async task to perform encryption via ServerCore
    /// 3. Send response back through channel (if provided)
    /// 4. Remove task from history (broadcast to all peers)
    /// 5. Decrement active task counter
    ///
    /// The encryption is performed in a blocking thread pool via ServerCore
    /// to avoid blocking the async runtime.
    async fn process_task(
        &self,
        request_id: u64,
        client_name: String,
        image_data: Vec<u8>,
        image_name: String,
        text_to_embed: String,
        response_tx: Option<mpsc::Sender<Message>>,
    ) {
        // START TRACKING: Increment active task count
        self.metrics.task_started();

        let current_tasks = self.metrics.get_active_tasks();
        let cpu_usage = self.metrics.get_cpu_usage();

        info!(
            "📊 Server {} starting task #{} (Active tasks: {}, CPU: {:.1}%)",
            self.config.server.id, request_id, current_tasks, cpu_usage
        );

        // Process task in background
        let server = self.clone_arc();
        let handle = tokio::spawn(async move {
            info!(
                "📷 Server {} processing encryption request #{} from client '{}'",
                server.config.server.id, request_id, client_name
            );

            // Delegate to ServerCore for actual encryption
            let encryption_result = server.core.encrypt_image(
                request_id,
                client_name.clone(),
                image_data,
                image_name,
                text_to_embed,
            ).await;

            let response = match encryption_result {
                Ok(encrypted_data) => {
                    info!(
                        "✅ Server {} completed encryption for request #{}",
                        server.config.server.id, request_id
                    );

                    Message::TaskResponse {
                        request_id,
                        encrypted_image_data: encrypted_data,
                        success: true,
                        error_message: None,
                    }
                }
                Err(e) => {
                    error!(
                        "❌ Server {} failed to encrypt image: {}",
                        server.config.server.id, e
                    );

                    Message::TaskResponse {
                        request_id,
                        encrypted_image_data: Vec::new(),
                        success: false,
                        error_message: Some(e.to_string()),
                    }
                }
            };

            // Send response if channel exists
            if let Some(tx) = response_tx {
                if let Err(e) = tx.send(response).await {
                    error!("❌ Failed to send response: {}", e);
                }
            }

            // Remove from history and broadcast to all servers
            let history_remove_msg = Message::HistoryRemove {
                client_name: client_name.clone(),
                request_id,
            };

            // Remove from own history
            server
                .task_history
                .write()
                .await
                .remove(&(client_name.clone(), request_id));

            // Broadcast to all peers
            server.broadcast(history_remove_msg).await;

            // FINISH TRACKING: Decrement active task count
            server.metrics.task_finished();

            let remaining_tasks = server.metrics.get_active_tasks();
            let new_cpu = server.metrics.get_cpu_usage();

            info!(
                "✅ Server {} completed task #{} (Remaining tasks: {}, CPU: {:.1}%)",
                server.config.server.id, request_id, remaining_tasks, new_cpu
            );
        });

        // Track the task handle
        self.active_tasks.write().await.insert(request_id, handle);
    }
}
