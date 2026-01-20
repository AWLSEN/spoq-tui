//! Thread list component for the dashboard
//!
//! Renders the list of threads with a separator between need-action and autonomous threads.
//! Supports both split view (for All filter) and filtered flat view.

use ratatui::{layout::Rect, style::Style, text::Span, Frame};

use super::context::{FilterState, RenderContext, ThreadView};
use super::thread_row;
use crate::models::dashboard::ThreadStatus;
use crate::ui::interaction::HitAreaRegistry;

// ============================================================================
// Constants
// ============================================================================

/// Minimum height required for the thread list
const MIN_HEIGHT: u16 = 5;

/// Maximum number of need-action threads to display before showing "+ N more"
const MAX_NEED_ACTION_DISPLAY: usize = 5;

/// Separator width as percentage of area width
const SEPARATOR_WIDTH_PERCENT: f32 = 0.10;

// ============================================================================
// Public API
// ============================================================================

/// Render the thread list
///
/// # Layout
///
/// Two modes based on filter state:
///
/// ## Split View (None or All filter)
/// ```text
/// [need_action threads - max 5]
/// [+ N more if needed]
/// [separator line ────────]
/// [autonomous threads - fill remaining space]
/// [+ N more if needed]
/// ```
///
/// ## Filtered View (Working, ReadyToTest, Idle filters)
/// ```text
/// [filtered threads - scrollable list]
/// [+ N more if needed]
/// ```
///
/// # Arguments
/// * `frame` - The ratatui frame to render into
/// * `area` - The rectangle area allocated for the thread list
/// * `ctx` - The render context containing thread views and filter state
/// * `registry` - Hit area registry for click handling
pub fn render(frame: &mut Frame, area: Rect, ctx: &RenderContext, registry: &mut HitAreaRegistry) {
    // Check minimum height requirement
    if area.height < MIN_HEIGHT {
        return;
    }

    match ctx.filter {
        None | Some(FilterState::All) => {
            // Split mode - separate need_action from autonomous
            render_split_view(frame, area, ctx, registry);
        }
        Some(_) => {
            // Flat mode - filtered list
            render_filtered_view(frame, area, ctx, registry);
        }
    }
}

// ============================================================================
// Split View (All/None filter)
// ============================================================================

