//! Image chip rendering for input area.
//!
//! Provides visual chips that display attached image indicators above the text area.
//! Follows the same pattern as `folder_chip.rs`.

use ratatui::{
    buffer::Buffer,
    style::{Color, Style},
};

use crate::clipboard::ImageAttachment;

// ============================================================================
// Image Chip Constants
// ============================================================================

/// Background color for image chip - subtle purple
pub const COLOR_IMAGE_CHIP_BG: Color = Color::Rgb(50, 40, 60);

/// Text color for image chip
pub const COLOR_IMAGE_CHIP_TEXT: Color = Color::White;

// ============================================================================
// Image Chip Rendering
// ============================================================================

/// Format the display text for a single image chip.
///
/// Returns a string like `[Image #1 a3f2b1c0]`.
pub fn format_image_chip_text(index: usize, hash: &str) -> String {
    let short_hash = &hash[..hash.len().min(8)];
    format!("[Image #{} {}]", index + 1, short_hash)
}

/// Calculate the total width of all image chips with spacing.
///
/// Each chip is separated by 1 space. Leading indent of 2 spaces.
pub fn calculate_image_chips_width(images: &[ImageAttachment]) -> u16 {
    if images.is_empty() {
        return 0;
    }
    let mut width: u16 = 2; // leading indent
    for (i, img) in images.iter().enumerate() {
        if i > 0 {
            width += 1; // space between chips
        }
        width += format_image_chip_text(i, &img.hash).len() as u16;
    }
    width
}

/// Render all image chips in a row directly to the buffer.
///
/// Renders at the specified position with purple background and white text.
pub fn render_image_chips(buf: &mut Buffer, x: u16, y: u16, images: &[ImageAttachment]) {
    if images.is_empty() {
        return;
    }

    let style = Style::default()
        .fg(COLOR_IMAGE_CHIP_TEXT)
        .bg(COLOR_IMAGE_CHIP_BG);

    let mut offset = x + 2; // 2-space leading indent
    for (i, img) in images.iter().enumerate() {
        if i > 0 {
            offset += 1; // space between chips
        }
        let chip_text = format_image_chip_text(i, &img.hash);
        let chip_len = chip_text.len() as u16;

        // Only render if there's room in the buffer
        if offset + chip_len <= buf.area().right() {
            buf.set_string(offset, y, &chip_text, style);
        }
        offset += chip_len;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_attachment(hash: &str) -> ImageAttachment {
        ImageAttachment {
            hash: hash.to_string(),
            base64_png: String::new(),
            byte_size: 0,
        }
    }

    #[test]
    fn test_format_image_chip_text() {
        assert_eq!(format_image_chip_text(0, "a3f2b1c0"), "[Image #1 a3f2b1c0]");
        assert_eq!(format_image_chip_text(1, "f7e1d09b"), "[Image #2 f7e1d09b]");
        assert_eq!(format_image_chip_text(2, "2c8a4e61"), "[Image #3 2c8a4e61]");
    }

    #[test]
    fn test_format_image_chip_text_long_hash_truncated() {
        let text = format_image_chip_text(0, "a3f2b1c0deadbeef");
        assert_eq!(text, "[Image #1 a3f2b1c0]"); // Only first 8 chars
    }

    #[test]
    fn test_calculate_image_chips_width_empty() {
        assert_eq!(calculate_image_chips_width(&[]), 0);
    }

    #[test]
    fn test_calculate_image_chips_width_one() {
        let images = vec![make_attachment("a3f2b1c0")];
        let width = calculate_image_chips_width(&images);
        // 2 (indent) + "[Image #1 a3f2b1c0]".len() = 2 + 19 = 21
        assert_eq!(width, 21);
    }

    #[test]
    fn test_calculate_image_chips_width_three() {
        let images = vec![
            make_attachment("a3f2b1c0"),
            make_attachment("f7e1d09b"),
            make_attachment("2c8a4e61"),
        ];
        let width = calculate_image_chips_width(&images);
        // 2 + 19 + 1 + 19 + 1 + 19 = 61
        assert_eq!(width, 61);
    }
}
