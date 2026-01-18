//! Style constants and OSC 8 hyperlink utilities for markdown rendering

use ratatui::style::{Color, Modifier, Style};

/// Style for code blocks - gray/dim color
pub const STYLE_CODE_BLOCK: Style = Style::new().fg(Color::DarkGray);

/// Style for inline code - cyan color
pub const STYLE_INLINE_CODE: Style = Style::new().fg(Color::Cyan);

/// Style for headings - cyan and bold
pub const STYLE_HEADING: Style = Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD);

/// Style for links - blue and underlined
pub const STYLE_LINK: Style = Style::new()
    .fg(Color::Blue)
    .add_modifier(Modifier::UNDERLINED);

/// Create an OSC 8 hyperlink escape sequence that wraps text
///
/// OSC 8 format: `\x1B]8;;{url}\x07{text}\x1B]8;;\x07`
/// This creates a clickable hyperlink in supported terminals (iTerm2, Konsole, etc.)
///
/// # Arguments
/// * `url` - The destination URL
/// * `text` - The display text for the link
///
/// # Returns
/// A string with the text wrapped in OSC 8 escape sequences
pub fn wrap_osc8_hyperlink(url: &str, text: &str) -> String {
    // OSC 8 format: ESC ] 8 ; ; url BEL text ESC ] 8 ; ; BEL
    // ESC = \x1B, BEL = \x07
    format!("\x1b]8;;{}\x07{}\x1b]8;;\x07", url, text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_osc8_hyperlink_format() {
        let result = wrap_osc8_hyperlink("https://example.com", "Click here");
        // OSC 8 format: ESC ] 8 ; ; url BEL text ESC ] 8 ; ; BEL
        assert_eq!(
            result,
            "\x1b]8;;https://example.com\x07Click here\x1b]8;;\x07"
        );
    }

    #[test]
    fn test_wrap_osc8_hyperlink_empty_text() {
        let result = wrap_osc8_hyperlink("https://example.com", "");
        assert_eq!(result, "\x1b]8;;https://example.com\x07\x1b]8;;\x07");
    }

    #[test]
    fn test_wrap_osc8_hyperlink_complex_url() {
        let url = "https://github.com/user/repo/issues?q=is:open&label=bug";
        let result = wrap_osc8_hyperlink(url, "issues");
        assert!(result.starts_with("\x1b]8;;"));
        assert!(result.contains(url));
        assert!(result.contains("issues"));
        assert!(result.ends_with("\x1b]8;;\x07"));
    }

    #[test]
    fn test_wrap_osc8_hyperlink_url_as_text() {
        // When URL is used as display text (plain URLs)
        let url = "https://example.com";
        let result = wrap_osc8_hyperlink(url, url);
        assert_eq!(result, format!("\x1b]8;;{}\x07{}\x1b]8;;\x07", url, url));
    }
}
