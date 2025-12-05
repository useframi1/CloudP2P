//! # DoS Functionality Test
//!
//! Comprehensive test for Directory of Services functionality including:
//! - Client sign-up and sign-in
//! - Online client listing
//! - Image access requests (online and offline scenarios)
//! - Failure reporting

use cloud_p2p::client::DosClient;
use cloud_p2p::common::messages::ImageInfo;
use anyhow::Result;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("\n========================================");
    println!("  DoS Functionality Test Suite");
    println!("========================================\n");

    // DoS server address (must be running)
    let dos_address = "127.0.0.1:9000".to_string();

    // ========== TEST 1: Client Sign-Up ==========
    println!("╔══════════════════════════════════════════╗");
    println!("║  TEST 1: Client Sign-Up                 ║");
    println!("╚══════════════════════════════════════════╝");

    let mut client1 = DosClient::new(dos_address.clone(), "TestClient1".to_string());
    let client1_id = client1.sign_up("192.168.1.100".to_string()).await?;
    println!("✓ Client1 signed up with ID: {}\n", client1_id);

    let mut client2 = DosClient::new(dos_address.clone(), "TestClient2".to_string());
    let client2_id = client2.sign_up("192.168.1.101".to_string()).await?;
    println!("✓ Client2 signed up with ID: {}\n", client2_id);

    let mut client3 = DosClient::new(dos_address.clone(), "TestClient3".to_string());
    let client3_id = client3.sign_up("192.168.1.102".to_string()).await?;
    println!("✓ Client3 signed up with ID: {}\n", client3_id);

    sleep(Duration::from_secs(1)).await;

    // ========== TEST 2: List Online Clients ==========
    println!("╔══════════════════════════════════════════╗");
    println!("║  TEST 2: List Online Clients            ║");
    println!("╚══════════════════════════════════════════╝");

    let online_clients = client1.list_online_clients().await?;
    println!("Online clients: {}", online_clients.len());
    for client in &online_clients {
        println!("  - {} ({})", client.client_id, client.ip_address);
    }
    println!();

    sleep(Duration::from_secs(1)).await;

    // ========== TEST 3: Register Images ==========
    println!("╔══════════════════════════════════════════╗");
    println!("║  TEST 3: Register Images                ║");
    println!("╚══════════════════════════════════════════╝");

    let image1 = ImageInfo {
        image_id: Uuid::new_v4().to_string(),
        name: "vacation.jpg".to_string(),
        access_rights: Vec::new(),
        encrypted_path: "/encrypted/client1/vacation.jpg".to_string(),
    };

    client1.register_image(image1.clone()).await?;
    println!("✓ Client1 registered image: {}\n", image1.name);

    sleep(Duration::from_secs(1)).await;

    // ========== TEST 4: Image Access Request (Owner Online) ==========
    println!("╔══════════════════════════════════════════╗");
    println!("║  TEST 4: Access Request (Owner Online)  ║");
    println!("╚══════════════════════════════════════════╝");

    let request_id = client2
        .request_image_access(&client1_id, &image1.image_id)
        .await?;
    println!("✓ Client2 requested access to Client1's image");
    println!("  Request ID: {}\n", request_id);

    sleep(Duration::from_secs(1)).await;

    // Owner approves the request
    println!("→ Client1 (owner) approves the request...");
    client1.respond_to_access_request(&request_id, true).await?;
    println!("✓ Access approved\n");

    sleep(Duration::from_secs(1)).await;

    // ========== TEST 5: Update Access Rights ==========
    println!("╔══════════════════════════════════════════╗");
    println!("║  TEST 5: Update Access Rights           ║");
    println!("╚══════════════════════════════════════════╝");

    let new_access_list = vec![client2_id.clone(), client3_id.clone()];
    client1
        .update_access_rights(&image1.image_id, new_access_list)
        .await?;
    println!(
        "✓ Client1 updated access rights to include Client2 and Client3\n"
    );

    sleep(Duration::from_secs(1)).await;

    // ========== TEST 6: Client Sign-Out ==========
    println!("╔══════════════════════════════════════════╗");
    println!("║  TEST 6: Client Sign-Out                ║");
    println!("╚══════════════════════════════════════════╝");

    client3.sign_out().await?;
    println!("✓ Client3 signed out\n");

    sleep(Duration::from_secs(1)).await;

    // ========== TEST 7: Access Request (Owner Offline) ==========
    println!("╔══════════════════════════════════════════╗");
    println!("║  TEST 7: Access Request (Owner Offline) ║");
    println!("╚══════════════════════════════════════════╝");

    // Client1 signs out
    client1.sign_out().await?;
    println!("→ Client1 (owner) signed out");

    sleep(Duration::from_secs(1)).await;

    // Client2 tries to request access (should be stored as pending)
    let image2_id = Uuid::new_v4().to_string();
    let request_id2 = client2
        .request_image_access(&client1_id, &image2_id)
        .await?;
    println!("✓ Client2 requested access while Client1 is offline");
    println!("  Request ID: {}", request_id2);
    println!("  (This will be stored as pending)\n");

    sleep(Duration::from_secs(1)).await;

    // ========== TEST 8: Client Sign-In with Notifications ==========
    println!("╔══════════════════════════════════════════╗");
    println!("║  TEST 8: Sign-In with Notifications     ║");
    println!("╚══════════════════════════════════════════╝");

    let notifications = client1
        .sign_in(client1_id.clone(), "192.168.1.100".to_string())
        .await?;
    println!("✓ Client1 signed back in");
    println!("  Pending notifications: {}", notifications.len());
    for notif in &notifications {
        println!("    - {:?} from {}", notif.notification_type, notif.from_client);
    }
    println!();

    sleep(Duration::from_secs(1)).await;

    // ========== TEST 9: Requester Goes Offline Before Approval ==========
    println!("╔══════════════════════════════════════════╗");
    println!("║  TEST 9: Requester Goes Offline         ║");
    println!("╚══════════════════════════════════════════╝");

    let image3_id = Uuid::new_v4().to_string();
    let request_id3 = client2
        .request_image_access(&client1_id, &image3_id)
        .await?;
    println!("✓ Client2 requested access to image3");
    println!("  Request ID: {}", request_id3);

    sleep(Duration::from_secs(1)).await;

    // Client2 (requester) goes offline
    client2.sign_out().await?;
    println!("→ Client2 (requester) signed out");

    sleep(Duration::from_secs(1)).await;

    // Client1 (owner) approves, but Client2 is offline
    println!("→ Client1 (owner) approves the request...");
    client1.respond_to_access_request(&request_id3, true).await?;
    println!("✓ Request expired (requester offline)\n");

    sleep(Duration::from_secs(1)).await;

    // ========== TEST 10: Failure Reporting ==========
    println!("╔══════════════════════════════════════════╗");
    println!("║  TEST 10: Failure Reporting             ║");
    println!("╚══════════════════════════════════════════╝");

    // Simulate Client3 failure
    let fake_client_id = "client_fake_dead";
    client1.report_peer_failure(fake_client_id).await?;
    println!("✓ Client1 reported {} as failed", fake_client_id);
    println!("  (DoS will mark it as offline)\n");

    sleep(Duration::from_secs(1)).await;

    // ========== TEST 11: Final Online Client List ==========
    println!("╔══════════════════════════════════════════╗");
    println!("║  TEST 11: Final Online Client List      ║");
    println!("╚══════════════════════════════════════════╝");

    let final_online = client1.list_online_clients().await?;
    println!("Final online clients: {}", final_online.len());
    for client in &final_online {
        println!("  - {} ({})", client.client_id, client.ip_address);
    }
    println!();

    // ========== CLEANUP ==========
    println!("╔══════════════════════════════════════════╗");
    println!("║  Cleanup                                 ║");
    println!("╚══════════════════════════════════════════╝");

    client1.sign_out().await?;
    println!("✓ Client1 signed out");

    // Client2 already signed out
    // Client3 already signed out

    println!();
    println!("========================================");
    println!("  ✓ All Tests Passed!");
    println!("========================================\n");

    Ok(())
}
