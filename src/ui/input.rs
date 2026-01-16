//! Input area rendering
//!
//! Implements the input box, keybind hints, and permission prompt UI.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::app::{App, Focus, Screen};
use crate::widgets::input_box::InputBoxWidget;

use super::theme::{COLOR_ACCENT, COLOR_BORDER, COLOR_DIM, COLOR_HEADER};

// ============================================================================
// Input Area
// ============================================================================

pub fn render_input_area(frame: &mut Frame, area: Rect, app: &App) {
    let input_focused = app.focus == Focus::Input;
    let border_color = if input_focused { COLOR_HEADER } else { COLOR_BORDER };

    let input_outer = Block::default()
        .borders(Borders::TOP)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(border_color));
    frame.render_widget(input_outer, area);

    let inner = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };

    let input_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // Input box (needs 5 for border + multi-line content)
            Constraint::Length(1), // Keybinds
        ])
        .split(inner);

    // Render the InputBox widget (never streaming on CommandDeck)
    let input_widget = InputBoxWidget::new(&app.input_box, "", input_focused);
    frame.render_widget(input_widget, input_chunks[0]);

    // Build contextual keybind hints
    let keybinds = build_contextual_keybinds(app);

    let keybinds_widget = Paragraph::new(keybinds);
    frame.render_widget(keybinds_widget, input_chunks[1]);
}

/// Render the input area for conversation screen
pub fn render_conversation_input(frame: &mut Frame, area: Rect, app: &App) {
    let input_focused = app.focus == Focus::Input;
    let is_streaming = app.is_streaming();
    let border_color = if input_focused { COLOR_HEADER } else { COLOR_BORDER };

    let input_outer = Block::default()
        .borders(Borders::TOP)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(border_color));
    frame.render_widget(input_outer, area);

    let inner = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };

    let input_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // Input box
            Constraint::Length(1), // Keybinds
        ])
        .split(inner);

    // Render the InputBox widget with appropriate border style
    let input_widget = if is_streaming {
        InputBoxWidget::dashed(&app.input_box, "", input_focused)
    } else {
        InputBoxWidget::new(&app.input_box, "", input_focused)
    };
    frame.render_widget(input_widget, input_chunks[0]);

    // Build contextual keybind hints
    let keybinds = build_contextual_keybinds(app);

    let keybinds_widget = Paragraph::new(keybinds);
    frame.render_widget(keybinds_widget, input_chunks[1]);
}

/// Build contextual keybind hints based on application state
pub fn build_contextual_keybinds(app: &App) -> Line<'static> {
    let mut spans = vec![Span::raw(" ")];

    // Check for visible elements that need special keybinds
    let has_error = app.stream_error.is_some();

    // Always show basic navigation
    if app.screen == Screen::Conversation {
        if app.is_active_thread_programming() {
            // Programming thread: show mode cycling hint
            spans.push(Span::styled("[Shift+Tab]", Style::default().fg(COLOR_ACCENT)));
            spans.push(Span::raw(" cycle mode │ "));
        }

        if has_error {
            // Error visible: show dismiss hint
            spans.push(Span::styled("d", Style::default().fg(COLOR_ACCENT)));
            spans.push(Span::raw(": dismiss error │ "));
        }

        spans.push(Span::styled("[Enter]", Style::default().fg(COLOR_ACCENT)));
        spans.push(Span::raw(" send │ "));

        spans.push(Span::styled("[Esc]", Style::default().fg(COLOR_ACCENT)));
        spans.push(Span::raw(" back"));
    } else {
        // CommandDeck screen
        spans.push(Span::styled("[Tab Tab]", Style::default().fg(COLOR_ACCENT)));
        spans.push(Span::raw(" switch thread │ "));

        spans.push(Span::styled("[Enter]", Style::default().fg(COLOR_ACCENT)));
        spans.push(Span::raw(" send │ "));

        spans.push(Span::styled("[Esc]", Style::default().fg(COLOR_ACCENT)));
        spans.push(Span::raw(" back"));
    }

    Line::from(spans)
}

// ============================================================================
// Inline Permission Prompt
// ============================================================================

