use log::{debug, error, info, warn};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

use crate::discovery::service::DiscoveryService;
use crate::election::modified_bully::ModifiedBullyElection;
use crate::encryption::service::EncryptionService;
use crate::messages::*;
use crate::server::config::ServerConfig;
use crate::server::connection::Connection;
use crate::server::metrics::ServerMetrics;
use crate::utils::current_timestamp;

pub struct Server {
    config: ServerConfig,
    metrics: ServerMetrics,

    // State
    current_leader: Arc<RwLock<Option<u32>>>,
    is_failed: Arc<AtomicBool>,

    // Peer connections
    peer_connections: Arc<RwLock<HashMap<u32, mpsc::Sender<Message>>>>,
    last_heartbeat_times: Arc<RwLock<HashMap<u32, u64>>>,

    // Services
    election_manager: Arc<ModifiedBullyElection>,
    encryption_service: Arc<EncryptionService>,
    discovery_service: Arc<DiscoveryService>,

    // Work distribution
    pending_proposals: Arc<RwLock<HashMap<Uuid, Vec<WorkProposal>>>>,
}

#[derive(Debug, Clone)]
struct WorkProposal {
    from_id: u32,
    priority: f64,
}

impl Server {
    pub fn new(config: ServerConfig) -> Self {
        let metrics = ServerMetrics::new(
            config.metrics.initial_load,
            config.metrics.initial_reliability,
            config.metrics.initial_response_time,
        );

        let election_manager = Arc::new(ModifiedBullyElection::new(
            config.server.id,
            metrics.clone(),
        ));

        let encryption_service =
            Arc::new(EncryptionService::new(config.encryption.thread_pool_size));

        let discovery_service = Arc::new(DiscoveryService::new());

        Self {
            config,
            metrics,
            current_leader: Arc::new(RwLock::new(None)),
            is_failed: Arc::new(AtomicBool::new(false)),
            peer_connections: Arc::new(RwLock::new(HashMap::new())),
            last_heartbeat_times: Arc::new(RwLock::new(HashMap::new())),
            election_manager,
            encryption_service,
            discovery_service,
            pending_proposals: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn run(&self) {
        info!(
            "🚀 Server {} starting on {}",
            self.config.server.id, self.config.server.address
        );

        // Start initial election as a separate spawned task (not blocking)
        let server_clone = self.clone_arc();
        tokio::spawn(async move {
            server_clone.start_election_process().await;
        });

        // Start all long-running tasks
        let listener_task = self.start_listener();
        let peer_task = self.connect_to_peers();
        let heartbeat_task = self.start_heartbeat();
        let monitor_task = self.monitor_heartbeats();

        // These tasks should run forever, so select will keep server alive
        tokio::select! {
            _ = listener_task => {
                error!("Listener task unexpectedly terminated");
            },
            _ = peer_task => {
                error!("Peer connection task unexpectedly terminated");
            },
            _ = heartbeat_task => {
                error!("Heartbeat task unexpectedly terminated");
            },
            _ = monitor_task => {
                error!("Monitor task unexpectedly terminated");
            },
        }
    }

    async fn start_listener(&self) {
        let listener = match TcpListener::bind(&self.config.server.address).await {
            Ok(l) => l,
            Err(e) => {
                error!("Failed to bind to {}: {}", self.config.server.address, e);
                return;
            }
        };

        info!(
            "📡 Server {} listening on {}",
            self.config.server.id, self.config.server.address
        );

        loop {
            match listener.accept().await {
                Ok((socket, addr)) => {
                    debug!(
                        "🔗 Server {} accepted connection from {}",
                        self.config.server.id, addr
                    );

                    let server = self.clone_arc();
                    tokio::spawn(async move {
                        server.handle_connection(socket).await;
                    });
                }
                Err(e) => {
                    error!("Accept error: {}", e);
                }
            }
        }
    }

    async fn handle_connection(&self, socket: TcpStream) {
        let mut conn = Connection::new(socket);

        loop {
            // Check if we're in failed state
            if self.is_failed.load(Ordering::Relaxed) {
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }

            match conn.read_message().await {
                Ok(Some(message)) => {
                    self.handle_message(message).await;
                }
                Ok(None) => break, // Connection closed
                Err(e) => {
                    error!("Error reading message: {}", e);
                    break;
                }
            }
        }
    }

    async fn connect_to_peers(&self) {
        tokio::time::sleep(Duration::from_secs(1)).await;

        for peer in &self.config.peers.peers {
            let peer_id = peer.id;
            let peer_addr = peer.address.clone();
            let server = self.clone_arc();

            tokio::spawn(async move {
                loop {
                    match TcpStream::connect(&peer_addr).await {
                        Ok(stream) => {
                            info!(
                                "✅ Server {} connected to peer {}",
                                server.config.server.id, peer_id
                            );

                            let (tx, mut rx) = mpsc::channel::<Message>(100);
                            server.peer_connections.write().await.insert(peer_id, tx);

                            let mut conn = Connection::new(stream);

                            // Send messages from channel to peer
                            while let Some(msg) = rx.recv().await {
                                if let Err(e) = conn.write_message(&msg).await {
                                    error!("Error sending to peer {}: {}", peer_id, e);
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
                        Err(_) => {}
                    }

                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            });
        }

        // ✅ Keep this function alive forever
        std::future::pending::<()>().await;
    }

    async fn handle_message(&self, message: Message) {
        match message {
            Message::Election { from_id, priority } => {
                self.handle_election_message(from_id, priority).await;
            }
            Message::Alive { from_id } => {
                debug!(
                    "👋 Server {} received ALIVE from {}",
                    self.config.server.id, from_id
                );
                *self.current_leader.write().await = Some(from_id);
            }
            Message::Coordinator { leader_id } => {
                info!(
                    "👑 Server {} acknowledges {} as LEADER",
                    self.config.server.id, leader_id
                );
                *self.current_leader.write().await = Some(leader_id);
            }
            Message::Heartbeat {
                from_id,
                timestamp,
                load,
            } => {
                self.last_heartbeat_times
                    .write()
                    .await
                    .insert(from_id, timestamp);
                debug!(
                    "💓 Server {} received heartbeat from {} (load: {:.2})",
                    self.config.server.id, from_id, load
                );
            }
            Message::WorkRequest {
                request_id,
                request_type,
                data,
                client_id,
            } => {
                self.handle_work_request(request_id, request_type, data, client_id)
                    .await;
            }
            Message::WorkProposal {
                request_id,
                from_id,
                priority,
            } => {
                self.handle_work_proposal(request_id, from_id, priority)
                    .await;
            }
            Message::RegisterUser {
                user_id,
                username,
                connection_info,
            } => {
                self.discovery_service
                    .register_user(user_id, username, connection_info)
                    .await;
            }
            Message::UnregisterUser { user_id } => {
                self.discovery_service.unregister_user(&user_id).await;
            }
            Message::GetOnlineUsers => {
                // Handle get online users (would send response back)
            }
            Message::Recovery {
                from_id,
                last_known_timestamp,
            } => {
                self.handle_recovery(from_id, last_known_timestamp).await;
            }
            Message::StateSync { state } => {
                self.apply_state(state).await;
            }
            Message::SimulateFail { duration_secs } => {
                self.simulate_failure(duration_secs).await;
            }
            _ => {}
        }
    }

    async fn handle_election_message(&self, from_id: u32, their_priority: f64) {
        info!(
            "🗳️  Server {} received ELECTION from {} (priority: {:.2})",
            self.config.server.id, from_id, their_priority
        );

        let my_priority = self.metrics.calculate_priority();

        if my_priority > their_priority {
            info!(
                "💪 Server {} has higher priority ({:.2} > {:.2})",
                self.config.server.id, my_priority, their_priority
            );

            // Send ALIVE
            let alive_msg = Message::Alive {
                from_id: self.config.server.id,
            };
            self.send_to_peer(from_id, alive_msg).await;

            // Start own election
            self.initiate_election().await;
        }
    }

    async fn handle_work_request(
        &self,
        request_id: Uuid,
        _request_type: RequestType,
        _data: Vec<u8>,
        client_id: String,
    ) {
        info!(
            "📨 Server {} received work request {:?} from client {}",
            self.config.server.id, request_id, client_id
        );

        // Initiate work distribution election
        self.elect_worker_for_request(request_id).await;
    }

    async fn elect_worker_for_request(&self, request_id: Uuid) {
        let my_priority = self.metrics.calculate_priority();

        let proposal = Message::WorkProposal {
            request_id,
            from_id: self.config.server.id,
            priority: my_priority,
        };

        // Store my own proposal
        self.pending_proposals
            .write()
            .await
            .entry(request_id)
            .or_insert_with(Vec::new)
            .push(WorkProposal {
                from_id: self.config.server.id,
                priority: my_priority,
            });

        // Broadcast to peers
        self.broadcast(proposal).await;

        // Wait for proposals
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Determine winner
        let proposals = self.pending_proposals.read().await;
        if let Some(proposal_list) = proposals.get(&request_id) {
            if let Some(winner) = proposal_list
                .iter()
                .max_by(|a, b| a.priority.partial_cmp(&b.priority).unwrap())
            {
                if winner.from_id == self.config.server.id {
                    info!(
                        "🎯 Server {} elected to process request {:?}",
                        self.config.server.id, request_id
                    );
                    self.process_request(request_id).await;
                } else {
                    debug!(
                        "📊 Server {} defers to {} for request {:?}",
                        self.config.server.id, winner.from_id, request_id
                    );
                }
            }
        }

        // Cleanup
        self.pending_proposals.write().await.remove(&request_id);
    }

    async fn handle_work_proposal(&self, request_id: Uuid, from_id: u32, priority: f64) {
        self.pending_proposals
            .write()
            .await
            .entry(request_id)
            .or_insert_with(Vec::new)
            .push(WorkProposal { from_id, priority });
    }

    async fn process_request(&self, request_id: Uuid) {
        // Simulate work
        tokio::time::sleep(Duration::from_millis(100)).await;
        info!(
            "✅ Server {} completed request {:?}",
            self.config.server.id, request_id
        );
    }

    async fn start_heartbeat(&self) {
        let interval = self.config.election.heartbeat_interval_secs;

        loop {
            tokio::time::sleep(Duration::from_secs(interval)).await;

            if self.is_failed.load(Ordering::Relaxed) {
                continue;
            }

            let heartbeat = Message::Heartbeat {
                from_id: self.config.server.id,
                timestamp: current_timestamp(),
                load: self.metrics.get_load(),
            };

            self.broadcast(heartbeat).await;
        }
    }

    async fn monitor_heartbeats(&self) {
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;

            let now = current_timestamp();
            let timeout = self.config.election.failure_timeout_secs;

            let heartbeats = self.last_heartbeat_times.read().await;
            let mut failed_peers = Vec::new();

            for (peer_id, last_seen) in heartbeats.iter() {
                if now - last_seen > timeout {
                    failed_peers.push(*peer_id);
                }
            }
            drop(heartbeats);

            for peer_id in failed_peers {
                self.handle_peer_failure(peer_id).await;
            }
        }
    }

    async fn handle_peer_failure(&self, peer_id: u32) {
        warn!(
            "⚠️  Server {} detected failure of Server {}",
            self.config.server.id, peer_id
        );

        let current_leader = *self.current_leader.read().await;

        if Some(peer_id) == current_leader {
            warn!("👑💥 Leader {} failed! Starting new election", peer_id);
            *self.current_leader.write().await = None;
            self.initiate_election().await;
        }
    }

    async fn start_election_process(&self) {
        tokio::time::sleep(Duration::from_secs(3)).await;
        self.initiate_election().await;
    }

    async fn initiate_election(&self) {
        *self.current_leader.write().await = None;
        info!("🗳️  Server {} initiating election", self.config.server.id);

        let my_priority = self.metrics.calculate_priority();

        let election_msg = Message::Election {
            from_id: self.config.server.id,
            priority: my_priority,
        };

        self.broadcast(election_msg).await;

        // Wait for responses
        tokio::time::sleep(Duration::from_secs(
            self.config.election.election_timeout_secs,
        ))
        .await;

        // If no one challenged, become leader
        if self.current_leader.read().await.is_none() {
            info!(
                "👑 Server {} declaring itself as LEADER (priority: {:.2})",
                self.config.server.id, my_priority
            );

            *self.current_leader.write().await = Some(self.config.server.id);

            let coordinator_msg = Message::Coordinator {
                leader_id: self.config.server.id,
            };

            self.broadcast(coordinator_msg).await;
        }
    }

    async fn handle_recovery(&self, from_id: u32, _last_known_timestamp: u64) {
        info!(
            "🔄 Server {} received recovery request from {}",
            self.config.server.id, from_id
        );

        let is_leader = self
            .current_leader
            .read()
            .await
            .map(|l| l == self.config.server.id)
            .unwrap_or(false);

        if is_leader {
            let state = StateSnapshot {
                leader_id: Some(self.config.server.id),
                active_peers: self.config.peers.peers.iter().map(|p| p.id).collect(),
                discovery_table: self.discovery_service.get_all_users().await,
                timestamp: current_timestamp(),
            };

            let sync_msg = Message::StateSync { state };
            self.send_to_peer(from_id, sync_msg).await;
        }
    }

    async fn apply_state(&self, state: StateSnapshot) {
        info!(
            "📊 Server {} applying state from leader",
            self.config.server.id
        );
        *self.current_leader.write().await = state.leader_id;
        self.discovery_service
            .apply_snapshot(state.discovery_table)
            .await;
        info!("✅ Server {} state synchronized", self.config.server.id);
    }

    async fn simulate_failure(&self, duration_secs: u64) {
        warn!(
            "💥 Server {} entering failure state for {}s",
            self.config.server.id, duration_secs
        );

        self.is_failed.store(true, Ordering::Relaxed);
        tokio::time::sleep(Duration::from_secs(duration_secs)).await;

        self.recover_from_failure().await;
    }

    async fn recover_from_failure(&self) {
        info!(
            "🔄 Server {} recovering from failure",
            self.config.server.id
        );

        self.is_failed.store(false, Ordering::Relaxed);

        let recovery_msg = Message::Recovery {
            from_id: self.config.server.id,
            last_known_timestamp: 0,
        };

        self.broadcast(recovery_msg).await;

        // Wait for state sync
        tokio::time::sleep(Duration::from_secs(2)).await;

        info!("✅ Server {} fully recovered", self.config.server.id);
    }

    async fn broadcast(&self, message: Message) {
        let connections = self.peer_connections.read().await;
        for tx in connections.values() {
            let _ = tx.send(message.clone()).await;
        }
    }

    async fn send_to_peer(&self, peer_id: u32, message: Message) {
        let connections = self.peer_connections.read().await;
        if let Some(tx) = connections.get(&peer_id) {
            let _ = tx.send(message).await;
        }
    }

    fn clone_arc(&self) -> Arc<Self> {
        Arc::new(Self {
            config: self.config.clone(),
            metrics: self.metrics.clone(),
            current_leader: self.current_leader.clone(),
            is_failed: self.is_failed.clone(),
            peer_connections: self.peer_connections.clone(),
            last_heartbeat_times: self.last_heartbeat_times.clone(),
            election_manager: self.election_manager.clone(),
            encryption_service: self.encryption_service.clone(),
            discovery_service: self.discovery_service.clone(),
            pending_proposals: self.pending_proposals.clone(),
        })
    }
}
