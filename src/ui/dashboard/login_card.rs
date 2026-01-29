//! Claude CLI login card rendering.
//!
//! Renders the Claude login dialog with different states:
//! - ShowingUrl: Initial state with URL and keyboard hints
//! - Verifying: Spinner while waiting for backend verification
//! - VerificationSuccess: Green checkmark and email
//! - VerificationFailed: Red error with retry option

use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::view_state::ClaudeLoginState;

/// Render the Claude login card content.
///
/// # Arguments
///
/// * `frame` - The ratatui frame to render to
/// * `area` - The inner area of the card (excluding border)
/// * `request_id` - The request ID for this login flow
/// * `auth_url` - The authentication URL to display
/// * `state` - Current state of the login dialog
pub fn render(
    frame: &mut Frame,
    area: Rect,
    _request_id: &str,
    auth_url: &str,
    state: &ClaudeLoginState,
) {
    let lines = match state {
        ClaudeLoginState::ShowingUrl { browser_opened } => {
            render_showing_url_content(auth_url, *browser_opened)
        }
        ClaudeLoginState::Verifying => render_verifying_content(),
        ClaudeLoginState::VerificationSuccess { email, .. } => render_success_content(email),
        ClaudeLoginState::VerificationFailed { error } => render_error_content(error),
        ClaudeLoginState::BrowserOpenFailed { auth_url, error } => {
            render_browser_failed_content(auth_url, error)
        }
    };

    let paragraph = Paragraph::new(lines).alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

/// Render the initial URL display state
fn render_showing_url_content(auth_url: &str, browser_opened: bool) -> Vec<Line<'static>> {
    let status_text = if browser_opened {
        "Browser opened - complete login there"
    } else {
        "Press Enter to open in browser"
    };

    // Truncate URL if too long for display
    let display_url = if auth_url.len() > 60 {
        format!("{}...", &auth_url[..57])
    } else {
        auth_url.to_string()
    };

    vec![
        Line::from(""),
        Line::from(Span::styled(
            "Claude CLI needs authentication.",
            Style::default().fg(Color::White),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw("URL: "),
            Span::styled(
                display_url,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::UNDERLINED),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            status_text,
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("[Enter]", Style::default().fg(Color::Green)),
            Span::raw(" Browser  "),
            Span::styled("[D]", Style::default().fg(Color::Green)),
            Span::raw(" Done  "),
            Span::styled("[Esc]", Style::default().fg(Color::Red)),
            Span::raw(" Cancel"),
        ]),
    ]
}

