//! Markdown parser for terminal rendering
//!
//! Converts markdown text to styled ratatui Lines for display in the TUI.
//! Handles code blocks, inline code, bold, italic, headings, and hyperlinks.
//!
//! Includes a memoization layer (`MarkdownCache`) that caches parsed output
//! keyed by content hash to avoid re-parsing unchanged content.
//!
//! URL Detection:
//! - Detects markdown links `[text](url)` via pulldown_cmark events
//! - Detects plain text URLs using regex pattern `https?://[^\s<>\[\]]+`
//! - Returns `LinkInfo` metadata for rendering OSC 8 hyperlinks

use once_cell::sync::Lazy;
use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use regex::Regex;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

/// Regex pattern for detecting plain text URLs (http:// or https://)
/// Matches URLs that don't contain whitespace, angle brackets, or square brackets
static URL_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"https?://[^\s<>\[\]]+").expect("Invalid URL regex pattern")
});

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
        Self { url, text, start, end }
    }
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

/// Result of parsing markdown with link detection
#[derive(Debug, Clone)]
pub struct ParsedMarkdown {
    /// The rendered lines
    pub lines: Vec<Line<'static>>,
    /// All links detected in the content (both markdown links and plain URLs)
    pub links: Vec<LinkInfo>,
}

/// Maximum number of entries in the markdown cache before eviction
const MARKDOWN_CACHE_MAX_ENTRIES: usize = 500;

/// Cached result from markdown rendering
#[derive(Clone)]
struct CachedLines {
    /// The rendered lines
    lines: Vec<Line<'static>>,
}

/// Memoization cache for markdown rendering.
///
/// Caches parsed output keyed by a hash of the input content.
/// When the same content is requested, returns cached lines instead of re-parsing.
///
/// This is critical for performance because:
/// - `render_markdown()` creates a new `pulldown_cmark::Parser` for every call
/// - It parses markdown syntax, builds style stacks, and generates spans
/// - This happens up to 60 times/second for ALL visible messages
/// - By caching, completed messages never need re-parsing
pub struct MarkdownCache {
    /// Cache entries keyed by content hash
    entries: HashMap<u64, CachedLines>,
    /// Insertion order for LRU-style eviction (oldest first)
    insertion_order: Vec<u64>,
    /// Statistics: cache hits
    hits: u64,
    /// Statistics: cache misses
    misses: u64,
}

impl Default for MarkdownCache {
    fn default() -> Self {
        Self::new()
    }
}

impl MarkdownCache {
    /// Create a new empty markdown cache
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            insertion_order: Vec::new(),
            hits: 0,
            misses: 0,
        }
    }

    /// Compute a hash for the given content string
    fn hash_content(content: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        hasher.finish()
    }

    /// Render markdown with caching.
    ///
    /// If the content has been rendered before, returns the cached result.
    /// Otherwise, parses the markdown, caches the result, and returns it.
    pub fn render(&mut self, content: &str) -> Vec<Line<'static>> {
        let hash = Self::hash_content(content);

        // Check cache
        if let Some(cached) = self.entries.get(&hash) {
            self.hits += 1;
            return cached.lines.clone();
        }

        // Cache miss - render and store
        self.misses += 1;
        let lines = render_markdown(content);

        // Evict oldest entries if at capacity
        while self.entries.len() >= MARKDOWN_CACHE_MAX_ENTRIES && !self.insertion_order.is_empty() {
            let oldest_hash = self.insertion_order.remove(0);
            self.entries.remove(&oldest_hash);
        }

        // Store the new entry
        self.entries.insert(hash, CachedLines { lines: lines.clone() });
        self.insertion_order.push(hash);

        lines
    }

    /// Get cache statistics (hits, misses)
    pub fn stats(&self) -> (u64, u64) {
        (self.hits, self.misses)
    }

    /// Get the number of entries currently in the cache
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the cache is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all entries from the cache
    pub fn clear(&mut self) {
        self.entries.clear();
        self.insertion_order.clear();
        // Don't reset stats - they're useful for debugging
    }

    /// Invalidate a specific content entry (useful when content changes)
    pub fn invalidate(&mut self, content: &str) {
        let hash = Self::hash_content(content);
        if self.entries.remove(&hash).is_some() {
            self.insertion_order.retain(|&h| h != hash);
        }
    }
}

