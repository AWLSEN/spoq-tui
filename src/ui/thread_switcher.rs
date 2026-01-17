//! Thread Switcher Dialog rendering
//!
//! Implements the Ctrl+Tab thread switcher overlay similar to macOS app switcher.
//! Shows threads in MRU (Most Recently Used) order with keyboard navigation.
//!
//! The overlay adapts to terminal size:
//! - On extra-small screens (< 60 cols): Uses compact layout with abbreviated hints
//! - On small screens (< 80 cols): Reduced width, fewer visible threads
//! - On medium/large screens: Full layout with all details

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::App;
use crate::models::ThreadType;

use super::helpers::extract_short_model_name;
use super::layout::LayoutContext;
use super::theme::{COLOR_ACCENT, COLOR_BORDER, COLOR_DIM, COLOR_HEADER};

/// Calculate maximum visible threads based on terminal dimensions
fn calculate_max_visible_threads(ctx: &LayoutContext) -> usize {
    // Reserve space for: borders (2) + padding (2) + hint line (1) = 5 minimum
    // On compact terminals, show fewer items
    if ctx.is_extra_small() {
        3
    } else if ctx.is_compact() {
        5
    } else {
        // Normal terminals: use available height
        let available = ctx.available_content_height(7); // borders + padding + hint
        (available as usize).clamp(3, 10)
    }
}

/// Calculate dialog width based on terminal dimensions
fn calculate_dialog_width(ctx: &LayoutContext, area_width: u16) -> u16 {
    if ctx.is_extra_small() {
        // Extra small: take most of the screen width, leave 2 cols margin
        area_width.saturating_sub(4).min(40)
    } else if ctx.is_narrow() {
        // Narrow: 80% of width, min 35, max 50
        ctx.bounded_width(80, 35, 50)
    } else {
        // Normal: 50% of width, min 40, max 60
        ctx.bounded_width(50, 40, 60)
    }
}

/// Calculate dialog height based on content and terminal dimensions
fn calculate_dialog_height(
    ctx: &LayoutContext,
    visible_count: usize,
    area_height: u16,
) -> u16 {
    // Height: 2 (borders) + 1 (padding top) + visible_count + 1 (padding bottom) + 1 (hint line)
    let content_height = visible_count as u16 + 5;

    // On extra-small terminals, use more of the available space
    let max_height = if ctx.is_extra_small() {
        area_height.saturating_sub(2)
    } else {
        area_height.saturating_sub(4)
    };

    content_height.min(max_height)
}

