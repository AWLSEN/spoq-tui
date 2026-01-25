//! Special state rendering for dashboard
//!
//! Provides helper functions for rendering special states like "all clear" and "heavy load".

use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Text},
    widgets::Paragraph,
    Frame,
};

/// Renders GitHub repos list when no threads exist
/// Displays top 10 recent repos + quick tip about @ tagging
pub fn render_all_clear(frame: &mut Frame, area: Rect, repos: &[crate::models::GitHubRepo]) {
    use ratatui::text::Span;

    let mut lines = vec![];

    // Header
    lines.push(Line::styled(
        "Recent GitHub Repositories",
        Style::default()
            .add_modifier(Modifier::BOLD)
            .fg(Color::Cyan),
    ));
    lines.push(Line::raw(""));

    // Repos list (top 10)
    if repos.is_empty() {
        lines.push(Line::styled(
            "Loading repositories...",
            Style::default().fg(Color::Gray),
        ));
    } else {
        for (i, repo) in repos.iter().take(10).enumerate() {
            let lang = repo.primary_language
                .as_ref()
                .map(|l| l.name.as_str())
                .unwrap_or("N/A");

            let line = Line::from(vec![
                Span::styled(
                    format!("{}. ", i + 1),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    &repo.name_with_owner,
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(
                    format!("({})", lang),
                    Style::default().fg(Color::Yellow),
                ),
            ]);
            lines.push(line);
        }
    }

    // Quick tip section
    lines.push(Line::raw(""));
    lines.push(Line::styled(
        "Quick Tip:",
        Style::default()
            .add_modifier(Modifier::BOLD)
            .fg(Color::Green),
    ));
    lines.push(Line::raw("Use @ to tag:"));
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled("@repo-name", Style::default().fg(Color::Cyan)),
        Span::raw(" - Start thread in a repository"),
    ]));
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled("@folder", Style::default().fg(Color::Cyan)),
        Span::raw(" - Start thread in a folder"),
    ]));
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled("@thread", Style::default().fg(Color::Cyan)),
        Span::raw(" - Resume a thread"),
    ]));
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled("@file.rs", Style::default().fg(Color::Cyan)),
        Span::raw(" - Tag a file"),
    ]));

    let text = Text::from(lines);
    // Calculate content height before moving text into Paragraph
    let content_height = text.lines.len() as u16;
    let paragraph = Paragraph::new(text).alignment(Alignment::Left);

    // Center vertically
    let y_offset = area.height.saturating_sub(content_height) / 2;
    let centered_area = Rect::new(area.x + 2, area.y + y_offset, area.width.saturating_sub(4), content_height);

    frame.render_widget(paragraph, centered_area);
}

/// Renders "heavy load" warning when system is under heavy load
/// Displayed at top of status bar area
pub fn render_heavy_load(frame: &mut Frame, area: Rect) {
    // Render at specified area:
    //   "⚠ heavy load"

    let warning = Line::styled(
        "⚠ heavy load",
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );

    frame.render_widget(warning, area);
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn test_heavy_load_text_content() {
        // Test heavy load warning content
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                let area = Rect::new(0, 0, 80, 1);
                render_heavy_load(frame, area);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let line: String = (0..buffer.area.width)
            .map(|x| buffer[(x, 0)].symbol())
            .collect();

        assert!(
            line.contains("⚠ heavy load"),
            "Expected buffer to contain '⚠ heavy load', got: {}",
            line
        );
    }
}
