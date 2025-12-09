//! DoS Client - Client-side interface for Directory of Services
//! Now connects to the leader server instead of a separate DoS server

use crate::common::connection::Connection;
use crate::common::messages::{ClientInfo, ImageInfo, Message};
use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::Mutex;

pub struct DosClient {
    /// List of compute server addresses to query for leader
    server_addresses: Vec<String>,
    _client_name: String,
    client_id: Option<String>,
    /// Cached leader address to avoid repeated queries (uses interior mutability)
    cached_leader_address: Arc<Mutex<Option<String>>>,
}

impl DosClient {
    pub fn new(server_addresses: Vec<String>, client_name: String) -> Self {
        Self {
            server_addresses,
            _client_name: client_name,
            client_id: None,
            cached_leader_address: Arc::new(Mutex::new(None)),
        }
    }

    pub fn client_id(&self) -> Option<&str> {
        self.client_id.as_deref()
    }

    /// Query the compute servers to find the current leader and return its address
    async fn discover_leader(&self) -> Result<String> {
        // If we have a cached leader, try it first
        let cached = self.cached_leader_address.lock().await.clone();
        if let Some(cached) = cached {
            // Verify the cached leader is still valid
            if let Ok(stream) = TcpStream::connect(&cached).await {
                let mut conn = Connection::new(stream);
                if conn.write_message(&Message::LeaderQuery).await.is_ok() {
                    if let Ok(Some(Message::LeaderResponse { leader_id })) = conn.read_message().await {
                        // Convert leader_id (1,2,3) to address
                        if let Some(addr) = self.server_addresses.get(leader_id as usize - 1) {
                            return Ok(addr.clone());
                        }
                    }
                }
            }
            // Cached leader is invalid, clear it
            *self.cached_leader_address.lock().await = None;
        }

        // Query each server until we find the leader
        for server_addr in &self.server_addresses {
            if let Ok(stream) = TcpStream::connect(server_addr).await {
                let mut conn = Connection::new(stream);
                if conn.write_message(&Message::LeaderQuery).await.is_ok() {
                    if let Ok(Some(Message::LeaderResponse { leader_id })) = conn.read_message().await {
                        // Convert leader_id to address
                        if let Some(leader_addr) = self.server_addresses.get(leader_id as usize - 1) {
                            *self.cached_leader_address.lock().await = Some(leader_addr.clone());
                            return Ok(leader_addr.clone());
                        }
                    }
                }
            }
        }

        anyhow::bail!("Failed to discover leader - all servers unreachable or no leader elected")
    }

    async fn connect(&self) -> Result<Connection> {
        let leader_address = self.discover_leader().await
            .context("Failed to connect to DoS (leader)")?;

        let stream = TcpStream::connect(&leader_address)
            .await
            .context("Failed to connect to leader server")?;
        Ok(Connection::new(stream))
    }

    // ========== CLIENT MANAGEMENT ==========

    pub async fn sign_up(&mut self, client_id: String, ip_address: String, p2p_port: u16) -> Result<String> {
        let mut conn = self.connect().await?;

        let message = Message::ClientSignUp {
            client_name: client_id.clone(),
            ip_address,
            p2p_port,
        };

        conn.write_message(&message).await?;

        let response = conn
            .read_message()
            .await?
            .ok_or_else(|| anyhow::anyhow!("Connection closed"))?;

        match response {
            Message::ClientSignUpResponse { client_id } => {
                self.client_id = Some(client_id.clone());
                Ok(client_id)
            }
            _ => anyhow::bail!("Unexpected response to sign up"),
        }
    }

    pub async fn sign_in(
        &mut self,
        client_id: String,
        ip_address: String,
        p2p_port: u16,
    ) -> Result<Vec<serde_json::Value>> {
        let mut conn = self.connect().await?;

        let message = Message::ClientSignIn {
            client_id: client_id.clone(),
            ip_address,
            p2p_port,
        };

        conn.write_message(&message).await?;

        let response = conn
            .read_message()
            .await?
            .ok_or_else(|| anyhow::anyhow!("Connection closed"))?;

        match response {
            Message::ClientSignInResponse { notifications } => {
                self.client_id = Some(client_id);
                Ok(notifications)
            }
            _ => anyhow::bail!("Unexpected response to sign in"),
        }
    }

