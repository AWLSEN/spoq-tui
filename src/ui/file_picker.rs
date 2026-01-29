//! File Picker Overlay rendering
//!
//! Implements the file picker overlay for conversation threads.
//! Allows selecting files from the thread's working_directory.
//!
//! Features:
//! - Directory navigation with .. to go back
//! - Multi-select file support
//! - Fuzzy filtering on file names
//! - Scroll support for large directories

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph},
    Frame,
};

use crate::state::FilePickerState;

use super::theme::{COLOR_ACCENT, COLOR_DIALOG_BG, COLOR_DIM, COLOR_HEADER};

/// Calculate dialog height based on content (borderless)
fn calculate_dialog_height(content_lines: usize, area_height: u16) -> u16 {
    // Height: content_lines only (no borders, hint is part of content)
    let content_height = content_lines as u16;

    // Cap at reasonable max based on terminal height
    let max_height = area_height.saturating_sub(4);
    content_height.min(max_height).max(3)
}

/// Format file size for display
fn format_size(size: Option<u64>) -> String {
    match size {
        None => String::new(),
        Some(bytes) if bytes < 1024 => format!("{} B", bytes),
        Some(bytes) if bytes < 1024 * 1024 => format!("{:.1} KB", bytes as f64 / 1024.0),
        Some(bytes) if bytes < 1024 * 1024 * 1024 => {
            format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
        }
        Some(bytes) => format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0)),
    }
}

/// Render the file picker dialog as a bottom-anchored overlay
///
/// # Arguments
/// * `frame` - The ratatui frame to render into
/// * `state` - The file picker state
/// * `input_area` - The input field area to anchor the picker to
pub fn render_file_picker(frame: &mut Frame, state: &FilePickerState, input_area: Rect) {
    if !state.visible {
        return;
    }

    let area = frame.area();

    // Build display lines
    let content_lines = build_picker_lines(state, input_area.width as usize);

    // Calculate dimensions
    let dialog_width = input_area.width;
    let line_count = content_lines.len().max(1);
    let dialog_height = calculate_dialog_height(line_count, area.height);

    // Position: BELOW input (Claude Code style)
    // Anchor is now at bottom of input, so render starting from anchor Y
    let x = input_area.x;

    // Check if there's room below, otherwise render above
    let space_below = area.height.saturating_sub(input_area.y);
    let y = if space_below >= dialog_height {
        // Render below anchor (preferred - Claude Code style)
        input_area.y
    } else {
        // Fallback: render above if not enough room below
        input_area.y.saturating_sub(dialog_height)
    };

    let dialog_area = Rect {
        x,
        y,
        width: dialog_width,
        height: dialog_height,
    };

    // Clear the background behind the dialog
    frame.render_widget(Clear, dialog_area);

    // Borderless block with just background (Claude Code style)
    let block = Block::default().style(Style::default().bg(COLOR_DIALOG_BG));
    frame.render_widget(block, dialog_area);

    // Minimal inner padding (no border to account for)
    let inner = Rect {
        x: dialog_area.x + 1,
        y: dialog_area.y,
        width: dialog_area.width.saturating_sub(2),
        height: dialog_area.height,
    };

    // Render content lines
    let content = Paragraph::new(content_lines).style(Style::default().bg(COLOR_DIALOG_BG));
    frame.render_widget(content, inner);
}

/// Build the display lines for the picker
fn build_picker_lines(state: &FilePickerState, available_width: usize) -> Vec<Line<'static>> {
    let mut lines: Vec<Line> = Vec::new();

    // Show error if any
    if let Some(ref error) = state.error {
        lines.push(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(
                format!("Error: {}", error),
                Style::default().fg(ratatui::style::Color::Red),
            ),
        ]));
        lines.push(Line::from("")); // Spacing
    }

    // Loading state
    if state.loading {
        lines.push(Line::from(vec![Span::styled(
            "  Loading...",
            Style::default().fg(COLOR_DIM).add_modifier(Modifier::ITALIC),
        )]));
    } else if state.is_empty() {
        // Empty state
        if !state.query.is_empty() {
            lines.push(Line::from(vec![Span::styled(
                format!("  No files matching \"{}\"", state.query),
                Style::default().fg(COLOR_DIM),
            )]));
        } else {
            lines.push(Line::from(vec![Span::styled(
                "  No files in this directory",
                Style::default().fg(COLOR_DIM),
            )]));
        }
    } else {
        // Note: The ".." entry for "go back" is handled by the backend
        // (included in the items list when not at base path)

        // Scroll indicators
        if state.has_more_above() {
            lines.push(Line::from(vec![Span::styled(
                format!("  + {} more above", state.scroll_offset),
                Style::default().fg(COLOR_DIM),
            )]));
        }

        // Render visible items
        let visible = state.visible_items();
        for (rel_idx, item) in visible.iter().enumerate() {
            let abs_idx = state.scroll_offset + rel_idx;
            let is_selected = abs_idx == state.selected_index;
            let is_file_selected = state.is_selected(&item.path);

            let line = render_file_line(item, is_selected, is_file_selected, available_width);
            lines.push(line);
        }

        // Scroll down indicator
        if state.has_more_below() {
            let remaining = state.total_items() - (state.scroll_offset + visible.len());
            lines.push(Line::from(vec![Span::styled(
                format!("  + {} more below", remaining),
                Style::default().fg(COLOR_DIM),
            )]));
        }
    }

    // Selected count if any
    if state.selected_count() > 0 {
        lines.push(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(
                format!("Selected: {} file(s)", state.selected_count()),
                Style::default().fg(COLOR_ACCENT).add_modifier(Modifier::BOLD),
            ),
        ]));
    }

    // Hint line
    lines.push(build_hint_line(state));

    lines
}

