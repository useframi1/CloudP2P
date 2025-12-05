//! Web server for image steganography API and DoS integration

use axum::{
    extract::{multipart::Multipart, ConnectInfo, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use base64::{engine::general_purpose, Engine as _};
use clap::Parser;
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

use cloud_p2p::client::client::ClientCore;
use cloud_p2p::client::dos_client::DosClient;
use cloud_p2p::client::middleware::{ClientConfig, ClientMiddleware};
use cloud_p2p::common::messages::{AccessRight, ImageInfo};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to client config file
    #[arg(long, default_value = "config/client1.toml")]
    config: String,

    /// Port to bind to
    #[arg(long, default_value_t = 3000)]
    port: u16,

    /// Client ID for this web server
    #[arg(long)]
    client_id: Option<String>,
}

#[derive(Serialize)]
struct ApiResponse {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    carrier_image_base64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    client_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    notifications: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    peers: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    images: Option<Vec<ImageInfo>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    requests: Option<Vec<serde_json::Value>>,
}

#[derive(Deserialize)]
struct SignUpRequest {
    client_id: String,
}

#[derive(Deserialize)]
struct SignInRequest {
    client_id: String,
}

#[derive(Deserialize)]
struct SignOutRequest {
    client_id: String,
}

#[derive(Deserialize)]
struct RequestAccessRequest {
    owner_id: String,
    image_id: String,
}

#[derive(Deserialize)]
struct AccessRightEntry {
    client_id: String,
    view_limit: u32,
}

#[derive(Deserialize)]
struct UpdateAccessRequest {
    image_id: String,
    access_list: Vec<AccessRightEntry>,
}

#[derive(Deserialize)]
struct RespondRequestRequest {
    request_id: String,
    approved: bool,
}

struct AppState {
    client: Arc<Mutex<ClientMiddleware>>,
    dos_client: Arc<Mutex<DosClient>>,
    current_user: Arc<Mutex<Option<String>>>,
    image_dir: String,
}

async fn register_local_images(dos_client: &DosClient, _client_id: &str, image_dir: &str) -> anyhow::Result<()> {
    let path = std::path::Path::new(image_dir);
    if !path.exists() {
        info!("Image directory {} does not exist, skipping auto-registration", image_dir);
        return Ok(());
    }

    let entries = match std::fs::read_dir(path) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
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

                    match dos_client.register_image(image_info).await {
                        Ok(_) => info!("Auto-registered image: {}", filename),
                        Err(e) => error!("Failed to register {}: {}", filename, e),
                    }
                }
            }
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let args = Args::parse();

    info!("🚀 Initializing web server on port {}...", args.port);

    // Load client configuration
    let config = ClientConfig::from_file(&args.config)?;

    // Create client core
    let core = Arc::new(ClientCore::new(config.client.name.clone()));

    // Create client middleware
    let client = ClientMiddleware::new(config.clone(), core);

    // Create DoS client
    let dos_client = DosClient::new(
        config.client.dos_address.clone(),
        config.client.name.clone(),
    );

    let state = Arc::new(AppState {
        client: Arc::new(Mutex::new(client)),
        dos_client: Arc::new(Mutex::new(dos_client)),
        current_user: Arc::new(Mutex::new(args.client_id)),
        image_dir: config.client.image_dir.clone(),
    });

    // Build router
    let app = Router::new()
        .route("/api/encrypt", post(encrypt_image_handler))
        .route("/api/signup", post(signup_handler))
        .route("/api/signin", post(signin_handler))
        .route("/api/signout", post(signout_handler))
        .route("/api/online-peers", get(online_peers_handler))
        .route("/api/request-access", post(request_access_handler))
        .route("/api/my-images", get(my_images_handler))
        .route("/api/update-access", post(update_access_handler))
        .route("/api/pending-requests", get(pending_requests_handler))
        .route("/api/respond-request", post(respond_request_handler))
        .route("/api/requested-images", get(requested_images_handler))
        .route("/api/health", get(health_check))
        .nest_service("/test_images", ServeDir::new("test_images"))
        .nest_service("/", ServeDir::new("frontend/build"))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = format!("0.0.0.0:{}", args.port);
    info!("🌐 Web server running on http://{}", addr);
    info!("📡 API endpoints ready");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}

async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "service": "cloudp2p-web-api",
        "features": ["steganography", "dos", "p2p-sharing"]
    }))
}

// Get client IP address
fn get_client_ip(connect_info: Option<ConnectInfo<SocketAddr>>) -> String {
    connect_info
        .map(|ci| ci.0.ip().to_string())
        .unwrap_or_else(|| "127.0.0.1".to_string())
}

