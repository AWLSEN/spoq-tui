//! Rendering for queued steering messages in unified scroll
//!
//! Shows the user's steering instruction as a right-indented text block
//! with user-message background. No icons, no state text — just the message.

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use crate::app::App;
use crate::models::SteeringMessageState;
use super::messages::{apply_background_to_line, wrap_line_with_prefix};
use super::theme::COLOR_HUMAN_BG;

/// Left indent for steering messages (8 spaces).
/// Must be `&'static str` for `wrap_line_with_prefix`.
const STEERING_INDENT: &str = "        ";

/// Right margin subtracted from max_width to make the block narrower
/// than normal user messages.
const STEERING_RIGHT_MARGIN: usize = 6;

/// Build lines for the queued steering message (if any).
///
/// For active states: shows only the instruction text, right-indented,
/// no `│` border, narrower width, with `COLOR_HUMAN_BG` background.
///
/// For Completed: returns empty (promotion handles display).
/// For Failed: shows error text with same indent, red styling.
pub fn build_steering_lines(app: &App, max_width: usize) -> Vec<Line<'static>> {
    let Some(ref qs) = app.queued_steering else {
        return Vec::new();
    };

    // Completed: return empty — promotion adds a normal user message
    if matches!(qs.state, SteeringMessageState::Completed) {
        return Vec::new();
    }

    let effective_width = max_width.saturating_sub(STEERING_RIGHT_MARGIN);

    // Narrow terminal fallback
    if effective_width < STEERING_INDENT.len() + 5 {
        let mut line = Line::from(vec![Span::styled(
            "...",
            Style::default().fg(Color::DarkGray),
        )]);
        apply_background_to_line(&mut line, COLOR_HUMAN_BG, max_width);
        return vec![line];
    }

    // Failed state: delegate to helper
    if let SteeringMessageState::Failed(ref err) = qs.state {
        return build_failed_lines(err, effective_width);
    }

    // Active states: render instruction text with indent and background
    let instruction = &qs.instruction;

    // Empty instruction guard
    if instruction.trim().is_empty() {
        let mut line = Line::from(vec![Span::styled(STEERING_INDENT, Style::default())]);
        apply_background_to_line(&mut line, COLOR_HUMAN_BG, effective_width);
        return vec![line];
    }

    let text_style = Style::default().fg(Color::White);
    let indent_style = Style::default();
    let mut all_lines: Vec<Line<'static>> = Vec::new();

    // Split on newlines to handle multi-line instructions (shift+enter)
    for raw_line in instruction.split('\n') {
        let content_line = Line::from(vec![Span::styled(raw_line.to_string(), text_style)]);
        let wrapped = wrap_line_with_prefix(
            content_line,
            STEERING_INDENT,
            indent_style,
            effective_width,
            Some(COLOR_HUMAN_BG),
        );
        all_lines.extend(wrapped);
    }

    // Defensive: ensure at least one line
    if all_lines.is_empty() {
        let mut line = Line::from(vec![Span::styled(STEERING_INDENT, indent_style)]);
        apply_background_to_line(&mut line, COLOR_HUMAN_BG, effective_width);
        all_lines.push(line);
    }

    all_lines
}

/// Build lines for the Failed state.
///
/// Shows "Error: <message>" with steering indent, in red text
/// on the user-message background.
fn build_failed_lines(err: &str, effective_width: usize) -> Vec<Line<'static>> {
    let error_style = Style::default()
        .fg(Color::Red)
        .add_modifier(Modifier::DIM);
    let indent_style = Style::default();

    let display_text = if err.is_empty() {
        "Steering failed".to_string()
    } else {
        format!("Error: {}", err)
    };

    let mut all_lines: Vec<Line<'static>> = Vec::new();

    for raw_line in display_text.split('\n') {
        let content_line = Line::from(vec![Span::styled(raw_line.to_string(), error_style)]);
        let wrapped = wrap_line_with_prefix(
            content_line,
            STEERING_INDENT,
            indent_style,
            effective_width,
            Some(COLOR_HUMAN_BG),
        );
        all_lines.extend(wrapped);
    }

    if all_lines.is_empty() {
        let mut line = Line::from(vec![
            Span::styled(STEERING_INDENT, indent_style),
            Span::styled("Steering failed", error_style),
        ]);
        apply_background_to_line(&mut line, COLOR_HUMAN_BG, effective_width);
        all_lines.push(line);
    }

    all_lines
}
