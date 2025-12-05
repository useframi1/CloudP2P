//! Directory of Services (DoS) Module
//!
//! Manages client registration, status tracking, and P2P coordination.

pub mod firebase;
pub mod service;

pub use firebase::FirebaseClient;
pub use service::DoSService;
