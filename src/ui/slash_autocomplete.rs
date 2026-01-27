//! Slash Command Autocomplete Dropdown rendering
//!
//! Implements the / slash command autocomplete dropdown for quick command selection.
//! Shows filtered commands with keyboard navigation.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::App;

use super::theme::{COLOR_ACCENT, COLOR_BORDER, COLOR_DIALOG_BG, COLOR_DIM, COLOR_HEADER};

/// Maximum visible rows in the autocomplete dropdown
const MAX_VISIBLE_ROWS: usize = 7;

/// Calculate dialog height based on content
fn calculate_dialog_height(visible_count: usize, area_height: u16) -> u16 {
    // Height: 2 (borders) + visible_count
    let content_height = (visible_count as u16) + 2;

    // Cap at reasonable max based on terminal height
    let max_height = area_height.saturating_sub(6);
    content_height.min(max_height)
}

/// Anchor mode for the autocomplete dropdown
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum AnchorMode {
    /// Bottom of dropdown anchored to input (dropdown grows upward) - used in CommandDeck
    Above,
    /// Top of dropdown anchored below input (dropdown grows downward) - used in Conversation
    Below,
}

/// Render the slash command autocomplete dropdown as an overlay
///
/// # Arguments
/// * `frame` - The ratatui frame to render into
/// * `app` - The application state containing autocomplete state
/// * `input_area` - The input field area to anchor the dropdown to
/// * `anchor_mode` - How to position the dropdown relative to input
pub fn render_slash_autocomplete_anchored(
    frame: &mut Frame,
    app: &App,
    input_area: Rect,
    anchor_mode: AnchorMode,
) {
    if !app.slash_autocomplete_visible {
        return;
    }

    let area = frame.area();

    // Get filtered commands
    let filtered_commands = app.filtered_slash_commands();
    let visible_count = filtered_commands.len().min(MAX_VISIBLE_ROWS);

    if filtered_commands.is_empty() {
        // Don't show dropdown if there are no matching commands
        return;
    }

    // Calculate dimensions - use a fixed width that's comfortable for command names
    let dialog_width = 50.min(input_area.width);
    let dialog_height = calculate_dialog_height(visible_count, area.height);

    // Position based on anchor mode
    let x = input_area.x;
    let y = match anchor_mode {
        AnchorMode::Above => {
            // Bottom-anchored: dropdown appears ABOVE input, grows upward
            input_area.y.saturating_sub(dialog_height)
        }
        AnchorMode::Below => {
            // Top-anchored: dropdown appears BELOW input, grows downward
            // Position just below the input line (input_area.y + input_area.height)
            let below_y = input_area.y + input_area.height;
            // Cap so it doesn't go off screen
            below_y.min(area.height.saturating_sub(dialog_height))
        }
    };

    let dialog_area = Rect {
        x,
        y,
        width: dialog_width,
        height: dialog_height,
    };

    // Clear the background behind the dialog
    frame.render_widget(Clear, dialog_area);

    // Create the dialog border with solid background
    let title = if app.slash_autocomplete_query.is_empty() {
        " Commands ".to_string()
    } else {
        format!(" /{} ", app.slash_autocomplete_query)
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

    // Calculate scroll offset if needed
    let scroll_offset = if app.slash_autocomplete_cursor >= MAX_VISIBLE_ROWS {
        app.slash_autocomplete_cursor - MAX_VISIBLE_ROWS + 1
    } else {
        0
    };

    // Show scroll up indicator if there are hidden commands above
    if scroll_offset > 0 {
        let indicator = format!("  {} more above", scroll_offset);
        lines.push(Line::from(vec![Span::styled(
            indicator,
            Style::default().fg(COLOR_DIM),
        )]));
    }

    // Render visible commands
    for (idx, command) in filtered_commands
        .iter()
        .skip(scroll_offset)
        .take(MAX_VISIBLE_ROWS)
        .enumerate()
    {
        let absolute_idx = idx + scroll_offset;
        let is_selected = absolute_idx == app.slash_autocomplete_cursor;

        // Build command line: "  /command - description"
        let command_text = format!("  {}", command.name());
        let description = command.description();

        let mut spans = Vec::new();

        if is_selected {
            // Highlighted (selected) command
            spans.push(Span::styled(
                "▸",
                Style::default()
                    .fg(COLOR_ACCENT)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(
                command_text,
                Style::default()
                    .fg(COLOR_ACCENT)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::raw(" - "));
            spans.push(Span::styled(
                description,
                Style::default().fg(COLOR_DIM),
            ));
        } else {
            // Non-selected command
            spans.push(Span::raw(" ")); // Space for the cursor
            spans.push(Span::raw(command_text));
            spans.push(Span::raw(" - "));
            spans.push(Span::styled(
                description,
                Style::default().fg(COLOR_DIM),
            ));
        }

        lines.push(Line::from(spans));
    }

    // Show scroll down indicator if there are more commands below
    let remaining = filtered_commands
        .len()
        .saturating_sub(scroll_offset + MAX_VISIBLE_ROWS);
    if remaining > 0 {
        let indicator = format!("  {} more below", remaining);
        lines.push(Line::from(vec![Span::styled(
            indicator,
            Style::default().fg(COLOR_DIM),
        )]));
    }

    // Render the content
    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

/// Render the slash command autocomplete dropdown (default: above input)
///
/// This is a convenience wrapper that uses `AnchorMode::Above` for backwards compatibility.
/// Use `render_slash_autocomplete_anchored` for explicit anchor control.
pub fn render_slash_autocomplete(frame: &mut Frame, app: &App, input_area: Rect) {
    render_slash_autocomplete_anchored(frame, app, input_area, AnchorMode::Above)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_dialog_height() {
        // With 2 visible items
        let height = calculate_dialog_height(2, 24);
        assert_eq!(height, 4); // 2 (borders) + 2 (items)

        // With more items than max visible
        let height = calculate_dialog_height(10, 24);
        assert_eq!(height, 12); // 2 (borders) + 10 (capped by max_height)
    }

    #[test]
    fn test_calculate_dialog_height_small_terminal() {
        // Very small terminal
        let height = calculate_dialog_height(5, 10);
        assert_eq!(height, 4); // Capped at area_height - 6
    }
}
