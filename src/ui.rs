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

/// Progress bar background (used in later phases)
#[allow(dead_code)]
pub const COLOR_PROGRESS_BG: Color = Color::DarkGray;

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

    let mut lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "COMMAND DECK v0.1.0",
                Style::default().fg(COLOR_ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled("[", Style::default().fg(COLOR_DIM)),
            Span::styled(status_icon, Style::default().fg(status_color)),
            Span::styled("] ", Style::default().fg(COLOR_DIM)),
            Span::styled(status_text, Style::default().fg(status_color)),
        ]),
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
    let border_color = if focused { COLOR_ACCENT } else { COLOR_DIM };
    let task_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(border_color));
    frame.render_widget(task_block.clone(), area);

    let inner = inner_rect(area, 1);

    // Split into saved and active columns
    let task_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(inner);

    render_saved_tasks(frame, task_chunks[0], app, focused);
    render_active_tasks(frame, task_chunks[1], app, focused);
}

fn render_saved_tasks(frame: &mut Frame, area: Rect, app: &App, focused: bool) {
    let header_style = Style::default().fg(COLOR_HEADER).add_modifier(Modifier::BOLD);

    let mut lines = vec![
        Line::from(Span::styled(
            if focused { "◇ SAVED ◄" } else { "◇ SAVED" },
            header_style,
        )),
        Line::from(Span::styled("────────────", Style::default().fg(if focused { COLOR_ACCENT } else { COLOR_DIM }))),
    ];

    // Mock saved tasks for static render
    let saved_tasks = ["task-001", "task-002", "task-003"];
    for (i, task) in saved_tasks.iter().enumerate() {
        let is_selected = focused && i == app.tasks_index;
        let marker = if is_selected { "▶ " } else { "□ " };
        let marker_style = if is_selected {
            Style::default().fg(COLOR_HEADER)
        } else {
            Style::default().fg(COLOR_DIM)
        };

        lines.push(Line::from(vec![
            Span::styled(marker, marker_style),
            Span::styled(
                *task,
                if is_selected {
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                },
            ),
        ]));
    }

    let saved = Paragraph::new(lines);
    frame.render_widget(saved, area);
}

fn render_active_tasks(frame: &mut Frame, area: Rect, _app: &App, _focused: bool) {
    let mut lines = vec![
        Line::from(Span::styled(
            "◆ ACTIVE",
            Style::default().fg(COLOR_ACTIVE).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled("─────────────", Style::default().fg(COLOR_DIM))),
    ];

    // Mock active task with progress
    lines.push(Line::from(vec![
        Span::styled("▶ ", Style::default().fg(COLOR_ACTIVE)),
        Span::raw("task-004"),
    ]));
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled("░░░░░░░", Style::default().fg(COLOR_PROGRESS)),
        Span::styled(" 23%", Style::default().fg(COLOR_ACCENT)),
    ]));
    lines.push(Line::from(""));

    // Mock queued task
    lines.push(Line::from(vec![
        Span::styled("◌ ", Style::default().fg(COLOR_QUEUED)),
        Span::raw("task-005"),
    ]));
    lines.push(Line::from(Span::styled(
        "  [QUEUED]",
        Style::default().fg(COLOR_QUEUED),
    )));

    let active = Paragraph::new(lines);
    frame.render_widget(active, area);
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

        // Thread type indicator and model info (centered)
        let thread = &cached_threads[i];
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

    // Render the InputBox widget
    let input_widget = InputBoxWidget::new(&app.input_box, "", input_focused);
    frame.render_widget(input_widget, input_chunks[0]);

    // Keybind hints
    let keybinds = Line::from(vec![
        Span::raw(" "),
        Span::styled("[ENTER]", Style::default().fg(COLOR_ACCENT)),
        Span::raw(" Send   "),
        Span::styled("[TAB]", Style::default().fg(COLOR_ACCENT)),
        Span::raw(" Switch   "),
        Span::styled("[CTRL+C]", Style::default().fg(COLOR_ACCENT)),
        Span::raw(" Exit   "),
        Span::styled("[ESC]", Style::default().fg(COLOR_ACCENT)),
        Span::raw(" Back"),
    ]);

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

    // Create main layout sections - conditionally include mode indicator
    let inner = inner_rect(size, 1);

    if let Some(mode_line) = mode_indicator_line {
        // Layout with mode indicator (4 sections)
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
    } else {
        // Layout without mode indicator (3 sections)
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

/// Render the mode indicator bar
fn render_mode_indicator(frame: &mut Frame, area: Rect, mode_line: Line<'static>) {
    let indicator = Paragraph::new(mode_line);
    frame.render_widget(indicator, area);
}

/// Render the thread title header with connection status
fn render_conversation_header(frame: &mut Frame, area: Rect, app: &App) {
    // Get thread title from cache or default
    let thread_title = app
        .active_thread_id
        .as_ref()
        .and_then(|id| app.cache.get_thread(id))
        .map(|t| t.title.as_str())
        .unwrap_or("New Conversation");

    // Connection status indicator
    let (status_icon, status_text, status_color) = if app.connection_status {
        ("●", "Connected", Color::LightGreen)
    } else {
        ("○", "Disconnected", Color::Red)
    };

    let header_block = Block::default()
        .borders(Borders::BOTTOM)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(COLOR_BORDER));

    // Split header area to show title on left and connection status on right
    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(20),     // Thread title (flexible)
            Constraint::Length(20),  // Connection status (fixed)
        ])
        .split(area);

    // Thread title (left side)
    let title_text = Line::from(vec![
        Span::styled("  Thread: ", Style::default().fg(COLOR_DIM)),
        Span::styled(
            thread_title,
            Style::default()
                .fg(COLOR_HEADER)
                .add_modifier(Modifier::BOLD),
        ),
    ]);

    let title_widget = Paragraph::new(title_text).block(header_block);
    frame.render_widget(title_widget, header_chunks[0]);

    // Connection status (right side)
    let status_text_widget = Line::from(vec![
        Span::styled("[", Style::default().fg(COLOR_DIM)),
        Span::styled(status_icon, Style::default().fg(status_color)),
        Span::styled("] ", Style::default().fg(COLOR_DIM)),
        Span::styled(status_text, Style::default().fg(status_color)),
    ]);

    let status_widget = Paragraph::new(status_text_widget)
        .alignment(ratatui::layout::Alignment::Right);
    frame.render_widget(status_widget, header_chunks[1]);
}

