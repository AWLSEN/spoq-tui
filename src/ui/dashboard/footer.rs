//! Footer hints for dashboard context-awareness
//!
//! Provides dynamic footer hints based on UI state (overlay, etc.)

use crate::ui::dashboard::RenderContext;

/// Get context-aware footer hint text based on UI state
///
/// Returns appropriate hint text depending on whether:
/// - An overlay is open (show "esc close")
/// - Default state (empty)
///
/// # Arguments
/// * `ctx` - The render context containing overlay state
///
/// # Returns
/// Static string reference with the appropriate hint text
pub fn get_footer_hint(ctx: &RenderContext) -> &'static str {
    if ctx.overlay.is_some() {
        "esc close"
    } else {
        ""
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::dashboard::Aggregate;
    use crate::ui::dashboard::{OverlayState, SystemStats, Theme};

    #[test]
    fn test_footer_hint_default_state() {
        let threads = vec![];
        let aggregate = Aggregate::new();
        let stats = SystemStats::default();
        let theme = Theme::default();

        let ctx = RenderContext::new(&threads, &aggregate, &stats, &theme);

        assert_eq!(get_footer_hint(&ctx), "");
    }

    #[test]
    fn test_footer_hint_with_overlay_question() {
        let threads = vec![];
        let aggregate = Aggregate::new();
        let stats = SystemStats::default();
        let theme = Theme::default();

        let overlay = OverlayState::Question {
            thread_id: "t1".to_string(),
            thread_title: "Test".to_string(),
            repository: "~/repo".to_string(),
            question_data: None,
            anchor_y: 10,
        };

        let ctx =
            RenderContext::new(&threads, &aggregate, &stats, &theme).with_overlay(Some(&overlay));

        assert_eq!(get_footer_hint(&ctx), "esc close");
    }

    #[test]
    fn test_footer_hint_with_overlay_freeform() {
        let threads = vec![];
        let aggregate = Aggregate::new();
        let stats = SystemStats::default();
        let theme = Theme::default();

        let overlay = OverlayState::FreeForm {
            thread_id: "t1".to_string(),
            thread_title: "Test".to_string(),
            repository: "~/repo".to_string(),
            question_data: None,
            input: String::new(),
            cursor_pos: 0,
            anchor_y: 8,
        };

        let ctx =
            RenderContext::new(&threads, &aggregate, &stats, &theme).with_overlay(Some(&overlay));

        assert_eq!(get_footer_hint(&ctx), "esc close");
    }

    #[test]
    fn test_footer_hint_with_overlay_plan() {
        let threads = vec![];
        let aggregate = Aggregate::new();
        let stats = SystemStats::default();
        let theme = Theme::default();

        let summary = crate::models::dashboard::PlanSummary::new(
            "Test Plan".to_string(),
            vec!["Phase 1".to_string()],
            2,
            Some(5000),
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

        let ctx =
            RenderContext::new(&threads, &aggregate, &stats, &theme).with_overlay(Some(&overlay));

        assert_eq!(get_footer_hint(&ctx), "esc close");
    }
}
