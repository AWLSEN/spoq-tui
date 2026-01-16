//! Thread Switcher Dialog rendering
//!
//! Implements the Ctrl+Tab thread switcher overlay similar to macOS app switcher.
//! Shows threads in MRU (Most Recently Used) order with keyboard navigation.

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
use super::theme::{COLOR_ACCENT, COLOR_BORDER, COLOR_DIM, COLOR_HEADER};

/// Maximum number of threads to show in the switcher
const MAX_VISIBLE_THREADS: usize = 8;

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

    // Calculate dialog dimensions
    let dialog_width = 50u16.min(area.width.saturating_sub(4));
    let visible_count = threads.len().min(MAX_VISIBLE_THREADS);
    // Height: 2 (borders) + 1 (padding top) + visible_count + 1 (padding bottom) + 1 (hint line)
    let dialog_height = (visible_count as u16 + 5).min(area.height.saturating_sub(4));

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

    // Create the dialog border
    let block = Block::default()
        .title(Span::styled(
            " Switch Thread ",
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
    lines.push(Line::from("")); // Top padding

    let selected_idx = app.thread_switcher.selected_index;
    let max_title_width = (inner.width as usize).saturating_sub(16); // Space for indicator and model

    for (idx, thread) in threads.iter().take(MAX_VISIBLE_THREADS).enumerate() {
        let is_selected = idx == selected_idx;

        // Selection marker
        let marker = if is_selected { "▶ " } else { "  " };
        let marker_style = if is_selected {
            Style::default().fg(COLOR_ACCENT).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(COLOR_DIM)
        };

        // Thread type indicator [C] or [P]
        let type_indicator = match thread.thread_type {
            ThreadType::Conversation => "[C]",
            ThreadType::Programming => "[P]",
        };
        let type_color = match thread.thread_type {
            ThreadType::Conversation => Color::Cyan,
            ThreadType::Programming => Color::Magenta,
        };

        // Model name (short form)
        let model_name = thread
            .model
            .as_ref()
            .map(|m| extract_short_model_name(m))
            .unwrap_or("--");

        // Truncate thread title if needed
        let title = if thread.title.len() > max_title_width {
            format!("{}...", &thread.title[..max_title_width.saturating_sub(3)])
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

        lines.push(Line::from(vec![
            Span::styled(marker, marker_style),
            Span::styled(type_indicator, Style::default().fg(type_color)),
            Span::raw(" "),
            Span::styled(model_name, model_style),
            Span::raw("  "),
            Span::styled(title, title_style),
        ]));
    }

    // If there are more threads than shown
    if threads.len() > MAX_VISIBLE_THREADS {
        lines.push(Line::from(vec![
            Span::styled(
                format!("  ... {} more", threads.len() - MAX_VISIBLE_THREADS),
                Style::default().fg(COLOR_DIM),
            ),
        ]));
    }

    lines.push(Line::from("")); // Bottom padding

    // Hint line
    lines.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled("Tab/↓", Style::default().fg(COLOR_ACCENT)),
        Span::styled(": next  ", Style::default().fg(COLOR_DIM)),
        Span::styled("↑", Style::default().fg(COLOR_ACCENT)),
        Span::styled(": prev  ", Style::default().fg(COLOR_DIM)),
        Span::styled("Esc", Style::default().fg(COLOR_ACCENT)),
        Span::styled(": cancel", Style::default().fg(COLOR_DIM)),
    ]));

    let content = Paragraph::new(lines);
    frame.render_widget(content, inner);
}
