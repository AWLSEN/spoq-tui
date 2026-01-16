//! Conversation screen rendering
//!
//! Implements the conversation view with header, messages, and streaming indicator.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::app::{App, ProgrammingMode};
use crate::models::{MessageSegment, ToolEventStatus};

use super::helpers::{inner_rect, SPINNER_FRAMES};
use super::input::render_conversation_input;
use super::messages::render_messages_area;
use super::theme::{COLOR_BORDER, COLOR_DIM, COLOR_HEADER};

// ============================================================================
// Mode Indicator
// ============================================================================

/// Create the mode indicator line for programming threads.
///
/// Returns Some(Line) with the mode indicator styled appropriately:
/// - PlanMode: '[PLAN MODE]' in yellow
/// - BypassPermissions: '[BYPASS]' in red
/// - None: returns None (no indicator shown)
///
/// This should only be called when the active thread is a Programming thread.
pub fn create_mode_indicator_line(mode: ProgrammingMode) -> Option<Line<'static>> {
    match mode {
        ProgrammingMode::PlanMode => Some(Line::from(vec![
            Span::styled(
                " [PLAN MODE]",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ])),
        ProgrammingMode::BypassPermissions => Some(Line::from(vec![
            Span::styled(
                " [BYPASS]",
                Style::default()
                    .fg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            ),
        ])),
        ProgrammingMode::None => None,
    }
}

// ============================================================================
// Conversation Screen
// ============================================================================

/// Render the conversation screen with header, messages area, and input
pub fn render_conversation_screen(frame: &mut Frame, app: &App) {
    let size = frame.area();

    // Main outer border
    let outer_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(COLOR_BORDER));
    frame.render_widget(outer_block, size);

    // Determine if we should show the mode indicator
    let show_mode_indicator = app.is_active_thread_programming();
    let mode_indicator_line = if show_mode_indicator {
        create_mode_indicator_line(app.programming_mode)
    } else {
        None
    };

    // Determine if we should show the streaming indicator
    let show_streaming_indicator = app.is_streaming();

    // Create main layout sections - conditionally include mode and streaming indicators
    let inner = inner_rect(size, 1);

    match (mode_indicator_line, show_streaming_indicator) {
        (Some(mode_line), true) => {
            // Layout with both mode and streaming indicators (5 sections)
            let main_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),  // Thread header
                    Constraint::Min(10),    // Messages area
                    Constraint::Length(1),  // Streaming indicator
                    Constraint::Length(1),  // Mode indicator
                    Constraint::Length(8),  // Input area with keybinds
                ])
                .split(inner);

            render_conversation_header(frame, main_chunks[0], app);
            render_messages_area(frame, main_chunks[1], app);
            render_streaming_indicator(frame, main_chunks[2], app);
            render_mode_indicator(frame, main_chunks[3], mode_line);
            render_conversation_input(frame, main_chunks[4], app);
        }
        (Some(mode_line), false) => {
            // Layout with mode indicator only (4 sections)
            let main_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),  // Thread header
                    Constraint::Min(10),    // Messages area
                    Constraint::Length(1),  // Mode indicator
                    Constraint::Length(8),  // Input area with keybinds
                ])
                .split(inner);

            render_conversation_header(frame, main_chunks[0], app);
            render_messages_area(frame, main_chunks[1], app);
            render_mode_indicator(frame, main_chunks[2], mode_line);
            render_conversation_input(frame, main_chunks[3], app);
        }
        (None, true) => {
            // Layout with streaming indicator only (4 sections)
            let main_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),  // Thread header
                    Constraint::Min(10),    // Messages area
                    Constraint::Length(1),  // Streaming indicator
                    Constraint::Length(8),  // Input area with keybinds
                ])
                .split(inner);

            render_conversation_header(frame, main_chunks[0], app);
            render_messages_area(frame, main_chunks[1], app);
            render_streaming_indicator(frame, main_chunks[2], app);
            render_conversation_input(frame, main_chunks[3], app);
        }
        (None, false) => {
            // Layout without indicators (3 sections)
            let main_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),  // Thread header
                    Constraint::Min(10),    // Messages area
                    Constraint::Length(8),  // Input area with keybinds
                ])
                .split(inner);

            render_conversation_header(frame, main_chunks[0], app);
            render_messages_area(frame, main_chunks[1], app);
            render_conversation_input(frame, main_chunks[2], app);
        }
    }
}

/// Render the mode indicator bar
pub fn render_mode_indicator(frame: &mut Frame, area: Rect, mode_line: Line<'static>) {
    let indicator = Paragraph::new(mode_line);
    frame.render_widget(indicator, area);
}

