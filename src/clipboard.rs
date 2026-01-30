//! Clipboard image reading and file-based image ingestion.
//!
//! Self-contained module for reading images from the system clipboard or from
//! file paths. Handles PNG encoding, hashing, and base64 encoding.
//! No coupling to UI, networking, or application state.

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use sha2::{Sha256, Digest};

/// Maximum image size in bytes (4 MB).
const MAX_IMAGE_BYTES: usize = 4 * 1024 * 1024;

/// Maximum number of pending images per submission.
pub const MAX_PENDING_IMAGES: usize = 3;

/// Image file extensions we recognize for drag-and-drop detection.
const IMAGE_EXTENSIONS: &[&str] = &[".png", ".jpg", ".jpeg", ".gif", ".webp"];

/// A pending image attachment stored in memory until submit.
#[derive(Debug, Clone)]
pub struct ImageAttachment {
    /// Short hash for display and dedup (first 8 hex chars of sha256).
    pub hash: String,
    /// PNG-encoded image as base64 (no data URI prefix).
    pub base64_png: String,
    /// Original byte size of the PNG.
    pub byte_size: usize,
}

/// Errors that can occur when reading an image.
#[derive(Debug)]
pub enum ClipboardImageError {
    /// No image found in the clipboard.
    NoImage,
    /// Image exceeds the 4MB size limit.
    TooLarge(usize),
    /// Failed to encode image to PNG.
    EncodeFailed(String),
    /// Clipboard access failed.
    ClipboardError(String),
    /// File read failed.
    FileError(String),
}

/// Try to read an image from the system clipboard.
///
/// Uses `arboard` to access OS-level clipboard (NSPasteboard on macOS,
/// X11/Wayland on Linux). Returns the image as a base64-encoded PNG
/// with a short sha256 hash for display.
pub fn try_read_clipboard_image() -> Result<ImageAttachment, ClipboardImageError> {
    let mut clipboard = arboard::Clipboard::new()
        .map_err(|e| ClipboardImageError::ClipboardError(e.to_string()))?;

    let image_data = clipboard
        .get_image()
        .map_err(|_| ClipboardImageError::NoImage)?;

    let png_bytes = encode_rgba_to_png(
        &image_data.bytes,
        image_data.width as u32,
        image_data.height as u32,
    )?;

    build_attachment(png_bytes)
}

/// Try to read an image from a file path.
///
/// Used for drag-and-drop: terminals paste file paths as text when files
/// are dropped. This reads the file, validates it's an image, and encodes
/// it to PNG.
pub fn try_read_image_file(path: &str) -> Result<ImageAttachment, ClipboardImageError> {
    let path = path.trim();

    let bytes = std::fs::read(path)
        .map_err(|e| ClipboardImageError::FileError(e.to_string()))?;

    // If it's already PNG, use directly. Otherwise try to decode and re-encode as PNG.
    let png_bytes = if is_png(&bytes) {
        bytes
    } else {
        let img = image::load_from_memory(&bytes)
            .map_err(|e| ClipboardImageError::EncodeFailed(e.to_string()))?;
        let mut buf = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut buf);
        img.write_to(&mut cursor, image::ImageFormat::Png)
            .map_err(|e| ClipboardImageError::EncodeFailed(e.to_string()))?;
        buf
    };

    build_attachment(png_bytes)
}

/// Check if pasted text looks like a single image file path.
///
/// Returns true if the text is a single line ending with a recognized
/// image extension (.png, .jpg, .jpeg, .gif, .webp).
pub fn is_image_file_path(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.contains('\n') {
        return false;
    }
    let lower = trimmed.to_lowercase();
    IMAGE_EXTENSIONS.iter().any(|ext| lower.ends_with(ext))
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Build an ImageAttachment from raw PNG bytes.
fn build_attachment(png_bytes: Vec<u8>) -> Result<ImageAttachment, ClipboardImageError> {
    if png_bytes.len() > MAX_IMAGE_BYTES {
        return Err(ClipboardImageError::TooLarge(png_bytes.len()));
    }

    let hash = compute_short_hash(&png_bytes);
    let base64_png = BASE64.encode(&png_bytes);
    let byte_size = png_bytes.len();

    Ok(ImageAttachment {
        hash,
        base64_png,
        byte_size,
    })
}

/// Encode RGBA pixel data to PNG bytes.
fn encode_rgba_to_png(
    rgba: &[u8],
    width: u32,
    height: u32,
) -> Result<Vec<u8>, ClipboardImageError> {
    use image::{ImageBuffer, RgbaImage};

    let img: RgbaImage = ImageBuffer::from_raw(width, height, rgba.to_vec())
        .ok_or_else(|| ClipboardImageError::EncodeFailed("Invalid RGBA buffer dimensions".into()))?;

    let mut buf = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut buf);
    img.write_to(&mut cursor, image::ImageFormat::Png)
        .map_err(|e| ClipboardImageError::EncodeFailed(e.to_string()))?;

    Ok(buf)
}

/// Compute the first 8 hex characters of the SHA-256 hash.
fn compute_short_hash(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    hex::encode(&result[..4]) // 4 bytes = 8 hex chars
}

/// Quick check for PNG magic bytes.
fn is_png(data: &[u8]) -> bool {
    data.len() >= 8 && data[..8] == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_image_file_path() {
        assert!(is_image_file_path("/Users/sam/screenshot.png"));
        assert!(is_image_file_path("/tmp/photo.JPG"));
        assert!(is_image_file_path("./test.jpeg"));
        assert!(is_image_file_path("image.webp"));
        assert!(is_image_file_path("  /path/with spaces/file.gif  "));

        assert!(!is_image_file_path(""));
        assert!(!is_image_file_path("hello world"));
        assert!(!is_image_file_path("/path/to/file.txt"));
        assert!(!is_image_file_path("/path/to/file.rs"));
        assert!(!is_image_file_path("line1.png\nline2.png"));
    }

    #[test]
    fn test_is_png() {
        let png_header = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00];
        assert!(is_png(&png_header));

        let not_png = [0xFF, 0xD8, 0xFF, 0xE0]; // JPEG header
        assert!(!is_png(&not_png));

        assert!(!is_png(&[0x89])); // Too short
    }

    #[test]
    fn test_compute_short_hash() {
        let hash = compute_short_hash(b"test data");
        assert_eq!(hash.len(), 8);
        // Deterministic
        assert_eq!(hash, compute_short_hash(b"test data"));
        // Different data → different hash
        assert_ne!(hash, compute_short_hash(b"other data"));
    }

    #[test]
    fn test_build_attachment_too_large() {
        let big = vec![0u8; MAX_IMAGE_BYTES + 1];
        match build_attachment(big) {
            Err(ClipboardImageError::TooLarge(size)) => {
                assert!(size > MAX_IMAGE_BYTES);
            }
            _ => panic!("Expected TooLarge error"),
        }
    }

    #[test]
    fn test_build_attachment_ok() {
        // Create a minimal valid PNG-like payload (just raw bytes for hashing test)
        let data = vec![0u8; 100];
        let result = build_attachment(data);
        assert!(result.is_ok());
        let attachment = result.unwrap();
        assert_eq!(attachment.hash.len(), 8);
        assert_eq!(attachment.byte_size, 100);
        assert!(!attachment.base64_png.is_empty());
    }
}
