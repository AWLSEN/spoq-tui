//! Folder Picker Overlay rendering
//!
//! Implements the @ folder picker overlay for quick folder selection.
//! Shows filtered folders with keyboard navigation.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::App;

use super::theme::{COLOR_ACCENT, COLOR_BORDER, COLOR_DIALOG_BG, COLOR_DIM, COLOR_HEADER};

#[cfg(test)]
use super::layout::LayoutContext;

/// Maximum visible rows in the folder picker
const MAX_VISIBLE_ROWS: usize = 6;

/// Calculate dialog width based on terminal dimensions (~60% width)
/// NOTE: This function is only used in tests after Round 1 refactoring
#[cfg(test)]
fn calculate_dialog_width(ctx: &LayoutContext, area_width: u16) -> u16 {
    if ctx.is_extra_small() {
        // Extra small: take most of the screen width, leave 4 cols margin
        area_width.saturating_sub(4).min(50)
    } else if ctx.is_narrow() {
        // Narrow: 60% of width, min 40, max 60
        ctx.bounded_width(60, 40, 60)
    } else {
        // Normal: 60% of width, min 50, max 80
        ctx.bounded_width(60, 50, 80)
    }
}

/// Calculate dialog height based on content
fn calculate_dialog_height(visible_count: usize, area_height: u16) -> u16 {
    // Height: 2 (borders) + 1 (filter line) + visible_count + 1 (hint line)
    let content_height = (visible_count as u16) + 4;

    // Cap at reasonable max based on terminal height
    let max_height = area_height.saturating_sub(6);
    content_height.min(max_height)
}

