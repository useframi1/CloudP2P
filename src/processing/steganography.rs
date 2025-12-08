//! # LSB Steganography Implementation
//!
//! Implements text embedding and extraction using Least Significant Bit (LSB) steganography.
//!
//! ## Algorithm
//!
//! The LSB steganography technique hides text within an image by modifying the least
//! significant bit of each color channel (R, G, B) in the image pixels.
//!
//! ### Encoding Process
//! 1. Convert text to bytes and prepend 4-byte length prefix
//! 2. For each bit in the text data:
//!    - Get the next pixel's RGB channels
//!    - Clear the LSB of each channel
//!    - Set the LSB to match the data bit
//!    - Move to next channel (R → G → B → next pixel)
//! 3. Save the modified image as PNG
//!
//! ### Decoding Process
//! 1. Read the first 32 bits (4 bytes) to get the text length
//! 2. Extract the next N bits (where N = length * 8)
//! 3. Convert bits back to bytes and then to UTF-8 string
//!
//! ### Capacity
//! An image can store approximately `(width * height * 3) / 8` bytes of text,
//! where 3 represents the RGB channels.
//!
//! Example: An 800x600 image can store ~180 KB of text.

use anyhow::Result;
use image::GenericImageView;
use serde::{Deserialize, Serialize};

/// Access rights for a specific user embedded in the carrier image
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddedAccessRights {
    pub username: String,
    pub view_limit: u32,
    pub view_count: u32,
}

/// Embed text into an image using LSB steganography.
///
/// The text is prefixed with its length (4 bytes, big-endian) and then embedded
/// into the least significant bits of the image's RGB channels.
///
/// # Arguments
/// - `image_bytes`: Raw bytes of the input image (any format supported by `image` crate)
/// - `text`: UTF-8 text to embed into the image
///
/// # Returns
/// - `Ok(Vec<u8>)`: PNG image bytes with embedded text
/// - `Err`: If image is too small, can't be loaded, or encoding fails
///
/// # Errors
/// - Image is too small to hold the text
/// - Image format is invalid
/// - Encoding to PNG fails
///
/// # Example
/// ```ignore
/// let image_data = std::fs::read("input.jpg")?;
/// let encrypted = embed_text_bytes(&image_data, "Secret message")?;
/// std::fs::write("output.png", encrypted)?;
/// ```
pub fn embed_text_bytes(image_bytes: &[u8], text: &str) -> Result<Vec<u8>> {
    // Load the image from bytes
    let img = image::load_from_memory(image_bytes)?;
    let (width, height) = img.dimensions();

    // Convert to RGBA format for consistent pixel manipulation
    let mut img = img.to_rgba8();

    // Prepare data to embed: [4 bytes length][text bytes]
    let text_bytes = text.as_bytes();
    let length = text_bytes.len() as u32;
    let mut data_to_embed = Vec::new();

    // Add length prefix (4 bytes, big-endian)
    data_to_embed.extend_from_slice(&length.to_be_bytes());
    // Add text content
    data_to_embed.extend_from_slice(text_bytes);

    // Check if image has enough capacity
    // Each pixel has 3 usable channels (R, G, B), so 3 bits per pixel
    let available_bits = (width * height * 3) as usize;
    let required_bits = data_to_embed.len() * 8;

    if required_bits > available_bits {
        return Err(anyhow::anyhow!(
            "Image too small for this text: need {} bits but only have {} bits available",
            required_bits, available_bits
        ));
    }

    // Embed data into LSBs of image pixels
    let mut data_index = 0; // Current byte being embedded
    let mut bit_index = 0;  // Current bit within the byte (0-7)

    'outer: for y in 0..height {
        for x in 0..width {
            // Stop if all data has been embedded
            if data_index >= data_to_embed.len() {
                break 'outer;
            }

            let pixel = img.get_pixel(x, y);
            let mut new_pixel = *pixel;

            // Embed into R, G, B channels (skip Alpha channel for compatibility)
            for channel in 0..3 {
                if data_index >= data_to_embed.len() {
                    break 'outer;
                }

                // Extract the current bit from data (MSB first)
                let bit = (data_to_embed[data_index] >> (7 - bit_index)) & 1;

                // Clear LSB and set it to our data bit
                new_pixel[channel] = (pixel[channel] & 0xFE) | bit;

                // Move to next bit
                bit_index += 1;
                if bit_index == 8 {
                    bit_index = 0;
                    data_index += 1;
                }
            }

            img.put_pixel(x, y, new_pixel);
        }
    }

    // Encode the modified image as PNG
    let mut output_bytes = Vec::new();
    img.write_to(
        &mut std::io::Cursor::new(&mut output_bytes),
        image::ImageFormat::Png,
    )?;

    Ok(output_bytes)
}

