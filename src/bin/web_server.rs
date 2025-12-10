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
use serde_json::json;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

use cloud_p2p::client::client::ClientCore;
use cloud_p2p::client::dos_client::DosClient;
use cloud_p2p::client::middleware::{ClientConfig, ClientMiddleware};
use cloud_p2p::client::p2p_service::P2PService;
use cloud_p2p::common::messages::{AccessRight, ImageInfo};
use cloud_p2p::dos::firebase::FirebaseClient;
use cloud_p2p::processing::{
    embed_image_with_access_rights, extract_image_with_access_rights, update_embedded_access_rights, EmbeddedAccessRights,
};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to client config file
    #[arg(long, default_value = "config/client1.toml")]
    config: String,

    /// Port to bind to
    #[arg(long, default_value_t = 3000)]
    port: u16,

    /// P2P port for direct peer connections
    #[arg(long, default_value_t = 7000)]
    p2p_port: u16,

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
    images: Option<Vec<serde_json::Value>>,
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
    req_access_limit: Option<u32>, // How many times the requester wants to view the image
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
    view_limit: Option<u32>, // Owner-set view limit when approving
}

#[derive(Deserialize)]
struct ViewImageRequest {
    owner_id: String,
    image_id: String,
}

#[derive(Deserialize)]
struct ManageAccessRequest {
    image_id: String,
    requester_id: String,
    action: String,              // "revoke" or "modify"
    new_view_limit: Option<u32>, // Only for "modify" action
}

struct AppState {
    client: Arc<Mutex<ClientMiddleware>>,
    dos_client: Arc<Mutex<DosClient>>,
    p2p_service: Arc<P2PService>,
    current_user: Arc<Mutex<Option<String>>>,
    firebase: Arc<FirebaseClient>,
    config: ClientConfig,
}

async fn register_local_images(
    dos_client: &DosClient,
    client_middleware: Arc<Mutex<ClientMiddleware>>,
    client_id: &str,
    image_dir: &str,
) -> anyhow::Result<()> {
    let path = std::path::Path::new(image_dir);
    if !path.exists() {
        info!(
            "Image directory {} does not exist, skipping auto-registration",
            image_dir
        );
        return Ok(());
    }

    // Create encrypted_images directory for this client
    let encrypted_dir = format!("encrypted_images/{}", client_id);
    std::fs::create_dir_all(&encrypted_dir)?;

    let entries = match std::fs::read_dir(path) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };

    // Get list of carrier images
    let carrier_files: Vec<_> = std::fs::read_dir("cover_images")?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().is_file()
                && e.path()
                    .extension()
                    .map(|ext| {
                        let ext = ext.to_string_lossy().to_lowercase();
                        ext == "jpg" || ext == "jpeg" || ext == "png"
                    })
                    .unwrap_or(false)
        })
        .collect();

    if carrier_files.is_empty() {
        error!("No carrier images found in cover_images/ folder. Please add carrier images.");
        return Ok(());
    }

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let secret_path = entry.path();

        if secret_path.is_file() {
            if let Some(extension) = secret_path.extension() {
                let ext = extension.to_string_lossy().to_lowercase();
                if ext == "jpg" || ext == "jpeg" || ext == "png" {
                    let filename = secret_path
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .to_string();

                    let image_id = filename.replace(".", "_");

                    // Read secret image
                    let secret_bytes = match std::fs::read(&secret_path) {
                        Ok(b) => b,
                        Err(e) => {
                            error!("Failed to read secret image {}: {}", filename, e);
                            continue;
                        }
                    };

                    // Generate unique request ID for this encryption
                    let request_id = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64;

                    // Send secret image to compute server for encryption (no access rights at registration)
                    info!(
                        "🔐 Sending {} to compute server for encryption...",
                        filename
                    );
                    let encrypted_carrier = match client_middleware
                        .lock()
                        .await
                        .submit_encryption_with_access_rights(
                            request_id,
                            secret_bytes.clone(),
                            None, // No access rights yet - will be added per requester on approval
                        )
                        .await
                    {
                        Ok(c) => c,
                        Err(e) => {
                            error!("Failed to encrypt {} on server: {}", filename, e);
                            continue;
                        }
                    };

                    info!("✅ Server encrypted {} successfully", filename);

                    // Save encrypted carrier
                    let encrypted_path = format!("{}/{}", encrypted_dir, filename);
                    if let Err(e) = std::fs::write(&encrypted_path, &encrypted_carrier) {
                        error!("Failed to save encrypted carrier: {}", e);
                        continue;
                    }

                    info!(
                        "✅ Encrypted {} into carrier, saved to {}",
                        filename, encrypted_path
                    );

                    let image_info = ImageInfo {
                        image_id: image_id.clone(),
                        name: filename.clone(),
                        encrypted_path: encrypted_path.clone(),
                        access_rights: std::collections::HashMap::new(),
                        personalized_carriers: std::collections::HashMap::new(),
                    };

                    match dos_client.register_image(image_info).await {
                        Ok(_) => info!("📝 Registered image in DoS: {}", filename),
                        Err(e) => error!("Failed to register {}: {}", filename, e),
                    }
                }
            }
        }
    }

    Ok(())
}

