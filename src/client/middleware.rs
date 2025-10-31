//! # Client Middleware
//!
//! This module contains the middleware layer that orchestrates client operations
//! and handles all coordination concerns for distributed task execution.
//!
//! ## Responsibilities
//!
//! The [`ClientMiddleware`] struct manages high-level coordination:
//! - **Leader Discovery**: Queries multiple servers to find the current leader
//! - **Request Management**: Sends requests at configured rates with delays
//! - **Retry Logic**: Implements 3-attempt retry with 10-second timeout and 5-second delays
//! - **Server Assignment**: Requests server assignment from the leader
//! - **Fault Tolerance**: Handles server failures and re-discovers leaders
//! - **Configuration**: Loads and manages client settings from TOML files
//!
//! ## Architecture
//!
//! The middleware follows a separation of concerns pattern:
//! - It owns a [`ClientCore`](super::client::ClientCore) instance via `Arc` for the actual image transmission
//! - It tracks the current leader state
//! - It orchestrates the multi-step request workflow
//!
//! ## Request Workflow
//!
//! 1. **Discover Leader**: Query servers to find who is the current leader
//! 2. **Get Assignment**: Ask the leader for a server assignment
//! 3. **Execute Task**: Delegate to `ClientCore` to send image and receive result
//! 4. **Retry on Failure**: Retry up to 3 times with timeouts and delays
//!
//! ## Usage
//!
//! ```rust,ignore
//! use cloudp2p::client::middleware::ClientMiddleware;
//! use cloudp2p::client::client::ClientCore;
//! use std::sync::Arc;
//!
//! // Load configuration from TOML file
//! let config = ClientConfig::from_file("client_config.toml")?;
//!
//! // Create the core client
//! let core = Arc::new(ClientCore::new(config.client.name.clone()));
//!
//! // Create and run the middleware
//! let mut middleware = ClientMiddleware::new(config, core);
//! middleware.run().await;
//! ```

use anyhow::Result;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;

use crate::client::client::ClientCore;
use crate::common::connection::Connection;
use crate::common::messages::Message;

/// Client configuration loaded from TOML file.
///
/// This struct represents the complete configuration for a client, including
/// its identity and request parameters.
///
/// # Example TOML
///
/// ```toml
/// [client]
/// name = "Client1"
/// server_addresses = ["127.0.0.1:5001", "127.0.0.1:5002", "127.0.0.1:5003"]
///
/// [requests]
/// rate_per_second = 2.0
/// duration_seconds = 30.0
/// request_processing_ms = 100
/// load_per_request = 0.5
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    /// Client identity and server connection information
    pub client: ClientInfo,
    /// Request rate and processing parameters
    pub requests: RequestConfig,
}

/// Client identity and server addresses.
///
/// Contains the client's unique name and the list of known server addresses
/// for leader discovery and task execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    /// Unique name for this client (e.g., "Client1", "Client2")
    pub name: String,
    /// List of server addresses to query for leader discovery (e.g., ["127.0.0.1:5001", "127.0.0.1:5002"])
    pub server_addresses: Vec<String>,
}

/// Request rate and load configuration.
///
/// Defines how frequently the client sends requests and the simulated
/// load characteristics of each request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestConfig {
    /// Number of requests to send per second
    pub rate_per_second: f64,
    /// Total duration to send requests (in seconds)
    pub duration_seconds: f64,
    /// Simulated processing time per request (in milliseconds)
    pub request_processing_ms: u64,
    /// Simulated CPU load per request (arbitrary units)
    pub load_per_request: f64,
}

impl ClientConfig {
    /// Loads client configuration from a TOML file.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the TOML configuration file
    ///
    /// # Returns
    ///
    /// * `Ok(ClientConfig)` - Successfully parsed configuration
    /// * `Err(anyhow::Error)` - If file reading or parsing fails
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let config = ClientConfig::from_file("configs/client1.toml")?;
    /// println!("Client: {}", config.client.name);
    /// ```
    pub fn from_file(path: &str) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: ClientConfig = toml::from_str(&content)?;
        Ok(config)
    }
}

/// Client middleware that orchestrates distributed task execution.
///
/// This struct manages the coordination layer for client operations:
/// - Broadcasts assignment requests to all servers (leader responds)
/// - Manages request lifecycle (assignment, execution, retry)
/// - Delegates actual image transmission to the core client
///
/// # Fields
///
/// * `config` - Client configuration loaded from TOML
/// * `core` - Shared reference to the core client for image transmission
pub struct ClientMiddleware {
    /// Client configuration
    config: ClientConfig,
    /// Core client for image transmission (shared via Arc for potential future multi-threading)
    core: Arc<ClientCore>,
}

