//! Login Screen UI rendering
//!
//! Implements the OAuth device flow login screen with real-time state updates.
//! Shows the verification URI, user code, and authorization status.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::App;
use crate::auth::device_flow::DeviceFlowState;

use super::command_deck::SPOQ_LOGO;
use super::layout::LayoutContext;
use super::theme::{COLOR_BORDER, COLOR_DIM, COLOR_HEADER};

/// Render the login screen as a centered overlay.
///
/// Displays different content based on the device flow state:
/// - NotStarted: 'Initializing...'
/// - WaitingForUser: Verification URI prominently displayed with spinner
/// - Authorized: Success message in green
/// - Denied/Expired/Error: Error message in red with retry hint
pub fn render_login_screen(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // Create layout context from app's terminal dimensions
    let ctx = LayoutContext::new(app.terminal_width, app.terminal_height);

    // Calculate dialog dimensions
    let dialog_width = calculate_dialog_width(&ctx, area.width);
    let dialog_height = calculate_dialog_height(&ctx, app, area.height);

    // Center the dialog
    let x = (area.width.saturating_sub(dialog_width)) / 2;
    let y = (area.height.saturating_sub(dialog_height)) / 2;

    let dialog_area = Rect {
        x,
        y,
        width: dialog_width,
        height: dialog_height,
    };

    // Clear the background behind the dialog
    frame.render_widget(Clear, dialog_area);

    // Outer border (double border like command deck)
    let outer_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(COLOR_BORDER));
    frame.render_widget(outer_block, dialog_area);

    // Inner area for logo
    let logo_area = Rect {
        x: dialog_area.x + 2,
        y: dialog_area.y + 1,
        width: dialog_area.width.saturating_sub(4),
        height: 6, // SPOQ logo height
    };

    // Render SPOQ logo centered at top
    render_centered_logo(frame, logo_area);

    // Inner rounded dialog box for content
    let content_y = logo_area.y + logo_area.height + 1;
    let content_height = dialog_area
        .height
        .saturating_sub(logo_area.height + 4);

    let content_dialog_area = Rect {
        x: dialog_area.x + 4,
        y: content_y,
        width: dialog_area.width.saturating_sub(8),
        height: content_height,
    };

    let inner_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(COLOR_BORDER));
    frame.render_widget(inner_block, content_dialog_area);

    // Inner content area
    let inner = Rect {
        x: content_dialog_area.x + 2,
        y: content_dialog_area.y + 1,
        width: content_dialog_area.width.saturating_sub(4),
        height: content_dialog_area.height.saturating_sub(2),
    };

    // Build content based on device flow state
    let content = build_content_for_state(app);
    let paragraph = Paragraph::new(content)
        .alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(paragraph, inner);
}

/// Calculate dialog width based on terminal dimensions.
fn calculate_dialog_width(ctx: &LayoutContext, area_width: u16) -> u16 {
    if ctx.is_extra_small() {
        area_width.saturating_sub(4).min(50)
    } else if ctx.is_narrow() {
        ctx.bounded_width(80, 50, 70)
    } else {
        ctx.bounded_width(50, 60, 80)
    }
}

/// Calculate dialog height based on content and terminal dimensions.
fn calculate_dialog_height(ctx: &LayoutContext, app: &App, area_height: u16) -> u16 {
    // Base height: 2 (outer border) + 6 (logo) + 1 (spacing) + 2 (inner border) + content
    let base_content_lines = if let Some(ref device_flow) = app.device_flow {
        match device_flow.state() {
            DeviceFlowState::WaitingForUser { .. } => 8, // More lines for URI display
            _ => 4,
        }
    } else {
        4
    };

    let content_height = 2 + 6 + 1 + 2 + base_content_lines + 2; // +2 for padding

    let max_height = if ctx.is_extra_small() {
        area_height.saturating_sub(2)
    } else {
        area_height.saturating_sub(4)
    };

    content_height.min(max_height)
}

/// Render the SPOQ logo centered in the given area.
fn render_centered_logo(frame: &mut Frame, area: Rect) {
    let logo_lines: Vec<Line> = SPOQ_LOGO
        .iter()
        .map(|line| Line::from(Span::styled(*line, Style::default().fg(COLOR_HEADER))))
        .collect();

    let logo = Paragraph::new(logo_lines)
        .alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(logo, area);
}

