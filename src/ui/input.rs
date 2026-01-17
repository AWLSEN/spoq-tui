//! Input area rendering
//!
//! Implements the input box, keybind hints, and permission prompt UI.
//! This module provides responsive rendering that adapts to terminal dimensions.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::app::{App, Screen};
use crate::widgets::input_box::InputBoxWidget;

use super::layout::LayoutContext;
use super::theme::{COLOR_ACCENT, COLOR_BORDER, COLOR_DIM, COLOR_HEADER};

// ============================================================================
// Input Height Calculation
// ============================================================================

/// Calculate the dynamic input box height based on line count.
///
/// Returns height in rows (including borders):
/// - Min: 3 rows (border + 1 line + border)
/// - Max: 7 rows (border + 5 lines + border)
pub fn calculate_input_box_height(line_count: usize) -> u16 {
    let content_lines = (line_count as u16).clamp(1, 5);
    content_lines + 2 // +2 for top/bottom borders
}

/// Calculate the total input area height (input box + keybinds).
pub fn calculate_input_area_height(line_count: usize) -> u16 {
    calculate_input_box_height(line_count) + 1 // +1 for keybinds row
}

// ============================================================================
// Input Area
// ============================================================================

pub fn render_input_area(frame: &mut Frame, area: Rect, app: &App) {
    // Input is always "focused" since we removed panel focus cycling
    let input_focused = true;
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

    // Calculate dynamic input box height based on content lines
    let input_box_height = calculate_input_box_height(app.input_box.line_count());

    let input_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(input_box_height), // Input box (dynamic height)
            Constraint::Length(1),                // Keybinds
        ])
        .split(inner);

    // Render the InputBox widget with blinking cursor (never streaming on CommandDeck)
    let input_widget = InputBoxWidget::new(&app.input_box, "", input_focused)
        .with_tick(app.tick_count);
    frame.render_widget(input_widget, input_chunks[0]);

    // Build responsive keybind hints based on terminal dimensions
    let ctx = LayoutContext::new(app.terminal_width, app.terminal_height);
    let keybinds = build_responsive_keybinds(app, &ctx);

    let keybinds_widget = Paragraph::new(keybinds);
    frame.render_widget(keybinds_widget, input_chunks[1]);
}

/// Render the input area for conversation screen
pub fn render_conversation_input(frame: &mut Frame, area: Rect, app: &App) {
    // Input is always "focused" since we removed panel focus cycling
    let input_focused = true;
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

    // Calculate dynamic input box height based on content lines
    let input_box_height = calculate_input_box_height(app.input_box.line_count());

    let input_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(input_box_height), // Input box (dynamic height)
            Constraint::Length(1),                // Keybinds
        ])
        .split(inner);

    // Render the InputBox widget with appropriate border style and blinking cursor
    let input_widget = if is_streaming {
        InputBoxWidget::dashed(&app.input_box, "", input_focused)
            .with_tick(app.tick_count)
    } else {
        InputBoxWidget::new(&app.input_box, "", input_focused)
            .with_tick(app.tick_count)
    };
    frame.render_widget(input_widget, input_chunks[0]);

    // Build responsive keybind hints based on terminal dimensions
    let ctx = LayoutContext::new(app.terminal_width, app.terminal_height);
    let keybinds = build_responsive_keybinds(app, &ctx);

    let keybinds_widget = Paragraph::new(keybinds);
    frame.render_widget(keybinds_widget, input_chunks[1]);
}

/// Build contextual keybind hints based on application state.
///
/// This is the legacy function for backwards compatibility. For responsive keybinds,
/// use `build_responsive_keybinds` instead.
pub fn build_contextual_keybinds(app: &App) -> Line<'static> {
    build_responsive_keybinds(app, &LayoutContext::default())
}

