//! Command Deck rendering
//!
//! Implements the main Command Deck UI with header, logo, and content layout.
//! Uses the responsive layout system for fluid sizing based on terminal dimensions.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::app::{ActivePanel, App};
use crate::state::TaskStatus;

use super::conversation::{create_mode_indicator_line, render_mode_indicator};
use super::folder_picker::render_folder_picker;
use super::helpers::{format_tokens, inner_rect};
use super::input::{calculate_input_area_height, render_input_area};
use super::layout::LayoutContext;
use super::panels::{render_left_panel, render_right_panel};
use super::theme::{COLOR_ACCENT, COLOR_ACTIVE, COLOR_BORDER, COLOR_DIM, COLOR_HEADER, COLOR_QUEUED};

// ============================================================================
// SPOQ ASCII Logo
// ============================================================================

pub const SPOQ_LOGO: &[&str] = &[
    "███████╗██████╗  ██████╗  ██████╗ ",
    "██╔════╝██╔══██╗██╔═══██╗██╔═══██╗",
    "███████╗██████╔╝██║   ██║██║   ██║",
    "╚════██║██╔═══╝ ██║   ██║██║▄▄ ██║",
    "███████║██║     ╚██████╔╝╚██████╔╝",
    "╚══════╝╚═╝      ╚═════╝  ╚══▀▀═╝ ",
];

// ============================================================================
// Main Command Deck Rendering
// ============================================================================

/// Render the complete Command Deck UI
pub fn render_command_deck(frame: &mut Frame, app: &mut App) {
    let size = frame.area();

    // Create layout context from terminal dimensions stored in app state
    let ctx = LayoutContext::new(app.terminal_width, app.terminal_height);

    // Main outer border
    let outer_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(COLOR_BORDER));
    frame.render_widget(outer_block, size);

    // Create main layout sections with responsive heights
    let inner = inner_rect(size, 0);
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
                Constraint::Length(header_height),  // Header with logo (responsive)
                Constraint::Min(10),                // Main content area
                Constraint::Length(1),              // Mode indicator
                Constraint::Length(input_height),   // Input area (responsive)
            ])
            .split(inner);

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
                Constraint::Length(header_height),  // Header with logo (responsive)
                Constraint::Min(10),                // Main content area
                Constraint::Length(input_height),   // Input area (responsive)
            ])
            .split(inner);

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
    // Split header into: [margin] [logo] [spacer] [status info right-aligned]
    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(2),  // Left margin
            Constraint::Length(38), // Logo width
            Constraint::Min(1),     // Flexible spacer
            Constraint::Length(44), // Right-aligned status info (wider for connection status)
        ])
        .split(area);

    render_logo(frame, header_chunks[1]);
    render_header_info(frame, header_chunks[3], app);
}

pub fn render_logo(frame: &mut Frame, area: Rect) {
    let logo_lines: Vec<Line> = SPOQ_LOGO
        .iter()
        .map(|line| Line::from(Span::styled(*line, Style::default().fg(COLOR_HEADER))))
        .collect();

    let logo = Paragraph::new(logo_lines);
    frame.render_widget(logo, area);
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
            format!("{} skill{}", skills_count, if skills_count == 1 { "" } else { "s" }),
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
            badges_spans.push(Span::styled(
                provider,
                Style::default().fg(Color::Yellow),
            ));
            badges_spans.push(Span::styled("] ", Style::default().fg(COLOR_DIM)));
            if app.session_state.oauth_url.is_some() {
                badges_spans.push(Span::styled(
                    "(press 'o') ",
                    Style::default().fg(COLOR_DIM).add_modifier(Modifier::ITALIC),
                ));
            }
        }
    }

    // Connection status
    badges_spans.push(Span::styled(status_icon, Style::default().fg(status_color)));
    badges_spans.push(Span::raw(" "));
    badges_spans.push(Span::styled(status_text, Style::default().fg(status_color)));

    let mut lines = vec![
        Line::from(""),
        Line::from(badges_spans),
        Line::from(""),
    ];

    // Show migration progress if it's running
    if let Some(progress) = app.migration_progress {
        lines.push(Line::from(vec![
            Span::styled("[MIGRATING] ", Style::default().fg(COLOR_QUEUED)),
            Span::styled(
                format!("{}%", progress),
                Style::default().fg(COLOR_ACCENT),
            ),
        ]));
    }

    // Thread/task counts
    let thread_count = app.threads.len();
    let active_tasks = app.tasks.iter().filter(|t| t.status == TaskStatus::InProgress).count();

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

    let info = Paragraph::new(lines)
        .alignment(ratatui::layout::Alignment::Right);
    frame.render_widget(info, area);
}

// ============================================================================
// Main Content Area
// ============================================================================

/// Render the main content area with responsive panel layout.
///
/// When the terminal is narrow (< 60 cols), panels are stacked and only one
/// is shown at a time with a panel switcher indicator. Otherwise, panels are
/// shown side-by-side with fluid widths calculated by LayoutContext.
pub fn render_main_content(frame: &mut Frame, area: Rect, app: &App, ctx: &LayoutContext) {
    use crate::app::Focus;

    // Check if we should stack panels (narrow terminal mode)
    if ctx.should_collapse_sidebar() {
        // Stacked mode: show only one panel at a time with panel switcher
        render_stacked_panels(frame, area, app, ctx);
    } else {
        // Side-by-side mode: use responsive two-column widths
        let (left_width, right_width) = ctx.two_column_widths();

        let content_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(left_width),
                Constraint::Length(right_width),
            ])
            .split(area);

        render_left_panel(frame, content_chunks[0], app);
        render_right_panel(frame, content_chunks[1], app, app.focus == Focus::Threads);
    }
}