/// Render split view with need-action and autonomous sections
fn render_split_view(
    frame: &mut Frame,
    area: Rect,
    ctx: &RenderContext,
    registry: &mut HitAreaRegistry,
) {
    // Partition threads into need_action and autonomous
    let (need_action, autonomous): (Vec<&ThreadView>, Vec<&ThreadView>) =
        ctx.threads.iter().partition(|t| t.needs_action);

    // Calculate layout heights
    let need_action_height = need_action.len().min(MAX_NEED_ACTION_DISPLAY) as u16;
    let separator_height: u16 = 1;
    // Note: autonomous_height is computed dynamically based on actual separator position
    let _autonomous_height = area.height.saturating_sub(need_action_height + separator_height);

    // Render need_action threads
    for (i, thread) in need_action.iter().take(MAX_NEED_ACTION_DISPLAY).enumerate() {
        let row_rect = Rect::new(area.x, area.y + i as u16, area.width, 1);
        thread_row::render(frame, row_rect, thread, ctx, registry);
    }

    // Show "+ N more" if there are more need_action threads than displayed
    let need_action_more_y = if need_action.len() > MAX_NEED_ACTION_DISPLAY {
        let more_text = format!("+ {} more", need_action.len() - MAX_NEED_ACTION_DISPLAY);
        let row_y = area.y + MAX_NEED_ACTION_DISPLAY as u16;
        if row_y < area.bottom() {
            frame.render_widget(
                Span::styled(more_text, Style::default()),
                Rect::new(area.x + 2, row_y, area.width.saturating_sub(2), 1),
            );
        }
        row_y + 1
    } else {
        area.y + need_action_height
    };

    // Render separator line "────────" at ~10% of area width
    let sep_y = need_action_more_y;
    if sep_y < area.bottom() {
        let sep_width = ((area.width as f32) * SEPARATOR_WIDTH_PERCENT).max(4.0) as u16;
        let separator = "\u{2500}".repeat(sep_width as usize);
        frame.render_widget(
            Span::raw(separator),
            Rect::new(area.x + 2, sep_y, sep_width, 1),
        );
    }

    // Render autonomous threads
    let autonomous_start_y = sep_y + 1;
    let available_autonomous_rows = area.bottom().saturating_sub(autonomous_start_y);

    for (i, thread) in autonomous
        .iter()
        .take(available_autonomous_rows as usize)
        .enumerate()
    {
        let row_rect = Rect::new(area.x, autonomous_start_y + i as u16, area.width, 1);
        thread_row::render(frame, row_rect, thread, ctx, registry);
    }

    // Show "+ N more" if there are more autonomous threads than displayed
    if autonomous.len() > available_autonomous_rows as usize {
        let more_text = format!(
            "+ {} more",
            autonomous.len() - available_autonomous_rows as usize
        );
        let row_y = area.bottom().saturating_sub(1);
        if row_y >= autonomous_start_y {
            frame.render_widget(
                Span::styled(more_text, Style::default()),
                Rect::new(area.x + 2, row_y, area.width.saturating_sub(2), 1),
            );
        }
    }

    // Special state: "all clear" when no need_action threads
    // (This will be delegated to states.rs in Phase 10)
    if need_action.is_empty() {
        // Render "all clear" centered in the need_action area
        // For now, just show a placeholder message
        let all_clear_text = "All clear - no threads need attention";
        let text_width = all_clear_text.len() as u16;
        let x_offset = area.width.saturating_sub(text_width) / 2;
        frame.render_widget(
            Span::raw(all_clear_text),
            Rect::new(area.x + x_offset, area.y, text_width, 1),
        );
    }
}

// ============================================================================
// Filtered View
// ============================================================================

/// Render filtered flat view based on filter state
fn render_filtered_view(
    frame: &mut Frame,
    area: Rect,
    ctx: &RenderContext,
    registry: &mut HitAreaRegistry,
) {
    // Filter threads based on ctx.filter
    let filtered_threads: Vec<&ThreadView> = ctx
        .threads
        .iter()
        .filter(|t| match ctx.filter {
            Some(FilterState::Working) => {
                matches!(t.status, ThreadStatus::Running | ThreadStatus::Waiting)
            }
            Some(FilterState::ReadyToTest) => t.status == ThreadStatus::Done,
            Some(FilterState::Idle) => {
                matches!(t.status, ThreadStatus::Idle | ThreadStatus::Error)
            }
            _ => true,
        })
        .collect();

    // No separator, scrollable list (for now render without scroll state)
    let max_visible = area.height as usize;
    for (i, thread) in filtered_threads.iter().take(max_visible).enumerate() {
        let row_rect = Rect::new(area.x, area.y + i as u16, area.width, 1);
        thread_row::render(frame, row_rect, thread, ctx, registry);
    }

    // Show "+ N more" if there are more threads than visible
    if filtered_threads.len() > max_visible {
        let more_text = format!("+ {} more", filtered_threads.len() - max_visible);
        // Show at bottom of area, overwriting the last row if needed
        let row_y = area.bottom().saturating_sub(1);
        frame.render_widget(
            Span::styled(more_text, Style::default()),
            Rect::new(area.x + 2, row_y, area.width.saturating_sub(2), 1),
        );
    }

    // Show empty state message if no threads match filter
    if filtered_threads.is_empty() {
        let empty_text = match ctx.filter {
            Some(FilterState::Working) => "No working threads",
            Some(FilterState::ReadyToTest) => "No threads ready to test",
            Some(FilterState::Idle) => "No idle threads",
            _ => "No threads",
        };
        let text_width = empty_text.len() as u16;
        let x_offset = area.width.saturating_sub(text_width) / 2;
        frame.render_widget(
            Span::raw(empty_text),
            Rect::new(area.x + x_offset, area.y, text_width, 1),
        );
    }
}