/// Render the streaming indicator bar
pub fn render_streaming_indicator(frame: &mut Frame, area: Rect, app: &App) {
    // Get messages from cache if we have an active thread
    let cached_messages = app
        .active_thread_id
        .as_ref()
        .and_then(|id| app.cache.get_messages(id));

    // Check if any message is currently streaming
    let streaming_message = cached_messages
        .as_ref()
        .and_then(|msgs| msgs.iter().find(|m| m.is_streaming));

    if let Some(streaming_msg) = streaming_message {
        // Use dots spinner
        let spinner_index = (app.tick_count % 10) as usize;
        let spinner = SPINNER_FRAMES[spinner_index];

        // Find the last running tool event in the message
        let running_tool_name = streaming_msg.segments.iter().rev().find_map(|seg| {
            if let MessageSegment::ToolEvent(event) = seg {
                if event.status == ToolEventStatus::Running {
                    return Some(event.function_name.clone());
                }
            }
            None
        });

        let status_text = if let Some(tool_name) = running_tool_name {
            format!("Using {}...", tool_name)
        } else {
            "Responding...".to_string()
        };

        let indicator_line = Line::from(vec![
            Span::styled(
                format!("  {} {}", spinner, status_text),
                Style::default()
                    .fg(Color::DarkGray),
            ),
        ]);

        let indicator = Paragraph::new(indicator_line);
        frame.render_widget(indicator, area);
    }
}

/// Render the thread title header with connection status and badges
pub fn render_conversation_header(frame: &mut Frame, area: Rect, app: &App) {
    // Get thread title and description from cache or default
    let thread_info = app
        .active_thread_id
        .as_ref()
        .and_then(|id| app.cache.get_thread(id));

    let thread_title = thread_info
        .map(|t| t.title.as_str())
        .unwrap_or("New Conversation");

    let thread_description = thread_info.and_then(|t| t.description.clone());

    // Get model name from thread if available
    let model_name = thread_info.and_then(|t| t.model.clone());

    let header_block = Block::default()
        .borders(Borders::BOTTOM)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(COLOR_BORDER));

    // Build badges
    let mut badges: Vec<Span> = Vec::new();

    // Skills badge [skills: N]
    let skills_count = app.session_state.skills.len();
    if skills_count > 0 {
        badges.push(Span::styled(
            format!("[skills: {}] ", skills_count),
            Style::default().fg(Color::Cyan),
        ));
    }

    // Context progress bar [████░░░░░░] 42% or [░░░░░░░░░░] -- when no data
    let ctx_badge = match (app.session_state.context_tokens_used, app.session_state.context_token_limit) {
        (Some(used), Some(limit)) if limit > 0 => {
            let percentage = (used as f64 / limit as f64 * 100.0).round() as u32;
            let filled_blocks = (percentage / 10).min(10) as usize;
            let empty_blocks = 10 - filled_blocks;
            let bar = format!(
                "[{}{}] {}% ",
                "█".repeat(filled_blocks),
                "░".repeat(empty_blocks),
                percentage
            );
            bar
        }
        _ => "[░░░░░░░░░░] -- ".to_string(),
    };
    badges.push(Span::styled(ctx_badge, Style::default().fg(COLOR_DIM)));

    // Model badge [sonnet] if available
    if let Some(model) = model_name {
        badges.push(Span::styled(
            format!("[{}] ", model),
            Style::default().fg(Color::Magenta),
        ));
    }

    // Connection status badge
    let (status_icon, status_color) = if app.connection_status {
        ("●", Color::LightGreen)
    } else {
        ("○", Color::Red)
    };
    badges.push(Span::styled(status_icon, Style::default().fg(status_color)));

    // Split header area to show title on left and badges on right
    let badges_width = badges.iter().map(|s| s.content.len()).sum::<usize>() + 2;
    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(20),                        // Thread title (flexible)
            Constraint::Length(badges_width as u16),   // Badges (dynamic)
        ])
        .split(area);

    // Thread title and description (left side)
    let mut title_lines = vec![
        Line::from(vec![
            Span::styled("  Thread: ", Style::default().fg(COLOR_DIM)),
            Span::styled(
                thread_title,
                Style::default()
                    .fg(COLOR_HEADER)
                    .add_modifier(Modifier::BOLD),
            ),
        ])
    ];

    // Add description line if present and not empty
    if let Some(description) = thread_description {
        if !description.is_empty() {
            title_lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(description, Style::default().fg(COLOR_DIM)),
            ]));
        }
    }

    let title_widget = Paragraph::new(title_lines).block(header_block);
    frame.render_widget(title_widget, header_chunks[0]);

    // Badges (right side)
    let badges_widget = Paragraph::new(Line::from(badges))
        .alignment(ratatui::layout::Alignment::Right);
    frame.render_widget(badges_widget, header_chunks[1]);
}