/// Render the folder picker dialog as a bottom-anchored overlay
pub fn render_folder_picker(frame: &mut Frame, app: &App, input_area: Rect) {
    if !app.folder_picker_visible {
        return;
    }

    let area = frame.area();

    // Get filtered folders
    let filtered_folders = app.filtered_folders();
    let visible_count = filtered_folders.len().min(MAX_VISIBLE_ROWS);

    // Calculate dimensions - use full width of input area
    let dialog_width = input_area.width;
    let dialog_height = calculate_dialog_height(visible_count.max(1), area.height);

    // Position: horizontally aligned with input area, bottom-anchored (above input area)
    let x = input_area.x;
    let y = input_area.y.saturating_sub(dialog_height);

    let dialog_area = Rect {
        x,
        y,
        width: dialog_width,
        height: dialog_height,
    };

    // Clear the background behind the dialog
    frame.render_widget(Clear, dialog_area);

    // Create the dialog border with solid background
    let title = if app.folder_picker_filter.is_empty() {
        " Select Folder ".to_string()
    } else {
        format!(" @{} ", app.folder_picker_filter)
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
        .border_style(Style::default().fg(COLOR_BORDER))
        .style(Style::default().bg(COLOR_DIALOG_BG));

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

    // Calculate max width for folder name and path
    let available_width = inner.width as usize;

    if app.folders.is_empty() {
        // Loading state
        lines.push(Line::from(vec![
            Span::styled("Loading folders...", Style::default().fg(COLOR_DIM)),
        ]));
    } else if filtered_folders.is_empty() {
        // No matches state
        if app.folder_picker_filter.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("No folders available", Style::default().fg(COLOR_DIM)),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("No folders matching \"{}\"", app.folder_picker_filter),
                    Style::default().fg(COLOR_DIM),
                ),
            ]));
        }
    } else {
        // Calculate scroll offset if needed
        let scroll_offset = if app.folder_picker_cursor >= MAX_VISIBLE_ROWS {
            app.folder_picker_cursor - MAX_VISIBLE_ROWS + 1
        } else {
            0
        };

        // Show scroll up indicator if there are hidden folders above
        if scroll_offset > 0 {
            let indicator = format!("  {} more above", scroll_offset);
            lines.push(Line::from(vec![
                Span::styled(indicator, Style::default().fg(COLOR_DIM)),
            ]));
        }

        // Iterate through visible folders
        let visible_folders: Vec<_> = filtered_folders
            .iter()
            .enumerate()
            .skip(scroll_offset)
            .take(MAX_VISIBLE_ROWS)
            .collect();

        for (idx, folder) in visible_folders {
            let is_selected = idx == app.folder_picker_cursor;

            // Selection marker
            let marker = if is_selected { " " } else { "  " };
            let marker_style = if is_selected {
                Style::default().fg(COLOR_ACCENT).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(COLOR_DIM)
            };

            // Folder name and path
            let name = &folder.name;
            let path = &folder.path;

            // Calculate how much space we have for the path
            let marker_len = 2;
            let name_len = name.chars().count();
            let separator_len = 2; // "  " between name and path
            let remaining = available_width.saturating_sub(marker_len + name_len + separator_len);

            // Truncate path if needed
            let display_path = if path.chars().count() > remaining {
                let end = remaining.saturating_sub(3);
                let boundary = path
                    .char_indices()
                    .take_while(|(i, _)| *i <= end)
                    .last()
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                format!("{}...", &path[..boundary])
            } else {
                path.clone()
            };

            // Build the line with styles
            let name_style = if is_selected {
                Style::default().fg(COLOR_HEADER).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(COLOR_ACCENT)
            };

            let path_style = Style::default().fg(COLOR_DIM);

            let spans = vec![
                Span::styled(marker, marker_style),
                Span::styled(name, name_style),
                Span::styled("  ", Style::default()),
                Span::styled(display_path, path_style),
            ];

            lines.push(Line::from(spans));
        }

        // Show scroll down indicator if there are hidden folders below
        let folders_below = filtered_folders.len().saturating_sub(scroll_offset + MAX_VISIBLE_ROWS);
        if folders_below > 0 {
            let indicator = format!("  {} more below", folders_below);
            lines.push(Line::from(vec![
                Span::styled(indicator, Style::default().fg(COLOR_DIM)),
            ]));
        }
    }

    // Hint line
    lines.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled("//", Style::default().fg(COLOR_ACCENT)),
        Span::styled(": navigate  ", Style::default().fg(COLOR_DIM)),
        Span::styled("Enter", Style::default().fg(COLOR_ACCENT)),
        Span::styled(": select  ", Style::default().fg(COLOR_DIM)),
        Span::styled("Esc", Style::default().fg(COLOR_ACCENT)),
        Span::styled(": cancel", Style::default().fg(COLOR_DIM)),
    ]));

    let content = Paragraph::new(lines).style(Style::default().bg(COLOR_DIALOG_BG));
    frame.render_widget(content, inner);
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Dialog Width Tests
    // ========================================================================

    #[test]
    fn test_dialog_width_extra_small() {
        let ctx = LayoutContext::new(50, 14);
        let width = calculate_dialog_width(&ctx, 50);
        assert!(width <= 50);
        assert!(width >= 10);
    }

    #[test]
    fn test_dialog_width_narrow() {
        let ctx = LayoutContext::new(70, 24);
        let width = calculate_dialog_width(&ctx, 70);
        assert!(width >= 40);
        assert!(width <= 60);
    }

    #[test]
    fn test_dialog_width_normal() {
        let ctx = LayoutContext::new(120, 40);
        let width = calculate_dialog_width(&ctx, 120);
        assert!(width >= 50);
        assert!(width <= 80);
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
    fn test_dialog_height_with_items() {
        let height = calculate_dialog_height(6, 40);
        // 2 (borders) + 1 (filter) + 6 (items) + 1 (hint) = 10
        assert_eq!(height, 10);
    }

    #[test]
    fn test_dialog_height_clamped_to_area() {
        let height = calculate_dialog_height(10, 12);
        // Should be capped at area.height - 6 = 6
        assert!(height <= 12);
    }

    #[test]
    fn test_dialog_height_minimum_one_row() {
        // The function is called with .max(1) so visible_count is always >= 1
        // For visible_count=1: 2 (borders) + 1 (filter) + 1 (item) + 1 (hint) = 5
        let height = calculate_dialog_height(1, 40);
        assert_eq!(height, 5);
    }

    // ========================================================================
    // Max Visible Rows Constant
    // ========================================================================

    #[test]
    fn test_max_visible_rows_is_six() {
        assert_eq!(MAX_VISIBLE_ROWS, 6);
    }
}
