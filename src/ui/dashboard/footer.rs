//! Footer hints for dashboard context-awareness
//!
//! Provides dynamic footer hints based on UI state (filter, overlay, etc.)

use crate::ui::dashboard::{FilterState, RenderContext};

/// Get context-aware footer hint text based on UI state
///
/// Returns appropriate hint text depending on whether:
/// - An overlay is open (show "esc close")
/// - A filter is active (show "✕ clear")
/// - Default state (show "click status to filter")
///
/// # Arguments
/// * `ctx` - The render context containing overlay and filter state
///
/// # Returns
/// Static string reference with the appropriate hint text
pub fn get_footer_hint(ctx: &RenderContext) -> &'static str {
    if ctx.overlay.is_some() {
        "esc close"
    } else if let Some(filter) = ctx.filter {
        if filter != FilterState::All {
            "✕ clear"
        } else {
            "click status to filter"
        }
    } else {
        "click status to filter"
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

        assert_eq!(get_footer_hint(&ctx), "click status to filter");
    }

    #[test]
    fn test_footer_hint_with_filter_all() {
        let threads = vec![];
        let aggregate = Aggregate::new();
        let stats = SystemStats::default();
        let theme = Theme::default();

        let ctx = RenderContext::new(&threads, &aggregate, &stats, &theme)
            .with_filter(Some(FilterState::All));

        // FilterState::All should show default hint
        assert_eq!(get_footer_hint(&ctx), "click status to filter");
    }

    #[test]
    fn test_footer_hint_with_filter_working() {
        let threads = vec![];
        let aggregate = Aggregate::new();
        let stats = SystemStats::default();
        let theme = Theme::default();

        let ctx = RenderContext::new(&threads, &aggregate, &stats, &theme)
            .with_filter(Some(FilterState::Working));

        assert_eq!(get_footer_hint(&ctx), "✕ clear");
    }

    #[test]
    fn test_footer_hint_with_filter_ready_to_test() {
        let threads = vec![];
        let aggregate = Aggregate::new();
        let stats = SystemStats::default();
        let theme = Theme::default();

        let ctx = RenderContext::new(&threads, &aggregate, &stats, &theme)
            .with_filter(Some(FilterState::ReadyToTest));

        assert_eq!(get_footer_hint(&ctx), "✕ clear");
    }

    #[test]
    fn test_footer_hint_with_filter_idle() {
        let threads = vec![];
        let aggregate = Aggregate::new();
        let stats = SystemStats::default();
        let theme = Theme::default();

        let ctx = RenderContext::new(&threads, &aggregate, &stats, &theme)
            .with_filter(Some(FilterState::Idle));

        assert_eq!(get_footer_hint(&ctx), "✕ clear");
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
            question: "Continue?".to_string(),
            options: vec!["Yes".to_string(), "No".to_string()],
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
            question: "Enter details".to_string(),
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

    #[test]
    fn test_footer_hint_overlay_takes_precedence_over_filter() {
        // When both overlay and filter are present, overlay hint should take precedence
        let threads = vec![];
        let aggregate = Aggregate::new();
        let stats = SystemStats::default();
        let theme = Theme::default();

        let overlay = OverlayState::Question {
            thread_id: "t1".to_string(),
            thread_title: "Test".to_string(),
            repository: "~/repo".to_string(),
            question: "Continue?".to_string(),
            options: vec!["Yes".to_string(), "No".to_string()],
            anchor_y: 10,
        };

        let ctx = RenderContext::new(&threads, &aggregate, &stats, &theme)
            .with_filter(Some(FilterState::Working))
            .with_overlay(Some(&overlay));

        assert_eq!(get_footer_hint(&ctx), "esc close");
    }
}
