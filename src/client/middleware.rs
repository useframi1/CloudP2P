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
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::net::TcpStream;

use crate::client::client::ClientCore;
use crate::client::metrics::ClientMetrics;
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
    /// Directory containing images to randomly select from (default: "test_images")
    #[serde(default = "default_image_dir")]
    pub image_dir: String,
}

fn default_image_dir() -> String {
    "test_images".to_string()
}

/// Request configuration for stress testing.
///
/// Defines how many requests to send and the delay between them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestConfig {
    /// Total number of requests to send
    pub total_requests: u64,
    /// Minimum delay between requests in milliseconds
    pub min_delay_ms: u64,
    /// Maximum delay between requests in milliseconds
    pub max_delay_ms: u64,
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
    /// Optional metrics collector for stress testing
    metrics: Option<Arc<Mutex<ClientMetrics>>>,
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
        Self {
            config,
            core,
            metrics: None,
        }
    }

    /// Sets the metrics collector for stress testing.
    ///
    /// # Arguments
    ///
    /// * `metrics` - Arc-wrapped mutex-protected metrics collector
    pub fn with_metrics(mut self, metrics: Arc<Mutex<ClientMetrics>>) -> Self {
        self.metrics = Some(metrics);
        self
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

        let total_requests = self.config.requests.total_requests;
        let min_delay = self.config.requests.min_delay_ms;
        let max_delay = self.config.requests.max_delay_ms;

        // Load all image files from the directory
        let image_files: Vec<String> = match fs::read_dir(&self.config.client.image_dir) {
            Ok(entries) => entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_file())
                .filter_map(|e| {
                    let path = e.path();
                    if let Some(ext) = path.extension() {
                        let ext = ext.to_str().unwrap_or("");
                        if ext == "jpg" || ext == "jpeg" || ext == "png" {
                            path.file_name()
                                .and_then(|n| n.to_str())
                                .map(|s| s.to_string())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect(),
            Err(e) => {
                error!(
                    "Failed to read image directory '{}': {}",
                    self.config.client.image_dir, e
                );
                return;
            }
        };

        if image_files.is_empty() {
            error!(
                "No images found in directory: {}",
                self.config.client.image_dir
            );
            return;
        }

        info!(
            "Client '{}' sending {} requests (delay: {}-{}ms, {} images available)...",
            self.config.client.name,
            total_requests,
            min_delay,
            max_delay,
            image_files.len()
        );

        // Send all requests with random delays and random image selection
        for i in 1..=total_requests {
            // Randomly select a secret image to hide
            let image_index = (rand::random::<f64>() * image_files.len() as f64) as usize;
            let image_name = &image_files[image_index % image_files.len()];

            // Read the image file
            let image_path = format!("{}/{}", self.config.client.image_dir, image_name);
            let secret_image_data = match std::fs::read(&image_path) {
                Ok(data) => data,
                Err(e) => {
                    error!("Failed to read image file '{}': {}", image_path, e);
                    continue;
                }
            };

            let result = self.send_request(i, secret_image_data).await;

            // Random delay between requests (only if task succeeded)
            if result.is_some() && i < total_requests {
                let range = max_delay - min_delay;
                let random_offset = (rand::random::<f64>() * range as f64) as u64;
                let delay = Duration::from_millis(min_delay + random_offset);
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
        const CONNECTION_TIMEOUT_SECS: u64 = 5;

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
        const CONNECTION_TIMEOUT_SECS: u64 = 5;

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
    /// 3. If no server responds for MAX_CONSECUTIVE_FAILURES attempts, assume task is lost
    ///    and return error to trigger resubmission
    ///
    /// # Arguments
    ///
    /// * `request_num` - Request ID to wait for
    /// * `failed_address` - Address of the server that just failed
    ///
    /// # Returns
    ///
    /// * `Ok((assigned_server_id, assigned_address))` - Server assignment
    /// * `Err` - Task appears to be lost (all servers failed or lost history)
    ///
    /// # Polling Behavior
    ///
    /// - Polls with 10-second intervals
    /// - Immediately accepts reassignment to a different server
    /// - Retries same server after MAX_SAME_SERVER_POLLS attempts (server might have recovered)
    /// - Gives up after MAX_CONSECUTIVE_FAILURES consecutive failures (triggers task resubmission)
    /// - Logs every polling attempt
    async fn wait_for_reassignment(
        &self,
        request_num: u64,
        failed_address: &str,
    ) -> Result<(u32, String)> {
        const POLL_INTERVAL_SECS: u64 = 2;
        const MAX_SAME_SERVER_POLLS: u32 = 10; // After 10 polls (100s), retry same server in case it recovered
        const MAX_CONSECUTIVE_FAILURES: u32 = 10; // After 6 consecutive failures (60s), assume task is lost

        info!(
            "⏳ {} Polling for task #{} assignment after {} failed (max {} consecutive failures before resubmission)...",
            self.config.client.name, request_num, failed_address, MAX_CONSECUTIVE_FAILURES
        );

        let mut attempt = 1;
        let mut same_server_count = 0;
        let mut consecutive_failures = 0;

        loop {
            info!(
                "🔄 {} Polling attempt {} for task #{}",
                self.config.client.name, attempt, request_num
            );

            match self.broadcast_status_query(request_num).await {
                Ok((server_id, address)) => {
                    // Reset consecutive failure counter - we got a response
                    consecutive_failures = 0;

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
                    consecutive_failures += 1;
                    warn!(
                        "Polling attempt {} failed for task #{}: {} ({}/{} consecutive failures)",
                        attempt, request_num, e, consecutive_failures, MAX_CONSECUTIVE_FAILURES
                    );

                    // If we've had too many consecutive failures, assume task is lost
                    if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                        error!(
                            "❌ {} Task #{} appears to be LOST - no server has record after {} consecutive failures. Task will be resubmitted.",
                            self.config.client.name, request_num, consecutive_failures
                        );
                        return Err(anyhow::anyhow!(
                            "Task #{} lost - all servers failed or lost task history after {} consecutive polling failures",
                            request_num, consecutive_failures
                        ));
                    }
                }
            }

            tokio::time::sleep(Duration::from_secs(POLL_INTERVAL_SECS)).await;
            attempt += 1;
        }
    }

    /// Sends a request with server-side failover handling and automatic resubmission.
    ///
    /// This method implements the complete workflow:
    /// 1. Polls indefinitely to get initial server assignment from leader (waits for leader if none available)
    /// 2. Executes task on assigned server
    /// 3. If server fails, polls for reassignment (up to 6 consecutive failures = 60s)
    /// 4. If task is lost (all servers failed/lost history), gets fresh assignment and resubmits
    /// 5. Retries complete workflow with MAX_RESUBMISSION_ATTEMPTS attempts
    ///
    /// # Arguments
    ///
    /// * `request_num` - Unique identifier for this request
    /// * `secret_image_data` - Binary data of the secret image to hide
    ///
    /// # Returns
    ///
    /// * `Some(Vec<u8>)` - If the request succeeded, returns the encrypted carrier image
    /// * `None` - If the request failed
    ///
    /// # Resubmission Strategy
    ///
    /// When task is lost (execute_task returns error after consecutive polling failures):
    /// - Get a fresh assignment from the current leader
    /// - Retry the entire task workflow
    /// - Maximum 3 complete resubmission attempts
    async fn send_request(
        &mut self,
        request_num: u64,
        secret_image_data: Vec<u8>,
    ) -> Option<Vec<u8>> {
        const POLL_INTERVAL_SECS: u64 = 2;
        const MAX_RESUBMISSION_ATTEMPTS: u32 = 3;

        // Start tracking latency
        let start_time = Instant::now();

        let mut resubmission_attempt = 0;

        loop {
            if resubmission_attempt > 0 {
                warn!(
                    "🔄 {} Task #{} resubmission attempt {}/{}",
                    self.config.client.name,
                    request_num,
                    resubmission_attempt,
                    MAX_RESUBMISSION_ATTEMPTS
                );
            }

            // Step 1: Get task assignment (poll indefinitely if no leader available)
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
                    secret_image_data.clone(),
                )
                .await;

            match result {
                Ok(encrypted_image_data) => {
                    // Calculate total latency
                    let latency = start_time.elapsed();

                    // Record metrics if enabled
                    if let Some(metrics) = &self.metrics {
                        let mut metrics = metrics.lock().unwrap();
                        metrics.record_request(
                            request_num,
                            latency,
                            true,
                            None,
                            Some(assigned_server_id),
                        );
                    }

                    info!(
                        "✅ {} Task #{} completed successfully{}",
                        self.config.client.name,
                        request_num,
                        if resubmission_attempt > 0 {
                            format!(" (after {} resubmission(s))", resubmission_attempt)
                        } else {
                            String::new()
                        }
                    );
                    Some(encrypted_image_data);
                }
                Err(e) => {
                    // Check if this is a task loss error (eligible for resubmission)
                    let error_msg = e.to_string();
                    let is_task_lost = error_msg.contains("lost")
                        || error_msg.contains("consecutive polling failures");

                    if is_task_lost && resubmission_attempt < MAX_RESUBMISSION_ATTEMPTS {
                        // Task was lost - try complete resubmission
                        resubmission_attempt += 1;
                        warn!(
                            "🔄 {} Task #{} lost - attempting resubmission ({}/{})",
                            self.config.client.name,
                            request_num,
                            resubmission_attempt,
                            MAX_RESUBMISSION_ATTEMPTS
                        );
                        // Continue to next iteration to get fresh assignment
                        continue;
                    } else {
                        // Either not a task loss error, or we've exhausted resubmission attempts
                        let latency = start_time.elapsed();

                        // Record metrics if enabled
                        if let Some(metrics) = &self.metrics {
                            let mut metrics = metrics.lock().unwrap();
                            metrics.record_request(
                                request_num,
                                latency,
                                false,
                                Some(error_msg.clone()),
                                Some(assigned_server_id),
                            );
                        }

                        error!(
                            "❌ {} Task #{} FAILED{}: {}",
                            self.config.client.name,
                            request_num,
                            if resubmission_attempt > 0 {
                                format!(" (after {} resubmission attempts)", resubmission_attempt)
                            } else {
                                String::new()
                            },
                            e
                        );
                        return None;
                    }
                }
            }
        }
    }

    /// Executes a task with automatic server-side failover handling.
    ///
    /// This method:
    /// 1. Uses the provided image data directly (already loaded)
    /// 2. Attempts to send task to assigned server
    /// 3. If server fails (TCP disconnect), polls for reassignment
    /// 4. Polling: up to MAX_CONSECUTIVE_FAILURES attempts, 10 seconds interval via broadcast to all servers
    /// 5. Retries with new server - if that server also fails, polls again
    /// 6. If all servers fail or lose task history, returns error to trigger complete resubmission
    ///
    /// # Arguments
    ///
    /// * `assigned_server_id` - ID of the initially assigned server
    /// * `assigned_address` - Network address of the initially assigned server
    /// * `leader_id` - ID of the leader that made the assignment
    /// * `request_num` - Unique identifier for this request
    /// * `secret_image_data` - Binary data of the secret image to hide
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<u8>)` - The encrypted carrier image with embedded secret
    /// * `Err(anyhow::Error)` - Only for non-connection errors (e.g., validation errors)
    /// * `Ok(())` - If the task completed successfully (possibly after multiple reassignments)
    /// * `Err(anyhow::Error)` - If task is lost (all servers failed/lost history) or other fatal errors
    ///
    /// # Server Failover
    ///
    /// When the assigned server fails:
    /// 1. Client detects TCP connection break
    /// 2. Client polls all servers (broadcast TaskStatusQuery every 10s)
    /// 3. If servers respond: Leader has detected failure and reassigned the task - retry with new server
    /// 4. If no servers respond after 6 consecutive failures: Task history is lost - return error for resubmission
    ///
    /// # File Locations
    ///
    /// - **Input**: `{image_dir}/{image_name}` (secret image to hide)
    /// - **Output**: Carrier image with embedded secret (returned by server)
    async fn execute_task(
        &self,
        _assigned_server_id: u32,
        mut assigned_address: String,
        mut leader_id: u32,
        request_num: u64,
        secret_image_data: Vec<u8>,
    ) -> Result<Vec<u8>> {
        loop {
            // Attempt to send task to assigned server
            let result = self
                .core
                .send_and_receive_encrypted_image(
                    &assigned_address,
                    request_num,
                    secret_image_data.clone(), // Clone cached data
                    leader_id,
                )
                .await;

            match result {
                Ok(encrypted_image_data) => {
                    return Ok(encrypted_image_data);
                }
                Err(e) => {
                    warn!(
                        "⚠️  {} Server failure detected for task #{} at {}: {}",
                        self.config.client.name, request_num, assigned_address, e
                    );

                    // Store the failed address
                    let failed_address = assigned_address.clone();

                    // Poll for reassignment until we get a valid assignment or determine task is lost
                    match self
                        .wait_for_reassignment(request_num, &failed_address)
                        .await
                    {
                        Ok((new_server_id, new_address)) => {
                            // Got a new assignment - retry with this server
                            info!(
                                "✅ {} Received assignment for task #{}: Server {} at {}",
                                self.config.client.name, request_num, new_server_id, new_address
                            );
                            assigned_address = new_address;
                            leader_id = new_server_id;
                            // Continue loop to retry with new server
                        }
                        Err(reassignment_error) => {
                            // Task appears to be lost - all servers failed or lost task history
                            // Return error to trigger complete resubmission from send_request
                            warn!(
                                "🔄 {} Task #{} lost during reassignment: {}",
                                self.config.client.name, request_num, reassignment_error
                            );
                            return Err(reassignment_error);
                        }
                    }
                }
            }
        }
    }

    /// Submits a task for web requests by calling send_request.
    ///
    /// This method wraps `send_request` to provide a simpler interface for web requests.
    ///
    /// # Arguments
    ///
    /// * `request_id` - Unique identifier for this request
    /// * `secret_image_data` - Binary data of the secret image to hide
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<u8>)` - The encrypted carrier image with embedded secret
    /// * `Err(anyhow::Error)` - If the task submission failed
    pub async fn submit_task(
        &mut self,
        request_id: u64,
        secret_image_data: Vec<u8>,
    ) -> anyhow::Result<Vec<u8>> {
        info!(
            "🌐 Web request #{}: Submitting image ({} bytes)",
            request_id,
            secret_image_data.len()
        );

        match self.send_request(request_id, secret_image_data).await {
            Some(encrypted_image_data) => Ok(encrypted_image_data),
            None => Err(anyhow::anyhow!("Task submission failed")),
        }
    }
}