    pub async fn sign_out(&mut self) -> Result<()> {
        let client_id = self
            .client_id
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Not signed in"))?;

        let mut conn = self.connect().await?;

        let message = Message::ClientSignOut {
            client_id: client_id.clone(),
        };

        conn.write_message(&message).await?;

        let response = conn
            .read_message()
            .await?
            .ok_or_else(|| anyhow::anyhow!("Connection closed"))?;

        match response {
            Message::Ack => {
                self.client_id = None;
                Ok(())
            }
            _ => anyhow::bail!("Unexpected response to sign out"),
        }
    }

    pub async fn list_online_clients(&self) -> Result<Vec<ClientInfo>> {
        let mut conn = self.connect().await?;

        let message = Message::ListOnlineClients;
        conn.write_message(&message).await?;

        let response = conn
            .read_message()
            .await?
            .ok_or_else(|| anyhow::anyhow!("Connection closed"))?;

        match response {
            Message::OnlineClientsList { clients } => Ok(clients),
            _ => anyhow::bail!("Unexpected response to list clients"),
        }
    }

    // ========== IMAGE MANAGEMENT ==========

    pub async fn register_image(&self, image: ImageInfo) -> Result<()> {
        let client_id = self
            .client_id
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Not signed in"))?;

        let mut conn = self.connect().await?;

        let message = Message::RegisterImage {
            client_id: client_id.clone(),
            image,
        };

        conn.write_message(&message).await?;

        let response = conn
            .read_message()
            .await?
            .ok_or_else(|| anyhow::anyhow!("Connection closed"))?;

        match response {
            Message::Ack => Ok(()),
            _ => anyhow::bail!("Unexpected response to register image"),
        }
    }

    pub async fn request_image_access(&self, owner_id: &str, image_id: &str, req_access_limit: Option<u32>) -> Result<String> {
        let client_id = self
            .client_id
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Not signed in"))?;

        let mut conn = self.connect().await?;

        let message = Message::ImageAccessRequest {
            request_id: String::new(), // Server will assign
            requester_id: client_id.clone(),
            owner_id: owner_id.to_string(),
            image_id: image_id.to_string(),
            req_access_limit,
        };

        conn.write_message(&message).await?;

        let response = conn
            .read_message()
            .await?
            .ok_or_else(|| anyhow::anyhow!("Connection closed"))?;

        match response {
            Message::ImageAccessRequest { request_id, .. } => Ok(request_id),
            _ => anyhow::bail!("Unexpected response to access request"),
        }
    }

    pub async fn respond_to_access_request(&self, request_id: &str, approved: bool, view_limit: Option<u32>) -> Result<()> {
        let mut conn = self.connect().await?;

        let message = Message::ImageAccessResponse {
            request_id: request_id.to_string(),
            approved,
            view_limit,
        };

        conn.write_message(&message).await?;

        let response = conn
            .read_message()
            .await?
            .ok_or_else(|| anyhow::anyhow!("Connection closed"))?;

        match response {
            Message::Ack => Ok(()),
            _ => anyhow::bail!("Unexpected response to access response"),
        }
    }

    pub async fn update_access_rights(
        &self,
        image_id: &str,
        access_rights: std::collections::HashMap<String, crate::common::messages::AccessRight>,
    ) -> Result<()> {
        let client_id = self
            .client_id
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Not signed in"))?;

        let mut conn = self.connect().await?;

        let message = Message::UpdateAccessRights {
            client_id: client_id.clone(),
            image_id: image_id.to_string(),
            access_rights,
        };

        conn.write_message(&message).await?;

        let response = conn
            .read_message()
            .await?
            .ok_or_else(|| anyhow::anyhow!("Connection closed"))?;

        match response {
            Message::Ack => Ok(()),
            _ => anyhow::bail!("Unexpected response to update access rights"),
        }
    }