/// Render the thread switcher dialog as a centered overlay
pub fn render_thread_switcher(frame: &mut Frame, app: &App) {
    if !app.thread_switcher.visible {
        return;
    }

    let threads = app.cache.threads();
    if threads.is_empty() {
        return;
    }

    let area = frame.area();

    // Create layout context from app's terminal dimensions
    let ctx = LayoutContext::new(app.terminal_width, app.terminal_height);

    // Calculate responsive dimensions
    let max_visible_threads = calculate_max_visible_threads(&ctx);
    let dialog_width = calculate_dialog_width(&ctx, area.width);
    let visible_count = threads.len().min(max_visible_threads);
    let dialog_height = calculate_dialog_height(&ctx, visible_count, area.height);

    // Center the dialog
    let x = (area.width.saturating_sub(dialog_width)) / 2;
    let y = (area.height.saturating_sub(dialog_height)) / 2;

    let dialog_area = Rect {
        x,
        y,
        width: dialog_width,
        height: dialog_height,
    };

    // Clear the background behind the dialog
    frame.render_widget(Clear, dialog_area);

    // Create the dialog border with responsive title
    let title = if ctx.is_extra_small() {
        " Threads "
    } else {
        " Switch Thread "
    };

    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default()
                .fg(COLOR_HEADER)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(COLOR_BORDER));

    frame.render_widget(block, dialog_area);

    // Inner area for content
    let inner = Rect {
        x: dialog_area.x + 2,
        y: dialog_area.y + 1,
        width: dialog_area.width.saturating_sub(4),
        height: dialog_area.height.saturating_sub(2),
    };

    // Build content lines
    let mut lines: Vec<Line> = Vec::new();

    // Skip top padding on extra-small screens to save space
    if !ctx.is_extra_small() {
        lines.push(Line::from("")); // Top padding
    }

    let selected_idx = app.thread_switcher.selected_index;
    let scroll_offset = app.thread_switcher.scroll_offset;

    // Calculate max title width based on available space
    // Compact layout: less padding, shorter model names
    let model_space = if ctx.is_extra_small() { 8 } else { 12 }; // Space for type indicator + model
    let marker_space = 2; // Selection marker
    let padding_space = if ctx.is_extra_small() { 1 } else { 2 }; // Spacing between elements
    let max_title_width = (inner.width as usize).saturating_sub(model_space + marker_space + padding_space);

    // Show scroll up indicator if there are hidden threads above
    if scroll_offset > 0 {
        let indicator = if ctx.is_extra_small() {
            format!("  ^ {}", scroll_offset)
        } else {
            format!("  ↑ {} more above", scroll_offset)
        };
        lines.push(Line::from(vec![
            Span::styled(indicator, Style::default().fg(COLOR_DIM)),
        ]));
    }

    // Iterate through visible threads starting from scroll_offset
    let visible_threads: Vec<_> = threads
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(max_visible_threads)
        .collect();

    for (idx, thread) in visible_threads {
        let is_selected = idx == selected_idx;

        // Selection marker
        let marker = if is_selected { "▶ " } else { "  " };
        let marker_style = if is_selected {
            Style::default().fg(COLOR_ACCENT).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(COLOR_DIM)
        };

        // Thread type indicator - abbreviated on compact screens
        let type_indicator = if ctx.is_extra_small() {
            match thread.thread_type {
                ThreadType::Conversation => "C",
                ThreadType::Programming => "P",
            }
        } else {
            match thread.thread_type {
                ThreadType::Conversation => "[C]",
                ThreadType::Programming => "[P]",
            }
        };
        let type_color = match thread.thread_type {
            ThreadType::Conversation => Color::Cyan,
            ThreadType::Programming => Color::Magenta,
        };

        // Model name (short form) - hide on extra-small screens
        let show_model = !ctx.is_extra_small();
        let model_name = if show_model {
            thread
                .model
                .as_ref()
                .map(|m| extract_short_model_name(m))
                .unwrap_or("--")
        } else {
            ""
        };

        // Truncate thread title if needed (respecting UTF-8 boundaries)
        let title = if thread.title.len() > max_title_width {
            let end = max_title_width.saturating_sub(3);
            let boundary = thread.title
                .char_indices()
                .take_while(|(i, _)| *i <= end)
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            format!("{}...", &thread.title[..boundary])
        } else {
            thread.title.clone()
        };

        // Build the line
        let title_style = if is_selected {
            Style::default().fg(COLOR_HEADER).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(COLOR_DIM)
        };

        let model_style = if is_selected {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        // Build line with adaptive spacing
        let mut spans = vec![
            Span::styled(marker, marker_style),
            Span::styled(type_indicator, Style::default().fg(type_color)),
        ];

        if show_model {
            spans.push(Span::raw(" "));
            spans.push(Span::styled(model_name, model_style));
            spans.push(Span::raw("  "));
        } else {
            spans.push(Span::raw(" "));
        }

        spans.push(Span::styled(title, title_style));

        lines.push(Line::from(spans));
    }

    // Show scroll down indicator if there are hidden threads below
    let threads_below = threads.len().saturating_sub(scroll_offset + max_visible_threads);
    if threads_below > 0 {
        let indicator = if ctx.is_extra_small() {
            format!("  v {}", threads_below)
        } else {
            format!("  ↓ {} more below", threads_below)
        };
        lines.push(Line::from(vec![
            Span::styled(indicator, Style::default().fg(COLOR_DIM)),
        ]));
    }

    // Skip bottom padding on extra-small screens
    if !ctx.is_extra_small() {
        lines.push(Line::from("")); // Bottom padding
    }

    // Hint line - abbreviated on compact screens
    if ctx.is_extra_small() {
        // Ultra-compact hints
        lines.push(Line::from(vec![
            Span::styled(" ", Style::default()),
            Span::styled("Tab", Style::default().fg(COLOR_ACCENT)),
            Span::styled("/", Style::default().fg(COLOR_DIM)),
            Span::styled("Esc", Style::default().fg(COLOR_ACCENT)),
        ]));
    } else if ctx.is_compact() {
        // Compact hints
        lines.push(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled("Tab", Style::default().fg(COLOR_ACCENT)),
            Span::styled(":nav ", Style::default().fg(COLOR_DIM)),
            Span::styled("Esc", Style::default().fg(COLOR_ACCENT)),
            Span::styled(":close", Style::default().fg(COLOR_DIM)),
        ]));
    } else {
        // Full hints
        lines.push(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled("Tab/↓↑", Style::default().fg(COLOR_ACCENT)),
            Span::styled(": navigate  ", Style::default().fg(COLOR_DIM)),
            Span::styled("Esc", Style::default().fg(COLOR_ACCENT)),
            Span::styled(": cancel", Style::default().fg(COLOR_DIM)),
        ]));
    }

    let content = Paragraph::new(lines);
    frame.render_widget(content, inner);
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Max Visible Threads Tests
    // ========================================================================

    #[test]
    fn test_max_visible_threads_extra_small() {
        let ctx = LayoutContext::new(50, 14);
        assert_eq!(calculate_max_visible_threads(&ctx), 3);
    }

    #[test]
    fn test_max_visible_threads_compact() {
        let ctx = LayoutContext::new(70, 20);
        assert_eq!(calculate_max_visible_threads(&ctx), 5);
    }

    #[test]
    fn test_max_visible_threads_normal() {
        let ctx = LayoutContext::new(120, 40);
        let result = calculate_max_visible_threads(&ctx);
        assert!(result >= 3 && result <= 10);
    }

    #[test]
    fn test_max_visible_threads_clamped_to_minimum() {
        let ctx = LayoutContext::new(100, 10);
        let result = calculate_max_visible_threads(&ctx);
        assert!(result >= 3);
    }

    #[test]
    fn test_max_visible_threads_clamped_to_maximum() {
        let ctx = LayoutContext::new(200, 100);
        let result = calculate_max_visible_threads(&ctx);
        assert!(result <= 10);
    }

    // ========================================================================
    // Dialog Width Tests
    // ========================================================================

    #[test]
    fn test_dialog_width_extra_small() {
        let ctx = LayoutContext::new(50, 14);
        let width = calculate_dialog_width(&ctx, 50);
        assert!(width <= 40);
        assert!(width >= 10);
    }

    #[test]
    fn test_dialog_width_narrow() {
        let ctx = LayoutContext::new(70, 24);
        let width = calculate_dialog_width(&ctx, 70);
        assert!(width >= 35);
        assert!(width <= 50);
    }

    #[test]
    fn test_dialog_width_normal() {
        let ctx = LayoutContext::new(120, 40);
        let width = calculate_dialog_width(&ctx, 120);
        assert!(width >= 40);
        assert!(width <= 60);
    }

    #[test]
    fn test_dialog_width_never_exceeds_area() {
        let ctx = LayoutContext::new(30, 14);
        let width = calculate_dialog_width(&ctx, 30);
        assert!(width <= 30);
    }

    // ========================================================================
    // Dialog Height Tests
    // ========================================================================

    #[test]
    fn test_dialog_height_extra_small() {
        let ctx = LayoutContext::new(50, 14);
        let height = calculate_dialog_height(&ctx, 3, 14);
        assert!(height <= 12); // area - 2 margin
    }

    #[test]
    fn test_dialog_height_normal() {
        let ctx = LayoutContext::new(120, 40);
        let height = calculate_dialog_height(&ctx, 5, 40);
        // 5 threads + 5 chrome = 10
        assert_eq!(height, 10);
    }

    #[test]
    fn test_dialog_height_clamped_to_area() {
        let ctx = LayoutContext::new(120, 12);
        let height = calculate_dialog_height(&ctx, 10, 12);
        assert!(height <= 12);
    }

    // ========================================================================
    // Responsive Layout Integration Tests
    // ========================================================================

    #[test]
    fn test_responsive_dimensions_are_consistent() {
        // On a tiny terminal, everything should fit
        let ctx = LayoutContext::new(40, 10);
        let max_threads = calculate_max_visible_threads(&ctx);
        let dialog_width = calculate_dialog_width(&ctx, 40);
        let dialog_height = calculate_dialog_height(&ctx, max_threads, 10);

        assert!(dialog_width <= 40);
        assert!(dialog_height <= 10);
    }

    #[test]
    fn test_responsive_dimensions_scale_up() {
        let small_ctx = LayoutContext::new(60, 20);
        let large_ctx = LayoutContext::new(160, 50);

        let small_threads = calculate_max_visible_threads(&small_ctx);
        let large_threads = calculate_max_visible_threads(&large_ctx);

        let small_width = calculate_dialog_width(&small_ctx, 60);
        let large_width = calculate_dialog_width(&large_ctx, 160);

        // Larger terminal should have more visible threads (or equal if capped)
        assert!(large_threads >= small_threads);
        // Larger terminal should have wider dialog
        assert!(large_width >= small_width);
    }
}
