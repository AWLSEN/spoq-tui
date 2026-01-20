//! Dashboard UI module
//!
//! Provides the multi-thread dashboard view components for managing
//! multiple concurrent agent threads.

mod context;
pub mod footer;
pub mod header;
pub mod overlay;
pub mod plan_card;
pub mod question_card;
pub mod states;
pub mod status_bar;
pub mod thread_list;
pub mod thread_row;

pub use context::{
    FilterState, OverlayState, Progress, RenderContext, SystemStats, Theme, ThreadMode, ThreadView,
};

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::Line,
    Frame,
};

use crate::ui::interaction::HitAreaRegistry;

// ============================================================================
// Main Dashboard Rendering
// ============================================================================

/// Render the complete dashboard view
///
/// This is the top-level function that composes all dashboard components:
/// - Header: System stats (CPU, RAM), SPOQ logo, aggregate counts (threads, repos)
/// - Status bar: Proportional bar showing working/ready/idle distribution
/// - Thread list: Threads split by need-action vs autonomous
/// - Overlay: Question/Plan dialogs when a thread is expanded
///
/// # Layout
/// ```text
/// +------------------------------------------+
/// | HEADER: cpu ▓▓░░  4/8g   SPOQ   n threads|
/// +------------------------------------------+
/// | STATUS BAR: ████████████░░░░░░░░░░░░░░░  |
/// |             working 24  ready 8   idle 15|
/// +------------------------------------------+
/// | THREAD LIST:                             |
/// |   ⚠ Thread needing action 1             |
/// |   ⚠ Thread needing action 2             |
/// |   ────────                               |
/// |   🔄 Running thread 1                    |
/// |   ✓ Done thread 2                        |
/// +------------------------------------------+
/// | [OVERLAY if expanded]                    |
/// +------------------------------------------+
/// ```
///
/// # Arguments
/// * `frame` - The ratatui frame to render into
/// * `area` - The rectangle area allocated for the dashboard
/// * `ctx` - The render context containing all data for rendering
/// * `registry` - Hit area registry for click handling
pub fn render_dashboard(
    frame: &mut Frame,
    area: Rect,
    ctx: &RenderContext,
    registry: &mut HitAreaRegistry,
) {
    // Minimum dimensions check
    if area.width < 40 || area.height < 10 {
        states::render_heavy_load(frame, area);
        return;
    }

    // Layout: header (3 rows) + margin + status bar (2 rows) + thread list (remaining) + footer (1 row)
    // Margin between header and status bar is ~8% of remaining height (min 1 row)
    let margin_rows = ((area.height as f32 * 0.08).round() as u16).max(1);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),          // Header (3 rows for better vertical alignment)
            Constraint::Length(margin_rows), // Margin between header and status bar
            Constraint::Length(2),          // Status bar
            Constraint::Min(5),             // Thread list
            Constraint::Length(1),          // Footer hint
        ])
        .split(area);

    // Render header (system stats, logo, counts)
    header::render(frame, chunks[0], ctx);

    // chunks[1] is the margin - intentionally left empty

    // Render status bar (proportional segments with filters)
    status_bar::render(frame, chunks[2], ctx, registry);

    // Render thread list (split or filtered view)
    thread_list::render(frame, chunks[3], ctx, registry);

    // Render footer hint
    let hint = footer::get_footer_hint(ctx);
    let hint_line = Line::styled(hint, Style::default().fg(Color::DarkGray));
    frame.render_widget(hint_line, chunks[4]);

    // Render overlay if present
    if let Some(overlay_state) = ctx.overlay {
        // Calculate overlay position based on anchor_y or center
        let overlay_area = calculate_overlay_area(area, overlay_state);
        overlay::render(frame, overlay_area, overlay_state, ctx, registry);
    }
}

/// Calculate the overlay area based on the overlay state's anchor position
fn calculate_overlay_area(parent_area: Rect, overlay: &OverlayState) -> Rect {
    // Overlay is centered horizontally, positioned near anchor_y vertically
    let overlay_width = (parent_area.width * 80 / 100).min(60);
    let overlay_height = match overlay {
        OverlayState::Question { options, .. } => {
            // Height based on question content: title + question + options + buttons
            (4 + options.len() as u16 + 3).min(parent_area.height - 4)
        }
        OverlayState::FreeForm { .. } => {
            // Fixed height for free-form input
            10.min(parent_area.height - 4)
        }
        OverlayState::Plan { summary, .. } => {
            // Height based on plan content: title + phases + stats
            (4 + summary.phases.len() as u16 + 4).min(parent_area.height - 4)
        }
    };

    let x = parent_area.x + (parent_area.width.saturating_sub(overlay_width)) / 2;

    // Position near anchor_y, but ensure it fits in the parent area
    let anchor_y = match overlay {
        OverlayState::Question { anchor_y, .. } => *anchor_y,
        OverlayState::FreeForm { anchor_y, .. } => *anchor_y,
        OverlayState::Plan { anchor_y, .. } => *anchor_y,
    };

    // Try to position overlay so anchor_y is near the top
    let y = anchor_y
        .max(parent_area.y + 2)
        .min(parent_area.y + parent_area.height - overlay_height - 2);

    Rect::new(x, y, overlay_width, overlay_height)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_overlay_area_question() {
        let parent = Rect::new(0, 0, 100, 40);
        let overlay = OverlayState::Question {
            thread_id: "t1".to_string(),
            thread_title: "Test".to_string(),
            repository: "~/repo".to_string(),
            question: "Continue?".to_string(),
            options: vec!["Yes".to_string(), "No".to_string()],
            anchor_y: 10,
        };

        let area = calculate_overlay_area(parent, &overlay);

        // Width should be 80% of parent or max 60
        assert!(area.width <= 60);
        assert!(area.width >= 60); // 80% of 100 = 80, capped at 60

        // Should be within parent bounds
        assert!(area.x >= parent.x);
        assert!(area.y >= parent.y);
        assert!(area.x + area.width <= parent.x + parent.width);
        assert!(area.y + area.height <= parent.y + parent.height);
    }

    #[test]
    fn test_calculate_overlay_area_plan() {
        let parent = Rect::new(0, 0, 80, 30);
        let summary = crate::models::dashboard::PlanSummary::new(
            "Test Plan".to_string(),
            vec!["Phase 1".to_string(), "Phase 2".to_string()],
            5,
            10000,
        );
        let overlay = OverlayState::Plan {
            thread_id: "t1".to_string(),
            thread_title: "Test".to_string(),
            repository: "~/repo".to_string(),
            request_id: "req-1".to_string(),
            summary,
            scroll_offset: 0,
            anchor_y: 15,
        };

        let area = calculate_overlay_area(parent, &overlay);

        // Should be within parent bounds
        assert!(area.y + area.height <= parent.y + parent.height);
    }

    #[test]
    fn test_calculate_overlay_area_free_form() {
        let parent = Rect::new(5, 5, 70, 25);
        let overlay = OverlayState::FreeForm {
            thread_id: "t1".to_string(),
            thread_title: "Test".to_string(),
            repository: "~/repo".to_string(),
            question: "Enter details".to_string(),
            input: String::new(),
            cursor_pos: 0,
            anchor_y: 8,
        };

        let area = calculate_overlay_area(parent, &overlay);

        // Height should be 10 for free-form
        assert_eq!(area.height, 10);

        // Should be centered horizontally
        let expected_x = parent.x + (parent.width - area.width) / 2;
        assert_eq!(area.x, expected_x);
    }
}
