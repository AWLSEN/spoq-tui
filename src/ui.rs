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

use crate::app::{App, Focus};
use crate::state::{Notification, TaskStatus};
use crate::widgets::input_box::InputBoxWidget;

// ============================================================================
// Cyberpunk Color Theme
// ============================================================================

/// Primary border color - cyan for that classic cyberpunk look
pub const COLOR_BORDER: Color = Color::Cyan;

/// Accent color - magenta for highlights and important elements
pub const COLOR_ACCENT: Color = Color::Magenta;

/// Header text color - bright cyan for the logo
pub const COLOR_HEADER: Color = Color::LightCyan;

/// Active/running elements - bright green
pub const COLOR_ACTIVE: Color = Color::LightGreen;

/// Queued/pending elements - yellow
pub const COLOR_QUEUED: Color = Color::Yellow;

/// Dim text for less important info
pub const COLOR_DIM: Color = Color::DarkGray;

/// Background for input areas (used in later phases)
#[allow(dead_code)]
pub const COLOR_INPUT_BG: Color = Color::Rgb(20, 20, 30);

/// Progress bar fill color
pub const COLOR_PROGRESS: Color = Color::Magenta;

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

/// Render the complete Command Deck UI
pub fn render(frame: &mut Frame, app: &App) {
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
            Constraint::Length(5),  // Input area
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
    // Split header into logo area and info area
    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(38), // Logo width
            Constraint::Min(20),    // Info area
        ])
        .split(area);

    render_logo(frame, header_chunks[0]);
    render_header_info(frame, header_chunks[1], app);
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
    // Calculate migration progress (mock for now - 67%)
    let migration_progress = 0.67;
    let is_migrating = migration_progress < 1.0;

    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "COMMAND DECK v0.1.0",
            Style::default().fg(COLOR_ACCENT).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    if is_migrating {
        lines.push(Line::from(vec![
            Span::styled("[MIGRATING] ", Style::default().fg(COLOR_QUEUED)),
            Span::styled(
                format!("{}%", (migration_progress * 100.0) as u8),
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

    let info = Paragraph::new(lines);
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
    let mock_notifications = vec![
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
    let header_style = if focused {
        Style::default().fg(COLOR_HEADER).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(COLOR_HEADER).add_modifier(Modifier::BOLD)
    };

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

    // Use actual threads from app if available, otherwise use mock
    let threads_to_render: Vec<(String, String)> = if app.threads.is_empty() {
        vec![
            ("Project Setup".to_string(), "Setting up Rust environment...".to_string()),
            ("Bug Investigation".to_string(), "Analyzing stack trace...".to_string()),
            ("Feature Request".to_string(), "Adding dark mode support...".to_string()),
        ]
    } else {
        app.threads.iter().map(|t| {
            (t.title.clone(), t.preview.clone())
        }).collect()
    };

    for (i, (title, preview)) in threads_to_render.iter().enumerate() {
        let is_selected = focused && i == app.threads_index;
        let card_border_color = if is_selected { COLOR_HEADER } else { COLOR_BORDER };

        // Thread card top border
        lines.push(Line::from(Span::styled(
            "┌─────────────────────────────────────┐",
            Style::default().fg(card_border_color),
        )));

        // Thread title
        let title_marker = if is_selected { "▶ " } else { "► " };
        lines.push(Line::from(vec![
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

        // Thread preview
        lines.push(Line::from(vec![
            Span::styled("│   ", Style::default().fg(card_border_color)),
            Span::styled(format!("\"{}\"", preview), Style::default().fg(COLOR_DIM)),
            Span::styled(
                format!("{:>width$}│", "", width = 35_usize.saturating_sub(4 + preview.len())),
                Style::default().fg(card_border_color),
            ),
        ]));

        // Thread card bottom border
        lines.push(Line::from(Span::styled(
            "└─────────────────────────────────────┘",
            Style::default().fg(card_border_color),
        )));
        lines.push(Line::from(""));
    }

    // Keybind hints at bottom of threads panel
    lines.push(Line::from(vec![
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
            Constraint::Length(3), // Input box (needs 3 for border + content)
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
