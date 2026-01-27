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

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
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
    // Enable table parsing in addition to default options
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);

    let parser = Parser::new_ext(text, options);
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

    // Table rendering state
    let mut in_table = false;
    let mut table_rows: Vec<Vec<String>> = Vec::new();
    let mut current_row: Vec<String> = Vec::new();
    let mut current_cell = String::new();

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
                    Tag::Table(_) => {
                        // Start of a table - flush current content and initialize table state
                        if !current_spans.is_empty() {
                            lines.push(Line::from(std::mem::take(&mut current_spans)));
                        }
                        in_table = true;
                        table_rows.clear();
                    }
                    Tag::TableHead => {
                        current_row.clear();
                    }
                    Tag::TableRow => {
                        current_row.clear();
                    }
                    Tag::TableCell => {
                        current_cell.clear();
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
                    TagEnd::TableCell => {
                        // End of table cell - save the cell content
                        current_row.push(std::mem::take(&mut current_cell));
                    }
                    TagEnd::TableHead | TagEnd::TableRow => {
                        // End of table row - save the row
                        if !current_row.is_empty() {
                            table_rows.push(std::mem::take(&mut current_row));
                        }
                    }
                    TagEnd::Table => {
                        // End of table - render the collected table data
                        render_table_to_lines(&table_rows, &mut lines);
                        in_table = false;
                        table_rows.clear();
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

                // If we're inside a table, buffer the cell content
                if in_table {
                    current_cell.push_str(&text_str);
                } else if in_code_block {
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
                if in_table {
                    // Inside a table cell - buffer the code text
                    current_cell.push_str(&code);
                } else {
                    // Inline code - cyan color
                    current_spans.push(Span::styled(code.to_string(), STYLE_INLINE_CODE));
                }
            }
            Event::SoftBreak => {
                if in_table {
                    // Inside a table cell - convert to space
                    current_cell.push(' ');
                } else {
                    // Soft break - create new line with blank line for visual separation
                    // This gives proper paragraph-like spacing in streamed content
                    lines.push(Line::from(std::mem::take(&mut current_spans)));
                    lines.push(Line::from("")); // Add blank line for visual separation
                }
            }
            Event::HardBreak => {
                if in_table {
                    // Inside a table cell - convert to space
                    current_cell.push(' ');
                } else {
                    // Hard break - new line
                    lines.push(Line::from(std::mem::take(&mut current_spans)));
                }
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

/// Render a table to styled Lines.
///
/// Takes the collected table rows (each row is a Vec of cell strings) and
/// renders them as formatted lines with proper column alignment and borders.
fn render_table_to_lines(table_rows: &[Vec<String>], lines: &mut Vec<Line<'static>>) {
    if table_rows.is_empty() {
        return;
    }

    // Calculate the maximum width for each column
    let num_cols = table_rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if num_cols == 0 {
        return;
    }

    let mut col_widths: Vec<usize> = vec![0; num_cols];
    for row in table_rows {
        for (i, cell) in row.iter().enumerate() {
            if i < num_cols {
                col_widths[i] = col_widths[i].max(cell.trim().len());
            }
        }
    }

    // Ensure minimum column width of 3 for readability
    for width in &mut col_widths {
        *width = (*width).max(3);
    }

    // Style for table borders
    let border_style = Style::default().fg(Color::DarkGray);
    let header_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let cell_style = Style::default();

    // Build the top border
    let top_border = build_table_border(&col_widths, '┌', '┬', '┐');
    lines.push(Line::from(Span::styled(top_border, border_style)));

    // Render each row
    for (row_idx, row) in table_rows.iter().enumerate() {
        let mut spans: Vec<Span<'static>> = Vec::new();
        spans.push(Span::styled("│".to_string(), border_style));

        for (col_idx, cell) in row.iter().enumerate() {
            if col_idx < num_cols {
                let width = col_widths[col_idx];
                let content = cell.trim();
                let padded = format!(" {:<width$} ", content, width = width);

                // Use header style for first row, regular style for others
                let style = if row_idx == 0 {
                    header_style
                } else {
                    cell_style
                };
                spans.push(Span::styled(padded, style));
                spans.push(Span::styled("│".to_string(), border_style));
            }
        }

        // Pad missing columns
        for col_idx in row.len()..num_cols {
            let width = col_widths[col_idx];
            let padded = format!(" {:<width$} ", "", width = width);
            let style = if row_idx == 0 {
                header_style
            } else {
                cell_style
            };
            spans.push(Span::styled(padded, style));
            spans.push(Span::styled("│".to_string(), border_style));
        }

        lines.push(Line::from(spans));

        // Add separator after header row
        if row_idx == 0 && table_rows.len() > 1 {
            let separator = build_table_border(&col_widths, '├', '┼', '┤');
            lines.push(Line::from(Span::styled(separator, border_style)));
        }
    }

    // Build the bottom border
    let bottom_border = build_table_border(&col_widths, '└', '┴', '┘');
    lines.push(Line::from(Span::styled(bottom_border, border_style)));

    // Add blank line after table for visual separation
    lines.push(Line::from(""));
}

/// Build a table border line with the given corner and junction characters.
fn build_table_border(col_widths: &[usize], left: char, middle: char, right: char) -> String {
    let mut border = String::new();
    border.push(left);

    for (i, &width) in col_widths.iter().enumerate() {
        // +2 for padding on each side of cell content
        for _ in 0..(width + 2) {
            border.push('─');
        }
        if i < col_widths.len() - 1 {
            border.push(middle);
        }
    }

    border.push(right);
    border
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

    #[test]
    fn test_table_rendering() {
        let md = "| Name | Age |\n|------|-----|\n| Alice | 30 |\n| Bob | 25 |";
        let lines = render_markdown(md);

        // Should have multiple lines for the table structure
        // (top border, header row, separator, data rows, bottom border, blank line)
        assert!(
            lines.len() >= 5,
            "Table should create multiple lines, got {} lines",
            lines.len()
        );

        // Verify table content is present
        let all_content: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect();
        assert!(all_content.contains("Alice"), "Should contain 'Alice'");
        assert!(all_content.contains("Bob"), "Should contain 'Bob'");
        assert!(all_content.contains("30"), "Should contain '30'");
        assert!(all_content.contains("25"), "Should contain '25'");

        // Verify box drawing characters are used (not raw pipe characters)
        assert!(
            all_content.contains('│') || all_content.contains('┌'),
            "Should use box drawing characters for borders"
        );
    }

    #[test]
    fn test_table_with_header_styling() {
        let md = "| Header1 | Header2 |\n|---------|----------|\n| Cell1 | Cell2 |";
        let lines = render_markdown(md);

        // Find header row (should be the second line after top border)
        // Header cells should have cyan + bold styling
        let header_line = &lines[1]; // After top border
        let has_cyan_bold = header_line.spans.iter().any(|s| {
            s.style.fg == Some(Color::Cyan) && s.style.add_modifier.contains(Modifier::BOLD)
        });
        assert!(has_cyan_bold, "Header should have cyan + bold styling");
    }

    #[test]
    fn test_empty_table() {
        // Edge case: table with no content rows
        let md = "| Header |\n|--------|";
        let lines = render_markdown(md);

        // Should not panic and should produce some output
        assert!(!lines.is_empty(), "Should handle table with only header");
    }

    #[test]
    fn test_table_column_alignment() {
        // Longer content in cells should result in wider columns
        let md = "| Short | VeryLongContent |\n|-------|------------------|\n| A | B |";
        let lines = render_markdown(md);

        // Table should be rendered
        assert!(lines.len() >= 4, "Should render table structure");
    }

    #[test]
    fn test_table_mixed_with_text() {
        let md = "Here's a table:\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\nEnd of table.";
        let lines = render_markdown(md);

        // Should have content from both text and table
        let all_content: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect();
        assert!(all_content.contains("table"), "Should contain surrounding text");
        assert!(all_content.contains("1"), "Should contain table content");
    }

    #[test]
    fn test_table_with_inline_code() {
        // Tables often contain inline code like function names
        let md = "| Function | Description |\n|----------|-------------|\n| `foo()` | Does foo |\n| `bar()` | Does bar |";
        let lines = render_markdown(md);

        // Should have the code content in the table (without backticks)
        let all_content: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect();
        assert!(all_content.contains("foo()"), "Should contain 'foo()' from inline code");
        assert!(all_content.contains("bar()"), "Should contain 'bar()' from inline code");
        assert!(all_content.contains("Does foo"), "Should contain description text");
    }

    #[test]
    fn test_empty_table_renders() {
        let md = "| Col1 | Col2 |\n|------|------|\n| | |";
        let lines = render_markdown(md);

        // Should not panic on empty cells
        assert!(lines.len() >= 4, "Should render table structure even with empty cells");
    }
}
