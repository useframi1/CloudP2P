//! # Firebase Population Script
//!
//! This script populates Firebase Realtime Database with initial client records
//! based on existing client configurations.

use anyhow::{Context, Result};
use cloud_p2p::common::messages::{ClientInfo, ClientStatus, ImageInfo};
use cloud_p2p::dos::FirebaseClient;
use std::collections::HashMap;
use std::env;

#[tokio::main]
async fn main() -> Result<()> {
    println!("========================================");
    println!("  Firebase Population Script");
    println!("========================================\n");

    // Get Firebase URL from environment
    let firebase_url = env::var("FIREBASE_DATABASE_URL")
        .context("FIREBASE_DATABASE_URL environment variable not set")?;

    println!("Firebase URL: {}", firebase_url);
    println!("Initializing Firebase client...\n");

    let firebase = FirebaseClient::new(firebase_url);

    // Create sample client records
    let clients = vec![
        create_sample_client("Client1", "10.40.39.41", ClientStatus::Offline),
        create_sample_client("Client2", "10.40.38.47", ClientStatus::Offline),
        create_sample_client("Client3", "10.40.51.153", ClientStatus::Offline),
        create_sample_client("Client4", "127.0.0.1", ClientStatus::Offline),
        create_sample_client("Client5", "127.0.0.1", ClientStatus::Offline),
    ];

    println!("Creating {} client records...\n", clients.len());

    for client in &clients {
        println!("Creating client: {}", client.client_id);
        println!("  Name: {}", client.client_id);
        println!("  IP: {}", client.ip_address);
        println!("  Status: {:?}", client.status);
        println!("  Images: {}", client.images.len());

        firebase
            .create_client(client)
            .await
            .context(format!("Failed to create client {}", client.client_id))?;

        println!("  ✓ Created successfully\n");
    }

    println!("========================================");
    println!("  ✓ Population Complete!");
    println!("========================================");
    println!("\nCreated {} clients", clients.len());
    println!("\nNext steps:");
    println!("1. Start the DoS server: cargo run --bin dos_server");
    println!("2. Check Firebase Console to verify data");
    println!("3. Run test_dos example: cargo run --example test_dos\n");

    Ok(())
}

/// Create a sample client with dummy data
fn create_sample_client(name: &str, ip: &str, status: ClientStatus) -> ClientInfo {
    let client_id = format!("client_{}", name.to_lowercase());

    // Create some sample images for each client
    let mut images = HashMap::new();

    // Each client has 2-3 sample images
    for i in 1..=2 {
        let image_id = format!("img_{}_{}", name.to_lowercase(), i);
        let image = ImageInfo {
            image_id: image_id.clone(),
            name: format!("image_{}.jpg", i),
            access_rights: Vec::new(), // Initially no access granted
            encrypted_path: format!("/encrypted/{}/{}", client_id, image_id),
        };
        images.insert(image_id, image);
    }

    ClientInfo {
        client_id,
        status,
        ip_address: ip.to_string(),
        last_seen: current_timestamp(),
        images,
        peers: Vec::new(),
    }
}

/// Get current Unix timestamp
fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