async fn signup_handler(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(payload): Json<SignUpRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiResponse>)> {
    let ip_address = addr.ip().to_string();
    info!("Sign up request for {} from {}", payload.client_id, ip_address);

    let mut dos_client = state.dos_client.lock().await;

    // Use the client_id provided by the user
    match dos_client.sign_up(payload.client_id.clone(), ip_address).await {
        Ok(client_id) => {
            *state.current_user.lock().await = Some(client_id.clone());
            info!("Sign up successful: {}", client_id);

            // Auto-register images from local folder
            let image_dir = state.image_dir.clone();
            if let Err(e) = register_local_images(&*dos_client, &client_id, &image_dir).await {
                error!("Failed to auto-register images: {}", e);
            }

            Ok((
                StatusCode::OK,
                Json(ApiResponse {
                    success: true,
                    message: Some("Sign up successful".to_string()),
                    client_id: Some(client_id),
                    error: None,
                    carrier_image_base64: None,
                    notifications: None,
                    request_id: None,
                    peers: None,
                    images: None,
                    requests: None,
                }),
            ))
        }
        Err(e) => {
            error!("Sign up failed: {}", e);
            let error_msg = if e.to_string().contains("already exists") {
                "Client ID already exists. Please sign in instead.".to_string()
            } else {
                format!("Sign up failed: {}", e)
            };
            Err((
                StatusCode::BAD_REQUEST,
                Json(ApiResponse {
                    success: false,
                    error: Some(error_msg),
                    message: None,
                    carrier_image_base64: None,
                    client_id: None,
                    notifications: None,
                    request_id: None,
                    peers: None,
                    images: None,
                    requests: None,
                }),
            ))
        }
    }
}

async fn signin_handler(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(payload): Json<SignInRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiResponse>)> {
    let ip_address = addr.ip().to_string();
    info!("Sign in request for {} from {}", payload.client_id, ip_address);

    let mut dos_client = state.dos_client.lock().await;

    match dos_client
        .sign_in(payload.client_id.clone(), ip_address)
        .await
    {
        Ok(notifications) => {
            *state.current_user.lock().await = Some(payload.client_id.clone());
            info!(
                "Sign in successful: {} ({} notifications)",
                payload.client_id,
                notifications.len()
            );

            // Auto-register images from local folder
            let image_dir = state.image_dir.clone();
            let client_id = payload.client_id.clone();
            if let Err(e) = register_local_images(&*dos_client, &client_id, &image_dir).await {
                error!("Failed to auto-register images: {}", e);
            }

            Ok((
                StatusCode::OK,
                Json(ApiResponse {
                    success: true,
                    message: Some("Sign in successful".to_string()),
                    notifications: Some(notifications),
                    error: None,
                    carrier_image_base64: None,
                    client_id: None,
                    request_id: None,
                    peers: None,
                    images: None,
                    requests: None,
                }),
            ))
        }
        Err(e) => {
            error!("Sign in failed: {}", e);
            let error_msg = if e.to_string().contains("Client not found") {
                "Client ID not found. Please sign up first.".to_string()
            } else {
                format!("Sign in failed: {}", e)
            };
            Err((
                StatusCode::UNAUTHORIZED,
                Json(ApiResponse {
                    success: false,
                    error: Some(error_msg),
                    message: None,
                    carrier_image_base64: None,
                    client_id: None,
                    notifications: None,
                    request_id: None,
                    peers: None,
                    images: None,
                    requests: None,
                }),
            ))
        }
    }
}

async fn signout_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SignOutRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiResponse>)> {
    info!("Sign out request for {}", payload.client_id);

    let mut dos_client = state.dos_client.lock().await;

    match dos_client.sign_out().await {
        Ok(_) => {
            *state.current_user.lock().await = None;
            info!("Sign out successful: {}", payload.client_id);
            Ok((
                StatusCode::OK,
                Json(ApiResponse {
                    success: true,
                    message: Some("Sign out successful".to_string()),
                    error: None,
                    carrier_image_base64: None,
                    client_id: None,
                    notifications: None,
                    request_id: None,
                    peers: None,
                    images: None,
                    requests: None,
                }),
            ))
        }
        Err(e) => {
            error!("Sign out failed: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse {
                    success: false,
                    error: Some(format!("Sign out failed: {}", e)),
                    message: None,
                    carrier_image_base64: None,
                    client_id: None,
                    notifications: None,
                    request_id: None,
                    peers: None,
                    images: None,
                    requests: None,
                }),
            ))
        }
    }
}