/// Style for code blocks - gray/dim color
const STYLE_CODE_BLOCK: Style = Style::new().fg(Color::DarkGray);

/// Style for inline code - cyan color
const STYLE_INLINE_CODE: Style = Style::new().fg(Color::Cyan);

/// Style for headings - cyan and bold
const STYLE_HEADING: Style = Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD);

/// Style for links - blue and underlined
const STYLE_LINK: Style = Style::new()
    .fg(Color::Blue)
    .add_modifier(Modifier::UNDERLINED);

/// Render markdown text to a vector of styled Lines.
///
/// Each newline in the input becomes a separate Line object, which is critical
/// for proper display of ASCII art and code blocks.
///
/// Supports:
/// - Code blocks (fenced with ```) - gray/dim color, preserves whitespace
/// - Inline code (`code`) - cyan color
/// - Bold (**text**) - bold modifier
/// - Italic (*text*) - italic modifier
/// - Headings (# Heading) - cyan and bold
///
/// Gracefully handles incomplete markdown during streaming by rendering
/// partial content without crashing.
pub fn render_markdown(text: &str) -> Vec<Line<'static>> {
    render_markdown_with_links(text).lines
}

/// Render markdown text to styled Lines with link detection.
///
/// Returns a `ParsedMarkdown` struct containing:
/// - `lines`: The rendered lines for display
/// - `links`: All detected links (markdown links and plain text URLs)
///
/// This is the primary function for rendering markdown when you need link
/// information for creating OSC 8 hyperlinks.
///
/// Supports:
/// - Code blocks (fenced with ```) - gray/dim color, preserves whitespace
/// - Inline code (`code`) - cyan color
/// - Bold (**text**) - bold modifier
/// - Italic (*text*) - italic modifier
/// - Headings (# Heading) - cyan and bold
/// - Markdown links [text](url) - blue and underlined
/// - Plain text URLs (http:// and https://) - detected via regex
///
/// Gracefully handles incomplete markdown during streaming by rendering
/// partial content without crashing.
pub fn render_markdown_with_links(text: &str) -> ParsedMarkdown {
    let parser = Parser::new(text);
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut current_spans: Vec<Span<'static>> = Vec::new();
    let mut links: Vec<LinkInfo> = Vec::new();

    // Style stack for nested formatting
    let mut style_stack: Vec<Style> = vec![Style::default()];
    let mut in_code_block = false;

    // Track current link context
    let mut current_link_url: Option<String> = None;
    let mut current_link_text = String::new();

    // Track byte position for plain URL detection
    let mut byte_position: usize = 0;

    for event in parser {
        match event {
            Event::Start(tag) => {
                match tag {
                    Tag::CodeBlock(_) => {
                        // Flush current line before code block
                        if !current_spans.is_empty() {
                            lines.push(Line::from(std::mem::take(&mut current_spans)));
                        }
                        in_code_block = true;
                        style_stack.push(STYLE_CODE_BLOCK);
                    }
                    Tag::Heading { .. } => {
                        style_stack.push(STYLE_HEADING);
                    }
                    Tag::Strong => {
                        let current = *style_stack.last().unwrap_or(&Style::default());
                        style_stack.push(current.add_modifier(Modifier::BOLD));
                    }
                    Tag::Emphasis => {
                        let current = *style_stack.last().unwrap_or(&Style::default());
                        style_stack.push(current.add_modifier(Modifier::ITALIC));
                    }
                    Tag::Paragraph => {
                        // Add blank line before paragraph (except first)
                        if !lines.is_empty() && !current_spans.is_empty() {
                            lines.push(Line::from(std::mem::take(&mut current_spans)));
                        }
                    }
                    Tag::Item => {
                        // Start of list item - flush current content to start new line
                        if !current_spans.is_empty() {
                            lines.push(Line::from(std::mem::take(&mut current_spans)));
                        }
                        // Add bullet point
                        let current_style = *style_stack.last().unwrap_or(&Style::default());
                        current_spans.push(Span::styled("• ".to_string(), current_style));
                    }
                    Tag::Link { dest_url, .. } => {
                        // Start of a markdown link - store URL and apply link style
                        current_link_url = Some(dest_url.to_string());
                        current_link_text.clear();
                        let current = *style_stack.last().unwrap_or(&Style::default());
                        // Combine current style with link style
                        style_stack.push(current.fg(Color::Blue).add_modifier(Modifier::UNDERLINED));
                    }
                    _ => {}
                }
            }
            Event::End(tag_end) => {
                match tag_end {
                    TagEnd::CodeBlock => {
                        // Flush any remaining code block content
                        if !current_spans.is_empty() {
                            lines.push(Line::from(std::mem::take(&mut current_spans)));
                        }
                        in_code_block = false;
                        style_stack.pop();
                    }
                    TagEnd::Heading(_) => {
                        // Flush heading line
                        if !current_spans.is_empty() {
                            lines.push(Line::from(std::mem::take(&mut current_spans)));
                        }
                        style_stack.pop();
                    }
                    TagEnd::Strong | TagEnd::Emphasis => {
                        style_stack.pop();
                    }
                    TagEnd::Paragraph => {
                        // Flush paragraph line
                        if !current_spans.is_empty() {
                            lines.push(Line::from(std::mem::take(&mut current_spans)));
                        }
                    }
                    TagEnd::Item => {
                        // End of list item - flush content to its own line
                        if !current_spans.is_empty() {
                            lines.push(Line::from(std::mem::take(&mut current_spans)));
                        }
                    }
                    TagEnd::Link => {
                        // End of markdown link - record the link info
                        if let Some(url) = current_link_url.take() {
                            links.push(LinkInfo::new(
                                url,
                                std::mem::take(&mut current_link_text),
                                0, // Position tracking for markdown links is not byte-based
                                0,
                            ));
                        }
                        style_stack.pop();
                    }
                    _ => {}
                }
            }
            Event::Text(text_content) => {
                let current_style = *style_stack.last().unwrap_or(&Style::default());
                let text_str = text_content.to_string();

                // If we're inside a link, accumulate the text
                if current_link_url.is_some() {
                    current_link_text.push_str(&text_str);
                }

                if in_code_block {
                    // In code blocks, preserve all whitespace and split on newlines
                    // Each newline becomes a separate Line object
                    let mut first = true;
                    for line_content in text_str.split('\n') {
                        if !first {
                            // Push the previous line and start a new one
                            lines.push(Line::from(std::mem::take(&mut current_spans)));
                        }
                        first = false;
                        if !line_content.is_empty() {
                            current_spans.push(Span::styled(
                                line_content.to_string(),
                                current_style,
                            ));
                        }
                    }
                } else if current_link_url.is_some() {
                    // Inside a markdown link - just render styled text (URL already tracked)
                    for (i, part) in text_str.split('\n').enumerate() {
                        if i > 0 {
                            lines.push(Line::from(std::mem::take(&mut current_spans)));
                            lines.push(Line::from(""));
                        }
                        if !part.is_empty() {
                            current_spans.push(Span::styled(part.to_string(), current_style));
                        }
                    }
                } else {
                    // Normal text - detect plain URLs and handle newlines
                    for (i, part) in text_str.split('\n').enumerate() {
                        if i > 0 {
                            // Flush current line and start new with blank line for separation
                            lines.push(Line::from(std::mem::take(&mut current_spans)));
                            lines.push(Line::from("")); // Visual separation
                        }
                        if !part.is_empty() {
                            // Check for plain text URLs and create styled spans
                            let spans_with_urls = render_text_with_urls(part, current_style, &mut links, byte_position);
                            current_spans.extend(spans_with_urls);
                        }
                        byte_position += part.len() + 1; // +1 for newline
                    }
                }
            }
            Event::Code(code) => {
                // Inline code - cyan color
                current_spans.push(Span::styled(code.to_string(), STYLE_INLINE_CODE));
            }
            Event::SoftBreak => {
                // Soft break - create new line with blank line for visual separation
                // This gives proper paragraph-like spacing in streamed content
                lines.push(Line::from(std::mem::take(&mut current_spans)));
                lines.push(Line::from("")); // Add blank line for visual separation
            }
            Event::HardBreak => {
                // Hard break - new line
                lines.push(Line::from(std::mem::take(&mut current_spans)));
            }
            _ => {}
        }
    }

    // Flush any remaining content
    if !current_spans.is_empty() {
        lines.push(Line::from(current_spans));
    }

    // Ensure we return at least one empty line for empty input
    if lines.is_empty() {
        lines.push(Line::from(""));
    }

    ParsedMarkdown { lines, links }
}

