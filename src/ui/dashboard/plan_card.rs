//! Plan card overlay for plan approval preview.
//!
//! This module renders the plan approval overlay card displaying the PlanSummary
//! from WebSocket. It shows plan metadata, a scrollable phase list, and action buttons.
//!
//! ## Layout
//!
//! ```text
//! +------------------------------------------+
//! | {title} . {repo}                         |
//! |                                          |
//! | plan ready . N phases . M files . ~Xk tokens |
//! |                                          |
//! | 1. Phase description                   ^ |
//! | 2. Another phase                         |
//! | 3. Yet another phase                     |
//! | 4. Fourth phase                          |
//! | 5. Fifth phase                         v |
//! |                                          |
//! | [view full]            [reject] [approve]|
//! +------------------------------------------+
//! ```

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    Frame,
};

use crate::models::dashboard::PlanSummary;
use crate::ui::interaction::{ClickAction, HitAreaRegistry};

/// Maximum number of phases visible without scrolling.
const MAX_VISIBLE_PHASES: usize = 5;

/// Render the plan card content inside the overlay border.
///
/// This function renders:
/// - Header with title and repository
/// - Summary line with phase count, file count, and token estimate
/// - Scrollable phase list with up/down indicators
/// - Action buttons: [view full], [reject], [approve]
///
/// # Arguments
///
/// * `frame` - The ratatui frame to render to
/// * `area` - Inner card area (inside the border)
/// * `thread_id` - ID of the thread for click actions
/// * `title` - Thread/plan title
/// * `repo` - Repository name
/// * `request_id` - Request ID for plan approval
/// * `summary` - PlanSummary containing phases and metadata
/// * `scroll_offset` - Current scroll position in phase list
/// * `registry` - Hit area registry for click handling
#[allow(clippy::too_many_arguments)]
pub fn render(
    frame: &mut Frame,
    area: Rect,
    thread_id: &str,
    title: &str,
    repo: &str,
    _request_id: &str,
    summary: &PlanSummary,
    scroll_offset: usize,
    registry: &mut HitAreaRegistry,
) {
    // Guard against zero-height areas
    if area.height < 3 {
        return;
    }

    let mut y = area.y;

    // Row 0: Header - "{title} . {repo}"
    let header = format!("{} \u{00b7} {}", title, repo);
    let header_truncated = truncate_string(&header, area.width as usize);
    frame.render_widget(
        Line::styled(
            header_truncated,
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Rect::new(area.x, y, area.width, 1),
    );
    y += 1;

    // Row 1: blank
    y += 1;

    // Row 2: Summary line
    let tokens_k = summary.estimated_tokens / 1000;
    let summary_line = format!(
        "plan ready \u{00b7} {} phases \u{00b7} {} files \u{00b7} ~{}k tokens",
        summary.phases.len(),
        summary.file_count,
        tokens_k
    );
    let summary_truncated = truncate_string(&summary_line, area.width as usize);
    frame.render_widget(
        Line::raw(summary_truncated),
        Rect::new(area.x, y, area.width, 1),
    );
    y += 1;

    // Row 3: blank
    y += 1;

    // Rows 4-8: Phase list with scroll indicators
    let phases_to_show: Vec<(usize, &String)> = summary
        .phases
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(MAX_VISIBLE_PHASES)
        .collect();

    // Show "^" indicator if scrolled down (more content above)
    if scroll_offset > 0 && area.width >= 2 {
        frame.render_widget(
            Span::styled("^", Style::default().fg(Color::DarkGray)),
            Rect::new(area.x + area.width - 2, y.saturating_sub(1), 1, 1),
        );
    }

    let phase_start_y = y;
    for (i, phase) in &phases_to_show {
        let phase_num = i + 1;
        let phase_text = format!("{}. {}", phase_num, phase);
        let truncated = truncate_string(&phase_text, area.width as usize - 2);
        frame.render_widget(Line::raw(truncated), Rect::new(area.x, y, area.width, 1));
        y += 1;
    }

    // Show "v" indicator if more phases below
    let has_more = scroll_offset + MAX_VISIBLE_PHASES < summary.phases.len();
    if has_more && y > phase_start_y && area.width >= 2 {
        frame.render_widget(
            Span::styled("v", Style::default().fg(Color::DarkGray)),
            Rect::new(area.x + area.width - 2, y.saturating_sub(1), 1, 1),
        );
    }

    // Pad to consistent height before buttons
    let button_row_y = area.y + area.height.saturating_sub(1);

    // Skip if button row would overlap with content
    if button_row_y <= y {
        return;
    }

    // Row 10: Buttons row - [view full]            [reject] [approve]
    let view_btn = "[view full]";
    let reject_btn = "[reject]";
    let approve_btn = "[approve]";

    // [view full] on the left
    let view_len = view_btn.len() as u16;
    let view_area = Rect::new(area.x, button_row_y, view_len, 1);
    frame.render_widget(Span::raw(view_btn), view_area);
    registry.register(
        view_area,
        ClickAction::ViewFullPlan(thread_id.to_string()),
        Some(Style::default().bg(Color::DarkGray)),
    );

    // [approve] on the right
    let approve_len = approve_btn.len() as u16;
    let approve_x = area.x + area.width.saturating_sub(approve_len);
    let approve_area = Rect::new(approve_x, button_row_y, approve_len, 1);
    frame.render_widget(
        Span::styled(approve_btn, Style::default().fg(Color::Green)),
        approve_area,
    );
    registry.register(
        approve_area,
        ClickAction::ApproveThread(thread_id.to_string()),
        Some(Style::default().bg(Color::DarkGray)),
    );

    // [reject] to the left of [approve] with 2-char gap
    let reject_len = reject_btn.len() as u16;
    let reject_x = approve_x.saturating_sub(reject_len + 2);
    let reject_area = Rect::new(reject_x, button_row_y, reject_len, 1);
    frame.render_widget(
        Span::styled(reject_btn, Style::default().fg(Color::Red)),
        reject_area,
    );
    registry.register(
        reject_area,
        ClickAction::RejectThread(thread_id.to_string()),
        Some(Style::default().bg(Color::DarkGray)),
    );
}

/// Truncate a string to fit within a given width, adding "..." if truncated.
fn truncate_string(s: &str, max_width: usize) -> String {
    if s.len() <= max_width {
        s.to_string()
    } else if max_width > 3 {
        format!("{}...", &s[..max_width - 3])
    } else {
        s[..max_width].to_string()
    }
}

/// Calculate the number of visible phases based on available height.
///
/// # Arguments
///
/// * `available_height` - Total height available for phase list
///
/// # Returns
///
/// Number of phases that can be displayed.
pub fn calculate_visible_phases(available_height: u16) -> usize {
    available_height as usize
}

/// Calculate the maximum valid scroll offset for a given phase count.
///
/// # Arguments
///
/// * `total_phases` - Total number of phases
///
/// # Returns
///
/// Maximum scroll offset (0 if all phases fit on screen).
pub fn max_scroll_offset(total_phases: usize) -> usize {
    total_phases.saturating_sub(MAX_VISIBLE_PHASES)
}

/// Clamp scroll offset to valid range.
///
/// # Arguments
///
/// * `offset` - Desired scroll offset
/// * `total_phases` - Total number of phases
///
/// # Returns
///
/// Clamped scroll offset within valid range.
pub fn clamp_scroll_offset(offset: usize, total_phases: usize) -> usize {
    offset.min(max_scroll_offset(total_phases))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------- truncate_string Tests --------------------

    #[test]
    fn test_truncate_string_no_truncation() {
        assert_eq!(truncate_string("hello", 10), "hello");
        assert_eq!(truncate_string("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_string_with_truncation() {
        assert_eq!(truncate_string("hello world", 8), "hello...");
        assert_eq!(truncate_string("hello world", 7), "hell...");
    }

    #[test]
    fn test_truncate_string_very_short_max() {
        assert_eq!(truncate_string("hello", 3), "hel");
        assert_eq!(truncate_string("hello", 2), "he");
    }

    #[test]
    fn test_truncate_string_empty() {
        assert_eq!(truncate_string("", 10), "");
    }

    // -------------------- max_scroll_offset Tests --------------------

    #[test]
    fn test_max_scroll_offset_fewer_than_visible() {
        // With 3 phases and 5 visible, no scrolling needed
        assert_eq!(max_scroll_offset(3), 0);
        assert_eq!(max_scroll_offset(0), 0);
        assert_eq!(max_scroll_offset(5), 0);
    }

    #[test]
    fn test_max_scroll_offset_more_than_visible() {
        // With 8 phases and 5 visible, can scroll 3 positions
        assert_eq!(max_scroll_offset(8), 3);
        assert_eq!(max_scroll_offset(10), 5);
        assert_eq!(max_scroll_offset(6), 1);
    }

    // -------------------- clamp_scroll_offset Tests --------------------

    #[test]
    fn test_clamp_scroll_offset_within_range() {
        // 8 phases, max offset is 3
        assert_eq!(clamp_scroll_offset(0, 8), 0);
        assert_eq!(clamp_scroll_offset(2, 8), 2);
        assert_eq!(clamp_scroll_offset(3, 8), 3);
    }

    #[test]
    fn test_clamp_scroll_offset_exceeds_max() {
        // 8 phases, max offset is 3
        assert_eq!(clamp_scroll_offset(5, 8), 3);
        assert_eq!(clamp_scroll_offset(100, 8), 3);
    }

    #[test]
    fn test_clamp_scroll_offset_no_scroll_needed() {
        // 3 phases, max offset is 0
        assert_eq!(clamp_scroll_offset(0, 3), 0);
        assert_eq!(clamp_scroll_offset(5, 3), 0);
    }

    // -------------------- calculate_visible_phases Tests --------------------

    #[test]
    fn test_calculate_visible_phases() {
        assert_eq!(calculate_visible_phases(10), 10);
        assert_eq!(calculate_visible_phases(5), 5);
        assert_eq!(calculate_visible_phases(0), 0);
    }

    // -------------------- Scroll Display Logic Tests --------------------

    #[test]
    fn test_scroll_display_cases() {
        // Scenario 1: 10 phases, scroll_offset = 0
        // Should show phases 1-5, no up indicator, down indicator visible
        let total_phases = 10;
        let scroll_offset = 0;
        let visible_phases: Vec<usize> = (0..total_phases)
            .skip(scroll_offset)
            .take(MAX_VISIBLE_PHASES)
            .collect();

        assert_eq!(visible_phases, vec![0, 1, 2, 3, 4]);
        assert!(!should_show_up_indicator(scroll_offset));
        assert!(should_show_down_indicator(scroll_offset, total_phases));

        // Scenario 2: 10 phases, scroll_offset = 3
        // Should show phases 4-8, both indicators visible
        let scroll_offset = 3;
        let visible_phases: Vec<usize> = (0..total_phases)
            .skip(scroll_offset)
            .take(MAX_VISIBLE_PHASES)
            .collect();

        assert_eq!(visible_phases, vec![3, 4, 5, 6, 7]);
        assert!(should_show_up_indicator(scroll_offset));
        assert!(should_show_down_indicator(scroll_offset, total_phases));

        // Scenario 3: 10 phases, scroll_offset = 5 (max)
        // Should show phases 6-10, up indicator visible, no down indicator
        let scroll_offset = 5;
        let visible_phases: Vec<usize> = (0..total_phases)
            .skip(scroll_offset)
            .take(MAX_VISIBLE_PHASES)
            .collect();

        assert_eq!(visible_phases, vec![5, 6, 7, 8, 9]);
        assert!(should_show_up_indicator(scroll_offset));
        assert!(!should_show_down_indicator(scroll_offset, total_phases));
    }

    #[test]
    fn test_scroll_display_all_fit() {
        // 3 phases, all should be visible, no indicators
        let total_phases = 3;
        let scroll_offset = 0;
        let visible_phases: Vec<usize> = (0..total_phases)
            .skip(scroll_offset)
            .take(MAX_VISIBLE_PHASES)
            .collect();

        assert_eq!(visible_phases, vec![0, 1, 2]);
        assert!(!should_show_up_indicator(scroll_offset));
        assert!(!should_show_down_indicator(scroll_offset, total_phases));
    }

    #[test]
    fn test_scroll_display_exactly_max() {
        // Exactly 5 phases, all fit, no indicators
        let total_phases = 5;
        let scroll_offset = 0;
        let visible_phases: Vec<usize> = (0..total_phases)
            .skip(scroll_offset)
            .take(MAX_VISIBLE_PHASES)
            .collect();

        assert_eq!(visible_phases, vec![0, 1, 2, 3, 4]);
        assert!(!should_show_up_indicator(scroll_offset));
        assert!(!should_show_down_indicator(scroll_offset, total_phases));
    }

    // Helper functions for tests (match render logic)
    fn should_show_up_indicator(scroll_offset: usize) -> bool {
        scroll_offset > 0
    }

    fn should_show_down_indicator(scroll_offset: usize, total_phases: usize) -> bool {
        scroll_offset + MAX_VISIBLE_PHASES < total_phases
    }
}