/// Build content lines based on the current device flow state.
fn build_content_for_state(app: &App) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from("")]; // Top padding

    if let Some(ref device_flow) = app.device_flow {
        match device_flow.state() {
            DeviceFlowState::NotStarted => {
                lines.push(Line::from(""));
                lines.push(Line::from(vec![
                    Span::styled(
                        "Initializing...".to_string(),
                        Style::default().fg(COLOR_DIM),
                    ),
                ]));
            }

            DeviceFlowState::WaitingForUser {
                verification_uri,
                user_code,
                ..
            } => {
                lines.push(Line::from(""));
                lines.push(Line::from(vec![
                    Span::styled(
                        "Please visit:".to_string(),
                        Style::default().fg(COLOR_HEADER),
                    ),
                ]));
                lines.push(Line::from(""));
                lines.push(Line::from(vec![
                    Span::styled(
                        verification_uri.clone(),
                        Style::default()
                            .fg(Color::LightGreen)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
                lines.push(Line::from(""));
                lines.push(Line::from(vec![
                    Span::styled(
                        "Enter code: ".to_string(),
                        Style::default().fg(COLOR_HEADER),
                    ),
                    Span::styled(
                        user_code.clone(),
                        Style::default()
                            .fg(Color::LightGreen)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
                lines.push(Line::from(""));
                lines.push(Line::from(vec![
                    Span::styled(
                        "⣾ ".to_string(),
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::styled(
                        "Waiting for authorization...".to_string(),
                        Style::default().fg(COLOR_DIM),
                    ),
                ]));
            }

            DeviceFlowState::Authorized { .. } => {
                lines.push(Line::from(""));
                lines.push(Line::from(vec![
                    Span::styled(
                        "✓ Successfully signed in!".to_string(),
                        Style::default()
                            .fg(Color::LightGreen)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
            }

            DeviceFlowState::Denied => {
                lines.push(Line::from(""));
                lines.push(Line::from(vec![
                    Span::styled(
                        "✗ Authorization denied".to_string(),
                        Style::default()
                            .fg(Color::Red)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
                lines.push(Line::from(""));
                lines.push(Line::from(vec![
                    Span::styled(
                        "[Enter] Try again".to_string(),
                        Style::default().fg(COLOR_DIM),
                    ),
                ]));
            }

            DeviceFlowState::Expired => {
                lines.push(Line::from(""));
                lines.push(Line::from(vec![
                    Span::styled(
                        "✗ Authorization code expired".to_string(),
                        Style::default()
                            .fg(Color::Red)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
                lines.push(Line::from(""));
                lines.push(Line::from(vec![
                    Span::styled(
                        "[Enter] Try again".to_string(),
                        Style::default().fg(COLOR_DIM),
                    ),
                ]));
            }

            DeviceFlowState::Error(message) => {
                lines.push(Line::from(""));
                lines.push(Line::from(vec![
                    Span::styled(
                        "✗ Error:".to_string(),
                        Style::default()
                            .fg(Color::Red)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
                lines.push(Line::from(""));
                lines.push(Line::from(vec![
                    Span::styled(
                        message.clone(),
                        Style::default().fg(Color::Red),
                    ),
                ]));
                lines.push(Line::from(""));
                lines.push(Line::from(vec![
                    Span::styled(
                        "[Enter] Try again".to_string(),
                        Style::default().fg(COLOR_DIM),
                    ),
                ]));
            }
        }
    } else {
        // No device flow manager, show initializing
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(
                "Initializing...".to_string(),
                Style::default().fg(COLOR_DIM),
            ),
        ]));
    }

    lines.push(Line::from("")); // Bottom padding
    lines
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_dialog_width_extra_small() {
        let ctx = LayoutContext::new(50, 14);
        let width = calculate_dialog_width(&ctx, 50);
        assert!(width <= 50);
    }

    #[test]
    fn test_calculate_dialog_width_normal() {
        let ctx = LayoutContext::new(120, 40);
        let width = calculate_dialog_width(&ctx, 120);
        assert!(width >= 60);
        assert!(width <= 80);
    }

    #[test]
    fn test_calculate_dialog_height_not_started() {
        let ctx = LayoutContext::new(120, 40);
        let app = App::default();
        let height = calculate_dialog_height(&ctx, &app, 40);
        assert!(height > 0);
        assert!(height <= 40);
    }
}
