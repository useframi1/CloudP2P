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
        let delay_ms = (1000.0 / self.config.requests.rate_per_second) as u64; // Add .0 and cast
        let total_requests = (self.config.requests.rate_per_second
            * self.config.requests.duration_seconds as f64) as u64; // Cast properly

        // Send requests
        for i in 1..=total_requests {
            if let Some(leader_id) = self.current_leader {
                self.send_request(leader_id, i, "test_image.jpg".to_string(), "username:alice,views:5".to_string()).await;
            } else {
                warn!("⚠️  Lost connection to leader, trying to find new one...");
                if !self.discover_leader().await {
                    error!("❌ Could not reconnect to leader");
                    break;
                }
            }

            sleep(Duration::from_millis(delay_ms)).await;
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

    async fn send_request(&mut self, leader_id: u32, request_num: u64, image_name: String,
        text_to_embed: String) {
        // Find leader's address
        let leader_idx = (leader_id - 1) as usize;
        if leader_idx >= self.config.client.server_addresses.len() {
            return;
        }

        let address = &self.config.client.server_addresses[leader_idx];

        // READ IMAGE FILE INTO BYTES
        let image_path = format!("user-data/uploads/{}", image_name);
        let image_data = match std::fs::read(&image_path) {
            Ok(data) => data,
            Err(e) => {
                error!("❌ Failed to read image {}: {}", image_path, e);
                return;
            }
        };

        match TcpStream::connect(address).await {
            Ok(stream) => {
                let mut conn = Connection::new(stream);

                let request = Message::TaskRequest {
                    client_name: self.config.client.name.clone(),
                    request_id: request_num,
                    image_data: image_data,
                    image_name: image_name.clone(),
                    text_to_embed: text_to_embed.clone(),
                    load_impact: self.config.requests.load_per_request
                };

                if conn.write_message(&request).await.is_ok() {
                    info!(
                        "📤 {} Sent request #{} to Server {}",
                        self.config.client.name, request_num, leader_id
                    );

                    match conn.read_message().await {
                        Ok(Some(Message::TaskResponse {
                            request_id,
                            encrypted_image_data,
                            success,
                            error_message,
                        })) => {
                            if success {
                                // Save encrypted image
                                let output_path = format!(
                                    "user-data/outputs/encrypted_{}_{}",
                                    self.config.client.name, image_name.clone()
                                );
                                
                                match std::fs::write(&output_path, &encrypted_image_data) {
                                    Ok(_) => {
                                        info!(
                                            "✅ {} Received and saved encrypted image for request #{}",
                                            self.config.client.name, request_id
                                        );

                                        match crate::steganography::extract_text_bytes(&encrypted_image_data) {
                                            Ok(extracted_text) => {
                                                info!(
                                                    "🔍 {} VERIFICATION for request #{}:",
                                                    self.config.client.name, request_id
                                                );
                                                info!(
                                                    "   Expected: {:?}",
                                                    text_to_embed.clone()
                                                );
                                                info!(
                                                    "   Extracted: {:?}",
                                                    extracted_text
                                                );
                                                
                                                if extracted_text == text_to_embed.clone() {
                                                    info!(
                                                        "✅ {} Encryption VERIFIED successfully for request #{}",
                                                        self.config.client.name, request_id
                                                    );
                                                } else {
                                                    error!(
                                                        "❌ {} Encryption MISMATCH for request #{}!",
                                                        self.config.client.name, request_id
                                                    );
                                                }
                                            }
                                            Err(e) => {
                                                error!(
                                                    "❌ {} Failed to extract text from encrypted image: {}",
                                                    self.config.client.name, e
                                                );
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        error!(
                                            "❌ {} Failed to save encrypted image: {}",
                                            self.config.client.name, e
                                        );
                                    }
                                }
                            } else {
                                error!(
                                    "❌ {} Task #{} failed: {}",
                                    self.config.client.name,
                                    request_id,
                                    error_message.unwrap_or_else(|| "Unknown error".to_string())
                                );
                            }
                        }
                        Ok(Some(_)) => {
                            warn!("⚠️  {} Received unexpected message type", self.config.client.name);
                        }
                        Ok(None) => {
                            warn!(
                                "⚠️  {} Connection closed before receiving response for request #{}",
                                self.config.client.name, request_num
                            );
                        }
                        Err(e) => {
                            error!(
                                "❌ {} Failed to read response for request #{}: {}",
                                self.config.client.name, request_num, e
                            );
                        }
                    }

                } else {
                    warn!("⚠️  Failed to send request #{}", request_num);
                }
            }
            Err(_) => {
                warn!("⚠️  Could not connect to leader");
                self.current_leader = None;
            }
        }
    }
}
