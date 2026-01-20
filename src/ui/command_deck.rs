//! Command Deck rendering
//!
//! Implements the main Command Deck UI with header and content layout.
//! Uses the responsive layout system for fluid sizing based on terminal dimensions.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::app::App;
use crate::state::TaskStatus;
use crate::ui::dashboard::{render_dashboard, Theme};

use super::conversation::{create_mode_indicator_line, render_mode_indicator};
use super::folder_picker::render_folder_picker;
use super::helpers::format_tokens;
use super::input::{calculate_input_area_height, render_input_area};
use super::layout::LayoutContext;
use super::theme::{COLOR_ACCENT, COLOR_ACTIVE, COLOR_DIM, COLOR_QUEUED};

// ============================================================================
// Main Command Deck Rendering
// ============================================================================

/// Render the complete Command Deck UI
pub fn render_command_deck(frame: &mut Frame, app: &mut App) {
    let size = frame.area();

    // Create layout context from terminal dimensions stored in app state
    let ctx = LayoutContext::new(app.terminal_width, app.terminal_height);

    // Create main layout sections with responsive heights
    let header_height = ctx.header_height();
    // Input height is dynamic based on line count (hard wrap inserts actual newlines)
    let line_count = app.textarea.line_count();
    let input_height = calculate_input_area_height(line_count);

    // Check if we need to show mode indicator
    let mode_indicator_line = create_mode_indicator_line(app.permission_mode);

    if let Some(mode_line) = mode_indicator_line {
        // Layout with mode indicator (4 sections)
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(header_height), // Header (responsive)
                Constraint::Min(10),               // Main content area
                Constraint::Length(1),             // Mode indicator
                Constraint::Length(input_height),  // Input area (responsive)
            ])
            .split(size);

        render_header(frame, main_chunks[0], app);
        render_main_content(frame, main_chunks[1], app, &ctx);
        render_mode_indicator(frame, main_chunks[2], mode_line);
        render_input_area(frame, main_chunks[3], app);

        // Render folder picker overlay (if visible) - must be last for proper layering
        if app.folder_picker_visible {
            render_folder_picker(frame, app, main_chunks[3]);
        }
    } else {
        // Layout without mode indicator (3 sections)
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(header_height), // Header (responsive)
                Constraint::Min(10),               // Main content area
                Constraint::Length(input_height),  // Input area (responsive)
            ])
            .split(size);

        render_header(frame, main_chunks[0], app);
        render_main_content(frame, main_chunks[1], app, &ctx);
        render_input_area(frame, main_chunks[2], app);

        // Render folder picker overlay (if visible) - must be last for proper layering
        if app.folder_picker_visible {
            render_folder_picker(frame, app, main_chunks[2]);
        }
    }
}

// ============================================================================
// Header Section
// ============================================================================

pub fn render_header(frame: &mut Frame, area: Rect, app: &App) {
    // Split header into: [margin] [status info]
    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(2), // Left margin
            Constraint::Min(1),    // Status info fills remaining space
        ])
        .split(area);

    render_header_info(frame, header_chunks[1], app);
}

pub fn render_header_info(frame: &mut Frame, area: Rect, app: &App) {
    // Connection status indicator
    let (status_icon, status_text, status_color) = if app.connection_status {
        ("●", "Connected", Color::LightGreen)
    } else {
        ("○", "Disconnected", Color::Red)
    };

    // Build session badges line
    let mut badges_spans = vec![];

    // Skills count badge
    let skills_count = app.session_state.skills.len();
    if skills_count > 0 {
        badges_spans.push(Span::styled("[", Style::default().fg(COLOR_DIM)));
        badges_spans.push(Span::styled(
            format!(
                "{} skill{}",
                skills_count,
                if skills_count == 1 { "" } else { "s" }
            ),
            Style::default().fg(COLOR_ACCENT),
        ));
        badges_spans.push(Span::styled("] ", Style::default().fg(COLOR_DIM)));
    }

    // Context usage badge
    if let Some(used) = app.session_state.context_tokens_used {
        badges_spans.push(Span::styled("[ctx: ", Style::default().fg(COLOR_DIM)));
        badges_spans.push(Span::styled(
            format_tokens(used),
            Style::default().fg(COLOR_ACCENT),
        ));
        if let Some(limit) = app.session_state.context_token_limit {
            badges_spans.push(Span::styled("/", Style::default().fg(COLOR_DIM)));
            badges_spans.push(Span::styled(
                format_tokens(limit),
                Style::default().fg(COLOR_DIM),
            ));
        }
        badges_spans.push(Span::styled("] ", Style::default().fg(COLOR_DIM)));
    }

    // OAuth status badge (flash if required)
    if app.session_state.needs_oauth() {
        if let Some((provider, _)) = &app.session_state.oauth_required {
            badges_spans.push(Span::styled("[OAuth: ", Style::default().fg(COLOR_DIM)));
            badges_spans.push(Span::styled(provider, Style::default().fg(Color::Yellow)));
            badges_spans.push(Span::styled("] ", Style::default().fg(COLOR_DIM)));
            if app.session_state.oauth_url.is_some() {
                badges_spans.push(Span::styled(
                    "(press 'o') ",
                    Style::default()
                        .fg(COLOR_DIM)
                        .add_modifier(Modifier::ITALIC),
                ));
            }
        }
    }

    // Connection status
    badges_spans.push(Span::styled(status_icon, Style::default().fg(status_color)));
    badges_spans.push(Span::raw(" "));
    badges_spans.push(Span::styled(status_text, Style::default().fg(status_color)));

    let mut lines = vec![Line::from(""), Line::from(badges_spans), Line::from("")];

    // Show migration progress if it's running
    if let Some(progress) = app.migration_progress {
        lines.push(Line::from(vec![
            Span::styled("[MIGRATING] ", Style::default().fg(COLOR_QUEUED)),
            Span::styled(format!("{}%", progress), Style::default().fg(COLOR_ACCENT)),
        ]));
    }

    // Thread/task counts
    let thread_count = app.threads.len();
    let active_tasks = app
        .tasks
        .iter()
        .filter(|t| t.status == TaskStatus::InProgress)
        .count();

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(
            format!("{} threads", thread_count),
            Style::default().fg(COLOR_DIM),
        ),
        Span::raw(" | "),
        Span::styled(
            format!("{} active", active_tasks),
            Style::default().fg(COLOR_ACTIVE),
        ),
    ]));

    let info = Paragraph::new(lines).alignment(ratatui::layout::Alignment::Right);
    frame.render_widget(info, area);
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
    let render_ctx = app.dashboard.build_render_context(
        &app.system_stats,
        &theme,
    );

    // Clear hit registry and render dashboard
    app.hit_registry.clear();
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
}