/// Extract text that was embedded in an image using LSB steganography.
///
/// Reads the 4-byte length prefix, then extracts that many bytes from the
/// LSBs of the image's RGB channels.
///
/// # Arguments
/// - `image_bytes`: Raw bytes of the steganography-encoded image
///
/// # Returns
/// - `Ok(String)`: The extracted UTF-8 text
/// - `Err`: If image can't be loaded or text extraction fails
///
/// # Errors
/// - Image format is invalid
/// - Extracted bytes are not valid UTF-8
/// - Length prefix is corrupted
///
/// # Example
/// ```ignore
/// let encrypted_image = std::fs::read("encrypted.png")?;
/// let secret_text = extract_text_bytes(&encrypted_image)?;
/// println!("Extracted: {}", secret_text);
/// ```
#[allow(dead_code)]
pub fn extract_text_bytes(image_bytes: &[u8]) -> Result<String> {
    // Load the image
    let img = image::load_from_memory(image_bytes)?;
    let img = img.to_rgba8();
    let (width, height) = img.dimensions();

    // ========== STEP 1: Extract length (first 4 bytes = 32 bits) ==========

    let mut length_bytes = [0u8; 4];
    let mut data_index = 0;
    let mut bit_index = 0;

    'length_loop: for y in 0..height {
        for x in 0..width {
            if data_index >= 4 {
                break 'length_loop;
            }

            let pixel = img.get_pixel(x, y);

            // Extract from R, G, B channels
            for channel in 0..3 {
                if data_index >= 4 {
                    break 'length_loop;
                }

                // Get the LSB from this channel
                let bit = pixel[channel] & 1;

                // Set this bit in our length bytes (MSB first)
                length_bytes[data_index] |= bit << (7 - bit_index);

                bit_index += 1;
                if bit_index == 8 {
                    bit_index = 0;
                    data_index += 1;
                }
            }
        }
    }

    let length = u32::from_be_bytes(length_bytes) as usize;

    // ========== STEP 2: Extract text data ==========

    let mut text_bytes = vec![0u8; length];
    data_index = 0;
    bit_index = 0;
    let mut skip_bits = 32; // Skip the length prefix we already read

    'outer: for y in 0..height {
        for x in 0..width {
            if data_index >= length {
                break 'outer;
            }

            let pixel = img.get_pixel(x, y);

            for channel in 0..3 {
                // Skip the first 32 bits (length prefix)
                if skip_bits > 0 {
                    skip_bits -= 1;
                    continue;
                }

                if data_index >= length {
                    break 'outer;
                }

                // Get the LSB from this channel
                let bit = pixel[channel] & 1;

                // Set this bit in our text bytes (MSB first)
                text_bytes[data_index] |= bit << (7 - bit_index);

                bit_index += 1;
                if bit_index == 8 {
                    bit_index = 0;
                    data_index += 1;
                }
            }
        }
    }

    // Convert bytes to UTF-8 string
    Ok(String::from_utf8(text_bytes)?)
}

