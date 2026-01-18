//! Link detection utilities for markdown rendering
//!
//! URL Detection:
//! - Detects plain text URLs using regex pattern `https?://[^\s<>\[\]]+`
//! - Returns `LinkInfo` metadata for rendering OSC 8 hyperlinks

use once_cell::sync::Lazy;
use ratatui::{style::Style, text::Span};
use regex::Regex;

use crate::markdown::styles::{wrap_osc8_hyperlink, STYLE_LINK};

/// Regex pattern for detecting plain text URLs (http:// or https://)
/// Matches URLs that don't contain whitespace, angle brackets, or square brackets
pub(crate) static URL_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"https?://[^\s<>\[\]]+").expect("Invalid URL regex pattern"));

/// Information about a detected link in markdown content
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkInfo {
    /// The URL/destination of the link
    pub url: String,
    /// The display text for the link
    pub text: String,
    /// Start byte position in the original text (for plain URLs)
    pub start: usize,
    /// End byte position in the original text (for plain URLs)
    pub end: usize,
}

impl LinkInfo {
    /// Create a new LinkInfo
    pub fn new(url: String, text: String, start: usize, end: usize) -> Self {
        Self {
            url,
            text,
            start,
            end,
        }
    }
}

/// Result of parsing markdown with link detection
#[derive(Debug, Clone)]
pub struct ParsedMarkdown {
    /// The rendered lines
    pub lines: Vec<ratatui::text::Line<'static>>,
    /// All links detected in the content (both markdown links and plain URLs)
    pub links: Vec<LinkInfo>,
}

/// Detect plain text URLs in a string using regex
/// Returns a vector of LinkInfo for each URL found
pub fn detect_plain_urls(text: &str) -> Vec<LinkInfo> {
    URL_REGEX
        .find_iter(text)
        .map(|m| {
            let url = m.as_str().to_string();
            LinkInfo::new(
                url.clone(),
                url, // For plain URLs, text and url are the same
                m.start(),
                m.end(),
            )
        })
        .collect()
}

