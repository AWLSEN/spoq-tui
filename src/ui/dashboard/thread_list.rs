//! Thread list component for the dashboard
//!
//! Renders the list of threads with a separator between need-action and autonomous threads.
//! Supports both split view (for All filter) and filtered flat view.

use ratatui::{layout::Rect, style::Style, text::Span, Frame};

use super::states;
use super::thread_row;
use super::{FilterState, RenderContext, ThreadView};
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

/// Thread list width as percentage of area width (8% margin on each side)
const THREAD_LIST_WIDTH_PERCENT: f32 = 0.84;

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

    // Calculate centered area with 84% width
    let centered_area = calculate_centered_area(area);

    match ctx.filter {
        None | Some(FilterState::All) => {
            // Split mode - separate need_action from autonomous
            render_split_view(frame, centered_area, ctx, registry);
        }
        Some(_) => {
            // Flat mode - filtered list
            render_filtered_view(frame, centered_area, ctx, registry);
        }
    }
}

/// Calculate a horizontally centered area with 84% width
///
/// Returns a centered rectangle using 84% of the available width (8% margin on each side).
/// This matches the status bar's width calculation for consistent horizontal alignment.
fn calculate_centered_area(area: Rect) -> Rect {
    // Calculate 84% width (8% margin on each side), matching status bar
    let card_width = (area.width as f32 * THREAD_LIST_WIDTH_PERCENT).round() as u16;
    let left_padding = (area.width - card_width) / 2;

    Rect::new(area.x + left_padding, area.y, card_width, area.height)
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
    let _autonomous_height = area
        .height
        .saturating_sub(need_action_height + separator_height);

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

    // Special state: "all clear" when no need_action threads AND no autonomous threads
    if need_action.is_empty() && autonomous.is_empty() {
        // Calculate the area available for the "all clear" message
        // (from top of area to separator line)
        let need_action_area = Rect::new(
            area.x,
            area.y,
            area.width,
            need_action_more_y.saturating_sub(area.y),
        );
        states::render_all_clear(frame, need_action_area, autonomous.len());
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
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use crate::models::dashboard::ThreadStatus;
    use crate::ui::dashboard::ThreadView;

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
        let (need_action, autonomous): (Vec<&ThreadView>, Vec<&ThreadView>) =
            threads.iter().partition(|t| t.needs_action);

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
        let (need_action, autonomous): (Vec<&ThreadView>, Vec<&ThreadView>) =
            threads.iter().partition(|t| t.needs_action);

        assert_eq!(need_action.len(), 2);
        assert_eq!(autonomous.len(), 0);
    }

    #[test]
    fn test_partition_threads_none_need_action() {
        let threads = vec![
            make_thread("1", "T1", false, ThreadStatus::Running),
            make_thread("2", "T2", false, ThreadStatus::Idle),
        ];
        let (need_action, autonomous): (Vec<&ThreadView>, Vec<&ThreadView>) =
            threads.iter().partition(|t| t.needs_action);

        assert_eq!(need_action.len(), 0);
        assert_eq!(autonomous.len(), 2);
    }

    #[test]
    fn test_partition_threads_empty() {
        let threads: Vec<ThreadView> = vec![];
        let (need_action, autonomous): (Vec<&ThreadView>, Vec<&ThreadView>) =
            threads.iter().partition(|t| t.needs_action);

        assert_eq!(need_action.len(), 0);
        assert_eq!(autonomous.len(), 0);
    }

    // -------------------- Filter Tests --------------------

    #[test]
    fn test_filter_threads_working() {
        let threads = make_test_threads();
        let filtered: Vec<&ThreadView> = threads
            .iter()
            .filter(|t| matches!(t.status, ThreadStatus::Running | ThreadStatus::Waiting))
            .collect();

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
        let filtered: Vec<&ThreadView> = threads
            .iter()
            .filter(|t| t.status == ThreadStatus::Done)
            .collect();

        // Should only include Done threads
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].status, ThreadStatus::Done);
    }

    #[test]
    fn test_filter_threads_idle() {
        let threads = make_test_threads();
        let filtered: Vec<&ThreadView> = threads
            .iter()
            .filter(|t| matches!(t.status, ThreadStatus::Idle | ThreadStatus::Error))
            .collect();

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
        let filtered: Vec<&ThreadView> = threads.iter().collect();

        assert_eq!(filtered.len(), threads.len());
    }

    #[test]
    fn test_filter_threads_empty_result() {
        let threads = vec![
            make_thread("1", "T1", false, ThreadStatus::Running),
            make_thread("2", "T2", false, ThreadStatus::Running),
        ];
        let filtered: Vec<&ThreadView> = threads
            .iter()
            .filter(|t| t.status == ThreadStatus::Done)
            .collect();

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
        let (need_action, autonomous): (Vec<&ThreadView>, Vec<&ThreadView>) =
            threads.iter().partition(|t| t.needs_action);

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
        let filtered: Vec<&ThreadView> = threads
            .iter()
            .filter(|t| matches!(t.status, ThreadStatus::Running | ThreadStatus::Waiting))
            .collect();

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

        let (need_action, autonomous): (Vec<&ThreadView>, Vec<&ThreadView>) =
            threads.iter().partition(|t| t.needs_action);
        assert_eq!(need_action.len(), 10);
        assert_eq!(autonomous.len(), 0);

        // The render function should show max 5 and "+ 5 more"
        // This is verified by the constant MAX_NEED_ACTION_DISPLAY = 5
    }

    // -------------------- Centering Tests --------------------

    #[test]
    fn test_calculate_centered_area_narrow_terminal() {
        use super::calculate_centered_area;
        use super::THREAD_LIST_WIDTH_PERCENT;
        use ratatui::layout::Rect;

        // Terminal should always use 84% width
        let area = Rect::new(0, 5, 60, 20);
        let centered = calculate_centered_area(area);

        // Width should be 84% of area width
        let expected_width = (area.width as f32 * THREAD_LIST_WIDTH_PERCENT).round() as u16;
        assert_eq!(centered.width, expected_width);

        // Should be horizontally centered
        let expected_x = (area.width - expected_width) / 2;
        assert_eq!(centered.x, expected_x);

        // Y and height should be unchanged
        assert_eq!(centered.y, area.y);
        assert_eq!(centered.height, area.height);
    }

    #[test]
    fn test_calculate_centered_area_wide_terminal() {
        use super::calculate_centered_area;
        use super::THREAD_LIST_WIDTH_PERCENT;
        use ratatui::layout::Rect;

        // Terminal should use 84% width regardless of size
        let area = Rect::new(0, 5, 150, 20);
        let centered = calculate_centered_area(area);

        // Width should be 84% of area width
        let expected_width = (area.width as f32 * THREAD_LIST_WIDTH_PERCENT).round() as u16;
        assert_eq!(centered.width, expected_width);

        // Should be horizontally centered
        let expected_x = (area.width - expected_width) / 2;
        assert_eq!(centered.x, expected_x);

        // Y and height should be unchanged
        assert_eq!(centered.y, area.y);
        assert_eq!(centered.height, area.height);
    }

    #[test]
    fn test_calculate_centered_area_100_width() {
        use super::calculate_centered_area;
        use super::THREAD_LIST_WIDTH_PERCENT;
        use ratatui::layout::Rect;

        // Test with width of 100 for easy percentage calculation
        let area = Rect::new(5, 10, 100, 15);
        let centered = calculate_centered_area(area);

        // 84% of 100 = 84
        let expected_width = (100_f32 * THREAD_LIST_WIDTH_PERCENT).round() as u16;
        assert_eq!(centered.width, expected_width);

        // Centered: (100 - 84) / 2 = 8 offset from area.x
        let expected_x = area.x + (area.width - expected_width) / 2;
        assert_eq!(centered.x, expected_x);

        // Y and height should be unchanged
        assert_eq!(centered.y, area.y);
        assert_eq!(centered.height, area.height);
    }

    #[test]
    fn test_calculate_centered_area_with_offset() {
        use super::calculate_centered_area;
        use super::THREAD_LIST_WIDTH_PERCENT;
        use ratatui::layout::Rect;

        // Area with non-zero x should center relative to area
        let area = Rect::new(10, 5, 200, 30);
        let centered = calculate_centered_area(area);

        // Width should be 84% of area width
        let expected_width = (area.width as f32 * THREAD_LIST_WIDTH_PERCENT).round() as u16;
        assert_eq!(centered.width, expected_width);

        // X offset should be area.x + centering offset
        let expected_x = area.x + (area.width - expected_width) / 2;
        assert_eq!(centered.x, expected_x);
    }
}
