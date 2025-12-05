//! DoS Client - Client-side interface for Directory of Services

use crate::common::connection::Connection;
use crate::common::messages::{ClientInfo, ImageInfo, Message};
use anyhow::{Context, Result};
use tokio::net::TcpStream;

pub struct DosClient {
    dos_address: String,
    client_name: String,
    client_id: Option<String>,
}

impl DosClient {
    pub fn new(dos_address: String, client_name: String) -> Self {
        Self {
            dos_address,
            client_name,
            client_id: None,
        }
    }

    pub fn client_id(&self) -> Option<&str> {
        self.client_id.as_deref()
    }

    async fn connect(&self) -> Result<Connection> {
        let stream = TcpStream::connect(&self.dos_address)
            .await
            .context("Failed to connect to DoS")?;
        Ok(Connection::new(stream))
    }

    // ========== CLIENT MANAGEMENT ==========

    pub async fn sign_up(&mut self, ip_address: String) -> Result<String> {
        let mut conn = self.connect().await?;

        let message = Message::ClientSignUp {
            client_name: self.client_name.clone(),
            ip_address,
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
    ) -> Result<Vec<serde_json::Value>> {
        let mut conn = self.connect().await?;

        let message = Message::ClientSignIn {
            client_id: client_id.clone(),
            ip_address,
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

    pub async fn request_image_access(&self, owner_id: &str, image_id: &str) -> Result<String> {
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

    pub async fn respond_to_access_request(&self, request_id: &str, approved: bool) -> Result<()> {
        let mut conn = self.connect().await?;

        let message = Message::ImageAccessResponse {
            request_id: request_id.to_string(),
            approved,
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
        access_list: Vec<String>,
    ) -> Result<()> {
        let client_id = self
            .client_id
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Not signed in"))?;

        let mut conn = self.connect().await?;

        let message = Message::UpdateAccessRights {
            client_id: client_id.clone(),
            image_id: image_id.to_string(),
            access_list,
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
}
