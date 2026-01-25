//! Command Deck rendering
//!
//! Implements the main Command Deck UI with header and content layout.
//! Uses the responsive layout system for fluid sizing based on terminal dimensions.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    Frame,
};

use crate::app::App;
use crate::ui::dashboard::{render_dashboard, Theme};

use super::conversation::{create_mode_indicator_line, render_mode_indicator};
use super::folder_picker::render_folder_picker;
use super::input::{calculate_input_area_height, render_input_area};
use super::layout::LayoutContext;

// ============================================================================
// Helper Functions
// ============================================================================

/// Create a Ctrl+C warning indicator line if Ctrl+C was recently pressed
fn create_ctrl_c_indicator_line(
    last_ctrl_c_time: Option<std::time::Instant>,
) -> Option<Line<'static>> {
    last_ctrl_c_time.map(|_| {
        Line::from(vec![Span::styled(
            " Press Ctrl+C again to exit",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )])
    })
}

// ============================================================================
// Main Command Deck Rendering
// ============================================================================

/// Render the complete Command Deck UI
pub fn render_command_deck(frame: &mut Frame, app: &mut App) {
    let size = frame.area();

    // Create layout context from terminal dimensions stored in app state
    let ctx = LayoutContext::new(app.terminal_width, app.terminal_height);

    // Input height is dynamic based on line count (hard wrap inserts actual newlines)
    let line_count = app.textarea.line_count();
    let input_height = calculate_input_area_height(line_count);

    // Check if we need to show mode indicator or Ctrl+C warning
    let mode_indicator_line = create_mode_indicator_line(app.permission_mode);
    let ctrl_c_indicator_line = create_ctrl_c_indicator_line(app.last_ctrl_c_time);

    // Prefer Ctrl+C warning over mode indicator (more urgent)
    let indicator_line = ctrl_c_indicator_line.or(mode_indicator_line);

    if let Some(indicator) = indicator_line {
        // Layout with mode indicator (3 sections)
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(10),              // Main content area
                Constraint::Length(1),            // Mode indicator
                Constraint::Length(input_height), // Input area (responsive)
            ])
            .split(size);

        render_main_content(frame, main_chunks[0], app, &ctx);
        render_mode_indicator(frame, main_chunks[1], indicator);
        render_input_area(frame, main_chunks[2], app);

        // Render folder picker overlay (if visible) - must be last for proper layering
        if app.folder_picker_visible {
            render_folder_picker(frame, app, main_chunks[2]);
        }
    } else {
        // Layout without mode indicator (2 sections)
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(10),              // Main content area
                Constraint::Length(input_height), // Input area (responsive)
            ])
            .split(size);

        render_main_content(frame, main_chunks[0], app, &ctx);
        render_input_area(frame, main_chunks[1], app);

        // Render folder picker overlay (if visible) - must be last for proper layering
        if app.folder_picker_visible {
            render_folder_picker(frame, app, main_chunks[1]);
        }
    }
}

// ============================================================================
// Main Content Area
// ============================================================================

/// Render the main content area with the dashboard view.
///
/// Renders the multi-thread dashboard showing active threads, plans, and questions.
pub fn render_main_content(frame: &mut Frame, area: Rect, app: &mut App, _ctx: &LayoutContext) {
    render_dashboard_content(frame, area, app);
}

/// Render the new dashboard content view.
///
/// This builds a RenderContext from App state and calls render_dashboard
/// to display the multi-thread dashboard view.
fn render_dashboard_content(frame: &mut Frame, area: Rect, app: &mut App) {
    // Build the render context from app state
    let theme = Theme::default();
    let render_ctx = app
        .dashboard
        .build_render_context(&app.system_stats, &theme, &app.repos);

    // Note: hit_registry is cleared in prepare_render()
    render_dashboard(frame, area, &render_ctx, &mut app.hit_registry);
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::ui::layout::LayoutContext;

    // ========================================================================
    // Layout Context Integration Tests
    // ========================================================================

    #[test]
    fn test_layout_context_from_app_dimensions() {
        let app = App::default();
        let ctx = LayoutContext::new(app.terminal_width, app.terminal_height);

        // Default app dimensions are 80x24
        assert_eq!(ctx.width, 80);
        assert_eq!(ctx.height, 24);
    }

    #[test]
    fn test_responsive_header_height() {
        // Normal terminal
        let ctx_normal = LayoutContext::new(100, 40);
        assert_eq!(ctx_normal.header_height(), 3);

        // Compact terminal (narrow)
        let ctx_narrow = LayoutContext::new(60, 40);
        assert_eq!(ctx_narrow.header_height(), 2);

        // Compact terminal (short)
        let ctx_short = LayoutContext::new(100, 16);
        assert_eq!(ctx_short.header_height(), 2);
    }

    #[test]
    fn test_responsive_input_area_height() {
        // Normal terminal
        let ctx_normal = LayoutContext::new(100, 40);
        assert_eq!(ctx_normal.input_area_height(), 6);

        // Compact terminal
        let ctx_compact = LayoutContext::new(60, 40);
        assert_eq!(ctx_compact.input_area_height(), 4);
    }

    // ========================================================================
    // Mode Indicator Tests
    // ========================================================================

    #[test]
    fn test_mode_indicator_not_shown_for_default_mode() {
        use crate::models::PermissionMode;

        let app = App::default();
        assert_eq!(app.permission_mode, PermissionMode::Default);

        // create_mode_indicator_line should return None for Default mode
        let indicator = create_mode_indicator_line(app.permission_mode);
        assert!(indicator.is_none());
    }

    #[test]
    fn test_mode_indicator_shown_for_plan_mode() {
        use crate::models::PermissionMode;

        let app = App {
            permission_mode: PermissionMode::Plan,
            ..Default::default()
        };

        // create_mode_indicator_line should return Some for Plan mode
        let indicator = create_mode_indicator_line(app.permission_mode);
        assert!(indicator.is_some());

        // Verify the indicator text contains PLAN
        let line = indicator.unwrap();
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("PLAN"));
    }

    #[test]
    fn test_mode_indicator_shown_for_execute_mode() {
        use crate::models::PermissionMode;

        let app = App {
            permission_mode: PermissionMode::BypassPermissions,
            ..Default::default()
        };

        // create_mode_indicator_line should return Some for BypassPermissions mode
        let indicator = create_mode_indicator_line(app.permission_mode);
        assert!(indicator.is_some());

        // Verify the indicator text contains EXECUTE
        let line = indicator.unwrap();
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("EXECUTE"));
    }

    // ========================================================================
    // Ctrl+C Indicator Tests
    // ========================================================================

    #[test]
    fn test_ctrl_c_indicator_returns_none_when_not_set() {
        let indicator = create_ctrl_c_indicator_line(None);
        assert!(indicator.is_none());
    }

    #[test]
    fn test_ctrl_c_indicator_returns_some_when_set() {
        let now = std::time::Instant::now();
        let indicator = create_ctrl_c_indicator_line(Some(now));
        assert!(indicator.is_some());
    }

    #[test]
    fn test_ctrl_c_indicator_contains_correct_message() {
        let now = std::time::Instant::now();
        let indicator = create_ctrl_c_indicator_line(Some(now));
        assert!(indicator.is_some());

        let line = indicator.unwrap();
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("Press Ctrl+C again to exit"));
    }
}