async fn online_peers_handler(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiResponse>)> {
    info!("Fetching online peers");

    let dos_client = state.dos_client.lock().await;

    match dos_client.list_online_clients().await {
        Ok(clients) => {
            let peers: Vec<serde_json::Value> = clients
                .into_iter()
                .map(|c| serde_json::to_value(c).unwrap())
                .collect();

            info!("Found {} online peers", peers.len());
            Ok((
                StatusCode::OK,
                Json(ApiResponse {
                    success: true,
                    peers: Some(peers),
                    message: None,
                    error: None,
                    carrier_image_base64: None,
                    client_id: None,
                    notifications: None,
                    request_id: None,
                    images: None,
                    requests: None,
                }),
            ))
        }
        Err(e) => {
            error!("Failed to fetch online peers: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse {
                    success: false,
                    error: Some(format!("Failed to fetch peers: {}", e)),
                    message: None,
                    carrier_image_base64: None,
                    client_id: None,
                    notifications: None,
                    request_id: None,
                    peers: None,
                    images: None,
                    requests: None,
                }),
            ))
        }
    }
}

async fn request_access_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<RequestAccessRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiResponse>)> {
    info!(
        "Request access to image {} from {}",
        payload.image_id, payload.owner_id
    );

    // Check if user is logged in
    let current_user = state.current_user.lock().await;
    if current_user.is_none() {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse {
                success: false,
                error: Some("Not signed in".to_string()),
                message: None,
                carrier_image_base64: None,
                client_id: None,
                notifications: None,
                request_id: None,
                peers: None,
                images: None,
                requests: None,
            }),
        ));
    }
    let requester_id = current_user.as_ref().unwrap().clone();
    drop(current_user); // Release the lock

    let mut dos_client = state.dos_client.lock().await;

    // Ensure dos_client is signed in with the correct client_id
    if dos_client.client_id() != Some(&requester_id) {
        info!(
            "DOS client client_id mismatch (expected: {}, got: {:?}). Re-signing in...",
            requester_id,
            dos_client.client_id()
        );
        // Re-sign in with the correct client_id
        match dos_client
            .sign_in(requester_id.clone(), "127.0.0.1".to_string())
            .await
        {
            Ok(_) => info!("Re-signed in successfully as {}", requester_id),
            Err(e) => {
                error!("Failed to re-sign in: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiResponse {
                        success: false,
                        error: Some(format!("Failed to authenticate: {}", e)),
                        message: None,
                        carrier_image_base64: None,
                        client_id: None,
                        notifications: None,
                        request_id: None,
                        peers: None,
                        images: None,
                        requests: None,
                    }),
                ));
            }
        }
    }

    match dos_client
        .request_image_access(&payload.owner_id, &payload.image_id)
        .await
    {
        Ok(request_id) => {
            info!("Access request created: {}", request_id);
            Ok((
                StatusCode::OK,
                Json(ApiResponse {
                    success: true,
                    message: Some("Access request sent".to_string()),
                    request_id: Some(request_id),
                    error: None,
                    carrier_image_base64: None,
                    client_id: None,
                    notifications: None,
                    peers: None,
                    images: None,
                    requests: None,
                }),
            ))
        }
        Err(e) => {
            error!("Failed to request access: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse {
                    success: false,
                    error: Some(format!("Failed to request access: {}", e)),
                    message: None,
                    carrier_image_base64: None,
                    client_id: None,
                    notifications: None,
                    request_id: None,
                    peers: None,
                    images: None,
                    requests: None,
                }),
            ))
        }
    }
}

