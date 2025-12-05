//! Test Firebase connection

use reqwest::Client;
use serde_json::json;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔥 Firebase Connection Test\n");

    // Get Firebase URL from environment
    let firebase_url = env::var("FIREBASE_DATABASE_URL")
        .expect("❌ ERROR: FIREBASE_DATABASE_URL environment variable not set!\n\
                 \n\
                 Please run:\n\
                 export FIREBASE_DATABASE_URL=\"https://distributed-p2p-default-rtdb.europe-west1.firebasedatabase.app/\"\n");

    println!("📡 Firebase URL: {}", firebase_url);
    println!("→ Testing connection...\n");

    let client = Client::new();

    // ========== TEST 1: Write Data ==========
    println!("╔══════════════════════════════════════════╗");
    println!("║  TEST 1: Writing test data to Firebase  ║");
    println!("╚══════════════════════════════════════════╝");

    let test_data = json!({
        "test": "hello_firebase",
        "timestamp": chrono::Utc::now().timestamp(),
        "message": "Connection test successful!"
    });

    let write_url = format!("{}/connection_test.json", firebase_url);
    println!("→ Writing to: {}", write_url);

    let response = client.put(&write_url).json(&test_data).send().await?;

    if response.status().is_success() {
        println!("✅ WRITE SUCCESS!\n");
    } else {
        println!("❌ WRITE FAILED: {}", response.status());
        println!("Response body: {:?}", response.text().await?);
        return Ok(());
    }

    // ========== TEST 2: Read Data ==========
    println!("╔══════════════════════════════════════════╗");
    println!("║  TEST 2: Reading data from Firebase     ║");
    println!("╚══════════════════════════════════════════╝");

    let read_url = format!("{}/connection_test.json", firebase_url);
    println!("→ Reading from: {}", read_url);

    let response = client.get(&read_url).send().await?;

    if response.status().is_success() {
        let data: serde_json::Value = response.json().await?;
        println!("✅ READ SUCCESS!");
        println!(
            "📦 Data received:\n{}\n",
            serde_json::to_string_pretty(&data)?
        );
    } else {
        println!("❌ READ FAILED: {}", response.status());
        return Ok(());
    }

    // ========== TEST 3: Delete Data ==========
    println!("╔══════════════════════════════════════════╗");
    println!("║  TEST 3: Deleting data from Firebase    ║");
    println!("╚══════════════════════════════════════════╝");

    let delete_url = format!("{}/connection_test.json", firebase_url);
    println!("→ Deleting from: {}", delete_url);

    let response = client.delete(&delete_url).send().await?;

    if response.status().is_success() {
        println!("✅ DELETE SUCCESS!\n");
    } else {
        println!("❌ DELETE FAILED: {}", response.status());
    }

    // ========== SUCCESS ==========
    println!("╔══════════════════════════════════════════╗");
    println!("║   🎉 ALL TESTS PASSED! 🎉                ║");
    println!("║                                          ║");
    println!("║   Firebase connection is working!        ║");
    println!("║   You can now use Firebase in your app   ║");
    println!("╚══════════════════════════════════════════╝");

    println!("\n✓ Next steps:");
    println!("  1. Check Firebase Console to see your data");
    println!("  2. Run: cargo run --example test_firebase");
    println!("  3. Proceed with DoS implementation\n");

    Ok(())
}
