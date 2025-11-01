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
//! For stress testing with metrics:
//! ```bash
//! cargo run --bin client -- --config config/client_stress.toml \
//!   --machine-id 1 --client-id 1 \
//!   --image-dir ./test_images \
//!   --metrics-output ./metrics/machine_1_client_1.json
//! ```
//!
//! The client will:
//! 1. Load configuration from the specified TOML file
//! 2. Initialize the client core (image transmission service)
//! 3. Initialize the client middleware (request coordination)
//! 4. Discover the current leader
//! 5. Submit encryption tasks at the configured rate
//! 6. Handle retries and failover automatically
//! 7. Track metrics and export to JSON (if metrics-output specified)

use clap::Parser;
use env_logger::Builder;
use log::LevelFilter;
use std::io::Write;
use std::sync::Arc;

// Import from the library crate
use cloud_p2p::client::middleware::ClientConfig;
use cloud_p2p::client::{ClientCore, ClientMetrics, ClientMiddleware};
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

    /// Path to write metrics JSON output (optional)
    #[arg(long)]
    metrics_output: Option<String>,

    /// Client ID (appended to name from config, e.g., "Machine_1" + "_Client_5")
    #[arg(long)]
    client_id: Option<u32>,
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
    let mut config: ClientConfig = load_config(&args.config)?;

    // Append client ID to name if provided
    let client_name = if let Some(id) = args.client_id {
        format!("{}_Client_{}", config.client.name, id)
    } else {
        config.client.name.clone()
    };

    config.client.name = client_name.clone();

    // Create the client core (handles image transmission)
    let core = Arc::new(ClientCore::new(client_name.clone()));

    // Create the client middleware (handles request coordination)
    let mut middleware = ClientMiddleware::new(config, core);

    // Initialize metrics if output path is specified
    let metrics = if args.metrics_output.is_some() {
        let m = Arc::new(std::sync::Mutex::new(ClientMetrics::new(
            client_name.clone(),
        )));
        middleware = middleware.with_metrics(m.clone());
        Some(m)
    } else {
        None
    };

    // Run the client
    middleware.run().await;

    // Export metrics if enabled
    if let Some(metrics) = metrics {
        if let Some(output_path) = args.metrics_output {
            let metrics = metrics.lock().unwrap();
            metrics.export_to_json(&output_path)?;
            println!("Metrics exported to: {}", output_path);
        }
    }

    Ok(())
}