async fn my_images_handler(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiResponse>)> {
    info!("Fetching my images");

    let current_user = state.current_user.lock().await;
    if current_user.is_none() {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse {
                success: false,
                error: Some("Not signed in".to_string()),
                message: None,
                carrier_image_base64: None,
                client_id: None,
                notifications: None,
                request_id: None,
                peers: None,
                images: None,
                requests: None,
            }),
        ));
    }

    let client_id = current_user.as_ref().unwrap();
    let dos_client = state.dos_client.lock().await;

    // Get client info from DoS
    match dos_client.list_online_clients().await {
        Ok(clients) => {
            let my_client = clients.iter().find(|c| &c.client_id == client_id);

            if let Some(client) = my_client {
                let images: Vec<ImageInfo> = client.images.values().cloned().collect();
                info!("Found {} images", images.len());
                Ok((
                    StatusCode::OK,
                    Json(ApiResponse {
                        success: true,
                        images: Some(images),
                        message: None,
                        error: None,
                        carrier_image_base64: None,
                        client_id: None,
                        notifications: None,
                        request_id: None,
                        peers: None,
                        requests: None,
                    }),
                ))
            } else {
                Ok((
                    StatusCode::OK,
                    Json(ApiResponse {
                        success: true,
                        images: Some(vec![]),
                        message: None,
                        error: None,
                        carrier_image_base64: None,
                        client_id: None,
                        notifications: None,
                        request_id: None,
                        peers: None,
                        requests: None,
                    }),
                ))
            }
        }
        Err(e) => {
            error!("Failed to fetch images: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse {
                    success: false,
                    error: Some(format!("Failed to fetch images: {}", e)),
                    message: None,
                    carrier_image_base64: None,
                    client_id: None,
                    notifications: None,
                    request_id: None,
                    peers: None,
                    images: None,
                    requests: None,
                }),
            ))
        }
    }
}

async fn update_access_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<UpdateAccessRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiResponse>)> {
    info!("Updating access rights for image {}", payload.image_id);

    // Check if user is logged in
    let current_user = state.current_user.lock().await;
    if current_user.is_none() {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse {
                success: false,
                error: Some("Not signed in".to_string()),
                message: None,
                carrier_image_base64: None,
                client_id: None,
                notifications: None,
                request_id: None,
                peers: None,
                images: None,
                requests: None,
            }),
        ));
    }
    let client_id = current_user.as_ref().unwrap().clone();
    drop(current_user);

    let mut dos_client = state.dos_client.lock().await;

    // Ensure dos_client is signed in with the correct client_id
    if dos_client.client_id() != Some(&client_id) {
        info!(
            "DOS client client_id mismatch (expected: {}, got: {:?}). Re-signing in...",
            client_id,
            dos_client.client_id()
        );
        match dos_client
            .sign_in(client_id.clone(), "127.0.0.1".to_string())
            .await
        {
            Ok(_) => info!("Re-signed in successfully as {}", client_id),
            Err(e) => {
                error!("Failed to re-sign in: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiResponse {
                        success: false,
                        error: Some(format!("Failed to authenticate: {}", e)),
                        message: None,
                        carrier_image_base64: None,
                        client_id: None,
                        notifications: None,
                        request_id: None,
                        peers: None,
                        images: None,
                        requests: None,
                    }),
                ));
            }
        }
    }

    // Convert access_list to HashMap<String, AccessRight>
    let access_rights: std::collections::HashMap<String, AccessRight> = payload
        .access_list
        .into_iter()
        .map(|entry| {
            (
                entry.client_id,
                AccessRight {
                    view_limit: entry.view_limit,
                    view_count: 0,
                },
            )
        })
        .collect();

    match dos_client
        .update_access_rights(&payload.image_id, access_rights)
        .await
    {
        Ok(_) => {
            info!("Access rights updated successfully");
            Ok((
                StatusCode::OK,
                Json(ApiResponse {
                    success: true,
                    message: Some("Access rights updated".to_string()),
                    error: None,
                    carrier_image_base64: None,
                    client_id: None,
                    notifications: None,
                    request_id: None,
                    peers: None,
                    images: None,
                    requests: None,
                }),
            ))
        }
        Err(e) => {
            error!("Failed to update access rights: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse {
                    success: false,
                    error: Some(format!("Failed to update access rights: {}", e)),
                    message: None,
                    carrier_image_base64: None,
                    client_id: None,
                    notifications: None,
                    request_id: None,
                    peers: None,
                    images: None,
                    requests: None,
                }),
            ))
        }
    }
}

async fn pending_requests_handler(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiResponse>)> {
    info!("Fetching pending requests");

    let current_user = state.current_user.lock().await;
    if current_user.is_none() {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse {
                success: false,
                error: Some("Not signed in".to_string()),
                message: None,
                carrier_image_base64: None,
                client_id: None,
                notifications: None,
                request_id: None,
                peers: None,
                images: None,
                requests: None,
            }),
        ));
    }

    let client_id = current_user.as_ref().unwrap();
    let dos_client = state.dos_client.lock().await;

    match dos_client.get_pending_requests(client_id).await {
        Ok(requests) => {
            info!("Found {} pending requests", requests.len());
            Ok((
                StatusCode::OK,
                Json(ApiResponse {
                    success: true,
                    requests: Some(requests),
                    message: None,
                    error: None,
                    carrier_image_base64: None,
                    client_id: None,
                    notifications: None,
                    request_id: None,
                    peers: None,
                    images: None,
                }),
            ))
        }
        Err(e) => {
            error!("Failed to fetch pending requests: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse {
                    success: false,
                    error: Some(format!("Failed to fetch pending requests: {}", e)),
                    message: None,
                    carrier_image_base64: None,
                    client_id: None,
                    notifications: None,
                    request_id: None,
                    peers: None,
                    images: None,
                    requests: None,
                }),
            ))
        }
    }
}

