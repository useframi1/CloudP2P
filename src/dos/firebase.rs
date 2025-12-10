//! Firebase Realtime Database client for DoS

use crate::common::messages::{ClientInfo as DosClientInfo, ClientStatus, ImageInfo};
use anyhow::Result;
use reqwest::Client;
use serde_json::Value;
use std::collections::HashMap;

pub struct FirebaseClient {
    client: Client,
    base_url: String,
}

impl FirebaseClient {
    pub fn new(base_url: String) -> Self {
        Self {
            client: Client::new(),
            base_url,
        }
    }

    // ========== ID GENERATION ==========

    pub async fn get_next_client_id(&self) -> Result<u64> {
        let url = format!("{}/metadata/next_client_id.json", self.base_url);

        // Get current value
        let response = self.client.get(&url).send().await?;
        let value: Value = response.json().await?;

        let current_id = if value.is_null() {
            1u64
        } else {
            value.as_u64().unwrap_or(1)
        };

        let next_id = current_id + 1;

        // Update to next value
        self.client.put(&url).json(&next_id).send().await?;

        Ok(current_id)
    }

    pub async fn get_next_request_id(&self) -> Result<u64> {
        let url = format!("{}/metadata/next_request_id.json", self.base_url);

        let response = self.client.get(&url).send().await?;
        let value: Value = response.json().await?;

        let current_id = if value.is_null() {
            1u64
        } else {
            value.as_u64().unwrap_or(1)
        };

        let next_id = current_id + 1;

        self.client.put(&url).json(&next_id).send().await?;

        Ok(current_id)
    }

    pub async fn get_default_view_limit(&self) -> Result<u32> {
        let url = format!("{}/metadata/default_view_limit.json", self.base_url);
        let response = self.client.get(&url).send().await?;
        let value: Value = response.json().await?;

        let limit = if value.is_null() {
            // Set default to 2 if not exists
            let default_limit = 2u32;
            self.client.put(&url).json(&default_limit).send().await?;
            default_limit
        } else {
            value.as_u64().unwrap_or(2) as u32
        };

        Ok(limit)
    }

    // ========== CLIENT OPERATIONS ==========

    pub async fn store_client(&self, client_id: &str, client: &DosClientInfo) -> Result<()> {
        let url = format!("{}/clients/{}.json", self.base_url, client_id);
        let response = self.client.put(&url).json(client).send().await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to store client: {}", response.status());
        }

