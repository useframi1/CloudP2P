//! # P2P Service
//!
//! Handles peer-to-peer direct image transfer between clients.
//! Provides:
//! - P2P listener for incoming image requests from peers
//! - Direct peer connection for requesting images
//! - Access control verification before sending images
//! - View count management

use crate::client::dos_client::DosClient;
use crate::client::middleware::ClientMiddleware;
use crate::common::connection::Connection;
use crate::common::messages::Message;
use crate::dos::firebase::FirebaseClient;
use anyhow::{anyhow, Result};
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

/// P2P Service for handling direct peer-to-peer image transfers
pub struct P2PService {
    /// Port this client listens on for P2P connections
    pub p2p_port: u16,
    /// Client ID of this client
    pub client_id: String,
    /// DoS client for access control verification
    pub dos_client: Arc<Mutex<DosClient>>,
    /// Firebase client for direct database access
    pub firebase: Arc<FirebaseClient>,
    /// Directory where encrypted images are stored
    pub image_dir: String,
    /// Client middleware for distributed server encryption
    pub client_middleware: Arc<Mutex<ClientMiddleware>>,
}

impl P2PService {
    /// Create a new P2P service instance
    pub fn new(
        p2p_port: u16,
        client_id: String,
        dos_client: Arc<Mutex<DosClient>>,
        firebase: Arc<FirebaseClient>,
        image_dir: String,
        client_middleware: Arc<Mutex<ClientMiddleware>>,
    ) -> Self {
        Self {
            p2p_port,
            client_id,
            dos_client,
            firebase,
            image_dir,
            client_middleware,
        }
    }

    /// Start listening for incoming P2P connections
    ///
    /// This method spawns a background task that listens on the P2P port
    /// and handles incoming image requests from peers.
    pub async fn start_listener(self: Arc<Self>) -> Result<()> {
        let addr = format!("0.0.0.0:{}", self.p2p_port);
        let listener = TcpListener::bind(&addr).await?;

        println!("[P2P] Listening on {} for peer connections", addr);

        loop {
            match listener.accept().await {
                Ok((socket, peer_addr)) => {
                    println!("[P2P] Accepted connection from {}", peer_addr);
                    let service = self.clone();
                    tokio::spawn(async move {
                        if let Err(e) = service.handle_peer_connection(socket).await {
                            eprintln!("[P2P] Error handling peer connection: {}", e);
                        }
                    });
                }
                Err(e) => {
                    eprintln!("[P2P] Error accepting connection: {}", e);
                }
            }
        }
    }

    /// Handle an incoming P2P connection from a peer
    async fn handle_peer_connection(&self, socket: TcpStream) -> Result<()> {
        let mut conn = Connection::new(socket);

        // Read the incoming request
        let message = conn
            .read_message()
            .await?
            .ok_or_else(|| anyhow!("Connection closed before receiving message"))?;

        match message {
            Message::P2PImageRequest {
                requester_id,
                image_id,
            } => {
                println!(
                    "[P2P] Received image request for {} from {}",
                    image_id, requester_id
                );
                self.handle_image_request(&mut conn, &requester_id, &image_id)
                    .await?;
            }
            Message::PersonalizedCarrierDelivery {
                image_id,
                owner_id,
                carrier_data,
            } => {
                println!(
                    "📥 [P2P] Receiving personalized carrier for {} from {}",
                    image_id, owner_id
                );
                self.handle_personalized_carrier_delivery(&image_id, &owner_id, carrier_data)
                    .await?;
            }
            Message::AccessRightsUpdate {
                owner_id,
                image_id,
                requester_id,
                revoked,
                new_view_limit,
            } => {
                println!(
                    "🔧 [P2P] Receiving access rights update for {} from {} (revoked: {}, new_limit: {:?})",
                    image_id, owner_id, revoked, new_view_limit
                );
                self.handle_access_rights_update(&owner_id, &image_id, &requester_id, revoked, new_view_limit)
                    .await?;
            }
            _ => {
                println!("[P2P] Unexpected message type received");
                let response = Message::P2PAccessDenied {
                    image_id: String::new(),
                    reason: "Invalid message type".to_string(),
                };
                conn.write_message(&response).await?;
            }
        }

        Ok(())
    }

