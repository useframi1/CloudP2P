use anyhow::Result;
use image::{GenericImage, GenericImageView};

/// Embed text into an image using LSB steganography
pub fn embed_text(image_path: &str, text: &str, output_path: &str) -> Result<()> {
    let mut img = image::open(image_path)?;
    let (width, height) = img.dimensions();

    // Convert text to binary with length prefix
    let text_bytes = text.as_bytes();
    let length = text_bytes.len() as u32;
    let mut data_to_embed = Vec::new();

    // Add length (4 bytes)
    data_to_embed.extend_from_slice(&length.to_be_bytes());
    // Add text
    data_to_embed.extend_from_slice(text_bytes);

    // Check if image is large enough
    if data_to_embed.len() * 8 > (width * height * 3) as usize {
        return Err(anyhow::anyhow!("Image too small for this text"));
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
            let mut new_pixel = pixel;

            // Embed into R, G, B channels
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

    img.save(output_path)?;
    Ok(())
}

/// Extract text from an image using LSB steganography
pub fn extract_text(image_path: &str) -> Result<String> {
    let img = image::open(image_path)?;
    let (width, height) = img.dimensions();

    // Extract length (first 4 bytes = 32 bits)
    let mut length_bytes = [0u8; 4];
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

    // Extract text data
    let mut text_bytes = vec![0u8; length];
    data_index = 0;
    bit_index = 0;
    let mut skip_bits = 32; // Skip the length we already read

    'outer: for y in 0..height {
        for x in 0..width {
            if data_index >= length {
                break 'outer;
            }

            let pixel = img.get_pixel(x, y);

            for channel in 0..3 {
                if skip_bits > 0 {
                    skip_bits -= 1;
                    continue;
                }

                if data_index >= length {
                    break 'outer;
                }

                let bit = pixel[channel] & 1;
                text_bytes[data_index] |= bit << (7 - bit_index);

                bit_index += 1;
                if bit_index == 8 {
                    bit_index = 0;
                    data_index += 1;
                }
            }
        }
    }

    Ok(String::from_utf8(text_bytes)?)
}