/// Embed an image into another (carrier) image using LSB steganography.
///
/// The embedded image is prefixed with its length (4 bytes, big-endian) and then embedded
/// into the least significant bits of the carrier image's RGB channels.
///
/// # Arguments
/// - `carrier_image_bytes`: Raw bytes of the carrier image (the image that will hide data)
/// - `secret_image_bytes`: Raw bytes of the secret image to embed
///
/// # Returns
/// - `Ok(Vec<u8>)`: PNG image bytes with embedded secret image
/// - `Err`: If carrier image is too small, can't be loaded, or encoding fails
///
/// # Errors
/// - Carrier image is too small to hold the secret image
/// - Image format is invalid
/// - Encoding to PNG fails
///
/// # Example
/// ```ignore
/// let carrier = std::fs::read("carrier.jpg")?;
/// let secret = std::fs::read("secret.png")?;
/// let result = embed_image_bytes(&carrier, &secret)?;
/// std::fs::write("output.png", result)?;
/// ```
pub fn embed_image_bytes(carrier_image_bytes: &[u8], secret_image_bytes: &[u8]) -> Result<Vec<u8>> {
    // Load the carrier image
    let img = image::load_from_memory(carrier_image_bytes)?;
    let (width, height) = img.dimensions();

    // Convert to RGBA format for consistent pixel manipulation
    let mut img = img.to_rgba8();

    // Prepare data to embed: [4 bytes length][secret image bytes]
    let length = secret_image_bytes.len() as u32;
    let mut data_to_embed = Vec::new();

    // Add length prefix (4 bytes, big-endian)
    data_to_embed.extend_from_slice(&length.to_be_bytes());
    // Add secret image content
    data_to_embed.extend_from_slice(secret_image_bytes);

    // Check if carrier image has enough capacity
    // Each pixel has 3 usable channels (R, G, B), so 3 bits per pixel
    let available_bits = (width * height * 3) as usize;
    let required_bits = data_to_embed.len() * 8;

    if required_bits > available_bits {
        return Err(anyhow::anyhow!(
            "Carrier image too small: need {} bits but only have {} bits available. Secret image size: {} bytes",
            required_bits, available_bits, secret_image_bytes.len()
        ));
    }

    // Embed data into LSBs of image pixels
    let mut data_index = 0; // Current byte being embedded
    let mut bit_index = 0;  // Current bit within the byte (0-7)

    'outer: for y in 0..height {
        for x in 0..width {
            // Stop if all data has been embedded
            if data_index >= data_to_embed.len() {
                break 'outer;
            }

            let pixel = img.get_pixel(x, y);
            let mut new_pixel = *pixel;

            // Embed into R, G, B channels (skip Alpha channel for compatibility)
            for channel in 0..3 {
                if data_index >= data_to_embed.len() {
                    break 'outer;
                }

                // Extract the current bit from data (MSB first)
                let bit = (data_to_embed[data_index] >> (7 - bit_index)) & 1;

                // Clear LSB and set it to our data bit
                new_pixel[channel] = (pixel[channel] & 0xFE) | bit;

                // Move to next bit
                bit_index += 1;
                if bit_index == 8 {
                    bit_index = 0;
                    data_index += 1;
                }
            }

            img.put_pixel(x, y, new_pixel);
        }
    }

    // Encode the modified image as PNG
    let mut output_bytes = Vec::new();
    img.write_to(
        &mut std::io::Cursor::new(&mut output_bytes),
        image::ImageFormat::Png,
    )?;

    Ok(output_bytes)
}

/// Extract an embedded image from a carrier image using LSB steganography.
///
/// Reads the 4-byte length prefix, then extracts that many bytes from the
/// LSBs of the carrier image's RGB channels.
///
/// # Arguments
/// - `carrier_image_bytes`: Raw bytes of the steganography-encoded carrier image
///
/// # Returns
/// - `Ok(Vec<u8>)`: The extracted secret image bytes
/// - `Err`: If image can't be loaded or extraction fails
///
/// # Errors
/// - Image format is invalid
/// - Length prefix is corrupted
/// - Not enough data in the image
///
/// # Example
/// ```ignore
/// let carrier = std::fs::read("carrier_with_secret.png")?;
/// let secret_image = extract_image_bytes(&carrier)?;
/// std::fs::write("extracted_secret.png", secret_image)?;
/// ```
pub fn extract_image_bytes(carrier_image_bytes: &[u8]) -> Result<Vec<u8>> {
    // Load the carrier image
    let img = image::load_from_memory(carrier_image_bytes)?;
    let img = img.to_rgba8();
    let (width, height) = img.dimensions();

    // ========== STEP 1: Extract length (first 4 bytes = 32 bits) ==========

    let mut length_bytes = [0u8; 4];
    let mut data_index = 0;
    let mut bit_index = 0;

    'length_loop: for y in 0..height {
        for x in 0..width {
            if data_index >= 4 {
                break 'length_loop;
            }

            let pixel = img.get_pixel(x, y);

            // Extract from R, G, B channels
            for channel in 0..3 {
                if data_index >= 4 {
                    break 'length_loop;
                }

                // Get the LSB from this channel
                let bit = pixel[channel] & 1;

                // Set this bit in our length bytes (MSB first)
                length_bytes[data_index] |= bit << (7 - bit_index);

                bit_index += 1;
                if bit_index == 8 {
                    bit_index = 0;
                    data_index += 1;
                }
            }
        }
    }

    let length = u32::from_be_bytes(length_bytes) as usize;

    // ========== STEP 2: Extract image data ==========

    let mut image_bytes = vec![0u8; length];
    data_index = 0;
    bit_index = 0;
    let mut skip_bits = 32; // Skip the length prefix we already read

    'outer: for y in 0..height {
        for x in 0..width {
            if data_index >= length {
                break 'outer;
            }

            let pixel = img.get_pixel(x, y);

            for channel in 0..3 {
                // Skip the first 32 bits (length prefix)
                if skip_bits > 0 {
                    skip_bits -= 1;
                    continue;
                }

                if data_index >= length {
                    break 'outer;
                }

                // Get the LSB from this channel
                let bit = pixel[channel] & 1;

                // Set this bit in our image bytes (MSB first)
                image_bytes[data_index] |= bit << (7 - bit_index);

                bit_index += 1;
                if bit_index == 8 {
                    bit_index = 0;
                    data_index += 1;
                }
            }
        }
    }

    Ok(image_bytes)
}

