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
use tokio::time::sleep;

use crate::common::connection::Connection;
use crate::common::messages::Message;
use crate::client::client::ClientCore;

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
/// - Tracks current leader state
/// - Manages request lifecycle (discovery, assignment, execution, retry)
/// - Delegates actual image transmission to the core client
///
/// # Fields
///
/// * `config` - Client configuration loaded from TOML
/// * `core` - Shared reference to the core client for image transmission
/// * `current_leader` - ID of the currently known leader (None if unknown)
pub struct ClientMiddleware {
    /// Client configuration
    config: ClientConfig,
    /// Core client for image transmission (shared via Arc for potential future multi-threading)
    core: Arc<ClientCore>,
    /// Currently known leader ID (None if no leader is known)
    current_leader: Option<u32>,
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
            current_leader: None,
        }
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

    /// Discovers the current leader by querying all known servers.
    ///
    /// This method:
    /// 1. Iterates through all configured server addresses
    /// 2. Attempts to query each server with a 2-second timeout
    /// 3. Stops on the first successful response indicating a leader
    /// 4. Updates `current_leader` with the discovered leader ID
    ///
    /// # Returns
    ///
    /// * `true` - If a leader was successfully discovered
    /// * `false` - If no server responded with a valid leader within the timeout
    ///
    /// # Timeout
    ///
    /// Each server query has a 2-second timeout. If a server doesn't respond
    /// within this time, the next server is tried.
    async fn discover_leader(&mut self) -> bool {
        info!("Looking for the current leader...");

        const CONNECTION_TIMEOUT_SECS: u64 = 2;

        // Try each server with a timeout
        for address in &self.config.client.server_addresses {
            // Wrap entire leader query in a timeout
            let result = tokio::time::timeout(
                Duration::from_secs(CONNECTION_TIMEOUT_SECS),
                self.query_leader(address),
            )
            .await;

            match result {
                Ok(Some(leader_id)) => {
                    self.current_leader = Some(leader_id);
                    info!("Found leader: Server {}", leader_id);
                    return true;
                }
                Ok(None) => {
                    // Server responded but not ready or invalid response
                    continue;
                }
                Err(_) => {
                    // Timeout - server not responding, try next one
                    continue;
                }
            }
        }

        false
    }

    /// Queries a specific server to determine the current leader.
    ///
    /// This helper method:
    /// 1. Connects to the specified server address
    /// 2. Sends a `LeaderQuery` message
    /// 3. Waits for a `LeaderResponse` message
    /// 4. Extracts and returns the leader ID
    ///
    /// # Arguments
    ///
    /// * `address` - Server address to query (e.g., "127.0.0.1:5001")
    ///
    /// # Returns
    ///
    /// * `Some(leader_id)` - If the server responded with a valid leader ID
    /// * `None` - If connection failed or response was invalid
    async fn query_leader(&self, address: &str) -> Option<u32> {
        // Try to connect
        let stream = TcpStream::connect(address).await.ok()?;
        let mut conn = Connection::new(stream);

        // Ask: who is the leader?
        let query = Message::LeaderQuery;
        conn.write_message(&query).await.ok()?;

        // Wait for response
        match conn.read_message().await.ok()? {
            Some(Message::LeaderResponse { leader_id }) => Some(leader_id),
            _ => None,
        }
    }

    /// Sends a request with retry logic and fault tolerance.
    ///
    /// This method implements the complete retry workflow:
    /// 1. Attempts the request up to 3 times
    /// 2. Each attempt has a 10-second timeout
    /// 3. Waits 5 seconds between retry attempts
    /// 4. Each attempt includes leader discovery and task processing
    ///
    /// # Arguments
    ///
    /// * `request_num` - Unique identifier for this request
    /// * `image_name` - Name of the image file to process (relative to uploads directory)
    /// * `text_to_embed` - Text to embed in the image using steganography
    ///
    /// # Returns
    ///
    /// * `true` - If the request succeeded within the retry attempts
    /// * `false` - If all retry attempts failed
    ///
    /// # Retry Parameters
    ///
    /// - **Max retries**: 3 attempts
    /// - **Timeout**: 10 seconds per attempt
    /// - **Retry delay**: 5 seconds between attempts
    async fn send_request(
        &mut self,
        request_num: u64,
        image_name: String,
        text_to_embed: String,
    ) -> bool {
        const MAX_RETRIES: u32 = 3;
        const TIMEOUT_SECS: u64 = 10;
        const RETRY_INTERVAL_SECS: u64 = 5;

        for attempt in 1..=MAX_RETRIES {
            if attempt > 1 {
                info!(
                    "🔄 {} Retry attempt {}/{} for task #{}",
                    self.config.client.name, attempt, MAX_RETRIES, request_num
                );
                // Wait 5 seconds between retries
                sleep(Duration::from_secs(RETRY_INTERVAL_SECS)).await;
            }

            // Step 1: Find the leader
            info!(
                "Finding leader for task #{} (attempt {}/{})",
                request_num, attempt, MAX_RETRIES
            );

            if !self.discover_leader().await {
                warn!(
                    "No leader found for task #{} on attempt {}/{}",
                    request_num, attempt, MAX_RETRIES
                );
                continue;
            }

            let leader_id = match self.current_leader {
                Some(id) => id,
                None => {
                    warn!(
                        "No leader available for task #{} on attempt {}/{}",
                        request_num, attempt, MAX_RETRIES
                    );
                    continue;
                }
            };

            info!(
                "✅ {} Found leader: Server {} for task #{}",
                self.config.client.name, leader_id, request_num
            );

            // Step 2-4: Get assignment, send request, wait for response (with timeout)
            let result = tokio::time::timeout(
                Duration::from_secs(TIMEOUT_SECS),
                self.process_request(
                    leader_id,
                    request_num,
                    image_name.clone(),
                    text_to_embed.clone(),
                ),
            )
            .await;

            match result {
                Ok(Ok(())) => {
                    info!(
                        "✅ {} Task #{} completed successfully",
                        self.config.client.name, request_num
                    );
                    return true;
                }
                Ok(Err(e)) => {
                    warn!(
                        "Task #{} failed on attempt {}/{}: {}",
                        request_num, attempt, MAX_RETRIES, e
                    );
                }
                Err(_) => {
                    warn!(
                        "Task #{} timed out after {}s on attempt {}/{}",
                        request_num, TIMEOUT_SECS, attempt, MAX_RETRIES
                    );
                }
            }
        }

        error!(
            "❌ {} Task #{} FAILED after {} attempts",
            self.config.client.name, request_num, MAX_RETRIES
        );
        false
    }

    /// Processes a single request by coordinating assignment and execution.
    ///
    /// This orchestrator method:
    /// 1. Requests a server assignment from the leader
    /// 2. Delegates task execution to `execute_task()`
    ///
    /// # Arguments
    ///
    /// * `leader_id` - ID of the current leader server
    /// * `request_num` - Unique identifier for this request
    /// * `image_name` - Name of the image file to process
    /// * `text_to_embed` - Text to embed in the image
    ///
    /// # Returns
    ///
    /// * `Ok(())` - If the request was successfully processed
    /// * `Err(anyhow::Error)` - If assignment or execution failed
    async fn process_request(
        &mut self,
        leader_id: u32,
        request_num: u64,
        image_name: String,
        text_to_embed: String,
    ) -> Result<()> {
        // Step 2: Get assigned server ID from leader
        let (assigned_server_id, assigned_address) =
            self.get_server_assignment(leader_id, request_num).await?;

        info!(
            "✅ {} Task #{} assigned to Server {} at {}",
            self.config.client.name, request_num, assigned_server_id, assigned_address
        );

        // Step 3: Execute the task on the assigned server
        self.execute_task(
            assigned_server_id,
            assigned_address,
            leader_id,
            request_num,
            image_name,
            text_to_embed,
        )
        .await
    }

    /// Requests a server assignment from the leader.
    ///
    /// This method:
    /// 1. Connects to the leader server
    /// 2. Sends a `TaskAssignmentRequest` with client info and request ID
    /// 3. Waits for a `TaskAssignmentResponse` with the assigned server details
    /// 4. Returns the assigned server ID and address
    ///
    /// # Arguments
    ///
    /// * `leader_id` - ID of the current leader server
    /// * `request_num` - Unique identifier for this request
    ///
    /// # Returns
    ///
    /// * `Ok((server_id, address))` - Assigned server ID and network address
    /// * `Err(anyhow::Error)` - If connection, request, or response failed
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The leader ID is invalid (out of bounds)
    /// - Connection to the leader fails
    /// - The leader doesn't respond with a valid assignment
    async fn get_server_assignment(
        &self,
        leader_id: u32,
        request_num: u64,
    ) -> Result<(u32, String)> {
        let leader_idx = (leader_id - 1) as usize;
        if leader_idx >= self.config.client.server_addresses.len() {
            return Err(anyhow::anyhow!("Invalid leader index"));
        }
        let leader_address = self.config.client.server_addresses[leader_idx].clone();

        info!(
            "{} Requesting assignment for task #{} from leader {}",
            self.config.client.name, request_num, leader_id
        );

        // Connect to leader
        let stream = TcpStream::connect(&leader_address).await?;
        let mut conn = Connection::new(stream);

        // Ask for server assignment
        let assignment_request = Message::TaskAssignmentRequest {
            client_name: self.config.client.name.clone(),
            request_id: request_num,
        };

        conn.write_message(&assignment_request).await?;

        // Get assignment response
        match conn.read_message().await? {
            Some(Message::TaskAssignmentResponse {
                request_id: _,
                assigned_server_id,
                assigned_server_address,
            }) => Ok((assigned_server_id, assigned_server_address)),
            _ => Err(anyhow::anyhow!("Failed to receive assignment response")),
        }
    }

    /// Executes a task by reading the image file and delegating to the core client.
    ///
    /// This method:
    /// 1. Reads the image file from the uploads directory
    /// 2. Calls the core client's `send_and_receive_encrypted_image()` method
    /// 3. The core handles connection, transmission, response, and verification
    ///
    /// # Arguments
    ///
    /// * `assigned_server_id` - ID of the server assigned to process this task
    /// * `assigned_address` - Network address of the assigned server
    /// * `leader_id` - ID of the leader that made the assignment
    /// * `request_num` - Unique identifier for this request
    /// * `image_name` - Name of the image file (relative to uploads directory)
    /// * `text_to_embed` - Text to embed in the image
    ///
    /// # Returns
    ///
    /// * `Ok(())` - If the task completed successfully (sent, encrypted, verified)
    /// * `Err(anyhow::Error)` - If file reading or core execution failed
    ///
    /// # File Locations
    ///
    /// - **Input**: `user-data/uploads/{image_name}`
    /// - **Output**: Handled by core client at `user-data/outputs/encrypted_{client_name}_{image_name}`
    async fn execute_task(
        &self,
        _assigned_server_id: u32,
        assigned_address: String,
        leader_id: u32,
        request_num: u64,
        image_name: String,
        text_to_embed: String,
    ) -> Result<()> {
        // Read the image file from the uploads directory
        let image_path = format!("user-data/uploads/{}", image_name);
        let image_data = std::fs::read(&image_path)?;

        // Delegate to the core client to handle the actual transmission and verification
        self.core
            .send_and_receive_encrypted_image(
                &assigned_address,
                request_num,
                image_data,
                &image_name,
                &text_to_embed,
                leader_id,
            )
            .await
    }
}
