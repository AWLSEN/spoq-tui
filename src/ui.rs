//! UI rendering for SPOQ Command Deck
//!
//! Implements the full cyberpunk-styled terminal interface with:
//! - Header with ASCII logo and migration progress
//! - Left panel: Notifications + Saved/Active task columns
//! - Right panel: Thread cards
//! - Bottom: Input box and keybind hints

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, Focus, ProgrammingMode, Screen};
use crate::markdown::render_markdown;
use crate::models::{MessageSegment, ToolEvent, ToolEventStatus};
use crate::state::{Notification, TaskStatus};
use crate::widgets::input_box::InputBoxWidget;

// ============================================================================
// Minimal Dark Color Theme
// ============================================================================

/// Primary border color - dark gray for minimal aesthetic
pub const COLOR_BORDER: Color = Color::DarkGray;

/// Accent color - white for highlights and important elements
pub const COLOR_ACCENT: Color = Color::White;

/// Header text color - white for the logo
pub const COLOR_HEADER: Color = Color::White;

/// Active/running elements - bright green
pub const COLOR_ACTIVE: Color = Color::LightGreen;

/// Queued/pending elements - gray
pub const COLOR_QUEUED: Color = Color::Gray;

/// Dim text for less important info
pub const COLOR_DIM: Color = Color::DarkGray;

/// Background for input areas (used in later phases)
#[allow(dead_code)]
pub const COLOR_INPUT_BG: Color = Color::Rgb(20, 20, 30);

/// Progress bar fill color - white
pub const COLOR_PROGRESS: Color = Color::White;

// ============================================================================
// Helper Functions
// ============================================================================

/// Format token count in a human-readable way (e.g., 45000 -> "45k")
fn format_tokens(tokens: u32) -> String {
    if tokens >= 1_000_000 {
        format!("{}M", tokens / 1_000_000)
    } else if tokens >= 1_000 {
        format!("{}k", tokens / 1_000)
    } else {
        format!("{}", tokens)
    }
}

/// Progress bar background (used in later phases)
#[allow(dead_code)]
pub const COLOR_PROGRESS_BG: Color = Color::DarkGray;

// ============================================================================
// Claude Code Tool Colors
// ============================================================================

/// Tool icon color - Claude Code blue
const COLOR_TOOL_ICON: Color = Color::Rgb(0, 122, 204); // blue #007ACC

/// Tool running state - gray
const COLOR_TOOL_RUNNING: Color = Color::Rgb(128, 128, 128); // gray for running state

/// Tool success state - Claude Code green
const COLOR_TOOL_SUCCESS: Color = Color::Rgb(4, 181, 117); // green #04B575

/// Tool error state - red
const COLOR_TOOL_ERROR: Color = Color::Red;

/// Tool result preview - dim gray
const COLOR_TOOL_RESULT: Color = Color::Rgb(100, 100, 100); // dim gray for preview

// ============================================================================
// Helper Functions
// ============================================================================

/// Extract a short model name from the full model string
/// Examples:
/// - "claude-opus-4-5-20250514" → "opus"
/// - "claude-sonnet-3-5" → "sonnet"
/// - "gpt-4" → "gpt"
fn extract_short_model_name(full_name: &str) -> &str {
    if full_name.contains("opus") {
        "opus"
    } else if full_name.contains("sonnet") {
        "sonnet"
    } else {
        full_name.split('-').next().unwrap_or(full_name)
    }
}

// ============================================================================
// SPOQ ASCII Logo
// ============================================================================

const SPOQ_LOGO: &[&str] = &[
    "███████╗██████╗  ██████╗  ██████╗ ",
    "██╔════╝██╔══██╗██╔═══██╗██╔═══██╗",
    "███████╗██████╔╝██║   ██║██║   ██║",
    "╚════██║██╔═══╝ ██║   ██║██║▄▄ ██║",
    "███████║██║     ╚██████╔╝╚██████╔╝",
    "╚══════╝╚═╝      ╚═════╝  ╚══▀▀═╝ ",
];

// ============================================================================
// Main UI Rendering
// ============================================================================

/// Render the UI based on current screen
pub fn render(frame: &mut Frame, app: &App) {
    match app.screen {
        Screen::CommandDeck => render_command_deck(frame, app),
        Screen::Conversation => render_conversation_screen(frame, app),
    }
}

/// Render the complete Command Deck UI
fn render_command_deck(frame: &mut Frame, app: &App) {
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
            Constraint::Length(7),  // Input area
        ])
        .split(inner);

    render_header(frame, main_chunks[0], app);
    render_main_content(frame, main_chunks[1], app);
    render_input_area(frame, main_chunks[2], app);
}

/// Get inner rect with margin
fn inner_rect(area: Rect, margin: u16) -> Rect {
    Rect {
        x: area.x + margin,
        y: area.y + margin,
        width: area.width.saturating_sub(margin * 2),
        height: area.height.saturating_sub(margin * 2),
    }
}

// ============================================================================
// Header Section
// ============================================================================

fn render_header(frame: &mut Frame, area: Rect, app: &App) {
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

fn render_logo(frame: &mut Frame, area: Rect) {
    let logo_lines: Vec<Line> = SPOQ_LOGO
        .iter()
        .map(|line| Line::from(Span::styled(*line, Style::default().fg(COLOR_HEADER))))
        .collect();

    let logo = Paragraph::new(logo_lines);
    frame.render_widget(logo, area);
}

fn render_header_info(frame: &mut Frame, area: Rect, app: &App) {
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

fn render_main_content(frame: &mut Frame, area: Rect, app: &App) {
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

// ============================================================================
// Left Panel: Notifications + Tasks
// ============================================================================

fn render_left_panel(frame: &mut Frame, area: Rect, app: &App) {
    let left_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(COLOR_BORDER));
    frame.render_widget(left_block.clone(), area);

    let inner = inner_rect(area, 1);
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(50), // Notifications
            Constraint::Percentage(50), // Tasks
        ])
        .split(inner);

    render_notifications(frame, left_chunks[0], app, app.focus == Focus::Notifications);
    render_tasks(frame, left_chunks[1], app, app.focus == Focus::Tasks);
}

fn render_notifications(frame: &mut Frame, area: Rect, app: &App, focused: bool) {
    // Header styling changes based on focus
    let header_style = if focused {
        Style::default().fg(COLOR_HEADER).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(COLOR_ACCENT).add_modifier(Modifier::BOLD)
    };

    let mut lines = vec![
        Line::from(Span::styled(
            if focused { "◈ NOTIFICATIONS ◄" } else { "◈ NOTIFICATIONS" },
            header_style,
        )),
        Line::from(Span::styled(
            "─────────────────────────────",
            Style::default().fg(if focused { COLOR_ACCENT } else { COLOR_DIM }),
        )),
    ];

    // Mock notifications for static render
    let mock_notifications = [
        Notification {
            timestamp: chrono::Utc::now(),
            message: "Agent completed task".to_string(),
        },
        Notification {
            timestamp: chrono::Utc::now() - chrono::Duration::minutes(1),
            message: "New message received".to_string(),
        },
        Notification {
            timestamp: chrono::Utc::now() - chrono::Duration::minutes(4),
            message: "Process spawned".to_string(),
        },
        Notification {
            timestamp: chrono::Utc::now() - chrono::Duration::minutes(6),
            message: "File saved".to_string(),
        },
    ];

    for (i, notif) in mock_notifications.iter().take(area.height.saturating_sub(3) as usize).enumerate() {
        let time = notif.timestamp.format("%H:%M").to_string();
        let is_selected = focused && i == app.notifications_index;
        let marker = if is_selected { "▶ " } else { "▸ " };
        let marker_style = if is_selected {
            Style::default().fg(COLOR_HEADER)
        } else {
            Style::default().fg(COLOR_ACCENT)
        };

        lines.push(Line::from(vec![
            Span::styled(marker, marker_style),
            Span::styled(format!("[{}] ", time), Style::default().fg(COLOR_DIM)),
            Span::styled(
                &notif.message,
                if is_selected {
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                },
            ),
        ]));
    }

    let notifications = Paragraph::new(lines);
    frame.render_widget(notifications, area);
}

