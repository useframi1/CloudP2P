use cloud_p2p_image_sharing::client::client::Client;
use cloud_p2p_image_sharing::utils::logging;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    logging::init_logger();
    
    let client = Client::new(
        "alice".to_string(),
        "alice".to_string(),
        vec![
            "127.0.0.1:8001".to_string(),
            "127.0.0.1:8002".to_string(),
            "127.0.0.1:8003".to_string(),
        ],
    );
    
    // Send test request
    let test_image = vec![1, 2, 3, 4]; // Dummy image data
    client.send_encryption_request(test_image).await?;
    
    // Keep client running
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    
    Ok(())
}