/// Render the verifying state with spinner
fn render_verifying_content() -> Vec<Line<'static>> {
    vec![
        Line::from(""),
        Line::from(""),
        Line::from(Span::styled(
            "Verifying login...",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Please wait while we confirm authentication.",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(""),
        Line::from(Span::styled(
            "(please wait)",
            Style::default().fg(Color::DarkGray),
        )),
    ]
}

/// Render the success state with checkmark
fn render_success_content(email: &str) -> Vec<Line<'static>> {
    vec![
        Line::from(""),
        Line::from(""),
        Line::from(Span::styled(
            "Authenticated!",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(format!("Logged in as: {}", email)),
        Line::from(""),
        Line::from(""),
        Line::from(Span::styled(
            "(closing...)",
            Style::default().fg(Color::DarkGray),
        )),
    ]
}

/// Render the error state with retry option
fn render_error_content(error: &str) -> Vec<Line<'static>> {
    // Truncate error if too long
    let display_error = if error.len() > 50 {
        format!("{}...", &error[..47])
    } else {
        error.to_string()
    };

    vec![
        Line::from(""),
        Line::from(Span::styled(
            "Authentication could not be verified.",
            Style::default().fg(Color::White),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw("Error: "),
            Span::styled(display_error, Style::default().fg(Color::Red)),
        ]),
        Line::from(""),
        Line::from(""),
        Line::from(vec![
            Span::styled("[R]", Style::default().fg(Color::Green)),
            Span::raw(" Retry  "),
            Span::styled("[Esc]", Style::default().fg(Color::Red)),
            Span::raw(" Cancel"),
        ]),
    ]
}

/// Render the browser open failed state with manual option
fn render_browser_failed_content(auth_url: &str, error: &str) -> Vec<Line<'static>> {
    // Truncate URL if too long
    let display_url = if auth_url.len() > 50 {
        format!("{}...", &auth_url[..47])
    } else {
        auth_url.to_string()
    };

    // Truncate error if too long
    let display_error = if error.len() > 60 {
        format!("{}...", &error[..57])
    } else {
        error.to_string()
    };

    vec![
        Line::from(""),
        Line::from(Span::styled(
            "Could not auto-open browser",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw("Error: "),
            Span::styled(display_error, Style::default().fg(Color::Red)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::raw("URL: "),
            Span::styled(
                display_url,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::UNDERLINED),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("[Enter]", Style::default().fg(Color::Green)),
            Span::raw(" Try Again  "),
            Span::styled("[Esc]", Style::default().fg(Color::Red)),
            Span::raw(" Cancel"),
        ]),
    ]
}

/// Calculate the height needed for the login card based on state
pub fn calculate_height(state: &ClaudeLoginState) -> u16 {
    match state {
        ClaudeLoginState::ShowingUrl { .. } => 10,
        ClaudeLoginState::Verifying => 10,
        ClaudeLoginState::VerificationSuccess { .. } => 10,
        ClaudeLoginState::VerificationFailed { .. } => 9,
        ClaudeLoginState::BrowserOpenFailed { .. } => 10,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn test_render_showing_url_browser_not_opened() {
        let backend = TestBackend::new(60, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                let area = Rect::new(2, 2, 56, 10);
                render(
                    frame,
                    area,
                    "req-123",
                    "https://example.com/auth",
                    &ClaudeLoginState::ShowingUrl {
                        browser_opened: false,
                    },
                );
            })
            .unwrap();

        // Should render without panic
    }

    #[test]
    fn test_render_showing_url_browser_opened() {
        let backend = TestBackend::new(60, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                let area = Rect::new(2, 2, 56, 10);
                render(
                    frame,
                    area,
                    "req-123",
                    "https://example.com/auth",
                    &ClaudeLoginState::ShowingUrl {
                        browser_opened: true,
                    },
                );
            })
            .unwrap();

        // Should render without panic
    }

    #[test]
    fn test_render_verifying() {
        let backend = TestBackend::new(60, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                let area = Rect::new(2, 2, 56, 10);
                render(
                    frame,
                    area,
                    "req-123",
                    "https://example.com/auth",
                    &ClaudeLoginState::Verifying,
                );
            })
            .unwrap();

        // Should render without panic
    }

    #[test]
    fn test_render_success() {
        let backend = TestBackend::new(60, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                let area = Rect::new(2, 2, 56, 10);
                render(
                    frame,
                    area,
                    "req-123",
                    "https://example.com/auth",
                    &ClaudeLoginState::VerificationSuccess {
                        email: "user@example.com".to_string(),
                        success_time: std::time::Instant::now(),
                    },
                );
            })
            .unwrap();

        // Should render without panic
    }

    #[test]
    fn test_render_error() {
        let backend = TestBackend::new(60, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                let area = Rect::new(2, 2, 56, 10);
                render(
                    frame,
                    area,
                    "req-123",
                    "https://example.com/auth",
                    &ClaudeLoginState::VerificationFailed {
                        error: "Authentication timeout".to_string(),
                    },
                );
            })
            .unwrap();

        // Should render without panic
    }

    #[test]
    fn test_render_long_url_truncated() {
        let backend = TestBackend::new(60, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        let long_url = "https://console.anthropic.com/oauth/authorize?client_id=abc123&redirect_uri=http://localhost:8080/callback&response_type=code&state=xyz";

        terminal
            .draw(|frame| {
                let area = Rect::new(2, 2, 56, 10);
                render(
                    frame,
                    area,
                    "req-123",
                    long_url,
                    &ClaudeLoginState::ShowingUrl {
                        browser_opened: false,
                    },
                );
            })
            .unwrap();

        // Should render without panic, URL should be truncated
    }

    #[test]
    fn test_calculate_height() {
        assert_eq!(
            calculate_height(&ClaudeLoginState::ShowingUrl {
                browser_opened: false
            }),
            10
        );
        assert_eq!(calculate_height(&ClaudeLoginState::Verifying), 10);
        assert_eq!(
            calculate_height(&ClaudeLoginState::VerificationSuccess {
                email: "test@example.com".to_string(),
                success_time: std::time::Instant::now(),
            }),
            10
        );
        assert_eq!(
            calculate_height(&ClaudeLoginState::VerificationFailed {
                error: "Test error".to_string()
            }),
            9
        );
    }
}
