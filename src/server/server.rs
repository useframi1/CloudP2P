//! # Server Core - Image Encryption Service
//!
//! The core server component is responsible for ONE thing: performing steganography
//! encryption on images. It receives task requests and returns encrypted images.
//!
//! All distributed system concerns (leader election, heartbeats, task distribution, etc.)
//! are handled by the [`ServerMiddleware`](super::middleware::ServerMiddleware).

use anyhow::Result;
use log::info;

use crate::processing::steganography;

/// Core server component that performs image encryption tasks.
///
/// This struct is intentionally simple - it only knows how to encrypt images
/// using steganography. The middleware layer handles all coordination.
pub struct ServerCore {
    /// Server ID for logging purposes
    server_id: u32,
}

impl ServerCore {
    /// Create a new server core instance.
    ///
    /// # Arguments
    /// - `server_id`: Unique identifier for this server (used for logging)
    ///
    /// # Example
    /// ```ignore
    /// let core = ServerCore::new(1);
    /// ```
    pub fn new(server_id: u32) -> Self {
        Self { server_id }
    }

    /// Process an encryption task by embedding text into an image using LSB steganography.
    ///
    /// This function:
    /// 1. Receives image data and text to embed
    /// 2. Performs LSB steganography encryption
    /// 3. Saves the encrypted image to disk
    /// 4. Returns the encrypted image bytes
    ///
    /// # Arguments
    /// - `request_id`: Unique identifier for this task (for logging)
    /// - `client_name`: Name of the client that submitted this task (for logging)
    /// - `image_data`: Raw bytes of the input image
    /// - `image_name`: Original filename of the image
    /// - `text_to_embed`: Text to hide within the image
    ///
    /// # Returns
    /// - `Ok(Vec<u8>)`: Encrypted image bytes (PNG format)
    /// - `Err`: Encryption failed (image too small, invalid format, etc.)
    ///
    /// # Example
    /// ```ignore
    /// let image_data = std::fs::read("input.jpg")?;
    /// let encrypted = core.encrypt_image(
    ///     1,
    ///     "Client1".to_string(),
    ///     image_data,
    ///     "photo.jpg".to_string(),
    ///     "username:alice,views:5".to_string()
    /// ).await?;
    /// ```
    pub async fn encrypt_image(
        &self,
        request_id: u64,
        client_name: String,
        image_data: Vec<u8>,
        text_to_embed: String,
    ) -> Result<Vec<u8>> {
        info!(
            "📷 Server {} processing encryption request #{} from client '{}'",
            self.server_id, request_id, client_name
        );

        // Perform encryption in a blocking thread pool to avoid blocking async runtime
        // This is important because steganography is CPU-intensive
        let encryption_result = tokio::task::spawn_blocking(move || {
            steganography::embed_text_bytes(&image_data, &text_to_embed)
        })
        .await
        .map_err(|e| anyhow::anyhow!("Encryption task panicked: {}", e))??;

        info!(
            "✅ Server {} completed encryption for request #{}",
            self.server_id, request_id
        );

        Ok(encryption_result)
    }
}
