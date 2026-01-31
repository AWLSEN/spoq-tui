//! Status Indicator Component
//!
//! Renders spinner, success, and error status indicators.
//! Used for provisioning progress, completion, and error states.

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use crate::ui::theme::{COLOR_DIM, COLOR_TOOL_SUCCESS};

/// Spinner animation frames
const SPINNER_FRAMES: [char; 4] = ['◐', '◓', '◑', '◒'];

/// Status indicator types
#[derive(Debug, Clone)]
pub enum StatusIndicatorType {
    /// Spinning indicator with a message
    Spinner {
        /// Message to display (e.g., "Replacing VPS...")
        message: String,
        /// Current frame index (0-3, auto-cycles)
        frame: usize,
    },
    /// Progress indicator with percentage and message
    Progress {
        /// Progress percentage (0-100)
        percent: u8,
        /// Current phase message
        message: String,
    },
    /// Success indicator with message
    Success {
        /// Success message (e.g., "VPS Connected!")
        message: String,
    },
    /// Error indicator with message and auth flag
    Error {
        /// Error header (e.g., "Failed to replace VPS")
        header: String,
        /// Error details
        details: Option<String>,
        /// Whether this is an auth error (shows login option)
        is_auth_error: bool,
    },
    /// Info indicator with message
    Info {
        /// Info message
        message: String,
    },
}

impl StatusIndicatorType {
    /// Create a new spinner indicator
    pub fn spinner(message: impl Into<String>) -> Self {
        Self::Spinner {
            message: message.into(),
            frame: 0,
        }
    }

    /// Create a new progress indicator
    pub fn progress(percent: u8, message: impl Into<String>) -> Self {
        Self::Progress {
            percent,
            message: message.into(),
        }
    }

    /// Create a new success indicator
    pub fn success(message: impl Into<String>) -> Self {
        Self::Success {
            message: message.into(),
        }
    }

    /// Create a new error indicator
    pub fn error(header: impl Into<String>, details: Option<String>, is_auth_error: bool) -> Self {
        Self::Error {
            header: header.into(),
            details,
            is_auth_error,
        }
    }

    /// Create a new info indicator
    pub fn info(message: impl Into<String>) -> Self {
        Self::Info {
            message: message.into(),
        }
    }
}

/// Get the current spinner character based on frame
pub fn get_spinner_char(frame: usize) -> char {
    SPINNER_FRAMES[frame % SPINNER_FRAMES.len()]
}

/// Advance the spinner frame
pub fn next_spinner_frame(current: usize) -> usize {
    (current + 1) % SPINNER_FRAMES.len()
}

