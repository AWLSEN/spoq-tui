//! Input area rendering
//!
//! Implements the input box, keybind hints, and permission prompt UI.
//! This module provides responsive rendering that adapts to terminal dimensions.

mod folder_chip;
mod height;
mod keybinds;
mod permission;

// Re-export public APIs to maintain backwards compatibility
pub use folder_chip::{
    calculate_chip_width, format_chip_folder_name, render_folder_chip, COLOR_CHIP_BG,
    COLOR_CHIP_TEXT, MAX_CHIP_FOLDER_NAME_LEN,
};
pub use height::{calculate_input_area_height, calculate_input_box_height, MAX_INPUT_LINES};
pub use keybinds::{build_contextual_keybinds, build_responsive_keybinds};
pub use permission::{
    get_permission_preview, parse_ask_user_question, DEFAULT_PERMISSION_BOX_HEIGHT,
    DEFAULT_PERMISSION_BOX_WIDTH, MIN_PERMISSION_BOX_HEIGHT, MIN_PERMISSION_BOX_WIDTH,
};

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Widget},
    Frame,
};

use crate::app::App;
use crate::models::PermissionMode;

use super::layout::LayoutContext;
use super::theme::{COLOR_ACCENT, COLOR_DIM};

// ============================================================================
// Input Area
// ============================================================================

/// Render the input area with optional cursor blinking.
///
/// # Arguments
/// * `frame` - The frame to render into
/// * `area` - The area to render the input
/// * `app` - The application state
/// * `blink_enabled` - If true, cursor will blink based on tick_count. If false, cursor is static.
pub fn render_input_area_with_blink(
    frame: &mut Frame,
    area: Rect,
    app: &mut App,
    blink_enabled: bool,
) {
    // Input is always "focused" since we removed panel focus cycling
    let input_focused = true;

    // Calculate cursor visibility for blinking
    // When blink_enabled is true: cursor blinks based on tick_count
    // When blink_enabled is false: cursor is always visible (static)
    let cursor_visible = if blink_enabled && input_focused {
        // Blink: visible when (tick_count / 5) is even (same pattern used elsewhere in codebase)
        (app.tick_count / 5).is_multiple_of(2)
    } else {
        // Static: always visible when focused
        input_focused
    };

    // No border - use spacing at top instead for visual separation
    let inner = Rect {
        x: area.x + 1,
        y: area.y + 1, // 1 row spacing at top
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };

    // Calculate chip width if a folder is selected
    let chip_width = app
        .selected_folder
        .as_ref()
        .map(|f| calculate_chip_width(&f.name) + 1) // +1 for space after chip
        .unwrap_or(0);

    // Calculate content width (accounting for input box borders and chip)
    let content_width = inner.width.saturating_sub(2).saturating_sub(chip_width);

    // Set hard wrap width so auto-newlines are inserted during typing
    app.textarea.set_wrap_width(Some(content_width));

    // Calculate dynamic input box height based on line count
    let line_count = app.textarea.line_count();
    let input_box_height = calculate_input_box_height(line_count);

    let input_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(input_box_height), // Input box (dynamic height)
            Constraint::Length(1),                // Keybinds
        ])
        .split(inner);

    // Render the folder chip + input widget using our custom composite widget
    let input_with_chip = InputWithChipWidget {
        textarea_input: &mut app.textarea,
        focused: input_focused,
        cursor_visible,
        selected_folder: app.selected_folder.as_ref(),
    };
    frame.render_widget(input_with_chip, input_chunks[0]);

    // Build responsive keybind hints based on terminal dimensions
    let ctx = LayoutContext::new(app.terminal_width, app.terminal_height);
    let keybinds = build_responsive_keybinds(app, &ctx);

    let keybinds_widget = Paragraph::new(keybinds);
    frame.render_widget(keybinds_widget, input_chunks[1]);
}

/// Render the input area with static (non-blinking) cursor.
///
/// This is the default rendering mode used by the command deck (dashboard).
/// For conversation view with blinking cursor, use `render_input_area_with_blink`.
pub fn render_input_area(frame: &mut Frame, area: Rect, app: &mut App) {
    // Command deck uses static cursor (no blinking)
    render_input_area_with_blink(frame, area, app, false);
}

