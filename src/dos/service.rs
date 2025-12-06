//! DoS Service - Business logic for Directory of Services

use super::firebase::FirebaseClient;
use crate::common::messages::{ClientInfo as DosClientInfo, ClientStatus, ImageInfo};
use anyhow::{Context, Result};
use log::info;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

pub struct DoSService {
    firebase: Arc<FirebaseClient>,
}

impl DoSService {
    pub async fn new(firebase_url: String) -> Result<Self> {
        let firebase = Arc::new(FirebaseClient::new(firebase_url));

        // Load existing clients to verify connection
        let clients = firebase.get_all_clients().await
            .context("Failed to load clients from Firebase")?;

        info!("Loaded {} online clients from Firebase", clients.len());

        Ok(Self { firebase })
    }

    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    // ========== CLIENT MANAGEMENT ==========

    pub async fn sign_up_client(&self, client_id: String, ip_address: String) -> Result<String> {
        // Check if client_id already exists
        if self.firebase.get_client(&client_id).await?.is_some() {
            anyhow::bail!("Client ID already exists");
        }

        let client_info = DosClientInfo {
            client_id: client_id.clone(),
            client_name: client_id.clone(),
            status: ClientStatus::Online,
            ip_address,
            last_seen: Self::current_timestamp(),
            images: HashMap::new(),
        };

        self.firebase.store_client(&client_id, &client_info).await?;

        info!("Client signed up: {}", client_id);
        Ok(client_id)
    }

    pub async fn sign_in_client(&self, client_id: String, ip_address: String) -> Result<Vec<serde_json::Value>> {
        // Verify client exists
        let client = self.firebase.get_client(&client_id).await?
            .ok_or_else(|| anyhow::anyhow!("Client not found"))?;

        // Update status to online
        self.firebase.update_client_status(&client_id, ClientStatus::Online).await?;

        // Get offline notifications
        let notifications = self.firebase.get_and_clear_notifications(&client_id).await?;

        info!("Client signed in: {} ({} notifications)", client_id, notifications.len());
        Ok(notifications)
    }

    pub async fn sign_out_client(&self, client_id: String) -> Result<()> {
        self.firebase.update_client_status(&client_id, ClientStatus::Offline).await?;
        info!("Client signed out: {}", client_id);
        Ok(())
    }

    pub async fn list_online_clients(&self) -> Result<Vec<DosClientInfo>> {
        let all_clients = self.firebase.get_all_clients().await?;
        let online: Vec<DosClientInfo> = all_clients
            .into_values()
            .filter(|c| matches!(c.status, ClientStatus::Online))
            .collect();
        Ok(online)
    }

    // ========== IMAGE MANAGEMENT ==========

    pub async fn register_image(&self, client_id: String, image: ImageInfo) -> Result<()> {
        self.firebase.store_image(&client_id, &image).await?;
        info!("Image registered: {} for client {}", image.image_id, client_id);
        Ok(())
    }

    pub async fn request_image_access(&self, requester_id: String, owner_id: String, image_id: String) -> Result<String> {
        let req_num = self.firebase.get_next_request_id().await?;
        let request_id = format!("req_{}_{}", req_num, requester_id);

        let request_data = serde_json::json!({
            "request_id": request_id,
            "requester_id": requester_id,
            "owner_id": owner_id,
            "image_id": image_id,
            "timestamp": Self::current_timestamp()
        });

        // Check if owner is online
        if let Ok(Some(owner)) = self.firebase.get_client(&owner_id).await {
            if matches!(owner.status, ClientStatus::Offline) {
                // Store as pending request
                self.firebase.store_pending_request(&request_id, &request_data).await?;
            }
        }

        info!("Access request created: {}", request_id);
        Ok(request_id)
    }

    pub async fn get_pending_requests(&self, owner_id: String) -> Result<Vec<serde_json::Value>> {
        self.firebase.get_pending_requests(&owner_id).await
    }

    pub async fn respond_to_access_request(&self, request_id: String, approved: bool, view_limit: Option<u32>) -> Result<()> {
        if approved {
            info!("Access request {} approved with view_limit: {:?}", request_id, view_limit);

            // Get the request details
            let request = self.firebase.get_pending_request(&request_id).await?;

            if let Some(req) = request {
                let requester_id = req.get("requester_id").and_then(|v| v.as_str()).unwrap_or("");
                let owner_id = req.get("owner_id").and_then(|v| v.as_str()).unwrap_or("");
                let image_id = req.get("image_id").and_then(|v| v.as_str()).unwrap_or("");

                // Grant access by adding AccessRight
                if let Some(mut owner_client) = self.firebase.get_client(owner_id).await? {
                    if let Some(image) = owner_client.images.get_mut(image_id) {
                        let access_right = crate::common::messages::AccessRight {
                            view_limit: view_limit.unwrap_or(999999), // Default to unlimited if not specified
                            view_count: 0,
                        };
                        image.access_rights.insert(requester_id.to_string(), access_right);
                        self.firebase.store_image(owner_id, image).await?;
                        info!("Granted access to {} for image {} with limit {}", requester_id, image_id, view_limit.unwrap_or(999999));
                    }
                }
            }
        } else {
            info!("Access request {} denied", request_id);
        }

        self.firebase.delete_pending_request(&request_id).await?;
        Ok(())
    }

    pub async fn update_access_rights(&self, client_id: String, image_id: String, access_rights: std::collections::HashMap<String, crate::common::messages::AccessRight>) -> Result<()> {
        // Get client and update image access rights
        if let Some(mut client) = self.firebase.get_client(&client_id).await? {
            if let Some(image) = client.images.get_mut(&image_id) {
                image.access_rights = access_rights;
                self.firebase.store_image(&client_id, image).await?;
            }
        }
        Ok(())
    }

    pub async fn report_peer_failure(&self, failed_client_id: String) -> Result<()> {
        self.firebase.update_client_status(&failed_client_id, ClientStatus::Offline).await?;
        info!("Client marked as failed: {}", failed_client_id);
        Ok(())
    }

    pub async fn increment_view_count(&self, owner_id: String, image_id: String, viewer_id: String) -> Result<bool> {
        // Get the owner's client info
        if let Some(mut owner_client) = self.firebase.get_client(&owner_id).await? {
            if let Some(image) = owner_client.images.get_mut(&image_id) {
                // Check if viewer has access rights
                if let Some(access_right) = image.access_rights.get_mut(&viewer_id) {
                    // Check if view limit exceeded
                    if access_right.view_count >= access_right.view_limit {
                        info!("View limit exceeded for {} viewing image {} of {}", viewer_id, image_id, owner_id);
                        return Ok(false);
                    }

                    // Increment view count
                    access_right.view_count += 1;
                    let new_count = access_right.view_count;
                    let limit = access_right.view_limit;

                    // Clone the image for storage
                    let image_clone = image.clone();
                    self.firebase.store_image(&owner_id, &image_clone).await?;
                    info!("Incremented view count for {} viewing image {} of {} ({}/{})",
                          viewer_id, image_id, owner_id, new_count, limit);
                    return Ok(true);
                } else {
                    anyhow::bail!("Viewer {} does not have access to image {}", viewer_id, image_id);
                }
            } else {
                anyhow::bail!("Image {} not found for owner {}", image_id, owner_id);
            }
        } else {
            anyhow::bail!("Owner {} not found", owner_id);
        }
    }
}
