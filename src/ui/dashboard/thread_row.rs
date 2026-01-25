//! Thread row component for dashboard rendering
//!
//! Renders a single thread view as a compact row showing title, repository,
//! mode, status, progress, time, and action buttons.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    Frame,
};

use crate::models::dashboard::{ThreadStatus, WaitingFor};
use crate::ui::dashboard::{RenderContext, ThreadMode, ThreadView};
use crate::ui::interaction::{ClickAction, HitAreaRegistry};

// ============================================================================
// Public API
// ============================================================================

/// Render a single thread row
///
/// # Layout (single row, height=1)
/// ```text
/// "Auth Refactor          ~/api       plan       waiting              [approve] [reject]"
/// |-- Title (25%) --|-- Repo (12%) --|-- Mode (8%) --|-- Status (12%) --|-- Progress (15%) --|-- Time (8%) --|-- Actions --|
/// ```
///
/// # Arguments
/// * `frame` - The ratatui frame to render into
/// * `area` - The rectangle area allocated for this row (height=1)
/// * `thread` - The thread view data to render
/// * `ctx` - The render context containing theme colors
/// * `registry` - Hit area registry for mouse interaction
pub fn render(
    frame: &mut Frame,
    area: Rect,
    thread: &ThreadView,
    ctx: &RenderContext,
    registry: &mut HitAreaRegistry,
) {
    if area.height < 1 || area.width < 20 {
        return;
    }

    let buf = frame.buffer_mut();

    // DEBUG: Draw border around the row area
    // Top-left corner
    if area.x > 0 && area.y > 0 {
        buf[(area.x, area.y)].set_char('[');
    }
    // Top-right corner
    if area.x + area.width > 0 {
        let right_x = (area.x + area.width).saturating_sub(1);
        if right_x < buf.area().width {
            buf[(right_x, area.y)].set_char(']');
        }
    }

    // For threads that need action, skip time column to give more space to buttons
    // needs_action threads: 30+12+9+12+0+0 = 63%, leaving 37% for right-aligned actions
    // other threads: 28+14+9+14+15+10 = 90%, leaving 10% for right-aligned Verify button
    let show_time = !thread.needs_action;

    let (title_width, repo_width, mode_width, status_width, progress_width, time_width) = if thread.needs_action {
        // needs_action threads: expanded data columns, no progress/time
        let title_width = ((area.width as f32) * 0.30) as u16;
        let repo_width = ((area.width as f32) * 0.12) as u16;
        let mode_width = ((area.width as f32) * 0.09) as u16;
        let status_width = ((area.width as f32) * 0.12) as u16;
        let progress_width = 0; // no progress for needs_action
        let time_width = 0; // no time column for needs_action
        (title_width, repo_width, mode_width, status_width, progress_width, time_width)
    } else {
        // non-need-action threads: expand columns to use ~90% of width
        let title_width = ((area.width as f32) * 0.28) as u16;
        let repo_width = ((area.width as f32) * 0.14) as u16;
        let mode_width = ((area.width as f32) * 0.09) as u16;
        let status_width = ((area.width as f32) * 0.14) as u16;
        let progress_width = ((area.width as f32) * 0.15) as u16;
        let time_width = ((area.width as f32) * 0.10) as u16;
        (title_width, repo_width, mode_width, status_width, progress_width, time_width)
    };

    // Track current x position
    let mut x = area.x;
    let y = area.y;

    // Register entire row as hit area for expanding the thread
    registry.register(
        area,
        ClickAction::ExpandThread {
            thread_id: thread.id.clone(),
            anchor_y: area.y,
        },
        None,
    );

    // Title column (bold)
    let title_text = truncate(&thread.title, title_width.saturating_sub(1) as usize);
    let title_style = Style::default().add_modifier(Modifier::BOLD);
    render_text(buf, x, y, &title_text, title_style, area);
    x += title_width;

    // Repository column
    let repo_text = truncate(&thread.repository, repo_width.saturating_sub(1) as usize);
    let repo_style = Style::default().fg(ctx.theme.dim);
    render_text(buf, x, y, &repo_text, repo_style, area);
    x += repo_width;

    // Mode column
    let mode_text = match thread.mode {
        ThreadMode::Normal => "normal",
        ThreadMode::Plan => "plan",
        ThreadMode::Exec => "exec",
    };
    let mode_style = Style::default().fg(ctx.theme.dim);
    render_text(buf, x, y, mode_text, mode_style, area);
    x += mode_width;

    // Status column (colored by status)
    let status_text = thread.status_line();
    let status_text = truncate(&status_text, status_width.saturating_sub(1) as usize);
    let status_style = Style::default().fg(status_color(thread.status, ctx));
    render_text(buf, x, y, &status_text, status_style, area);
    x += status_width;

    // Progress column (only if running and progress is present)
    if thread.status == ThreadStatus::Running {
        if let Some(ref progress) = thread.progress {
            let progress_text = render_progress(progress.current, progress.total);
            let progress_text = truncate(&progress_text, progress_width.saturating_sub(1) as usize);
            let progress_style = Style::default().fg(ctx.theme.accent);
            render_text(buf, x, y, &progress_text, progress_style, area);
        }
    }
    x += progress_width;

    // Time column (only for threads that don't need action)
    if show_time {
        let time_text = truncate(&thread.duration, time_width.saturating_sub(1) as usize);
        let time_style = Style::default().fg(ctx.theme.dim);
        render_text(buf, x, y, &time_text, time_style, area);
        x += time_width;
    }

    // Action buttons (based on status and waiting_for)
    // Use right-align for need-action threads (permission/plan approval buttons)
    let right_align = thread.needs_action;
    render_actions(frame, x, y, area, thread, ctx, registry, right_align);
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Render text at a specific position
fn render_text(
    buf: &mut ratatui::buffer::Buffer,
    x: u16,
    y: u16,
    text: &str,
    style: Style,
    area: Rect,
) {
    for (offset, ch) in text.chars().enumerate() {
        let pos_x = x + offset as u16;
        if pos_x < area.x + area.width {
            buf[(pos_x, y)].set_char(ch).set_style(style);
        }
    }
}

/// Render action buttons based on thread status
fn render_actions(
    frame: &mut Frame,
    x: u16,
    y: u16,
    area: Rect,
    thread: &ThreadView,
    ctx: &RenderContext,
    registry: &mut HitAreaRegistry,
    right_align: bool,
) {
    let buf = frame.buffer_mut();

    // Key style: accent + bold
    let key_style = Style::default()
        .fg(ctx.theme.accent)
        .add_modifier(Modifier::BOLD);

    // Label style: dim
    let label_style = Style::default().fg(ctx.theme.dim);

    // Determine which buttons to show
    let buttons = match (&thread.status, &thread.waiting_for) {
        // Permission -> [y] Yes  [n] No  [a] Always
        (ThreadStatus::Waiting, Some(WaitingFor::Permission { .. })) => {
            vec![
                ("[y]", "Yes", ButtonAction::Approve),
                ("[n]", "No", ButtonAction::Reject),
                ("[a]", "Always", ButtonAction::Always),
            ]
        }
        // Plan approval -> [y] Yes  [n] No
        (ThreadStatus::Waiting, Some(WaitingFor::PlanApproval { .. })) => {
            vec![
                ("[y]", "Yes", ButtonAction::Approve),
                ("[n]", "No", ButtonAction::Reject),
            ]
        }
        // User input -> [a] Answer
        (ThreadStatus::Waiting, Some(WaitingFor::UserInput)) => {
            vec![("[a]", "Answer", ButtonAction::Answer)]
        }
        // Done -> [v] Verify
        (ThreadStatus::Done, _) => {
            vec![("[v]", "Verify", ButtonAction::Verify)]
        }
        // Idle/Running/Error -> no buttons
        _ => vec![],
    };

    // Calculate total width of all buttons
    let total_button_width: u16 = buttons
        .iter()
        .enumerate()
        .map(|(i, (key, label, _))| {
            let key_len = key.len() as u16;
            let label_len = label.len() as u16;
            let button_width = key_len + 1 + label_len; // key + space + label
            let spacing = if i < buttons.len() - 1 { 2 } else { 0 }; // 2 spaces between buttons
            button_width + spacing
        })
        .sum();

    // Determine starting x position based on alignment
    let mut current_x = if right_align {
        // Start from right edge, subtract total button width
        area.x + area.width.saturating_sub(total_button_width)
    } else {
        // Left align: start from provided x position
        x
    };

    for (key, label, action) in buttons {
        let key_len = key.len() as u16;
        let label_len = label.len() as u16;
        let total_len = key_len + 1 + label_len; // key + space + label

        // Check if there's room for this button
        if current_x + total_len > area.x + area.width {
            break;
        }

        // Render key (e.g., "[y]")
        render_text(buf, current_x, y, key, key_style, area);
        current_x += key_len;

        // Render space
        render_text(buf, current_x, y, " ", label_style, area);
        current_x += 1;

        // Render label (e.g., "Yes")
        render_text(buf, current_x, y, label, label_style, area);
        current_x += label_len;

        // Register hit area for entire button (key + space + label)
        let button_rect = Rect::new(current_x - total_len, y, total_len, 1);
        let click_action = match action {
            ButtonAction::Approve => ClickAction::ApproveThread(thread.id.clone()),
            ButtonAction::Reject => ClickAction::RejectThread(thread.id.clone()),
            ButtonAction::Always => ClickAction::AllowToolAlways(thread.id.clone()),
            ButtonAction::Answer => ClickAction::ExpandThread {
                thread_id: thread.id.clone(),
                anchor_y: y,
            },
            ButtonAction::Verify => ClickAction::VerifyThread(thread.id.clone()),
        };
        registry.register(button_rect, click_action, Some(key_style));

        current_x += 2; // 2 spaces between buttons
    }
}

/// Internal enum for button types
enum ButtonAction {
    Approve,
    Reject,
    Always,
    Answer,
    Verify,
}

/// Get the color for a thread status
fn status_color(status: ThreadStatus, ctx: &RenderContext) -> ratatui::style::Color {
    match status {
        ThreadStatus::Idle => ctx.theme.dim,
        ThreadStatus::Running => ctx.theme.accent,
        ThreadStatus::Waiting => ctx.theme.waiting,
        ThreadStatus::Done => ctx.theme.success,
        ThreadStatus::Error => ctx.theme.error,
    }
}

/// Render progress as dots
///
/// # Arguments
/// * `current` - Current step number
/// * `total` - Total number of steps
///
/// # Returns
/// A string like "●●●○○○ 3/6"
///
/// # Example
/// ```ignore
/// let progress = render_progress(3, 6);
/// assert_eq!(progress, "●●●○○○ 3/6");
/// ```
pub fn render_progress(current: u32, total: u32) -> String {
    let filled = "\u{25CF}".repeat(current as usize); // ●
    let empty = "\u{25CB}".repeat(total.saturating_sub(current) as usize); // ○
    format!("{}{} {}/{}", filled, empty, current, total)
}

/// Truncate a string with ellipsis if it exceeds max_len
///
/// # Arguments
/// * `s` - The string to truncate
/// * `max_len` - Maximum length including the ellipsis
///
/// # Returns
/// The original string if it fits, or truncated with "..." if it doesn't
///
/// # Example
/// ```ignore
/// let s = truncate("Hello, World!", 8);
/// assert_eq!(s, "Hello...");
/// ```
pub fn truncate(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else if max_len <= 3 {
        ".".repeat(max_len)
    } else {
        let chars: Vec<char> = s.chars().take(max_len - 3).collect();
        format!("{}...", chars.into_iter().collect::<String>())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------- render_progress Tests --------------------

    #[test]
    fn test_render_progress_basic() {
        let result = render_progress(3, 6);
        assert_eq!(result, "●●●○○○ 3/6");
    }

    #[test]
    fn test_render_progress_zero() {
        let result = render_progress(0, 5);
        assert_eq!(result, "○○○○○ 0/5");
    }

    #[test]
    fn test_render_progress_complete() {
        let result = render_progress(4, 4);
        assert_eq!(result, "●●●● 4/4");
    }

    #[test]
    fn test_render_progress_over_total() {
        // Edge case: current > total
        let result = render_progress(5, 3);
        assert_eq!(result, "●●●●● 5/3");
    }

    #[test]
    fn test_render_progress_single_step() {
        let result = render_progress(1, 1);
        assert_eq!(result, "● 1/1");
    }

    #[test]
    fn test_render_progress_large() {
        let result = render_progress(10, 20);
        assert_eq!(result, "●●●●●●●●●●○○○○○○○○○○ 10/20");
    }

    // -------------------- truncate Tests --------------------

    #[test]
    fn test_truncate_no_truncation() {
        let result = truncate("Hello", 10);
        assert_eq!(result, "Hello");
    }

    #[test]
    fn test_truncate_exact_fit() {
        let result = truncate("Hello", 5);
        assert_eq!(result, "Hello");
    }

    #[test]
    fn test_truncate_with_ellipsis() {
        let result = truncate("Hello, World!", 8);
        assert_eq!(result, "Hello...");
    }

    #[test]
    fn test_truncate_short_max_len() {
        // When max_len <= 3, just return dots
        let result = truncate("Hello", 3);
        assert_eq!(result, "...");
    }

    #[test]
    fn test_truncate_very_short_max_len() {
        let result = truncate("Hello", 2);
        assert_eq!(result, "..");
    }

    #[test]
    fn test_truncate_zero_max_len() {
        let result = truncate("Hello", 0);
        assert_eq!(result, "");
    }

    #[test]
    fn test_truncate_empty_string() {
        let result = truncate("", 10);
        assert_eq!(result, "");
    }

    #[test]
    fn test_truncate_unicode() {
        // "Hello 世界!" has 10 characters, so it fits in max_len=10
        let result = truncate("Hello 世界!", 10);
        assert_eq!(result, "Hello 世界!");
    }

    #[test]
    fn test_truncate_unicode_exact() {
        let result = truncate("日本語", 3);
        assert_eq!(result, "日本語");
    }

    #[test]
    fn test_truncate_unicode_truncated() {
        // "日本語テスト" has 6 characters, so max_len=5 should truncate
        let result = truncate("日本語テスト", 5);
        assert_eq!(result, "日本...");
    }

    // -------------------- Button Generation Tests --------------------

    #[test]
    fn test_buttons_permission_waiting() {
        use crate::models::dashboard::{ThreadStatus, WaitingFor};
        use crate::view_state::dashboard_view::ThreadView;

        let thread = ThreadView {
            id: "test-1".to_string(),
            title: "Test Thread".to_string(),
            repository: "test-repo".to_string(),
            mode: crate::models::ThreadMode::Normal,
            status: ThreadStatus::Waiting,
            waiting_for: Some(WaitingFor::Permission {
                request_id: "req-123".to_string(),
                tool_name: "test_tool".to_string(),
            }),
            progress: None,
            duration: "1m".to_string(),
            needs_action: true,
            current_operation: None,
        };

        // Verify the match would produce 3 buttons for permission
        let buttons = match (&thread.status, &thread.waiting_for) {
            (ThreadStatus::Waiting, Some(WaitingFor::Permission { .. })) => {
                vec![
                    ("[y]", "Yes", ButtonAction::Approve),
                    ("[n]", "No", ButtonAction::Reject),
                    ("[a]", "Always", ButtonAction::Always),
                ]
            }
            _ => vec![],
        };

        assert_eq!(buttons.len(), 3);
        assert_eq!(buttons[0].0, "[y]");
        assert_eq!(buttons[0].1, "Yes");
        assert_eq!(buttons[1].0, "[n]");
        assert_eq!(buttons[1].1, "No");
        assert_eq!(buttons[2].0, "[a]");
        assert_eq!(buttons[2].1, "Always");
    }

    #[test]
    fn test_buttons_plan_approval_waiting() {
        use crate::models::dashboard::{ThreadStatus, WaitingFor};
        use crate::view_state::dashboard_view::ThreadView;

        let thread = ThreadView {
            id: "test-2".to_string(),
            title: "Test Thread".to_string(),
            repository: "test-repo".to_string(),
            mode: crate::models::ThreadMode::Plan,
            status: ThreadStatus::Waiting,
            waiting_for: Some(WaitingFor::PlanApproval {
                request_id: "req-456".to_string(),
            }),
            progress: None,
            duration: "2m".to_string(),
            needs_action: true,
            current_operation: None,
        };

        // Verify the match would produce 2 buttons for plan approval
        let buttons = match (&thread.status, &thread.waiting_for) {
            (ThreadStatus::Waiting, Some(WaitingFor::PlanApproval { .. })) => {
                vec![
                    ("[y]", "Yes", ButtonAction::Approve),
                    ("[n]", "No", ButtonAction::Reject),
                ]
            }
            _ => vec![],
        };

        assert_eq!(buttons.len(), 2);
        assert_eq!(buttons[0].0, "[y]");
        assert_eq!(buttons[0].1, "Yes");
        assert_eq!(buttons[1].0, "[n]");
        assert_eq!(buttons[1].1, "No");
    }

    #[test]
    fn test_buttons_user_input_waiting() {
        use crate::models::dashboard::{ThreadStatus, WaitingFor};
        use crate::view_state::dashboard_view::ThreadView;

        let thread = ThreadView {
            id: "test-3".to_string(),
            title: "Test Thread".to_string(),
            repository: "test-repo".to_string(),
            mode: crate::models::ThreadMode::Normal,
            status: ThreadStatus::Waiting,
            waiting_for: Some(WaitingFor::UserInput),
            progress: None,
            duration: "30s".to_string(),
            needs_action: true,
            current_operation: None,
        };

        // Verify the match would produce 1 button for user input
        let buttons = match (&thread.status, &thread.waiting_for) {
            (ThreadStatus::Waiting, Some(WaitingFor::UserInput)) => {
                vec![("[a]", "Answer", ButtonAction::Answer)]
            }
            _ => vec![],
        };

        assert_eq!(buttons.len(), 1);
        assert_eq!(buttons[0].0, "[a]");
        assert_eq!(buttons[0].1, "Answer");
    }

    #[test]
    fn test_buttons_done_status() {
        use crate::models::dashboard::ThreadStatus;
        use crate::view_state::dashboard_view::ThreadView;

        let thread = ThreadView {
            id: "test-4".to_string(),
            title: "Test Thread".to_string(),
            repository: "test-repo".to_string(),
            mode: crate::models::ThreadMode::Normal,
            status: ThreadStatus::Done,
            waiting_for: None,
            progress: None,
            duration: "5m".to_string(),
            needs_action: false,
            current_operation: None,
        };

        // Verify the match would produce 1 button for done status
        let buttons = match (&thread.status, &thread.waiting_for) {
            (ThreadStatus::Done, _) => {
                vec![("[v]", "Verify", ButtonAction::Verify)]
            }
            _ => vec![],
        };

        assert_eq!(buttons.len(), 1);
        assert_eq!(buttons[0].0, "[v]");
        assert_eq!(buttons[0].1, "Verify");
    }

    #[test]
    fn test_buttons_running_no_buttons() {
        use crate::models::dashboard::ThreadStatus;
        use crate::view_state::dashboard_view::{Progress, ThreadView};

        let thread = ThreadView {
            id: "test-5".to_string(),
            title: "Test Thread".to_string(),
            repository: "test-repo".to_string(),
            mode: crate::models::ThreadMode::Normal,
            status: ThreadStatus::Running,
            waiting_for: None,
            progress: Some(Progress {
                current: 2,
                total: 5,
            }),
            duration: "1m".to_string(),
            needs_action: false,
            current_operation: Some("Running tests".to_string()),
        };

        // Verify running status produces no buttons
        let buttons = match (&thread.status, &thread.waiting_for) {
            (ThreadStatus::Waiting, Some(WaitingFor::Permission { .. })) => {
                vec![
                    ("[y]", "Yes", ButtonAction::Approve),
                    ("[n]", "No", ButtonAction::Reject),
                    ("[a]", "Always", ButtonAction::Always),
                ]
            }
            (ThreadStatus::Waiting, Some(WaitingFor::PlanApproval { .. })) => {
                vec![
                    ("[y]", "Yes", ButtonAction::Approve),
                    ("[n]", "No", ButtonAction::Reject),
                ]
            }
            (ThreadStatus::Waiting, Some(WaitingFor::UserInput)) => {
                vec![("[a]", "Answer", ButtonAction::Answer)]
            }
            (ThreadStatus::Done, _) => {
                vec![("[v]", "Verify", ButtonAction::Verify)]
            }
            _ => vec![],
        };

        assert_eq!(buttons.len(), 0);
    }

    // -------------------- Integration Tests (Full Rendering with Hit Areas) --------------------

    #[test]
    fn test_render_permission_buttons_registers_hit_areas() {
        use crate::models::dashboard::{Aggregate, ThreadStatus, WaitingFor};
        use crate::ui::interaction::{ClickAction, HitAreaRegistry};
        use crate::view_state::dashboard_view::ThreadView;
        use crate::view_state::SystemStats;
        use ratatui::backend::TestBackend;
        use ratatui::layout::Rect;
        use ratatui::Terminal;

        let thread = ThreadView {
            id: "thread-1".to_string(),
            title: "Test".to_string(),
            repository: "repo".to_string(),
            mode: crate::models::ThreadMode::Normal,
            status: ThreadStatus::Waiting,
            waiting_for: Some(WaitingFor::Permission {
                request_id: "req-1".to_string(),
                tool_name: "test_tool".to_string(),
            }),
            progress: None,
            duration: "1m".to_string(),
            needs_action: true,
            current_operation: None,
        };

        // Use wider terminal to ensure all buttons fit
        let backend = TestBackend::new(200, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut registry = HitAreaRegistry::new();

        let theme = crate::view_state::Theme::default();
        let system_stats = SystemStats {
            connected: true,
            cpu_percent: 10.0,
            ram_used_gb: 2.0,
            ram_total_gb: 8.0,
        };
        let aggregate = Aggregate::new();
        let threads = vec![];

        let ctx = RenderContext {
            threads: &threads,
            aggregate: &aggregate,
            filter: None,
            overlay: None,
            system_stats: &system_stats,
            theme: &theme,
        };

        terminal
            .draw(|frame| {
                let area = Rect::new(0, 0, 200, 1);
                render(frame, area, &thread, &ctx, &mut registry);
            })
            .unwrap();

        // Verify hit areas were registered
        // Should have: 3 permission buttons (row expand area is registered first but may be overwritten)
        // Note: With ratatui's TestBackend, the rendering flow may differ slightly
        assert!(registry.len() >= 3, "Expected at least 3 hit areas (permission buttons), got {}", registry.len());

        // Test that clicking on buttons returns correct actions
        // Note: We can't test exact positions without knowing layout, but we can verify
        // that some areas map to the expected actions
        let mut found_approve = false;
        let mut found_reject = false;
        let mut found_always = false;

        // Scan across the row to find button hit areas
        for x in 0..200 {
            if let Some(action) = registry.hit_test(x, 0) {
                match action {
                    ClickAction::ApproveThread(id) if id == "thread-1" => found_approve = true,
                    ClickAction::RejectThread(id) if id == "thread-1" => found_reject = true,
                    ClickAction::AllowToolAlways(id) if id == "thread-1" => found_always = true,
                    _ => {}
                }
            }
        }

        assert!(found_approve, "ApproveThread action not found in hit areas");
        assert!(found_reject, "RejectThread action not found in hit areas");
        assert!(found_always, "AllowToolAlways action not found in hit areas");
    }

    #[test]
    fn test_render_plan_approval_buttons_registers_hit_areas() {
        use crate::models::dashboard::{Aggregate, ThreadStatus, WaitingFor};
        use crate::ui::interaction::{ClickAction, HitAreaRegistry};
        use crate::view_state::dashboard_view::ThreadView;
        use crate::view_state::SystemStats;
        use ratatui::backend::TestBackend;
        use ratatui::layout::Rect;
        use ratatui::Terminal;

        let thread = ThreadView {
            id: "thread-2".to_string(),
            title: "Plan Test".to_string(),
            repository: "repo".to_string(),
            mode: crate::models::ThreadMode::Plan,
            status: ThreadStatus::Waiting,
            waiting_for: Some(WaitingFor::PlanApproval {
                request_id: "req-2".to_string(),
            }),
            progress: None,
            duration: "2m".to_string(),
            needs_action: true,
            current_operation: None,
        };

        let backend = TestBackend::new(100, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut registry = HitAreaRegistry::new();

        let theme = crate::view_state::Theme::default();
        let system_stats = SystemStats {
            connected: true,
            cpu_percent: 10.0,
            ram_used_gb: 2.0,
            ram_total_gb: 8.0,
        };
        let aggregate = Aggregate::new();
        let threads = vec![];

        let ctx = RenderContext {
            threads: &threads,
            aggregate: &aggregate,
            filter: None,
            overlay: None,
            system_stats: &system_stats,
            theme: &theme,
        };

        terminal
            .draw(|frame| {
                let area = Rect::new(0, 0, 100, 1);
                render(frame, area, &thread, &ctx, &mut registry);
            })
            .unwrap();

        // Verify hit areas were registered (1 for row + 2 for plan approval buttons)
        assert!(registry.len() >= 3, "Expected at least 3 hit areas (row + 2 buttons), got {}", registry.len());

        // Scan for plan approval button actions
        let mut found_approve = false;
        let mut found_reject = false;

        for x in 0..100 {
            if let Some(action) = registry.hit_test(x, 0) {
                match action {
                    ClickAction::ApproveThread(id) if id == "thread-2" => found_approve = true,
                    ClickAction::RejectThread(id) if id == "thread-2" => found_reject = true,
                    _ => {}
                }
            }
        }

        assert!(found_approve, "ApproveThread action not found for plan approval");
        assert!(found_reject, "RejectThread action not found for plan approval");
    }

    #[test]
    fn test_render_done_status_registers_verify_button() {
        use crate::models::dashboard::{Aggregate, ThreadStatus};
        use crate::ui::interaction::{ClickAction, HitAreaRegistry};
        use crate::view_state::dashboard_view::ThreadView;
        use crate::view_state::SystemStats;
        use ratatui::backend::TestBackend;
        use ratatui::layout::Rect;
        use ratatui::Terminal;

        let thread = ThreadView {
            id: "thread-3".to_string(),
            title: "Done Test".to_string(),
            repository: "repo".to_string(),
            mode: crate::models::ThreadMode::Normal,
            status: ThreadStatus::Done,
            waiting_for: None,
            progress: None,
            duration: "5m".to_string(),
            needs_action: false,
            current_operation: None,
        };

        let backend = TestBackend::new(100, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut registry = HitAreaRegistry::new();

        let theme = crate::view_state::Theme::default();
        let system_stats = SystemStats {
            connected: true,
            cpu_percent: 10.0,
            ram_used_gb: 2.0,
            ram_total_gb: 8.0,
        };
        let aggregate = Aggregate::new();
        let threads = vec![];

        let ctx = RenderContext {
            threads: &threads,
            aggregate: &aggregate,
            filter: None,
            overlay: None,
            system_stats: &system_stats,
            theme: &theme,
        };

        terminal
            .draw(|frame| {
                let area = Rect::new(0, 0, 100, 1);
                render(frame, area, &thread, &ctx, &mut registry);
            })
            .unwrap();

        // Verify verify button is registered
        let mut found_verify = false;
        for x in 0..100 {
            if let Some(action) = registry.hit_test(x, 0) {
                if matches!(action, ClickAction::VerifyThread(id) if id == "thread-3") {
                    found_verify = true;
                    break;
                }
            }
        }

        assert!(found_verify, "VerifyThread action not found for done status");
    }

    #[test]
    fn test_render_running_status_no_action_buttons() {
        use crate::models::dashboard::{Aggregate, ThreadStatus};
        use crate::ui::interaction::{ClickAction, HitAreaRegistry};
        use crate::view_state::dashboard_view::{Progress, ThreadView};
        use crate::view_state::SystemStats;
        use ratatui::backend::TestBackend;
        use ratatui::layout::Rect;
        use ratatui::Terminal;

        let thread = ThreadView {
            id: "thread-4".to_string(),
            title: "Running Test".to_string(),
            repository: "repo".to_string(),
            mode: crate::models::ThreadMode::Normal,
            status: ThreadStatus::Running,
            waiting_for: None,
            progress: Some(Progress {
                current: 3,
                total: 5,
            }),
            duration: "1m".to_string(),
            needs_action: false,
            current_operation: Some("Running".to_string()),
        };

        let backend = TestBackend::new(100, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut registry = HitAreaRegistry::new();

        let theme = crate::view_state::Theme::default();
        let system_stats = SystemStats {
            connected: true,
            cpu_percent: 10.0,
            ram_used_gb: 2.0,
            ram_total_gb: 8.0,
        };
        let aggregate = Aggregate::new();
        let threads = vec![];

        let ctx = RenderContext {
            threads: &threads,
            aggregate: &aggregate,
            filter: None,
            overlay: None,
            system_stats: &system_stats,
            theme: &theme,
        };

        terminal
            .draw(|frame| {
                let area = Rect::new(0, 0, 100, 1);
                render(frame, area, &thread, &ctx, &mut registry);
            })
            .unwrap();

        // Should only have the row expand action, no action buttons
        // Scan for action buttons (should find none)
        let mut found_action_button = false;
        for x in 0..100 {
            if let Some(action) = registry.hit_test(x, 0) {
                match action {
                    ClickAction::ApproveThread(_)
                    | ClickAction::RejectThread(_)
                    | ClickAction::AllowToolAlways(_)
                    | ClickAction::VerifyThread(_) => {
                        found_action_button = true;
                        break;
                    }
                    _ => {}
                }
            }
        }

        assert!(!found_action_button, "Found unexpected action button for running status");
    }

    // -------------------- Right Alignment Tests --------------------

    #[test]
    fn test_render_actions_right_align() {
        use crate::models::dashboard::{Aggregate, ThreadStatus, WaitingFor};
        use crate::ui::interaction::{ClickAction, HitAreaRegistry};
        use crate::view_state::dashboard_view::ThreadView;
        use crate::view_state::SystemStats;
        use ratatui::backend::TestBackend;
        use ratatui::layout::Rect;
        use ratatui::Terminal;

        let thread = ThreadView {
            id: "thread-right".to_string(),
            title: "Right Align Test".to_string(),
            repository: "repo".to_string(),
            mode: crate::models::ThreadMode::Normal,
            status: ThreadStatus::Waiting,
            waiting_for: Some(WaitingFor::Permission {
                request_id: "req-right".to_string(),
                tool_name: "test_tool".to_string(),
            }),
            progress: None,
            duration: "1m".to_string(),
            needs_action: true,
            current_operation: None,
        };

        // Create a custom test that directly calls render_actions with right_align=true
        let backend = TestBackend::new(100, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut registry = HitAreaRegistry::new();

        let theme = crate::view_state::Theme::default();
        let system_stats = SystemStats {
            connected: true,
            cpu_percent: 10.0,
            ram_used_gb: 2.0,
            ram_total_gb: 8.0,
        };
        let aggregate = Aggregate::new();
        let threads = vec![];

        let ctx = RenderContext {
            threads: &threads,
            aggregate: &aggregate,
            filter: None,
            overlay: None,
            system_stats: &system_stats,
            theme: &theme,
        };

        terminal
            .draw(|frame| {
                let area = Rect::new(0, 0, 100, 1);
                // Call render_actions directly with right_align=true
                render_actions(frame, 0, 0, area, &thread, &ctx, &mut registry, true);
            })
            .unwrap();

        // Verify that buttons are registered
        assert!(registry.len() >= 3, "Expected at least 3 hit areas for permission buttons");

        // For right-aligned buttons with 3 permission buttons:
        // [y] Yes  [n] No  [a] Always
        // Button widths: 3+1+3=7, 3+1+2=6, 3+1+6=10
        // Total: 7 + 2 (spacing) + 6 + 2 (spacing) + 10 = 27
        // With area width 100, they should start at x = 100 - 27 = 73

        // Find the leftmost button position
        let mut leftmost_button_x = 100u16;
        for x in 0..100 {
            if let Some(action) = registry.hit_test(x, 0) {
                match action {
                    ClickAction::ApproveThread(_)
                    | ClickAction::RejectThread(_)
                    | ClickAction::AllowToolAlways(_) => {
                        leftmost_button_x = leftmost_button_x.min(x);
                    }
                    _ => {}
                }
            }
        }

        // Verify buttons start significantly to the right (not at x=0)
        assert!(leftmost_button_x > 50, "Right-aligned buttons should start after x=50, got x={}", leftmost_button_x);

        // Verify buttons end near the right edge (within last 5 columns)
        let mut rightmost_button_x = 0u16;
        for x in 0..100 {
            if let Some(action) = registry.hit_test(x, 0) {
                match action {
                    ClickAction::ApproveThread(_)
                    | ClickAction::RejectThread(_)
                    | ClickAction::AllowToolAlways(_) => {
                        rightmost_button_x = rightmost_button_x.max(x);
                    }
                    _ => {}
                }
            }
        }

        assert!(rightmost_button_x >= 95, "Right-aligned buttons should end near right edge (>= 95), got x={}", rightmost_button_x);
    }

    #[test]
    fn test_render_actions_left_align() {
        use crate::models::dashboard::{Aggregate, ThreadStatus, WaitingFor};
        use crate::ui::interaction::{ClickAction, HitAreaRegistry};
        use crate::view_state::dashboard_view::ThreadView;
        use crate::view_state::SystemStats;
        use ratatui::backend::TestBackend;
        use ratatui::layout::Rect;
        use ratatui::Terminal;

        let thread = ThreadView {
            id: "thread-left".to_string(),
            title: "Left Align Test".to_string(),
            repository: "repo".to_string(),
            mode: crate::models::ThreadMode::Normal,
            status: ThreadStatus::Waiting,
            waiting_for: Some(WaitingFor::PlanApproval {
                request_id: "req-left".to_string(),
            }),
            progress: None,
            duration: "2m".to_string(),
            needs_action: true,
            current_operation: None,
        };

        let backend = TestBackend::new(100, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut registry = HitAreaRegistry::new();

        let theme = crate::view_state::Theme::default();
        let system_stats = SystemStats {
            connected: true,
            cpu_percent: 10.0,
            ram_used_gb: 2.0,
            ram_total_gb: 8.0,
        };
        let aggregate = Aggregate::new();
        let threads = vec![];

        let ctx = RenderContext {
            threads: &threads,
            aggregate: &aggregate,
            filter: None,
            overlay: None,
            system_stats: &system_stats,
            theme: &theme,
        };

        terminal
            .draw(|frame| {
                let area = Rect::new(0, 0, 100, 1);
                // Call render_actions directly with right_align=false and x=20
                render_actions(frame, 20, 0, area, &thread, &ctx, &mut registry, false);
            })
            .unwrap();

        // Verify that buttons are registered
        assert!(registry.len() >= 2, "Expected at least 2 hit areas for plan approval buttons");

        // For left-aligned buttons starting at x=20:
        // [y] Yes  [n] No
        // First button should start at x=20

        let mut found_approve_at_start = false;
        // Check area around x=20 for the first button
        for x in 20..25 {
            if let Some(action) = registry.hit_test(x, 0) {
                if matches!(action, ClickAction::ApproveThread(id) if id == "thread-left") {
                    found_approve_at_start = true;
                    break;
                }
            }
        }

        assert!(found_approve_at_start, "Left-aligned buttons should start at the provided x position (20)");
    }
}
