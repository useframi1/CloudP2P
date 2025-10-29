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

        // Find the leader
        if !self.discover_leader().await {
            error!("❌ Could not find a leader. Make sure servers are running!");
            return;
        }

        info!(
            "⏳ Client '{}' sending requests for {} seconds...",
            self.config.client.name, self.config.requests.duration_seconds
        );

        // Calculate delay between requests
        let delay = Duration::from_millis((1000.0 / self.config.requests.rate_per_second) as u64);
        let total_requests = (self.config.requests.rate_per_second
            * self.config.requests.duration_seconds as f64) as u64; // Cast properly

        // Send requests
        for i in 1..=total_requests {
            // Discover leader fresh for each request
            if !self.discover_leader().await {
                warn!(
                    "⚠️  Could not find a leader for task #{}, waiting 5s before retry...",
                    i
                );
                sleep(Duration::from_secs(5)).await;
                continue; // Try this same task again after delay
            }

            // Send request with freshly discovered leader
            if let Some(leader_id) = self.current_leader {
                let success = self
                    .send_request(
                        leader_id,
                        i,
                        "test_image.jpg".to_string(),
                        "username:alice,views:5".to_string(),
                    )
                    .await;

                // Only sleep if task succeeded
                if success {
                    tokio::time::sleep(delay).await;
                }
                // If failed, skip sleep to try next task immediately with fresh leader discovery
            }
        }

        info!("✅ Client finished sending {} requests", total_requests);
    }

    async fn discover_leader(&mut self) -> bool {
        info!("🔍 Looking for the current leader...");

        // Try each server
        for address in &self.config.client.server_addresses {
            match TcpStream::connect(address).await {
                Ok(stream) => {
                    let mut conn = Connection::new(stream);

                    // Ask: who is the leader?
                    let query = Message::LeaderQuery;
                    if conn.write_message(&query).await.is_ok() {
                        // Wait for response
                        if let Ok(Some(Message::LeaderResponse { leader_id })) =
                            conn.read_message().await
                        {
                            self.current_leader = Some(leader_id);
                            info!("🤝 Found leader: Server {}", leader_id);
                            return true;
                        }
                    }
                }
                Err(_) => continue,
            }
        }

        false
    }

    async fn send_request(
        &mut self,
        mut leader_id: u32,
        request_num: u64,
        image_name: String,
        text_to_embed: String,
    ) -> bool {
        const MAX_RETRIES: u32 = 3;
        const TIMEOUT_SECS: u64 = 10;

        for attempt in 1..=MAX_RETRIES {
            if attempt > 1 {
                info!(
                    "🔄 {} Retry attempt {}/{} for task #{}",
                    self.config.client.name, attempt, MAX_RETRIES, request_num
                );
            }

            // Rediscover leader before EVERY attempt (including first) to ensure freshness
            info!(
                "🔍 {} Discovering leader for attempt {}/{} of task #{}",
                self.config.client.name, attempt, MAX_RETRIES, request_num
            );
            if self.discover_leader().await {
                if let Some(new_leader) = self.current_leader {
                    if new_leader != leader_id {
                        info!(
                            "📍 {} Switched from leader {} to leader {}",
                            self.config.client.name, leader_id, new_leader
                        );
                        leader_id = new_leader; // Update to new leader
                    }
                }
            } else {
                warn!(
                    "⚠️  No leader found for attempt {}/{}, waiting 5s for election...",
                    attempt, MAX_RETRIES
                );
                sleep(Duration::from_secs(5)).await;
                continue; // Try again
            }

            // Small delay before actual request (except first attempt)
            if attempt > 1 {
                sleep(Duration::from_millis(500)).await;
            }

            let result = tokio::time::timeout(
                Duration::from_secs(TIMEOUT_SECS),
                self.try_send_request(
                    leader_id,
                    request_num,
                    image_name.clone(),
                    text_to_embed.clone(),
                ),
            )
            .await;

            match result {
                Ok(Ok(())) => {
                    // Success!
                    return true;
                }
                Ok(Err(e)) => {
                    warn!(
                        "⚠️  {} Task #{} failed on attempt {}: {}",
                        self.config.client.name, request_num, attempt, e
                    );
                }
                Err(_) => {
                    warn!(
                        "⏱️  {} Task #{} timed out after {}s on attempt {}",
                        self.config.client.name, request_num, TIMEOUT_SECS, attempt
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

    async fn try_send_request(
        &mut self,
        leader_id: u32,
        request_num: u64,
        image_name: String,
        text_to_embed: String,
    ) -> Result<()> {
        // PHASE 1: ASK LEADER FOR ASSIGNMENT

        let leader_idx = (leader_id - 1) as usize;
        if leader_idx >= self.config.client.server_addresses.len() {
            return Err(anyhow::anyhow!("Invalid leader index"));
        }
        let leader_address = self.config.client.server_addresses[leader_idx].clone();

        info!(
            "📋 {} Requesting assignment for task #{} from leader {}",
            self.config.client.name, request_num, leader_id
        );

        // Create lightweight request
        let assignment_request = Message::TaskAssignmentRequest {
            client_name: self.config.client.name.clone(),
            request_id: request_num,
        };

        // Connect to leader and ask for assignment
        let stream = TcpStream::connect(&leader_address).await?;
        let mut conn = Connection::new(stream);

        // Send assignment request
        conn.write_message(&assignment_request).await?;

        // Wait for leader's response
        let (assigned_server_id, assigned_address) = match conn.read_message().await? {
            Some(Message::TaskAssignmentResponse {
                request_id: _,
                assigned_server_id,
                assigned_server_address,
            }) => {
                info!(
                    "✅ {} Task #{} assigned to Server {} at {}",
                    self.config.client.name,
                    request_num,
                    assigned_server_id,
                    assigned_server_address
                );
                (assigned_server_id, assigned_server_address)
            }
            _ => {
                return Err(anyhow::anyhow!("Failed to receive assignment response"));
            }
        };

        // PHASE 2: READ THE IMAGE FILE
        let image_path = format!("user-data/uploads/{}", image_name);
        let image_data = std::fs::read(&image_path)?;

        // PHASE 3: SEND IMAGE DIRECTLY TO ASSIGNED SERVER
        info!(
            "📤 {} Sending image for task #{} directly to Server {}",
            self.config.client.name, request_num, assigned_server_id
        );

        let stream = TcpStream::connect(&assigned_address).await?;
        let mut conn = Connection::new(stream);

        // Now send the actual image data
        let task_request = Message::TaskRequest {
            client_name: self.config.client.name.clone(),
            request_id: request_num,
            image_data,
            image_name: image_name.clone(),
            text_to_embed: text_to_embed.clone(),
            assigned_by_leader: leader_id,
        };

        conn.write_message(&task_request).await?;

        // Wait for encrypted image response
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
