//! VPS configuration card rendering.
//!
//! Renders the VPS config dialog with different states:
//! - InputFields: Form with IP, username, password fields
//! - Provisioning: Progress indicator while replacing VPS
//! - Success: Green checkmark with hostname
//! - Error: Red error message with retry option

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::ui::theme::{COLOR_DIM, COLOR_DIALOG_BG, COLOR_INPUT_BG, COLOR_TOOL_SUCCESS};
use crate::view_state::{VpsConfigMode, VpsConfigState};

/// Calculate the height needed for the VPS config card based on state.
///
/// CRITICAL: Height must account for all rendered elements including borders.
/// Input fields with borders need 3 rows (top border + content + bottom border).
pub fn calculate_height(state: &VpsConfigState) -> u16 {
    match state {
        VpsConfigState::InputFields { mode, error, .. } => {
            match mode {
                VpsConfigMode::Remote => {
                    // Dynamic height calculation:
                    // - Title: 1
                    // - Blank: 1
                    // - Mode selector: 1
                    // - Blank: 1
                    // - IP label: 1
                    // - IP input box (with border): 3
                    // - Blank: 1
                    // - Username (static): 1
                    // - Blank: 1
                    // - Password label: 1
                    // - Password input box (with border): 3
                    // - Error (optional): 1
                    // - Blank: 1
                    // - Help: 1
                    // Total without error: 17
                    // Total with error: 18
                    if error.is_some() { 18 } else { 17 }
                }
                VpsConfigMode::Local => {
                    // - Title: 1
                    // - Blank: 1
                    // - Mode selector: 1
                    // - Blank: 1
                    // - Info line 1: 1
                    // - Info line 2: 1
                    // - Blank: 1
                    // - Help: 1
                    8
                }
            }
        }
        VpsConfigState::Provisioning { .. } => {
            // - Title: 1
            // - Blank: 2
            // - Spinner + message: 1
            // - Blank: 1
            // - Secondary text: 1
            // - Blank: 2
            8
        }
        VpsConfigState::Success { .. } => {
            // - Title: 1
            // - Blank: 2
            // - Success message: 1
            // - Hostname: 1
            // - Blank: 1
            // - Help: 1
            // - Blank: 1
            8
        }
        VpsConfigState::Error { .. } => {
            // - Title: 1
            // - Blank: 2
            // - Error header: 1
            // - Error details: 1
            // - Blank: 1
            // - Help: 1
            // - Blank: 1
            8
        }
        VpsConfigState::Authenticating { .. } => {
            // - Title: 1
            // - Blank: 1
            // - Instruction: 1
            // - Blank: 1
            // - URL: 1
            // - Blank: 1
            // - Code: 1
            // - Blank: 1
            // - Status: 1
            // - Blank: 1
            10
        }
    }
}

/// Render the VPS config card content.
///
/// # Arguments
///
/// * `frame` - The ratatui frame to render to
/// * `area` - The inner area of the card (excluding border)
/// * `state` - Current state of the VPS config dialog
pub fn render(frame: &mut Frame, area: Rect, state: &VpsConfigState) {
    // Set dialog background
    let bg_block = Block::default().style(Style::default().bg(COLOR_DIALOG_BG));
    frame.render_widget(bg_block, area);

    match state {
        VpsConfigState::InputFields {
            mode,
            ip,
            password,
            field_focus,
            error,
        } => {
            render_input_fields(frame, area, mode, ip, password, *field_focus, error.as_deref());
        }
        VpsConfigState::Provisioning { phase } => {
            render_provisioning(frame, area, phase);
        }
        VpsConfigState::Success { hostname } => {
            render_success(frame, area, hostname);
        }
        VpsConfigState::Error { error, is_auth_error } => {
            render_error(frame, area, error, *is_auth_error);
        }
        VpsConfigState::Authenticating { verification_url, user_code } => {
            render_authenticating(frame, area, verification_url, user_code);
        }
    }
}