impl ClientMiddleware {
    /// Creates a new `ClientMiddleware` instance.
    ///
    /// # Arguments
    ///
    /// * `config` - Client configuration loaded from file
    /// * `core` - Arc-wrapped core client instance
    ///
    /// # Returns
    ///
    /// A new `ClientMiddleware` ready to run
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let config = ClientConfig::from_file("client_config.toml")?;
    /// let core = Arc::new(ClientCore::new(config.client.name.clone()));
    /// let middleware = ClientMiddleware::new(config, core);
    /// ```
    pub fn new(config: ClientConfig, core: Arc<ClientCore>) -> Self {
        Self { config, core }
    }

    /// Runs the main client loop, sending requests at the configured rate.
    ///
    /// This method:
    /// 1. Calculates the delay between requests based on `rate_per_second`
    /// 2. Sends the total number of requests over the configured duration
    /// 3. For each request, calls `send_request()` which handles retries
    /// 4. Only sleeps between requests if the previous request succeeded
    ///
    /// The loop continues until all requests have been sent or the duration elapses.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let mut middleware = ClientMiddleware::new(config, core);
    /// middleware.run().await;  // Blocks until all requests are sent
    /// ```
    pub async fn run(&mut self) {
        info!("Client '{}' starting", self.config.client.name);

        info!(
            "Client '{}' sending requests for {} seconds...",
            self.config.client.name, self.config.requests.duration_seconds
        );

        // Calculate delay between requests based on rate
        let delay = Duration::from_millis((1000.0 / self.config.requests.rate_per_second) as u64);
        let total_requests = (self.config.requests.rate_per_second
            * self.config.requests.duration_seconds as f64) as u64;

        // Send requests according to the configured rate
        for i in 1..=total_requests {
            let success = self
                .send_request(
                    i,
                    "test_image.jpg".to_string(),
                    "username:alice,views:5".to_string(),
                )
                .await;

            // Only sleep if task succeeded; if failed, immediately try next task
            if success {
                tokio::time::sleep(delay).await;
            }
        }

        info!("✅ Client finished sending {} requests", total_requests);
    }

    /// Broadcasts a task assignment request to all servers and waits for the leader's response.
    ///
    /// This method:
    /// 1. Sends `TaskAssignmentRequest` to all configured server addresses concurrently
    /// 2. Waits for the first valid `TaskAssignmentResponse` (from the leader)
    /// 3. Returns the assigned server ID, address, and which server was the leader
    ///
    /// Only the current leader will respond with an assignment. Non-leader servers will ignore
    /// the request or not respond.
    ///
    /// # Arguments
    ///
    /// * `request_num` - Unique identifier for this request
    ///
    /// # Returns
    ///
    /// * `Ok((assigned_server_id, assigned_address, leader_id))` - Assignment details and which server was leader
    /// * `Err(anyhow::Error)` - If no server responded with a valid assignment
    ///
    /// # Timeout
    ///
    /// Each server connection attempt has a 2-second timeout. Returns the first valid response.
    async fn broadcast_assignment_request(&self, request_num: u64) -> Result<(u32, String, u32)> {
        const CONNECTION_TIMEOUT_SECS: u64 = 2;

        info!(
            "📡 {} Broadcasting assignment request for task #{} to {} servers",
            self.config.client.name,
            request_num,
            self.config.client.server_addresses.len()
        );

        // Create futures for querying all servers concurrently
        let mut tasks = Vec::new();

        for (idx, address) in self.config.client.server_addresses.iter().enumerate() {
            let address = address.clone();
            let client_name = self.config.client.name.clone();
            let server_id = (idx + 1) as u32; // Server IDs are 1-indexed

            let task = tokio::spawn(async move {
                // Wrap in timeout
                let result = tokio::time::timeout(
                    Duration::from_secs(CONNECTION_TIMEOUT_SECS),
                    Self::request_assignment_from_server(&address, &client_name, request_num),
                )
                .await;

                match result {
                    Ok(Ok(assignment)) => Some((assignment, server_id)),
                    Ok(Err(_)) | Err(_) => None,
                }
            });

            tasks.push(task);
        }

        // Wait for all tasks and collect the first successful response
        for task in tasks {
            if let Ok(Some(((assigned_server_id, assigned_address), responder_id))) = task.await {
                info!(
                    "✅ {} Received assignment from leader (Server {}): Task #{} → Server {}",
                    self.config.client.name, responder_id, request_num, assigned_server_id
                );
                return Ok((assigned_server_id, assigned_address, responder_id));
            }
        }

        Err(anyhow::anyhow!(
            "No server responded with a task assignment (no leader available)"
        ))
    }

