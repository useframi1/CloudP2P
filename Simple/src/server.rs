use anyhow::Result;
use log::{debug, error, info, warn};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, RwLock};

use crate::election::ServerMetrics;
use crate::messages::*;

// ============================================================================
// CONFIGURATION - What each server needs to know about itself and others
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub server: ServerInfo,       // Info about THIS server
    pub peers: PeersConfig,       // Info about OTHER servers
    pub election: ElectionConfig, // Election timing settings
    pub metrics: MetricsConfig,   // Initial performance metrics
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub id: u32,         // Unique ID for this server (1, 2, or 3)
    pub address: String, // Where this server listens (e.g., "127.0.0.1:8001")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeersConfig {
    pub peers: Vec<PeerInfo>, // List of other servers to connect to
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub id: u32,         // ID of the peer server
    pub address: String, // Where to connect to the peer
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElectionConfig {
    pub heartbeat_interval_secs: u64, // How often to send "I'm alive" messages
    pub election_timeout_secs: u64,   // How long to wait during election
    pub failure_timeout_secs: u64,    // How long before we consider a server dead
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    pub initial_load: f64,          // Starting load (0.0-1.0, lower is better)
    pub initial_reliability: f64,   // Starting reliability (0.0-1.0, higher is better)
    pub initial_response_time: f64, // Starting response time in ms (lower is better)
}

impl ServerConfig {
    // Load configuration from a TOML file
    pub fn from_file(path: &str) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: ServerConfig = toml::from_str(&content)?;
        Ok(config)
    }
}

// ============================================================================
// CONNECTION - Handles reading/writing messages over TCP
// ============================================================================

pub struct Connection {
    stream: TcpStream,
}

impl Connection {
    pub fn new(stream: TcpStream) -> Self {
        Self { stream }
    }

    // Read a message from the connection
    // Returns None if connection is closed
    pub async fn read_message(&mut self) -> Result<Option<Message>> {
        // First, read 4 bytes that tell us how long the message is
        let mut length_buf = [0u8; 4];

        match self.stream.read_exact(&mut length_buf).await {
            Ok(_) => {
                let length = u32::from_be_bytes(length_buf) as usize;

                // Sanity check: don't allow messages larger than 10MB
                if length > 10_000_000 {
                    error!("❌ Message too large: {} bytes", length);
                    return Ok(None);
                }

                // Now read the actual message data
                let mut data = vec![0u8; length];
                self.stream.read_exact(&mut data).await?;

                // Convert bytes back into a Message
                match Message::from_bytes(&data) {
                    Ok(msg) => Ok(Some(msg)),
                    Err(e) => {
                        error!("❌ Failed to deserialize message: {}", e);
                        Ok(None)
                    }
                }
            }
            Err(_) => Ok(None), // Connection closed cleanly
        }
    }

    // Write a message to the connection
    pub async fn write_message(&mut self, message: &Message) -> Result<()> {
        // Convert message to bytes
        let data = message.to_bytes()?;
        let length = data.len() as u32;

        // Send: [4 bytes length][message data]
        self.stream.write_all(&length.to_be_bytes()).await?;
        self.stream.write_all(&data).await?;
        self.stream.flush().await?;

        Ok(())
    }
}

// ============================================================================
// SERVER - The main server that does leader election
// ============================================================================

pub struct Server {
    // Configuration loaded from TOML file
    config: ServerConfig,

    // Performance metrics used to calculate priority during election
    metrics: ServerMetrics,

    // Current leader (None if no leader, Some(id) if we have a leader)
    current_leader: Arc<RwLock<Option<u32>>>,
    received_alive: Arc<RwLock<bool>>,
    // Peer connections: peer_id -> channel to send messages to that peer
    // We use channels so we can send messages from anywhere in the code
    peer_connections: Arc<RwLock<HashMap<u32, mpsc::Sender<Message>>>>,

    // Last time we heard from each peer (used to detect failures)
    last_heartbeat_times: Arc<RwLock<HashMap<u32, u64>>>,
    // NEW: Track active tasks
    active_tasks: Arc<RwLock<HashMap<u64, tokio::task::JoinHandle<()>>>>,

    peer_loads: Arc<RwLock<HashMap<u32, f64>>>,
}