/// Render the input fields state
fn render_input_fields(
    frame: &mut Frame,
    area: Rect,
    mode: &VpsConfigMode,
    ip: &str,
    password: &str,
    field_focus: u8,
    error: Option<&str>,
) {
    match mode {
        VpsConfigMode::Remote => render_remote_fields(frame, area, ip, password, field_focus, error),
        VpsConfigMode::Local => render_local_fields(frame, area, field_focus),
    }
}

/// Render the mode selector row
fn render_mode_selector(frame: &mut Frame, area: Rect, mode: &VpsConfigMode, focused: bool) {
    let (remote_style, local_style) = match mode {
        VpsConfigMode::Remote => (
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            Style::default().fg(COLOR_DIM),
        ),
        VpsConfigMode::Local => (
            Style::default().fg(COLOR_DIM),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ),
    };

    let highlight_left = if focused { "\u{25b8} " } else { "" };
    let highlight_right = if focused { " \u{25c2}" } else { "" };

    let line = match mode {
        VpsConfigMode::Remote => Line::from(vec![
            Span::styled("   Mode: ", Style::default().fg(COLOR_DIM)),
            Span::styled(format!("{highlight_left}Remote VPS{highlight_right}"), remote_style),
            Span::raw("  "),
            Span::styled("Local", local_style),
        ]),
        VpsConfigMode::Local => Line::from(vec![
            Span::styled("   Mode: ", Style::default().fg(COLOR_DIM)),
            Span::styled("Remote VPS", remote_style),
            Span::raw("  "),
            Span::styled(format!("{highlight_left}Local{highlight_right}"), local_style),
        ]),
    };

    frame.render_widget(Paragraph::new(line), area);
}

/// Render Remote mode: mode selector + IP/username/password fields
fn render_remote_fields(
    frame: &mut Frame,
    area: Rect,
    ip: &str,
    password: &str,
    field_focus: u8,
    error: Option<&str>,
) {
    // Build constraints dynamically based on error presence
    // CRITICAL: Input boxes with borders need 3 rows (top + content + bottom)
    let mut constraints = vec![
        Constraint::Length(1), // 0: Title
        Constraint::Length(1), // 1: Blank
        Constraint::Length(1), // 2: Mode selector
        Constraint::Length(1), // 3: Blank
        Constraint::Length(1), // 4: IP label
        Constraint::Length(3), // 5: IP input box (with border)
        Constraint::Length(1), // 6: Blank
        Constraint::Length(1), // 7: Username (static)
        Constraint::Length(1), // 8: Blank
        Constraint::Length(1), // 9: Password label
        Constraint::Length(3), // 10: Password input box (with border)
    ];

    // Track indices for dynamic elements
    let error_idx: Option<usize>;
    let help_idx: usize;

    if error.is_some() {
        constraints.push(Constraint::Length(1)); // 11: Error
        error_idx = Some(11);
        constraints.push(Constraint::Length(1)); // 12: Blank
        constraints.push(Constraint::Length(1)); // 13: Help
        help_idx = 13;
    } else {
        error_idx = None;
        constraints.push(Constraint::Length(1)); // 11: Blank
        constraints.push(Constraint::Length(1)); // 12: Help
        help_idx = 12;
    }

    constraints.push(Constraint::Min(0)); // Flexible remaining space

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    // Title
    let title = Line::from(Span::styled(
        "Change VPS",
        Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
    ));
    frame.render_widget(Paragraph::new(title).alignment(Alignment::Center), chunks[0]);

    // Mode selector
    render_mode_selector(frame, chunks[2], &VpsConfigMode::Remote, field_focus == 0);

    // IP Address field (label at index 4, input at index 5)
    render_input_box(frame, chunks[4], chunks[5], "VPS IP Address", ip, field_focus == 1, false);

    // Username (static, non-editable)
    let username_line = Line::from(vec![
        Span::styled("  SSH Username: ", Style::default().fg(COLOR_DIM)),
        Span::styled("root", Style::default().fg(Color::White)),
    ]);
    frame.render_widget(Paragraph::new(username_line), chunks[7]);

    // Password field (label at index 9, input at index 10)
    render_input_box(frame, chunks[9], chunks[10], "SSH Password", password, field_focus == 2, true);

    // Error message (if present)
    if let Some(idx) = error_idx {
        let error_line = Line::from(vec![
            Span::styled("\u{2717} ", Style::default().fg(Color::Red)), // X mark
            Span::styled(
                error.unwrap_or(""),
                Style::default().fg(Color::Red),
            ),
        ]);
        frame.render_widget(Paragraph::new(error_line).alignment(Alignment::Center), chunks[idx]);
    }

    // Help line
    let help = Line::from(vec![
        Span::styled("Tab", Style::default().fg(Color::Green)),
        Span::styled(" Next \u{00B7} ", Style::default().fg(COLOR_DIM)),
        Span::styled("Enter", Style::default().fg(Color::Green)),
        Span::styled(" Submit \u{00B7} ", Style::default().fg(COLOR_DIM)),
        Span::styled("Esc", Style::default().fg(Color::Red)),
        Span::styled(" Back", Style::default().fg(COLOR_DIM)),
    ]);
    frame.render_widget(Paragraph::new(help).alignment(Alignment::Center), chunks[help_idx]);
}

