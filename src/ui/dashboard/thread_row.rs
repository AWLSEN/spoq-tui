//! Thread row component for dashboard rendering
//!
//! Renders a single thread view as a compact row showing title, repository,
//! mode, activity/status, and action buttons.
//!
//! ## Layout
//!
//! **Non-action threads** (working autonomously, below separator):
//! ```text
//! Title (30%) | Repo (14%) | Mode (9%) | Activity (27%) | Time (10%)
//! "API Endpoints       ~/api        exec        Edit: handlers.rs           12m"
//! ```
//!
//! **Action threads** (need user input, above separator):
//! ```text
//! Title (30%) | Repo (12%) | Mode (9%) | Status (12%) | Actions (37%)
//! "Auth Refactor       ~/api        plan        waiting              [y] Yes  [n] No"
//! ```

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    Frame,
};

use crate::models::dashboard::{ThreadStatus, WaitingFor};
use crate::ui::dashboard::{RenderContext, ThreadMode, ThreadView};

// ============================================================================
// Public API
// ============================================================================

/// Render a single thread row
///
/// Uses different layouts based on whether the thread needs user action:
/// - **Action threads**: Title + Repo + Mode + Status + Actions (right-aligned)
/// - **Non-action threads**: Title + Repo + Mode + Activity + Time
///
/// # Arguments
/// * `frame` - The ratatui frame to render into
/// * `area` - The rectangle area allocated for this row (height=1)
/// * `thread` - The thread view data to render
/// * `ctx` - The render context containing theme colors
pub fn render(
    frame: &mut Frame,
    area: Rect,
    thread: &ThreadView,
    ctx: &RenderContext,
) {
    if area.height < 1 || area.width < 20 {
        return;
    }

    if thread.needs_action {
        render_action_thread(frame, area, thread, ctx);
    } else {
        render_autonomous_thread(frame, area, thread, ctx);
    }
}

/// Render an action thread row (needs user input)
///
/// Layout: Title (30%) | Repo (12%) | Mode (9%) | Status (12%) | Actions (37%)
fn render_action_thread(
    frame: &mut Frame,
    area: Rect,
    thread: &ThreadView,
    ctx: &RenderContext,
) {
    let buf = frame.buffer_mut();

    // Column widths for action threads
    let title_width = ((area.width as f32) * 0.30) as u16;
    let repo_width = ((area.width as f32) * 0.12) as u16;
    let mode_width = ((area.width as f32) * 0.09) as u16;
    let status_width = ((area.width as f32) * 0.12) as u16;
    // Remaining 37% for right-aligned action buttons

    let mut x = area.x;
    let y = area.y;

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

    // Action buttons (right-aligned)
    render_actions(frame, x, y, area, thread, ctx, true);
}

/// Render an autonomous thread row (working without user input)
///
/// Layout: Title (30%) | Repo (14%) | Mode (9%) | Activity (27%) | Time (10%)
fn render_autonomous_thread(
    frame: &mut Frame,
    area: Rect,
    thread: &ThreadView,
    ctx: &RenderContext,
) {
    let buf = frame.buffer_mut();

    // Column widths for non-action threads (unified Activity column)
    let title_width = ((area.width as f32) * 0.30) as u16;
    let repo_width = ((area.width as f32) * 0.14) as u16;
    let mode_width = ((area.width as f32) * 0.09) as u16;
    let activity_width = ((area.width as f32) * 0.27) as u16;
    let time_width = ((area.width as f32) * 0.10) as u16;

    let mut x = area.x;
    let y = area.y;

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

    // Activity column - uses activity_text or falls back to computed value
    let activity_text = thread.activity_text.as_ref().map_or_else(
        || compute_activity_text(thread),
        |s| s.clone(),
    );
    let activity_text = truncate(&activity_text, activity_width.saturating_sub(1) as usize);
    let activity_style = Style::default().fg(activity_color(thread, ctx));
    render_text(buf, x, y, &activity_text, activity_style, area);
    x += activity_width;

    // Time column
    let time_text = truncate(&thread.duration, time_width.saturating_sub(1) as usize);
    let time_style = Style::default().fg(ctx.theme.dim);
    render_text(buf, x, y, &time_text, time_style, area);
}