/// Render text with plain URL detection
/// Splits text into spans where URLs are styled differently and tracked in links vec
pub(crate) fn render_text_with_urls(
    text: &str,
    base_style: Style,
    links: &mut Vec<LinkInfo>,
    base_position: usize,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut last_end = 0;

    for m in URL_REGEX.find_iter(text) {
        // Add text before the URL
        if m.start() > last_end {
            let before = &text[last_end..m.start()];
            if !before.is_empty() {
                spans.push(Span::styled(before.to_string(), base_style));
            }
        }

        // Add the URL with link style, wrapped in OSC 8 escape sequence for clickability
        let url = m.as_str().to_string();
        let osc8_text = wrap_osc8_hyperlink(&url, &url);
        spans.push(Span::styled(osc8_text, STYLE_LINK));

        // Track the link
        links.push(LinkInfo::new(
            url.clone(),
            url,
            base_position + m.start(),
            base_position + m.end(),
        ));

        last_end = m.end();
    }

    // Add remaining text after last URL
    if last_end < text.len() {
        let after = &text[last_end..];
        if !after.is_empty() {
            spans.push(Span::styled(after.to_string(), base_style));
        }
    }

    // If no URLs found, return the whole text as one span
    if spans.is_empty() && !text.is_empty() {
        spans.push(Span::styled(text.to_string(), base_style));
    }

    spans
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown::{render_markdown_with_links, wrap_osc8_hyperlink};
    use ratatui::style::{Color, Modifier};

    #[test]
    fn test_detect_plain_urls_http() {
        let urls = detect_plain_urls("Visit http://example.com for more info");
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].url, "http://example.com");
        assert_eq!(urls[0].text, "http://example.com");
        assert_eq!(urls[0].start, 6);
        assert_eq!(urls[0].end, 24);
    }

    #[test]
    fn test_detect_plain_urls_https() {
        let urls = detect_plain_urls("Visit https://example.com for more info");
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].url, "https://example.com");
        assert_eq!(urls[0].text, "https://example.com");
    }

    #[test]
    fn test_detect_plain_urls_with_path() {
        let urls = detect_plain_urls("Check https://github.com/user/repo/issues/123");
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].url, "https://github.com/user/repo/issues/123");
    }

    #[test]
    fn test_detect_plain_urls_with_query() {
        let urls = detect_plain_urls("Search at https://google.com/search?q=rust+programming");
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].url, "https://google.com/search?q=rust+programming");
    }

    #[test]
    fn test_detect_plain_urls_multiple() {
        let urls = detect_plain_urls("Visit https://one.com and https://two.com today");
        assert_eq!(urls.len(), 2);
        assert_eq!(urls[0].url, "https://one.com");
        assert_eq!(urls[1].url, "https://two.com");
    }

    #[test]
    fn test_detect_plain_urls_no_urls() {
        let urls = detect_plain_urls("This is plain text without URLs");
        assert!(urls.is_empty());
    }

    #[test]
    fn test_detect_plain_urls_only_url() {
        let urls = detect_plain_urls("https://example.com");
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].url, "https://example.com");
        assert_eq!(urls[0].start, 0);
        assert_eq!(urls[0].end, 19);
    }

    #[test]
    fn test_detect_plain_urls_not_ftp() {
        // Should not match ftp:// URLs
        let urls = detect_plain_urls("Visit ftp://files.example.com for files");
        assert!(urls.is_empty());
    }

    #[test]
    fn test_markdown_link_detection() {
        let parsed = render_markdown_with_links("Click [here](https://example.com) for info");
        assert_eq!(parsed.links.len(), 1);
        assert_eq!(parsed.links[0].url, "https://example.com");
        assert_eq!(parsed.links[0].text, "here");
    }

    #[test]
    fn test_markdown_link_styled_blue_underlined() {
        let parsed = render_markdown_with_links("Click [here](https://example.com) for info");

        // Find the link span - content is now wrapped with OSC 8 escape sequences
        let expected_osc8 = wrap_osc8_hyperlink("https://example.com", "here");
        let link_span = parsed.lines[0]
            .spans
            .iter()
            .find(|s| s.content == expected_osc8)
            .expect("Should have link text span with OSC 8 escape sequences");

        assert_eq!(link_span.style.fg, Some(Color::Blue));
        assert!(link_span.style.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn test_markdown_link_multiple() {
        let md = "Visit [GitHub](https://github.com) and [Rust](https://rust-lang.org)";
        let parsed = render_markdown_with_links(md);

        assert_eq!(parsed.links.len(), 2);
        assert_eq!(parsed.links[0].url, "https://github.com");
        assert_eq!(parsed.links[0].text, "GitHub");
        assert_eq!(parsed.links[1].url, "https://rust-lang.org");
        assert_eq!(parsed.links[1].text, "Rust");
    }

    #[test]
    fn test_plain_url_in_text() {
        let parsed = render_markdown_with_links("Visit https://example.com for more info");

        assert_eq!(parsed.links.len(), 1);
        assert_eq!(parsed.links[0].url, "https://example.com");
        assert_eq!(parsed.links[0].text, "https://example.com");
    }

    #[test]
    fn test_plain_url_styled_blue_underlined() {
        let parsed = render_markdown_with_links("Visit https://example.com for info");

        // Find the URL span - content is now wrapped with OSC 8 escape sequences
        let expected_osc8 = wrap_osc8_hyperlink("https://example.com", "https://example.com");
        let url_span = parsed.lines[0]
            .spans
            .iter()
            .find(|s| s.content == expected_osc8)
            .expect("Should have URL span with OSC 8 escape sequences");

        assert_eq!(url_span.style.fg, Some(Color::Blue));
        assert!(url_span.style.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn test_mixed_markdown_and_plain_urls() {
        let md = "See [docs](https://docs.rs) or https://github.com for code";
        let parsed = render_markdown_with_links(md);

        assert_eq!(parsed.links.len(), 2);
        // First link is from markdown
        assert_eq!(parsed.links[0].url, "https://docs.rs");
        assert_eq!(parsed.links[0].text, "docs");
        // Second link is plain URL
        assert_eq!(parsed.links[1].url, "https://github.com");
        assert_eq!(parsed.links[1].text, "https://github.com");
    }

    #[test]
    fn test_url_not_detected_in_code_block() {
        let md = "```\nhttps://example.com\n```";
        let parsed = render_markdown_with_links(md);

        // URL inside code block should NOT be detected as a link
        // (code blocks should preserve content as-is)
        assert!(
            parsed.links.is_empty(),
            "URLs in code blocks should not be detected as links"
        );
    }

    #[test]
    fn test_url_not_detected_in_inline_code() {
        let md = "Use `https://example.com` as the base URL";
        let parsed = render_markdown_with_links(md);

        // URL inside inline code should NOT be detected as a link
        assert!(
            parsed.links.is_empty(),
            "URLs in inline code should not be detected as links"
        );
    }

    #[test]
    fn test_link_info_struct() {
        let link = LinkInfo::new(
            "https://example.com".to_string(),
            "Example".to_string(),
            10,
            29,
        );

        assert_eq!(link.url, "https://example.com");
        assert_eq!(link.text, "Example");
        assert_eq!(link.start, 10);
        assert_eq!(link.end, 29);
    }

    #[test]
    fn test_link_info_equality() {
        let link1 = LinkInfo::new("https://a.com".to_string(), "A".to_string(), 0, 10);
        let link2 = LinkInfo::new("https://a.com".to_string(), "A".to_string(), 0, 10);
        let link3 = LinkInfo::new("https://b.com".to_string(), "B".to_string(), 0, 10);

        assert_eq!(link1, link2);
        assert_ne!(link1, link3);
    }

    #[test]
    fn test_parsed_markdown_struct() {
        let parsed = render_markdown_with_links("Hello [world](https://world.com)");

        assert!(!parsed.lines.is_empty());
        assert_eq!(parsed.links.len(), 1);
    }

    #[test]
    fn test_empty_link_text() {
        // Edge case: empty link text (valid markdown but unusual)
        let parsed = render_markdown_with_links("[](https://example.com)");

        // Should still detect the link even with empty text
        assert_eq!(parsed.links.len(), 1);
        assert_eq!(parsed.links[0].url, "https://example.com");
        assert!(parsed.links[0].text.is_empty());
    }

    #[test]
    fn test_url_with_fragment() {
        let parsed = render_markdown_with_links("See https://example.com/page#section for details");

        assert_eq!(parsed.links.len(), 1);
        assert_eq!(parsed.links[0].url, "https://example.com/page#section");
    }

    #[test]
    fn test_url_with_port() {
        let parsed = render_markdown_with_links("Server at http://localhost:8080/api");

        assert_eq!(parsed.links.len(), 1);
        assert_eq!(parsed.links[0].url, "http://localhost:8080/api");
    }

    #[test]
    fn test_url_stops_at_angle_bracket() {
        // URLs should not include angle brackets (common in markdown/email contexts)
        let parsed = render_markdown_with_links("Check <https://example.com> for info");

        assert_eq!(parsed.links.len(), 1);
        assert_eq!(parsed.links[0].url, "https://example.com");
    }

    #[test]
    fn test_url_stops_at_square_bracket() {
        // URLs should not include square brackets
        let urls = detect_plain_urls("See [https://example.com] for info");

        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].url, "https://example.com");
    }

    #[test]
    fn test_complex_url_with_params() {
        let url = "https://example.com/path?param1=value1&param2=value2";
        let urls = detect_plain_urls(&format!("Visit {} today", url));

        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].url, url);
    }

    #[test]
    fn test_markdown_link_contains_osc8_sequence() {
        let parsed = render_markdown_with_links("Check [docs](https://docs.rs)");

        // The span content should contain OSC 8 escape sequences
        let link_content: String = parsed.lines[0]
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();

        // Should contain the OSC 8 start sequence
        assert!(
            link_content.contains("\x1b]8;;"),
            "Should contain OSC 8 start sequence"
        );
        // Should contain the URL
        assert!(
            link_content.contains("https://docs.rs"),
            "Should contain the URL"
        );
        // Should contain the BEL terminator
        assert!(
            link_content.contains("\x07"),
            "Should contain BEL terminator"
        );
    }

    #[test]
    fn test_plain_url_contains_osc8_sequence() {
        let parsed = render_markdown_with_links("Visit https://github.com today");

        // The span content should contain OSC 8 escape sequences
        let all_content: String = parsed.lines[0]
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();

        // Should contain the OSC 8 start sequence
        assert!(
            all_content.contains("\x1b]8;;"),
            "Should contain OSC 8 start sequence"
        );
        // Should contain the URL
        assert!(
            all_content.contains("https://github.com"),
            "Should contain the URL"
        );
    }
}
