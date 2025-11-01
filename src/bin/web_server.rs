//! Web server for image steganography API

use axum::{
    extract::{multipart::Multipart, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use base64::{engine::general_purpose, Engine as _};
use log::{error, info};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

// Import your existing client middleware
use cloud_p2p::client::client::ClientCore;
use cloud_p2p::client::middleware::{ClientConfig, ClientMiddleware};

#[derive(Serialize)]
struct EncryptResponse {
    success: bool,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    carrier_image_base64: Option<String>,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

struct AppState {
    client: Arc<Mutex<ClientMiddleware>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    info!("🚀 Initializing web server...");

    // Load client configuration
    let config = ClientConfig::from_file("config/client1.toml")?;

    // Create client core
    let core = Arc::new(ClientCore::new(config.client.name.clone()));

    // Create client middleware
    let client = ClientMiddleware::new(config, core);

    let state = Arc::new(AppState {
        client: Arc::new(Mutex::new(client)),
    });

    // Build router
    let app = Router::new()
        .route("/api/encrypt", post(encrypt_image_handler))
        .route("/api/health", get(health_check))
        .nest_service("/", ServeDir::new("frontend/build"))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = "127.0.0.1:3000";
    info!("🌐 Web server running on http://{}", addr);
    info!("📡 API endpoint: http://{}/api/encrypt", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "service": "steganography-api",
        "encryption": "server-side",
        "decryption": "client-side"
    }))
}

async fn encrypt_image_handler(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    let mut secret_image_data: Option<Vec<u8>> = None;
    let mut filename = String::from("uploaded_image.jpg");

    // Parse multipart form data
    while let Some(field) = multipart.next_field().await.map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Failed to read multipart data: {}", e),
            }),
        )
    })? {
        let name = field.name().unwrap_or("").to_string();

        if name == "image" {
            filename = field.file_name().unwrap_or("image.jpg").to_string();
            let data = field.bytes().await.map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: format!("Failed to read image data: {}", e),
                    }),
                )
            })?;
            secret_image_data = Some(data.to_vec());
        }
    }

    let secret_image_data = secret_image_data.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "No image provided".to_string(),
            }),
        )
    })?;

    info!(
        "📤 Received secret image: {} ({} bytes)",
        filename,
        secret_image_data.len()
    );

    let request_id = rand::random::<u64>();

    // Submit to distributed system for encryption
    let mut client = state.client.lock().await;
    match client.submit_task(request_id, secret_image_data).await {
        Ok(carrier_image_with_secret) => {
            info!(
                "✅ Encryption complete! Carrier size: {} bytes",
                carrier_image_with_secret.len()
            );

            let carrier_base64 = general_purpose::STANDARD.encode(&carrier_image_with_secret);

            Ok((
                StatusCode::OK,
                Json(EncryptResponse {
                    success: true,
                    message: format!("Successfully encrypted {}", filename),
                    carrier_image_base64: Some(carrier_base64),
                }),
            ))
        }
        Err(e) => {
            error!("❌ Encryption failed: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Server-side encryption failed: {}", e),
                }),
            ))
        }
    }
}
