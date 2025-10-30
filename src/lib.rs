//! # CloudP2P - Distributed Peer-to-Peer Cloud Computing System
//!
//! CloudP2P is a fault-tolerant distributed system that implements:
//! - **Modified Bully Algorithm** for leader election based on real-time load metrics
//! - **Load-balanced task distribution** across multiple servers
//! - **Image steganography** processing using LSB (Least Significant Bit) technique
//! - **Automatic failover** and fault tolerance mechanisms
//!
//! ## Architecture
//!
//! The system consists of three main components:
//!
//! 1. **Server Core**: Handles image encryption/decryption using steganography
//! 2. **Server Middleware**: Manages leader election, heartbeats, peer coordination,
//!    task distribution, and fault tolerance
//! 3. **Client Core**: Sends images and receives encrypted results
//! 4. **Client Middleware**: Handles leader discovery, request broadcasting, retry logic,
//!    and failover
//!
//! ## Modules
//!
//! - [`common`]: Shared components (messages, connections, config utilities)
//! - [`server`]: Server implementation (core + middleware)
//! - [`client`]: Client implementation (core + middleware)
//! - [`processing`]: Image processing and steganography algorithms

// Public modules
pub mod client;
pub mod common;
pub mod processing;
pub mod server;

// Re-export commonly used types for convenience
pub use client::middleware::ClientMiddleware;
pub use common::messages::Message;
pub use server::middleware::ServerMiddleware;
