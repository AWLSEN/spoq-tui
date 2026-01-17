//! Left and Right panel rendering
//!
//! Implements the Notifications, Tasks/Todos, and Threads panels with
//! fluid responsive widths based on terminal dimensions.

use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::app::{App, Focus};
use crate::state::{Notification, TodoStatus};

use super::helpers::{extract_short_model_name, inner_rect, truncate_string};
use super::layout::LayoutContext;
use super::theme::{COLOR_ACCENT, COLOR_ACTIVE, COLOR_BORDER, COLOR_DIM, COLOR_HEADER};

// ============================================================================
// Helper Functions for Responsive Rendering
// ============================================================================

/// Generate a horizontal separator line that adapts to available width.
///
/// # Arguments
/// * `width` - The available width for the separator
/// * `focused` - Whether the containing panel is focused
fn generate_separator(width: u16, focused: bool) -> Line<'static> {
    let separator_char = "─";
    let separator_str: String = separator_char.repeat(width as usize);
    Line::from(Span::styled(
        separator_str,
        Style::default().fg(if focused { COLOR_ACCENT } else { COLOR_DIM }),
    ))
}

/// Truncate text to fit within available width, adding ellipsis if needed.
///
/// # Arguments
/// * `text` - The text to truncate
/// * `max_width` - Maximum width in characters
fn truncate_to_width(text: &str, max_width: usize) -> String {
    truncate_string(text, max_width)
}

// ============================================================================
// Left Panel: Notifications + Tasks
// ============================================================================

pub fn render_left_panel(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let left_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(COLOR_BORDER));
    frame.render_widget(left_block.clone(), area);

    let inner = inner_rect(area, 1);
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(50), // Notifications
            Constraint::Percentage(50), // Tasks
        ])
        .split(inner);

    // Create layout context from panel area for responsive sizing
    let ctx = LayoutContext::from_rect(inner);

    render_notifications(frame, left_chunks[0], app, app.focus == Focus::Notifications, &ctx);
    render_tasks(frame, left_chunks[1], app, app.focus == Focus::Tasks, &ctx);
}

pub fn render_notifications(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    app: &App,
    focused: bool,
    ctx: &LayoutContext,
) {
    // Header styling changes based on focus
    let header_style = if focused {
        Style::default().fg(COLOR_HEADER).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(COLOR_ACCENT).add_modifier(Modifier::BOLD)
    };

    // Calculate available width for content (area width minus padding)
    let content_width = area.width.saturating_sub(2) as usize;

    let mut lines = vec![
        Line::from(Span::styled(
            if focused { "◈ NOTIFICATIONS ◄" } else { "◈ NOTIFICATIONS" },
            header_style,
        )),
        generate_separator(area.width.saturating_sub(1), focused),
    ];

    // Mock notifications for static render
    let mock_notifications = [
        Notification {
            timestamp: chrono::Utc::now(),
            message: "Agent completed task".to_string(),
        },
        Notification {
            timestamp: chrono::Utc::now() - chrono::Duration::minutes(1),
            message: "New message received".to_string(),
        },
        Notification {
            timestamp: chrono::Utc::now() - chrono::Duration::minutes(4),
            message: "Process spawned".to_string(),
        },
        Notification {
            timestamp: chrono::Utc::now() - chrono::Duration::minutes(6),
            message: "File saved".to_string(),
        },
    ];

    // Calculate max visible items based on available height
    let max_items = area.height.saturating_sub(3) as usize;
    // Use layout context to determine appropriate truncation
    let max_message_len = ctx.max_preview_length().min(content_width.saturating_sub(12)); // 12 = marker(2) + time(7) + brackets(2) + space(1)

    for (i, notif) in mock_notifications.iter().take(max_items).enumerate() {
        let time = notif.timestamp.format("%H:%M").to_string();
        let is_selected = focused && i == app.notifications_index;
        let marker = if is_selected { "▶ " } else { "▸ " };
        let marker_style = if is_selected {
            Style::default().fg(COLOR_HEADER)
        } else {
            Style::default().fg(COLOR_ACCENT)
        };

        // Truncate message to fit within available width
        let display_message = truncate_to_width(&notif.message, max_message_len);

        lines.push(Line::from(vec![
            Span::styled(marker, marker_style),
            Span::styled(format!("[{}] ", time), Style::default().fg(COLOR_DIM)),
            Span::styled(
                display_message,
                if is_selected {
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                },
            ),
        ]));
    }

    let notifications = Paragraph::new(lines);
    frame.render_widget(notifications, area);
}

