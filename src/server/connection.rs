use crate::messages::Message;
use anyhow::Result;
use log::error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

pub struct Connection {
    stream: TcpStream,
}

impl Connection {
    pub fn new(stream: TcpStream) -> Self {
        Self { stream }
    }

    pub async fn read_message(&mut self) -> Result<Option<Message>> {
        let mut length_buf = [0u8; 4];

        match self.stream.read_exact(&mut length_buf).await {
            Ok(_) => {
                let length = u32::from_be_bytes(length_buf) as usize;

                if length > 10_000_000 {
                    // 10MB max
                    error!("Message too large: {} bytes", length);
                    return Ok(None);
                }

                let mut data = vec![0u8; length];
                self.stream.read_exact(&mut data).await?;

                match Message::from_bytes(&data) {
                    Ok(msg) => Ok(Some(msg)),
                    Err(e) => {
                        error!("Failed to deserialize message: {}", e);
                        Ok(None)
                    }
                }
            }
            Err(_) => Ok(None), // Connection closed
        }
    }

    pub async fn write_message(&mut self, message: &Message) -> Result<()> {
        let data = message.to_bytes()?;
        let length = data.len() as u32;

        self.stream.write_all(&length.to_be_bytes()).await?;
        self.stream.write_all(&data).await?;
        self.stream.flush().await?;

        Ok(())
    }
}

pub async fn send_message_to_peer(tx: &mpsc::Sender<Message>, message: Message) -> Result<()> {
    tx.send(message).await?;
    Ok(())
}
