//! # Client Core
//!
//! This module contains the minimal core client implementation that handles
//! the fundamental task of sending images to servers and receiving encrypted results.
//!
//! ## Responsibility
//!
//! The [`ClientCore`] struct focuses on a single, well-defined responsibility:
//! - Connect to an assigned server
//! - Send a task request with image data and text to embed
//! - Receive the encrypted image response
//! - Save the encrypted image locally
//! - Verify the encryption by extracting and comparing the embedded text
//!
//! ## Design Philosophy
//!
//! This core component is intentionally minimal and stateless. It does not handle:
//! - Leader discovery
//! - Server assignment logic
//! - Retry mechanisms
//! - Connection pooling
//! - Configuration management
//!
//! Those concerns are delegated to the [`ClientMiddleware`](super::middleware::ClientMiddleware).
//!
//! ## Usage
//!
//! ```rust,ignore
//! use cloudp2p::client::client::ClientCore;
//!
//! let core = ClientCore::new("Client1".to_string());
//!
//! // Called by middleware after obtaining server assignment
//! core.send_and_receive_encrypted_image(
//!     "127.0.0.1:5001",  // assigned server address
//!     request_id,
//!     image_data,
//!     "photo.jpg",
//!     "username:alice,views:5",
//!     leader_id
//! ).await?;
//! ```

use anyhow::Result;
use log::{error, info};
use tokio::net::TcpStream;

use crate::common::connection::Connection;
use crate::common::messages::Message;
use crate::processing::steganography;

/// The minimal core client that handles direct image transmission and encryption verification.
///
/// This struct represents a client identified by name that can send images to servers
/// and receive encrypted results. It performs no coordination logic - it simply executes
/// the core task when instructed by the middleware layer.
///
/// # Fields
///
/// * `client_name` - Unique identifier for this client, used in requests and logging
pub struct ClientCore {
    /// The unique name identifying this client
    client_name: String,
}

impl ClientCore {
    /// Creates a new `ClientCore` instance with the specified name.
    ///
    /// # Arguments
    ///
    /// * `client_name` - A unique identifier for this client
    ///
    /// # Returns
    ///
    /// A new `ClientCore` instance
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let core = ClientCore::new("Client1".to_string());
    /// ```
    pub fn new(client_name: String) -> Self {
        Self { client_name }
    }

    /// Sends an image to a server for encryption and receives the encrypted result.
    ///
    /// This method performs the complete image processing workflow:
    /// 1. Connects to the assigned server address
    /// 2. Sends a `TaskRequest` containing the image data and text to embed
    /// 3. Waits for and receives a `TaskResponse` with the encrypted image
    /// 4. Saves the encrypted image to the local filesystem
    /// 5. Verifies the encryption by extracting the embedded text
    ///
    /// # Arguments
    ///
    /// * `assigned_address` - Network address of the server (e.g., "127.0.0.1:5001")
    /// * `request_id` - Unique identifier for this request (used for tracking and logging)
    /// * `image_data` - Raw bytes of the image file to be encrypted
    /// * `image_name` - Name of the image file (used for output filename)
    /// * `text_to_embed` - Text to embed in the image using steganography
    /// * `assigned_by_leader` - Server ID of the leader that assigned this task
    ///
    /// # Returns
    ///
    /// * `Ok(())` - If the image was successfully sent, encrypted, received, saved, and verified
    /// * `Err(anyhow::Error)` - If any step fails (connection, transmission, encryption, or verification)
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// * Connection to the server fails
    /// * Message transmission fails
    /// * The server returns an error response
    /// * Writing the encrypted image to disk fails
    /// * The encrypted image verification fails
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let core = ClientCore::new("Client1".to_string());
    /// let image_data = std::fs::read("photo.jpg")?;
    ///
    /// core.send_and_receive_encrypted_image(
    ///     "127.0.0.1:5001",
    ///     42,
    ///     image_data,
    ///     "photo.jpg",
    ///     "metadata:secret",
    ///     1  // leader ID
    /// ).await?;
    /// ```
    pub async fn send_and_receive_encrypted_image(
        &self,
        assigned_address: &str,
        request_id: u64,
        image_data: Vec<u8>,
        image_name: &str,
        text_to_embed: &str,
        assigned_by_leader: u32,
    ) -> Result<()> {
        info!(
            "📤 {} Sending task #{} to server at {}",
            self.client_name, request_id, assigned_address
        );

        // Connect to the assigned server
        let stream = TcpStream::connect(assigned_address).await?;
        let mut conn = Connection::new(stream);

        // Construct and send the task request
        let task_request = Message::TaskRequest {
            client_name: self.client_name.clone(),
            request_id,
            image_data,
            image_name: image_name.to_string(),
            text_to_embed: text_to_embed.to_string(),
            assigned_by_leader,
        };

        conn.write_message(&task_request).await?;

        // Wait for and process the response
        match conn.read_message().await? {
            Some(Message::TaskResponse {
                request_id: response_id,
                encrypted_image_data,
                success,
                error_message,
            }) => {
                if success {
                    // Save the encrypted image to the outputs directory
                    let output_path = format!(
                        "user-data/outputs/encrypted_{}_{}",
                        self.client_name, image_name
                    );

                    std::fs::write(&output_path, &encrypted_image_data)?;

                    info!(
                        "✅ {} Saved encrypted image for task #{}",
                        self.client_name, response_id
                    );

                    // Verify the encryption by extracting the embedded text
                    match steganography::extract_text_bytes(&encrypted_image_data) {
                        Ok(extracted_text) => {
                            if extracted_text == text_to_embed {
                                info!(
                                    "✅ {} Encryption VERIFIED for task #{}",
                                    self.client_name, response_id
                                );
                            } else {
                                error!(
                                    "❌ {} Encryption MISMATCH for task #{}: expected '{}', got '{}'",
                                    self.client_name, response_id, text_to_embed, extracted_text
                                );
                                return Err(anyhow::anyhow!("Encryption verification failed: text mismatch"));
                            }
                        }
                        Err(e) => {
                            error!(
                                "❌ {} Failed to extract text from task #{}: {}",
                                self.client_name, response_id, e
                            );
                            return Err(anyhow::anyhow!("Failed to extract embedded text: {}", e));
                        }
                    }

                    // CRITICAL: Send acknowledgment to server that we received the response
                    // This allows the server to safely remove the task from history
                    let ack_message = Message::TaskAck {
                        client_name: self.client_name.clone(),
                        request_id: response_id,
                    };

                    if let Err(e) = conn.write_message(&ack_message).await {
                        error!(
                            "⚠️  {} Failed to send ACK for task #{}: {}",
                            self.client_name, response_id, e
                        );
                        // Don't fail the entire task if ACK fails - the task succeeded
                        // The server will retry later or detect orphaned task
                    } else {
                        info!(
                            "📨 {} Sent ACK for task #{}",
                            self.client_name, response_id
                        );
                    }

                    Ok(())
                } else {
                    // Server reported task failure
                    Err(anyhow::anyhow!(
                        "Task failed on server: {}",
                        error_message.unwrap_or_else(|| "Unknown error".to_string())
                    ))
                }
            }
            _ => Err(anyhow::anyhow!(
                "Unexpected response or connection closed"
            )),
        }
    }
}
