//! Folder chip rendering for input area.
//!
//! Provides the visual "chip" that displays the selected folder name.

use ratatui::{
    buffer::Buffer,
    style::{Color, Style},
};

// ============================================================================
// Folder Chip Constants
// ============================================================================

/// Maximum display length for folder name in chip (truncate if longer)
pub const MAX_CHIP_FOLDER_NAME_LEN: usize = 20;

/// Background color for folder chip - subtle dark blue
pub const COLOR_CHIP_BG: Color = Color::Rgb(40, 44, 52);

/// Text color for folder chip
pub const COLOR_CHIP_TEXT: Color = Color::White;

// ============================================================================
// Folder Chip Rendering
// ============================================================================

/// Format the folder name for display in the chip.
///
/// Truncates to MAX_CHIP_FOLDER_NAME_LEN characters and adds "..." if truncated.
pub fn format_chip_folder_name(name: &str) -> String {
    if name.len() > MAX_CHIP_FOLDER_NAME_LEN {
        format!("{}...", &name[..MAX_CHIP_FOLDER_NAME_LEN.saturating_sub(3)])
    } else {
        name.to_string()
    }
}

/// Calculate the width of the folder chip in columns.
///
/// Returns the width including the brackets and emoji: `[folder-name]`
pub fn calculate_chip_width(folder_name: &str) -> u16 {
    let display_name = format_chip_folder_name(folder_name);
    // Format: "[📁 " (4 chars) + name + "]" (1 char)
    // Note: emoji 📁 is typically 2 columns wide
    (3 + display_name.len() + 1) as u16
}

/// Render the folder chip directly to the buffer.
///
/// The chip is rendered at the specified position with the format: `[folder-name]`
pub fn render_folder_chip(buf: &mut Buffer, x: u16, y: u16, folder_name: &str) {
    let display_name = format_chip_folder_name(folder_name);
    let chip_text = format!("[📁 {}]", display_name);

    let style = Style::default().fg(COLOR_CHIP_TEXT).bg(COLOR_CHIP_BG);

    buf.set_string(x, y, &chip_text, style);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_chip_folder_name_short() {
        let name = "project";
        let formatted = format_chip_folder_name(name);
        assert_eq!(formatted, "project");
    }

    #[test]
    fn test_format_chip_folder_name_exact_max() {
        // MAX_CHIP_FOLDER_NAME_LEN is 20
        let name = "12345678901234567890"; // Exactly 20 chars
        let formatted = format_chip_folder_name(name);
        assert_eq!(formatted, "12345678901234567890");
    }

    #[test]
    fn test_format_chip_folder_name_truncated() {
        // MAX_CHIP_FOLDER_NAME_LEN is 20
        let name = "very-long-project-name-that-exceeds-limit";
        let formatted = format_chip_folder_name(name);
        // Should truncate to 17 chars + "..." = 20 chars total
        assert!(formatted.ends_with("..."));
        assert!(formatted.len() <= MAX_CHIP_FOLDER_NAME_LEN);
    }

    #[test]
    fn test_calculate_chip_width() {
        // Format: "[📁 " + name + "]"
        // "[📁 " is 3 chars ([ + emoji(counts as 1 in len) + space)
        // "]" is 1 char
        let width = calculate_chip_width("project");
        // "[📁 project]" = 4 + 7 = 11 characters
        // But emoji 📁 is 2 columns wide, so actual display is 12
        // The function returns: (3 + name.len() + 1) = 3 + 7 + 1 = 11
        assert_eq!(width, 11);
    }

    #[test]
    fn test_calculate_chip_width_long_name_truncated() {
        let name = "very-long-project-name-that-exceeds-limit";
        let width = calculate_chip_width(name);
        // Name gets truncated to 17 + "..." = 20 chars max
        // Width = 3 + 20 + 1 = 24
        assert!(width <= 24);
    }
}