    /// Helper method to request assignment from a specific server.
    ///
    /// # Arguments
    ///
    /// * `address` - Server address to connect to
    /// * `client_name` - Name of this client
    /// * `request_num` - Request ID
    ///
    /// # Returns
    ///
    /// * `Ok((assigned_server_id, assigned_address))` - If server responded with assignment
    /// * `Err` - If connection failed or no valid response
    async fn request_assignment_from_server(
        address: &str,
        client_name: &str,
        request_num: u64,
    ) -> Result<(u32, String)> {
        // Connect to server
        let stream = TcpStream::connect(address).await?;
        let mut conn = Connection::new(stream);

        // Send assignment request
        let request = Message::TaskAssignmentRequest {
            client_name: client_name.to_string(),
            request_id: request_num,
        };
        conn.write_message(&request).await?;

        // Wait for response
        match conn.read_message().await? {
            Some(Message::TaskAssignmentResponse {
                request_id: _,
                assigned_server_id,
                assigned_server_address,
            }) => Ok((assigned_server_id, assigned_server_address)),
            _ => Err(anyhow::anyhow!("Invalid or no response from server")),
        }
    }

    /// Broadcasts a task status query to all servers and waits for a response.
    ///
    /// Used when the originally assigned server fails - client polls to discover
    /// if the task has been reassigned to a new server. Any server can respond
    /// by checking the shared task history.
    ///
    /// # Arguments
    ///
    /// * `request_num` - Request ID to query
    ///
    /// # Returns
    ///
    /// * `Ok((assigned_server_id, assigned_address))` - Current server assignment
    /// * `Err` - If no server responded with valid status
    async fn broadcast_status_query(&self, request_num: u64) -> Result<(u32, String)> {
        const CONNECTION_TIMEOUT_SECS: u64 = 2;

        info!(
            "🔍 {} Broadcasting status query for task #{} to {} servers",
            self.config.client.name,
            request_num,
            self.config.client.server_addresses.len()
        );

        // Create futures for querying all servers concurrently
        let mut tasks = Vec::new();

        for address in &self.config.client.server_addresses {
            let address = address.clone();
            let client_name = self.config.client.name.clone();

            let task = tokio::spawn(async move {
                // Wrap in timeout
                let result = tokio::time::timeout(
                    Duration::from_secs(CONNECTION_TIMEOUT_SECS),
                    Self::query_task_status(&address, &client_name, request_num),
                )
                .await;

                match result {
                    Ok(Ok(status)) => Some(status),
                    Ok(Err(_)) | Err(_) => None,
                }
            });

            tasks.push(task);
        }

        // Wait for first successful response
        for task in tasks {
            if let Ok(Some((assigned_server_id, assigned_address))) = task.await {
                info!(
                    "✅ {} Task #{} is assigned to Server {}",
                    self.config.client.name, request_num, assigned_server_id
                );
                return Ok((assigned_server_id, assigned_address));
            }
        }

        Err(anyhow::anyhow!(
            "No server responded with task status (task may not exist)"
        ))
    }

    /// Helper method to query task status from a specific server.
    ///
    /// # Arguments
    ///
    /// * `address` - Server address to query
    /// * `client_name` - Name of this client
    /// * `request_num` - Request ID to query
    ///
    /// # Returns
    ///
    /// * `Ok((assigned_server_id, assigned_address))` - Current assignment
    /// * `Err` - If connection failed or no valid response
    async fn query_task_status(
        address: &str,
        client_name: &str,
        request_num: u64,
    ) -> Result<(u32, String)> {
        // Connect to server
        let stream = TcpStream::connect(address).await?;
        let mut conn = Connection::new(stream);

        // Send status query
        let query = Message::TaskStatusQuery {
            client_name: client_name.to_string(),
            request_id: request_num,
        };
        conn.write_message(&query).await?;

        // Wait for response
        match conn.read_message().await? {
            Some(Message::TaskStatusResponse {
                request_id: _,
                assigned_server_id,
                assigned_server_address,
            }) => Ok((assigned_server_id, assigned_server_address)),
            _ => Err(anyhow::anyhow!("Invalid or no response from server")),
        }
    }