/// Render Local mode: mode selector + info text
fn render_local_fields(frame: &mut Frame, area: Rect, field_focus: u8) {
    let constraints = vec![
        Constraint::Length(1), // Title
        Constraint::Length(1), // Blank
        Constraint::Length(1), // Mode selector
        Constraint::Length(1), // Blank
        Constraint::Length(1), // Info line 1
        Constraint::Length(1), // Info line 2
        Constraint::Length(1), // Blank
        Constraint::Length(1), // Help
        Constraint::Min(0),
    ];

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    // Title
    let title = Line::from(Span::styled(
        "Change VPS",
        Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
    ));
    frame.render_widget(Paragraph::new(title).alignment(Alignment::Center), chunks[0]);

    // Mode selector
    render_mode_selector(frame, chunks[2], &VpsConfigMode::Local, field_focus == 0);

    // Info text
    let info1 = Line::from(Span::styled(
        "   Conductor will run locally",
        Style::default().fg(COLOR_DIM),
    ));
    frame.render_widget(Paragraph::new(info1), chunks[4]);

    let info2 = Line::from(Span::styled(
        "   on http://localhost:8000",
        Style::default().fg(COLOR_DIM),
    ));
    frame.render_widget(Paragraph::new(info2), chunks[5]);

    // Help line
    let help = Line::from(vec![
        Span::styled("[\u{2190}\u{2192}]", Style::default().fg(Color::Green)),
        Span::raw(" Mode   "),
        Span::styled("[Enter]", Style::default().fg(Color::Green)),
        Span::raw(" Start   "),
        Span::styled("[Esc]", Style::default().fg(Color::Red)),
        Span::raw(" Cancel"),
    ]);
    frame.render_widget(Paragraph::new(help).alignment(Alignment::Center), chunks[7]);
}