/// Render text with plain URL detection
/// Splits text into spans where URLs are styled differently and tracked in links vec
fn render_text_with_urls(
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

        // Add the URL with link style
        let url = m.as_str().to_string();
        spans.push(Span::styled(url.clone(), STYLE_LINK));

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

    #[test]
    fn test_plain_text() {
        let lines = render_markdown("Hello, world!");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].spans.len(), 1);
        assert_eq!(lines[0].spans[0].content, "Hello, world!");
    }

    #[test]
    fn test_empty_input() {
        let lines = render_markdown("");
        assert_eq!(lines.len(), 1);
        assert!(lines[0].spans.is_empty() || lines[0].spans[0].content.is_empty());
    }

    #[test]
    fn test_bold_text() {
        let lines = render_markdown("This is **bold** text");
        assert_eq!(lines.len(), 1);
        // Should have: "This is ", "bold", " text"
        assert!(lines[0].spans.len() >= 1);

        // Find the bold span
        let bold_span = lines[0]
            .spans
            .iter()
            .find(|s| s.content.contains("bold"))
            .expect("Should have bold span");
        assert!(bold_span.style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_italic_text() {
        let lines = render_markdown("This is *italic* text");
        assert_eq!(lines.len(), 1);

        // Find the italic span
        let italic_span = lines[0]
            .spans
            .iter()
            .find(|s| s.content.contains("italic"))
            .expect("Should have italic span");
        assert!(italic_span.style.add_modifier.contains(Modifier::ITALIC));
    }

    #[test]
    fn test_inline_code() {
        let lines = render_markdown("Use `cargo run` to start");
        assert_eq!(lines.len(), 1);

        // Find the code span
        let code_span = lines[0]
            .spans
            .iter()
            .find(|s| s.content == "cargo run")
            .expect("Should have inline code span");
        assert_eq!(code_span.style.fg, Some(Color::Cyan));
    }

    #[test]
    fn test_code_block() {
        let md = "```rust\nfn main() {\n    println!(\"Hello\");\n}\n```";
        let lines = render_markdown(md);

        // Should have multiple lines for the code block content
        assert!(lines.len() >= 3, "Code block should create multiple lines");

        // Check that code block lines have the gray/dim style
        for line in &lines {
            for span in &line.spans {
                if !span.content.is_empty() {
                    assert_eq!(
                        span.style.fg,
                        Some(Color::DarkGray),
                        "Code block should be gray/dim"
                    );
                }
            }
        }
    }

    #[test]
    fn test_code_block_preserves_whitespace() {
        let md = "```\n    indented\n        more indented\n```";
        let lines = render_markdown(md);

        // Find lines with indentation
        let has_indented = lines.iter().any(|line| {
            line.spans
                .iter()
                .any(|span| span.content.starts_with("    "))
        });
        assert!(has_indented, "Code block should preserve leading whitespace");
    }

    #[test]
    fn test_heading() {
        let lines = render_markdown("# Main Heading");
        assert_eq!(lines.len(), 1);

        // Heading should be cyan and bold
        let heading_span = &lines[0].spans[0];
        assert_eq!(heading_span.style.fg, Some(Color::Cyan));
        assert!(heading_span.style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_heading_levels() {
        let lines = render_markdown("## Second Level");
        assert_eq!(lines.len(), 1);

        let heading_span = &lines[0].spans[0];
        assert_eq!(heading_span.style.fg, Some(Color::Cyan));
        assert!(heading_span.style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_newlines_become_separate_lines() {
        // In code blocks, newlines become separate lines (critical for ASCII art)
        let md = "```\nLine 1\nLine 2\nLine 3\n```";
        let lines = render_markdown(md);

        // Each newline in code block should become a separate Line
        assert!(lines.len() >= 3, "Code block lines should be separate");

        // Also test hard breaks in regular text
        let md2 = "Line 1  \nLine 2  \nLine 3"; // Two spaces = hard break
        let lines2 = render_markdown(md2);
        assert!(lines2.len() >= 3, "Hard breaks should create separate lines");
    }

    #[test]
    fn test_ascii_art_in_code_block() {
        let md = r#"```
╔════════════════╗
║  ASCII Art     ║
╚════════════════╝
```"#;
        let lines = render_markdown(md);

        // Should preserve ASCII art characters on separate lines
        let has_box_chars = lines.iter().any(|line| {
            line.spans
                .iter()
                .any(|span| span.content.contains("╔") || span.content.contains("║"))
        });
        assert!(has_box_chars, "Should preserve ASCII box-drawing characters");
    }

    #[test]
    fn test_nested_bold_italic() {
        let lines = render_markdown("This is ***bold and italic*** text");
        assert_eq!(lines.len(), 1);

        // Find the nested span
        let nested_span = lines[0]
            .spans
            .iter()
            .find(|s| s.content.contains("bold and italic"))
            .expect("Should have nested span");
        assert!(nested_span.style.add_modifier.contains(Modifier::BOLD));
        assert!(nested_span.style.add_modifier.contains(Modifier::ITALIC));
    }

    #[test]
    fn test_incomplete_bold_during_streaming() {
        // Simulate streaming where bold is not yet closed
        let lines = render_markdown("This is **incomplete");
        // Should not panic, should render gracefully
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_incomplete_code_block_during_streaming() {
        // Simulate streaming where code block is not yet closed
        let lines = render_markdown("```rust\nfn incomplete(");
        // Should not panic, should render gracefully
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_incomplete_inline_code_during_streaming() {
        // Simulate streaming where inline code is not yet closed
        let lines = render_markdown("Use `incomplete");
        // Should not panic, should render gracefully
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_multiple_paragraphs() {
        let md = "First paragraph.\n\nSecond paragraph.";
        let lines = render_markdown(md);

        // Should have content from both paragraphs
        let all_content: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect();
        assert!(all_content.contains("First paragraph"));
        assert!(all_content.contains("Second paragraph"));
    }

    #[test]
    fn test_mixed_formatting() {
        let md = "Normal **bold** and `code` and *italic*";
        let lines = render_markdown(md);
        assert_eq!(lines.len(), 1);

        // Verify different styles exist
        let has_bold = lines[0]
            .spans
            .iter()
            .any(|s| s.style.add_modifier.contains(Modifier::BOLD));
        let has_italic = lines[0]
            .spans
            .iter()
            .any(|s| s.style.add_modifier.contains(Modifier::ITALIC));
        let has_cyan = lines[0]
            .spans
            .iter()
            .any(|s| s.style.fg == Some(Color::Cyan));

        assert!(has_bold, "Should have bold text");
        assert!(has_italic, "Should have italic text");
        assert!(has_cyan, "Should have cyan (inline code) text");
    }

    #[test]
    fn test_code_block_with_language() {
        let md = "```python\nprint('hello')\n```";
        let lines = render_markdown(md);

        // Should have the code content
        let has_print = lines
            .iter()
            .any(|l| l.spans.iter().any(|s| s.content.contains("print")));
        assert!(has_print, "Should have code block content");
    }

    #[test]
    fn test_empty_code_block() {
        let md = "```\n```";
        let lines = render_markdown(md);
        // Should not panic on empty code block
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_only_newlines() {
        let md = "\n\n\n";
        let lines = render_markdown(md);
        // Should handle gracefully
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_hard_break() {
        let md = "Line one  \nLine two"; // Two spaces before newline = hard break
        let lines = render_markdown(md);
        // Should create separate lines
        assert!(lines.len() >= 2, "Hard break should create new line");
    }

    #[test]
    fn test_soft_break_creates_line() {
        // GitHub-flavored markdown: single newline creates a line break
        let md = "Line one\nLine two";
        let lines = render_markdown(md);
        // Should create separate lines (not collapse to single line with space)
        assert!(
            lines.len() >= 2,
            "Soft break (single newline) should create new line, got {} lines",
            lines.len()
        );
    }

    #[test]
    fn test_gfm_style_line_breaks() {
        // Realistic multi-line content like streaming responses
        let md = "Here's what I found:\n\n1. First item\n2. Second item\n\nLet me explain...";
        let lines = render_markdown(md);

        // Should have multiple lines for the structure
        // Note: double newlines create paragraph breaks, exact count depends on parser
        assert!(
            lines.len() >= 3,
            "Multi-line content should preserve structure, got {} lines",
            lines.len()
        );

        // Verify content is present across lines
        let all_content: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect();
        assert!(all_content.contains("First item"));
        assert!(all_content.contains("Second item"));
        assert!(all_content.contains("explain"));
    }

    #[test]
    fn test_streaming_content_line_breaks() {
        // Simulates typical streaming response with line breaks
        let md = "Step 1: Do this\nStep 2: Do that\nStep 3: Done";
        let lines = render_markdown(md);

        // Each step should be on its own line
        assert!(
            lines.len() >= 3,
            "Each step should be on separate line, got {} lines",
            lines.len()
        );
    }

    // ========================================================================
    // MarkdownCache Tests
    // ========================================================================

    #[test]
    fn test_cache_new() {
        let cache = MarkdownCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
        assert_eq!(cache.stats(), (0, 0));
    }

    #[test]
    fn test_cache_default() {
        let cache = MarkdownCache::default();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_cache_hit_and_miss() {
        let mut cache = MarkdownCache::new();

        // First render - should be a miss
        let content = "Hello, **world**!";
        let lines1 = cache.render(content);
        let (hits, misses) = cache.stats();
        assert_eq!(hits, 0);
        assert_eq!(misses, 1);
        assert_eq!(cache.len(), 1);

        // Second render of same content - should be a hit
        let lines2 = cache.render(content);
        let (hits, misses) = cache.stats();
        assert_eq!(hits, 1);
        assert_eq!(misses, 1);
        assert_eq!(cache.len(), 1);

        // Results should be identical
        assert_eq!(lines1.len(), lines2.len());
        for (l1, l2) in lines1.iter().zip(lines2.iter()) {
            assert_eq!(l1.spans.len(), l2.spans.len());
            for (s1, s2) in l1.spans.iter().zip(l2.spans.iter()) {
                assert_eq!(s1.content, s2.content);
            }
        }
    }

    #[test]
    fn test_cache_different_content() {
        let mut cache = MarkdownCache::new();

        // Render different content
        cache.render("Content A");
        cache.render("Content B");
        cache.render("Content C");

        assert_eq!(cache.len(), 3);
        let (hits, misses) = cache.stats();
        assert_eq!(hits, 0);
        assert_eq!(misses, 3);
    }

    #[test]
    fn test_cache_clear() {
        let mut cache = MarkdownCache::new();

        // Add some entries
        cache.render("Content 1");
        cache.render("Content 2");
        assert_eq!(cache.len(), 2);

        // Clear the cache
        cache.clear();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);

        // Stats should be preserved (useful for debugging)
        let (_hits, misses) = cache.stats();
        assert_eq!(misses, 2);
    }

    #[test]
    fn test_cache_invalidate() {
        let mut cache = MarkdownCache::new();

        let content = "Some markdown content";
        cache.render(content);
        assert_eq!(cache.len(), 1);

        // Invalidate the entry
        cache.invalidate(content);
        assert!(cache.is_empty());

        // Re-render should be a miss
        cache.render(content);
        let (hits, misses) = cache.stats();
        assert_eq!(hits, 0);
        assert_eq!(misses, 2);
    }

    #[test]
    fn test_cache_invalidate_nonexistent() {
        let mut cache = MarkdownCache::new();
        cache.render("Existing content");

        // Invalidating non-existent content should be a no-op
        cache.invalidate("Non-existent content");
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_cache_eviction() {
        let mut cache = MarkdownCache::new();

        // Fill cache beyond max capacity
        for i in 0..(MARKDOWN_CACHE_MAX_ENTRIES + 50) {
            cache.render(&format!("Content {}", i));
        }

        // Cache should not exceed max entries
        assert!(cache.len() <= MARKDOWN_CACHE_MAX_ENTRIES);
    }

    #[test]
    fn test_cache_complex_markdown() {
        let mut cache = MarkdownCache::new();

        let complex_md = r#"# Heading

This is **bold** and *italic* text.

```rust
fn main() {
    println!("Hello, world!");
}
```

- List item 1
- List item 2

`inline code`
"#;

        // First render
        let lines1 = cache.render(complex_md);
        assert!(!lines1.is_empty());

        // Second render should return cached result
        let lines2 = cache.render(complex_md);
        let (hits, misses) = cache.stats();
        assert_eq!(hits, 1);
        assert_eq!(misses, 1);

        // Verify identical results
        assert_eq!(lines1.len(), lines2.len());
    }

    #[test]
    fn test_cache_hash_collision_resistant() {
        let mut cache = MarkdownCache::new();

        // Test that different but similar content produces different cache entries
        let content_a = "Hello World";
        let content_b = "Hello World!";
        let content_c = "hello world";

        cache.render(content_a);
        cache.render(content_b);
        cache.render(content_c);

        // All three should be separate cache entries
        assert_eq!(cache.len(), 3);
    }

    #[test]
    fn test_cache_empty_content() {
        let mut cache = MarkdownCache::new();

        // Empty content should still cache
        let lines = cache.render("");
        assert!(!lines.is_empty()); // render_markdown returns at least one empty line
        assert_eq!(cache.len(), 1);

        // Second render should hit cache
        cache.render("");
        let (hits, _) = cache.stats();
        assert_eq!(hits, 1);
    }

    #[test]
    fn test_cache_whitespace_sensitive() {
        let mut cache = MarkdownCache::new();

        // Whitespace differences should produce different cache entries
        cache.render("word");
        cache.render(" word");
        cache.render("word ");
        cache.render("  word  ");

        assert_eq!(cache.len(), 4);
    }

    // ========================================================================
    // URL Detection Tests
    // ========================================================================

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

        // Find the link span
        let link_span = parsed.lines[0]
            .spans
            .iter()
            .find(|s| s.content == "here")
            .expect("Should have link text span");

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

        // Find the URL span
        let url_span = parsed.lines[0]
            .spans
            .iter()
            .find(|s| s.content == "https://example.com")
            .expect("Should have URL span");

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
        assert!(parsed.links.is_empty(), "URLs in code blocks should not be detected as links");
    }

    #[test]
    fn test_url_not_detected_in_inline_code() {
        let md = "Use `https://example.com` as the base URL";
        let parsed = render_markdown_with_links(md);

        // URL inside inline code should NOT be detected as a link
        assert!(parsed.links.is_empty(), "URLs in inline code should not be detected as links");
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
    fn test_render_markdown_backward_compatible() {
        // Original render_markdown should still work
        let lines = render_markdown("Hello **world**");

        assert_eq!(lines.len(), 1);
        let has_bold = lines[0]
            .spans
            .iter()
            .any(|s| s.content == "world" && s.style.add_modifier.contains(Modifier::BOLD));
        assert!(has_bold);
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
}