    /// Waits for task assignment/reassignment after server failure by polling all servers.
    ///
    /// When the assigned server fails, this method polls all servers (via broadcast)
    /// to get the current task assignment. The strategy is:
    /// 1. Prefer reassignment to a **different** server (immediate return)
    /// 2. If same server keeps being returned, retry after MAX_SAME_SERVER_POLLS attempts
    ///    (in case the server came back online)
    ///
    /// This method polls **indefinitely** until it gets an assignment.
    ///
    /// # Arguments
    ///
    /// * `request_num` - Request ID to wait for
    /// * `failed_address` - Address of the server that just failed
    ///
    /// # Returns
    ///
    /// * `Ok((assigned_server_id, assigned_address))` - Server assignment (always succeeds eventually)
    ///
    /// # Polling Behavior
    ///
    /// - Polls indefinitely with 2-second intervals
    /// - Immediately accepts reassignment to a different server
    /// - Retries same server after MAX_SAME_SERVER_POLLS attempts (server might have recovered)
    /// - Logs every polling attempt
    /// - Never gives up
    async fn wait_for_reassignment(
        &self,
        request_num: u64,
        failed_address: &str,
    ) -> Result<(u32, String)> {
        const POLL_INTERVAL_SECS: u64 = 2;
        const MAX_SAME_SERVER_POLLS: u32 = 10; // After 10 polls (20s), retry same server in case it recovered

        info!(
            "⏳ {} Polling for task #{} assignment after {} failed (indefinitely, 2s interval)...",
            self.config.client.name, request_num, failed_address
        );

        let mut attempt = 1;
        let mut same_server_count = 0;

        loop {
            info!(
                "🔄 {} Polling attempt {} for task #{}",
                self.config.client.name, attempt, request_num
            );

            match self.broadcast_status_query(request_num).await {
                Ok((server_id, address)) => {
                    if address != failed_address {
                        // Different server - immediately accept
                        info!(
                            "✅ {} Task #{} reassigned to different Server {} at {}",
                            self.config.client.name, request_num, server_id, address
                        );
                        return Ok((server_id, address));
                    } else {
                        // Same server - might have recovered, but wait a bit first
                        same_server_count += 1;

                        if same_server_count >= MAX_SAME_SERVER_POLLS {
                            info!(
                                "🔄 {} Task #{} still at {} after {} polls - will retry in case server recovered",
                                self.config.client.name, request_num, address, same_server_count
                            );
                            return Ok((server_id, address));
                        } else {
                            warn!(
                                "⏸️  {} Poll {}: Task #{} still at {} ({}/{} polls) - waiting for reassignment or recovery...",
                                self.config.client.name, attempt, request_num, failed_address,
                                same_server_count, MAX_SAME_SERVER_POLLS
                            );
                        }
                    }
                }
                Err(e) => {
                    warn!(
                        "Polling attempt {} failed for task #{}: {}",
                        attempt, request_num, e
                    );
                }
            }

            tokio::time::sleep(Duration::from_secs(POLL_INTERVAL_SECS)).await;
            attempt += 1;
        }
    }

    /// Sends a request with server-side failover handling.
    ///
    /// This method implements the complete workflow:
    /// 1. Polls indefinitely to get initial server assignment from leader (waits for leader if none available)
    /// 2. Executes task on assigned server
    /// 3. If server fails, polls indefinitely for reassignment
    /// 4. Retries on new/recovered server indefinitely until success
    ///
    /// # Arguments
    ///
    /// * `request_num` - Unique identifier for this request
    /// * `image_name` - Name of the image file to process (relative to uploads directory)
    /// * `text_to_embed` - Text to embed in the image using steganography
    ///
    /// # Returns
    ///
    /// * `true` - If the request succeeded
    /// * `false` - If the request failed
    ///
    /// # Polling Parameters
    ///
    /// - **Max poll attempts**: Unlimited - polls indefinitely per server failure
    /// - **Poll interval**: 2 seconds
    /// - **No timeout**: Request continues retrying indefinitely until success
    async fn send_request(
        &mut self,
        request_num: u64,
        image_name: String,
        text_to_embed: String,
    ) -> bool {
        const POLL_INTERVAL_SECS: u64 = 2;

        // Step 1: Get initial task assignment (poll indefinitely if no leader available)
        info!(
            "📡 {} Getting task assignment for task #{}",
            self.config.client.name, request_num
        );

        let (assigned_server_id, assigned_address, leader_id) = loop {
            match self.broadcast_assignment_request(request_num).await {
                Ok(assignment) => break assignment,
                Err(e) => {
                    warn!(
                        "Assignment request failed for task #{}: {} - waiting for leader...",
                        request_num, e
                    );
                    tokio::time::sleep(Duration::from_secs(POLL_INTERVAL_SECS)).await;
                }
            }
        };

        info!(
            "✅ {} Task #{} assigned to Server {} by leader {}",
            self.config.client.name, request_num, assigned_server_id, leader_id
        );

        // Step 2: Execute task on assigned server (handles failover internally)
        let result = self
            .execute_task(
                assigned_server_id,
                assigned_address,
                leader_id,
                request_num,
                image_name,
                text_to_embed,
            )
            .await;

        match result {
            Ok(()) => {
                info!(
                    "✅ {} Task #{} completed successfully",
                    self.config.client.name, request_num
                );
                true
            }
            Err(e) => {
                error!(
                    "❌ {} Task #{} FAILED: {}",
                    self.config.client.name, request_num, e
                );
                false
            }
        }
    }