/// Render a single file/directory line
fn render_file_line(
    item: &crate::models::file::FileEntry,
    is_cursor: bool,
    is_selected: bool,
    available_width: usize,
) -> Line<'static> {
    // Icons
    let icon = if item.is_dir { " " } else { " " };

    // Selection marker
    let marker = if is_cursor { "" } else { "  " };
    let marker_style = if is_cursor {
        Style::default()
            .fg(COLOR_ACCENT)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    // Name style
    let name_style = if is_cursor {
        Style::default()
            .fg(COLOR_HEADER)
            .add_modifier(Modifier::BOLD)
    } else if item.is_dir {
        Style::default().fg(COLOR_ACCENT)
    } else {
        Style::default()
    };

    // Build name with trailing / for directories
    let name = if item.is_dir {
        format!("{}/", item.name)
    } else {
        item.name.clone()
    };

    // Calculate remaining space for size/selected indicator
    let marker_len = 2;
    let icon_len = 2;
    let name_len = name.chars().count();
    let separator_len = 2;
    let remaining = available_width.saturating_sub(marker_len + icon_len + name_len + separator_len + 4);

    // Right-side info: size for files, "(go back)" for .., or selection mark
    let right_info = if item.is_dir {
        if item.name == ".." {
            "(go back)".to_string()
        } else {
            String::new()
        }
    } else {
        let size_str = format_size(item.size);
        if is_selected {
            if remaining > size_str.len() + 12 {
                format!("{}  ✓ selected", size_str)
            } else {
                "✓ selected".to_string()
            }
        } else {
            size_str
        }
    };

    let right_style = if is_selected {
        Style::default().fg(ratatui::style::Color::Green).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(COLOR_DIM)
    };

    let mut spans = vec![
        Span::styled(marker.to_string(), marker_style),
        Span::styled(icon.to_string(), Style::default().fg(COLOR_DIM)),
        Span::styled(name, name_style),
    ];

    if !right_info.is_empty() {
        // Calculate padding to right-align the info
        let content_len = marker_len + icon_len + name_len;
        let info_len = right_info.chars().count();
        let padding = available_width.saturating_sub(content_len + info_len + 6);

        if padding > 0 {
            spans.push(Span::styled(" ".repeat(padding), Style::default()));
        } else {
            spans.push(Span::styled("  ", Style::default()));
        }
        spans.push(Span::styled(right_info, right_style));
    }

    Line::from(spans)
}

/// Build the hint line based on current state
fn build_hint_line(state: &FilePickerState) -> Line<'static> {
    let mut spans = vec![Span::styled("  ", Style::default())];

    // Navigation
    spans.push(Span::styled("↑↓", Style::default().fg(COLOR_ACCENT)));
    spans.push(Span::styled(" nav", Style::default().fg(COLOR_DIM)));

    // Directory navigation
    if state.can_go_up() {
        spans.push(Span::styled(" │ ", Style::default().fg(COLOR_DIM)));
        spans.push(Span::styled("←", Style::default().fg(COLOR_ACCENT)));
        spans.push(Span::styled(" back", Style::default().fg(COLOR_DIM)));
    }

    spans.push(Span::styled(" │ ", Style::default().fg(COLOR_DIM)));
    spans.push(Span::styled("→", Style::default().fg(COLOR_ACCENT)));
    spans.push(Span::styled(" into", Style::default().fg(COLOR_DIM)));

    // Tab to select
    spans.push(Span::styled(" │ ", Style::default().fg(COLOR_DIM)));
    spans.push(Span::styled("Tab", Style::default().fg(COLOR_ACCENT)));
    spans.push(Span::styled(" select", Style::default().fg(COLOR_DIM)));

    // Enter to confirm
    spans.push(Span::styled(" │ ", Style::default().fg(COLOR_DIM)));
    spans.push(Span::styled("Enter", Style::default().fg(COLOR_ACCENT)));
    spans.push(Span::styled(" ok", Style::default().fg(COLOR_DIM)));

    // Esc to cancel
    spans.push(Span::styled(" │ ", Style::default().fg(COLOR_DIM)));
    spans.push(Span::styled("Esc", Style::default().fg(COLOR_ACCENT)));
    spans.push(Span::styled(" cancel", Style::default().fg(COLOR_DIM)));

    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size_bytes() {
        assert_eq!(format_size(Some(512)), "512 B");
    }

    #[test]
    fn test_format_size_kilobytes() {
        assert_eq!(format_size(Some(2048)), "2.0 KB");
    }

    #[test]
    fn test_format_size_megabytes() {
        assert_eq!(format_size(Some(5 * 1024 * 1024)), "5.0 MB");
    }

    #[test]
    fn test_format_size_none() {
        assert_eq!(format_size(None), "");
    }
}
