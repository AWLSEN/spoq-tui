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

mod cache;
mod links;
mod styles;

pub use cache::MarkdownCache;
pub use links::{detect_plain_urls, LinkInfo, ParsedMarkdown};
pub use styles::wrap_osc8_hyperlink;

use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use links::render_text_with_urls;
use styles::{STYLE_CODE_BLOCK, STYLE_HEADING, STYLE_INLINE_CODE};

/// Maximum number of entries in the markdown cache before eviction
pub const MARKDOWN_CACHE_MAX_ENTRIES: usize = 500;

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
                        style_stack
                            .push(current.fg(Color::Blue).add_modifier(Modifier::UNDERLINED));
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
                            current_spans
                                .push(Span::styled(line_content.to_string(), current_style));
                        }
                    }
                } else if let Some(ref url) = current_link_url {
                    // Inside a markdown link - wrap text with OSC 8 hyperlink escape sequence
                    for (i, part) in text_str.split('\n').enumerate() {
                        if i > 0 {
                            lines.push(Line::from(std::mem::take(&mut current_spans)));
                            lines.push(Line::from(""));
                        }
                        if !part.is_empty() {
                            // Wrap the link text with OSC 8 escape sequences for clickable hyperlinks
                            let osc8_text = wrap_osc8_hyperlink(url, part);
                            current_spans.push(Span::styled(osc8_text, current_style));
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
                            let spans_with_urls = render_text_with_urls(
                                part,
                                current_style,
                                &mut links,
                                byte_position,
                            );
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
        assert!(!lines[0].spans.is_empty());

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
        assert!(
            has_indented,
            "Code block should preserve leading whitespace"
        );
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
        assert!(
            lines2.len() >= 3,
            "Hard breaks should create separate lines"
        );
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
        assert!(
            has_box_chars,
            "Should preserve ASCII box-drawing characters"
        );
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
}
