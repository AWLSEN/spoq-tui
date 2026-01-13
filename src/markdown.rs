//! Markdown parser for terminal rendering
//!
//! Converts markdown text to styled ratatui Lines for display in the TUI.
//! Handles code blocks, inline code, bold, italic, and headings.

use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

/// Style for code blocks - gray/dim color
const STYLE_CODE_BLOCK: Style = Style::new().fg(Color::DarkGray);

/// Style for inline code - cyan color
const STYLE_INLINE_CODE: Style = Style::new().fg(Color::Cyan);

/// Style for headings - cyan and bold
const STYLE_HEADING: Style = Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD);

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
    let parser = Parser::new(text);
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut current_spans: Vec<Span<'static>> = Vec::new();

    // Style stack for nested formatting
    let mut style_stack: Vec<Style> = vec![Style::default()];
    let mut in_code_block = false;

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
                    _ => {}
                }
            }
            Event::Text(text) => {
                let current_style = *style_stack.last().unwrap_or(&Style::default());
                let text_str = text.to_string();

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
                } else {
                    // Normal text - handle newlines
                    for (i, part) in text_str.split('\n').enumerate() {
                        if i > 0 {
                            // Flush current line and start new
                            lines.push(Line::from(std::mem::take(&mut current_spans)));
                        }
                        if !part.is_empty() {
                            current_spans.push(Span::styled(part.to_string(), current_style));
                        }
                    }
                }
            }
            Event::Code(code) => {
                // Inline code - cyan color
                current_spans.push(Span::styled(code.to_string(), STYLE_INLINE_CODE));
            }
            Event::SoftBreak => {
                // Soft break - add space
                let current_style = *style_stack.last().unwrap_or(&Style::default());
                current_spans.push(Span::styled(" ".to_string(), current_style));
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

    lines
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
}