        Ok(())
    }

    pub async fn get_client(&self, client_id: &str) -> Result<Option<DosClientInfo>> {
        let url = format!("{}/clients/{}.json", self.base_url, client_id);
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to get client: {}", response.status());
        }

        let client: Option<DosClientInfo> = response.json().await?;
        Ok(client)
    }

    pub async fn get_all_clients(&self) -> Result<HashMap<String, DosClientInfo>> {
        let url = format!("{}/clients.json", self.base_url);
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to get clients: {}", response.status());
        }

        let clients: Option<HashMap<String, DosClientInfo>> = response.json().await?;
        Ok(clients.unwrap_or_default())
    }

    pub async fn update_client_status(&self, client_id: &str, status: ClientStatus) -> Result<()> {
        let url = format!("{}/clients/{}/status.json", self.base_url, client_id);
        let response = self.client.put(&url).json(&status).send().await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to update status: {}", response.status());
        }

        Ok(())
    }

    // ========== IMAGE OPERATIONS ==========

    pub async fn store_image(&self, client_id: &str, image: &ImageInfo) -> Result<()> {
        let url = format!("{}/clients/{}/images/{}.json",
                         self.base_url, client_id, image.image_id);
        let response = self.client.put(&url).json(image).send().await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to store image: {}", response.status());
        }

        Ok(())
    }

    pub async fn grant_access(
        &self,
        owner_id: &str,
        image_id: &str,
        requester_id: &str,
        view_limit: u32,
    ) -> Result<()> {
        use crate::common::messages::AccessRight;

        // Get the image
        let url = format!("{}/clients/{}/images/{}.json", self.base_url, owner_id, image_id);
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            anyhow::bail!("Image not found");
        }

        let mut image: ImageInfo = response.json().await?;

        // Grant access
        image.access_rights.insert(
            requester_id.to_string(),
            AccessRight {
                view_limit,
                view_count: 0,
            },
        );

        // Store updated image
        let response = self.client.put(&url).json(&image).send().await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to grant access: {}", response.status());
        }

        Ok(())
    }

    // ========== PENDING REQUESTS ==========

    pub async fn store_pending_request(&self, request_id: &str, data: &Value) -> Result<()> {
        let url = format!("{}/pending_requests/{}.json", self.base_url, request_id);
        let response = self.client.put(&url).json(data).send().await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to store pending request: {}", response.status());
        }

        Ok(())
    }

    pub async fn get_pending_request(&self, request_id: &str) -> Result<Option<Value>> {
        let url = format!("{}/pending_requests/{}.json", self.base_url, request_id);
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Ok(None);
        }

        let request: Option<Value> = response.json().await?;
        Ok(request)
    }

    pub async fn get_pending_requests(&self, owner_id: &str) -> Result<Vec<Value>> {
        let url = format!("{}/pending_requests.json", self.base_url);
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to get pending requests: {}", response.status());
        }

        let all_requests: Option<HashMap<String, Value>> = response.json().await?;

        if let Some(requests) = all_requests {
            let owner_requests: Vec<Value> = requests
                .into_values()
                .filter(|req| {
                    req.get("owner_id")
                        .and_then(|v| v.as_str())
                        .map(|id| id == owner_id)
                        .unwrap_or(false)
                })
                .collect();
            Ok(owner_requests)
        } else {
            Ok(Vec::new())
        }
    }

    pub async fn delete_pending_request(&self, request_id: &str) -> Result<()> {
        let url = format!("{}/pending_requests/{}.json", self.base_url, request_id);
        let response = self.client.delete(&url).send().await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to delete pending request: {}", response.status());
        }

        Ok(())
    }

    // ========== OFFLINE NOTIFICATIONS ==========

    pub async fn store_offline_notification(&self, client_id: &str, notification: &Value) -> Result<()> {
        let url = format!("{}/offline_notifications/{}.json", self.base_url, client_id);
        let response = self.client.post(&url).json(notification).send().await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to store notification: {}", response.status());
        }

        Ok(())
    }

    pub async fn get_and_clear_notifications(&self, client_id: &str) -> Result<Vec<Value>> {
        let url = format!("{}/offline_notifications/{}.json", self.base_url, client_id);

        // Get notifications
        let response = self.client.get(&url).send().await?;
        let notifications: Option<HashMap<String, Value>> = if response.status().is_success() {
            response.json().await?
        } else {
            None
        };

        // Clear notifications
        let _ = self.client.delete(&url).send().await;

        Ok(notifications.map(|map| map.into_values().collect()).unwrap_or_default())
    }

    // ========== PENDING ACCESS CHANGES ==========
    // Used when owner modifies/revokes access but requester is offline

    /// Store a pending access change for an offline requester
    /// Path: /pending_access_changes/{requester_id}/{change_id}
    pub async fn store_pending_access_change(&self, requester_id: &str, change: &Value) -> Result<()> {
        let url = format!("{}/pending_access_changes/{}.json", self.base_url, requester_id);
        let response = self.client.post(&url).json(change).send().await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to store pending access change: {}", response.status());
        }

        Ok(())
    }

    /// Get and clear all pending access changes for a requester (called on sign-in)
    pub async fn get_and_clear_pending_access_changes(&self, requester_id: &str) -> Result<Vec<Value>> {
        let url = format!("{}/pending_access_changes/{}.json", self.base_url, requester_id);

        // Get pending changes
        let response = self.client.get(&url).send().await?;
        let changes: Option<HashMap<String, Value>> = if response.status().is_success() {
            response.json().await?
        } else {
            None
        };

        // Clear pending changes
        let _ = self.client.delete(&url).send().await;

        Ok(changes.map(|map| map.into_values().collect()).unwrap_or_default())
    }
}