    /// Handle receiving a personalized carrier from an owner (push delivery at approval time)
    ///
    /// Saves the personalized carrier to local disk for offline viewing
    async fn handle_personalized_carrier_delivery(
        &self,
        image_id: &str,
        owner_id: &str,
        carrier_data: Vec<u8>,
    ) -> Result<()> {
        println!(
            "📦 [P2P_RECEIVE] Received personalized carrier: {} bytes",
            carrier_data.len()
        );

        // Create directory for received images from this owner
        let save_dir = format!("encrypted_images/{}", self.client_id);
        std::fs::create_dir_all(&save_dir)?;

        // Save with owner info in filename
        let save_path = format!("{}/{}_from_{}.png", save_dir, image_id, owner_id);

        std::fs::write(&save_path, &carrier_data)?;
        println!(
            "💾 [P2P_RECEIVE] Saved personalized carrier to: {}",
            save_path
        );

        Ok(())
    }

    /// Handle access rights update from owner
    ///
    /// Owner notifies requester that access has been revoked or modified.
    /// Requester updates or deletes their local personalized carrier.
    async fn handle_access_rights_update(
        &self,
        owner_id: &str,
        image_id: &str,
        requester_id: &str,
        revoked: bool,
        new_view_limit: Option<u32>,
    ) -> Result<()> {
        use crate::processing::{extract_image_with_access_rights, update_embedded_access_rights, EmbeddedAccessRights};

        // Verify this update is for me
        if requester_id != self.client_id {
            println!(
                "⚠️ [P2P_ACCESS_UPDATE] Update not for me (expected {}, got {})",
                self.client_id, requester_id
            );
            return Ok(());
        }

        let carrier_path = format!(
            "encrypted_images/{}/{}_from_{}.png",
            self.client_id, image_id, owner_id
        );

        if revoked {
            // Delete the local personalized carrier
            if std::fs::remove_file(&carrier_path).is_ok() {
                println!(
                    "🗑️ [P2P_ACCESS_UPDATE] Access revoked - deleted carrier: {}",
                    carrier_path
                );
            } else {
                println!(
                    "⚠️ [P2P_ACCESS_UPDATE] Carrier not found (may already be deleted): {}",
                    carrier_path
                );
            }
        } else if let Some(new_limit) = new_view_limit {
            // Update the embedded access rights in the local carrier
            match std::fs::read(&carrier_path) {
                Ok(carrier_data) => {
                    // Extract current access rights to get view count
                    match extract_image_with_access_rights(&carrier_data) {
                        Ok((_secret, Some(current_access))) => {
                            // Create updated access rights with new limit and reset count to 0
                            let updated_access = EmbeddedAccessRights {
                                username: self.client_id.clone(),
                                view_limit: new_limit,
                                view_count: 0, // Reset count to 0
                            };

                            // Update the carrier with new access rights
                            match update_embedded_access_rights(&carrier_data, &updated_access) {
                                Ok(updated_carrier) => {
                                    std::fs::write(&carrier_path, updated_carrier)?;
                                    println!(
                                        "✅ [P2P_ACCESS_UPDATE] Updated view limit from {} to {} (reset count to 0)",
                                        current_access.view_limit, new_limit
                                    );
                                }
                                Err(e) => {
                                    println!("❌ [P2P_ACCESS_UPDATE] Failed to update carrier: {}", e);
                                }
                            }
                        }
                        Ok((_secret, None)) => {
                            println!("⚠️ [P2P_ACCESS_UPDATE] No access rights found in carrier");
                        }
                        Err(e) => {
                            println!("❌ [P2P_ACCESS_UPDATE] Failed to extract access rights: {}", e);
                        }
                    }
                }
                Err(e) => {
                    println!(
                        "⚠️ [P2P_ACCESS_UPDATE] Carrier not found: {} - {}",
                        carrier_path, e
                    );
                }
            }
        }

        Ok(())
    }