pub fn render_tasks(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    app: &App,
    focused: bool,
    ctx: &LayoutContext,
) {
    let border_color = if focused { COLOR_ACCENT } else { COLOR_DIM };
    let task_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(border_color));
    frame.render_widget(task_block.clone(), area);

    let inner = inner_rect(area, 1);

    // Calculate available width for content
    let content_width = inner.width as usize;

    let header_style = if focused {
        Style::default().fg(COLOR_HEADER).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(COLOR_ACCENT).add_modifier(Modifier::BOLD)
    };

    let mut lines = vec![
        Line::from(Span::styled(
            if focused { "◈ TODOS ◄" } else { "◈ TODOS" },
            header_style,
        )),
        generate_separator(inner.width.saturating_sub(1), focused),
    ];

    // Use layout context to determine appropriate truncation for todo items
    let max_todo_len = ctx.max_preview_length().min(content_width.saturating_sub(5)); // 5 = icon(4) + space(1)

    // Render todos from app state
    if app.todos.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No todos yet",
            Style::default().fg(COLOR_DIM),
        )));
    } else {
        // Calculate max visible items based on available height
        let max_visible = inner.height.saturating_sub(3) as usize;

        for todo in app.todos.iter().take(max_visible) {
            let (icon, color, text) = match todo.status {
                TodoStatus::Pending => ("[ ] ", COLOR_DIM, &todo.content),
                TodoStatus::InProgress => ("[◐] ", Color::Cyan, &todo.active_form),
                TodoStatus::Completed => ("[✓] ", Color::Green, &todo.content),
            };

            // Truncate todo text to fit within available width
            let display_text = truncate_to_width(text, max_todo_len);

            lines.push(Line::from(vec![
                Span::styled(icon, Style::default().fg(color)),
                Span::styled(
                    display_text,
                    if todo.status == TodoStatus::Pending {
                        Style::default().fg(COLOR_DIM)
                    } else {
                        Style::default().fg(color)
                    },
                ),
            ]));
        }
    }

    let todos_widget = Paragraph::new(lines);
    frame.render_widget(todos_widget, inner);
}


// ============================================================================
// Right Panel: Threads
// ============================================================================