fn render_tasks(frame: &mut Frame, area: Rect, app: &App, focused: bool) {
    use crate::state::TodoStatus;

    let border_color = if focused { COLOR_ACCENT } else { COLOR_DIM };
    let task_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(border_color));
    frame.render_widget(task_block.clone(), area);

    let inner = inner_rect(area, 1);

    let header_style = if focused {
        Style::default().fg(COLOR_HEADER).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(COLOR_ACCENT).add_modifier(Modifier::BOLD)
    };

    let mut lines = vec![
        Line::from(Span::styled(
            if focused { "◈ TODOS ◄" } else { "◈ TODOS" },
            header_style,
        )),
        Line::from(Span::styled(
            "─────────────────────────────",
            Style::default().fg(if focused { COLOR_ACCENT } else { COLOR_DIM }),
        )),
    ];

    // Render todos from app state
    if app.todos.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No todos yet",
            Style::default().fg(COLOR_DIM),
        )));
    } else {
        for todo in &app.todos {
            let (icon, color, text) = match todo.status {
                TodoStatus::Pending => ("[ ] ", COLOR_DIM, &todo.content),
                TodoStatus::InProgress => ("[◐] ", Color::Cyan, &todo.active_form),
                TodoStatus::Completed => ("[✓] ", Color::Green, &todo.content),
            };

            lines.push(Line::from(vec![
                Span::styled(icon, Style::default().fg(color)),
                Span::styled(
                    text,
                    if todo.status == TodoStatus::Pending {
                        Style::default().fg(COLOR_DIM)
                    } else {
                        Style::default().fg(color)
                    },
                ),
            ]));
        }
    }

    let todos_widget = Paragraph::new(lines);
    frame.render_widget(todos_widget, inner);
}


// ============================================================================
// Right Panel: Threads
// ============================================================================