/// Render a single input box with label
///
/// # Arguments
/// * `frame` - The frame to render to
/// * `label_area` - Area for the label (1 row)
/// * `input_area` - Area for the input box (3 rows for borders)
/// * `label` - Label text
/// * `value` - Current input value
/// * `focused` - Whether the field is focused
/// * `is_password` - Whether to mask the value
fn render_input_box(
    frame: &mut Frame,
    label_area: Rect,
    input_area: Rect,
    label: &str,
    value: &str,
    focused: bool,
    is_password: bool,
) {
    // Label
    let label_color = if focused { Color::White } else { COLOR_DIM };
    let label_line = Line::from(Span::styled(
        format!("  {}", label),
        Style::default().fg(label_color),
    ));
    frame.render_widget(Paragraph::new(label_line), label_area);

    // Input box with rounded border - needs 3 rows minimum
    let border_color = if focused { Color::White } else { COLOR_DIM };

    // Calculate bordered area with proper margins
    let bordered_area = Rect::new(
        input_area.x + 2,
        input_area.y,
        input_area.width.saturating_sub(4),
        3.min(input_area.height), // Ensure we don't exceed available height
    );

    // Prepare display text
    let display_text = if is_password {
        "\u{2022}".repeat(value.len()) // Bullet character
    } else {
        value.to_string()
    };

    // Build text with cursor if focused
    let text_color = if focused { Color::White } else { COLOR_DIM };
    let content = if focused {
        Line::from(vec![
            Span::styled(&display_text, Style::default().fg(text_color)),
            Span::styled("\u{2588}", Style::default().fg(Color::White)), // Block cursor
        ])
    } else {
        Line::from(Span::styled(&display_text, Style::default().fg(text_color)))
    };

    // Render the input field with block and border
    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(COLOR_INPUT_BG));

    let input_widget = Paragraph::new(content).block(input_block);
    frame.render_widget(input_widget, bordered_area);
}

/// Render the provisioning state
fn render_provisioning(frame: &mut Frame, area: Rect, phase: &str) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Title
            Constraint::Length(1), // Blank
            Constraint::Length(1), // Blank
            Constraint::Length(1), // Phase text
            Constraint::Length(1), // Blank
            Constraint::Length(1), // Secondary text
            Constraint::Min(0),
        ])
        .split(area);

    // Title
    let title = Line::from(Span::styled(
        "Change VPS",
        Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
    ));
    frame.render_widget(
        Paragraph::new(title).alignment(Alignment::Center),
        chunks[0],
    );

    // Phase text - yellow + bold
    let phase_line = Line::from(Span::styled(
        phase,
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
    ));
    frame.render_widget(
        Paragraph::new(phase_line).alignment(Alignment::Center),
        chunks[3],
    );

    // Secondary text
    let secondary = Line::from(Span::styled(
        "Please wait, this may take a few minutes",
        Style::default().fg(COLOR_DIM),
    ));
    frame.render_widget(
        Paragraph::new(secondary).alignment(Alignment::Center),
        chunks[5],
    );
}

