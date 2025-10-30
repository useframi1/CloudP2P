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

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub server: ServerInfo,       // Info about THIS server
    pub peers: PeersConfig,       // Info about OTHER servers
    pub election: ElectionConfig, // Election timing settings
}

#[allow(dead_code)]
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
    pub monitor_interval_secs: u64,   // How often to check for failed peers
}

// NEW: Client information structure
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub name: String, // Name of the client
}

#[allow(dead_code)]
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
                if length > 50_000_000 {
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
// TASK HISTORY - For fault tolerance tracking
// ============================================================================

#[derive(Debug, Clone)]
struct TaskHistoryEntry {
    _client_name: String,
    _request_id: u64,
    assigned_server_id: u32,
    _timestamp: u64,
}

// ============================================================================
// SERVER - The main server that does leader election
// ============================================================================

#[allow(dead_code)]
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

    // Task history for fault tolerance: (client_name, request_id) -> entry
    task_history: Arc<RwLock<HashMap<(String, u64), TaskHistoryEntry>>>,
}

#[allow(dead_code)]
impl Server {
    // Create a new server with the given configuration
    pub fn new(config: ServerConfig) -> Self {
        // Initialize metrics from config
        let metrics = ServerMetrics::new();

        Self {
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

                // If we have higher priority, respond and start our own election
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
                    "📥 Server {} received task #{} from 🔵 {} (assigned by leader {})",
                    self.config.server.id, request_id, client_name, assigned_by_leader
                );

                // Create a channel for response
                let (tx, mut rx) = mpsc::channel::<Message>(1);

                // Process the task (this part stays the same)
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
                    let timestamp = crate::messages::current_timestamp();
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

    // Start a new election
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
            task_history: self.task_history.clone(),
        })
    }

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
                "📷 Server {} processing encryption request #{} from 🔵 {}",
                server.config.server.id, request_id, client_name
            );

            // Do the actual encryption using spawn_blocking to avoid blocking async threads
            let encryption_result = tokio::task::spawn_blocking(move || {
                crate::steganography::embed_text_bytes(&image_data, &text_to_embed)
            })
            .await
            .expect("Encryption task panicked");

            let response = match encryption_result {
                Ok(encrypted_data) => {
                    info!(
                        "✅ Server {} completed encryption for request #{}",
                        server.config.server.id, request_id
                    );

                    // Save encrypted image
                    let output_path = format!("user-data/outputs/encrypted_{}", image_name);
                    if let Err(e) = std::fs::write(&output_path, &encrypted_data) {
                        error!("❌ Failed to save encrypted image: {}", e);
                    }

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