/// Render a status indicator as multiple lines
///
/// # Arguments
/// * `indicator` - The status indicator to render
///
/// # Returns
/// A vector of lines to be rendered
pub fn render_status_indicator(indicator: &StatusIndicatorType) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    match indicator {
        StatusIndicatorType::Spinner { message, frame } => {
            lines.push(Line::from("")); // Top padding

            let spinner_char = get_spinner_char(*frame);
            lines.push(Line::from(vec![
                Span::styled(
                    format!("         {} ", spinner_char),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(
                    message.clone(),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));

            lines.push(Line::from("")); // Padding
        }

        StatusIndicatorType::Progress { percent, message } => {
            lines.push(Line::from("")); // Top padding

            lines.push(Line::from(vec![Span::styled(
                format!("      {}% - {}", percent, message),
                Style::default().fg(Color::Yellow),
            )]));

            lines.push(Line::from("")); // Padding

            lines.push(Line::from(vec![Span::styled(
                "    Please wait, this may take",
                Style::default().fg(COLOR_DIM),
            )]));

            lines.push(Line::from(vec![Span::styled(
                "    a few minutes.",
                Style::default().fg(COLOR_DIM),
            )]));
        }

        StatusIndicatorType::Success { message } => {
            lines.push(Line::from("")); // Top padding
            lines.push(Line::from("")); // Extra padding

            lines.push(Line::from(vec![
                Span::styled(
                    "         \u{25CF} ", // Bullet
                    Style::default().fg(COLOR_TOOL_SUCCESS),
                ),
                Span::styled(
                    message.clone(),
                    Style::default()
                        .fg(COLOR_TOOL_SUCCESS)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));

            lines.push(Line::from("")); // Padding
        }

        StatusIndicatorType::Error {
            header,
            details,
            is_auth_error: _,
        } => {
            lines.push(Line::from("")); // Top padding
            lines.push(Line::from("")); // Extra padding

            lines.push(Line::from(vec![
                Span::styled(
                    "    \u{2717} ", // X mark
                    Style::default().fg(Color::Red),
                ),
                Span::styled(
                    header.clone(),
                    Style::default()
                        .fg(Color::Red)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));

            lines.push(Line::from("")); // Padding

            if let Some(detail) = details {
                // Truncate long details
                let truncated = if detail.len() > 40 {
                    format!("{}...", &detail[..37])
                } else {
                    detail.clone()
                };
                lines.push(Line::from(vec![Span::styled(
                    format!("    {}", truncated),
                    Style::default().fg(Color::White),
                )]));
            }

            lines.push(Line::from("")); // Padding
        }

        StatusIndicatorType::Info { message } => {
            lines.push(Line::from(vec![Span::styled(
                format!("    {}", message),
                Style::default().fg(COLOR_DIM),
            )]));
        }
    }

    lines
}

/// Calculate the height needed for a status indicator
pub fn calculate_status_height(indicator: &StatusIndicatorType) -> u16 {
    match indicator {
        StatusIndicatorType::Spinner { .. } => 3,
        StatusIndicatorType::Progress { .. } => 5,
        StatusIndicatorType::Success { .. } => 4,
        StatusIndicatorType::Error { details, .. } => {
            if details.is_some() {
                6
            } else {
                5
            }
        }
        StatusIndicatorType::Info { .. } => 1,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spinner_frames() {
        assert_eq!(get_spinner_char(0), '◐');
        assert_eq!(get_spinner_char(1), '◓');
        assert_eq!(get_spinner_char(2), '◑');
        assert_eq!(get_spinner_char(3), '◒');
        assert_eq!(get_spinner_char(4), '◐'); // Wraps around
    }

    #[test]
    fn test_next_spinner_frame() {
        assert_eq!(next_spinner_frame(0), 1);
        assert_eq!(next_spinner_frame(1), 2);
        assert_eq!(next_spinner_frame(2), 3);
        assert_eq!(next_spinner_frame(3), 0); // Wraps around
    }

    #[test]
    fn test_status_indicator_spinner() {
        let indicator = StatusIndicatorType::spinner("Loading...");
        let lines = render_status_indicator(&indicator);
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_status_indicator_progress() {
        let indicator = StatusIndicatorType::progress(45, "Executing script");
        let lines = render_status_indicator(&indicator);
        let text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect();
        assert!(text.contains("45%"));
        assert!(text.contains("Executing script"));
    }

    #[test]
    fn test_status_indicator_success() {
        let indicator = StatusIndicatorType::success("VPS Connected!");
        let lines = render_status_indicator(&indicator);
        let text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect();
        assert!(text.contains("VPS Connected!"));
    }

    #[test]
    fn test_status_indicator_error() {
        let indicator =
            StatusIndicatorType::error("Failed", Some("Connection refused".to_string()), false);
        let lines = render_status_indicator(&indicator);
        let text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect();
        assert!(text.contains("Failed"));
        assert!(text.contains("Connection refused"));
    }

    #[test]
    fn test_calculate_status_height() {
        assert_eq!(
            calculate_status_height(&StatusIndicatorType::spinner("Test")),
            3
        );
        assert_eq!(
            calculate_status_height(&StatusIndicatorType::progress(50, "Test")),
            5
        );
        assert_eq!(
            calculate_status_height(&StatusIndicatorType::success("Test")),
            4
        );
        assert_eq!(
            calculate_status_height(&StatusIndicatorType::error("Test", None, false)),
            5
        );
        assert_eq!(
            calculate_status_height(&StatusIndicatorType::error(
                "Test",
                Some("Details".to_string()),
                false
            )),
            6
        );
    }
}
