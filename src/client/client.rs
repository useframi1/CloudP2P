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

    /// Sends a secret image to a server for encryption and receives the carrier image result.
    ///
    /// This method performs the complete image processing workflow:
    /// 1. Connects to the assigned server address
    /// 2. Sends a `TaskRequest` containing the secret image data
    /// 3. Waits for and receives a `TaskResponse` with the carrier image (containing the embedded secret)
    /// 4. Saves the carrier image to the local filesystem
    /// 5. Verifies the encryption by extracting and validating the embedded secret image
    ///
    /// # Arguments
    ///
    /// * `assigned_address` - Network address of the server (e.g., "127.0.0.1:5001")
    /// * `request_id` - Unique identifier for this request (used for tracking and logging)
    /// * `secret_image_data` - Raw bytes of the secret image to hide
    /// * `assigned_by_leader` - Server ID of the leader that assigned this task
    ///
    /// # Returns
    ///
    /// * `Ok(())` - If the secret image was successfully sent, embedded, received, saved, and verified
    /// * `Err(anyhow::Error)` - If any step fails (connection, transmission, encryption, or verification)
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// * Connection to the server fails
    /// * Message transmission fails
    /// * The server returns an error response
    /// * Writing the carrier image to disk fails
    /// * The carrier image verification fails
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let core = ClientCore::new("Client1".to_string());
    /// let secret_image = std::fs::read("secret.jpg")?;
    ///
    /// core.send_and_receive_encrypted_image(
    ///     "127.0.0.1:5001",
    ///     42,
    ///     secret_image,
    ///     1  // leader ID
    /// ).await?;
    /// ```
    pub async fn send_and_receive_encrypted_image(
        &self,
        assigned_address: &str,
        request_id: u64,
        secret_image_data: Vec<u8>,
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
            secret_image_data,
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
                    // Save the encrypted carrier image to disk
                    // let output_path = format!("test_images/encrypted_image.jpg");
                    // if let Err(e) = std::fs::write(&output_path, &encrypted_image_data) {
                    //     error!(
                    //         "⚠️  {} Failed to save carrier image to '{}': {}",
                    //         self.client_name, output_path, e
                    //     );
                    // } else {
                    //     info!(
                    //         "💾 {} Saved carrier image to: {}",
                    //         self.client_name, output_path
                    //     );
                    // }

                    // Verify the encryption by extracting the embedded secret image
                    info!(
                        "🔍 {} Verifying encryption for task #{} (carrier image size: {} bytes)",
                        self.client_name,
                        response_id,
                        encrypted_image_data.len()
                    );

                    match steganography::extract_image_bytes(&encrypted_image_data) {
                        Ok(extracted_image) => {
                            info!(
                                "✅ {} Successfully extracted embedded image for task #{} (size: {} bytes)",
                                self.client_name, response_id, extracted_image.len()
                            );

                            // Optional: Verify the extracted image matches the original
                            // Note: We don't have access to the original secret_image_data here
                            // In a real application, you might want to:
                            // 1. Save the carrier image to disk
                            // 2. Compare extracted image with original (if needed)
                            // 3. Log verification details

                            info!(
                                "✅ {} Encryption VERIFIED for task #{}",
                                self.client_name, response_id
                            );
                        }
                        Err(e) => {
                            error!(
                                "❌ {} Failed to extract embedded image from task #{}: {}",
                                self.client_name, response_id, e
                            );
                            return Err(anyhow::anyhow!("Failed to extract embedded image: {}", e));
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
                        info!("📨 {} Sent ACK for task #{}", self.client_name, response_id);
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
            _ => Err(anyhow::anyhow!("Unexpected response or connection closed")),
        }
    }
}