// ============================================================================
// Helper Functions for Partitioning (exposed for testing)
// ============================================================================

/// Partition threads into need-action and autonomous groups
///
/// # Arguments
/// * `threads` - Slice of thread views to partition
///
/// # Returns
/// Tuple of (need_action, autonomous) thread references
pub fn partition_threads<'a>(
    threads: &'a [ThreadView],
) -> (Vec<&'a ThreadView>, Vec<&'a ThreadView>) {
    threads.iter().partition(|t| t.needs_action)
}

/// Filter threads by status based on filter state
///
/// # Arguments
/// * `threads` - Slice of thread views to filter
/// * `filter` - Optional filter state
///
/// # Returns
/// Vector of filtered thread references
pub fn filter_threads_by_status<'a>(
    threads: &'a [ThreadView],
    filter: Option<FilterState>,
) -> Vec<&'a ThreadView> {
    threads
        .iter()
        .filter(|t| match filter {
            Some(FilterState::Working) => {
                matches!(t.status, ThreadStatus::Running | ThreadStatus::Waiting)
            }
            Some(FilterState::ReadyToTest) => t.status == ThreadStatus::Done,
            Some(FilterState::Idle) => {
                matches!(t.status, ThreadStatus::Idle | ThreadStatus::Error)
            }
            None | Some(FilterState::All) => true,
        })
        .collect()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::dashboard::ThreadStatus;
    use crate::ui::dashboard::context::ThreadView;

    // -------------------- Helper Functions --------------------

    fn make_thread(id: &str, title: &str, needs_action: bool, status: ThreadStatus) -> ThreadView {
        ThreadView::new(id.to_string(), title.to_string(), "~/repo".to_string())
            .with_status(status)
            .with_waiting_for(if needs_action {
                Some(crate::models::dashboard::WaitingFor::UserInput)
            } else {
                None
            })
    }

    fn make_test_threads() -> Vec<ThreadView> {
        vec![
            // Need action threads
            make_thread("1", "Waiting Thread", true, ThreadStatus::Waiting),
            make_thread("2", "Error Thread", true, ThreadStatus::Error),
            make_thread("3", "Another Waiting", true, ThreadStatus::Waiting),
            // Autonomous threads
            make_thread("4", "Running Thread", false, ThreadStatus::Running),
            make_thread("5", "Idle Thread", false, ThreadStatus::Idle),
            make_thread("6", "Done Thread", false, ThreadStatus::Done),
        ]
    }

    // -------------------- Partitioning Tests --------------------

    #[test]
    fn test_partition_threads_basic() {
        let threads = make_test_threads();
        let (need_action, autonomous) = partition_threads(&threads);

        assert_eq!(need_action.len(), 3);
        assert_eq!(autonomous.len(), 3);

        // Verify need_action threads
        for t in &need_action {
            assert!(t.needs_action, "Expected thread {} to need action", t.id);
        }

        // Verify autonomous threads
        for t in &autonomous {
            assert!(
                !t.needs_action,
                "Expected thread {} to not need action",
                t.id
            );
        }
    }

    #[test]
    fn test_partition_threads_all_need_action() {
        let threads = vec![
            make_thread("1", "T1", true, ThreadStatus::Waiting),
            make_thread("2", "T2", true, ThreadStatus::Error),
        ];
        let (need_action, autonomous) = partition_threads(&threads);

        assert_eq!(need_action.len(), 2);
        assert_eq!(autonomous.len(), 0);
    }

    #[test]
    fn test_partition_threads_none_need_action() {
        let threads = vec![
            make_thread("1", "T1", false, ThreadStatus::Running),
            make_thread("2", "T2", false, ThreadStatus::Idle),
        ];
        let (need_action, autonomous) = partition_threads(&threads);

        assert_eq!(need_action.len(), 0);
        assert_eq!(autonomous.len(), 2);
    }

    #[test]
    fn test_partition_threads_empty() {
        let threads: Vec<ThreadView> = vec![];
        let (need_action, autonomous) = partition_threads(&threads);

        assert_eq!(need_action.len(), 0);
        assert_eq!(autonomous.len(), 0);
    }

    // -------------------- Filter Tests --------------------

    #[test]
    fn test_filter_threads_working() {
        let threads = make_test_threads();
        let filtered = filter_threads_by_status(&threads, Some(FilterState::Working));

        // Should include Running and Waiting threads
        // From make_test_threads: 2 Waiting (id=1,3) + 1 Running (id=4) = 3 working threads
        assert_eq!(filtered.len(), 3);
        for t in &filtered {
            assert!(
                matches!(t.status, ThreadStatus::Running | ThreadStatus::Waiting),
                "Expected working status for thread {}",
                t.id
            );
        }
    }

    #[test]
    fn test_filter_threads_ready_to_test() {
        let threads = make_test_threads();
        let filtered = filter_threads_by_status(&threads, Some(FilterState::ReadyToTest));

        // Should only include Done threads
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].status, ThreadStatus::Done);
    }

    #[test]
    fn test_filter_threads_idle() {
        let threads = make_test_threads();
        let filtered = filter_threads_by_status(&threads, Some(FilterState::Idle));

        // Should include Idle and Error threads
        assert_eq!(filtered.len(), 2); // 1 Idle + 1 Error
        for t in &filtered {
            assert!(
                matches!(t.status, ThreadStatus::Idle | ThreadStatus::Error),
                "Expected idle status for thread {}",
                t.id
            );
        }
    }

    #[test]
    fn test_filter_threads_all() {
        let threads = make_test_threads();
        let filtered = filter_threads_by_status(&threads, Some(FilterState::All));

        assert_eq!(filtered.len(), threads.len());
    }

    #[test]
    fn test_filter_threads_none() {
        let threads = make_test_threads();
        let filtered = filter_threads_by_status(&threads, None);

        assert_eq!(filtered.len(), threads.len());
    }

    #[test]
    fn test_filter_threads_empty_result() {
        let threads = vec![
            make_thread("1", "T1", false, ThreadStatus::Running),
            make_thread("2", "T2", false, ThreadStatus::Running),
        ];
        let filtered = filter_threads_by_status(&threads, Some(FilterState::ReadyToTest));

        assert_eq!(filtered.len(), 0);
    }

    // -------------------- Edge Case Tests --------------------

    #[test]
    fn test_partition_preserves_order() {
        let threads = vec![
            make_thread("a", "First", true, ThreadStatus::Waiting),
            make_thread("b", "Second", false, ThreadStatus::Running),
            make_thread("c", "Third", true, ThreadStatus::Error),
            make_thread("d", "Fourth", false, ThreadStatus::Idle),
        ];
        let (need_action, autonomous) = partition_threads(&threads);

        // Check order is preserved within each partition
        assert_eq!(need_action[0].id, "a");
        assert_eq!(need_action[1].id, "c");
        assert_eq!(autonomous[0].id, "b");
        assert_eq!(autonomous[1].id, "d");
    }

    #[test]
    fn test_filter_preserves_order() {
        let threads = vec![
            make_thread("a", "First", false, ThreadStatus::Running),
            make_thread("b", "Second", false, ThreadStatus::Done),
            make_thread("c", "Third", false, ThreadStatus::Running),
        ];
        let filtered = filter_threads_by_status(&threads, Some(FilterState::Working));

        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].id, "a");
        assert_eq!(filtered[1].id, "c");
    }

    #[test]
    fn test_many_need_action_threads() {
        // Test with more than MAX_NEED_ACTION_DISPLAY threads
        let mut threads = Vec::new();
        for i in 0..10 {
            threads.push(make_thread(
                &i.to_string(),
                &format!("Thread {}", i),
                true,
                ThreadStatus::Waiting,
            ));
        }

        let (need_action, autonomous) = partition_threads(&threads);
        assert_eq!(need_action.len(), 10);
        assert_eq!(autonomous.len(), 0);

        // The render function should show max 5 and "+ 5 more"
        // This is verified by the constant MAX_NEED_ACTION_DISPLAY = 5
    }
}