/// Render the messages area with user messages and AI responses
fn render_messages_area(frame: &mut Frame, area: Rect, app: &App) {
    use crate::models::MessageRole;

    let inner = inner_rect(area, 1);
    let mut lines: Vec<Line> = Vec::new();

    // Show error banner if there's a stream error
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

    // Check if any message is currently streaming
    let has_streaming_message = cached_messages
        .as_ref()
        .map(|msgs| msgs.iter().any(|m| m.is_streaming))
        .unwrap_or(false);

    // Show "AI is responding..." indicator if streaming
    if has_streaming_message {
        lines.push(Line::from(vec![
            Span::styled(
                "  AI is responding...",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(""));
    }

    if let Some(messages) = cached_messages {
        for message in messages {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![Span::styled(
                "───────────────────────────────────────────────",
                Style::default().fg(COLOR_DIM),
            )]));

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
                // Display partial_content with blinking cursor
                // Blink cursor every ~500ms (assuming 10 ticks/sec, toggle every 5 ticks)
                let show_cursor = (app.tick_count / 5) % 2 == 0;
                let cursor_span = Span::styled(
                    if show_cursor { "█" } else { " " },
                    Style::default().fg(COLOR_ACCENT),
                );

                // Parse partial content with markdown renderer
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
            } else {
                // Display completed message content with markdown rendering
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
}

/// Render the input area for conversation screen
fn render_conversation_input(frame: &mut Frame, area: Rect, app: &App) {
    let input_focused = app.focus == Focus::Input;
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

    // Render the InputBox widget
    let input_widget = InputBoxWidget::new(&app.input_box, "", input_focused);
    frame.render_widget(input_widget, input_chunks[0]);

    // Conversation-specific keybind hints
    let keybinds = Line::from(vec![
        Span::raw(" "),
        Span::styled("[ENTER]", Style::default().fg(COLOR_ACCENT)),
        Span::raw(" Send   "),
        Span::styled("[Shift+ESC]", Style::default().fg(COLOR_ACCENT)),
        Span::raw(" Back to CommandDeck"),
    ]);

    let keybinds_widget = Paragraph::new(keybinds);
    frame.render_widget(keybinds_widget, input_chunks[1]);
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
            buffer_str.contains("Disconnected"),
            "Conversation screen should show Disconnected status"
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
            buffer_str.contains("Connected"),
            "Conversation screen should show Connected status"
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
            buffer_str.contains("AI is responding"),
            "Conversation screen should show 'AI is responding...' when a message is streaming"
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
            !buffer_str.contains("AI is responding"),
            "Conversation screen should NOT show 'AI is responding...' for completed messages"
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
}