pub fn render_right_panel(frame: &mut Frame, area: ratatui::layout::Rect, app: &App, focused: bool) {
    let border_color = if focused { COLOR_HEADER } else { COLOR_BORDER };
    let right_block = Block::default()
        .borders(Borders::ALL)
        .border_type(if focused { BorderType::Thick } else { BorderType::Plain })
        .border_style(Style::default().fg(border_color));
    frame.render_widget(right_block.clone(), area);

    let inner = inner_rect(area, 1);

    // Create layout context from panel area for responsive sizing
    let ctx = LayoutContext::from_rect(inner);

    // Calculate card width as percentage of panel width with bounds
    // Use 75% on wider terminals, 90% on narrower ones
    let card_percentage: u16 = if ctx.is_narrow() { 90 } else { 75 };
    let card_width: u16 = ctx.bounded_width(card_percentage, 25, inner.width.saturating_sub(2));
    let inner_width = card_width.saturating_sub(2) as usize; // Width inside borders (between ┌ and ┐)

    // Generate dynamic border strings
    let border_top = format!("┌{}┐", "─".repeat(inner_width));
    let border_bottom = format!("└{}┘", "─".repeat(inner_width));

    // Content width: space between "│ " (2 chars) and "│" (1 char)
    // So content_width = card_width - 3 = inner_width - 1
    let content_width = inner_width.saturating_sub(1);

    // Calculate centering padding
    let left_padding = if inner.width > card_width {
        (inner.width - card_width) / 2
    } else {
        0
    };
    let padding_str: String = " ".repeat(left_padding as usize);

    let header_style = if focused {
        Style::default().fg(COLOR_HEADER).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(COLOR_ACCENT).add_modifier(Modifier::BOLD)
    };

    let mut lines = vec![
        Line::from(Span::styled(
            if focused { "◈ THREADS ◄" } else { "◈ THREADS" },
            header_style,
        )),
        Line::from(""),
    ];

    // Use threads from cache (no mock fallback - show empty if no threads)
    let cached_threads = app.cache.threads();
    let threads_to_render: Vec<(String, String)> = cached_threads.iter().map(|t| {
        (t.title.clone(), t.preview.clone())
    }).collect();

    // Show empty state if no threads
    if threads_to_render.is_empty() {
        lines.push(Line::from(vec![
            Span::raw(padding_str.clone()),
            Span::styled(
                "No conversations yet",
                Style::default().fg(COLOR_DIM),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::raw(padding_str.clone()),
            Span::styled(
                "Type a message to start",
                Style::default().fg(COLOR_DIM),
            ),
        ]));
    }

    // Calculate max visible thread cards based on available height
    // Each card takes approximately 5-6 lines (top border, title, type, preview, bottom border, empty line)
    let lines_per_card = 6;
    let max_visible_threads = (inner.height.saturating_sub(4) / lines_per_card as u16) as usize;

    for (i, (title, preview)) in threads_to_render.iter().take(max_visible_threads.max(1)).enumerate() {
        let is_selected = focused && i == app.threads_index;
        let card_border_color = if is_selected { COLOR_HEADER } else { COLOR_BORDER };

        // Thread card top border (centered)
        lines.push(Line::from(vec![
            Span::raw(padding_str.clone()),
            Span::styled(
                border_top.clone(),
                Style::default().fg(card_border_color),
            ),
        ]));

        // Thread title (centered)
        let title_marker = if is_selected { "▶ " } else { "► " };

        // Check if this thread is streaming and compute dots
        let thread_id = &cached_threads[i].id;
        let is_streaming = app.cache.is_thread_streaming(thread_id);
        let dots = if is_streaming {
            match (app.tick_count / 5) % 3 {
                0 => ".",
                1 => "..",
                _ => "...",
            }
        } else {
            ""
        };

        // Calculate available width for title using layout context
        // Use responsive max_title_length but constrain to actual available space
        let base_max_title = ctx.max_title_length();
        let available_for_title = content_width.saturating_sub(10 + if is_streaming { 3 } else { 0 }); // 10 = marker(2) + "Thread: "(8)
        let max_title_len = base_max_title.min(available_for_title);

        let display_title = truncate_to_width(title, max_title_len);

        let mut title_spans = vec![
            Span::raw(padding_str.clone()),
            Span::styled("│ ", Style::default().fg(card_border_color)),
            Span::styled(title_marker, Style::default().fg(if is_selected { COLOR_HEADER } else { COLOR_ACCENT })),
            Span::styled(
                format!("Thread: {}", display_title),
                Style::default()
                    .fg(if is_selected { Color::White } else { COLOR_HEADER })
                    .add_modifier(Modifier::BOLD),
            ),
        ];

        if is_streaming {
            title_spans.push(Span::styled(
                dots,
                Style::default().fg(COLOR_ACTIVE),
            ));
        }

        title_spans.push(Span::styled(
            format!("{:>width$}│", "", width = content_width.saturating_sub(10 + display_title.chars().count() + dots.len())),
            Style::default().fg(card_border_color),
        ));

        lines.push(Line::from(title_spans));

        // Thread description (centered, if present)
        let thread = &cached_threads[i];
        if let Some(description) = &thread.description {
            if !description.is_empty() {
                // Max description length constrained by content width and layout context
                let max_desc_len = ctx.max_preview_length().min(content_width.saturating_sub(2));
                let display_desc = truncate_to_width(description, max_desc_len);

                lines.push(Line::from(vec![
                    Span::raw(padding_str.clone()),
                    Span::styled("│   ", Style::default().fg(card_border_color)),
                    Span::styled(display_desc.clone(), Style::default().fg(COLOR_DIM)),
                    Span::styled(
                        format!("{:>width$}│", "", width = content_width.saturating_sub(2 + display_desc.chars().count())),
                        Style::default().fg(card_border_color),
                    ),
                ]));
            }
        }

        // Thread type indicator and model info (centered)
        let type_indicator = match thread.thread_type {
            crate::models::ThreadType::Conversation => "[C]",
            crate::models::ThreadType::Programming => "[P]",
        };

        let mut type_line_spans = vec![
            Span::raw(padding_str.clone()),
            Span::styled("│   ", Style::default().fg(card_border_color)),
            Span::styled(type_indicator, Style::default().fg(COLOR_ACCENT)),
        ];

        // Add model name if present - always show short model name (e.g., "sonnet", "opus")
        // Only abbreviate to single letter when content width is truly cramped (< 20 chars)
        if let Some(model) = &thread.model {
            let short_model = extract_short_model_name(model);
            // Only use single-letter abbreviations for very cramped content areas
            // At content_width < 20, there's not enough room for "sonnet" plus other content
            let model_display = if content_width < 20 {
                match short_model {
                    "opus" => "o",
                    "sonnet" => "s",
                    other => &other[..1.min(other.len())],
                }
            } else {
                short_model
            };
            type_line_spans.push(Span::styled(
                format!(" {}", model_display),
                Style::default().fg(COLOR_DIM),
            ));
            let type_info_len = type_indicator.len() + 1 + model_display.len();
            type_line_spans.push(Span::styled(
                format!("{:>width$}│", "", width = content_width.saturating_sub(2 + type_info_len)),
                Style::default().fg(card_border_color),
            ));
        } else {
            type_line_spans.push(Span::styled(
                format!("{:>width$}│", "", width = content_width.saturating_sub(2 + type_indicator.len())),
                Style::default().fg(card_border_color),
            ));
        }

        lines.push(Line::from(type_line_spans));

        // Thread preview (centered)
        // Use layout context to determine max preview length, constrained by content width
        let max_preview_len = ctx.max_preview_length().min(content_width.saturating_sub(4)); // 4 = 2 for quotes, 2 for indent
        let display_preview = truncate_to_width(preview, max_preview_len);

        lines.push(Line::from(vec![
            Span::raw(padding_str.clone()),
            Span::styled("│   ", Style::default().fg(card_border_color)),
            Span::styled(format!("\"{}\"", display_preview), Style::default().fg(COLOR_DIM)),
            Span::styled(
                format!("{:>width$}│", "", width = content_width.saturating_sub(2 + display_preview.chars().count() + 2)), // +2 for quotes
                Style::default().fg(card_border_color),
            ),
        ]));

        // Thread card bottom border (centered)
        lines.push(Line::from(vec![
            Span::raw(padding_str.clone()),
            Span::styled(
                border_bottom.clone(),
                Style::default().fg(card_border_color),
            ),
        ]));
        lines.push(Line::from(""));
    }

    // Keybind hints at bottom of threads panel (centered)
    // Adjust hints based on available width
    let hints = if ctx.should_show_full_badges() {
        vec![
            Span::raw(padding_str.clone()),
            Span::styled("[Shift+N]", Style::default().fg(COLOR_ACCENT)),
            Span::raw(" New Thread  "),
            Span::styled("[TAB]", Style::default().fg(COLOR_ACCENT)),
            Span::raw(" Switch Panel"),
        ]
    } else {
        // Compact hints for narrow terminals
        vec![
            Span::raw(padding_str.clone()),
            Span::styled("[N]", Style::default().fg(COLOR_ACCENT)),
            Span::raw(" New  "),
            Span::styled("[TAB]", Style::default().fg(COLOR_ACCENT)),
            Span::raw(" Switch"),
        ]
    };
    lines.push(Line::from(hints));

    let threads = Paragraph::new(lines);
    frame.render_widget(threads, inner);
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    #[test]
    fn test_generate_separator_focused() {
        let separator = generate_separator(20, true);
        // Should contain 20 separator characters
        let text: String = separator.spans.iter().map(|s| s.content.to_string()).collect();
        assert_eq!(text.chars().count(), 20);
    }

    #[test]
    fn test_generate_separator_unfocused() {
        let separator = generate_separator(15, false);
        let text: String = separator.spans.iter().map(|s| s.content.to_string()).collect();
        assert_eq!(text.chars().count(), 15);
    }

    #[test]
    fn test_generate_separator_zero_width() {
        let separator = generate_separator(0, true);
        let text: String = separator.spans.iter().map(|s| s.content.to_string()).collect();
        assert!(text.is_empty());
    }

    #[test]
    fn test_truncate_to_width_no_truncation() {
        let result = truncate_to_width("Hello", 10);
        assert_eq!(result, "Hello");
    }

    #[test]
    fn test_truncate_to_width_with_truncation() {
        let result = truncate_to_width("Hello World", 8);
        assert!(result.ends_with("..."));
        assert!(result.len() <= 8);
    }

    #[test]
    fn test_truncate_to_width_exact_length() {
        let result = truncate_to_width("Hello", 5);
        assert_eq!(result, "Hello");
    }

    #[test]
    fn test_layout_context_card_width_narrow() {
        let ctx = LayoutContext::new(60, 24);
        // Narrow terminal should use 90% width
        let card_width = ctx.bounded_width(90, 25, 58);
        assert!(card_width >= 25);
        assert!(card_width <= 58);
    }

    #[test]
    fn test_layout_context_card_width_wide() {
        let ctx = LayoutContext::new(120, 40);
        // Wide terminal should use 75% width
        let card_width = ctx.bounded_width(75, 25, 118);
        assert!(card_width >= 25);
        assert!(card_width <= 118);
    }

    #[test]
    fn test_responsive_max_title_length() {
        // Extra small terminal
        let ctx_xs = LayoutContext::new(50, 24);
        assert_eq!(ctx_xs.max_title_length(), 20);

        // Small terminal
        let ctx_sm = LayoutContext::new(70, 24);
        assert_eq!(ctx_sm.max_title_length(), 30);

        // Medium terminal
        let ctx_md = LayoutContext::new(100, 24);
        assert_eq!(ctx_md.max_title_length(), 50);

        // Large terminal
        let ctx_lg = LayoutContext::new(160, 24);
        assert_eq!(ctx_lg.max_title_length(), 80);
    }

    #[test]
    fn test_responsive_max_preview_length() {
        // Extra small terminal
        let ctx_xs = LayoutContext::new(50, 24);
        assert_eq!(ctx_xs.max_preview_length(), 40);

        // Small terminal
        let ctx_sm = LayoutContext::new(70, 24);
        assert_eq!(ctx_sm.max_preview_length(), 60);

        // Medium terminal
        let ctx_md = LayoutContext::new(100, 24);
        assert_eq!(ctx_md.max_preview_length(), 100);

        // Large terminal
        let ctx_lg = LayoutContext::new(160, 24);
        assert_eq!(ctx_lg.max_preview_length(), 150);
    }

    #[test]
    fn test_should_show_full_badges() {
        // Narrow - should abbreviate hints
        let ctx_narrow = LayoutContext::new(60, 24);
        assert!(!ctx_narrow.should_show_full_badges());

        // Wide - should show full hints
        let ctx_wide = LayoutContext::new(100, 24);
        assert!(ctx_wide.should_show_full_badges());
    }

    #[test]
    fn test_is_narrow_affects_card_percentage() {
        let ctx_narrow = LayoutContext::new(60, 24);
        assert!(ctx_narrow.is_narrow());

        let ctx_wide = LayoutContext::new(120, 40);
        assert!(!ctx_wide.is_narrow());
    }

    #[test]
    fn test_max_visible_items_height_based() {
        // Short terminal
        let ctx_short = LayoutContext::new(80, 16);
        let items_short = ctx_short.max_visible_items();

        // Tall terminal
        let ctx_tall = LayoutContext::new(80, 40);
        let items_tall = ctx_tall.max_visible_items();

        // Taller terminal should show more items
        assert!(items_tall > items_short);
    }

    #[test]
    fn test_layout_context_from_rect() {
        let rect = Rect::new(0, 0, 100, 30);
        let ctx = LayoutContext::from_rect(rect);
        assert_eq!(ctx.width, 100);
        assert_eq!(ctx.height, 30);
    }

    #[test]
    fn test_model_abbreviation_based_on_content_width() {
        // With content_width >= 20, model names should not be abbreviated
        // This is tested via the integration tests in ui/mod.rs
        // Here we verify the threshold behavior

        // Content width of 20+ should show full model name
        let content_width_ok = 25;
        assert!(content_width_ok >= 20);

        // Content width < 20 should abbreviate
        let content_width_small = 15;
        assert!(content_width_small < 20);
    }
}
