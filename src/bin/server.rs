//! # Server Binary Entry Point
//!
//! Thin wrapper that initializes and runs the CloudP2P server with its middleware.
//!
//! ## Usage
//!
//! ```bash
//! cargo run --bin server -- --config config/server1.toml
//! ```
//!
//! The server will:
//! 1. Load configuration from the specified TOML file
//! 2. Initialize the server core (encryption service)
//! 3. Initialize the server middleware (distributed coordination)
//! 4. Start all server tasks (listener, heartbeat, peer connections, monitoring)
//! 5. Participate in leader election using Modified Bully Algorithm

use clap::Parser;
use env_logger::Builder;
use log::LevelFilter;
use std::io::Write;
use std::sync::Arc;

// Import from the library crate
use cloud_p2p::common::config::load_config;
use cloud_p2p::server::middleware::ServerConfig;
use cloud_p2p::server::{ServerCore, ServerMiddleware};

/// Command-line arguments for the server binary
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the server configuration file (TOML format)
    ///
    /// Example: config/server1.toml
    #[arg(short, long)]
    config: String,
}

/// Initialize the logging system with timestamp, level, and message formatting.
///
/// Logs are printed to stdout with INFO level by default.
/// Format: `[HH:MM:SS] [LEVEL] message`
fn init_logger() {
    Builder::new()
        .format(|buf, record| {
            writeln!(
                buf,
                "[{}] [{}] {}",
                chrono::Local::now().format("%H:%M:%S"),
                record.level(),
                record.args()
            )
        })
        .filter_level(LevelFilter::Info)
        .init();
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    init_logger();

    // Parse command-line arguments
    let args = Args::parse();

    // Load server configuration from TOML file
    let config: ServerConfig = load_config(&args.config)?;

    // Create the server core (handles encryption)
    let core = Arc::new(ServerCore::new(config.server.id));

    // Create the server middleware (handles distributed coordination)
    let middleware = ServerMiddleware::new(config, core);

    // Start the server (runs indefinitely until error or shutdown)
    middleware.run().await;

    Ok(())
}
