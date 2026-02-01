//! Style constants and OSC 8 hyperlink utilities for markdown rendering

use once_cell::sync::Lazy;
use regex::Regex;
use ratatui::style::{Color, Modifier, Style};
use unicode_width::UnicodeWidthStr;

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

/// Remove OSC 8 escape sequences from a string, returning just the display text.
///
/// OSC 8 sequences have the format: `\x1b]8;;{url}\x07{text}\x1b]8;;\x07`
/// This function strips both the opening `\x1b]8;;{url}\x07` and closing `\x1b]8;;\x07` sequences.
///
/// # Arguments
/// * `s` - The string potentially containing OSC 8 sequences
///
/// # Returns
/// The string with all OSC 8 escape sequences removed
pub fn strip_osc8_sequences(s: &str) -> String {
    // Pattern: \x1b]8;;[^\x07]*\x07
    // This matches: ESC ] 8 ; ; (any chars except BEL) BEL
    static OSC8_REGEX: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"\x1b\]8;;[^\x07]*\x07").expect("Invalid OSC8 regex")
    });
    OSC8_REGEX.replace_all(s, "").to_string()
}

/// Calculate the display width of a string, ignoring ANSI/OSC escape sequences.
///
/// Escape sequences have zero display width in the terminal but contain characters
/// that would otherwise be counted. This function strips OSC 8 sequences before
/// calculating width using Unicode width rules.
///
/// # Arguments
/// * `s` - The string to measure
///
/// # Returns
/// The display width in terminal columns
pub fn display_width_ignoring_escapes(s: &str) -> usize {
    let stripped = strip_osc8_sequences(s);
    stripped.width()
}

/// Check if a string contains OSC 8 escape sequences
///
/// # Arguments
/// * `s` - The string to check
///
/// # Returns
/// `true` if the string contains OSC 8 sequences, `false` otherwise
pub fn contains_osc8_sequence(s: &str) -> bool {
    s.contains("\x1b]8;;")
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

    #[test]
    fn test_strip_osc8_sequences() {
        let text = "\x1b]8;;https://example.com\x07Click here\x1b]8;;\x07";
        let result = strip_osc8_sequences(text);
        assert_eq!(result, "Click here");
    }

    #[test]
    fn test_strip_osc8_sequences_multiple() {
        let text = "\x1b]8;;https://example.com\x07Link1\x1b]8;;\x07 and \x1b]8;;https://other.com\x07Link2\x1b]8;;\x07";
        let result = strip_osc8_sequences(text);
        assert_eq!(result, "Link1 and Link2");
    }

    #[test]
    fn test_strip_osc8_sequences_no_sequences() {
        let text = "Plain text without sequences";
        let result = strip_osc8_sequences(text);
        assert_eq!(result, "Plain text without sequences");
    }

    #[test]
    fn test_display_width_ignoring_escapes() {
        let text = "\x1b]8;;https://example.com\x07Click\x1b]8;;\x07";
        // Should only count "Click" = 5 chars
        assert_eq!(display_width_ignoring_escapes(text), 5);
    }

    #[test]
    fn test_display_width_ignoring_escapes_plain_text() {
        let text = "Hello";
        assert_eq!(display_width_ignoring_escapes(text), 5);
    }

    #[test]
    fn test_contains_osc8_sequence() {
        assert!(contains_osc8_sequence("\x1b]8;;https://example.com\x07text\x1b]8;;\x07"));
        assert!(!contains_osc8_sequence("Plain text"));
        assert!(contains_osc8_sequence("Some text \x1b]8;;url\x07link\x1b]8;;\x07 more"));
    }
}