/// Embed a secret image AND access rights into a carrier image using LSB steganography.
///
/// Data format: [4 bytes: secret_len][secret_image_bytes][4 bytes: access_len][access_rights_json]
///
/// # Arguments
/// - `carrier_image_bytes`: Raw bytes of the carrier image
/// - `secret_image_bytes`: Raw bytes of the secret image to embed
/// - `access_rights`: Optional access rights for a specific user
///
/// # Returns
/// - `Ok(Vec<u8>)`: PNG image bytes with embedded secret image and access rights
///
/// # Example
/// ```ignore
/// let carrier = std::fs::read("carrier.jpg")?;
/// let secret = std::fs::read("secret.png")?;
/// let rights = EmbeddedAccessRights {
///     username: "client2".to_string(),
///     view_limit: 5,
///     view_count: 0,
/// };
/// let result = embed_image_with_access_rights(&carrier, &secret, Some(&rights))?;
/// std::fs::write("personalized_carrier.png", result)?;
/// ```
pub fn embed_image_with_access_rights(
    carrier_image_bytes: &[u8],
    secret_image_bytes: &[u8],
    access_rights: Option<&EmbeddedAccessRights>,
) -> Result<Vec<u8>> {
    // Load the carrier image
    let img = image::load_from_memory(carrier_image_bytes)?;
    let (width, height) = img.dimensions();

    // Convert to RGBA format
    let mut img = img.to_rgba8();

    // Serialize access rights if provided
    let access_json = match access_rights {
        Some(rights) => serde_json::to_vec(rights)?,
        None => Vec::new(),
    };

    let secret_len = secret_image_bytes.len() as u32;
    let access_len = access_json.len() as u32;

    // Prepare data: [secret_len: 4][secret_bytes][access_len: 4][access_json]
    let mut data_to_embed = Vec::new();
    data_to_embed.extend_from_slice(&secret_len.to_be_bytes());
    data_to_embed.extend_from_slice(secret_image_bytes);
    data_to_embed.extend_from_slice(&access_len.to_be_bytes());
    data_to_embed.extend_from_slice(&access_json);

    // Check capacity
    let available_bits = (width * height * 3) as usize;
    let required_bits = data_to_embed.len() * 8;

    if required_bits > available_bits {
        return Err(anyhow::anyhow!(
            "Carrier image too small: need {} bits but only have {} bits available",
            required_bits, available_bits
        ));
    }

    // Embed data into LSBs
    let mut data_index = 0;
    let mut bit_index = 0;

    'outer: for y in 0..height {
        for x in 0..width {
            if data_index >= data_to_embed.len() {
                break 'outer;
            }

            let pixel = img.get_pixel(x, y);
            let mut new_pixel = *pixel;

            for channel in 0..3 {
                if data_index >= data_to_embed.len() {
                    break 'outer;
                }

                let bit = (data_to_embed[data_index] >> (7 - bit_index)) & 1;
                new_pixel[channel] = (pixel[channel] & 0xFE) | bit;

                bit_index += 1;
                if bit_index == 8 {
                    bit_index = 0;
                    data_index += 1;
                }
            }

            img.put_pixel(x, y, new_pixel);
        }
    }

    // Encode as PNG
    let mut output_bytes = Vec::new();
    img.write_to(
        &mut std::io::Cursor::new(&mut output_bytes),
        image::ImageFormat::Png,
    )?;

    Ok(output_bytes)
}