impl Server {
    // Create a new server with the given configuration
    pub fn new(config: ServerConfig) -> Self {
        // Initialize metrics from config
        let metrics = ServerMetrics::new(
            config.metrics.initial_load,
            config.metrics.initial_reliability,
            config.metrics.initial_response_time,
        );

        Self {
            config,
            metrics,
            current_leader: Arc::new(RwLock::new(None)),
            received_alive: Arc::new(RwLock::new(false)),
            peer_connections: Arc::new(RwLock::new(HashMap::new())),
            last_heartbeat_times: Arc::new(RwLock::new(HashMap::new())),
            active_tasks: Arc::new(RwLock::new(HashMap::new())),
            peer_loads: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    // Main entry point - starts all server tasks
    pub async fn run(&self) {
        info!(
            "🚀 Server {} starting on {}",
            self.config.server.id, self.config.server.address
        );

        // After 3 seconds + random delay, start an election
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

        // Run all tasks concurrently
        tokio::select! {
            _ = listener_task => error!("❌ Listener task terminated"),
            _ = peer_task => error!("❌ Peer connection task terminated"),
            _ = heartbeat_task => error!("❌ Heartbeat task terminated"),
            _ = monitor_task => error!("❌ Monitor task terminated"),
        }
    }

    // ========================================================================
    // TASK 1: Listen for incoming connections from peers
    // ========================================================================
    async fn start_listener(&self) {
        // Bind to our address and start listening
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

    // Handle a single connection - read messages in a loop
    async fn handle_connection(&self, socket: TcpStream) {
        let mut conn = Connection::new(socket);

        loop {
            match conn.read_message().await {
                Ok(Some(message)) => {
                    // Check if it's a LeaderQuery
                    if matches!(message, Message::LeaderQuery) {
                        let leader = *self.current_leader.read().await;
                        if let Some(leader_id) = leader {
                            let response = Message::LeaderResponse { leader_id };
                            let _ = conn.write_message(&response).await;
                        }
                        continue; // Don't process this as a normal message
                    }

                    // Normal message handling
                    self.handle_message(message).await;
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
    async fn connect_to_peers(&self) {
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
                                "✅ Server {} connected to peer {}",
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
    async fn handle_message(&self, message: Message) {
        match message {
            // Someone started an election
            Message::Election { from_id, priority } => {
                info!(
                    "🗳️  Server {} received ELECTION from {} (priority: {:.2})",
                    self.config.server.id, from_id, priority
                );

                // Calculate our priority
                let my_priority = self.metrics.calculate_priority();

                // If we have higher priority, respond and start our own election
                if my_priority > priority {
                    info!(
                        "💪 Server {} has higher priority ({:.2} > {:.2}), responding with ALIVE",
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
                        "📊 Server {} has lower priority ({:.2} < {:.2}), deferring",
                        self.config.server.id, my_priority, priority
                    );
                }
            }

            // Someone responded to our election with "I'm alive and have higher priority"
            Message::Alive { from_id } => {
                info!(
                    "👋 Server {} received ALIVE from {} (they have higher priority)",
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
            // NEW: Client asking who the leader is
            Message::LeaderQuery => {
                let leader = *self.current_leader.read().await;
                if let Some(leader_id) = leader {
                    info!("📋 Client queried leader, responding with: {}", leader_id);
                    // Response is sent back through the same connection
                    // (handled in handle_connection)
                }
            }

            // NEW: Client sending a task
            Message::TaskRequest {
                task_id,
                processing_time_ms,
                load_impact,
            } => {
                info!(
                    "📥 Server {} received task #{}",
                    self.config.server.id, task_id
                );

                // Check if we are the leader
                let current_leader = *self.current_leader.read().await;
                let am_i_leader = current_leader == Some(self.config.server.id);

                if am_i_leader {
                    // We're the leader - decide who should handle this task
                    let my_load = self.metrics.get_load();
                    let peer_loads = self.peer_loads.read().await;

                    // Print all server loads for debugging
                    info!("📊 LOAD DISTRIBUTION:");
                    info!(
                        "   Server {} (me, leader): {:.2}",
                        self.config.server.id, my_load
                    );
                    for (peer_id, peer_load) in peer_loads.iter() {
                        info!("   Server {}: {:.2}", peer_id, peer_load);
                    }

                    // Find the server with the lowest load (including ourselves)
                    let mut lowest_load = my_load;
                    let mut best_server = self.config.server.id;

                    for (peer_id, peer_load) in peer_loads.iter() {
                        if *peer_load < lowest_load {
                            lowest_load = *peer_load;
                            best_server = *peer_id;
                        }
                    }

                    if best_server == self.config.server.id {
                        // We have the lowest load, process it ourselves
                        info!(
                            "✅ Task #{} assigned to Server {} (me) - lowest load: {:.2}",
                            task_id, self.config.server.id, my_load
                        );

                        self.process_task(task_id, processing_time_ms, load_impact)
                            .await;
                    } else {
                        // Delegate to the server with lowest load
                        info!(
                            "📤 Task #{} delegated to Server {} - their load: {:.2} vs my load: {:.2}",
                            task_id, best_server, lowest_load, my_load
                        );

                        let delegate_msg = Message::TaskDelegate {
                            task_id,
                            processing_time_ms,
                            load_impact,
                        };
                        self.send_to_peer(best_server, delegate_msg).await;
                    }
                } else {
                    // We're not the leader, just process the task
                    info!(
                        "📥 Server {} (follower) processing task #{}",
                        self.config.server.id, task_id
                    );
                    self.process_task(task_id, processing_time_ms, load_impact)
                        .await;
                }
            }
            Message::TaskDelegate {
                task_id,
                processing_time_ms,
                load_impact,
            } => {
                info!(
                    "📨 Server {} received delegated task #{} from leader",
                    self.config.server.id, task_id
                );

                self.process_task(task_id, processing_time_ms, load_impact)
                    .await;
            }
            Message::TaskAck { .. } => {
                // Client doesn't need to handle this
            }

            Message::LeaderResponse { .. } => {
                // Servers don't need to handle this
            }
        }
    }

    // ========================================================================
    // TASK 3: Send heartbeats periodically
    // ========================================================================
    async fn start_heartbeat(&self) {
        let interval = self.config.election.heartbeat_interval_secs;

        loop {
            // Wait for the heartbeat interval
            tokio::time::sleep(Duration::from_secs(interval)).await;

            // Create a heartbeat message
            let heartbeat = Message::Heartbeat {
                from_id: self.config.server.id,
                timestamp: current_timestamp(),
                load: self.metrics.get_load(),
            };

            debug!(
                "💓 Server {} sending heartbeat (load: {:.2})",
                self.config.server.id,
                self.metrics.get_load()
            );

            // Send to all peers
            self.broadcast(heartbeat).await;
        }
    }

    // ========================================================================
    // TASK 4: Monitor heartbeats and detect failures
    // ========================================================================
    async fn monitor_heartbeats(&self) {
        loop {
            tokio::time::sleep(Duration::from_secs(
                self.config.election.failure_timeout_secs,
            ))
            .await;

            let now = current_timestamp();
            let timeout = self.config.election.failure_timeout_secs;
            let heartbeats = self.last_heartbeat_times.read().await;
            let current_leader = *self.current_leader.read().await;

            // Check if any peer has timed out
            for (peer_id, last_seen) in heartbeats.iter() {
                if now - last_seen > timeout {
                    warn!(
                        "⚠️  Server {} detected peer {} may have failed (no heartbeat for {}s)",
                        self.config.server.id,
                        peer_id,
                        now - last_seen
                    );

                    // If the leader failed, start a new election
                    if Some(*peer_id) == current_leader {
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
    }

    // ========================================================================
    // ELECTION LOGIC
    // ========================================================================

    // Start a new election
    async fn initiate_election(&self) {
        *self.received_alive.write().await = false;
        info!("🗳️  Server {} initiating election", self.config.server.id);

        // Calculate our priority based on metrics
        let my_priority = self.metrics.calculate_priority();
        info!(
            "📊 Server {} priority: {:.2} (load: {:.2}, reliability: {:.2}, response_time: {:.2}ms)",
            self.config.server.id,
            my_priority,
            self.metrics.get_load(),
            self.metrics.get_reliability(),
            self.metrics.get_response_time()
        );

        // Send election message to all peers
        let election_msg = Message::Election {
            from_id: self.config.server.id,
            priority: my_priority,
        };

        info!(
            "📤 Server {} broadcasting ELECTION message",
            self.config.server.id
        );
        self.broadcast(election_msg).await;

        // Wait for responses (election timeout)
        info!(
            "⏳ Server {} waiting {}s for election responses...",
            self.config.server.id, self.config.election.election_timeout_secs
        );
        tokio::time::sleep(Duration::from_secs(
            self.config.election.election_timeout_secs,
        ))
        .await;

        // Check if we received any ALIVE responses
        if !*self.received_alive.read().await {
            info!(
                "🎉 Server {} won the election! Declaring as LEADER (priority: {:.2})",
                self.config.server.id, my_priority
            );

            // Set ourselves as leader
            *self.current_leader.write().await = Some(self.config.server.id);

            // Announce to everyone
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
                "📊 Server {} lost the election (received ALIVE responses)",
                self.config.server.id
            );
        }
    }

    // ========================================================================
    // HELPER FUNCTIONS
    // ========================================================================

    // Send a message to all peers
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

    // Send a message to a specific peer
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

    // Create an Arc-wrapped clone of this server
    // Needed because we need to pass the server to async tasks
    fn clone_arc(&self) -> Arc<Self> {
        Arc::new(Self {
            config: self.config.clone(),
            metrics: self.metrics.clone(),
            current_leader: self.current_leader.clone(),
            received_alive: self.received_alive.clone(),
            peer_connections: self.peer_connections.clone(),
            last_heartbeat_times: self.last_heartbeat_times.clone(),
            active_tasks: self.active_tasks.clone(),
            peer_loads: self.peer_loads.clone(),
        })
    }

    async fn process_task(&self, task_id: u64, processing_time_ms: u64, load_impact: f64) {
        // Increase load immediately
        let current_load = self.metrics.get_load();
        self.metrics.set_load(current_load + load_impact);

        info!(
            "📊 Server {} load: {:.2} → {:.2}",
            self.config.server.id,
            current_load,
            current_load + load_impact
        );

        // Process task in background
        let server = self.clone_arc();
        let handle = tokio::spawn(async move {
            // Simulate processing
            tokio::time::sleep(Duration::from_millis(processing_time_ms)).await;

            // Decrease load when done
            let new_load = server.metrics.get_load() - load_impact;
            server.metrics.set_load(new_load.max(0.0));

            info!(
                "✅ Server {} completed task #{}, load now: {:.2}",
                server.config.server.id, task_id, new_load
            );
        });

        // Track the task
        self.active_tasks.write().await.insert(task_id, handle);
    }
}
