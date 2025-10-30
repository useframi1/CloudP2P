//! # Common Components
//!
//! Shared utilities and data structures used by both client and server components.
//!
//! ## Modules
//!
//! - [`messages`]: Protocol message definitions for client-server and peer-to-peer communication
//! - [`connection`]: TCP connection abstraction with message framing
//! - [`config`]: Configuration parsing utilities

pub mod messages;
pub mod connection;
pub mod config;