async fn respond_request_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<RespondRequestRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiResponse>)> {
    info!(
        "Responding to request {} with {}",
        payload.request_id, payload.approved
    );

    let dos_client = state.dos_client.lock().await;

    match dos_client
        .respond_to_access_request(&payload.request_id, payload.approved)
        .await
    {
        Ok(_) => {
            info!("Response sent successfully");
            Ok((
                StatusCode::OK,
                Json(ApiResponse {
                    success: true,
                    message: Some("Response sent".to_string()),
                    error: None,
                    carrier_image_base64: None,
                    client_id: None,
                    notifications: None,
                    request_id: None,
                    peers: None,
                    images: None,
                    requests: None,
                }),
            ))
        }
        Err(e) => {
            error!("Failed to respond to request: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse {
                    success: false,
                    error: Some(format!("Failed to respond: {}", e)),
                    message: None,
                    carrier_image_base64: None,
                    client_id: None,
                    notifications: None,
                    request_id: None,
                    peers: None,
                    images: None,
                    requests: None,
                }),
            ))
        }
    }
}

async fn encrypt_image_handler(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiResponse>)> {
    let mut secret_image_data: Option<Vec<u8>> = None;
    let mut filename = String::from("uploaded_image.jpg");

    // Parse multipart form data
    while let Some(field) = multipart.next_field().await.map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse {
                success: false,
                error: Some(format!("Failed to read multipart data: {}", e)),
                message: None,
                carrier_image_base64: None,
                client_id: None,
                notifications: None,
                request_id: None,
                peers: None,
                images: None,
                requests: None,
            }),
        )
    })? {
        let name = field.name().unwrap_or("").to_string();

        if name == "image" {
            filename = field.file_name().unwrap_or("image.jpg").to_string();
            let data = field.bytes().await.map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(ApiResponse {
                        success: false,
                        error: Some(format!("Failed to read image data: {}", e)),
                        message: None,
                        carrier_image_base64: None,
                        client_id: None,
                        notifications: None,
                        request_id: None,
                        peers: None,
                        images: None,
                        requests: None,
                    }),
                )
            })?;
            secret_image_data = Some(data.to_vec());
        }
    }

    let secret_image_data = secret_image_data.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse {
                success: false,
                error: Some("No image provided".to_string()),
                message: None,
                carrier_image_base64: None,
                client_id: None,
                notifications: None,
                request_id: None,
                peers: None,
                images: None,
                requests: None,
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
                Json(ApiResponse {
                    success: true,
                    message: Some(format!("Successfully encrypted {}", filename)),
                    carrier_image_base64: Some(carrier_base64),
                    error: None,
                    client_id: None,
                    notifications: None,
                    request_id: None,
                    peers: None,
                    images: None,
                    requests: None,
                }),
            ))
        }
        Err(e) => {
            error!("❌ Encryption failed: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse {
                    success: false,
                    error: Some(format!("Server-side encryption failed: {}", e)),
                    message: None,
                    carrier_image_base64: None,
                    client_id: None,
                    notifications: None,
                    request_id: None,
                    peers: None,
                    images: None,
                    requests: None,
                }),
            ))
        }
    }
}

async fn requested_images_handler(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiResponse>)> {
    info!("Fetching requested images");

    let current_user = state.current_user.lock().await;
    if current_user.is_none() {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse {
                success: false,
                error: Some("Not signed in".to_string()),
                message: None,
                carrier_image_base64: None,
                client_id: None,
                notifications: None,
                request_id: None,
                peers: None,
                images: None,
                requests: None,
            }),
        ));
    }

    let client_id = current_user.as_ref().unwrap();
    let dos_client = state.dos_client.lock().await;

    // Return empty list for now - will implement proper tracking later
    Ok((
        StatusCode::OK,
        Json(ApiResponse {
            success: true,
            requests: Some(vec![]),
            message: None,
            error: None,
            carrier_image_base64: None,
            client_id: None,
            notifications: None,
            request_id: None,
            peers: None,
            images: None,
        }),
    ))
}