    /// Handle an image request from a peer - Steganography Flow
    ///
    /// 1. Get image info from Firebase
    /// 2. Check if personalized carrier exists (created at approval time)
    /// 3. Send the pre-created personalized carrier
    /// 4. Create personalized carrier with secret + requester's access rights
    /// 5. Send personalized carrier to requester
    /// 6. Increment view count
    async fn handle_image_request(
        &self,
        conn: &mut Connection,
        requester_id: &str,
        image_id: &str,
    ) -> Result<()> {
        println!(
            "🔐 [P2P_HANDLER] Processing request from {} for {}",
            requester_id, image_id
        );

        // Get image info and access rights from Firebase
        let owner_client = match self.firebase.get_client(&self.client_id).await {
            Ok(Some(client)) => client,
            Ok(None) => {
                println!("❌ [P2P_HANDLER] Owner {} not found", self.client_id);
                let response = Message::P2PAccessDenied {
                    image_id: image_id.to_string(),
                    reason: "Owner not found".to_string(),
                };
                conn.write_message(&response).await?;
                return Ok(());
            }
            Err(e) => {
                println!("❌ [P2P_HANDLER] Failed to get owner info: {}", e);
                let response = Message::P2PAccessDenied {
                    image_id: image_id.to_string(),
                    reason: format!("Failed to get owner info: {}", e),
                };
                conn.write_message(&response).await?;
                return Ok(());
            }
        };

        // Get the image and check access rights
        let image_info = match owner_client.images.get(image_id) {
            Some(img) => img.clone(),
            None => {
                println!("❌ [P2P_HANDLER] Image {} not found", image_id);
                let response = Message::P2PAccessDenied {
                    image_id: image_id.to_string(),
                    reason: "Image not found".to_string(),
                };
                conn.write_message(&response).await?;
                return Ok(());
            }
        };

        // Check if there's a personalized carrier for this requester (created at approval time)
        let personalized_path = match image_info.personalized_carriers.get(requester_id) {
            Some(path) => path.clone(),
            None => {
                println!("❌ [P2P_HANDLER] No personalized carrier found for {}. Request may not be approved yet.", requester_id);
                let response = Message::P2PAccessDenied {
                    image_id: image_id.to_string(),
                    reason: "Access not granted or personalized carrier not created".to_string(),
                };
                conn.write_message(&response).await?;
                return Ok(());
            }
        };

        // Get access rights for view count checking
        let access_right = match image_info.access_rights.get(requester_id) {
            Some(ar) => ar.clone(),
            None => {
                println!("❌ [P2P_HANDLER] No access rights for {}", requester_id);
                let response = Message::P2PAccessDenied {
                    image_id: image_id.to_string(),
                    reason: "No access rights".to_string(),
                };
                conn.write_message(&response).await?;
                return Ok(());
            }
        };

        // Check view limit BEFORE sending
        if access_right.view_count >= access_right.view_limit {
            println!(
                "🚫 [P2P_HANDLER] View limit reached: {}/{}",
                access_right.view_count, access_right.view_limit
            );
            let response = Message::P2PAccessDenied {
                image_id: image_id.to_string(),
                reason: "View limit reached".to_string(),
            };
            conn.write_message(&response).await?;
            return Ok(());
        }

        println!(
            "✅ [P2P_HANDLER] Access granted: {}/{} views used",
            access_right.view_count, access_right.view_limit
        );

        // Read the pre-created personalized carrier from disk
        println!(
            "📂 [P2P_HANDLER] Reading personalized carrier from: {}",
            personalized_path
        );
        let personalized_carrier = match std::fs::read(&personalized_path) {
            Ok(data) => data,
            Err(e) => {
                println!(
                    "❌ [P2P_HANDLER] Failed to read personalized carrier: {}",
                    e
                );
                let response = Message::P2PAccessDenied {
                    image_id: image_id.to_string(),
                    reason: format!("Personalized carrier not found: {}", e),
                };
                conn.write_message(&response).await?;
                return Ok(());
            }
        };

        println!(
            "✅ [P2P_HANDLER] Read personalized carrier: {} bytes",
            personalized_carrier.len()
        );

        // Increment view count in Firebase
        let dos_client = self.dos_client.lock().await;
        if let Err(e) = dos_client
            .increment_view_count(&self.client_id, image_id, requester_id)
            .await
        {
            println!("⚠️ [P2P_HANDLER] Failed to increment view count: {}", e);
        }
        drop(dos_client);

        // Send personalized carrier to requester
        println!(
            "📤 [P2P_HANDLER] Sending personalized carrier to {}",
            requester_id
        );
        let response = Message::P2PImageResponse {
            image_id: image_id.to_string(),
            image_data: personalized_carrier,
            success: true,
        };
        conn.write_message(&response).await?;
        println!("✅ [P2P_HANDLER] Personalized carrier sent successfully");

        Ok(())
    }

