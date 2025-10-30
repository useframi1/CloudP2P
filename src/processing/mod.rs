//! # Image Processing and Steganography
//!
//! This module provides image encryption and decryption using LSB (Least Significant Bit)
//! steganography technique.

pub mod steganography;

// Re-export main functions for convenience
pub use steganography::{embed_text_bytes, extract_text_bytes};
