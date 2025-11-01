//! # TCP Connection Abstraction
//!
//! Provides a wrapper around TCP streams with message framing for the CloudP2P protocol.
//!
//! ## Wire Protocol
//!
//! Messages are sent with a 4-byte length prefix (big-endian) followed by JSON data:
//! ```text
//! [4 bytes: message length] [N bytes: JSON message data]
//! ```
//!
//! This length-prefixed protocol allows for:
//! - Variable-length messages (images can be large)
//! - Reliable message boundaries over TCP streams
//! - Protection against incomplete reads

use anyhow::Result;
use log::error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use super::messages::Message;

/// Maximum allowed message size (100MB) to prevent memory exhaustion attacks.
const MAX_MESSAGE_SIZE: usize = 100 * 1024 * 1024;

/// TCP connection wrapper with message framing support.
///
/// Handles serialization, deserialization, and length-prefixed framing of messages
/// over a TCP stream.
pub struct Connection {
    /// Underlying TCP stream
    stream: TcpStream,
}

impl Connection {
    /// Create a new Connection from an existing TCP stream.
    ///
    /// # Arguments
    /// - `stream`: An established TCP connection
    ///
    /// # Example
    /// ```ignore
    /// let stream = TcpStream::connect("127.0.0.1:8001").await?;
    /// let mut conn = Connection::new(stream);
    /// ```
    pub fn new(stream: TcpStream) -> Self {
        Self { stream }
    }

    /// Read a message from the connection.
    ///
    /// # Returns
    /// - `Ok(Some(Message))`: Successfully read and deserialized a message
    /// - `Ok(None)`: Connection closed cleanly or message deserialization failed
    /// - `Err`: I/O error occurred
    ///
    /// # Protocol
    /// 1. Reads 4-byte length prefix (big-endian u32)
    /// 2. Validates message size (max 50MB)
    /// 3. Reads message data of specified length
    /// 4. Deserializes JSON to Message enum
    ///
    /// # Example
    /// ```ignore
    /// match conn.read_message().await? {
    ///     Some(Message::Heartbeat { from_id, .. }) => {
    ///         println!("Received heartbeat from server {}", from_id);
    ///     }
    ///     Some(msg) => println!("Received: {:?}", msg),
    ///     None => println!("Connection closed"),
    /// }
    /// ```
    pub async fn read_message(&mut self) -> Result<Option<Message>> {
        // First, read 4-byte length prefix that tells us the message size
        let mut length_buf = [0u8; 4];

        match self.stream.read_exact(&mut length_buf).await {
            Ok(_) => {
                let length = u32::from_be_bytes(length_buf) as usize;

                // Sanity check: reject messages larger than MAX_MESSAGE_SIZE
                if length > MAX_MESSAGE_SIZE {
                    error!(
                        "❌ Message too large: {} bytes (max: {} bytes)",
                        length, MAX_MESSAGE_SIZE
                    );
                    return Ok(None);
                }

                // Now read the actual message data
                let mut data = vec![0u8; length];
                self.stream.read_exact(&mut data).await?;

                // Deserialize bytes into a Message enum
                match Message::from_bytes(&data) {
                    Ok(msg) => Ok(Some(msg)),
                    Err(e) => {
                        error!("❌ Failed to deserialize message: {}", e);
                        Ok(None)
                    }
                }
            }
            Err(_) => Ok(None), // Connection closed cleanly
        }
    }

    /// Write a message to the connection.
    ///
    /// # Arguments
    /// - `message`: The message to send
    ///
    /// # Returns
    /// - `Ok(())`: Message successfully sent
    /// - `Err`: I/O or serialization error
    ///
    /// # Protocol
    /// 1. Serializes message to JSON
    /// 2. Writes 4-byte length prefix (big-endian u32)
    /// 3. Writes message data
    /// 4. Flushes stream to ensure delivery
    ///
    /// # Example
    /// ```ignore
    /// let heartbeat = Message::Heartbeat {
    ///     from_id: 1,
    ///     timestamp: current_timestamp(),
    ///     load: 0.3,
    /// };
    /// conn.write_message(&heartbeat).await?;
    /// ```
    pub async fn write_message(&mut self, message: &Message) -> Result<()> {
        // Serialize message to JSON bytes
        let data = message.to_bytes()?;
        let length = data.len() as u32;

        // Send: [4 bytes length][message data]
        self.stream.write_all(&length.to_be_bytes()).await?;
        self.stream.write_all(&data).await?;
        self.stream.flush().await?;

        Ok(())
    }
}