    /// Request an image directly from a peer
    ///
    /// # Arguments
    /// - `peer_ip`: IP address of the peer
    /// - `peer_port`: P2P port of the peer
    /// - `image_id`: ID of the image to request
    /// - `requester_id`: ID of the client making the request
    ///
    /// # Returns
    /// Image data as bytes on success
    pub async fn request_image_from_peer(
        peer_ip: &str,
        peer_port: u16,
        image_id: &str,
        requester_id: &str,
    ) -> Result<Vec<u8>> {
        let addr = format!("{}:{}", peer_ip, peer_port);
        println!("🔌 [P2P_REQUEST] Attempting to connect to peer at {}", addr);
        println!(
            "📋 [P2P_REQUEST] Requester: {}, Image: {}",
            requester_id, image_id
        );

        // Connect to peer
        println!("🔗 [P2P_REQUEST] Connecting...");
        let socket = match TcpStream::connect(&addr).await {
            Ok(s) => {
                println!("✅ [P2P_REQUEST] TCP connection established");
                s
            }
            Err(e) => {
                println!("❌ [P2P_REQUEST] Failed to connect: {}", e);
                return Err(anyhow!("Failed to connect to peer: {}", e));
            }
        };
        let mut conn = Connection::new(socket);

        // Send image request
        let request = Message::P2PImageRequest {
            requester_id: requester_id.to_string(),
            image_id: image_id.to_string(),
        };
        println!("📤 [P2P_REQUEST] Sending P2PImageRequest message");
        conn.write_message(&request).await?;
        println!("✅ [P2P_REQUEST] Request sent, waiting for response...");

        // Read response
        println!("📥 [P2P_REQUEST] Reading response...");
        let response = conn
            .read_message()
            .await?
            .ok_or_else(|| anyhow!("Connection closed before receiving response"))?;

        println!("📨 [P2P_REQUEST] Received response: {:?}", response);

        match response {
            Message::P2PImageResponse {
                image_data,
                success,
                ..
            } => {
                if success {
                    println!(
                        "✅ [P2P_REQUEST] Success! Received image data ({} bytes)",
                        image_data.len()
                    );
                    Ok(image_data)
                } else {
                    println!("❌ [P2P_REQUEST] Peer returned unsuccessful response");
                    Err(anyhow!("Peer returned unsuccessful response"))
                }
            }
            Message::P2PAccessDenied { reason, .. } => {
                println!("🚫 [P2P_REQUEST] Access denied: {}", reason);
                Err(anyhow!("Access denied by peer: {}", reason))
            }
            _ => {
                println!("❌ [P2P_REQUEST] Unexpected response type");
                Err(anyhow!("Unexpected response type from peer"))
            }
        }
    }

    /// Send a personalized carrier to a requester (called when owner approves request)
    ///
    /// # Arguments
    /// - `requester_ip`: IP address of the requester
    /// - `requester_p2p_port`: P2P port of the requester
    /// - `image_id`: ID of the image
    /// - `owner_id`: ID of the owner
    /// - `personalized_carrier`: The personalized carrier bytes to send
    ///
    /// # Returns
    /// Ok(()) on success
    pub async fn send_personalized_carrier(
        &self,
        requester_ip: &str,
        requester_p2p_port: u16,
        image_id: &str,
        owner_id: &str,
        personalized_carrier: Vec<u8>,
    ) -> Result<()> {
        println!(
            "📡 [P2P_SEND] Sending personalized carrier for {} to {}:{}",
            image_id, requester_ip, requester_p2p_port
        );

        let address = format!("{}:{}", requester_ip, requester_p2p_port);
        let stream = TcpStream::connect(&address).await?;
        let mut conn = Connection::new(stream);

        // Send PersonalizedCarrierDelivery message
        let message = Message::PersonalizedCarrierDelivery {
            image_id: image_id.to_string(),
            owner_id: owner_id.to_string(),
            carrier_data: personalized_carrier,
        };

        conn.write_message(&message).await?;
        println!("✅ [P2P_SEND] Personalized carrier sent successfully");

        Ok(())
    }

    /// Send access rights update to a requester
    ///
    /// Owner calls this to notify a requester that their access has been revoked or modified.
    /// The requester will update or delete their local personalized carrier.
    pub async fn send_access_rights_update(
        &self,
        requester_ip: &str,
        requester_port: u16,
        owner_id: String,
        image_id: String,
        requester_id: String,
        revoked: bool,
        new_view_limit: Option<u32>,
    ) -> Result<()> {
        println!(
            "📡 [P2P_ACCESS_UPDATE] Sending access update to {}:{} for image {} (revoked: {}, new_limit: {:?})",
            requester_ip, requester_port, image_id, revoked, new_view_limit
        );

        // Connect to requester's P2P port
        let addr = format!("{}:{}", requester_ip, requester_port);
        let socket = TcpStream::connect(&addr).await?;
        let mut conn = Connection::new(socket);

        // Send AccessRightsUpdate message
        let message = Message::AccessRightsUpdate {
            owner_id,
            image_id,
            requester_id,
            revoked,
            new_view_limit,
        };

        conn.write_message(&message).await?;
        println!("✅ [P2P_ACCESS_UPDATE] Access rights update sent successfully");

        Ok(())
    }
}
