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
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::app::{App, Focus, Screen};
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

    // Use threads from cache
    let cached_threads = app.cache.threads();
    let threads_to_render: Vec<(String, String)> = if cached_threads.is_empty() {
        // Fallback mock data if cache is empty
        vec![
            ("Project Setup".to_string(), "Setting up Rust environment...".to_string()),
            ("Bug Investigation".to_string(), "Analyzing stack trace...".to_string()),
            ("Feature Request".to_string(), "Adding dark mode support...".to_string()),
        ]
    } else {
        cached_threads.iter().map(|t| {
            (t.title.clone(), t.preview.clone())
        }).collect()
    };

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
        lines.push(Line::from(vec![
            Span::raw(padding_str.clone()),
            Span::styled("│ ", Style::default().fg(card_border_color)),
            Span::styled(title_marker, Style::default().fg(if is_selected { COLOR_HEADER } else { COLOR_ACCENT })),
            Span::styled(
                format!("Thread: {}", title),
                Style::default()
                    .fg(if is_selected { Color::White } else { COLOR_HEADER })
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{:>width$}│", "", width = 35_usize.saturating_sub(10 + title.len())),
                Style::default().fg(card_border_color),
            ),
        ]));

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

    // Create main layout sections
    let inner = inner_rect(size, 1);
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

            lines.push(Line::from(vec![
                Span::styled(label, label_style),
                Span::styled(&message.content, Style::default().fg(Color::White)),
            ]));
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

    // Show current input preview if there's input being typed
    let user_input = app.input_box.content();
    if !user_input.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            "─────────────────────────────────────────",
            Style::default().fg(COLOR_DIM),
        )]));
        lines.push(Line::from(vec![
            Span::styled(
                "You (typing): ",
                Style::default()
                    .fg(COLOR_ACTIVE)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(user_input, Style::default().fg(Color::White)),
        ]));
        lines.push(Line::from(""));
    }

    let messages_widget = Paragraph::new(lines);
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
    fn test_conversation_screen_shows_user_input() {
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

        // Check that the buffer shows "You (typing):" label when there's input being typed
        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(
            buffer_str.contains("You (typing):"),
            "Conversation screen should show typing indicator when input is present"
        );
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
}
