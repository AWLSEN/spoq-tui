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
    paste_mode: bool,
    paste_buffer: &str,
    auth_url: Option<&str>,
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
            let is_current = account.priority == 0; // Priority 0 = current/primary account
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

            let mut spans = vec![
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

            // Show "current" indicator for primary account (priority 0)
            if is_current {
                spans.push(Span::styled(
                    " ← current",
                    Style::default().fg(Color::Cyan),
                ));
            }

            lines.push(Line::from(spans));
        }
    }

    lines.push(Line::from(""));

    // Paste-token input field
    if paste_mode {
        lines.push(Line::from(vec![
            Span::styled("  Token: ", Style::default().fg(Color::Cyan)),
            Span::styled(
                if paste_buffer.is_empty() {
                    "paste or type token here..."
                } else {
                    paste_buffer
                },
                Style::default().fg(if paste_buffer.is_empty() { Color::DarkGray } else { Color::White }),
            ),
            Span::styled("_", Style::default().fg(Color::White)),
        ]));
        lines.push(Line::from(vec![
            Span::styled(
                "  [Enter] Submit  [Esc] Cancel",
                Style::default().fg(Color::DarkGray),
            ),
        ]));
        lines.push(Line::from(""));
    }

    // Status message (e.g., "Authenticating...", "Account added!", error)
    if let Some(msg) = status_message {
        let color = if msg.starts_with("Failed") || msg.starts_with("Auth failed") || msg.starts_with("Invalid") || msg.contains("already added") {
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

    // Auth URL (shown when setup-token surfaces an OAuth URL)
    if let Some(url) = auth_url {
        lines.push(Line::from(vec![
            Span::styled("  URL: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled(url, Style::default().fg(Color::White)),
        ]));
        lines.push(Line::from(Span::styled(
            "  Open this URL to authenticate",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(""));
    }

    // Help text - dim [A]/[T] when adding or paste mode is active
    let disabled = adding || paste_mode;
    let add_key_color = if disabled { Color::DarkGray } else { Color::Cyan };
    let add_label_color = if disabled { Color::DarkGray } else { Color::Gray };
    lines.push(Line::from(vec![
        Span::styled(" [Enter/1-9] ", Style::default().fg(if paste_mode { Color::DarkGray } else { Color::Cyan })),
        Span::styled("Select", Style::default().fg(if paste_mode { Color::DarkGray } else { Color::Gray })),
        Span::raw("  "),
        Span::styled(" [A] ", Style::default().fg(add_key_color)),
        Span::styled(if adding { "Adding..." } else { "Add" }, Style::default().fg(add_label_color)),
        Span::raw("  "),
        Span::styled(" [T] ", Style::default().fg(add_key_color)),
        Span::styled(if paste_mode { "Pasting..." } else { "Paste" }, Style::default().fg(add_label_color)),
        Span::raw("  "),
        Span::styled(" [R] ", Style::default().fg(if disabled { Color::DarkGray } else { Color::Cyan })),
        Span::styled("Remove", Style::default().fg(if disabled { Color::DarkGray } else { Color::Gray })),
        Span::raw("  "),
        Span::styled(" [Esc] ", Style::default().fg(Color::Cyan)),
        Span::styled(if paste_mode { "Cancel" } else { "Close" }, Style::default().fg(Color::Gray)),
    ]));

    let paragraph = Paragraph::new(lines).alignment(Alignment::Left);
    frame.render_widget(paragraph, area);
}

/// Calculate the height needed for the accounts card.
pub fn calculate_height(account_count: usize, has_status: bool, paste_mode: bool, has_auth_url: bool) -> u16 {
    // title(1) + blank(1) + accounts(max(1,n)) + blank(1) + [paste(2) + blank(1)] + [status(1) + blank(1)] + [url(2) + blank(1)] + help(1)
    let rows = account_count.max(1) as u16;
    let status_rows = if has_status { 2 } else { 0 };
    let paste_rows = if paste_mode { 3 } else { 0 };
    let url_rows = if has_auth_url { 3 } else { 0 };
    4 + rows + status_rows + paste_rows + url_rows
}
