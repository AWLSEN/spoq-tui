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

/// Renders "all clear" message when no threads need user action
/// Displayed centered in the given area
pub fn render_all_clear(frame: &mut Frame, area: Rect, autonomous_count: usize) {
    // Centered in area:
    //   "all clear"
    //   ""
    //   "nothing needs your attention"
    //   "{n} threads working autonomously"

    let text = Text::from(vec![
        Line::styled(
            "all clear",
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Green),
        ),
        Line::raw(""),
        Line::raw("nothing needs your attention"),
        Line::raw(format!("{} threads working autonomously", autonomous_count)),
    ]);

    let paragraph = Paragraph::new(text).alignment(Alignment::Center);

    // Center vertically in the area
    let text_height = 4;
    let y_offset = area.height.saturating_sub(text_height) / 2;
    let centered_area = Rect::new(area.x, area.y + y_offset, area.width, text_height);

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
    fn test_all_clear_formatting() {
        // Test that the autonomous_count is properly formatted
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                let area = Rect::new(0, 0, 80, 24);
                render_all_clear(frame, area, 5);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();

        // Search for the autonomous count line
        let mut found_count_line = false;
        for y in 0..buffer.area.height {
            let line: String = (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect();

            if line.contains("5 threads working autonomously") {
                found_count_line = true;
                break;
            }
        }

        assert!(
            found_count_line,
            "Expected to find '5 threads working autonomously' in buffer"
        );
    }

    #[test]
    fn test_all_clear_zero_autonomous() {
        // Test with zero autonomous threads
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                let area = Rect::new(0, 0, 80, 24);
                render_all_clear(frame, area, 0);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();

        let mut found_zero_line = false;
        for y in 0..buffer.area.height {
            let line: String = (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect();

            if line.contains("0 threads working autonomously") {
                found_zero_line = true;
                break;
            }
        }

        assert!(
            found_zero_line,
            "Expected to find '0 threads working autonomously' in buffer"
        );
    }

    #[test]
    fn test_all_clear_large_count() {
        // Test with a large autonomous count
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                let area = Rect::new(0, 0, 80, 24);
                render_all_clear(frame, area, 100);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();

        let mut found_large_count = false;
        for y in 0..buffer.area.height {
            let line: String = (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect();

            if line.contains("100 threads working autonomously") {
                found_large_count = true;
                break;
            }
        }

        assert!(
            found_large_count,
            "Expected to find '100 threads working autonomously' in buffer"
        );
    }

    #[test]
    fn test_all_clear_text_content() {
        // Test all text content is present
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                let area = Rect::new(0, 0, 80, 24);
                render_all_clear(frame, area, 3);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let full_text: String = (0..buffer.area.height)
            .flat_map(|y| (0..buffer.area.width).map(move |x| buffer[(x, y)].symbol().to_string()))
            .collect();

        assert!(
            full_text.contains("all clear"),
            "Expected buffer to contain 'all clear'"
        );
        assert!(
            full_text.contains("nothing needs your attention"),
            "Expected buffer to contain 'nothing needs your attention'"
        );
        assert!(
            full_text.contains("3 threads working autonomously"),
            "Expected buffer to contain '3 threads working autonomously'"
        );
    }

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