fn render_right_panel(frame: &mut Frame, area: Rect, app: &App, focused: bool) {
    let border_color = if focused { COLOR_HEADER } else { COLOR_BORDER };
    let right_block = Block::default()
        .borders(Borders::ALL)
        .border_type(if focused { BorderType::Thick } else { BorderType::Plain })
        .border_style(Style::default().fg(border_color));
    frame.render_widget(right_block.clone(), area);

    let inner = inner_rect(area, 1);

    // Calculate centering padding for thread cards
    // Card width is 39 chars (including borders)
    let card_width: u16 = 39;
    let panel_width = inner.width;
    let left_padding = if panel_width > card_width {
        (panel_width - card_width) / 2
    } else {
        0
    };
    let padding_str: String = " ".repeat(left_padding as usize);

    let header_style = if focused {
        Style::default().fg(COLOR_HEADER).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(COLOR_ACCENT).add_modifier(Modifier::BOLD)
    };

    let mut lines = vec![
        Line::from(Span::styled(
            if focused { "◈ THREADS ◄" } else { "◈ THREADS" },
            header_style,
        )),
        Line::from(""),
    ];

    // Use threads from cache (no mock fallback - show empty if no threads)
    let cached_threads = app.cache.threads();
    let threads_to_render: Vec<(String, String)> = cached_threads.iter().map(|t| {
        (t.title.clone(), t.preview.clone())
    }).collect();

    // Show empty state if no threads
    if threads_to_render.is_empty() {
        lines.push(Line::from(vec![
            Span::raw(padding_str.clone()),
            Span::styled(
                "No conversations yet",
                Style::default().fg(COLOR_DIM),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::raw(padding_str.clone()),
            Span::styled(
                "Type a message to start",
                Style::default().fg(COLOR_DIM),
            ),
        ]));
    }

    for (i, (title, preview)) in threads_to_render.iter().enumerate() {
        let is_selected = focused && i == app.threads_index;
        let card_border_color = if is_selected { COLOR_HEADER } else { COLOR_BORDER };

        // Thread card top border (centered)
        lines.push(Line::from(vec![
            Span::raw(padding_str.clone()),
            Span::styled(
                "┌─────────────────────────────────────┐",
                Style::default().fg(card_border_color),
            ),
        ]));

        // Thread title (centered)
        let title_marker = if is_selected { "▶ " } else { "► " };

        // Check if this thread is streaming and compute dots
        let thread_id = &cached_threads[i].id;
        let is_streaming = app.cache.is_thread_streaming(thread_id);
        let dots = if is_streaming {
            match (app.tick_count / 5) % 3 {
                0 => ".",
                1 => "..",
                _ => "...",
            }
        } else {
            ""
        };

        // Calculate available width for title to ensure dots fit
        // Card width is 37 inner chars, minus "Thread: " (8) and marker (2) = 27 chars available
        // Reserve 3 chars for dots if streaming
        let max_title_len = if is_streaming { 24 } else { 27 };
        let display_title = if title.len() > max_title_len {
            format!("{}...", &title[..max_title_len.saturating_sub(3)])
        } else {
            title.clone()
        };

        let mut title_spans = vec![
            Span::raw(padding_str.clone()),
            Span::styled("│ ", Style::default().fg(card_border_color)),
            Span::styled(title_marker, Style::default().fg(if is_selected { COLOR_HEADER } else { COLOR_ACCENT })),
            Span::styled(
                format!("Thread: {}", display_title),
                Style::default()
                    .fg(if is_selected { Color::White } else { COLOR_HEADER })
                    .add_modifier(Modifier::BOLD),
            ),
        ];

        if is_streaming {
            title_spans.push(Span::styled(
                dots,
                Style::default().fg(COLOR_ACTIVE),
            ));
        }

        title_spans.push(Span::styled(
            format!("{:>width$}│", "", width = 35_usize.saturating_sub(10 + display_title.len() + dots.len())),
            Style::default().fg(card_border_color),
        ));

        lines.push(Line::from(title_spans));

        // Thread description (centered, if present)
        let thread = &cached_threads[i];
        if let Some(description) = &thread.description {
            if !description.is_empty() {
                // Max description length is 35 chars (card inner width minus borders and padding)
                let max_desc_len = 35;
                let display_desc = if description.len() > max_desc_len {
                    format!("{}...", &description[..max_desc_len.saturating_sub(3)])
                } else {
                    description.clone()
                };

                lines.push(Line::from(vec![
                    Span::raw(padding_str.clone()),
                    Span::styled("│   ", Style::default().fg(card_border_color)),
                    Span::styled(display_desc.clone(), Style::default().fg(COLOR_DIM)),
                    Span::styled(
                        format!("{:>width$}│", "", width = 35_usize.saturating_sub(4 + display_desc.len())),
                        Style::default().fg(card_border_color),
                    ),
                ]));
            }
        }

        // Thread type indicator and model info (centered)
        let type_indicator = match thread.thread_type {
            crate::models::ThreadType::Normal => "[N]",
            crate::models::ThreadType::Programming => "[P]",
        };

        let mut type_line_spans = vec![
            Span::raw(padding_str.clone()),
            Span::styled("│   ", Style::default().fg(card_border_color)),
            Span::styled(type_indicator, Style::default().fg(COLOR_ACCENT)),
        ];

        // Add model name if present
        if let Some(model) = &thread.model {
            let short_model = extract_short_model_name(model);
            type_line_spans.push(Span::styled(
                format!(" {}", short_model),
                Style::default().fg(COLOR_DIM),
            ));
            let type_info_len = type_indicator.len() + 1 + short_model.len();
            type_line_spans.push(Span::styled(
                format!("{:>width$}│", "", width = 35_usize.saturating_sub(4 + type_info_len)),
                Style::default().fg(card_border_color),
            ));
        } else {
            type_line_spans.push(Span::styled(
                format!("{:>width$}│", "", width = 35_usize.saturating_sub(4 + type_indicator.len())),
                Style::default().fg(card_border_color),
            ));
        }

        lines.push(Line::from(type_line_spans));

        // Thread preview (centered)
        lines.push(Line::from(vec![
            Span::raw(padding_str.clone()),
            Span::styled("│   ", Style::default().fg(card_border_color)),
            Span::styled(format!("\"{}\"", preview), Style::default().fg(COLOR_DIM)),
            Span::styled(
                format!("{:>width$}│", "", width = 35_usize.saturating_sub(4 + preview.len())),
                Style::default().fg(card_border_color),
            ),
        ]));

        // Thread card bottom border (centered)
        lines.push(Line::from(vec![
            Span::raw(padding_str.clone()),
            Span::styled(
                "└─────────────────────────────────────┘",
                Style::default().fg(card_border_color),
            ),
        ]));
        lines.push(Line::from(""));
    }

    // Keybind hints at bottom of threads panel (centered)
    lines.push(Line::from(vec![
        Span::raw(padding_str),
        Span::styled("[Shift+N]", Style::default().fg(COLOR_ACCENT)),
        Span::raw(" New Thread  "),
        Span::styled("[TAB]", Style::default().fg(COLOR_ACCENT)),
        Span::raw(" Switch Panel"),
    ]));

    let threads = Paragraph::new(lines);
    frame.render_widget(threads, inner);
}

// ============================================================================
// Input Area
// ============================================================================

fn render_input_area(frame: &mut Frame, area: Rect, app: &App) {
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
fn create_mode_indicator_line(mode: ProgrammingMode) -> Option<Line<'static>> {
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
fn render_conversation_screen(frame: &mut Frame, app: &App) {
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
fn render_mode_indicator(frame: &mut Frame, area: Rect, mode_line: Line<'static>) {
    let indicator = Paragraph::new(mode_line);
    frame.render_widget(indicator, area);
}

/// Render the streaming indicator bar
fn render_streaming_indicator(frame: &mut Frame, area: Rect, app: &App) {
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
fn render_conversation_header(frame: &mut Frame, area: Rect, app: &App) {
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

    // Context badge [ctx: 45K/100K] or [ctx: -/-]
    let ctx_badge = match (app.session_state.context_tokens_used, app.session_state.context_token_limit) {
        (Some(used), Some(limit)) => {
            // Format as K if over 1000
            let used_str = if used >= 1000 { format!("{}K", used / 1000) } else { format!("{}", used) };
            let limit_str = if limit >= 1000 { format!("{}K", limit / 1000) } else { format!("{}", limit) };
            format!("[ctx: {}/{}] ", used_str, limit_str)
        }
        _ => "[ctx: -/-] ".to_string(),
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

/// Maximum number of inline error banners to display
const MAX_VISIBLE_ERRORS: usize = 2;

/// Render inline error banners for a thread
/// Returns the lines to be added to the messages area
fn render_inline_error_banners(app: &App) -> Vec<Line<'static>> {
    let mut lines: Vec<Line> = Vec::new();

    // Get errors for the active thread
    let errors = app
        .active_thread_id
        .as_ref()
        .and_then(|id| app.cache.get_errors(id));

    let Some(errors) = errors else {
        return lines;
    };

    if errors.is_empty() {
        return lines;
    }

    let focused_index = app.cache.focused_error_index();
    let total_errors = errors.len();

    // Only show up to MAX_VISIBLE_ERRORS
    for (i, error) in errors.iter().take(MAX_VISIBLE_ERRORS).enumerate() {
        let is_focused = i == focused_index;
        let border_color = if is_focused { Color::Red } else { Color::DarkGray };
        let border_char_top = if is_focused { "═" } else { "─" };
        let border_char_bottom = if is_focused { "═" } else { "─" };

        // Top border with error code
        let header = format!("─[!] {} ", error.error_code);
        let remaining_width = 50_usize.saturating_sub(header.len());
        let top_border = format!(
            "┌{}{}┐",
            header,
            border_char_top.repeat(remaining_width)
        );
        lines.push(Line::from(Span::styled(
            top_border,
            Style::default().fg(border_color),
        )));

        // Error message line
        let msg_display = if error.message.len() > 46 {
            format!("{}...", &error.message[..43])
        } else {
            error.message.clone()
        };
        let msg_padding = 48_usize.saturating_sub(msg_display.len());
        lines.push(Line::from(vec![
            Span::styled("│ ", Style::default().fg(border_color)),
            Span::styled(msg_display, Style::default().fg(Color::White)),
            Span::styled(
                format!("{:>width$}│", "", width = msg_padding),
                Style::default().fg(border_color),
            ),
        ]));

        // Dismiss hint line
        let dismiss_text = "[d]ismiss";
        let dismiss_padding = 48_usize.saturating_sub(dismiss_text.len());
        lines.push(Line::from(vec![
            Span::styled("│ ", Style::default().fg(border_color)),
            Span::styled(
                format!("{:>width$}", "", width = dismiss_padding),
                Style::default().fg(border_color),
            ),
            Span::styled(
                dismiss_text,
                Style::default().fg(COLOR_DIM),
            ),
            Span::styled(" │", Style::default().fg(border_color)),
        ]));

        // Bottom border
        let bottom_border = format!(
            "└{}┘",
            border_char_bottom.repeat(48)
        );
        lines.push(Line::from(Span::styled(
            bottom_border,
            Style::default().fg(border_color),
        )));

        lines.push(Line::from(""));
    }

    // Show "+N more" if there are more errors
    if total_errors > MAX_VISIBLE_ERRORS {
        let more_count = total_errors - MAX_VISIBLE_ERRORS;
        lines.push(Line::from(vec![
            Span::styled(
                format!("  +{} more error{}", more_count, if more_count > 1 { "s" } else { "" }),
                Style::default().fg(Color::Red).add_modifier(Modifier::ITALIC),
            ),
        ]));
        lines.push(Line::from(""));
    }

    lines
}

// ============================================================================
// Thinking/Reasoning Block
// ============================================================================

/// Render a collapsible thinking block for assistant messages.
///
/// Collapsed: ▸ Thinking... (847 tokens)
/// Expanded:
/// ▾ Thinking
/// │ Let me analyze this step by step...
/// │ First, I need to understand the structure.
/// └──────────────────────────────────────────
fn render_thinking_block(
    message: &crate::models::Message,
    tick_count: u64,
) -> Vec<Line<'static>> {
    let mut lines: Vec<Line> = Vec::new();

    // Only render for assistant messages with reasoning content
    if message.role != crate::models::MessageRole::Assistant {
        return lines;
    }

    if message.reasoning_content.is_empty() {
        return lines;
    }

    let token_count = message.reasoning_token_count();
    let collapsed = message.reasoning_collapsed;

    // Determine the arrow and style based on collapsed state
    let (arrow, header_color) = if collapsed {
        ("▸", Color::Magenta)
    } else {
        ("▾", Color::Magenta)
    };

    // Header line
    if collapsed {
        // Collapsed: ▸ Thinking... (847 tokens)
        lines.push(Line::from(vec![
            Span::styled(
                format!("{} ", arrow),
                Style::default().fg(header_color),
            ),
            Span::styled(
                "Thinking...",
                Style::default().fg(Color::Magenta).add_modifier(Modifier::ITALIC),
            ),
            Span::styled(
                format!(" ({} tokens)", token_count),
                Style::default().fg(COLOR_DIM),
            ),
            Span::styled(
                "  [t] toggle",
                Style::default().fg(COLOR_DIM),
            ),
        ]));
    } else {
        // Expanded header: ▾ Thinking
        lines.push(Line::from(vec![
            Span::styled(
                format!("{} ", arrow),
                Style::default().fg(header_color),
            ),
            Span::styled(
                "Thinking",
                Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" ({} tokens)", token_count),
                Style::default().fg(COLOR_DIM),
            ),
            Span::styled(
                "  [t] toggle",
                Style::default().fg(COLOR_DIM),
            ),
        ]));

        // Render the reasoning content with box-drawing border
        let content = &message.reasoning_content;
        for line in content.lines() {
            lines.push(Line::from(vec![
                Span::styled(
                    "│ ",
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    line.to_string(),
                    Style::default().fg(Color::Gray).add_modifier(Modifier::ITALIC),
                ),
            ]));
        }

        // If streaming, add a blinking cursor at the end
        if message.is_streaming {
            let show_cursor = (tick_count / 5) % 2 == 0;
            if show_cursor {
                lines.push(Line::from(vec![
                    Span::styled(
                        "│ █",
                        Style::default().fg(Color::Magenta),
                    ),
                ]));
            }
        }

        // Bottom border
        lines.push(Line::from(vec![
            Span::styled(
                "└──────────────────────────────────────────",
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    }

    lines.push(Line::from("")); // Add spacing after thinking block

    lines
}

/// Spinner frames for tool status animation
const SPINNER_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Render tool status indicators inline (LEGACY - kept for potential future use)
/// Shows: ◐ Reading src/main.rs...  (executing, with spinner)
///        ✓ Read complete           (success, fades after 30 ticks)
///        ✗ Write failed: error     (failure, persists)
/// Note: Tool events are now rendered inline with messages via render_tool_event()
#[allow(dead_code)]
fn render_tool_status_lines(app: &App) -> Vec<Line<'static>> {
    use crate::state::ToolDisplayStatus;

    let mut lines: Vec<Line> = Vec::new();

    // Get tools that should be rendered at current tick
    let tools = app.tool_tracker.tools_to_render(app.tick_count);

    if tools.is_empty() {
        return lines;
    }

    for (_tool_id, state) in tools {
        let Some(ref display_status) = state.display_status else {
            continue;
        };

        let line = match display_status {
            ToolDisplayStatus::Started { .. } | ToolDisplayStatus::Executing { .. } => {
                // Animate spinner based on tick count
                let spinner_idx = (app.tick_count % 10) as usize;
                let spinner = SPINNER_FRAMES[spinner_idx];
                let text = display_status.display_text();

                Line::from(vec![
                    Span::styled(
                        format!("  {} ", spinner),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(
                        text,
                        Style::default().fg(Color::DarkGray),
                    ),
                ])
            }
            ToolDisplayStatus::Completed { success, summary, .. } => {
                if *success {
                    Line::from(vec![
                        Span::styled(
                            "  ✓ ",
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::styled(
                            summary.clone(),
                            Style::default().fg(Color::DarkGray),
                        ),
                    ])
                } else {
                    Line::from(vec![
                        Span::styled(
                            "  ✗ ",
                            Style::default().fg(Color::Red),
                        ),
                        Span::styled(
                            summary.clone(),
                            Style::default().fg(Color::Red),
                        ),
                    ])
                }
            }
        };

        lines.push(line);
    }

    if !lines.is_empty() {
        lines.push(Line::from("")); // Add spacing after tool status
    }

    lines
}

/// Render subagent status with spinner and progress (LEGACY - kept for potential future use)
/// UI design:
/// ```text
/// ┌ ◐ Exploring codebase structure
/// │   Found 5 relevant files...
/// └ ✓ Complete (8 tool calls)
/// ```
/// Note: Subagent status may be integrated inline in future iterations
#[allow(dead_code)]
fn render_subagent_status_lines(app: &App) -> Vec<Line<'static>> {
    use crate::state::SubagentDisplayStatus;

    let mut lines: Vec<Line> = Vec::new();

    // Get subagents that should be rendered at current tick
    let subagents = app.subagent_tracker.subagents_to_render(app.tick_count);

    if subagents.is_empty() {
        return lines;
    }

    for (_subagent_id, state) in subagents {
        // Render main line with appropriate prefix and spinner/checkmark
        let main_line = match &state.display_status {
            SubagentDisplayStatus::Started { description, .. } |
            SubagentDisplayStatus::Progress { description, .. } => {
                // Animate spinner based on tick count
                let spinner_idx = (app.tick_count % 10) as usize;
                let spinner = SPINNER_FRAMES[spinner_idx];

                Line::from(vec![
                    Span::styled(
                        format!("┌ {} ", spinner),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(
                        description.clone(),
                        Style::default().fg(Color::DarkGray),
                    ),
                ])
            }
            SubagentDisplayStatus::Completed { success, summary, .. } => {
                let (prefix, color) = if *success {
                    ("└ ✓ ", Color::DarkGray)
                } else {
                    ("└ ✗ ", Color::Red)
                };

                Line::from(vec![
                    Span::styled(prefix, Style::default().fg(color)),
                    Span::styled(summary.clone(), Style::default().fg(color)),
                ])
            }
        };
        lines.push(main_line);

        // Render progress line if we have a progress message (only for in-progress subagents)
        if let SubagentDisplayStatus::Progress { progress_message, .. } = &state.display_status {
            if !progress_message.is_empty() {
                lines.push(Line::from(vec![
                    Span::styled(
                        "│   ",
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(
                        progress_message.clone(),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
            }
        }
    }

    if !lines.is_empty() {
        lines.push(Line::from("")); // Add spacing after subagent status
    }

    lines
}

/// Render a single tool event as a Line
fn render_tool_event(event: &ToolEvent, tick_count: u64) -> Line<'static> {
    // Use display_name if available, otherwise fall back to function_name
    let tool_name = event.display_name.as_ref()
        .unwrap_or(&event.function_name)
        .clone();

    match event.status {
        ToolEventStatus::Running => {
            // Animated spinner - cycle through frames ~100ms per frame (assuming 10 ticks/sec)
            let frame_index = (tick_count % 10) as usize;
            let spinner = SPINNER_FRAMES[frame_index];
            Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    format!("{} {}", spinner, tool_name),
                    Style::default().fg(Color::DarkGray),
                ),
            ])
        }
        ToolEventStatus::Complete => {
            let duration_str = event.duration_secs
                .map(|d| format!(" ({:.1}s)", d))
                .unwrap_or_default();
            Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    "✓ ",
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("{}{}", tool_name, duration_str),
                    Style::default().fg(Color::DarkGray),
                ),
            ])
        }
        ToolEventStatus::Failed => {
            Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    "✗ ",
                    Style::default().fg(Color::Red),
                ),
                Span::styled(
                    format!("{} failed", tool_name),
                    Style::default().fg(Color::Red),
                ),
            ])
        }
    }
}

/// Render the messages area with user messages and AI responses
fn render_messages_area(frame: &mut Frame, area: Rect, app: &App) {
    use crate::models::MessageRole;

    let inner = inner_rect(area, 1);
    let mut lines: Vec<Line> = Vec::new();

    // Show inline error banners for the thread
    lines.extend(render_inline_error_banners(app));

    // Note: Tool status is now rendered inline with messages via render_tool_event()
    // The legacy render_tool_status_lines and render_subagent_status_lines functions are kept
    // for potential future use but removed from the main render flow.

    // Show stream error banner if there's a stream error (legacy, for non-thread errors)
    if let Some(error) = &app.stream_error {
        lines.push(Line::from(vec![
            Span::styled(
                "  ⚠ ERROR: ",
                Style::default()
                    .fg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                error.as_str(),
                Style::default().fg(Color::Red),
            ),
        ]));
        lines.push(Line::from(vec![Span::styled(
            "═══════════════════════════════════════════════",
            Style::default().fg(Color::Red),
        )]));
    }

    // Get messages from cache if we have an active thread
    let cached_messages = app
        .active_thread_id
        .as_ref()
        .and_then(|id| app.cache.get_messages(id));

    if let Some(messages) = cached_messages {
        for message in messages {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![Span::styled(
                "───────────────────────────────────────────────",
                Style::default().fg(COLOR_DIM),
            )]));

            // Render thinking/reasoning block for assistant messages (before content)
            if message.role == MessageRole::Assistant {
                lines.extend(render_thinking_block(message, app.tick_count));
            }

            let (label, label_style) = match message.role {
                MessageRole::User => (
                    "You: ",
                    Style::default()
                        .fg(COLOR_ACTIVE)
                        .add_modifier(Modifier::BOLD),
                ),
                MessageRole::Assistant => (
                    "AI: ",
                    Style::default()
                        .fg(COLOR_ACCENT)
                        .add_modifier(Modifier::BOLD),
                ),
                MessageRole::System => (
                    "System: ",
                    Style::default().fg(COLOR_DIM).add_modifier(Modifier::BOLD),
                ),
            };

            // Handle streaming vs completed messages
            if message.is_streaming {
                // Display streaming content with blinking cursor
                // Blink cursor every ~500ms (assuming 10 ticks/sec, toggle every 5 ticks)
                let show_cursor = (app.tick_count / 5) % 2 == 0;
                let cursor_span = Span::styled(
                    if show_cursor { "█" } else { " " },
                    Style::default().fg(COLOR_ACCENT),
                );

                // For assistant messages with segments, render segments in order (interleaved)
                // This shows text and tool events in the order they occurred
                if message.role == MessageRole::Assistant && !message.segments.is_empty() {
                    let mut is_first_line = true;

                    for segment in &message.segments {
                        match segment {
                            MessageSegment::Text(text) => {
                                let mut segment_lines = render_markdown(text);
                                if is_first_line && !segment_lines.is_empty() {
                                    // Prepend label to first line of first text segment
                                    let first_line = segment_lines.remove(0);
                                    let mut first_spans = vec![Span::styled(label, label_style)];
                                    first_spans.extend(first_line.spans);
                                    lines.push(Line::from(first_spans));
                                    is_first_line = false;
                                }
                                lines.extend(segment_lines);
                            }
                            MessageSegment::ToolEvent(event) => {
                                if is_first_line {
                                    // No text before first tool event, show label first
                                    lines.push(Line::from(vec![Span::styled(label, label_style)]));
                                    is_first_line = false;
                                }
                                lines.push(render_tool_event(event, app.tick_count));
                            }
                        }
                    }

                    // If we never added any content, show label with cursor
                    if is_first_line {
                        lines.push(Line::from(vec![
                            Span::styled(label, label_style),
                            cursor_span,
                        ]));
                    } else {
                        // Append cursor to last line
                        if let Some(last_pushed) = lines.last_mut() {
                            last_pushed.spans.push(cursor_span);
                        }
                    }
                } else {
                    // Fall back to partial_content for backward compatibility
                    // (non-assistant messages or when segments is empty)
                    let mut content_lines = render_markdown(&message.partial_content);

                    // Add label to first line, append cursor to last line
                    if content_lines.is_empty() {
                        // No content yet, just show label with cursor
                        lines.push(Line::from(vec![
                            Span::styled(label, label_style),
                            cursor_span,
                        ]));
                    } else {
                        // Prepend label to first line
                        let first_line = content_lines.remove(0);
                        let mut first_spans = vec![Span::styled(label, label_style)];
                        first_spans.extend(first_line.spans);
                        lines.push(Line::from(first_spans));

                        // Add middle lines as-is
                        for line in content_lines.drain(..content_lines.len().saturating_sub(1)) {
                            lines.push(line);
                        }

                        // Append cursor to last line (if there are remaining lines)
                        if let Some(last_line) = content_lines.pop() {
                            let mut last_spans = last_line.spans;
                            last_spans.push(cursor_span);
                            lines.push(Line::from(last_spans));
                        } else {
                            // Only had one line, cursor was not added yet
                            // The first line is already pushed, so add cursor separately
                            // Actually, we need to modify the last pushed line
                            if let Some(last_pushed) = lines.last_mut() {
                                last_pushed.spans.push(cursor_span);
                            }
                        }
                    }
                }
            } else {
                // Display completed message with interleaved text and tool events
                // For assistant messages with segments, render segments in order
                if message.role == MessageRole::Assistant && !message.segments.is_empty() {
                    let mut is_first_line = true;

                    for segment in &message.segments {
                        match segment {
                            MessageSegment::Text(text) => {
                                let mut segment_lines = render_markdown(text);
                                if is_first_line && !segment_lines.is_empty() {
                                    // Prepend label to first line of first text segment
                                    let first_line = segment_lines.remove(0);
                                    let mut first_spans = vec![Span::styled(label, label_style)];
                                    first_spans.extend(first_line.spans);
                                    lines.push(Line::from(first_spans));
                                    is_first_line = false;
                                }
                                lines.extend(segment_lines);
                            }
                            MessageSegment::ToolEvent(event) => {
                                if is_first_line {
                                    // No text before first tool event, show label first
                                    lines.push(Line::from(vec![Span::styled(label, label_style)]));
                                    is_first_line = false;
                                }
                                lines.push(render_tool_event(event, app.tick_count));
                            }
                        }
                    }

                    // If we never added any content, show just the label
                    if is_first_line {
                        lines.push(Line::from(vec![Span::styled(label, label_style)]));
                    }
                } else {
                    // Fall back to content field for non-assistant messages or empty segments
                    let content_lines = render_markdown(&message.content);

                    if content_lines.is_empty() {
                        // Empty content, just show label
                        lines.push(Line::from(vec![Span::styled(label, label_style)]));
                    } else {
                        // Prepend label to first line
                        let mut iter = content_lines.into_iter();
                        if let Some(first_line) = iter.next() {
                            let mut first_spans = vec![Span::styled(label, label_style)];
                            first_spans.extend(first_line.spans);
                            lines.push(Line::from(first_spans));
                        }

                        // Add remaining lines as-is
                        for line in iter {
                            lines.push(line);
                        }
                    }
                }
            }

            lines.push(Line::from(""));
        }
    } else {
        // No messages yet - show placeholder
        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            "───────────────────────────────────────────────",
            Style::default().fg(COLOR_DIM),
        )]));
        lines.push(Line::from(vec![
            Span::styled(
                "AI: ",
                Style::default()
                    .fg(COLOR_ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("Waiting for your message...", Style::default().fg(COLOR_DIM)),
        ]));
        lines.push(Line::from(""));
    }

    // Calculate content height for scroll bounds
    // With word wrap enabled, we need to estimate wrapped line count
    let viewport_height = inner.height as usize;
    let viewport_width = inner.width as usize;

    // Estimate total lines after wrapping
    let mut total_lines: usize = 0;
    for line in &lines {
        let line_width: usize = line.spans.iter().map(|s| s.content.len()).sum();
        if line_width == 0 {
            total_lines += 1; // Empty line
        } else {
            // Estimate wrapped lines (ceil division)
            total_lines += (line_width + viewport_width - 1) / viewport_width.max(1);
        }
    }

    // Calculate max scroll (how far up we can scroll from bottom)
    // scroll=0 means showing the bottom (latest content)
    // scroll=max means showing the top (oldest content)
    let max_scroll = total_lines.saturating_sub(viewport_height) as u16;

    // Clamp user's scroll to valid range
    let clamped_scroll = app.conversation_scroll.min(max_scroll);

    // Convert from "scroll from bottom" to ratatui's "scroll from top"
    // If user_scroll=0, show bottom → actual_scroll = max_scroll
    // If user_scroll=max, show top → actual_scroll = 0
    let actual_scroll = max_scroll.saturating_sub(clamped_scroll);

    let messages_widget = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((actual_scroll, 0));
    frame.render_widget(messages_widget, inner);

    // Render inline permission prompt if pending (overlays on top of messages)
    if app.session_state.has_pending_permission() {
        render_permission_prompt(frame, inner, app);
    }
}

