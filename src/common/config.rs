//! # Configuration Utilities
//!
//! Shared configuration structures and parsing utilities used by both
//! client and server components.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;

/// Load a TOML configuration file and deserialize it into the specified type.
///
/// # Arguments
/// - `path`: Path to the TOML configuration file
///
/// # Returns
/// - `Ok(T)`: Successfully loaded and parsed configuration
/// - `Err`: File I/O or parsing error
///
/// # Example
/// ```ignore
/// let config: ServerConfig = load_config("config/server1.toml")?;
/// ```
pub fn load_config<T>(path: &str) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    let content = fs::read_to_string(path)?;
    let config: T = toml::from_str(&content)?;
    Ok(config)
}

/// Information about a peer server in the distributed system.
///
/// Used to configure how servers connect to each other for leader election
/// and heartbeat monitoring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    /// Unique identifier for this peer server (e.g., 1, 2, 3)
    pub id: u32,
    /// Network address for connecting to this peer (e.g., "127.0.0.1:8001")
    pub address: String,
}

/// Container for the list of peer servers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeersConfig {
    /// List of all other servers in the cluster
    pub peers: Vec<PeerInfo>,
}

/// Election timing configuration.
///
/// Controls the timeouts and intervals for the Modified Bully Algorithm.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElectionConfig {
    /// How often to send heartbeat messages (seconds)
    pub heartbeat_interval_secs: u64,
    /// How long to wait for responses during an election (seconds)
    pub election_timeout_secs: u64,
    /// How long before a peer is considered failed (seconds)
    pub failure_timeout_secs: u64,
    /// How often to check for failed peers (seconds)
    pub monitor_interval_secs: u64,
}