/// Widget that renders a folder chip followed by the TextArea input.
///
/// This composite widget handles:
/// - Rendering the folder chip at the start (if selected)
/// - Rendering the TextArea input in the remaining space
/// - Supporting cursor blinking via `cursor_visible` flag
struct InputWithChipWidget<'a, 'b> {
    textarea_input: &'b mut crate::widgets::textarea_input::TextAreaInput<'a>,
    focused: bool,
    /// Whether the cursor should be visible (for blinking support)
    cursor_visible: bool,
    selected_folder: Option<&'b crate::models::Folder>,
}

impl Widget for InputWithChipWidget<'_, '_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Create the outer border block
        let border_style = Style::default().fg(COLOR_DIM);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(border_style);

        // Render the border
        let inner_area = block.inner(area);
        block.render(area, buf);

        // If a folder is selected, render the chip at the start of the input
        let textarea_area = if let Some(folder) = self.selected_folder {
            let chip_width = calculate_chip_width(&folder.name);
            let spacing = 1u16; // Space after chip

            // Render the chip at the start of the inner area (top-left)
            render_folder_chip(buf, inner_area.x, inner_area.y, &folder.name);

            // Calculate remaining area for textarea
            let chip_total_width = chip_width + spacing;
            let textarea_x = inner_area.x + chip_total_width;
            let textarea_width = inner_area.width.saturating_sub(chip_total_width);

            Rect {
                x: textarea_x,
                y: inner_area.y,
                width: textarea_width,
                height: inner_area.height,
            }
        } else {
            inner_area
        };

        // Render textarea without border (we handle the border ourselves)
        // Pass cursor_visible to control whether cursor is shown (for blinking support)
        self.textarea_input.render_without_border_with_cursor(
            textarea_area,
            buf,
            self.focused,
            self.cursor_visible,
        );
    }
}

// ============================================================================
// Input Section Builder (Unified Scroll)
// ============================================================================

/// Build the input section as content lines for unified scroll with cursor blinking.
///
/// Returns lines for: top border, input content (with blinking cursor), bottom border, keybinds.
///
/// # Arguments
/// * `app` - The application state
/// * `viewport_width` - Width of the viewport in characters
/// * `cursor_visible` - Whether to show the cursor (for blink support)
pub fn build_input_section_with_cursor(
    app: &App,
    viewport_width: u16,
    cursor_visible: bool,
) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let border_width = viewport_width as usize;

    // 1. Mode indicator (only shown when Plan or Execute mode is active)
    match app.permission_mode {
        PermissionMode::Default => {}
        PermissionMode::Plan => {
            lines.push(Line::from(vec![Span::styled(
                "  [PLAN]",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            )]));
        }
        PermissionMode::BypassPermissions => {
            lines.push(Line::from(vec![Span::styled(
                "  [EXECUTE]",
                Style::default()
                    .fg(Color::Rgb(255, 140, 0))
                    .add_modifier(Modifier::BOLD),
            )]));
        }
    };

    // 2. Input top border (full-width horizontal line)
    lines.push(Line::from(Span::styled(
        "─".repeat(border_width),
        Style::default().fg(COLOR_ACCENT),
    )));

    // 3. Input content lines with optional blinking cursor
    let (cursor_row, cursor_col) = app.textarea.cursor();
    for (line_idx, text_line) in app.textarea.lines().iter().enumerate() {
        if cursor_visible && line_idx == cursor_row {
            // Insert cursor at the cursor position
            let text = text_line.as_str();
            let col = cursor_col.min(text.len());
            let before_cursor = text[..col].to_string();
            let cursor_char = if col < text.len() {
                text.chars().nth(col).map(|c| c.to_string()).unwrap_or_else(|| " ".to_string())
            } else {
                " ".to_string()
            };
            let after_cursor = if col + 1 < text.len() {
                text[col + cursor_char.len()..].to_string()
            } else {
                String::new()
            };

            lines.push(Line::from(vec![
                Span::raw(format!("  {}", before_cursor)),
                Span::styled(cursor_char, Style::default().fg(Color::Black).bg(Color::White)),
                Span::raw(after_cursor),
            ]));
        } else {
            lines.push(Line::from(format!("  {}", text_line)));
        }
    }

    // 4. Input bottom border (full-width horizontal line)
    lines.push(Line::from(Span::styled(
        "─".repeat(border_width),
        Style::default().fg(COLOR_ACCENT),
    )));

    // 5. Keybind hints
    lines.push(Line::from(vec![
        Span::styled("  Enter", Style::default().fg(COLOR_DIM)),
        Span::styled(" send ", Style::default().fg(COLOR_DIM)),
        Span::styled("|", Style::default().fg(COLOR_DIM)),
        Span::styled(" Shift+Enter", Style::default().fg(COLOR_DIM)),
        Span::styled(" newline ", Style::default().fg(COLOR_DIM)),
        Span::styled("|", Style::default().fg(COLOR_DIM)),
        Span::styled(" Esc", Style::default().fg(COLOR_DIM)),
        Span::styled(" menu", Style::default().fg(COLOR_DIM)),
    ]));

    lines
}

