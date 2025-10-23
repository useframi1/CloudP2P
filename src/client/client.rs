use tokio::net::TcpStream;
use crate::server::connection::Connection;
use crate::messages::*;
use uuid::Uuid;
use anyhow::Result;
use log::info;

pub struct Client {
    user_id: String,
    username: String,
    server_addresses: Vec<String>,
}

impl Client {
    pub fn new(user_id: String, username: String, server_addresses: Vec<String>) -> Self {
        Self {
            user_id,
            username,
            server_addresses,
        }
    }
    
    pub async fn send_encryption_request(&self, image_data: Vec<u8>) -> Result<()> {
        let request_id = Uuid::new_v4();
        
        info!("📤 Client {} sending encryption request {:?}", self.user_id, request_id);
        
        let encryption_req = EncryptionRequest {
            image_data,
            username: self.username.clone(),
            allowed_users: vec![self.username.clone()],
            view_count: 5,
        };
        
        let message = Message::WorkRequest {
            request_id,
            request_type: RequestType::Encryption,
            data: serde_json::to_vec(&encryption_req)?,
            client_id: self.user_id.clone(),
        };
        
        // Multicast to all servers
        for addr in &self.server_addresses {
            match TcpStream::connect(addr).await {
                Ok(stream) => {
                    let mut conn = Connection::new(stream);
                    conn.write_message(&message).await?;
                    info!("✉️  Sent to server {}", addr);
                }
                Err(e) => {
                    eprintln!("Failed to connect to {}: {}", addr, e);
                }
            }
        }
        
        Ok(())
    }
}