async fn register_images_handler(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiResponse>)> {
    info!("📝 Register images request");

    // Get current user
    let current_user = state.current_user.lock().await.clone();
    let client_id = match current_user {
        Some(id) => id,
        None => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(ApiResponse {
                    success: false,
                    message: None,
                    error: Some("Not signed in".to_string()),
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
    };

    let dos_client = state.dos_client.lock().await;
    // Dynamically construct image directory based on actual client_id
    let image_dir = format!("test_images/{}", client_id);

    // Register images with server-based encryption
    match register_local_images(&*dos_client, state.client.clone(), &client_id, &image_dir).await {
        Ok(_) => {
            info!("✅ Images registered successfully for {}", client_id);
            Ok((
                StatusCode::OK,
                Json(ApiResponse {
                    success: true,
                    message: Some("Images registered successfully".to_string()),
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
            error!("❌ Failed to register images: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse {
                    success: false,
                    message: None,
                    error: Some(format!("Failed to register images: {}", e)),
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

    // Create DoS client - connects to leader server instead of separate DoS
    let dos_client = DosClient::new(
        config.client.server_addresses.clone(),
        config.client.name.clone(),
    );

    // Initialize Firebase client
    let firebase_url =
        "https://distributed-p2p-default-rtdb.europe-west1.firebasedatabase.app/".to_string();
    let firebase = Arc::new(FirebaseClient::new(firebase_url));

    let dos_client_arc = Arc::new(Mutex::new(dos_client));

    // Create P2P service
    // IMPORTANT: Use lowercase client_id to match Firebase convention (client3 not Client3)
    let p2p_client_id = args
        .client_id
        .clone()
        .unwrap_or_else(|| config.client.name.to_lowercase());

    // Wrap client middleware in Arc<Mutex<>> for sharing
    let client_middleware_arc = Arc::new(Mutex::new(client));

    let p2p_service = Arc::new(P2PService::new(
        args.p2p_port,
        p2p_client_id,
        dos_client_arc.clone(),
        firebase.clone(),
        config.client.image_dir.clone(),
        client_middleware_arc.clone(),
    ));

    let state = Arc::new(AppState {
        client: client_middleware_arc,
        dos_client: dos_client_arc,
        p2p_service: p2p_service.clone(),
        current_user: Arc::new(Mutex::new(args.client_id)),
        firebase,
        config: config.clone(),
    });

    // Build router
    let app = Router::new()
        .route("/api/encrypt", post(encrypt_image_handler))
        .route("/api/signup", post(signup_handler))
        .route("/api/signin", post(signin_handler))
        .route("/api/signout", post(signout_handler))
        .route("/api/register-images", post(register_images_handler))
        .route("/api/online-peers", get(online_peers_handler))
        .route("/api/request-access", post(request_access_handler))
        .route("/api/my-images", get(my_images_handler))
        .route("/api/update-access", post(update_access_handler))
        .route("/api/pending-requests", get(pending_requests_handler))
        .route("/api/respond-request", post(respond_request_handler))
        .route("/api/requested-images", get(requested_images_handler))
        .route("/api/accessible-images", get(accessible_images_handler))
        .route("/api/my-shared-images", get(my_shared_images_handler))
        .route("/api/manage-access", post(manage_access_handler))
        .route("/api/view-image", post(view_image_handler))
        .route(
            "/api/default-view-limit",
            get(get_default_view_limit_handler),
        )
        .route("/api/auto-grant-access", post(auto_grant_access_handler))
        .route("/api/health", get(health_check))
        .nest_service("/test_images", ServeDir::new("test_images"))
        .nest_service("/encrypted_images", ServeDir::new("encrypted_images"))
        .nest_service("/", ServeDir::new("frontend/build"))
        .layer(CorsLayer::permissive())
        .with_state(state.clone());

    let addr = format!("0.0.0.0:{}", args.port);
    info!("🌐 Web server running on http://{}", addr);
    info!("📡 API endpoints ready");
    info!("🔗 P2P listener starting on port {}", args.p2p_port);

    let listener = tokio::net::TcpListener::bind(&addr).await?;

    // Start P2P listener in background
    let p2p_service_for_listener = p2p_service.clone();
    tokio::spawn(async move {
        if let Err(e) = p2p_service_for_listener.start_listener().await {
            error!("❌ P2P listener error: {}", e);
        }
    });

    // Set up graceful shutdown handler
    let state_for_shutdown = state.clone();
    let shutdown_signal = async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install CTRL+C signal handler");

        info!("🛑 Shutdown signal received, signing out from DoS...");

        // Sign out from DoS to mark as offline
        let mut dos_client = state_for_shutdown.dos_client.lock().await;
        match dos_client.sign_out().await {
            Ok(_) => info!("✅ Successfully signed out from DoS"),
            Err(e) => error!("❌ Failed to sign out from DoS: {}", e),
        }
    };

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal)
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

// Get client IP address (currently unused but kept for future use)
#[allow(dead_code)]
fn get_client_ip(connect_info: Option<ConnectInfo<SocketAddr>>) -> String {
    connect_info
        .map(|ci| ci.0.ip().to_string())
        .unwrap_or_else(|| "127.0.0.1".to_string())
}

async fn signup_handler(
    State(state): State<Arc<AppState>>,
    ConnectInfo(_addr): ConnectInfo<SocketAddr>,
    Json(payload): Json<SignUpRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiResponse>)> {
    // Use IP address from config file, not from HTTP connection
    // HTTP connection will be localhost if browser is on same machine
    let ip_address = state.config.client.ip_address.clone();
    info!(
        "Sign up request for {} with IP {}",
        payload.client_id, ip_address
    );

    let mut dos_client = state.dos_client.lock().await;
    let p2p_port = state.p2p_service.p2p_port;

    // Use the client_id provided by the user
    match dos_client
        .sign_up(payload.client_id.clone(), ip_address, p2p_port)
        .await
    {
        Ok(client_id) => {
            *state.current_user.lock().await = Some(client_id.clone());
            info!("Sign up successful: {}", client_id);

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
    ConnectInfo(_addr): ConnectInfo<SocketAddr>,
    Json(payload): Json<SignInRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiResponse>)> {
    // Use IP address from config file, not from HTTP connection
    // HTTP connection will be localhost if browser is on same machine
    let ip_address = state.config.client.ip_address.clone();
    info!(
        "Sign in request for {} with IP {}",
        payload.client_id, ip_address
    );

    let mut dos_client = state.dos_client.lock().await;
    let p2p_port = state.p2p_service.p2p_port;

    match dos_client
        .sign_in(payload.client_id.clone(), ip_address, p2p_port)
        .await
    {
        Ok(notifications) => {
            *state.current_user.lock().await = Some(payload.client_id.clone());

            // CRITICAL: Update P2P service's client_id to match the actual signed-in ID
            // The P2P service was initialized with config.client.name which might have different casing
            // But we need it to match the actual client_id used in Firebase (e.g., "client3" not "Client3")
            // This is a hack - ideally P2P service client_id should be mutable or set after sign-in
            // For now, we'll create a new P2P service with the correct client_id
            // Actually, we can't easily do this because P2P service is Arc<> and already listening
            // The real fix is to make client_id in P2P service Arc<Mutex<String>> or similar
            // For now, let's just log a warning if they don't match
            if state.p2p_service.client_id != payload.client_id {
                error!(
                    "⚠️  WARNING: P2P service client_id ({}) doesn't match signed-in client_id ({}). \
                    P2P transfers will fail! Update config file or sign in with correct ID.",
                    state.p2p_service.client_id, payload.client_id
                );
            }

            // Process any pending access changes that occurred while offline
            process_pending_access_changes(&state, &payload.client_id).await;

            info!(
                "Sign in successful: {} ({} notifications)",
                payload.client_id,
                notifications.len()
            );

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
        let p2p_port = state.p2p_service.p2p_port;
        let ip_address = state.config.client.ip_address.clone();
        match dos_client
            .sign_in(requester_id.clone(), ip_address, p2p_port)
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
        .request_image_access(
            &payload.owner_id,
            &payload.image_id,
            payload.req_access_limit,
        )
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
                let images_json: Vec<serde_json::Value> = images
                    .iter()
                    .map(|img| serde_json::to_value(img).unwrap())
                    .collect();
                Ok((
                    StatusCode::OK,
                    Json(ApiResponse {
                        success: true,
                        images: Some(images_json),
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
        let p2p_port = state.p2p_service.p2p_port;
        let ip_address = state.config.client.ip_address.clone();
        match dos_client
            .sign_in(client_id.clone(), ip_address, p2p_port)
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
        "Responding to request {} with approved={}",
        payload.request_id, payload.approved
    );

    // Get current user (owner)
    let current_user = state.current_user.lock().await.clone();
    let owner_id = match current_user {
        Some(id) => id,
        None => {
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
    };

    if payload.approved {
        // Get request details from Firebase
        let request = match state
            .firebase
            .get_pending_request(&payload.request_id)
            .await
        {
            Ok(Some(req)) => req,
            Ok(None) => {
                return Err((
                    StatusCode::NOT_FOUND,
                    Json(ApiResponse {
                        success: false,
                        error: Some("Request not found".to_string()),
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
            Err(e) => {
                error!("Failed to get pending request: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiResponse {
                        success: false,
                        error: Some(format!("Failed to get request: {}", e)),
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
        };

        let requester_id = request
            .get("requester_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let image_id = request
            .get("image_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        info!(
            "Creating personalized carrier for {} requesting {} (view_limit: {:?})",
            requester_id, image_id, payload.view_limit
        );

        // Get image info from Firebase to get the base carrier path
        let owner_client = match state.firebase.get_client(&owner_id).await {
            Ok(Some(client)) => client,
            Ok(None) => {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiResponse {
                        success: false,
                        error: Some("Owner not found in Firebase".to_string()),
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
            Err(e) => {
                error!("Failed to get owner from Firebase: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiResponse {
                        success: false,
                        error: Some(format!("Failed to get owner: {}", e)),
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
        };

        let image_info = match owner_client.images.get(image_id) {
            Some(img) => img,
            None => {
                return Err((
                    StatusCode::NOT_FOUND,
                    Json(ApiResponse {
                        success: false,
                        error: Some("Image not found".to_string()),
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
        };

        // Read the base encrypted carrier from disk
        let carrier_bytes = match std::fs::read(&image_info.encrypted_path) {
            Ok(bytes) => bytes,
            Err(e) => {
                error!(
                    "Failed to read carrier from {}: {}",
                    image_info.encrypted_path, e
                );
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiResponse {
                        success: false,
                        error: Some(format!("Failed to read carrier: {}", e)),
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
        };

        info!("📂 Read base carrier: {} bytes", carrier_bytes.len());

        // Extract the secret image from the base carrier
        let secret_image = match extract_image_with_access_rights(&carrier_bytes) {
            Ok((img, _)) => img,
            Err(e) => {
                error!("Failed to extract secret image: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiResponse {
                        success: false,
                        error: Some(format!("Failed to extract secret: {}", e)),
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
        };

        info!("🔓 Extracted secret image: {} bytes", secret_image.len());

        // Create personalized carrier with requester's access rights via compute servers
        let view_limit = payload.view_limit.unwrap_or(999999);
        let embedded_rights = EmbeddedAccessRights {
            username: requester_id.to_string(),
            view_limit,
            view_count: 0,
        };

        info!(
            "🎨 Requesting server to create personalized carrier for {} (limit: {})",
            requester_id, view_limit
        );

        let request_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let personalized_carrier = match state
            .client
            .lock()
            .await
            .submit_encryption_with_access_rights(request_id, secret_image, Some(embedded_rights))
            .await
        {
            Ok(data) => data,
            Err(e) => {
                error!("Failed to create personalized carrier: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiResponse {
                        success: false,
                        error: Some(format!("Failed to create personalized carrier: {}", e)),
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
        };

        info!(
            "✅ Created personalized carrier: {} bytes",
            personalized_carrier.len()
        );

        // Save personalized carrier to disk
        let personalized_path = format!(
            "encrypted_images/{}/{}_for_{}.png",
            owner_id, image_id, requester_id
        );

        if let Err(e) = std::fs::write(&personalized_path, &personalized_carrier) {
            error!("Failed to save personalized carrier: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse {
                    success: false,
                    error: Some(format!("Failed to save carrier: {}", e)),
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

        info!("💾 Saved personalized carrier to: {}", personalized_path);

        // Send personalized carrier to requester via P2P immediately
        info!(
            "📡 Sending personalized carrier to {} via P2P...",
            requester_id
        );

        // Get requester's P2P address from DoS
        let dos_client = state.dos_client.lock().await;
        let (requester_ip, requester_p2p_port) =
            match dos_client.get_peer_address(requester_id).await {
                Ok(Some((ip, port))) => (ip, port),
                Ok(None) => {
                    error!(
                        "Requester {} not found or offline, cannot send personalized carrier",
                        requester_id
                    );
                    drop(dos_client);
                    return Err((
                        StatusCode::SERVICE_UNAVAILABLE,
                        Json(ApiResponse {
                            success: false,
                            error: Some(
                                "Requester is offline. Cannot deliver personalized carrier."
                                    .to_string(),
                            ),
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
                Err(e) => {
                    error!("Failed to get requester address: {}", e);
                    drop(dos_client);
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ApiResponse {
                            success: false,
                            error: Some(format!("Failed to get requester address: {}", e)),
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
            };
        drop(dos_client);

        info!("📍 Requester at {}:{}", requester_ip, requester_p2p_port);

        // Send the personalized carrier via P2P
        match state
            .p2p_service
            .send_personalized_carrier(
                &requester_ip,
                requester_p2p_port,
                image_id,
                &owner_id,
                personalized_carrier.clone(),
            )
            .await
        {
            Ok(_) => {
                info!(
                    "✅ Personalized carrier sent to {} successfully",
                    requester_id
                );
            }
            Err(e) => {
                error!(
                    "Failed to send personalized carrier to {}: {}",
                    requester_id, e
                );
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiResponse {
                        success: false,
                        error: Some(format!("Failed to deliver personalized carrier: {}", e)),
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

        // Update Firebase with personalized carrier path ONLY (no access_rights in Firebase)
        // Access rights are embedded in the carrier image itself
        let mut updated_image = image_info.clone();
        updated_image
            .personalized_carriers
            .insert(requester_id.to_string(), personalized_path.clone());

        if let Err(e) = state.firebase.store_image(&owner_id, &updated_image).await {
            error!(
                "Failed to update Firebase with personalized carrier path: {}",
                e
            );
        }

        info!("✅ Updated Firebase with personalized carrier path (access rights embedded in carrier)");
    }

    // Send approval/denial response to DoS
    let dos_client = state.dos_client.lock().await;
    match dos_client
        .respond_to_access_request(&payload.request_id, payload.approved, payload.view_limit)
        .await
    {
        Ok(_) => {
            info!("✅ Access request response sent successfully");
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

    let _client_id = current_user.as_ref().unwrap();
    let _dos_client = state.dos_client.lock().await;

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

async fn accessible_images_handler(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiResponse>)> {
    info!("Fetching accessible images from local storage");

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

    let mut accessible_images = vec![];

    // Scan local encrypted_images directory for personalized carriers
    // Format: encrypted_images/{client_id}/{image_id}_from_{owner_id}.png
    let local_dir = format!("encrypted_images/{}", client_id);
    info!("📂 [ACCESSIBLE_IMAGES] Scanning directory: {}", local_dir);

    match std::fs::read_dir(&local_dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(filename) = path.file_name().and_then(|f| f.to_str()) {
                    // Check if it's a personalized carrier (ends with _from_*.png)
                    if filename.contains("_from_") && filename.ends_with(".png") {
                        info!("📄 [ACCESSIBLE_IMAGES] Found carrier file: {}", filename);

                        // Parse filename: {image_id}_from_{owner_id}.png
                        if let Some((image_id_part, rest)) = filename.rsplit_once("_from_") {
                            if let Some(owner_id) = rest.strip_suffix(".png") {
                                // Read and extract access rights from the carrier
                                match std::fs::read(&path) {
                                    Ok(carrier_data) => {
                                        match extract_image_with_access_rights(&carrier_data) {
                                            Ok((_, Some(embedded_access))) => {
                                                info!(
                                                    "✅ [ACCESSIBLE_IMAGES] Extracted access for {}: {}/{}",
                                                    image_id_part, embedded_access.view_count, embedded_access.view_limit
                                                );

                                                // Create AccessRight from embedded data
                                                let access = AccessRight {
                                                    view_limit: embedded_access.view_limit,
                                                    view_count: embedded_access.view_count,
                                                };

                                                // Create basic ImageInfo
                                                let image = ImageInfo {
                                                    image_id: image_id_part.to_string(),
                                                    name: image_id_part.replace('_', "."),
                                                    encrypted_path: path
                                                        .to_string_lossy()
                                                        .to_string(),
                                                    access_rights: std::collections::HashMap::new(),
                                                    personalized_carriers:
                                                        std::collections::HashMap::new(),
                                                };

                                                accessible_images.push(serde_json::json!({
                                                    "owner_id": owner_id,
                                                    "image": image,
                                                    "access": access
                                                }));
                                            }
                                            Ok((_, None)) => {
                                                info!("⚠️ [ACCESSIBLE_IMAGES] No embedded access rights in {}", filename);
                                            }
                                            Err(e) => {
                                                error!("❌ [ACCESSIBLE_IMAGES] Failed to extract access rights from {}: {}", filename, e);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        error!(
                                            "❌ [ACCESSIBLE_IMAGES] Failed to read {}: {}",
                                            filename, e
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }

            info!(
                "✅ [ACCESSIBLE_IMAGES] Found {} accessible images",
                accessible_images.len()
            );
            Ok((
                StatusCode::OK,
                Json(ApiResponse {
                    success: true,
                    images: Some(accessible_images),
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
        Err(e) => {
            info!(
                "⚠️ [ACCESSIBLE_IMAGES] Directory not found or empty: {} - {}",
                local_dir, e
            );
            // Return empty list if directory doesn't exist (user has no shared images yet)
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
}

async fn my_shared_images_handler(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiResponse>)> {
    info!("📤 [MY_SHARED_IMAGES] Fetching images I've shared with others");

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

    let owner_id = current_user.as_ref().unwrap().clone();
    drop(current_user);

    // Get my client info from Firebase
    match state.firebase.get_client(&owner_id).await {
        Ok(Some(client)) => {
            let mut shared_images = vec![];

            // For each image I own, check who has access
            for (image_id, image) in &client.images {
                // Get access rights for this image
                let mut access_list = vec![];

                for (requester_id, access) in &image.access_rights {
                    access_list.push(serde_json::json!({
                        "requester_id": requester_id,
                        "view_limit": access.view_limit,
                        "view_count": access.view_count,
                        "views_left": access.view_limit.saturating_sub(access.view_count)
                    }));
                }

                // Only include images that have been shared (have access rights)
                if !access_list.is_empty() {
                    shared_images.push(serde_json::json!({
                        "image_id": image_id,
                        "name": image.name,
                        "access_list": access_list,
                        "total_shared_with": access_list.len()
                    }));
                }
            }

            info!(
                "✅ [MY_SHARED_IMAGES] Found {} shared images",
                shared_images.len()
            );
            Ok((
                StatusCode::OK,
                Json(ApiResponse {
                    success: true,
                    images: Some(shared_images),
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
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(ApiResponse {
                success: false,
                error: Some("Client not found".to_string()),
                message: None,
                carrier_image_base64: None,
                client_id: None,
                notifications: None,
                request_id: None,
                peers: None,
                images: None,
                requests: None,
            }),
        )),
        Err(e) => {
            error!("❌ [MY_SHARED_IMAGES] Failed to fetch client: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse {
                    success: false,
                    error: Some(format!("Failed to fetch shared images: {}", e)),
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

async fn manage_access_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ManageAccessRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiResponse>)> {
    info!(
        "🔧 [MANAGE_ACCESS] {} access for image {} to requester {}",
        payload.action, payload.image_id, payload.requester_id
    );

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

    let owner_id = current_user.as_ref().unwrap().clone();
    drop(current_user);

    // Validate action
    if payload.action != "revoke" && payload.action != "modify" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse {
                success: false,
                error: Some("Invalid action. Must be 'revoke' or 'modify'".to_string()),
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

    if payload.action == "modify" && payload.new_view_limit.is_none() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse {
                success: false,
                error: Some("new_view_limit is required for modify action".to_string()),
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

    // Get current client data from Firebase
    let mut client = match state.firebase.get_client(&owner_id).await {
        Ok(Some(c)) => c,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ApiResponse {
                    success: false,
                    error: Some("Owner client not found".to_string()),
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
        Err(e) => {
            error!("❌ [MANAGE_ACCESS] Failed to get client: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse {
                    success: false,
                    error: Some(format!("Failed to get client: {}", e)),
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
    };

    // Get the image
    let image = match client.images.get_mut(&payload.image_id) {
        Some(img) => img,
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ApiResponse {
                    success: false,
                    error: Some("Image not found".to_string()),
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
    };

    let revoked = payload.action == "revoke";
    let new_view_limit = if revoked {
        None
    } else {
        payload.new_view_limit
    };

    // Update Firebase based on action
    if revoked {
        // Remove access rights from Firebase
        image.access_rights.remove(&payload.requester_id);
        image.personalized_carriers.remove(&payload.requester_id);
        info!(
            "🗑️ [MANAGE_ACCESS] Revoked access for {} to image {}",
            payload.requester_id, payload.image_id
        );
    } else {
        // Modify view limit and reset view count to 0
        if let Some(new_limit) = new_view_limit {
            image.access_rights.insert(
                payload.requester_id.clone(),
                AccessRight {
                    view_limit: new_limit,
                    view_count: 0, // Reset count to 0
                },
            );
            info!(
                "✏️ [MANAGE_ACCESS] Modified view limit for {} to {} (reset count to 0)",
                payload.requester_id, new_limit
            );
        }
    }

    // Update Firebase
    if let Err(e) = state.firebase.store_client(&owner_id, &client).await {
        error!("❌ [MANAGE_ACCESS] Failed to update Firebase: {}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse {
                success: false,
                error: Some(format!("Failed to update Firebase: {}", e)),
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

    // Send P2P message to requester to update their local carrier
    info!(
        "📡 [MANAGE_ACCESS] Sending P2P update to {} for image {}",
        payload.requester_id, payload.image_id
    );

    // Get requester's address from Firebase
    let requester_client = match state.firebase.get_client(&payload.requester_id).await {
        Ok(Some(c)) => c,
        Ok(None) => {
            error!(
                "⚠️ [MANAGE_ACCESS] Requester not found, changes saved but P2P notification failed"
            );
            return Ok((
                StatusCode::OK,
                Json(ApiResponse {
                    success: true,
                    message: Some(format!(
                        "Access {} but requester is offline. Changes will sync when they come online.",
                        if revoked { "revoked" } else { "modified" }
                    )),
                    error: None,
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
        Err(e) => {
            error!("❌ [MANAGE_ACCESS] Failed to get requester: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse {
                    success: false,
                    error: Some(format!("Failed to get requester: {}", e)),
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
    };

    // Send P2P AccessRightsUpdate message
    let p2p_result = state
        .p2p_service
        .send_access_rights_update(
            &requester_client.ip_address,
            requester_client.p2p_port,
            owner_id.clone(),
            payload.image_id.clone(),
            payload.requester_id.clone(),
            revoked,
            new_view_limit,
        )
        .await;

    match p2p_result {
        Ok(_) => {
            info!("✅ [MANAGE_ACCESS] P2P update sent successfully");
            Ok((
                StatusCode::OK,
                Json(ApiResponse {
                    success: true,
                    message: Some(format!(
                        "Access {} and requester notified",
                        if revoked { "revoked" } else { "modified" }
                    )),
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
            error!("⚠️ [MANAGE_ACCESS] Failed to send P2P update: {}", e);

            // Store pending access change in Firebase for when requester comes online
            let pending_change = serde_json::json!({
                "owner_id": owner_id,
                "image_id": payload.image_id,
                "revoked": revoked,
                "new_view_limit": new_view_limit,
                "timestamp": chrono::Utc::now().to_rfc3339()
            });

            if let Err(store_err) = state
                .firebase
                .store_pending_access_change(&payload.requester_id, &pending_change)
                .await
            {
                error!(
                    "❌ [MANAGE_ACCESS] Failed to store pending change: {}",
                    store_err
                );
            } else {
                info!("💾 [MANAGE_ACCESS] Stored pending access change for offline requester");
            }

            Ok((
                StatusCode::OK,
                Json(ApiResponse {
                    success: true,
                    message: Some(format!(
                        "Access {} but requester is offline. Changes will apply when they sign in.",
                        if revoked { "revoked" } else { "modified" }
                    )),
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
    }
}

async fn view_image_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ViewImageRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiResponse>)> {
    info!(
        "🔍 [VIEW_IMAGE] Request received - image_id: {}, owner_id: {}",
        payload.image_id, payload.owner_id
    );

    let current_user = state.current_user.lock().await;
    if current_user.is_none() {
        error!("❌ [VIEW_IMAGE] Not signed in");
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

    let viewer_id = current_user.as_ref().unwrap().clone();
    info!("👤 [VIEW_IMAGE] Viewer ID: {}", viewer_id);
    drop(current_user);

    // Read the personalized carrier from local disk (received at approval time)
    let local_carrier_path = format!(
        "encrypted_images/{}/{}_from_{}.png",
        viewer_id, payload.image_id, payload.owner_id
    );

    info!(
        "📂 [VIEW_IMAGE] Reading personalized carrier from: {}",
        local_carrier_path
    );

    let carrier_data = match std::fs::read(&local_carrier_path) {
        Ok(data) => {
            info!(
                "✅ [VIEW_IMAGE] Read personalized carrier: {} bytes",
                data.len()
            );
            data
        }
        Err(e) => {
            error!("❌ [VIEW_IMAGE] Failed to read personalized carrier: {}", e);
            error!("💡 [VIEW_IMAGE] Make sure the access request was approved and carrier was delivered");
            return Err((
                StatusCode::NOT_FOUND,
                Json(ApiResponse {
                    success: false,
                    error: Some(format!(
                        "Personalized carrier not found. Request may not be approved yet or carrier delivery failed: {}",
                        e
                    )),
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
    };

    // Extract secret image and access rights from carrier
    info!("🔓 [VIEW_IMAGE] Extracting secret image and access rights...");
    let (secret_image, access_rights_opt) = match extract_image_with_access_rights(&carrier_data) {
        Ok(data) => data,
        Err(e) => {
            error!("❌ [VIEW_IMAGE] Failed to extract secret: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse {
                    success: false,
                    error: Some(format!("Failed to decrypt carrier: {}", e)),
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
    };

    // Check and enforce view limits
    if let Some(mut access_rights) = access_rights_opt {
        info!(
            "🔐 [VIEW_IMAGE] Access rights found - views: {}/{}",
            access_rights.view_count, access_rights.view_limit
        );

        // Check if view limit exceeded
        if access_rights.view_count >= access_rights.view_limit {
            error!(
                "🚫 [VIEW_IMAGE] View limit exceeded: {}/{}",
                access_rights.view_count, access_rights.view_limit
            );

            // Delete local carrier file
            if let Err(e) = std::fs::remove_file(&local_carrier_path) {
                error!("⚠️ [VIEW_IMAGE] Failed to delete carrier file: {}", e);
            } else {
                info!(
                    "🗑️ [VIEW_IMAGE] Deleted local carrier file: {}",
                    local_carrier_path
                );
            }

            // Remove access rights from Firebase
            let owner_id = payload.owner_id.clone();
            let image_id = payload.image_id.clone();

            if let Ok(Some(mut owner_client)) = state.firebase.get_client(&owner_id).await {
                if let Some(image) = owner_client.images.get_mut(&image_id) {
                    image.access_rights.remove(&viewer_id);
                    image.personalized_carriers.remove(&viewer_id);

                    if let Err(e) = state.firebase.store_client(&owner_id, &owner_client).await {
                        error!("❌ [VIEW_IMAGE] Failed to update owner's Firebase: {}", e);
                    } else {
                        info!("✅ [VIEW_IMAGE] Removed access rights from owner's Firebase");
                    }
                }
            }

            return Err((
                StatusCode::FORBIDDEN,
                Json(ApiResponse {
                    success: false,
                    error: Some(format!(
                        "View limit exceeded ({}/{}). Image access has been removed.",
                        access_rights.view_count, access_rights.view_limit
                    )),
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

        // Increment view count
        access_rights.view_count += 1;
        info!(
            "✅ [VIEW_IMAGE] Incremented view count: {}/{}",
            access_rights.view_count, access_rights.view_limit
        );

        // Check if this was the last allowed view
        let is_last_view = access_rights.view_count >= access_rights.view_limit;

        if is_last_view {
            info!(
                "🎯 [VIEW_IMAGE] This is the last allowed view ({}/{})",
                access_rights.view_count, access_rights.view_limit
            );
        }

        // Re-embed with updated view count using the cover image
        let carrier_path = "cover_images/cover_image.png";
        match std::fs::read(carrier_path) {
            Ok(carrier_bytes) => {
                match embed_image_with_access_rights(
                    &carrier_bytes,
                    &secret_image,
                    Some(&access_rights),
                ) {
                    Ok(updated_carrier) => {
                        if let Err(e) = std::fs::write(&local_carrier_path, &updated_carrier) {
                            error!("⚠️ [VIEW_IMAGE] Failed to save updated carrier: {}", e);
                        } else {
                            info!(
                                "💾 [VIEW_IMAGE] Saved updated carrier with new view count: {}/{}",
                                access_rights.view_count, access_rights.view_limit
                            );
                        }
                    }
                    Err(e) => {
                        error!("⚠️ [VIEW_IMAGE] Failed to re-embed carrier: {}", e);
                    }
                }
            }
            Err(e) => {
                error!(
                    "⚠️ [VIEW_IMAGE] Failed to read carrier image {}: {}",
                    carrier_path, e
                );
            }
        }

        // If this was the last view, clean up after showing the image
        if is_last_view {
            // Delete local carrier file
            if let Err(e) = std::fs::remove_file(&local_carrier_path) {
                error!(
                    "⚠️ [VIEW_IMAGE] Failed to delete carrier file after last view: {}",
                    e
                );
            } else {
                info!(
                    "🗑️ [VIEW_IMAGE] Deleted local carrier file after last view: {}",
                    local_carrier_path
                );
            }

            // Remove access rights from Firebase
            let owner_id = payload.owner_id.clone();
            let image_id = payload.image_id.clone();

            if let Ok(Some(mut owner_client)) = state.firebase.get_client(&owner_id).await {
                if let Some(image) = owner_client.images.get_mut(&image_id) {
                    image.access_rights.remove(&viewer_id);
                    image.personalized_carriers.remove(&viewer_id);

                    if let Err(e) = state.firebase.store_client(&owner_id, &owner_client).await {
                        error!(
                            "❌ [VIEW_IMAGE] Failed to update owner's Firebase after last view: {}",
                            e
                        );
                    } else {
                        info!("✅ [VIEW_IMAGE] Removed access rights from owner's Firebase after last view");
                    }
                }
            }
        }
    } else {
        info!("ℹ️ [VIEW_IMAGE] No access rights embedded (unlimited viewing)");
    }

    // Encode secret image as base64 and return
    info!("🔄 [VIEW_IMAGE] Encoding secret image as base64...");
    let image_base64 = general_purpose::STANDARD.encode(&secret_image);
    info!(
        "✅ [VIEW_IMAGE] Successfully returning secret image ({} base64 chars)",
        image_base64.len()
    );

    Ok((
        StatusCode::OK,
        Json(ApiResponse {
            success: true,
            message: Some("Image viewed successfully".to_string()),
            carrier_image_base64: Some(image_base64),
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

/// Process pending access changes for a client who just signed in
/// Called automatically after successful sign-in
async fn process_pending_access_changes(state: &Arc<AppState>, client_id: &str) {
    info!(
        "🔄 [PENDING_CHANGES] Processing pending access changes for {}",
        client_id
    );

    // Get and clear all pending changes from Firebase
    let pending_changes = match state
        .firebase
        .get_and_clear_pending_access_changes(client_id)
        .await
    {
        Ok(changes) => changes,
        Err(e) => {
            error!("❌ [PENDING_CHANGES] Failed to get pending changes: {}", e);
            return;
        }
    };

    if pending_changes.is_empty() {
        info!("ℹ️ [PENDING_CHANGES] No pending changes for {}", client_id);
        return;
    }

    info!(
        "📋 [PENDING_CHANGES] Found {} pending change(s)",
        pending_changes.len()
    );

    // Process each pending change
    for change in pending_changes {
        let owner_id = match change.get("owner_id").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => {
                error!("❌ [PENDING_CHANGES] Missing owner_id in change");
                continue;
            }
        };

        let image_id = match change.get("image_id").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => {
                error!("❌ [PENDING_CHANGES] Missing image_id in change");
                continue;
            }
        };

        let revoked = change
            .get("revoked")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let new_view_limit = change
            .get("new_view_limit")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32);

        info!(
            "🔧 [PENDING_CHANGES] Applying change: owner={}, image={}, revoked={}, new_limit={:?}",
            owner_id, image_id, revoked, new_view_limit
        );

        // Apply the change to local carrier file
        let carrier_path = format!(
            "encrypted_images/{}/{}_from_{}.png",
            client_id, image_id, owner_id
        );

        if revoked {
            // Delete the carrier file
            if let Err(e) = std::fs::remove_file(&carrier_path) {
                error!("⚠️ [PENDING_CHANGES] Failed to delete carrier: {}", e);
            } else {
                info!("🗑️ [PENDING_CHANGES] Deleted carrier: {}", carrier_path);
            }
        } else if let Some(new_limit) = new_view_limit {
            // Update the carrier with new view limit and reset count
            match std::fs::read(&carrier_path) {
                Ok(carrier_data) => {
                    match extract_image_with_access_rights(&carrier_data) {
                        Ok((_secret, Some(_current_access))) => {
                            let updated_access = EmbeddedAccessRights {
                                username: client_id.to_string(),
                                view_limit: new_limit,
                                view_count: 0, // Reset count to 0
                            };

                            match update_embedded_access_rights(&carrier_data, &updated_access) {
                                Ok(updated_carrier) => {
                                    if let Err(e) = std::fs::write(&carrier_path, updated_carrier) {
                                        error!("❌ [PENDING_CHANGES] Failed to write updated carrier: {}", e);
                                    } else {
                                        info!("✅ [PENDING_CHANGES] Updated carrier with new limit: {}", new_limit);
                                    }
                                }
                                Err(e) => {
                                    error!(
                                        "❌ [PENDING_CHANGES] Failed to update access rights: {}",
                                        e
                                    );
                                }
                            }
                        }
                        Ok((_secret, None)) => {
                            error!("⚠️ [PENDING_CHANGES] No access rights found in carrier");
                        }
                        Err(e) => {
                            error!(
                                "❌ [PENDING_CHANGES] Failed to extract access rights: {}",
                                e
                            );
                        }
                    }
                }
                Err(e) => {
                    error!(
                        "⚠️ [PENDING_CHANGES] Carrier not found: {} - {}",
                        carrier_path, e
                    );
                }
            }
        }
    }

    info!(
        "✅ [PENDING_CHANGES] Finished processing pending changes for {}",
        client_id
    );
}

async fn get_default_view_limit_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse>, (StatusCode, Json<ApiResponse>)> {
    match state.firebase.get_default_view_limit().await {
        Ok(limit) => Ok(Json(ApiResponse {
            success: true,
            error: None,
            message: Some(format!("Default view limit: {}", limit)),
            carrier_image_base64: None,
            client_id: None,
            notifications: Some(vec![json!({"view_limit": limit})]),
            request_id: None,
            peers: None,
            images: None,
            requests: None,
        })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse {
                success: false,
                error: Some(format!("Failed to get default view limit: {}", e)),
                message: None,
                carrier_image_base64: None,
                client_id: None,
                notifications: None,
                request_id: None,
                peers: None,
                images: None,
                requests: None,
            }),
        )),
    }
}

#[derive(Deserialize)]
struct AutoGrantAccessRequest {
    client_id: String,
    owner_id: String,
    image_id: String,
}

async fn auto_grant_access_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AutoGrantAccessRequest>,
) -> Result<Json<ApiResponse>, (StatusCode, Json<ApiResponse>)> {
    // Get default view limit
    let view_limit = match state.firebase.get_default_view_limit().await {
        Ok(limit) => limit,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse {
                    success: false,
                    error: Some(format!("Failed to get default view limit: {}", e)),
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
    };

    // Auto-grant access to the requesting client
    match state
        .firebase
        .grant_access(&req.owner_id, &req.image_id, &req.client_id, view_limit)
        .await
    {
        Ok(_) => Ok(Json(ApiResponse {
            success: true,
            error: None,
            message: Some(format!("Access granted with {} views", view_limit)),
            carrier_image_base64: None,
            client_id: None,
            notifications: Some(vec![json!({"view_limit": view_limit})]),
            request_id: None,
            peers: None,
            images: None,
            requests: None,
        })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse {
                success: false,
                error: Some(format!("Failed to grant access: {}", e)),
                message: None,
                carrier_image_base64: None,
                client_id: None,
                notifications: None,
                request_id: None,
                peers: None,
                images: None,
                requests: None,
            }),
        )),
    }
}
