//! Text processing utilities for message content.

use once_cell::sync::Lazy;
use regex::Regex;

/// Regex pattern to match thread prefix: [Thread: <any-id>]\n\n
static THREAD_PREFIX_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\[Thread: [^\]]+\]\n\n").expect("Invalid thread prefix regex")
});

/// Strip the `[Thread: <id>]\n\n` prefix from message content if present.
///
/// This prefix is added by the backend for internal context but should not
/// be displayed to users.
pub fn strip_thread_prefix(content: &str) -> String {
    THREAD_PREFIX_REGEX.replace(content, "").into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_with_uuid() {
        let content = "[Thread: abc123-def456-ghi789]\n\nActual message";
        assert_eq!(strip_thread_prefix(content), "Actual message");
    }

    #[test]
    fn test_strip_no_prefix() {
        let content = "Message without prefix";
        assert_eq!(strip_thread_prefix(content), "Message without prefix");
    }

    #[test]
    fn test_strip_partial_match_one_newline() {
        // Only one newline - should NOT strip
        let content = "[Thread: abc123]\nOnly one newline";
        assert_eq!(strip_thread_prefix(content), content);
    }

    #[test]
    fn test_strip_prefix_not_at_start() {
        // Prefix in middle - should NOT strip
        let content = "Some text [Thread: abc123]\n\nMore text";
        assert_eq!(strip_thread_prefix(content), content);
    }

    #[test]
    fn test_strip_empty_content_after() {
        let content = "[Thread: abc123]\n\n";
        assert_eq!(strip_thread_prefix(content), "");
    }

    #[test]
    fn test_strip_multiline_message() {
        let content = "[Thread: abc123]\n\nLine 1\nLine 2\nLine 3";
        assert_eq!(strip_thread_prefix(content), "Line 1\nLine 2\nLine 3");
    }
}
