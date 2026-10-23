//! Local Steve/Alex-style faces when skin CDNs are unreachable (common on RU IPs).

use std::io::Cursor;

use image::{DynamicImage, GenericImageView, ImageBuffer, ImageFormat, Rgba, imageops};

/// Crop the front head (+ hat overlay) from a classic/slim Minecraft skin PNG → 64×64.
pub fn face_from_skin_png(skin_bytes: &[u8]) -> Option<Vec<u8>> {
    let img = image::load_from_memory(skin_bytes).ok()?;
    let (w, h) = img.dimensions();
    if w < 64 || h < 32 {
        return None;
    }
    // Classic skin head front is at (8,8) size 8×8; hat overlay at (40,8).
    let scale = w / 64;
    let hx = 8 * scale;
    let hy = 8 * scale;
    let hs = 8 * scale;
    let ox = 40 * scale;
    let oy = 8 * scale;
    let head = img.crop_imm(hx, hy, hs, hs);
    let hat = img.crop_imm(ox, oy, hs, hs);
    let mut composed = head.to_rgba8();
    imageops::overlay(&mut composed, &hat.to_rgba8(), 0, 0);
    let scaled = imageops::resize(&composed, 64, 64, imageops::FilterType::Nearest);
    encode_png(&DynamicImage::ImageRgba8(scaled))
}

fn encode_png(img: &DynamicImage) -> Option<Vec<u8>> {
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, ImageFormat::Png).ok()?;
    Some(out.into_inner())
}

/// Deterministic 64×64 PNG head from username/uuid seed. Never the launcher icon.
pub fn procedural_face_png(seed: &str) -> Vec<u8> {
    let mut hash: u32 = 2166136261;
    for byte in seed.bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(16777619);
    }
    let alex = hash & 1 == 1;
    let skin = if alex {
        Rgba([0xF2_u8, 0xC0_u8, 0x9A_u8, 0xFF])
    } else {
        Rgba([0xC6_u8, 0x86_u8, 0x42_u8, 0xFF])
    };
    let hair = if alex {
        Rgba([0x3B_u8, 0x2A_u8, 0x1A_u8, 0xFF])
    } else {
        Rgba([0x59_u8, 0x3D_u8, 0x29_u8, 0xFF])
    };
    let shade = Rgba([
        skin[0].saturating_sub(28),
        skin[1].saturating_sub(28),
        skin[2].saturating_sub(28),
        255,
    ]);
    let eye_white = Rgba([0xF0, 0xF0, 0xF0, 0xFF]);
    let eye = Rgba([
        40 + ((hash >> 8) & 0x3F) as u8,
        60 + ((hash >> 14) & 0x3F) as u8,
        90 + ((hash >> 20) & 0x3F) as u8,
        255,
    ]);
    let mouth = Rgba([0x6A, 0x3A, 0x2A, 0xFF]);

    let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_fn(8, 8, |x, y| {
        // Hair band
        if y <= 1 {
            return hair;
        }
        // Eyes
        if y == 3 && (x == 1 || x == 2 || x == 5 || x == 6) {
            return eye_white;
        }
        if y == 3 && (x == 2 || x == 5) {
            return eye;
        }
        // Brow / shade
        if y == 2 && (x == 1 || x == 2 || x == 5 || x == 6) {
            return shade;
        }
        // Mouth
        if y == 5 && x >= 2 && x <= 5 {
            return mouth;
        }
        // Side shade
        if x == 0 || x == 7 {
            return shade;
        }
        skin
    });
    let scaled = imageops::resize(&img, 64, 64, imageops::FilterType::Nearest);
    encode_png(&DynamicImage::ImageRgba8(scaled)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn procedural_face_is_png() {
        let bytes = procedural_face_png("Steve");
        assert!(bytes.len() > 64);
        assert_eq!(&bytes[0..8], b"\x89PNG\r\n\x1a\n");
        let other = procedural_face_png("Alex");
        assert_ne!(bytes, other);
    }
}
