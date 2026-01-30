//! Rendering for queued steering messages in unified scroll

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use crate::app::App;
use crate::models::SteeringMessageState;

/// Prefix for steering lines (matches message prefix pattern)
const STEERING_PREFIX: &str = "│ ";

/// Build lines for the queued steering message (if any)
///
/// Returns lines that integrate into the unified scroll above the input section.
/// Uses the same `│ ` prefix as messages for visual consistency.
pub fn build_steering_lines(app: &App, max_width: usize) -> Vec<Line<'static>> {
    let Some(ref qs) = app.queued_steering else {
        return Vec::new();
    };

    // Don't render if completed (it's been promoted to a message)
    if matches!(qs.state, SteeringMessageState::Completed) {
        return Vec::new();
    }

    let mut lines: Vec<Line<'static>> = Vec::new();
    let prefix_style = Style::default().fg(Color::DarkGray);

    // Style based on state
    let (icon, main_style) = match &qs.state {
        SteeringMessageState::Queued => (
            "⏳",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::DIM),
        ),
        SteeringMessageState::Sent => (
            "→",
            Style::default().fg(Color::Yellow),
        ),
        SteeringMessageState::Interrupting => (
            "⚡",
            Style::default().fg(Color::Cyan),
        ),
        SteeringMessageState::Resuming => (
            "⚡",
            Style::default().fg(Color::Cyan),
        ),
        SteeringMessageState::Completed => (
            "✓",
            Style::default().fg(Color::Green),
        ),
        SteeringMessageState::Failed(_) => (
            "❌",
            Style::default().fg(Color::Red),
        ),
    };

    let state_text = qs.state.display_text();

    // Calculate max preview length (leave room for prefix + icon + state)
    let overhead = STEERING_PREFIX.len() + icon.len() + state_text.len() + 4; // 4 for ": " and spacing
    let preview_max = max_width.saturating_sub(overhead);
    let preview = qs.preview(preview_max);

    // Line 1: Icon + State + Preview
    lines.push(Line::from(vec![
        Span::styled(STEERING_PREFIX, prefix_style),
        Span::styled(format!("{} ", icon), main_style),
        Span::styled(format!("{}: ", state_text), main_style),
        Span::styled(preview, main_style.add_modifier(Modifier::ITALIC)),
    ]));

    // Line 2: State details (for some states)
    let detail_text = match &qs.state {
        SteeringMessageState::Queued => Some("   Sending to backend..."),
        SteeringMessageState::Sent => Some("   Waiting for safe boundary..."),
        SteeringMessageState::Interrupting => Some("   Interrupting current stream..."),
        SteeringMessageState::Resuming => Some("   Resuming with steering..."),
        SteeringMessageState::Failed(_err) => {
            // Error will be shown on separate line below
            None
        }
        _ => None,
    };

    if let Some(detail) = detail_text {
        lines.push(Line::from(vec![
            Span::styled(STEERING_PREFIX, prefix_style),
            Span::styled(
                detail,
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    }

    // Special handling for failed state - show error
    if let SteeringMessageState::Failed(ref err) = qs.state {
        let err_preview = if err.len() > max_width - 6 {
            format!("   {}...", &err[..max_width.saturating_sub(9)])
        } else {
            format!("   {}", err)
        };
        lines.push(Line::from(vec![
            Span::styled(STEERING_PREFIX, prefix_style),
            Span::styled(
                err_preview,
                Style::default().fg(Color::Red).add_modifier(Modifier::DIM),
            ),
        ]));
    }

    // Add blank line after steering block for spacing
    lines.push(Line::from(vec![
        Span::styled(STEERING_PREFIX, prefix_style),
    ]));

    lines
}
