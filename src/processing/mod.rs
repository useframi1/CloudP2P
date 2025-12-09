//! # Image Processing and Steganography
//!
//! This module provides image encryption and decryption using LSB (Least Significant Bit)
//! steganography technique.

pub mod steganography;

// Re-export main functions for convenience
pub use steganography::{
    embed_image_with_access_rights, embed_text_bytes, extract_image_with_access_rights,
    extract_text_bytes, update_embedded_access_rights, EmbeddedAccessRights,
};