/// Build contextual keybind hints based on application state
fn build_contextual_keybinds(app: &App) -> Line<'static> {
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
        spans.push(Span::styled("[Tab]", Style::default().fg(COLOR_ACCENT)));
        spans.push(Span::raw(" switch focus │ "));

        spans.push(Span::styled("[Enter]", Style::default().fg(COLOR_ACCENT)));
        spans.push(Span::raw(" send │ "));

        spans.push(Span::styled("[Esc]", Style::default().fg(COLOR_ACCENT)));
        spans.push(Span::raw(" back"));
    }

    Line::from(spans)
}

/// Render the input area for conversation screen
fn render_conversation_input(frame: &mut Frame, area: Rect, app: &App) {
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
fn render_permission_box(frame: &mut Frame, area: Rect, perm: &crate::state::session::PermissionRequest) {
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
fn get_permission_preview(perm: &crate::state::session::PermissionRequest) -> String {
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
            // Truncate long content
            if content.len() > 100 {
                return format!("{}...", &content[..100]);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::ProgrammingMode;
    use ratatui::{backend::TestBackend, Terminal};

    fn create_test_app() -> App {
        let (message_tx, message_rx) = tokio::sync::mpsc::unbounded_channel();
        App {
            threads: vec![],
            tasks: vec![],
            todos: vec![],
            should_quit: false,
            screen: Screen::CommandDeck,
            active_thread_id: None,
            focus: Focus::default(),
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
            programming_mode: ProgrammingMode::default(),
            session_state: crate::state::SessionState::new(),
            tool_tracker: crate::state::ToolTracker::new(),
            subagent_tracker: crate::state::SubagentTracker::new(),
            debug_tx: None,
            stream_start_time: None,
            last_event_time: None,
            cumulative_token_count: 0,
        }
    }

    #[test]
    fn test_screen_enum_default() {
        let screen = Screen::default();
        assert_eq!(screen, Screen::CommandDeck);
    }

    #[test]
    fn test_screen_enum_variants() {
        let command_deck = Screen::CommandDeck;
        let conversation = Screen::Conversation;
        assert_ne!(command_deck, conversation);
    }

    #[test]
    fn test_render_command_deck_screen() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = create_test_app();

        terminal
            .draw(|f| {
                render(f, &app);
            })
            .unwrap();

        // Check that the terminal rendered without panic
        let buffer = terminal.backend().buffer();
        // Verify the buffer contains some content (not all spaces)
        let has_content = buffer
            .content()
            .iter()
            .any(|cell| cell.symbol() != " ");
        assert!(has_content, "CommandDeck screen should render content");
    }

    #[test]
    fn test_render_conversation_screen() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;

        terminal
            .draw(|f| {
                render(f, &app);
            })
            .unwrap();

        // Check that the terminal rendered without panic
        let buffer = terminal.backend().buffer();
        // Verify the buffer contains some content
        let has_content = buffer
            .content()
            .iter()
            .any(|cell| cell.symbol() != " ");
        assert!(has_content, "Conversation screen should render content");
    }

    #[test]
    fn test_conversation_screen_shows_thread_title() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;
        // Add thread to cache instead of legacy threads vec
        app.cache.upsert_thread(crate::models::Thread {
            id: "test-thread".to_string(),
            title: "Test Thread".to_string(),
            description: None,
            preview: "Test preview".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::default(),
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: chrono::Utc::now(),
        });
        app.active_thread_id = Some("test-thread".to_string());

        terminal
            .draw(|f| {
                render(f, &app);
            })
            .unwrap();

        // Check that the buffer contains the thread title
        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(
            buffer_str.contains("Test Thread"),
            "Conversation screen should show thread title"
        );
    }

    #[test]
    fn test_conversation_screen_default_title() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;
        app.active_thread_id = None;

        terminal
            .draw(|f| {
                render(f, &app);
            })
            .unwrap();

        // Check that the buffer contains the default title
        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(
            buffer_str.contains("New Conversation"),
            "Conversation screen should show default title when no active thread"
        );
    }

    #[test]
    fn test_conversation_screen_renders_with_user_input() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;
        app.input_box.insert_char('H');
        app.input_box.insert_char('e');
        app.input_box.insert_char('l');
        app.input_box.insert_char('l');
        app.input_box.insert_char('o');

        terminal
            .draw(|f| {
                render(f, &app);
            })
            .unwrap();

        // Check that the screen renders without panic when there's input
        let buffer = terminal.backend().buffer();
        let has_content = buffer
            .content()
            .iter()
            .any(|cell| cell.symbol() != " ");
        assert!(has_content, "Conversation screen should render content with user input");
    }

    #[test]
    fn test_conversation_screen_shows_ai_stub() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;

        terminal
            .draw(|f| {
                render(f, &app);
            })
            .unwrap();

        // Check that the buffer shows AI stub response
        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(
            buffer_str.contains("AI:"),
            "Conversation screen should show AI label"
        );
        assert!(
            buffer_str.contains("Waiting for your message"),
            "Conversation screen should show AI stub response"
        );
    }

    #[test]
    fn test_command_deck_shows_disconnected_status() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.connection_status = false;

        terminal
            .draw(|f| {
                render(f, &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(
            buffer_str.contains("Disconnected"),
            "CommandDeck should show Disconnected status when connection_status is false"
        );
        assert!(
            buffer_str.contains("○"),
            "CommandDeck should show empty circle icon when disconnected"
        );
    }

    #[test]
    fn test_command_deck_shows_connected_status() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.connection_status = true;

        terminal
            .draw(|f| {
                render(f, &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(
            buffer_str.contains("Connected"),
            "CommandDeck should show Connected status when connection_status is true"
        );
        assert!(
            buffer_str.contains("●"),
            "CommandDeck should show filled circle icon when connected"
        );
    }

    #[test]
    fn test_conversation_screen_shows_disconnected_status() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;
        app.connection_status = false;

        terminal
            .draw(|f| {
                render(f, &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(
            buffer_str.contains("○"),
            "Conversation screen should show disconnected status icon (○)"
        );
    }

    #[test]
    fn test_conversation_screen_shows_connected_status() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;
        app.connection_status = true;

        terminal
            .draw(|f| {
                render(f, &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(
            buffer_str.contains("●"),
            "Conversation screen should show connected status icon (●)"
        );
    }

    #[test]
    fn test_conversation_screen_shows_error_banner() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;
        app.stream_error = Some("Connection timed out".to_string());

        terminal
            .draw(|f| {
                render(f, &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(
            buffer_str.contains("ERROR"),
            "Conversation screen should show ERROR label when stream_error is set"
        );
        assert!(
            buffer_str.contains("Connection timed out"),
            "Conversation screen should show the error message"
        );
    }

    #[test]
    fn test_conversation_screen_no_error_banner_when_no_error() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;
        app.stream_error = None;

        terminal
            .draw(|f| {
                render(f, &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(
            !buffer_str.contains("ERROR"),
            "Conversation screen should not show ERROR label when stream_error is None"
        );
    }

    #[test]
    fn test_conversation_screen_shows_streaming_indicator() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;

        // Create a streaming thread with a streaming message
        let thread_id = app.cache.create_streaming_thread("Test message".to_string());
        app.active_thread_id = Some(thread_id);

        terminal
            .draw(|f| {
                render(f, &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(
            buffer_str.contains("Responding"),
            "Conversation screen should show spinner with 'Responding...' when a message is streaming"
        );
    }

    #[test]
    fn test_conversation_screen_shows_partial_content_during_streaming() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;

        // Create a streaming thread and append some tokens
        let thread_id = app.cache.create_streaming_thread("Test message".to_string());
        app.cache.append_to_message(&thread_id, "Hello from ");
        app.cache.append_to_message(&thread_id, "the AI");
        app.active_thread_id = Some(thread_id);

        terminal
            .draw(|f| {
                render(f, &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(
            buffer_str.contains("Hello from the AI"),
            "Conversation screen should show partial_content during streaming"
        );
    }

    #[test]
    fn test_conversation_screen_shows_cursor_during_streaming() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;
        app.tick_count = 0; // Ensure cursor is visible (tick_count / 5) % 2 == 0

        // Create a streaming thread
        let thread_id = app.cache.create_streaming_thread("Test".to_string());
        app.cache.append_to_message(&thread_id, "Response");
        app.active_thread_id = Some(thread_id);

        terminal
            .draw(|f| {
                render(f, &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        // The cursor character █ should be present when tick_count makes it visible
        assert!(
            buffer_str.contains("█"),
            "Conversation screen should show blinking cursor during streaming"
        );
    }

    #[test]
    fn test_conversation_screen_cursor_blinks() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;

        // Create a streaming thread
        let thread_id = app.cache.create_streaming_thread("Test".to_string());
        app.cache.append_to_message(&thread_id, "Response");
        app.active_thread_id = Some(thread_id.clone());

        // Test cursor visible (tick_count = 0, 0/5 % 2 == 0)
        app.tick_count = 0;
        terminal
            .draw(|f| {
                render(f, &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str_visible: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // Test cursor hidden (tick_count = 5, 5/5 % 2 == 1)
        app.tick_count = 5;
        let backend2 = TestBackend::new(100, 30);
        let mut terminal2 = Terminal::new(backend2).unwrap();
        terminal2
            .draw(|f| {
                render(f, &app);
            })
            .unwrap();

        let buffer2 = terminal2.backend().buffer();
        let buffer_str_hidden: String = buffer2
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // When visible, should have █; when hidden, the cursor position should have space
        assert!(
            buffer_str_visible.contains("█"),
            "Cursor should be visible at tick_count=0"
        );
        // Note: The hidden cursor shows a space, so we check that █ is not present
        // or that the behavior differs
        assert!(
            !buffer_str_hidden.contains("█"),
            "Cursor should be hidden at tick_count=5"
        );
    }

    #[test]
    fn test_conversation_screen_no_streaming_indicator_for_completed_messages() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;

        // Create a streaming thread and finalize it
        let thread_id = app.cache.create_streaming_thread("Test message".to_string());
        app.cache.append_to_message(&thread_id, "Completed response");
        app.cache.finalize_message(&thread_id, 123);
        app.active_thread_id = Some(thread_id);

        terminal
            .draw(|f| {
                render(f, &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(
            !buffer_str.contains("Responding..."),
            "Conversation screen should NOT show 'Responding...' spinner for completed messages"
        );
    }

    #[test]
    fn test_conversation_screen_shows_completed_message_content() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;

        // Create a streaming thread and finalize it
        let thread_id = app.cache.create_streaming_thread("User question".to_string());
        app.cache.append_to_message(&thread_id, "Final answer from AI");
        app.cache.finalize_message(&thread_id, 456);
        app.active_thread_id = Some(thread_id);

        terminal
            .draw(|f| {
                render(f, &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(
            buffer_str.contains("Final answer from AI"),
            "Conversation screen should show completed message content"
        );
        // Should NOT have the blinking cursor for completed messages
        assert!(
            !buffer_str.contains("█"),
            "Conversation screen should NOT show cursor for completed messages"
        );
    }

    #[test]
    fn test_conversation_screen_shows_user_message() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;

        // Create a streaming thread (which includes a user message)
        let thread_id = app.cache.create_streaming_thread("Hello from user".to_string());
        app.active_thread_id = Some(thread_id);

        terminal
            .draw(|f| {
                render(f, &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(
            buffer_str.contains("Hello from user"),
            "Conversation screen should show user message content"
        );
        assert!(
            buffer_str.contains("You:"),
            "Conversation screen should show 'You:' label for user messages"
        );
    }

    // ============= Mode Indicator Tests =============

    #[test]
    fn test_create_mode_indicator_line_plan_mode() {
        let line = create_mode_indicator_line(ProgrammingMode::PlanMode);
        assert!(line.is_some());
        let line = line.unwrap();
        // Check that the line contains "[PLAN MODE]"
        let content: String = line.spans.iter().map(|s| s.content.to_string()).collect();
        assert!(content.contains("[PLAN MODE]"));
    }

    #[test]
    fn test_create_mode_indicator_line_bypass() {
        let line = create_mode_indicator_line(ProgrammingMode::BypassPermissions);
        assert!(line.is_some());
        let line = line.unwrap();
        // Check that the line contains "[BYPASS]"
        let content: String = line.spans.iter().map(|s| s.content.to_string()).collect();
        assert!(content.contains("[BYPASS]"));
    }

    #[test]
    fn test_create_mode_indicator_line_none() {
        let line = create_mode_indicator_line(ProgrammingMode::None);
        assert!(line.is_none());
    }

    #[test]
    fn test_mode_indicator_not_shown_for_conversation_thread() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;

        // Create a Normal thread (not Programming)
        app.cache.upsert_thread(crate::models::Thread {
            id: "conv-thread".to_string(),
            title: "Normal Thread".to_string(),
            description: None,
            preview: "Just chatting".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Normal,
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: chrono::Utc::now(),
        });
        app.active_thread_id = Some("conv-thread".to_string());
        app.programming_mode = ProgrammingMode::PlanMode; // Set mode, but shouldn't show

        terminal
            .draw(|f| {
                render(f, &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // Mode indicator should NOT be shown for Conversation threads
        assert!(
            !buffer_str.contains("[PLAN MODE]"),
            "Mode indicator should not be shown for Conversation threads"
        );
        assert!(
            !buffer_str.contains("[BYPASS]"),
            "Mode indicator should not be shown for Conversation threads"
        );
    }

    #[test]
    fn test_mode_indicator_shown_for_programming_thread_plan_mode() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;

        // Create a Programming thread
        app.cache.upsert_thread(crate::models::Thread {
            id: "prog-thread".to_string(),
            title: "Programming Thread".to_string(),
            description: None,
            preview: "Code review".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Programming,
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: chrono::Utc::now(),
        });
        app.active_thread_id = Some("prog-thread".to_string());
        app.programming_mode = ProgrammingMode::PlanMode;

        terminal
            .draw(|f| {
                render(f, &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        assert!(
            buffer_str.contains("[PLAN MODE]"),
            "Mode indicator should show '[PLAN MODE]' for Programming thread in PlanMode"
        );
    }

    #[test]
    fn test_mode_indicator_shown_for_programming_thread_bypass_mode() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;

        // Create a Programming thread
        app.cache.upsert_thread(crate::models::Thread {
            id: "prog-thread".to_string(),
            title: "Programming Thread".to_string(),
            description: None,
            preview: "Code review".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Programming,
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: chrono::Utc::now(),
        });
        app.active_thread_id = Some("prog-thread".to_string());
        app.programming_mode = ProgrammingMode::BypassPermissions;

        terminal
            .draw(|f| {
                render(f, &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        assert!(
            buffer_str.contains("[BYPASS]"),
            "Mode indicator should show '[BYPASS]' for Programming thread in BypassPermissions"
        );
    }

    #[test]
    fn test_mode_indicator_not_shown_for_programming_thread_none_mode() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;

        // Create a Programming thread
        app.cache.upsert_thread(crate::models::Thread {
            id: "prog-thread".to_string(),
            title: "Programming Thread".to_string(),
            description: None,
            preview: "Code review".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Programming,
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: chrono::Utc::now(),
        });
        app.active_thread_id = Some("prog-thread".to_string());
        app.programming_mode = ProgrammingMode::None;

        terminal
            .draw(|f| {
                render(f, &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // When mode is None, no indicator should be shown
        assert!(
            !buffer_str.contains("[PLAN MODE]"),
            "Mode indicator should not show '[PLAN MODE]' when mode is None"
        );
        assert!(
            !buffer_str.contains("[BYPASS]"),
            "Mode indicator should not show '[BYPASS]' when mode is None"
        );
    }

    #[test]
    fn test_mode_indicator_not_shown_on_command_deck() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::CommandDeck; // Not on Conversation screen
        app.programming_mode = ProgrammingMode::PlanMode;

        terminal
            .draw(|f| {
                render(f, &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // Mode indicator should not be shown on CommandDeck
        assert!(
            !buffer_str.contains("[PLAN MODE]"),
            "Mode indicator should not be shown on CommandDeck screen"
        );
    }

    #[test]
    fn test_mode_indicator_not_shown_when_no_active_thread() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;
        app.active_thread_id = None; // No active thread
        app.programming_mode = ProgrammingMode::PlanMode;

        terminal
            .draw(|f| {
                render(f, &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // Mode indicator should not be shown when there's no active thread
        assert!(
            !buffer_str.contains("[PLAN MODE]"),
            "Mode indicator should not be shown when there's no active thread"
        );
    }

    // ========================================================================
    // Tests for Phase 6: Thread Type Indicators and Model Names
    // ========================================================================

    #[test]
    fn test_extract_short_model_name_opus() {
        assert_eq!(extract_short_model_name("claude-opus-4-5-20250514"), "opus");
        assert_eq!(extract_short_model_name("claude-opus-3-5"), "opus");
        assert_eq!(extract_short_model_name("opus-anything"), "opus");
    }

    #[test]
    fn test_extract_short_model_name_sonnet() {
        assert_eq!(extract_short_model_name("claude-sonnet-4-5-20250514"), "sonnet");
        assert_eq!(extract_short_model_name("claude-sonnet-3-5"), "sonnet");
        assert_eq!(extract_short_model_name("sonnet-anything"), "sonnet");
    }

    #[test]
    fn test_extract_short_model_name_other_models() {
        assert_eq!(extract_short_model_name("gpt-4"), "gpt");
        assert_eq!(extract_short_model_name("gpt-3.5-turbo"), "gpt");
        assert_eq!(extract_short_model_name("llama-2-70b"), "llama");
    }

    #[test]
    fn test_extract_short_model_name_simple_model() {
        assert_eq!(extract_short_model_name("simple"), "simple");
        assert_eq!(extract_short_model_name("model"), "model");
    }

    #[test]
    fn test_thread_type_indicator_shown_for_normal_thread() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::CommandDeck;

        // Add a normal thread to the cache
        app.cache.upsert_thread(crate::models::Thread {
            id: "thread-1".to_string(),
            title: "Normal Thread".to_string(),
            description: None,
            preview: "A normal conversation".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Normal,
            model: Some("claude-sonnet-4-5".to_string()),
            permission_mode: None,
            message_count: 0,
            created_at: chrono::Utc::now(),
        });

        terminal
            .draw(|f| {
                render(f, &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // Should show [N] indicator for Normal thread
        assert!(
            buffer_str.contains("[N]"),
            "Thread type indicator [N] should be shown for Normal threads"
        );
    }

    #[test]
    fn test_thread_type_indicator_shown_for_programming_thread() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::CommandDeck;

        // Add a programming thread to the cache
        app.cache.upsert_thread(crate::models::Thread {
            id: "thread-1".to_string(),
            title: "Programming Thread".to_string(),
            description: None,
            preview: "A programming conversation".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Programming,
            model: Some("claude-opus-4-5".to_string()),
            permission_mode: None,
            message_count: 0,
            created_at: chrono::Utc::now(),
        });

        terminal
            .draw(|f| {
                render(f, &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // Should show [P] indicator for Programming thread
        assert!(
            buffer_str.contains("[P]"),
            "Thread type indicator [P] should be shown for Programming threads"
        );
    }

    #[test]
    fn test_model_name_shown_with_thread_type_indicator() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::CommandDeck;

        // Add a thread with model information
        app.cache.upsert_thread(crate::models::Thread {
            id: "thread-1".to_string(),
            title: "Thread with Model".to_string(),
            description: None,
            preview: "Testing model display".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Normal,
            model: Some("claude-sonnet-4-5-20250514".to_string()),
            permission_mode: None,
            message_count: 0,
            created_at: chrono::Utc::now(),
        });

        terminal
            .draw(|f| {
                render(f, &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // Should show [N] and "sonnet" model name
        assert!(
            buffer_str.contains("[N]"),
            "Thread type indicator should be shown"
        );
        assert!(
            buffer_str.contains("sonnet"),
            "Short model name should be shown next to type indicator"
        );
    }

    #[test]
    fn test_thread_type_indicator_without_model() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::CommandDeck;

        // Add a thread without model information
        app.cache.upsert_thread(crate::models::Thread {
            id: "thread-1".to_string(),
            title: "Thread without Model".to_string(),
            description: None,
            preview: "No model info".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Programming,
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: chrono::Utc::now(),
        });

        terminal
            .draw(|f| {
                render(f, &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // Should show [P] indicator even without model
        assert!(
            buffer_str.contains("[P]"),
            "Thread type indicator should be shown even without model information"
        );
    }

    #[test]
    fn test_multiple_threads_show_different_type_indicators() {
        let backend = TestBackend::new(120, 50);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::CommandDeck;

        // Add a normal thread
        app.cache.upsert_thread(crate::models::Thread {
            id: "thread-1".to_string(),
            title: "Normal".to_string(),
            description: None,
            preview: "Normal thread".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Normal,
            model: Some("claude-sonnet-4-5".to_string()),
            permission_mode: None,
            message_count: 0,
            created_at: chrono::Utc::now(),
        });

        // Add a programming thread
        app.cache.upsert_thread(crate::models::Thread {
            id: "thread-2".to_string(),
            title: "Programming".to_string(),
            description: None,
            preview: "Programming thread".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Programming,
            model: Some("claude-opus-4-5".to_string()),
            permission_mode: None,
            message_count: 0,
            created_at: chrono::Utc::now(),
        });

        terminal
            .draw(|f| {
                render(f, &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // Both indicators should be present
        assert!(
            buffer_str.contains("[N]"),
            "Should show [N] indicator for normal thread"
        );
        assert!(
            buffer_str.contains("[P]"),
            "Should show [P] indicator for programming thread"
        );
        assert!(
            buffer_str.contains("sonnet"),
            "Should show sonnet model name"
        );
        assert!(
            buffer_str.contains("opus"),
            "Should show opus model name"
        );
    }

    // ============= Phase 10: Contextual Keybinds Tests =============

    #[test]
    fn test_contextual_keybinds_command_deck() {
        let app = create_test_app();
        // app.screen defaults to CommandDeck

        let keybinds = build_contextual_keybinds(&app);
        let content: String = keybinds.spans.iter().map(|s| s.content.to_string()).collect();

        // Should show basic CommandDeck hints
        assert!(content.contains("Tab"));
        assert!(content.contains("switch focus"));
        assert!(content.contains("Enter"));
        assert!(content.contains("send"));
    }

    #[test]
    fn test_contextual_keybinds_conversation_with_error() {
        let mut app = create_test_app();
        app.screen = Screen::Conversation;
        app.stream_error = Some("Test error".to_string());

        let keybinds = build_contextual_keybinds(&app);
        let content: String = keybinds.spans.iter().map(|s| s.content.to_string()).collect();

        // Should show dismiss error hint
        assert!(content.contains("d"));
        assert!(content.contains("dismiss error"));
    }

    #[test]
    fn test_contextual_keybinds_programming_thread_shows_mode_cycling() {
        let mut app = create_test_app();
        app.screen = Screen::Conversation;

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

        let keybinds = build_contextual_keybinds(&app);
        let content: String = keybinds.spans.iter().map(|s| s.content.to_string()).collect();

        // Should show mode cycling hint for programming thread
        assert!(content.contains("Shift+Tab"));
        assert!(content.contains("cycle mode"));
    }

    #[test]
    fn test_contextual_keybinds_normal_thread_no_mode_cycling() {
        let mut app = create_test_app();
        app.screen = Screen::Conversation;

        // Create a normal thread
        app.cache.upsert_thread(crate::models::Thread {
            id: "conv-thread".to_string(),
            title: "Normal".to_string(),
            description: None,
            preview: "Chat".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Normal,
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: chrono::Utc::now(),
        });
        app.active_thread_id = Some("conv-thread".to_string());

        let keybinds = build_contextual_keybinds(&app);
        let content: String = keybinds.spans.iter().map(|s| s.content.to_string()).collect();

        // Should NOT show mode cycling hint for normal thread
        assert!(!content.contains("Shift+Tab"));
        assert!(!content.contains("cycle mode"));
    }

    // ============= Phase 10: Streaming Input Border Tests =============

    #[test]
    fn test_conversation_input_uses_dashed_border_when_streaming() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;

        // Create a streaming thread
        let thread_id = app.cache.create_streaming_thread("Test".to_string());
        app.active_thread_id = Some(thread_id);

        terminal
            .draw(|f| {
                render(f, &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // Should have dashed border characters (┄)
        assert!(
            buffer_str.contains("┄"),
            "Input should use dashed border when streaming"
        );
    }

    #[test]
    fn test_conversation_input_uses_solid_border_when_not_streaming() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;

        // Create a thread with completed message
        let thread_id = app.cache.create_streaming_thread("Test".to_string());
        app.cache.append_to_message(&thread_id, "Response");
        app.cache.finalize_message(&thread_id, 1);
        app.active_thread_id = Some(thread_id);

        terminal
            .draw(|f| {
                render(f, &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // Should have solid border characters (─), not dashed
        assert!(
            buffer_str.contains("─"),
            "Input should use solid border when not streaming"
        );
    }

    // ============= Permission Prompt Tests =============

    #[test]
    fn test_get_permission_preview_returns_context() {
        use crate::state::session::PermissionRequest;

        let perm = PermissionRequest {
            permission_id: "perm-test".to_string(),
            tool_name: "Write".to_string(),
            description: "Write file".to_string(),
            context: Some("/home/user/test.rs".to_string()),
            tool_input: None,
        };

        let preview = get_permission_preview(&perm);
        assert_eq!(preview, "/home/user/test.rs");
    }

    #[test]
    fn test_get_permission_preview_extracts_file_path() {
        use crate::state::session::PermissionRequest;

        let perm = PermissionRequest {
            permission_id: "perm-test".to_string(),
            tool_name: "Read".to_string(),
            description: "Read file".to_string(),
            context: None,
            tool_input: Some(serde_json::json!({"file_path": "/var/log/test.log"})),
        };

        let preview = get_permission_preview(&perm);
        assert_eq!(preview, "/var/log/test.log");
    }

    #[test]
    fn test_get_permission_preview_extracts_command() {
        use crate::state::session::PermissionRequest;

        let perm = PermissionRequest {
            permission_id: "perm-test".to_string(),
            tool_name: "Bash".to_string(),
            description: "Run command".to_string(),
            context: None,
            tool_input: Some(serde_json::json!({"command": "npm install"})),
        };

        let preview = get_permission_preview(&perm);
        assert_eq!(preview, "npm install");
    }

    #[test]
    fn test_get_permission_preview_truncates_long_content() {
        use crate::state::session::PermissionRequest;

        let long_content = "a".repeat(150);
        let perm = PermissionRequest {
            permission_id: "perm-test".to_string(),
            tool_name: "Write".to_string(),
            description: "Write file".to_string(),
            context: None,
            tool_input: Some(serde_json::json!({"content": long_content})),
        };

        let preview = get_permission_preview(&perm);
        assert!(preview.len() < 110); // Should be truncated
        assert!(preview.ends_with("..."));
    }

    #[test]
    fn test_get_permission_preview_empty_when_no_info() {
        use crate::state::session::PermissionRequest;

        let perm = PermissionRequest {
            permission_id: "perm-test".to_string(),
            tool_name: "Custom".to_string(),
            description: "Custom action".to_string(),
            context: None,
            tool_input: None,
        };

        let preview = get_permission_preview(&perm);
        assert!(preview.is_empty());
    }

    #[test]
    fn test_permission_prompt_renders_with_pending_permission() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;

        // Set up a pending permission
        use crate::state::session::PermissionRequest;
        app.session_state.set_pending_permission(PermissionRequest {
            permission_id: "perm-render".to_string(),
            tool_name: "Bash".to_string(),
            description: "Run npm install".to_string(),
            context: Some("npm install".to_string()),
            tool_input: None,
        });

        terminal
            .draw(|f| {
                render(f, &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // Check that permission prompt elements are rendered
        assert!(
            buffer_str.contains("Permission Required"),
            "Should show 'Permission Required' title"
        );
        assert!(
            buffer_str.contains("Bash"),
            "Should show tool name"
        );
    }

    #[test]
    fn test_format_tokens_small() {
        assert_eq!(format_tokens(0), "0");
        assert_eq!(format_tokens(500), "500");
        assert_eq!(format_tokens(999), "999");
    }

    #[test]
    fn test_format_tokens_thousands() {
        assert_eq!(format_tokens(1_000), "1k");
        assert_eq!(format_tokens(5_000), "5k");
        assert_eq!(format_tokens(45_000), "45k");
        assert_eq!(format_tokens(100_000), "100k");
        assert_eq!(format_tokens(999_999), "999k");
    }

    #[test]
    fn test_format_tokens_millions() {
        assert_eq!(format_tokens(1_000_000), "1M");
        assert_eq!(format_tokens(5_000_000), "5M");
        assert_eq!(format_tokens(10_000_000), "10M");
    }
}
