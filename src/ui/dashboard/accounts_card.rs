//! Claude accounts management card rendering.
//!
//! Renders the Claude accounts overlay showing:
//! - List of configured accounts with status indicators
//! - Keyboard hints for add/remove/close

use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::view_state::dashboard_view::ClaudeAccountInfo;

/// Render the Claude accounts card content.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    accounts: &[ClaudeAccountInfo],
    selected_index: usize,
    adding: bool,
    status_message: Option<&str>,
) {
    let mut lines: Vec<Line> = Vec::new();

    // Title
    lines.push(Line::from(vec![
        Span::styled(
            " Claude Accounts ",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(""));

    if accounts.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No accounts configured",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for (i, account) in accounts.iter().enumerate() {
            let is_selected = i == selected_index;
            let pointer = if is_selected { " > " } else { "   " };

            let status_indicator = match account.status.as_str() {
                "active" => Span::styled("●", Style::default().fg(Color::Green)),
                "rate_limited" => Span::styled("◌", Style::default().fg(Color::Yellow)),
                "error" => Span::styled("✗", Style::default().fg(Color::Red)),
                "disabled" => Span::styled("○", Style::default().fg(Color::DarkGray)),
                _ => Span::styled("?", Style::default().fg(Color::DarkGray)),
            };

            let label_style = if is_selected {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };

            let status_text = match account.status.as_str() {
                "active" => "Active",
                "rate_limited" => "Rate limited",
                "error" => "Error",
                "disabled" => "Disabled",
                s => s,
            };

            // Use email as display name if available, otherwise label
            let display_name = account.email.as_deref().unwrap_or(&account.label);

            let spans = vec![
                Span::raw(pointer),
                Span::styled(
                    format!("{}. ", i + 1),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(display_name, label_style),
                Span::raw("  "),
                status_indicator,
                Span::styled(
                    format!(" {}", status_text),
                    Style::default().fg(Color::DarkGray),
                ),
            ];

            lines.push(Line::from(spans));
        }
    }

    lines.push(Line::from(""));

    // Status message (e.g., "Authenticating...", "Account added!", error)
    if let Some(msg) = status_message {
        let color = if msg.starts_with("Failed") || msg.starts_with("Auth failed") || msg.contains("already added") {
            Color::Red
        } else if msg.contains("successfully") {
            Color::Green
        } else {
            Color::Yellow
        };
        lines.push(Line::from(Span::styled(
            format!("  {}", msg),
            Style::default().fg(color),
        )));
        lines.push(Line::from(""));
    }

    // Help text - dim [A] when adding is in progress
    let add_key_color = if adding { Color::DarkGray } else { Color::Cyan };
    let add_label_color = if adding { Color::DarkGray } else { Color::Gray };
    lines.push(Line::from(vec![
        Span::styled(" [A] ", Style::default().fg(add_key_color)),
        Span::styled(if adding { "Adding..." } else { "Add" }, Style::default().fg(add_label_color)),
        Span::raw("  "),
        Span::styled(" [R] ", Style::default().fg(Color::Cyan)),
        Span::styled("Remove", Style::default().fg(Color::Gray)),
        Span::raw("  "),
        Span::styled(" [Esc] ", Style::default().fg(Color::Cyan)),
        Span::styled("Close", Style::default().fg(Color::Gray)),
    ]));

    let paragraph = Paragraph::new(lines).alignment(Alignment::Left);
    frame.render_widget(paragraph, area);
}

/// Calculate the height needed for the accounts card.
pub fn calculate_height(account_count: usize, has_status: bool) -> u16 {
    // title(1) + blank(1) + accounts(max(1,n)) + blank(1) + [status(1) + blank(1)] + help(1)
    let rows = account_count.max(1) as u16;
    let status_rows = if has_status { 2 } else { 0 };
    4 + rows + status_rows
}