/// Build the input section as content lines for unified scroll.
///
/// This is a convenience wrapper around `build_input_section_with_cursor` that
/// uses `app.cursor_blink_visible` for cursor visibility.
///
/// Returns lines for: top border, input content (with blinking cursor), bottom border, keybinds.
pub fn build_input_section(app: &App, viewport_width: u16) -> Vec<Line<'static>> {
    build_input_section_with_cursor(app, viewport_width, app.cursor_blink_visible)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Folder;

    #[test]
    fn test_build_input_section_basic_structure() {
        let app = App::default();
        let lines = build_input_section(&app, 80);

        // Default mode: top border, content (1 line), bottom border, keybinds = 4 lines
        assert_eq!(lines.len(), 4);

        // First line should be top border
        let first_text: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(first_text.contains("─"), "First line should be top border");

        // Last line should be keybinds
        let last_text: String = lines[lines.len() - 1]
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert!(last_text.contains("Enter"), "Last line should be keybinds");
    }

    #[test]
    fn test_build_input_section_default_mode_no_indicator() {
        let app = App::default();
        let lines = build_input_section(&app, 80);

        // Default mode should NOT have a mode indicator line
        // Structure: top border, content, bottom border, keybinds = 4 lines
        assert_eq!(lines.len(), 4);

        // First line should be border, not mode indicator
        let first_text: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            first_text.contains("─"),
            "First line should be border in Default mode"
        );
    }

    #[test]
    fn test_build_input_section_plan_mode_indicator() {
        let app = App {
            permission_mode: PermissionMode::Plan,
            ..Default::default()
        };

        let lines = build_input_section(&app, 80);

        // Plan mode: mode indicator, top border, content, bottom border, keybinds = 5 lines
        assert_eq!(lines.len(), 5);

        // First line should show [PLAN] indicator
        let mode_line = &lines[0];
        assert!(
            !mode_line.spans.is_empty(),
            "Plan mode should show mode indicator"
        );

        let mode_text: String = mode_line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            mode_text.contains("[PLAN]"),
            "Plan mode should display '[PLAN]'"
        );

        // Verify magenta color styling
        if let Some(span) = mode_line.spans.first() {
            assert_eq!(
                span.style.fg,
                Some(Color::Magenta),
                "Plan mode indicator should be magenta"
            );
        }
    }

    #[test]
    fn test_build_input_section_execute_mode_indicator() {
        let app = App {
            permission_mode: PermissionMode::BypassPermissions,
            ..Default::default()
        };

        let lines = build_input_section(&app, 80);

        // Execute mode: mode indicator, top border, content, bottom border, keybinds = 5 lines
        assert_eq!(lines.len(), 5);

        // First line should show [EXECUTE] indicator
        let mode_line = &lines[0];
        assert!(
            !mode_line.spans.is_empty(),
            "BypassPermissions mode should show mode indicator"
        );

        let mode_text: String = mode_line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            mode_text.contains("[EXECUTE]"),
            "BypassPermissions mode should display '[EXECUTE]'"
        );

        // Verify orange color styling (RGB 255, 140, 0)
        if let Some(span) = mode_line.spans.first() {
            assert_eq!(
                span.style.fg,
                Some(Color::Rgb(255, 140, 0)),
                "Execute mode indicator should be orange"
            );
        }
    }

    #[test]
    fn test_build_input_section_folder_does_not_affect_structure() {
        let app = App {
            selected_folder: Some(Folder {
                name: "my-project".to_string(),
                path: "/path/to/project".to_string(),
            }),
            ..Default::default()
        };

        let lines = build_input_section(&app, 80);

        // Default mode should still have 4 lines (folder doesn't add mode indicator)
        assert_eq!(
            lines.len(),
            4,
            "Default mode should have same structure with folder selected"
        );
    }

    #[test]
    fn test_build_input_section_plan_mode_with_folder() {
        let app = App {
            permission_mode: PermissionMode::Plan,
            selected_folder: Some(Folder {
                name: "my-project".to_string(),
                path: "/path/to/project".to_string(),
            }),
            ..Default::default()
        };

        let lines = build_input_section(&app, 80);

        // Plan mode with folder: mode indicator, top border, content, bottom border, keybinds = 5 lines
        assert_eq!(lines.len(), 5);

        // First line should show [PLAN]
        let mode_line = &lines[0];
        let mode_text: String = mode_line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            mode_text.contains("[PLAN]"),
            "Plan mode should display '[PLAN]' even with folder selected"
        );
        // Folder chip should NOT appear in mode line
        assert!(
            !mode_text.contains("my-project"),
            "Folder chip should not appear in mode line"
        );
    }

    #[test]
    fn test_build_input_section_multiline_input() {
        let mut app = App::default();
        app.textarea
            .set_content("First line\nSecond line\nThird line");

        let lines = build_input_section(&app, 80);

        // Default mode with 3 content lines: top border, 3 content, bottom border, keybinds = 6 lines
        assert_eq!(lines.len(), 6);
    }

    #[test]
    fn test_build_input_section_borders_present() {
        let app = App::default();
        let lines = build_input_section(&app, 80);

        // Line 0 should be top border (full-width horizontal line, no corners)
        let top_border = &lines[0];
        let top_text: String = top_border
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert!(
            top_text.contains("─"),
            "Top border should contain horizontal line"
        );
        assert!(!top_text.contains("┌"), "Top border should not have corner");
        assert!(!top_text.contains("┐"), "Top border should not have corner");

        // Line 2 should be bottom border (before keybinds)
        let bottom_border = &lines[2];
        let bottom_text: String = bottom_border
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert!(
            bottom_text.contains("─"),
            "Bottom border should contain horizontal line"
        );
        assert!(
            !bottom_text.contains("└"),
            "Bottom border should not have corner"
        );
        assert!(
            !bottom_text.contains("┘"),
            "Bottom border should not have corner"
        );
    }

    #[test]
    fn test_build_input_section_keybind_hints() {
        let app = App::default();
        let lines = build_input_section(&app, 80);

        // Last line should be keybind hints
        let keybinds_line = &lines[lines.len() - 1];
        let keybinds_text: String = keybinds_line
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();

        // Should contain key keybind hints
        assert!(keybinds_text.contains("Enter"), "Should contain 'Enter'");
        assert!(keybinds_text.contains("send"), "Should contain 'send'");
        assert!(
            keybinds_text.contains("Shift+Enter"),
            "Should contain 'Shift+Enter'"
        );
        assert!(
            keybinds_text.contains("newline"),
            "Should contain 'newline'"
        );
        assert!(keybinds_text.contains("Esc"), "Should contain 'Esc'");
        assert!(keybinds_text.contains("menu"), "Should contain 'menu'");

        // Should use pipe separators between keybind hints
        assert!(
            keybinds_text.contains("|"),
            "Keybind hints should use '|' as separator"
        );

        // Verify there are multiple pipe separators (at least 2 for 3 keybinds)
        let pipe_count = keybinds_text.matches('|').count();
        assert!(
            pipe_count >= 2,
            "Should have at least 2 pipe separators, found {}",
            pipe_count
        );
    }

    #[test]
    fn test_build_input_section_keybind_styling() {
        let app = App::default();
        let lines = build_input_section(&app, 80);

        // Last line should be keybind hints
        let keybinds_line = &lines[lines.len() - 1];

        // All spans in keybind hints should use COLOR_DIM
        for span in &keybinds_line.spans {
            if !span.content.trim().is_empty() {
                assert_eq!(
                    span.style.fg,
                    Some(COLOR_DIM),
                    "Keybind span '{}' should use COLOR_DIM",
                    span.content
                );
            }
        }
    }

    #[test]
    fn test_build_input_section_narrow_viewport() {
        let app = App::default();
        let lines = build_input_section(&app, 40);

        // Should still render without panicking on narrow width
        // Default mode: 4 lines
        assert_eq!(lines.len(), 4);
    }

    // =========================================================================
    // Cursor Blink Tests
    // =========================================================================

    #[test]
    fn test_cursor_visibility_blink_enabled_tick_0() {
        // At tick_count = 0: (0 / 5) = 0, 0.is_multiple_of(2) = true (even)
        // Cursor should be visible
        let tick_count = 0u64;
        let blink_enabled = true;
        let focused = true;

        let cursor_visible = if blink_enabled && focused {
            (tick_count / 5).is_multiple_of(2)
        } else {
            focused
        };

        assert!(cursor_visible, "Cursor should be visible at tick_count=0");
    }

    #[test]
    fn test_cursor_visibility_blink_enabled_tick_5() {
        // At tick_count = 5: (5 / 5) = 1, 1.is_multiple_of(2) = false (odd)
        // Cursor should be hidden (blink off phase)
        let tick_count = 5u64;
        let blink_enabled = true;
        let focused = true;

        let cursor_visible = if blink_enabled && focused {
            (tick_count / 5).is_multiple_of(2)
        } else {
            focused
        };

        assert!(
            !cursor_visible,
            "Cursor should be hidden at tick_count=5 (blink off)"
        );
    }

    #[test]
    fn test_cursor_visibility_blink_enabled_tick_10() {
        // At tick_count = 10: (10 / 5) = 2, 2.is_multiple_of(2) = true (even)
        // Cursor should be visible again
        let tick_count = 10u64;
        let blink_enabled = true;
        let focused = true;

        let cursor_visible = if blink_enabled && focused {
            (tick_count / 5).is_multiple_of(2)
        } else {
            focused
        };

        assert!(
            cursor_visible,
            "Cursor should be visible at tick_count=10"
        );
    }

    #[test]
    fn test_cursor_visibility_blink_disabled() {
        // When blink_enabled = false, cursor should always be visible when focused
        let tick_count = 5u64; // Would be hidden if blink was enabled
        let blink_enabled = false;
        let focused = true;

        let cursor_visible = if blink_enabled && focused {
            (tick_count / 5).is_multiple_of(2)
        } else {
            focused
        };

        assert!(
            cursor_visible,
            "Cursor should always be visible when blink_enabled=false"
        );
    }

    #[test]
    fn test_cursor_visibility_not_focused() {
        // When not focused, cursor should never be visible regardless of blink setting
        let tick_count = 0u64;
        let blink_enabled = true;
        let focused = false;

        let cursor_visible = if blink_enabled && focused {
            (tick_count / 5).is_multiple_of(2)
        } else {
            focused
        };

        assert!(
            !cursor_visible,
            "Cursor should be hidden when not focused"
        );
    }

    #[test]
    fn test_cursor_blink_cycle() {
        // Test a full blink cycle: visible -> hidden -> visible
        let blink_enabled = true;
        let focused = true;

        let mut visible_count = 0;
        let mut hidden_count = 0;

        for tick in 0u64..20u64 {
            let cursor_visible = if blink_enabled && focused {
                (tick / 5).is_multiple_of(2)
            } else {
                focused
            };

            if cursor_visible {
                visible_count += 1;
            } else {
                hidden_count += 1;
            }
        }

        // Should have roughly equal visible/hidden time
        // Ticks 0-4 (5 ticks): visible (0/5=0, even)
        // Ticks 5-9 (5 ticks): hidden (1, odd)
        // Ticks 10-14 (5 ticks): visible (2, even)
        // Ticks 15-19 (5 ticks): hidden (3, odd)
        assert_eq!(visible_count, 10, "Should have 10 visible ticks");
        assert_eq!(hidden_count, 10, "Should have 10 hidden ticks");
    }
}
