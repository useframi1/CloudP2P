//! # Client Binary Entry Point
//!
//! Thin wrapper that initializes and runs the CloudP2P client with its middleware.
//!
//! ## Usage
//!
//! ```bash
//! cargo run --bin client -- --config config/client1.toml
//! ```
//!
//! The client will:
//! 1. Load configuration from the specified TOML file
//! 2. Initialize the client core (image transmission service)
//! 3. Initialize the client middleware (request coordination)
//! 4. Discover the current leader
//! 5. Submit encryption tasks at the configured rate
//! 6. Handle retries and failover automatically

use clap::Parser;
use env_logger::Builder;
use log::LevelFilter;
use std::io::Write;
use std::sync::Arc;

// Import from the library crate
use cloud_p2p::client::middleware::ClientConfig;
use cloud_p2p::client::{ClientCore, ClientMiddleware};
use cloud_p2p::common::config::load_config;

/// Command-line arguments for the client binary
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the client configuration file (TOML format)
    ///
    /// Example: config/client1.toml
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
                chrono::Local::now().format("%H:%M:%SS"),
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

    // Load client configuration from TOML file
    let config: ClientConfig = load_config(&args.config)?;

    // Create the client core (handles image transmission)
    let core = Arc::new(ClientCore::new(config.client.name.clone()));

    // Create the client middleware (handles request coordination)
    let mut middleware = ClientMiddleware::new(config, core);

    // Run the client (sends tasks for configured duration)
    middleware.run().await;

    Ok(())
}