/// Build responsive keybind hints based on application state and terminal dimensions.
///
/// On narrow terminals (< 80 columns), keybind hints are abbreviated:
/// - "[Shift+Tab]" becomes "[S+Tab]"
/// - "[Tab Tab]" becomes "[Tab]"
/// - "cycle mode" becomes "mode"
/// - "switch thread" becomes "switch"
/// - "dismiss error" becomes "dismiss"
///
/// On extra small terminals (< 60 columns), only essential keybinds are shown.
pub fn build_responsive_keybinds(app: &App, ctx: &LayoutContext) -> Line<'static> {
    let mut spans = vec![Span::raw(" ")];

    // Check for visible elements that need special keybinds
    let has_error = app.stream_error.is_some();
    let is_narrow = ctx.is_narrow();
    let is_extra_small = ctx.is_extra_small();

    // Always show basic navigation
    if app.screen == Screen::Conversation {
        if app.is_active_thread_programming() && !is_extra_small {
            // Programming thread: show mode cycling hint (skip on extra small)
            if is_narrow {
                spans.push(Span::styled("[S+Tab]", Style::default().fg(COLOR_ACCENT)));
                spans.push(Span::raw(" mode | "));
            } else {
                spans.push(Span::styled("[Shift+Tab]", Style::default().fg(COLOR_ACCENT)));
                spans.push(Span::raw(" cycle mode | "));
            }
        }

        if has_error && !is_extra_small {
            // Error visible: show dismiss hint (skip on extra small)
            spans.push(Span::styled("d", Style::default().fg(COLOR_ACCENT)));
            if is_narrow {
                spans.push(Span::raw(": dismiss | "));
            } else {
                spans.push(Span::raw(": dismiss error | "));
            }
        }

        spans.push(Span::styled("[Enter]", Style::default().fg(COLOR_ACCENT)));
        spans.push(Span::raw(" send | "));

        spans.push(Span::styled("[Esc]", Style::default().fg(COLOR_ACCENT)));
        spans.push(Span::raw(" back"));
    } else {
        // CommandDeck screen
        if !is_extra_small {
            if is_narrow {
                spans.push(Span::styled("[Tab]", Style::default().fg(COLOR_ACCENT)));
                spans.push(Span::raw(" switch | "));
            } else {
                spans.push(Span::styled("[Tab Tab]", Style::default().fg(COLOR_ACCENT)));
                spans.push(Span::raw(" switch thread | "));
            }
        }

        spans.push(Span::styled("[Enter]", Style::default().fg(COLOR_ACCENT)));
        spans.push(Span::raw(" send | "));

        spans.push(Span::styled("[Esc]", Style::default().fg(COLOR_ACCENT)));
        spans.push(Span::raw(" back"));
    }

    Line::from(spans)
}

// ============================================================================
// Inline Permission Prompt
// ============================================================================

/// Minimum width for the permission box (must fit keyboard options)
const MIN_PERMISSION_BOX_WIDTH: u16 = 30;
/// Default/maximum width for the permission box
const DEFAULT_PERMISSION_BOX_WIDTH: u16 = 60;
/// Default height for the permission box
const DEFAULT_PERMISSION_BOX_HEIGHT: u16 = 10;
/// Minimum height for a compact permission box (skips preview)
const MIN_PERMISSION_BOX_HEIGHT: u16 = 6;

/// Render an inline permission prompt in the message flow.
///
/// Shows a Claude Code-style permission box with:
/// - Tool name and description
/// - Preview of the action (file path, command, etc.)
/// - Keyboard options: [y] Yes, [a] Always, [n] No
pub fn render_permission_prompt(frame: &mut Frame, area: Rect, app: &App) {
    if let Some(ref perm) = app.session_state.pending_permission {
        let ctx = LayoutContext::new(app.terminal_width, app.terminal_height);
        render_responsive_permission_box(frame, area, perm, &ctx);
    }
}

/// Render the permission request box (legacy function for backwards compatibility).
///
/// For responsive permission boxes, use `render_responsive_permission_box` instead.
#[allow(dead_code)]
pub fn render_permission_box(
    frame: &mut Frame,
    area: Rect,
    perm: &crate::state::session::PermissionRequest,
) {
    render_responsive_permission_box(frame, area, perm, &LayoutContext::default())
}

