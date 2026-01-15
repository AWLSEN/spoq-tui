//! Left and Right panel rendering
//!
//! Implements the Notifications, Tasks/Todos, and Threads panels.

use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::app::{App, Focus};
use crate::state::{Notification, TodoStatus};

use super::helpers::{extract_short_model_name, inner_rect};
use super::theme::{COLOR_ACCENT, COLOR_ACTIVE, COLOR_BORDER, COLOR_DIM, COLOR_HEADER};

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

    render_notifications(frame, left_chunks[0], app, app.focus == Focus::Notifications);
    render_tasks(frame, left_chunks[1], app, app.focus == Focus::Tasks);
}

pub fn render_notifications(frame: &mut Frame, area: ratatui::layout::Rect, app: &App, focused: bool) {
    // Header styling changes based on focus
    let header_style = if focused {
        Style::default().fg(COLOR_HEADER).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(COLOR_ACCENT).add_modifier(Modifier::BOLD)
    };

    let mut lines = vec![
        Line::from(Span::styled(
            if focused { "◈ NOTIFICATIONS ◄" } else { "◈ NOTIFICATIONS" },
            header_style,
        )),
        Line::from(Span::styled(
            "─────────────────────────────",
            Style::default().fg(if focused { COLOR_ACCENT } else { COLOR_DIM }),
        )),
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

    for (i, notif) in mock_notifications.iter().take(area.height.saturating_sub(3) as usize).enumerate() {
        let time = notif.timestamp.format("%H:%M").to_string();
        let is_selected = focused && i == app.notifications_index;
        let marker = if is_selected { "▶ " } else { "▸ " };
        let marker_style = if is_selected {
            Style::default().fg(COLOR_HEADER)
        } else {
            Style::default().fg(COLOR_ACCENT)
        };

        lines.push(Line::from(vec![
            Span::styled(marker, marker_style),
            Span::styled(format!("[{}] ", time), Style::default().fg(COLOR_DIM)),
            Span::styled(
                &notif.message,
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

pub fn render_tasks(frame: &mut Frame, area: ratatui::layout::Rect, app: &App, focused: bool) {
    let border_color = if focused { COLOR_ACCENT } else { COLOR_DIM };
    let task_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(border_color));
    frame.render_widget(task_block.clone(), area);

    let inner = inner_rect(area, 1);

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
        Line::from(Span::styled(
            "─────────────────────────────",
            Style::default().fg(if focused { COLOR_ACCENT } else { COLOR_DIM }),
        )),
    ];

    // Render todos from app state
    if app.todos.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No todos yet",
            Style::default().fg(COLOR_DIM),
        )));
    } else {
        for todo in &app.todos {
            let (icon, color, text) = match todo.status {
                TodoStatus::Pending => ("[ ] ", COLOR_DIM, &todo.content),
                TodoStatus::InProgress => ("[◐] ", Color::Cyan, &todo.active_form),
                TodoStatus::Completed => ("[✓] ", Color::Green, &todo.content),
            };

            lines.push(Line::from(vec![
                Span::styled(icon, Style::default().fg(color)),
                Span::styled(
                    text,
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

    // Calculate centering padding for thread cards
    // Card width is 39 chars (including borders)
    let card_width: u16 = 39;
    let panel_width = inner.width;
    let left_padding = if panel_width > card_width {
        (panel_width - card_width) / 2
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

    for (i, (title, preview)) in threads_to_render.iter().enumerate() {
        let is_selected = focused && i == app.threads_index;
        let card_border_color = if is_selected { COLOR_HEADER } else { COLOR_BORDER };

        // Thread card top border (centered)
        lines.push(Line::from(vec![
            Span::raw(padding_str.clone()),
            Span::styled(
                "┌─────────────────────────────────────┐",
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

        // Calculate available width for title to ensure dots fit
        // Card width is 37 inner chars, minus "Thread: " (8) and marker (2) = 27 chars available
        // Reserve 3 chars for dots if streaming
        let max_title_len = if is_streaming { 24 } else { 27 };
        let display_title = if title.len() > max_title_len {
            format!("{}...", &title[..max_title_len.saturating_sub(3)])
        } else {
            title.clone()
        };

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
            format!("{:>width$}│", "", width = 35_usize.saturating_sub(10 + display_title.len() + dots.len())),
            Style::default().fg(card_border_color),
        ));

        lines.push(Line::from(title_spans));

        // Thread description (centered, if present)
        let thread = &cached_threads[i];
        if let Some(description) = &thread.description {
            if !description.is_empty() {
                // Max description length is 35 chars (card inner width minus borders and padding)
                let max_desc_len = 35;
                let display_desc = if description.len() > max_desc_len {
                    format!("{}...", &description[..max_desc_len.saturating_sub(3)])
                } else {
                    description.clone()
                };

                lines.push(Line::from(vec![
                    Span::raw(padding_str.clone()),
                    Span::styled("│   ", Style::default().fg(card_border_color)),
                    Span::styled(display_desc.clone(), Style::default().fg(COLOR_DIM)),
                    Span::styled(
                        format!("{:>width$}│", "", width = 35_usize.saturating_sub(4 + display_desc.len())),
                        Style::default().fg(card_border_color),
                    ),
                ]));
            }
        }

        // Thread type indicator and model info (centered)
        let type_indicator = match thread.thread_type {
            crate::models::ThreadType::Normal => "[N]",
            crate::models::ThreadType::Programming => "[P]",
        };

        let mut type_line_spans = vec![
            Span::raw(padding_str.clone()),
            Span::styled("│   ", Style::default().fg(card_border_color)),
            Span::styled(type_indicator, Style::default().fg(COLOR_ACCENT)),
        ];

        // Add model name if present
        if let Some(model) = &thread.model {
            let short_model = extract_short_model_name(model);
            type_line_spans.push(Span::styled(
                format!(" {}", short_model),
                Style::default().fg(COLOR_DIM),
            ));
            let type_info_len = type_indicator.len() + 1 + short_model.len();
            type_line_spans.push(Span::styled(
                format!("{:>width$}│", "", width = 35_usize.saturating_sub(4 + type_info_len)),
                Style::default().fg(card_border_color),
            ));
        } else {
            type_line_spans.push(Span::styled(
                format!("{:>width$}│", "", width = 35_usize.saturating_sub(4 + type_indicator.len())),
                Style::default().fg(card_border_color),
            ));
        }

        lines.push(Line::from(type_line_spans));

        // Thread preview (centered)
        lines.push(Line::from(vec![
            Span::raw(padding_str.clone()),
            Span::styled("│   ", Style::default().fg(card_border_color)),
            Span::styled(format!("\"{}\"", preview), Style::default().fg(COLOR_DIM)),
            Span::styled(
                format!("{:>width$}│", "", width = 35_usize.saturating_sub(4 + preview.len())),
                Style::default().fg(card_border_color),
            ),
        ]));

        // Thread card bottom border (centered)
        lines.push(Line::from(vec![
            Span::raw(padding_str.clone()),
            Span::styled(
                "└─────────────────────────────────────┘",
                Style::default().fg(card_border_color),
            ),
        ]));
        lines.push(Line::from(""));
    }

    // Keybind hints at bottom of threads panel (centered)
    lines.push(Line::from(vec![
        Span::raw(padding_str),
        Span::styled("[Shift+N]", Style::default().fg(COLOR_ACCENT)),
        Span::raw(" New Thread  "),
        Span::styled("[TAB]", Style::default().fg(COLOR_ACCENT)),
        Span::raw(" Switch Panel"),
    ]));

    let threads = Paragraph::new(lines);
    frame.render_widget(threads, inner);
}