/// Render the success state
fn render_success(frame: &mut Frame, area: Rect, hostname: &str) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Title
            Constraint::Length(1), // Blank
            Constraint::Length(1), // Blank
            Constraint::Length(1), // Success message
            Constraint::Length(1), // Hostname
            Constraint::Length(1), // Blank
            Constraint::Length(1), // Help
            Constraint::Min(0),
        ])
        .split(area);

    // Title
    let title = Line::from(Span::styled(
        "Change VPS",
        Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
    ));
    frame.render_widget(
        Paragraph::new(title).alignment(Alignment::Center),
        chunks[0],
    );

    // Success message with green bullet
    let success = Line::from(vec![
        Span::styled("\u{25CF} ", Style::default().fg(COLOR_TOOL_SUCCESS)), // Filled circle
        Span::styled(
            "VPS connected!",
            Style::default().fg(COLOR_TOOL_SUCCESS).add_modifier(Modifier::BOLD),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(success).alignment(Alignment::Center),
        chunks[3],
    );

    // Hostname
    let hostname_line = Line::from(Span::styled(
        hostname,
        Style::default().fg(Color::Cyan),
    ));
    frame.render_widget(
        Paragraph::new(hostname_line).alignment(Alignment::Center),
        chunks[4],
    );

    // Help line
    let help = Line::from(vec![
        Span::styled("[Enter]", Style::default().fg(Color::Green)),
        Span::raw(" Done"),
    ]);
    frame.render_widget(
        Paragraph::new(help).alignment(Alignment::Center),
        chunks[6],
    );
}

/// Render the error state
fn render_error(frame: &mut Frame, area: Rect, error: &str, is_auth_error: bool) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Title
            Constraint::Length(1), // Blank
            Constraint::Length(1), // Blank
            Constraint::Length(1), // Error header
            Constraint::Length(1), // Error details
            Constraint::Length(1), // Blank
            Constraint::Length(1), // Help
            Constraint::Min(0),
        ])
        .split(area);

    // Title
    let title = Line::from(Span::styled(
        "Change VPS",
        Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
    ));
    frame.render_widget(
        Paragraph::new(title).alignment(Alignment::Center),
        chunks[0],
    );

    // Error header with X mark
    let error_header = Line::from(vec![
        Span::styled("\u{2717} ", Style::default().fg(Color::Red)), // X mark
        Span::styled(
            if is_auth_error { "Session expired" } else { "Failed to replace VPS" },
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(error_header).alignment(Alignment::Center),
        chunks[3],
    );

    // Error details (truncated to 60 chars)
    let display_error = if error.len() > 60 {
        format!("{}...", &error[..57])
    } else {
        error.to_string()
    };
    let error_details = Line::from(Span::styled(
        display_error,
        Style::default().fg(Color::White),
    ));
    frame.render_widget(
        Paragraph::new(error_details).alignment(Alignment::Center),
        chunks[4],
    );

    // Help line - different options for auth errors
    let help = if is_auth_error {
        Line::from(vec![
            Span::styled("[L]", Style::default().fg(Color::Green)),
            Span::raw(" Login   "),
            Span::styled("[Esc]", Style::default().fg(Color::Red)),
            Span::raw(" Cancel"),
        ])
    } else {
        Line::from(vec![
            Span::styled("[R]", Style::default().fg(Color::Green)),
            Span::raw(" Retry   "),
            Span::styled("[Esc]", Style::default().fg(Color::Red)),
            Span::raw(" Cancel"),
        ])
    };
    frame.render_widget(
        Paragraph::new(help).alignment(Alignment::Center),
        chunks[6],
    );
}

/// Render the authenticating state (device flow)
fn render_authenticating(frame: &mut Frame, area: Rect, verification_url: &str, user_code: &str) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Title
            Constraint::Length(1), // Blank
            Constraint::Length(1), // Instruction
            Constraint::Length(1), // Blank
            Constraint::Length(1), // URL
            Constraint::Length(1), // Blank
            Constraint::Length(1), // Code
            Constraint::Length(1), // Blank
            Constraint::Length(1), // Status
            Constraint::Min(0),
        ])
        .split(area);

    // Title
    let title = Line::from(Span::styled(
        "Change VPS",
        Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
    ));
    frame.render_widget(
        Paragraph::new(title).alignment(Alignment::Center),
        chunks[0],
    );

    // Instruction
    let instruction = Line::from(Span::styled(
        "Open this URL in your browser:",
        Style::default().fg(Color::White),
    ));
    frame.render_widget(
        Paragraph::new(instruction).alignment(Alignment::Center),
        chunks[2],
    );

    // URL
    let url_line = Line::from(Span::styled(
        verification_url,
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
    ));
    frame.render_widget(
        Paragraph::new(url_line).alignment(Alignment::Center),
        chunks[4],
    );

    // Code
    let code_line = Line::from(vec![
        Span::styled("Code: ", Style::default().fg(COLOR_DIM)),
        Span::styled(
            user_code,
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(code_line).alignment(Alignment::Center),
        chunks[6],
    );

    // Status
    let status = Line::from(Span::styled(
        "Waiting for authorization...",
        Style::default().fg(Color::Yellow),
    ));
    frame.render_widget(
        Paragraph::new(status).alignment(Alignment::Center),
        chunks[8],
    );
}
