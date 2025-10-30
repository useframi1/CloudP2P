//! # Server Components
//!
//! The server is split into two main components:
//!
//! ## Core Server ([`server`])
//! Handles the primary responsibility: image encryption using steganography.
//! This component receives task requests and returns encrypted images.
//!
//! ## Server Middleware ([`middleware`])
//! Manages all distributed system concerns:
//! - Leader election using Modified Bully Algorithm
//! - Heartbeat sending and monitoring
//! - Peer connection management
//! - Task assignment and load balancing
//! - Fault tolerance and orphaned task cleanup
//! - Message routing and coordination

pub mod server;
pub mod middleware;
pub mod election;

// Re-export for convenience
pub use middleware::ServerMiddleware;
pub use server::ServerCore;
pub use election::ServerMetrics;