    /// Executes a task with automatic server-side failover handling.
    ///
    /// This method:
    /// 1. Reads the image file from the uploads directory and caches it
    /// 2. Attempts to send task to assigned server
    /// 3. If server fails (TCP disconnect), polls for reassignment **indefinitely**
    /// 4. Polling: indefinite attempts, 2 seconds interval via broadcast to all servers
    /// 5. Retries with new server - if that server also fails, polls again indefinitely
    /// 6. Continues this cycle until the task succeeds
    ///
    /// # Arguments
    ///
    /// * `assigned_server_id` - ID of the initially assigned server
    /// * `assigned_address` - Network address of the initially assigned server
    /// * `leader_id` - ID of the leader that made the assignment
    /// * `request_num` - Unique identifier for this request
    /// * `image_name` - Name of the image file (relative to uploads directory)
    /// * `text_to_embed` - Text to embed in the image
    ///
    /// # Returns
    ///
    /// * `Ok(())` - If the task completed successfully (possibly after multiple reassignments)
    /// * `Err(anyhow::Error)` - Only for non-connection errors (e.g., file not found, validation errors)
    ///
    /// # Server Failover
    ///
    /// When the assigned server fails:
    /// 1. Client detects TCP connection break
    /// 2. Client polls all servers (broadcast TaskStatusQuery every 2s, indefinitely)
    /// 3. Leader has detected failure and reassigned the task
    /// 4. Client receives new assignment and retries
    /// 5. If the new server also fails, repeat from step 1
    /// 6. Continues indefinitely until success
    ///
    /// # File Locations
    ///
    /// - **Input**: `user-data/uploads/{image_name}`
    /// - **Output**: `user-data/outputs/encrypted_{client_name}_{image_name}`
    async fn execute_task(
        &self,
        _assigned_server_id: u32,
        mut assigned_address: String,
        mut leader_id: u32,
        request_num: u64,
        image_name: String,
        text_to_embed: String,
    ) -> Result<()> {
        // Read the image file once and cache it (avoid repeated disk I/O)
        let image_path = format!("user-data/uploads/{}", image_name);
        let image_data = std::fs::read(&image_path)?;

        loop {
            // Attempt to send task to assigned server
            let result = self
                .core
                .send_and_receive_encrypted_image(
                    &assigned_address,
                    request_num,
                    image_data.clone(), // Clone cached data
                    &text_to_embed,
                    leader_id,
                )
                .await;

            match result {
                Ok(()) => {
                    return Ok(());
                }
                Err(e) => {
                    // Check if it's a connection error (server failure)
                    let error_str = e.to_string();
                    let is_connection_error = error_str.contains("Connection")
                        || error_str.contains("connection")
                        || error_str.contains("refused")
                        || error_str.contains("timeout")
                        || error_str.contains("broken pipe");

                    if is_connection_error {
                        warn!(
                            "⚠️  {} Server failure detected for task #{} at {}: {}",
                            self.config.client.name, request_num, assigned_address, e
                        );

                        // Store the failed address
                        let failed_address = assigned_address.clone();

                        // Poll indefinitely until we get a valid assignment
                        // (prefer different server, but retry same server after 10 polls in case it recovered)
                        let (new_server_id, new_address) = self
                            .wait_for_reassignment(request_num, &failed_address)
                            .await?;

                        info!(
                            "✅ {} Received assignment for task #{}: Server {} at {}",
                            self.config.client.name, request_num, new_server_id, new_address
                        );
                        assigned_address = new_address;
                        leader_id = new_server_id;
                        // Continue loop to retry with assigned server
                    } else {
                        // Non-connection error (e.g., validation error, disk error, etc.)
                        return Err(e);
                    }
                }
            }
        }
    }
}
