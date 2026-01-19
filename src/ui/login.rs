use ratatui::{
    prelude::*,
    widgets::{Block, Borders, BorderType, Paragraph},
};
use crate::app::App;
use crate::auth::DeviceFlowState;
use super::theme::{COLOR_BORDER, COLOR_HEADER};
use super::helpers::SPINNER_FRAMES;
use super::command_deck::SPOQ_LOGO;

pub fn render_login_screen(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // Outer block with double border
    let outer_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(COLOR_BORDER));
    frame.render_widget(outer_block, area);

    let inner = area.inner(Margin::new(2, 1));

    // Render SPOQ logo at top
    let logo_area = Rect::new(inner.x, inner.y, inner.width, 6);
    let logo = Paragraph::new(SPOQ_LOGO.join("\n"))
        .style(Style::default().fg(COLOR_HEADER))
        .alignment(Alignment::Center);
    frame.render_widget(logo, logo_area);

    // Dialog content based on device flow state
    let dialog_area = Rect::new(inner.x + 4, inner.y + 8, inner.width.saturating_sub(8), 10);
    let dialog_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(COLOR_BORDER));

    let content = if let Some(ref device_flow) = app.device_flow {
        match device_flow.state() {
            DeviceFlowState::NotStarted => "Initializing...".to_string(),
            DeviceFlowState::WaitingForUser { verification_uri, user_code, .. } => {
                format!("Sign in to continue\n\nVisit: {}\nCode: {}\n\n{} Waiting for authorization...",
                    verification_uri, user_code, SPINNER_FRAMES[0])
            }
            DeviceFlowState::Authorized { .. } => "✓ Successfully signed in!".to_string(),
            DeviceFlowState::Denied => "✗ Authorization denied\n\n[Enter] Try again  [Q] Quit".to_string(),
            DeviceFlowState::Expired => "✗ Code expired\n\n[Enter] Try again  [Q] Quit".to_string(),
            DeviceFlowState::Error(e) => format!("✗ Error: {}\n\n[Enter] Try again  [Q] Quit", e),
        }
    } else {
        "Initializing...".to_string()
    };

    let para = Paragraph::new(content)
        .block(dialog_block)
        .alignment(Alignment::Center)
        .wrap(ratatui::widgets::Wrap { trim: true });
    frame.render_widget(para, dialog_area);
}