/// Render stacked panels for narrow terminals (< 60 cols).
///
/// Shows only one panel at a time with a panel switcher indicator at the top.
/// Users can switch between panels using a keyboard shortcut.
fn render_stacked_panels(frame: &mut Frame, area: Rect, app: &App, _ctx: &LayoutContext) {
    use crate::app::Focus;

    // Reserve space for panel switcher indicator at the top
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Panel switcher indicator
            Constraint::Min(5),    // Panel content
        ])
        .split(area);

    // Render panel switcher indicator
    render_panel_switcher(frame, chunks[0], app.active_panel);

    // Render the active panel
    match app.active_panel {
        ActivePanel::Left => {
            render_left_panel(frame, chunks[1], app);
        }
        ActivePanel::Right => {
            render_right_panel(frame, chunks[1], app, app.focus == Focus::Threads);
        }
    }
}

/// Render the panel switcher indicator for stacked layout mode.
///
/// Shows which panel is currently active and provides visual hint for switching.
fn render_panel_switcher(frame: &mut Frame, area: Rect, active_panel: ActivePanel) {
    let (left_style, right_style) = match active_panel {
        ActivePanel::Left => (
            Style::default().fg(COLOR_HEADER).add_modifier(Modifier::BOLD),
            Style::default().fg(COLOR_DIM),
        ),
        ActivePanel::Right => (
            Style::default().fg(COLOR_DIM),
            Style::default().fg(COLOR_HEADER).add_modifier(Modifier::BOLD),
        ),
    };

    let indicator = Line::from(vec![
        Span::styled(" [", Style::default().fg(COLOR_DIM)),
        Span::styled("◀ ", if active_panel == ActivePanel::Left {
            Style::default().fg(COLOR_ACCENT)
        } else {
            Style::default().fg(COLOR_DIM)
        }),
        Span::styled("Tasks", left_style),
        Span::styled(" | ", Style::default().fg(COLOR_DIM)),
        Span::styled("Threads", right_style),
        Span::styled(" ▶", if active_panel == ActivePanel::Right {
            Style::default().fg(COLOR_ACCENT)
        } else {
            Style::default().fg(COLOR_DIM)
        }),
        Span::styled("] ", Style::default().fg(COLOR_DIM)),
        Span::styled("(←/→ switch)", Style::default().fg(COLOR_DIM).add_modifier(Modifier::ITALIC)),
    ]);

    let paragraph = Paragraph::new(indicator);
    frame.render_widget(paragraph, area);
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{ActivePanel, App};
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
    fn test_narrow_terminal_triggers_stacked_mode() {
        // Terminal width < 60 should trigger stacked panels
        let ctx = LayoutContext::new(59, 24);
        assert!(ctx.should_collapse_sidebar());

        // Width >= 60 should use side-by-side layout
        let ctx_wide = LayoutContext::new(60, 24);
        assert!(!ctx_wide.should_collapse_sidebar());
    }

    #[test]
    fn test_responsive_header_height() {
        // Normal terminal
        let ctx_normal = LayoutContext::new(100, 40);
        assert_eq!(ctx_normal.header_height(), 9);

        // Compact terminal (narrow)
        let ctx_narrow = LayoutContext::new(60, 40);
        assert_eq!(ctx_narrow.header_height(), 3);

        // Compact terminal (short)
        let ctx_short = LayoutContext::new(100, 16);
        assert_eq!(ctx_short.header_height(), 3);
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

    #[test]
    fn test_two_column_widths_medium_terminal() {
        // Medium terminal (80-120 cols) should use 40/60 split
        let ctx = LayoutContext::new(100, 24);
        let (left, right) = ctx.two_column_widths();
        assert_eq!(left, 40);
        assert_eq!(right, 60);
    }

    #[test]
    fn test_two_column_widths_wide_terminal() {
        // Wide terminal (>= 120 cols) should cap left at 60
        let ctx = LayoutContext::new(200, 24);
        let (left, right) = ctx.two_column_widths();
        assert_eq!(left, 60);
        assert_eq!(right, 140);
    }

    // ========================================================================
    // Active Panel Tests
    // ========================================================================

    #[test]
    fn test_active_panel_default_is_left() {
        assert_eq!(ActivePanel::default(), ActivePanel::Left);
    }

    #[test]
    fn test_active_panel_equality() {
        assert_eq!(ActivePanel::Left, ActivePanel::Left);
        assert_eq!(ActivePanel::Right, ActivePanel::Right);
        assert_ne!(ActivePanel::Left, ActivePanel::Right);
    }

    #[test]
    fn test_active_panel_copy() {
        let panel = ActivePanel::Right;
        let copied = panel;
        assert_eq!(panel, copied);
    }

    #[test]
    fn test_app_initializes_with_left_panel_active() {
        let app = App::default();
        assert_eq!(app.active_panel, ActivePanel::Left);
    }

    // ========================================================================
    // SPOQ Logo Tests
    // ========================================================================

    #[test]
    fn test_spoq_logo_has_correct_line_count() {
        assert_eq!(SPOQ_LOGO.len(), 6);
    }

    #[test]
    fn test_spoq_logo_lines_are_consistent_width() {
        // All logo lines should be the same width for proper rendering
        let first_width = SPOQ_LOGO[0].chars().count();
        for line in SPOQ_LOGO.iter() {
            assert_eq!(
                line.chars().count(),
                first_width,
                "Logo line has inconsistent width: {}",
                line
            );
        }
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
