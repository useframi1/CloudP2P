//! # Client Components
//!
//! The client is split into two main components:
//!
//! ## Core Client ([`client`])
//! Handles the primary responsibility: sending images and receiving encrypted results.
//! This component performs the actual image transmission and validates the encryption.
//!
//! ## Client Middleware ([`middleware`])
//! Manages all coordination concerns:
//! - Leader discovery across servers
//! - Request broadcasting to leader
//! - Retry logic (3 attempts with timeouts)
//! - Server assignment request handling
//! - Failover on server failure
//! - Connection management

pub mod client;
pub mod middleware;

// Re-export for convenience
pub use middleware::ClientMiddleware;
pub use client::ClientCore;
