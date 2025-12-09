//! Directory of Services (DoS) Server
//!
//! Centralized service for client registration, image metadata, and P2P coordination.

use anyhow::{Context, Result};
use clap::Parser;
use cloud_p2p::common::connection::Connection;
use cloud_p2p::common::messages::Message;
use cloud_p2p::dos::DoSService;
use log::{error, info};
use std::sync::Arc;
use tokio::net::TcpListener;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Firebase Database URL
    #[arg(long)]
    firebase_url: Option<String>,

    /// Bind address (default: 127.0.0.1:9000)
    #[arg(long, default_value = "127.0.0.1:9000")]
    bind_address: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = Args::parse();

    // Get Firebase URL from args or environment
    let firebase_url = args
        .firebase_url
        .or_else(|| std::env::var("FIREBASE_DATABASE_URL").ok())
        .context("Firebase URL must be provided via --firebase-url or FIREBASE_DATABASE_URL env var")?;

    info!("==============================================");
    info!("  Directory of Services (DoS) Server");
    info!("==============================================");
    info!("Firebase URL: {}", firebase_url);
    info!("Bind address: {}", args.bind_address);
    info!("==============================================");
    info!("");

    // Initialize DoS service
    let dos = Arc::new(DoSService::new(firebase_url).await?);

    // Start TCP listener
    let listener = TcpListener::bind(&args.bind_address).await?;
    info!("DoS server listening on {}", args.bind_address);

    loop {
        let (socket, addr) = listener.accept().await?;
        let dos = Arc::clone(&dos);

        tokio::spawn(async move {
            info!("New connection from {}", addr);
            if let Err(e) = handle_client(dos, socket).await {
                error!("Error handling client {}: {}", addr, e);
            }
            info!("Client disconnected");
        });
    }
}

async fn handle_client(dos: Arc<DoSService>, socket: tokio::net::TcpStream) -> Result<()> {
    let mut conn = Connection::new(socket);

    let message = conn.read_message().await?.ok_or_else(|| anyhow::anyhow!("Connection closed"))?;

    match message {
        Message::ClientSignUp {
            client_name,
            ip_address,
            p2p_port,
        } => {
            info!("Processing ClientSignUp: {} (P2P port: {})", client_name, p2p_port);
            let client_id = dos.sign_up_client(client_name, ip_address, p2p_port).await?;
            conn.write_message(&Message::ClientSignUpResponse { client_id })
                .await?;
        }

        Message::ClientSignIn {
            client_id,
            ip_address,
            p2p_port,
        } => {
            info!("Processing ClientSignIn: {} (P2P port: {})", client_id, p2p_port);
            let notifications = dos.sign_in_client(client_id, ip_address, p2p_port).await?;
            conn.write_message(&Message::ClientSignInResponse { notifications })
                .await?;
        }

        Message::ClientSignOut { client_id } => {
            info!("Processing ClientSignOut: {}", client_id);
            dos.sign_out_client(client_id).await?;
            conn.write_message(&Message::Ack).await?;
        }

        Message::ListOnlineClients => {
            info!("Processing ListOnlineClients");
            let clients = dos.list_online_clients().await?;
            conn.write_message(&Message::OnlineClientsList { clients }).await?;
        }

        Message::RegisterImage { client_id, image } => {
            info!("Processing RegisterImage: {} for {}", image.image_id, client_id);
            dos.register_image(client_id, image).await?;
            conn.write_message(&Message::Ack).await?;
        }

        Message::ImageAccessRequest {
            request_id: _request_id,
            requester_id,
            owner_id,
            image_id,
            req_access_limit,
        } => {
            info!("Processing ImageAccessRequest from {} for {} (limit: {:?})", requester_id, image_id, req_access_limit);
            let assigned_request_id = dos
                .request_image_access(requester_id.clone(), owner_id.clone(), image_id.clone(), req_access_limit)
                .await?;
            conn.write_message(&Message::ImageAccessRequest {
                request_id: assigned_request_id,
                requester_id,
                owner_id,
                image_id,
                req_access_limit,
            })
            .await?;
        }

        Message::ImageAccessResponse {
            request_id,
            approved,
            view_limit,
        } => {
            info!("Processing ImageAccessResponse: {} (approved: {}, view_limit: {:?})", request_id, approved, view_limit);
            dos.respond_to_access_request(request_id, approved, view_limit).await?;
            conn.write_message(&Message::Ack).await?;
        }

        Message::UpdateAccessRights {
            client_id,
            image_id,
            access_rights,
        } => {
            info!("Processing UpdateAccessRights for image {} of client {}", image_id, client_id);
            dos.update_access_rights(client_id, image_id, access_rights)
                .await?;
            conn.write_message(&Message::Ack).await?;
        }

        Message::ReportPeerFailure { failed_client_id } => {
            info!("Processing ReportPeerFailure: {}", failed_client_id);
            dos.report_peer_failure(failed_client_id).await?;
            conn.write_message(&Message::Ack).await?;
        }

        Message::GetPendingRequests { owner_id } => {
            info!("Processing GetPendingRequests for owner: {}", owner_id);
            let requests = dos.get_pending_requests(owner_id).await?;
            conn.write_message(&Message::PendingRequestsList { requests }).await?;
        }

        Message::IncrementViewCount {
            owner_id,
            image_id,
            viewer_id,
        } => {
            info!("Processing IncrementViewCount: viewer {} viewing image {} of {}", viewer_id, image_id, owner_id);
            let allowed = dos.increment_view_count(owner_id, image_id, viewer_id).await?;
            conn.write_message(&Message::IncrementViewCountResponse { allowed }).await?;
        }

        Message::GetPeerAddress { peer_id } => {
            info!("Processing GetPeerAddress: {}", peer_id);
            match dos.get_peer_address(&peer_id).await {
                Ok((ip_address, p2p_port, online)) => {
                    conn.write_message(&Message::PeerAddressResponse {
                        peer_id,
                        ip_address,
                        p2p_port,
                        online,
                    }).await?;
                }
                Err(e) => {
                    error!("Failed to get peer address: {}", e);
                    // Send response with offline status
                    conn.write_message(&Message::PeerAddressResponse {
                        peer_id,
                        ip_address: String::new(),
                        p2p_port: 0,
                        online: false,
                    }).await?;
                }
            }
        }

        _ => {
            error!("Unexpected message type");
            anyhow::bail!("Unexpected message type");
        }
    }

    Ok(())
}
