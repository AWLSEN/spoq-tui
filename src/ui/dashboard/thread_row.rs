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

    // Calculate column widths as percentages of area.width
    let title_width = ((area.width as f32) * 0.25) as u16;
    let repo_width = ((area.width as f32) * 0.12) as u16;
    let mode_width = ((area.width as f32) * 0.08) as u16;
    let status_width = ((area.width as f32) * 0.12) as u16;
    let progress_width = ((area.width as f32) * 0.15) as u16;
    let time_width = ((area.width as f32) * 0.08) as u16;

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

    // Time column
    let time_text = truncate(&thread.duration, time_width.saturating_sub(1) as usize);
    let time_style = Style::default().fg(ctx.theme.dim);
    render_text(buf, x, y, &time_text, time_style, area);
    x += time_width;

    // Action buttons (based on status and waiting_for)
    render_actions(frame, x, y, area, thread, ctx, registry);
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
) {
    let buf = frame.buffer_mut();
    let mut current_x = x;

    // Determine which buttons to show
    let buttons = match (&thread.status, &thread.waiting_for) {
        // Waiting + Permission or PlanApproval -> [approve] [reject]
        (ThreadStatus::Waiting, Some(WaitingFor::Permission { .. }))
        | (ThreadStatus::Waiting, Some(WaitingFor::PlanApproval { .. })) => {
            vec![("[approve]", ButtonAction::Approve), ("[reject]", ButtonAction::Reject)]
        }
        // Waiting + UserInput -> [answer]
        (ThreadStatus::Waiting, Some(WaitingFor::UserInput)) => {
            vec![("[answer]", ButtonAction::Answer)]
        }
        // Done -> [verify]
        (ThreadStatus::Done, _) => {
            vec![("[verify]", ButtonAction::Verify)]
        }
        // Idle/Running/Error -> no buttons
        _ => vec![],
    };

    for (label, action) in buttons {
        let label_len = label.len() as u16;

        // Check if there's room for this button
        if current_x + label_len > area.x + area.width {
            break;
        }

        // Render button text
        let button_style = Style::default().fg(ctx.theme.accent);
        render_text(buf, current_x, y, label, button_style, area);

        // Register hit area for button
        let button_rect = Rect::new(current_x, y, label_len, 1);
        let click_action = match action {
            ButtonAction::Approve => ClickAction::ApproveThread(thread.id.clone()),
            ButtonAction::Reject => ClickAction::RejectThread(thread.id.clone()),
            ButtonAction::Answer => ClickAction::ExpandThread {
                thread_id: thread.id.clone(),
                anchor_y: y,
            },
            ButtonAction::Verify => ClickAction::VerifyThread(thread.id.clone()),
        };
        registry.register(button_rect, click_action, Some(button_style));

        current_x += label_len + 1; // +1 for spacing between buttons
    }
}

/// Internal enum for button types
enum ButtonAction {
    Approve,
    Reject,
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
}