/// Render an inline permission prompt in the message flow.
///
/// Shows a Claude Code-style permission box with:
/// - Tool name and description
/// - Preview of the action (file path, command, etc.)
/// - Keyboard options: [y] Yes, [a] Always, [n] No
pub fn render_permission_prompt(frame: &mut Frame, area: Rect, app: &App) {
    if let Some(ref perm) = app.session_state.pending_permission {
        render_permission_box(frame, area, perm);
    }
}

/// Render the permission request box.
pub fn render_permission_box(frame: &mut Frame, area: Rect, perm: &crate::state::session::PermissionRequest) {
    // Calculate box dimensions - center in the given area
    let box_width = 60u16.min(area.width.saturating_sub(4));
    let box_height = 10u16.min(area.height.saturating_sub(2));

    // Center the box
    let x = area.x + (area.width.saturating_sub(box_width)) / 2;
    let y = area.y + (area.height.saturating_sub(box_height)) / 2;

    let box_area = Rect {
        x,
        y,
        width: box_width,
        height: box_height,
    };

    // Create the permission box with border
    let block = Block::default()
        .title(Span::styled(
            " Permission Required ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Yellow));

    // Render the block first
    frame.render_widget(block, box_area);

    // Inner area for content
    let inner = Rect {
        x: box_area.x + 2,
        y: box_area.y + 1,
        width: box_area.width.saturating_sub(4),
        height: box_area.height.saturating_sub(2),
    };

    // Build content lines
    let mut lines: Vec<Line> = Vec::new();

    // Tool name line
    lines.push(Line::from(vec![
        Span::styled(
            format!("{}: ", perm.tool_name),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(&perm.description, Style::default().fg(Color::White)),
    ]));

    lines.push(Line::from("")); // Empty line

    // Preview box - show context or tool_input
    let preview_content = get_permission_preview(perm);
    if !preview_content.is_empty() {
        // Preview border top
        lines.push(Line::from(vec![
            Span::styled(
                format!("┌{}┐", "─".repeat((inner.width as usize).saturating_sub(2))),
                Style::default().fg(COLOR_DIM),
            ),
        ]));

        // Preview content (truncated if needed)
        let max_preview_width = (inner.width as usize).saturating_sub(4);
        for line in preview_content.lines().take(3) {
            let truncated = if line.len() > max_preview_width {
                format!("{}...", &line[..max_preview_width.saturating_sub(3)])
            } else {
                line.to_string()
            };
            lines.push(Line::from(vec![
                Span::styled("│ ", Style::default().fg(COLOR_DIM)),
                Span::styled(truncated, Style::default().fg(Color::Gray)),
                Span::raw(" "),
            ]));
        }

        // Preview border bottom
        lines.push(Line::from(vec![
            Span::styled(
                format!("└{}┘", "─".repeat((inner.width as usize).saturating_sub(2))),
                Style::default().fg(COLOR_DIM),
            ),
        ]));
    }

    lines.push(Line::from("")); // Empty line

    // Keyboard options
    lines.push(Line::from(vec![
        Span::styled("[y]", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::raw(" Yes  "),
        Span::styled("[a]", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
        Span::raw(" Always  "),
        Span::styled("[n]", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        Span::raw(" No"),
    ]));

    let content = Paragraph::new(lines);
    frame.render_widget(content, inner);
}

/// Extract preview content from a PermissionRequest.
pub fn get_permission_preview(perm: &crate::state::session::PermissionRequest) -> String {
    // First try context (human-readable description)
    if let Some(ref ctx) = perm.context {
        return ctx.clone();
    }

    // Fall back to tool_input if available
    if let Some(ref input) = perm.tool_input {
        // Try to extract common fields
        if let Some(path) = input.get("file_path").and_then(|v| v.as_str()) {
            return path.to_string();
        }
        if let Some(cmd) = input.get("command").and_then(|v| v.as_str()) {
            return cmd.to_string();
        }
        if let Some(content) = input.get("content").and_then(|v| v.as_str()) {
            // Truncate long content (respecting UTF-8 boundaries)
            if content.len() > 100 {
                return super::helpers::truncate_string(content, 100);
            }
            return content.to_string();
        }
        // Fallback: pretty print JSON
        if let Ok(pretty) = serde_json::to_string_pretty(input) {
            return pretty;
        }
    }

    String::new()
}