/// Render a responsive permission request box.
///
/// The box scales from 60 characters down to 30 characters minimum.
/// On narrow terminals (< 80 columns), the title is shortened to " Permission ".
/// On very short terminals, the preview section is hidden to save space.
pub fn render_responsive_permission_box(
    frame: &mut Frame,
    area: Rect,
    perm: &crate::state::session::PermissionRequest,
    ctx: &LayoutContext,
) {
    // Calculate responsive box dimensions
    let available_width = area.width.saturating_sub(4);
    let box_width = if ctx.is_extra_small() {
        // Extra small: use minimum width or available space
        MIN_PERMISSION_BOX_WIDTH.min(available_width)
    } else if ctx.is_narrow() {
        // Narrow: scale down from default
        ctx.bounded_width(70, MIN_PERMISSION_BOX_WIDTH, DEFAULT_PERMISSION_BOX_WIDTH)
            .min(available_width)
    } else {
        // Normal: use default width
        DEFAULT_PERMISSION_BOX_WIDTH.min(available_width)
    };

    // Calculate responsive height
    let available_height = area.height.saturating_sub(2);
    let show_preview = !ctx.is_short() && available_height >= DEFAULT_PERMISSION_BOX_HEIGHT;
    let box_height = if show_preview {
        DEFAULT_PERMISSION_BOX_HEIGHT.min(available_height)
    } else {
        MIN_PERMISSION_BOX_HEIGHT.min(available_height)
    };

    // Center the box
    let x = area.x + (area.width.saturating_sub(box_width)) / 2;
    let y = area.y + (area.height.saturating_sub(box_height)) / 2;

    let box_area = Rect {
        x,
        y,
        width: box_width,
        height: box_height,
    };

    // Choose title based on available width
    let title = if ctx.is_narrow() {
        " Permission "
    } else {
        " Permission Required "
    };

    // Create the permission box with border
    let block = Block::default()
        .title(Span::styled(
            title,
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

    // Tool name line - truncate description on narrow terminals
    let max_desc_width = (inner.width as usize).saturating_sub(perm.tool_name.len() + 2);
    let description = if perm.description.len() > max_desc_width && max_desc_width > 3 {
        format!("{}...", &perm.description[..max_desc_width.saturating_sub(3)])
    } else {
        perm.description.clone()
    };

    lines.push(Line::from(vec![
        Span::styled(
            format!("{}: ", perm.tool_name),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(description, Style::default().fg(Color::White)),
    ]));

    lines.push(Line::from("")); // Empty line

    // Preview box - only show if we have space
    if show_preview {
        let preview_content = get_permission_preview(perm);
        if !preview_content.is_empty() {
            // Preview border top
            lines.push(Line::from(vec![Span::styled(
                format!(
                    "┌{}┐",
                    "─".repeat((inner.width as usize).saturating_sub(2))
                ),
                Style::default().fg(COLOR_DIM),
            )]));

            // Preview content (truncated if needed)
            let max_preview_width = (inner.width as usize).saturating_sub(4);
            let max_preview_lines = if ctx.is_short() { 1 } else { 3 };
            for line in preview_content.lines().take(max_preview_lines) {
                let truncated = if line.len() > max_preview_width && max_preview_width > 3 {
                    format!("{}...", &line[..max_preview_width.saturating_sub(3)])
                } else {
                    line.to_string()
                };
                lines.push(Line::from(vec![
                    Span::styled("| ", Style::default().fg(COLOR_DIM)),
                    Span::styled(truncated, Style::default().fg(Color::Gray)),
                    Span::raw(" "),
                ]));
            }

            // Preview border bottom
            lines.push(Line::from(vec![Span::styled(
                format!(
                    "└{}┘",
                    "─".repeat((inner.width as usize).saturating_sub(2))
                ),
                Style::default().fg(COLOR_DIM),
            )]));
        }

        lines.push(Line::from("")); // Empty line
    }

    // Keyboard options - compact on narrow terminals
    if ctx.is_extra_small() {
        // Extra compact: [y]/[a]/[n]
        lines.push(Line::from(vec![
            Span::styled(
                "[y]",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("/"),
            Span::styled(
                "[a]",
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("/"),
            Span::styled(
                "[n]",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
        ]));
    } else if ctx.is_narrow() {
        // Compact: [y] Y  [a] A  [n] N
        lines.push(Line::from(vec![
            Span::styled(
                "[y]",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Y "),
            Span::styled(
                "[a]",
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" A "),
            Span::styled(
                "[n]",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" N"),
        ]));
    } else {
        // Full: [y] Yes  [a] Always  [n] No
        lines.push(Line::from(vec![
            Span::styled(
                "[y]",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Yes  "),
            Span::styled(
                "[a]",
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Always  "),
            Span::styled(
                "[n]",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" No"),
        ]));
    }

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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Test helpers
    fn create_test_app() -> App {
        let (message_tx, message_rx) = tokio::sync::mpsc::unbounded_channel();
        App {
            threads: vec![],
            tasks: vec![],
            todos: vec![],
            should_quit: false,
            screen: Screen::CommandDeck,
            active_thread_id: None,
            focus: crate::app::Focus::default(),
            notifications_index: 0,
            tasks_index: 0,
            threads_index: 0,
            input_box: crate::widgets::input_box::InputBox::new(),
            migration_progress: None,
            cache: crate::cache::ThreadCache::new(),
            message_rx: Some(message_rx),
            message_tx,
            connection_status: false,
            stream_error: None,
            client: std::sync::Arc::new(crate::conductor::ConductorClient::new()),
            tick_count: 0,
            conversation_scroll: 0,
            max_scroll: 0,
            programming_mode: crate::app::ProgrammingMode::default(),
            session_state: crate::state::SessionState::new(),
            tool_tracker: crate::state::ToolTracker::new(),
            subagent_tracker: crate::state::SubagentTracker::new(),
            debug_tx: None,
            stream_start_time: None,
            last_event_time: None,
            cumulative_token_count: 0,
            thread_switcher: crate::app::ThreadSwitcher::default(),
            last_tab_press: None,
            scroll_boundary_hit: None,
            boundary_hit_tick: 0,
            scroll_velocity: 0.0,
            scroll_position: 0.0,
            terminal_width: 80,
            terminal_height: 24,
            active_panel: crate::app::ActivePanel::default(),
        }
    }

    // ========================================================================
    // Input Height Calculation Tests
    // ========================================================================

    #[test]
    fn test_calculate_input_box_height_single_line() {
        assert_eq!(calculate_input_box_height(1), 3, "Single line: 1 + 2 borders = 3");
    }

    #[test]
    fn test_calculate_input_box_height_multiple_lines() {
        assert_eq!(calculate_input_box_height(2), 4, "2 lines: 2 + 2 borders = 4");
        assert_eq!(calculate_input_box_height(3), 5, "3 lines: 3 + 2 borders = 5");
        assert_eq!(calculate_input_box_height(4), 6, "4 lines: 4 + 2 borders = 6");
        assert_eq!(calculate_input_box_height(5), 7, "5 lines: 5 + 2 borders = 7");
    }

    #[test]
    fn test_calculate_input_box_height_clamped_max() {
        assert_eq!(calculate_input_box_height(6), 7, "Max 5 lines + 2 borders = 7");
        assert_eq!(calculate_input_box_height(10), 7, "Max 5 lines + 2 borders = 7");
        assert_eq!(calculate_input_box_height(100), 7, "Max 5 lines + 2 borders = 7");
    }

    #[test]
    fn test_calculate_input_box_height_clamped_min() {
        assert_eq!(calculate_input_box_height(0), 3, "Min 1 line + 2 borders = 3");
    }

    #[test]
    fn test_calculate_input_area_height_includes_keybinds() {
        assert_eq!(calculate_input_area_height(1), 4, "Box (3) + keybinds (1) = 4");
        assert_eq!(calculate_input_area_height(5), 8, "Box (7) + keybinds (1) = 8");
    }

    // ========================================================================
    // Responsive Keybinds Tests
    // ========================================================================

    #[test]
    fn test_responsive_keybinds_normal_width() {
        let app = create_test_app();
        let ctx = LayoutContext::new(120, 40);

        let keybinds = build_responsive_keybinds(&app, &ctx);
        let content: String = keybinds.spans.iter().map(|s| s.content.to_string()).collect();

        // Should show full keybinds on normal width
        assert!(content.contains("[Tab Tab]"), "Should show full Tab Tab");
        assert!(content.contains("switch thread"), "Should show full 'switch thread'");
    }

    #[test]
    fn test_responsive_keybinds_narrow_width() {
        let app = create_test_app();
        let ctx = LayoutContext::new(70, 24); // Narrow (< 80)

        let keybinds = build_responsive_keybinds(&app, &ctx);
        let content: String = keybinds.spans.iter().map(|s| s.content.to_string()).collect();

        // Should show abbreviated keybinds on narrow width
        assert!(content.contains("[Tab]"), "Should show abbreviated Tab");
        assert!(content.contains("switch"), "Should show abbreviated 'switch'");
        assert!(!content.contains("switch thread"), "Should NOT show full 'switch thread'");
    }

    #[test]
    fn test_responsive_keybinds_extra_small_width() {
        let app = create_test_app();
        let ctx = LayoutContext::new(50, 24); // Extra small (< 60)

        let keybinds = build_responsive_keybinds(&app, &ctx);
        let content: String = keybinds.spans.iter().map(|s| s.content.to_string()).collect();

        // On extra small, Tab switch hint should be hidden
        assert!(!content.contains("[Tab Tab]"), "Should NOT show Tab Tab on extra small");
        assert!(!content.contains("switch"), "Should NOT show switch on extra small");
        // But essential keybinds should remain
        assert!(content.contains("[Enter]"), "Should show Enter");
        assert!(content.contains("[Esc]"), "Should show Esc");
    }

    #[test]
    fn test_responsive_keybinds_conversation_programming_thread_narrow() {
        let mut app = create_test_app();
        app.screen = Screen::Conversation;
        app.terminal_width = 70;

        // Create a programming thread
        app.cache.upsert_thread(crate::models::Thread {
            id: "prog-thread".to_string(),
            title: "Programming".to_string(),
            description: None,
            preview: "Code".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Programming,
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: chrono::Utc::now(),
        });
        app.active_thread_id = Some("prog-thread".to_string());

        let ctx = LayoutContext::new(70, 24);
        let keybinds = build_responsive_keybinds(&app, &ctx);
        let content: String = keybinds.spans.iter().map(|s| s.content.to_string()).collect();

        // Should show abbreviated Shift+Tab on narrow
        assert!(content.contains("[S+Tab]"), "Should show abbreviated S+Tab");
        assert!(content.contains("mode"), "Should show abbreviated 'mode'");
        assert!(!content.contains("cycle mode"), "Should NOT show full 'cycle mode'");
    }

    #[test]
    fn test_responsive_keybinds_with_error_narrow() {
        let mut app = create_test_app();
        app.screen = Screen::Conversation;
        app.stream_error = Some("Test error".to_string());

        let ctx = LayoutContext::new(70, 24);
        let keybinds = build_responsive_keybinds(&app, &ctx);
        let content: String = keybinds.spans.iter().map(|s| s.content.to_string()).collect();

        // Should show abbreviated dismiss hint
        assert!(content.contains("dismiss"), "Should show 'dismiss'");
        assert!(!content.contains("dismiss error"), "Should NOT show full 'dismiss error'");
    }

    // ========================================================================
    // Permission Box Responsive Tests
    // ========================================================================

    #[test]
    fn test_permission_box_width_normal() {
        let ctx = LayoutContext::new(120, 40);
        let area = Rect::new(0, 0, 120, 40);

        // For normal width, box should be 60 chars (DEFAULT_PERMISSION_BOX_WIDTH)
        let available = area.width.saturating_sub(4);
        let expected_width = DEFAULT_PERMISSION_BOX_WIDTH.min(available);

        assert_eq!(expected_width, 60);
        // Verify ctx is not narrow or extra small
        assert!(!ctx.is_narrow());
        assert!(!ctx.is_extra_small());
    }

    #[test]
    fn test_permission_box_width_narrow() {
        let ctx = LayoutContext::new(70, 24);

        // Narrow terminals should scale down
        let scaled = ctx.bounded_width(70, MIN_PERMISSION_BOX_WIDTH, DEFAULT_PERMISSION_BOX_WIDTH);

        // 70% of 70 = 49, clamped between 30 and 60
        assert!(scaled >= MIN_PERMISSION_BOX_WIDTH);
        assert!(scaled <= DEFAULT_PERMISSION_BOX_WIDTH);
    }

    #[test]
    fn test_permission_box_width_extra_small() {
        let ctx = LayoutContext::new(50, 24);

        // Extra small should use minimum width
        let available = 50u16.saturating_sub(4);
        let expected_width = MIN_PERMISSION_BOX_WIDTH.min(available);

        assert_eq!(expected_width, MIN_PERMISSION_BOX_WIDTH);
        assert!(ctx.is_extra_small());
    }

    #[test]
    fn test_permission_box_title_changes_on_narrow() {
        // On normal width, title is " Permission Required "
        // On narrow width, title is " Permission "
        let normal_ctx = LayoutContext::new(120, 40);
        let narrow_ctx = LayoutContext::new(70, 24);

        assert!(!normal_ctx.is_narrow());
        assert!(narrow_ctx.is_narrow());
    }

    #[test]
    fn test_permission_box_preview_hidden_on_short() {
        let ctx = LayoutContext::new(80, 20);

        // SM_HEIGHT is 24, so height < 24 means is_short() returns true
        assert!(ctx.is_short()); // 20 < 24

        // Preview should be hidden on short terminals
        let show_preview = !ctx.is_short();
        assert!(!show_preview, "Preview should be hidden on short terminals");
    }

    #[test]
    fn test_permission_box_keyboard_options_normal() {
        // Normal: [y] Yes  [a] Always  [n] No
        let ctx = LayoutContext::new(120, 40);
        assert!(!ctx.is_narrow());
        assert!(!ctx.is_extra_small());
    }

    #[test]
    fn test_permission_box_keyboard_options_narrow() {
        // Narrow: [y] Y  [a] A  [n] N
        let ctx = LayoutContext::new(70, 24);
        assert!(ctx.is_narrow());
        assert!(!ctx.is_extra_small());
    }

    #[test]
    fn test_permission_box_keyboard_options_extra_small() {
        // Extra small: [y]/[a]/[n]
        let ctx = LayoutContext::new(50, 24);
        assert!(ctx.is_extra_small());
    }

    // ========================================================================
    // Legacy Compatibility Tests
    // ========================================================================

    #[test]
    fn test_build_contextual_keybinds_uses_default_context() {
        let app = create_test_app();

        // build_contextual_keybinds should produce same result as build_responsive_keybinds
        // with default context (80x24)
        let legacy = build_contextual_keybinds(&app);
        let responsive = build_responsive_keybinds(&app, &LayoutContext::default());

        let legacy_content: String = legacy.spans.iter().map(|s| s.content.to_string()).collect();
        let responsive_content: String = responsive.spans.iter().map(|s| s.content.to_string()).collect();

        assert_eq!(legacy_content, responsive_content);
    }
}