/// Extract secret image and access rights from a carrier image.
///
/// # Returns
/// - Tuple of (secret_image_bytes, optional_access_rights)
///
/// # Example
/// ```ignore
/// let carrier = std::fs::read("personalized_carrier.png")?;
/// let (secret_image, access_rights) = extract_image_with_access_rights(&carrier)?;
/// if let Some(rights) = access_rights {
///     println!("User: {}, views: {}/{}", rights.username, rights.view_count, rights.view_limit);
/// }
/// std::fs::write("extracted_secret.png", secret_image)?;
/// ```
pub fn extract_image_with_access_rights(
    carrier_image_bytes: &[u8],
) -> Result<(Vec<u8>, Option<EmbeddedAccessRights>)> {
    let img = image::load_from_memory(carrier_image_bytes)?;
    let img = img.to_rgba8();
    let (width, height) = img.dimensions();

    // Extract secret_len (first 4 bytes = 32 bits)
    let mut secret_len_bytes = [0u8; 4];
    let mut data_index = 0;
    let mut bit_index = 0;

    'length_loop: for y in 0..height {
        for x in 0..width {
            if data_index >= 4 {
                break 'length_loop;
            }

            let pixel = img.get_pixel(x, y);

            for channel in 0..3 {
                if data_index >= 4 {
                    break 'length_loop;
                }

                let bit = pixel[channel] & 1;
                secret_len_bytes[data_index] |= bit << (7 - bit_index);

                bit_index += 1;
                if bit_index == 8 {
                    bit_index = 0;
                    data_index += 1;
                }
            }
        }
    }

    let secret_len = u32::from_be_bytes(secret_len_bytes) as usize;

    // Extract secret image bytes
    let mut secret_bytes = vec![0u8; secret_len];
    data_index = 0;
    bit_index = 0;
    let mut skip_bits = 32; // Skip the secret_len we already read

    'secret_loop: for y in 0..height {
        for x in 0..width {
            if data_index >= secret_len {
                break 'secret_loop;
            }

            let pixel = img.get_pixel(x, y);

            for channel in 0..3 {
                if skip_bits > 0 {
                    skip_bits -= 1;
                    continue;
                }

                if data_index >= secret_len {
                    break 'secret_loop;
                }

                let bit = pixel[channel] & 1;
                secret_bytes[data_index] |= bit << (7 - bit_index);

                bit_index += 1;
                if bit_index == 8 {
                    bit_index = 0;
                    data_index += 1;
                }
            }
        }
    }

    // Extract access_len (next 4 bytes)
    let total_bits_read = 32 + (secret_len * 8);
    let mut access_len_bytes = [0u8; 4];
    data_index = 0;
    bit_index = 0;
    skip_bits = total_bits_read;

    'access_len_loop: for y in 0..height {
        for x in 0..width {
            if data_index >= 4 {
                break 'access_len_loop;
            }

            let pixel = img.get_pixel(x, y);

            for channel in 0..3 {
                if skip_bits > 0 {
                    skip_bits -= 1;
                    continue;
                }

                if data_index >= 4 {
                    break 'access_len_loop;
                }

                let bit = pixel[channel] & 1;
                access_len_bytes[data_index] |= bit << (7 - bit_index);

                bit_index += 1;
                if bit_index == 8 {
                    bit_index = 0;
                    data_index += 1;
                }
            }
        }
    }

    let access_len = u32::from_be_bytes(access_len_bytes) as usize;

    // Extract access rights JSON if present
    let access_rights = if access_len > 0 {
        let mut access_bytes = vec![0u8; access_len];
        data_index = 0;
        bit_index = 0;
        skip_bits = total_bits_read + 32; // Skip secret + secret_len + access_len

        'access_loop: for y in 0..height {
            for x in 0..width {
                if data_index >= access_len {
                    break 'access_loop;
                }

                let pixel = img.get_pixel(x, y);

                for channel in 0..3 {
                    if skip_bits > 0 {
                        skip_bits -= 1;
                        continue;
                    }

                    if data_index >= access_len {
                        break 'access_loop;
                    }

                    let bit = pixel[channel] & 1;
                    access_bytes[data_index] |= bit << (7 - bit_index);

                    bit_index += 1;
                    if bit_index == 8 {
                        bit_index = 0;
                        data_index += 1;
                    }
                }
            }
        }

        match serde_json::from_slice::<EmbeddedAccessRights>(&access_bytes) {
            Ok(rights) => Some(rights),
            Err(_) => None,
        }
    } else {
        None
    };

    Ok((secret_bytes, access_rights))
}
