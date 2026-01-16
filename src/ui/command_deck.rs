//! Command Deck rendering
//!
//! Implements the main Command Deck UI with header, logo, and content layout.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::app::App;
use crate::state::TaskStatus;

use super::helpers::{format_tokens, inner_rect};
use super::input::render_input_area;
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
pub fn render_command_deck(frame: &mut Frame, app: &App) {
    let size = frame.area();

    // Main outer border
    let outer_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(COLOR_BORDER));
    frame.render_widget(outer_block, size);

    // Create main layout sections
    let inner = inner_rect(size, 1);
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(9),  // Header with logo
            Constraint::Min(10),    // Main content area
            Constraint::Length(6),  // Input area
        ])
        .split(inner);

    render_header(frame, main_chunks[0], app);
    render_main_content(frame, main_chunks[1], app);
    render_input_area(frame, main_chunks[2], app);
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

pub fn render_main_content(frame: &mut Frame, area: Rect, app: &App) {
    use crate::app::Focus;

    // Split into left and right panels
    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40), // Left panel
            Constraint::Percentage(60), // Right panel
        ])
        .split(area);

    render_left_panel(frame, content_chunks[0], app);
    render_right_panel(frame, content_chunks[1], app, app.focus == Focus::Threads);
}
