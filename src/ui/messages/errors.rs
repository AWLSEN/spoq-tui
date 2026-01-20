//! Error banner rendering
//!
//! Renders inline error banners for thread-specific errors with responsive sizing.

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use crate::app::App;

use super::super::helpers::{truncate_string, MAX_VISIBLE_ERRORS};
use super::super::layout::LayoutContext;
use super::super::theme::COLOR_DIM;

/// Render inline error banners for a thread
///
/// Uses `LayoutContext` for responsive banner width that adapts to terminal size.
/// Returns the lines to be added to the messages area
pub fn render_inline_error_banners(app: &App, ctx: &LayoutContext) -> Vec<Line<'static>> {
    let mut lines: Vec<Line> = Vec::new();

    // Get errors for the active thread
    let errors = app
        .active_thread_id
        .as_ref()
        .and_then(|id| app.cache.get_errors(id));

    let Some(errors) = errors else {
        return lines;
    };

    if errors.is_empty() {
        return lines;
    }

    let focused_index = app.cache.focused_error_index();
    let total_errors = errors.len();

    // Calculate responsive error box width based on terminal width
    // Use 80% of terminal width, clamped between 40 and 80
    let box_width = ctx.bounded_width(80, 40, 80) as usize;
    let inner_width = box_width.saturating_sub(2); // Account for border chars

    // Only show up to MAX_VISIBLE_ERRORS
    for (i, error) in errors.iter().take(MAX_VISIBLE_ERRORS).enumerate() {
        let is_focused = i == focused_index;
        let border_color = if is_focused {
            Color::Red
        } else {
            Color::DarkGray
        };
        let border_char_top = if is_focused { "\u{2550}" } else { "\u{2500}" };
        let border_char_bottom = if is_focused { "\u{2550}" } else { "\u{2500}" };

        // Top border with error code
        let header = format!("\u{2500}[!] {} ", error.error_code);
        let remaining_width = inner_width.saturating_sub(header.len());
        let top_border = format!(
            "\u{250C}{}{}{}",
            header,
            border_char_top.repeat(remaining_width),
            "\u{2510}"
        );
        lines.push(Line::from(Span::styled(
            top_border,
            Style::default().fg(border_color),
        )));

        // Error message line - truncate based on responsive width
        let max_msg_len = inner_width.saturating_sub(4); // Account for borders and padding
        let msg_display = if error.message.len() > max_msg_len {
            truncate_string(&error.message, max_msg_len)
        } else {
            error.message.clone()
        };
        let msg_padding = inner_width.saturating_sub(msg_display.len() + 2); // +2 for "| " prefix
        lines.push(Line::from(vec![
            Span::styled("\u{2502} ", Style::default().fg(border_color)),
            Span::styled(msg_display, Style::default().fg(Color::White)),
            Span::styled(
                format!("{:>width$}\u{2502}", "", width = msg_padding),
                Style::default().fg(border_color),
            ),
        ]));

        // Dismiss hint line - abbreviate on narrow terminals
        let dismiss_text = if ctx.is_extra_small() {
            "[d]"
        } else {
            "[d]ismiss"
        };
        let dismiss_padding = inner_width.saturating_sub(dismiss_text.len() + 2);
        lines.push(Line::from(vec![
            Span::styled("\u{2502} ", Style::default().fg(border_color)),
            Span::styled(
                format!("{:>width$}", "", width = dismiss_padding),
                Style::default().fg(border_color),
            ),
            Span::styled(dismiss_text, Style::default().fg(COLOR_DIM)),
            Span::styled(" \u{2502}", Style::default().fg(border_color)),
        ]));

        // Bottom border
        let bottom_border = format!("\u{2514}{}\u{2518}", border_char_bottom.repeat(inner_width));
        lines.push(Line::from(Span::styled(
            bottom_border,
            Style::default().fg(border_color),
        )));

        lines.push(Line::from(""));
    }

    // Show "+N more" if there are more errors
    if total_errors > MAX_VISIBLE_ERRORS {
        let more_count = total_errors - MAX_VISIBLE_ERRORS;
        lines.push(Line::from(vec![Span::styled(
            format!(
                "  +{} more error{}",
                more_count,
                if more_count > 1 { "s" } else { "" }
            ),
            Style::default()
                .fg(Color::Red)
                .add_modifier(Modifier::ITALIC),
        )]));
        lines.push(Line::from(""));
    }

    lines
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use crate::ui::LayoutContext;

    #[test]
    fn test_render_inline_error_banners_responsive_width() {
        // Test with different contexts - error banners use bounded_width(80, 40, 80)
        let narrow_ctx = LayoutContext::new(50, 24);
        let wide_ctx = LayoutContext::new(160, 40);

        // Error banner width calculation: bounded_width(80, 40, 80)
        // For 50 cols: 80% = 40, clamped to 40-80 = 40
        assert_eq!(narrow_ctx.bounded_width(80, 40, 80), 40);
        // For 160 cols: 80% = 128, clamped to 40-80 = 80
        assert_eq!(wide_ctx.bounded_width(80, 40, 80), 80);
    }
}