    pub async fn report_peer_failure(&self, failed_client_id: &str) -> Result<()> {
        let mut conn = self.connect().await?;

        let message = Message::ReportPeerFailure {
            failed_client_id: failed_client_id.to_string(),
        };

        conn.write_message(&message).await?;

        let response = conn
            .read_message()
            .await?
            .ok_or_else(|| anyhow::anyhow!("Connection closed"))?;

        match response {
            Message::Ack => Ok(()),
            _ => anyhow::bail!("Unexpected response to failure report"),
        }
    }

    pub async fn get_pending_requests(&self, owner_id: &str) -> Result<Vec<serde_json::Value>> {
        let mut conn = self.connect().await?;

        let message = Message::GetPendingRequests {
            owner_id: owner_id.to_string(),
        };

        conn.write_message(&message).await?;

        let response = conn
            .read_message()
            .await?
            .ok_or_else(|| anyhow::anyhow!("Connection closed"))?;

        match response {
            Message::PendingRequestsList { requests } => Ok(requests),
            _ => anyhow::bail!("Unexpected response to get pending requests"),
        }
    }

    pub async fn increment_view_count(&self, owner_id: &str, image_id: &str, viewer_id: &str) -> Result<bool> {
        let mut conn = self.connect().await?;

        let message = Message::IncrementViewCount {
            owner_id: owner_id.to_string(),
            image_id: image_id.to_string(),
            viewer_id: viewer_id.to_string(),
        };

        conn.write_message(&message).await?;

        let response = conn
            .read_message()
            .await?
            .ok_or_else(|| anyhow::anyhow!("Connection closed"))?;

        match response {
            Message::IncrementViewCountResponse { allowed } => Ok(allowed),
            _ => anyhow::bail!("Unexpected response to increment view count"),
        }
    }

    // ========== P2P SUPPORT ==========

    /// Get a peer's IP address and P2P port for direct connection
    ///
    /// # Arguments
    /// - `peer_id`: ID of the peer client
    ///
    /// # Returns
    /// - `Ok(Some((ip_address, p2p_port)))`: Peer's connection info if online
    /// - `Ok(None)`: Peer is offline or not found
    pub async fn get_peer_address(&self, peer_id: &str) -> Result<Option<(String, u16)>> {
        println!("🔍 [DOS_CLIENT] Requesting peer address for: {}", peer_id);
        let mut conn = self.connect().await?;
        println!("✅ [DOS_CLIENT] Connected to DoS server");

        let message = Message::GetPeerAddress {
            peer_id: peer_id.to_string(),
        };

        println!("📤 [DOS_CLIENT] Sending GetPeerAddress request");
        conn.write_message(&message).await?;

        println!("📥 [DOS_CLIENT] Waiting for response...");
        let response = conn
            .read_message()
            .await?
            .ok_or_else(|| anyhow::anyhow!("Connection closed"))?;

        println!("📨 [DOS_CLIENT] Received response: {:?}", response);

        match response {
            Message::PeerAddressResponse {
                ip_address,
                p2p_port,
                online,
                ..
            } => {
                println!("📊 [DOS_CLIENT] Peer info - IP: {}, Port: {}, Online: {}", ip_address, p2p_port, online);
                if online && p2p_port > 0 {
                    println!("✅ [DOS_CLIENT] Peer is online and available");
                    Ok(Some((ip_address, p2p_port)))
                } else {
                    println!("⚠️  [DOS_CLIENT] Peer is offline or has no P2P port");
                    Ok(None)
                }
            }
            _ => {
                println!("❌ [DOS_CLIENT] Unexpected response type");
                anyhow::bail!("Unexpected response to get peer address")
            }
        }
    }
}
