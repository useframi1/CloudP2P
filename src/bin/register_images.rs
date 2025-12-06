//! Register images from local folders to Firebase for each client

use cloud_p2p::client::dos_client::DosClient;
use cloud_p2p::common::messages::ImageInfo;
use std::fs;
use std::path::Path;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let dos_address = "127.0.0.1:9000".to_string();

    // Register images for Client1
    println!("Registering images for client1...");
    register_client_images(
        dos_address.clone(),
        "client1",
        "test_images/client1",
    )
    .await?;

    // Register images for Client2
    println!("Registering images for client2...");
    register_client_images(
        dos_address.clone(),
        "client2",
        "test_images/client2",
    )
    .await?;

    // Register images for Client3
    println!("Registering images for client3...");
    register_client_images(
        dos_address.clone(),
        "client3",
        "test_images/client3",
    )
    .await?;

    println!("All images registered successfully!");
    Ok(())
}

async fn register_client_images(
    dos_address: String,
    client_id: &str,
    image_dir: &str,
) -> anyhow::Result<()> {
    let mut dos_client = DosClient::new(dos_address, client_id.to_string());

    // Sign in to the client
    match dos_client
        .sign_in(client_id.to_string(), "127.0.0.1".to_string())
        .await
    {
        Ok(_) => println!("  {} signed in successfully", client_id),
        Err(_e) => {
            println!("  {} not found, signing up...", client_id);
            dos_client
                .sign_up(client_id.to_string(), "127.0.0.1".to_string())
                .await?;
        }
    }

    // Read all image files from the directory
    let path = Path::new(image_dir);
    if !path.exists() {
        println!("  Warning: {} does not exist", image_dir);
        return Ok(());
    }

    let entries = fs::read_dir(path)?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            if let Some(extension) = path.extension() {
                let ext = extension.to_string_lossy().to_lowercase();
                if ext == "jpg" || ext == "jpeg" || ext == "png" {
                    let filename = path
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .to_string();

                    let image_id = filename.replace(".", "_");

                    let image_info = ImageInfo {
                        image_id: image_id.clone(),
                        name: filename.clone(),
                        access_rights: std::collections::HashMap::new(),
                        encrypted_path: path.to_string_lossy().to_string(),
                    };

                    dos_client.register_image(image_info).await?;
                    println!("  Registered: {}", filename);
                }
            }
        }
    }

    dos_client.sign_out().await?;
    println!("  {} signed out", client_id);

    Ok(())
}
