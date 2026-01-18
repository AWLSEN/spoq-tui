//! Thinking/reasoning block rendering
//!
//! Renders collapsible thinking blocks for assistant messages that show
//! the model's reasoning process.

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use crate::models::{Message, MessageRole};

use super::super::layout::LayoutContext;
use super::super::theme::COLOR_DIM;

/// Render a collapsible thinking block for assistant messages.
///
/// Uses `LayoutContext` for responsive border width.
///
/// Collapsed: arrow Thinking... (847 tokens)
/// Expanded:
/// arrow Thinking
/// vertical-bar Let me analyze this step by step...
/// vertical-bar First, I need to understand the structure.
/// bottom-left-corner followed by dashes
pub fn render_thinking_block(
    message: &Message,
    tick_count: u64,
    ctx: &LayoutContext,
) -> Vec<Line<'static>> {
    let mut lines: Vec<Line> = Vec::new();

    // Only render for assistant messages with reasoning content
    if message.role != MessageRole::Assistant {
        return lines;
    }

    if message.reasoning_content.is_empty() {
        return lines;
    }

    let token_count = message.reasoning_token_count();
    let collapsed = message.reasoning_collapsed;

    // Determine the arrow and style based on collapsed state
    let (arrow, header_color) = if collapsed {
        ("\u{25B8}", Color::Magenta)
    } else {
        ("\u{25BE}", Color::Magenta)
    };

    // Calculate responsive bottom border width (use available content width)
    let border_width = ctx.text_wrap_width(0).min(80) as usize;

    // Header line - abbreviate toggle hint on narrow terminals
    let toggle_hint = if ctx.is_extra_small() {
        " [t]"
    } else {
        "  [t] toggle"
    };

    if collapsed {
        // Collapsed: arrow Thinking... (847 tokens)
        lines.push(Line::from(vec![
            Span::styled(
                format!("{} ", arrow),
                Style::default().fg(header_color),
            ),
            Span::styled(
                "Thinking...",
                Style::default().fg(Color::Magenta).add_modifier(Modifier::ITALIC),
            ),
            Span::styled(
                format!(" ({} tokens)", token_count),
                Style::default().fg(COLOR_DIM),
            ),
            Span::styled(
                toggle_hint,
                Style::default().fg(COLOR_DIM),
            ),
        ]));
    } else {
        // Expanded header: arrow Thinking
        lines.push(Line::from(vec![
            Span::styled(
                format!("{} ", arrow),
                Style::default().fg(header_color),
            ),
            Span::styled(
                "Thinking",
                Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" ({} tokens)", token_count),
                Style::default().fg(COLOR_DIM),
            ),
            Span::styled(
                toggle_hint,
                Style::default().fg(COLOR_DIM),
            ),
        ]));

        // Render the reasoning content with box-drawing border
        let content = &message.reasoning_content;
        for line in content.lines() {
            lines.push(Line::from(vec![
                Span::styled(
                    "\u{2502} ",
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    line.to_string(),
                    Style::default().fg(Color::Gray).add_modifier(Modifier::ITALIC),
                ),
            ]));
        }

        // If streaming, add a blinking cursor at the end
        if message.is_streaming {
            let show_cursor = (tick_count / 5).is_multiple_of(2);
            if show_cursor {
                lines.push(Line::from(vec![
                    Span::styled(
                        "\u{2502} \u{2588}",
                        Style::default().fg(Color::Magenta),
                    ),
                ]));
            }
        }

        // Bottom border - responsive width
        lines.push(Line::from(vec![
            Span::styled(
                format!("\u{2514}{}", "\u{2500}".repeat(border_width.saturating_sub(1))),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    }

    lines.push(Line::from("")); // Add spacing after thinking block

    lines
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use crate::ui::LayoutContext;

    #[test]
    fn test_render_thinking_block_responsive_border_width() {
        // Test that thinking block border width adapts to terminal size
        let narrow_ctx = LayoutContext::new(60, 24);
        let wide_ctx = LayoutContext::new(120, 40);

        // Thinking block uses text_wrap_width(0).min(80)
        // For 60 cols: 60 - 4 = 56, min(56, 80) = 56
        assert_eq!(narrow_ctx.text_wrap_width(0).min(80), 56);
        // For 120 cols: 120 - 4 = 116, min(116, 80) = 80
        assert_eq!(wide_ctx.text_wrap_width(0).min(80), 80);
    }
}
