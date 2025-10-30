use anyhow::Result;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use std::fs;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::sleep;

use crate::messages::Message;
use crate::server::Connection;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    pub client: ClientInfo,
    pub requests: RequestConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub name: String,
    pub server_addresses: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestConfig {
    pub rate_per_second: f64,
    pub duration_seconds: f64,
    pub request_processing_ms: u64,
    pub load_per_request: f64,
}

impl ClientConfig {
    pub fn from_file(path: &str) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: ClientConfig = toml::from_str(&content)?;
        Ok(config)
    }
}

pub struct Client {
    config: ClientConfig,
    current_leader: Option<u32>,
}

impl Client {
    pub fn new(config: ClientConfig) -> Self {
        Self {
            config,
            current_leader: None,
        }
    }

    pub async fn run(&mut self) {
        info!("🔵 Client '{}' starting", self.config.client.name);

        info!(
            "⏳ Client '{}' sending requests for {} seconds...",
            self.config.client.name, self.config.requests.duration_seconds
        );

        // Calculate delay between requests
        let delay = Duration::from_millis((1000.0 / self.config.requests.rate_per_second) as u64);
        let total_requests = (self.config.requests.rate_per_second
            * self.config.requests.duration_seconds as f64) as u64;

        // Send requests
        for i in 1..=total_requests {
            let success = self
                .send_request(
                    i,
                    "test_image.jpg".to_string(),
                    "username:alice,views:5".to_string(),
                )
                .await;

            // Only sleep if task succeeded
            if success {
                tokio::time::sleep(delay).await;
            }
            // If failed, no sleep - immediately try next task
        }

        info!("✅ Client finished sending {} requests", total_requests);
    }

    async fn discover_leader(&mut self) -> bool {
        info!("🔍 Looking for the current leader...");

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
                    info!("🤝 Found leader: Server {}", leader_id);
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
                "🔍 {} Finding leader for task #{} (attempt {}/{})",
                self.config.client.name, request_num, attempt, MAX_RETRIES
            );

            if !self.discover_leader().await {
                warn!(
                    "⚠️  {} No leader found for task #{} on attempt {}/{}",
                    self.config.client.name, request_num, attempt, MAX_RETRIES
                );
                continue;
            }

            let leader_id = match self.current_leader {
                Some(id) => id,
                None => {
                    warn!(
                        "⚠️  {} No leader available for task #{} on attempt {}/{}",
                        self.config.client.name, request_num, attempt, MAX_RETRIES
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
                        "⚠️  {} Task #{} failed on attempt {}/{}: {}",
                        self.config.client.name, request_num, attempt, MAX_RETRIES, e
                    );
                }
                Err(_) => {
                    warn!(
                        "⏱️  {} Task #{} timed out after {}s on attempt {}/{}",
                        self.config.client.name, request_num, TIMEOUT_SECS, attempt, MAX_RETRIES
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

        // Step 3: Make request to assigned server and wait for response
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
            "📋 {} Requesting assignment for task #{} from leader {}",
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

    async fn execute_task(
        &self,
        assigned_server_id: u32,
        assigned_address: String,
        leader_id: u32,
        request_num: u64,
        image_name: String,
        text_to_embed: String,
    ) -> Result<()> {
        // Read the image file
        let image_path = format!("user-data/uploads/{}", image_name);
        let image_data = std::fs::read(&image_path)?;

        info!(
            "📤 {} Sending task #{} to Server {}",
            self.config.client.name, request_num, assigned_server_id
        );

        // Connect to assigned server
        let stream = TcpStream::connect(&assigned_address).await?;
        let mut conn = Connection::new(stream);

        // Send task request
        let task_request = Message::TaskRequest {
            client_name: self.config.client.name.clone(),
            request_id: request_num,
            image_data,
            image_name: image_name.clone(),
            text_to_embed: text_to_embed.clone(),
            assigned_by_leader: leader_id,
        };

        conn.write_message(&task_request).await?;

        // Wait for response
        match conn.read_message().await? {
            Some(Message::TaskResponse {
                request_id,
                encrypted_image_data,
                success,
                error_message,
            }) => {
                if success {
                    // Save the encrypted image
                    let output_path = format!(
                        "user-data/outputs/encrypted_{}_{}",
                        self.config.client.name, image_name
                    );

                    std::fs::write(&output_path, &encrypted_image_data)?;

                    info!(
                        "✅ {} Saved encrypted image for task #{}",
                        self.config.client.name, request_id
                    );

                    // Verify the encryption worked
                    match crate::steganography::extract_text_bytes(&encrypted_image_data) {
                        Ok(extracted_text) => {
                            if extracted_text == text_to_embed {
                                info!(
                                    "✅ {} Encryption VERIFIED for task #{}",
                                    self.config.client.name, request_id
                                );
                            } else {
                                error!(
                                    "❌ {} Encryption MISMATCH for task #{}",
                                    self.config.client.name, request_id
                                );
                            }
                        }
                        Err(e) => {
                            error!("❌ Failed to extract text: {}", e);
                        }
                    }

                    Ok(())
                } else {
                    Err(anyhow::anyhow!(
                        "Task failed: {}",
                        error_message.unwrap_or_else(|| "Unknown error".to_string())
                    ))
                }
            }
            _ => Err(anyhow::anyhow!("Unexpected response or connection closed")),
        }
    }
}
