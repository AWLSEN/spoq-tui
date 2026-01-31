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
pub fn calculate_height(state: &VpsConfigState) -> u16 {
    match state {
        VpsConfigState::InputFields { mode, error, .. } => {
            match mode {
                VpsConfigMode::Remote => {
                    // Title(1) + blank(1) + mode(1) + blank(1) + IP(2) + username_static(1) + password(2) + error?(1) + blank(1) + help(1)
                    if error.is_some() { 12 } else { 11 }
                }
                VpsConfigMode::Local => {
                    // Title(1) + blank(1) + mode(1) + blank(1) + info(2) + blank(1) + help(1)
                    8
                }
            }
        }
        VpsConfigState::Provisioning { .. } => 8,
        VpsConfigState::Success { .. } => 8,
        VpsConfigState::Error { .. } => 8,
        VpsConfigState::Authenticating { .. } => 10,
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
    let mut constraints = vec![
        Constraint::Length(1), // Title
        Constraint::Length(1), // Blank
        Constraint::Length(1), // Mode selector
        Constraint::Length(1), // Blank
        Constraint::Length(1), // IP label
        Constraint::Length(1), // IP input
        Constraint::Length(1), // Username (static)
        Constraint::Length(1), // Password label
        Constraint::Length(1), // Password input
    ];

    if error.is_some() {
        constraints.push(Constraint::Length(1)); // Error
    }

    constraints.push(Constraint::Length(1)); // Blank
    constraints.push(Constraint::Length(1)); // Help
    constraints.push(Constraint::Min(0));

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

    // IP Address field
    render_field(frame, chunks[4], chunks[5], "VPS IP Address", ip, field_focus == 1, false);

    // Username (static, non-editable)
    let username_line = Line::from(vec![
        Span::styled("  SSH Username: ", Style::default().fg(COLOR_DIM)),
        Span::styled("root", Style::default().fg(Color::White)),
    ]);
    frame.render_widget(Paragraph::new(username_line), chunks[6]);

    // Password field
    render_field(frame, chunks[7], chunks[8], "SSH Password", password, field_focus == 2, true);

    // Error message
    let help_idx = if error.is_some() {
        let error_line = Line::from(Span::styled(
            error.unwrap_or(""),
            Style::default().fg(Color::Red),
        ));
        frame.render_widget(Paragraph::new(error_line).alignment(Alignment::Center), chunks[9]);
        11
    } else {
        10
    };

    // Help line
    let help = Line::from(vec![
        Span::styled("[Tab]", Style::default().fg(Color::Green)),
        Span::raw(" Next   "),
        Span::styled("[Enter]", Style::default().fg(Color::Green)),
        Span::raw(" Submit   "),
        Span::styled("[Esc]", Style::default().fg(Color::Red)),
        Span::raw(" Cancel"),
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

/// Render a single input field with label
fn render_field(
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
        format!("   {}", label),
        Style::default().fg(label_color),
    ));
    frame.render_widget(Paragraph::new(label_line), label_area);

    // Input box with rounded border
    let border_color = if focused { Color::White } else { COLOR_DIM };

    let bordered_area = Rect::new(
        input_area.x + 3,
        input_area.y,
        input_area.width.saturating_sub(6),
        1,
    );

    // For single-line input, we render text with cursor inline
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
            Span::styled(" ", Style::default().bg(Color::White).fg(Color::Black)), // Block cursor
        ])
    } else {
        Line::from(Span::styled(&display_text, Style::default().fg(text_color)))
    };

    // Render the input field background and border
    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(COLOR_INPUT_BG));

    frame.render_widget(input_block, bordered_area);

    // Render text inside (offset by 1 for border)
    let text_area = Rect::new(
        bordered_area.x + 1,
        bordered_area.y,
        bordered_area.width.saturating_sub(2),
        1,
    );
    frame.render_widget(Paragraph::new(content), text_area);
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