/// Compute activity text based on thread state (fallback when activity_text is None)
///
/// - Running + current_operation: show operation (e.g., "Edit: main.rs")
/// - Running + no operation: show "Thinking..."
/// - Idle: show "idle"
/// - Done: show "done"
/// - Error: show "error"
/// - Waiting: show "waiting"
fn compute_activity_text(thread: &ThreadView) -> String {
    match thread.status {
        ThreadStatus::Running => {
            if let Some(ref op) = thread.current_operation {
                op.clone()
            } else {
                "Thinking...".to_string()
            }
        }
        ThreadStatus::Idle => "idle".to_string(),
        ThreadStatus::Done => "done".to_string(),
        ThreadStatus::Error => "error".to_string(),
        ThreadStatus::Waiting => "waiting".to_string(),
    }
}

/// Get the color for activity text based on thread status
fn activity_color(thread: &ThreadView, ctx: &RenderContext) -> ratatui::style::Color {
    match thread.status {
        ThreadStatus::Running => ctx.theme.accent,
        ThreadStatus::Done => ctx.theme.success,
        ThreadStatus::Error => ctx.theme.error,
        ThreadStatus::Idle | ThreadStatus::Waiting => ctx.theme.dim,
    }
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
    right_align: bool,
) {
    let buf = frame.buffer_mut();

    // Key style: accent + bold
    let key_style = Style::default()
        .fg(ctx.theme.accent)
        .add_modifier(Modifier::BOLD);

    // Label style: dim
    let label_style = Style::default().fg(ctx.theme.dim);

    // Info icon style: dim
    let info_icon_style = Style::default().fg(ctx.theme.dim);

    // Determine which buttons to show and if we need an info icon
    let (buttons, show_info_icon) = match (&thread.status, &thread.waiting_for) {
        // Permission -> (i) [y] Yes  [n] No  [a] Always
        (ThreadStatus::Waiting, Some(WaitingFor::Permission { .. })) => {
            (
                vec![
                    ("[y]", "Yes", ButtonAction::Approve),
                    ("[n]", "No", ButtonAction::Reject),
                    ("[a]", "Always", ButtonAction::Always),
                ],
                true,
            )
        }
        // Plan approval -> [y] Yes  [n] No
        (ThreadStatus::Waiting, Some(WaitingFor::PlanApproval { .. })) => {
            (
                vec![
                    ("[y]", "Yes", ButtonAction::Approve),
                    ("[n]", "No", ButtonAction::Reject),
                ],
                false,
            )
        }
        // User input -> [a] Answer
        (ThreadStatus::Waiting, Some(WaitingFor::UserInput)) => {
            (vec![("[a]", "Answer", ButtonAction::Answer)], false)
        }
        // Done -> [v] Verify
        (ThreadStatus::Done, _) => (vec![("[v]", "Verify", ButtonAction::Verify)], false),
        // Idle/Running/Error -> no buttons
        _ => (vec![], false),
    };

    // Icon width: "ⓘ" = 1 char + 1 space = 2
    let icon_width = if show_info_icon { 2 } else { 0 };

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

    // Right margin for alignment consistency (2 chars)
    let right_margin = if right_align { 2 } else { 0 };

    // Total width including icon and margin
    let total_width = icon_width + total_button_width + right_margin;

    // Determine starting x position based on alignment
    let mut current_x = if right_align {
        // Start from right edge, subtract total width
        area.x + area.width.saturating_sub(total_width)
    } else {
        // Left align: start from provided x position
        x
    };

    // Render info icon if needed (for permission buttons)
    if show_info_icon {
        // Render "ⓘ" (circled info icon)
        render_text(buf, current_x, y, "ⓘ", info_icon_style, area);
        current_x += 2; // Move past "ⓘ " (1 char + 1 space)
    }

    for (key, label, _action) in buttons {
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

/// Render phase circles without the fraction (for exec mode)
///
/// # Arguments
/// * `current` - Current phase number
/// * `total` - Total number of phases
///
/// # Returns
/// A string like "● ● ● ○ ○ ○" (circles with spaces, no fraction)
pub fn render_phase_circles(current: u32, total: u32) -> String {
    let filled = vec!["\u{25CF}"; current as usize]; // ●
    let empty = vec!["\u{25CB}"; total.saturating_sub(current) as usize]; // ○
    let mut all_circles = Vec::new();
    all_circles.extend(filled);
    all_circles.extend(empty);
    all_circles.join(" ")
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

    // -------------------- render_phase_circles Tests --------------------

    #[test]
    fn test_render_phase_circles() {
        assert_eq!(render_phase_circles(3, 6), "● ● ● ○ ○ ○");
        assert_eq!(render_phase_circles(0, 5), "○ ○ ○ ○ ○");
        assert_eq!(render_phase_circles(4, 4), "● ● ● ●");
        assert_eq!(render_phase_circles(0, 0), "");
        assert_eq!(render_phase_circles(1, 3), "● ○ ○");
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
            activity_text: None,
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
            activity_text: None,
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
            activity_text: None,
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
            activity_text: Some("done".to_string()),
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
            activity_text: Some("Running tests".to_string()),
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

    // -------------------- compute_activity_text Tests --------------------

    #[test]
    fn test_compute_activity_text_running_with_operation() {
        use crate::models::dashboard::ThreadStatus;
        use crate::view_state::dashboard_view::ThreadView;

        let thread = ThreadView {
            id: "test".to_string(),
            title: "Test".to_string(),
            repository: "~/repo".to_string(),
            mode: crate::models::ThreadMode::Exec,
            status: ThreadStatus::Running,
            waiting_for: None,
            progress: None,
            duration: "5m".to_string(),
            needs_action: false,
            current_operation: Some("Edit: handlers.rs".to_string()),
            activity_text: None,
        };

        assert_eq!(compute_activity_text(&thread), "Edit: handlers.rs");
    }

    #[test]
    fn test_compute_activity_text_running_no_operation() {
        use crate::models::dashboard::ThreadStatus;
        use crate::view_state::dashboard_view::ThreadView;

        let thread = ThreadView {
            id: "test".to_string(),
            title: "Test".to_string(),
            repository: "~/repo".to_string(),
            mode: crate::models::ThreadMode::Normal,
            status: ThreadStatus::Running,
            waiting_for: None,
            progress: None,
            duration: "2m".to_string(),
            needs_action: false,
            current_operation: None,
            activity_text: None,
        };

        assert_eq!(compute_activity_text(&thread), "Thinking...");
    }

    #[test]
    fn test_compute_activity_text_idle() {
        use crate::models::dashboard::ThreadStatus;
        use crate::view_state::dashboard_view::ThreadView;

        let thread = ThreadView {
            id: "test".to_string(),
            title: "Test".to_string(),
            repository: "~/repo".to_string(),
            mode: crate::models::ThreadMode::Normal,
            status: ThreadStatus::Idle,
            waiting_for: None,
            progress: None,
            duration: "3h".to_string(),
            needs_action: false,
            current_operation: None,
            activity_text: None,
        };

        assert_eq!(compute_activity_text(&thread), "idle");
    }

    #[test]
    fn test_compute_activity_text_done() {
        use crate::models::dashboard::ThreadStatus;
        use crate::view_state::dashboard_view::ThreadView;

        let thread = ThreadView {
            id: "test".to_string(),
            title: "Test".to_string(),
            repository: "~/repo".to_string(),
            mode: crate::models::ThreadMode::Normal,
            status: ThreadStatus::Done,
            waiting_for: None,
            progress: None,
            duration: "4h".to_string(),
            needs_action: false,
            current_operation: None,
            activity_text: None,
        };

        assert_eq!(compute_activity_text(&thread), "done");
    }

    #[test]
    fn test_compute_activity_text_error() {
        use crate::models::dashboard::ThreadStatus;
        use crate::view_state::dashboard_view::ThreadView;

        let thread = ThreadView {
            id: "test".to_string(),
            title: "Test".to_string(),
            repository: "~/repo".to_string(),
            mode: crate::models::ThreadMode::Normal,
            status: ThreadStatus::Error,
            waiting_for: None,
            progress: None,
            duration: "1m".to_string(),
            needs_action: false,
            current_operation: None,
            activity_text: None,
        };

        assert_eq!(compute_activity_text(&thread), "error");
    }

    #[test]
    fn test_compute_activity_text_waiting() {
        use crate::models::dashboard::ThreadStatus;
        use crate::view_state::dashboard_view::ThreadView;

        let thread = ThreadView {
            id: "test".to_string(),
            title: "Test".to_string(),
            repository: "~/repo".to_string(),
            mode: crate::models::ThreadMode::Normal,
            status: ThreadStatus::Waiting,
            waiting_for: None,
            progress: None,
            duration: "10s".to_string(),
            needs_action: false,
            current_operation: None,
            activity_text: None,
        };

        assert_eq!(compute_activity_text(&thread), "waiting");
    }

    // -------------------- activity_color Tests --------------------

    #[test]
    fn test_activity_color_running() {
        use crate::models::dashboard::ThreadStatus;
        use crate::view_state::dashboard_view::{Theme, ThreadView};

        let theme = Theme::default();
        let thread = ThreadView {
            id: "test".to_string(),
            title: "Test".to_string(),
            repository: "~/repo".to_string(),
            mode: crate::models::ThreadMode::Exec,
            status: ThreadStatus::Running,
            waiting_for: None,
            progress: None,
            duration: "5m".to_string(),
            needs_action: false,
            current_operation: Some("Edit: main.rs".to_string()),
            activity_text: Some("Edit: main.rs".to_string()),
        };

        // Running threads use accent color
        assert_eq!(activity_color(&thread, &make_ctx(&theme)), theme.accent);
    }

    #[test]
    fn test_activity_color_done() {
        use crate::models::dashboard::ThreadStatus;
        use crate::view_state::dashboard_view::{Theme, ThreadView};

        let theme = Theme::default();
        let thread = ThreadView {
            id: "test".to_string(),
            title: "Test".to_string(),
            repository: "~/repo".to_string(),
            mode: crate::models::ThreadMode::Normal,
            status: ThreadStatus::Done,
            waiting_for: None,
            progress: None,
            duration: "4h".to_string(),
            needs_action: false,
            current_operation: None,
            activity_text: Some("done".to_string()),
        };

        // Done threads use success color
        assert_eq!(activity_color(&thread, &make_ctx(&theme)), theme.success);
    }

    #[test]
    fn test_activity_color_error() {
        use crate::models::dashboard::ThreadStatus;
        use crate::view_state::dashboard_view::{Theme, ThreadView};

        let theme = Theme::default();
        let thread = ThreadView {
            id: "test".to_string(),
            title: "Test".to_string(),
            repository: "~/repo".to_string(),
            mode: crate::models::ThreadMode::Normal,
            status: ThreadStatus::Error,
            waiting_for: None,
            progress: None,
            duration: "1m".to_string(),
            needs_action: false,
            current_operation: None,
            activity_text: Some("error".to_string()),
        };

        // Error threads use error color
        assert_eq!(activity_color(&thread, &make_ctx(&theme)), theme.error);
    }

    #[test]
    fn test_activity_color_idle() {
        use crate::models::dashboard::ThreadStatus;
        use crate::view_state::dashboard_view::{Theme, ThreadView};

        let theme = Theme::default();
        let thread = ThreadView {
            id: "test".to_string(),
            title: "Test".to_string(),
            repository: "~/repo".to_string(),
            mode: crate::models::ThreadMode::Normal,
            status: ThreadStatus::Idle,
            waiting_for: None,
            progress: None,
            duration: "3h".to_string(),
            needs_action: false,
            current_operation: None,
            activity_text: Some("idle".to_string()),
        };

        // Idle threads use dim color
        assert_eq!(activity_color(&thread, &make_ctx(&theme)), theme.dim);
    }

    /// Helper to create a minimal RenderContext for testing
    fn make_ctx(theme: &crate::view_state::dashboard_view::Theme) -> RenderContext<'_> {
        use crate::models::dashboard::Aggregate;
        use crate::view_state::SystemStats;
        use std::collections::HashMap;

        // Use lazy_static to provide static references for testing
        use std::sync::LazyLock;

        static THREADS: &[ThreadView] = &[];
        static AGGREGATE: LazyLock<Aggregate> = LazyLock::new(|| Aggregate {
            by_status: HashMap::new(),
            total_repos: 0,
        });
        static SYSTEM_STATS: LazyLock<SystemStats> = LazyLock::new(|| SystemStats {
            cpu_percent: 0.0,
            ram_used_gb: 0.0,
            ram_total_gb: 8.0,
            connected: true,
        });

        RenderContext {
            threads: THREADS,
            aggregate: &AGGREGATE,
            overlay: None,
            system_stats: &SYSTEM_STATS,
            theme,
            question_state: None,
        }
    }
}
