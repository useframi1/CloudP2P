//! # Server Core - Image Encryption Service
//!
//! The core server component is responsible for ONE thing: performing steganography
//! encryption on images. It receives task requests and returns encrypted images.
//!
//! All distributed system concerns (leader election, heartbeats, task distribution, etc.)
//! are handled by the [`ServerMiddleware`](super::middleware::ServerMiddleware).

use anyhow::Result;
use log::info;
use std::sync::Arc;

use crate::processing::steganography;

/// Core server component that performs image encryption tasks.
///
/// This struct is intentionally simple - it only knows how to encrypt images
/// using steganography. The middleware layer handles all coordination.
pub struct ServerCore {
    /// Server ID for logging purposes
    server_id: u32,
    /// Default carrier image used to hide secret images
    default_carrier_image: Arc<Vec<u8>>,
}

impl ServerCore {
    /// Create a new server core instance by loading a cover image from a file path.
    ///
    /// This function:
    /// 1. Reads the cover image file from the specified path
    /// 2. Validates it's a valid image format
    /// 3. Logs the image dimensions and capacity
    /// 4. Creates a ServerCore with the loaded cover image
    ///
    /// # Arguments
    /// - `server_id`: Unique identifier for this server (used for logging)
    /// - `cover_image_path`: Path to the cover/carrier image file
    ///
    /// # Returns
    /// - `Ok(ServerCore)`: Successfully created with loaded cover image
    /// - `Err`: If file doesn't exist, can't be read, or isn't a valid image
    ///
    /// # Example
    /// ```ignore
    /// let core = ServerCore::new(1, "test_images/medium.jpg")?;
    /// ```
    pub fn new(server_id: u32, cover_image_path: &str) -> Result<Self> {
        use image::GenericImageView;

        info!("📂 Server {} loading cover image from: {}", server_id, cover_image_path);

        // Read the cover image file
        let carrier_image_bytes = std::fs::read(cover_image_path)
            .map_err(|e| anyhow::anyhow!(
                "Failed to read cover image '{}': {}", cover_image_path, e
            ))?;

        // Validate it's a valid image and get dimensions
        let img = image::load_from_memory(&carrier_image_bytes)
            .map_err(|e| anyhow::anyhow!(
                "Invalid cover image format '{}': {}", cover_image_path, e
            ))?;

        let (width, height) = img.dimensions();
        let capacity = (width * height * 3) / 8;

        info!(
            "✅ Server {} loaded cover image: {}x{} pixels ({} KB capacity)",
            server_id, width, height, capacity / 1024
        );

        Ok(Self {
            server_id,
            default_carrier_image: Arc::new(carrier_image_bytes),
        })
    }

    /// Legacy constructor: Create a server core with pre-loaded image bytes.
    ///
    /// This is kept for backward compatibility.
    #[allow(dead_code)]
    pub fn from_bytes(server_id: u32, carrier_image_bytes: Vec<u8>) -> Self {
        Self {
            server_id,
            default_carrier_image: Arc::new(carrier_image_bytes),
        }
    }

    /// Process an encryption task by embedding a secret image into the server's carrier image.
    ///
    /// This function:
    /// 1. Receives a secret image from the client
    /// 2. Embeds it into the server's default carrier image using LSB steganography
    /// 3. Returns the carrier image with the embedded secret
    ///
    /// # Arguments
    /// - `request_id`: Unique identifier for this task (for logging)
    /// - `client_name`: Name of the client that submitted this task (for logging)
    /// - `secret_image_data`: Raw bytes of the secret image to hide
    ///
    /// # Returns
    /// - `Ok(Vec<u8>)`: Carrier image bytes with embedded secret (PNG format)
    /// - `Err`: Encryption failed (carrier too small, invalid format, etc.)
    ///
    /// # Example
    /// ```ignore
    /// let secret_image = std::fs::read("secret.jpg")?;
    /// let result = core.encrypt_image(
    ///     1,
    ///     "Client1".to_string(),
    ///     secret_image,
    /// ).await?;
    /// ```
    pub async fn encrypt_image(
        &self,
        request_id: u64,
        client_name: String,
        secret_image_data: Vec<u8>,
    ) -> Result<Vec<u8>> {
        info!(
            "📷 Server {} processing encryption request #{} from client '{}' (secret image size: {} bytes)",
            self.server_id, request_id, client_name, secret_image_data.len()
        );

        // Clone the carrier image for this task
        let carrier_image = self.default_carrier_image.clone();

        // Perform encryption in a blocking thread pool to avoid blocking async runtime
        // This is important because steganography is CPU-intensive
        let encryption_result = tokio::task::spawn_blocking(move || {
            steganography::embed_image_bytes(&carrier_image, &secret_image_data)
        })
        .await
        .map_err(|e| anyhow::anyhow!("Encryption task panicked: {}", e))??;

        info!(
            "✅ Server {} completed encryption for request #{} (result size: {} bytes)",
            self.server_id, request_id, encryption_result.len()
        );

        Ok(encryption_result)
    }

    /// Legacy function: Process an encryption task by embedding text into an image.
    ///
    /// This is kept for backward compatibility with the existing text-based workflow.
    #[allow(dead_code)]
    pub async fn encrypt_image_with_text(
        &self,
        request_id: u64,
        client_name: String,
        image_data: Vec<u8>,
        text_to_embed: String,
    ) -> Result<Vec<u8>> {
        info!(
            "📷 Server {} processing text encryption request #{} from client '{}'",
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
            "✅ Server {} completed text encryption for request #{}",
            self.server_id, request_id
        );

        Ok(encryption_result)
    }
}
