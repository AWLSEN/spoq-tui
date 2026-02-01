//! Plan mode event rendering for conversation view.
//!
//! Provides `render_plan_approval` to show plan summary with approve/reject options.

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use crate::markdown::MarkdownCache;
use crate::models::dashboard::PlanSummary;

use super::super::layout::LayoutContext;

/// Color for plan mode elements (matches [PLAN] header)
const COLOR_PLAN: Color = Color::Magenta;

/// Color for phase numbers
const COLOR_PHASE: Color = Color::Cyan;

/// Render plan approval prompt with full plan content
///
/// When plan_content is available, renders the full markdown content with borders.
/// Falls back to phase list when content is unavailable.
///
/// # Display format (with content)
/// ```text
/// │
///   ◈ Plan ready for approval
/// │
/// ├────────────────────────────────────────────────────────
/// │ # Plan Title
/// │
/// │ ## Summary
/// │ Description of the plan...
/// │
/// │ ## Implementation Steps
/// │ 1. First step
/// │ 2. Second step
/// ├────────────────────────────────────────────────────────
/// │
/// │   Saved to: ~/.claude/plans/plan-abc123.md
/// │
/// │   [y] approve and continue    [n] reject
/// │
/// ```
pub fn render_plan_approval(
    summary: &PlanSummary,
    ctx: &LayoutContext,
    markdown_cache: &mut MarkdownCache,
    selected_action: usize,
    feedback_active: bool,
    feedback_text: &str,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    // Header line
    lines.push(Line::raw("│"));
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled("◈ ", Style::default().fg(COLOR_PLAN)),
        Span::styled(
            "Plan ready for approval",
            Style::default().fg(COLOR_PLAN).add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::raw("│"));

    // Calculate separator width
    let separator_width = ctx.text_wrap_width(0) as usize;

    // Top separator
    lines.push(Line::from(vec![
        Span::styled("├", Style::default().fg(Color::DarkGray)),
        Span::styled(
            "─".repeat(separator_width.saturating_sub(1)),
            Style::default().fg(Color::DarkGray),
        ),
    ]));

    // Plan content (markdown rendered) or fallback
    if let Some(content) = &summary.plan_content {
        if !content.trim().is_empty() {
            let rendered = markdown_cache.render(content);
            for line in rendered.iter() {
                // Prefix each line with │
                let mut prefixed = vec![Span::styled("│ ", Style::default().fg(Color::DarkGray))];
                prefixed.extend(line.spans.iter().cloned());
                lines.push(Line::from(prefixed));
            }
        } else {
            // Content exists but is empty
            lines.push(Line::from(vec![
                Span::styled("│ ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    "(Plan content is empty)",
                    Style::default().fg(Color::Yellow),
                ),
            ]));
        }
    } else {
        // Fallback: show title and phases with notice
        lines.push(Line::from(vec![
            Span::styled("│ ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                summary.title.clone(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(vec![Span::styled(
            "│",
            Style::default().fg(Color::DarkGray),
        )]));

        // Notice about content unavailability
        lines.push(Line::from(vec![
            Span::styled("│ ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "(Full plan content unavailable - showing summary)",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::ITALIC),
            ),
        ]));
        lines.push(Line::from(vec![Span::styled(
            "│",
            Style::default().fg(Color::DarkGray),
        )]));

        // Show phases as fallback
        for (i, phase) in summary.phases.iter().enumerate() {
            lines.push(Line::from(vec![
                Span::styled("│ ", Style::default().fg(Color::DarkGray)),
                Span::styled(format!("{}. ", i + 1), Style::default().fg(COLOR_PHASE)),
                Span::raw(phase.clone()),
            ]));
        }
    }

    // Bottom separator
    lines.push(Line::from(vec![
        Span::styled("├", Style::default().fg(Color::DarkGray)),
        Span::styled(
            "─".repeat(separator_width.saturating_sub(1)),
            Style::default().fg(Color::DarkGray),
        ),
    ]));
    lines.push(Line::raw("│"));

    // File path (if available)
    if let Some(path) = &summary.plan_file_path {
        lines.push(Line::from(vec![
            Span::raw("│   "),
            Span::styled("Saved to: ", Style::default().fg(Color::DarkGray)),
            Span::styled(path.clone(), Style::default().fg(Color::DarkGray)),
        ]));
        lines.push(Line::raw("│"));
    }

    // Vertical action selection
    // Action 0: Approve
    if selected_action == 0 {
        lines.push(Line::from(vec![
            Span::raw("│   "),
            Span::styled("> ", Style::default().fg(Color::Green)),
            Span::styled("Approve", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::raw("│   "),
            Span::raw("  "),
            Span::styled("Approve", Style::default().fg(Color::DarkGray)),
        ]));
    }

    // Action 1: Reject
    if selected_action == 1 {
        lines.push(Line::from(vec![
            Span::raw("│   "),
            Span::styled("> ", Style::default().fg(Color::Red)),
            Span::styled("Reject", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::raw("│   "),
            Span::raw("  "),
            Span::styled("Reject", Style::default().fg(Color::DarkGray)),
        ]));
    }

    // Action 2: Feedback
    if feedback_active {
        // Show active feedback text input with cursor
        lines.push(Line::from(vec![
            Span::raw("│   "),
            Span::raw("  "),
            Span::styled("Feedback: ", Style::default().fg(Color::Yellow)),
            Span::raw(feedback_text.to_string()),
            Span::styled("\u{2588}", Style::default().fg(Color::Yellow)),
        ]));
    } else if selected_action == 2 {
        lines.push(Line::from(vec![
            Span::raw("│   "),
            Span::styled("> ", Style::default().fg(Color::Yellow)),
            Span::styled("Feedback", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::raw("│   "),
            Span::raw("  "),
            Span::styled("Feedback", Style::default().fg(Color::DarkGray)),
        ]));
    }

    lines.push(Line::raw("│"));

    // Help text
    let help_text = if feedback_active {
        "enter submit  esc cancel"
    } else {
        "↑↓ navigate  enter select"
    };
    lines.push(Line::from(vec![
        Span::raw("│   "),
        Span::styled(help_text, Style::default().fg(Color::DarkGray)),
    ]));

    lines.push(Line::raw("│"));

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_plan_approval_with_content() {
        let summary = PlanSummary::with_content(
            "Test Plan".to_string(),
            vec!["Phase 1".to_string(), "Phase 2".to_string()],
            5,
            Some(1000),
            Some("/path/to/plan.md".to_string()),
            Some("# Test Plan\n\nThis is the plan content.".to_string()),
        );
        let ctx = LayoutContext::new(100, 40);
        let mut cache = MarkdownCache::new();
        let lines = render_plan_approval(&summary, &ctx, &mut cache, 0, false, "");

        // Should have header, separators, content, file path, actions
        assert!(lines.len() >= 8);

        // Check header contains "Plan ready for approval"
        let header_str = lines[1].to_string();
        assert!(header_str.contains("Plan ready for approval"));

        // Check content is rendered (markdown renders "# Test Plan" as styled text)
        let full_text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(full_text.contains("Test Plan"));
        assert!(full_text.contains("plan content"));

        // Check file path is shown
        assert!(full_text.contains("/path/to/plan.md"));
    }

    #[test]
    fn test_render_plan_approval_fallback_without_content() {
        let summary = PlanSummary::new(
            "Test Plan".to_string(),
            vec!["Phase 1".to_string(), "Phase 2".to_string()],
            5,
            Some(1000),
        );
        let ctx = LayoutContext::new(100, 40);
        let mut cache = MarkdownCache::new();
        let lines = render_plan_approval(&summary, &ctx, &mut cache, 0, false, "");

        let full_text: String = lines.iter().map(|l| l.to_string()).collect();

        // Should show fallback notice
        assert!(full_text.contains("unavailable"));

        // Should show phases as fallback
        assert!(full_text.contains("1. Phase 1"));
        assert!(full_text.contains("2. Phase 2"));
    }

    #[test]
    fn test_render_plan_approval_shows_action_hints() {
        let summary = PlanSummary::new(
            "Test".to_string(),
            vec!["Phase 1".to_string()],
            1,
            None,
        );
        let ctx = LayoutContext::new(100, 40);
        let mut cache = MarkdownCache::new();
        let lines = render_plan_approval(&summary, &ctx, &mut cache, 0, false, "");

        let full_text: String = lines.iter().map(|l| l.to_string()).collect();
        // Check for vertical selection UI
        assert!(full_text.contains("Approve"));
        assert!(full_text.contains("Reject"));
        assert!(full_text.contains("Feedback"));
        // Check for help text
        assert!(full_text.contains("navigate"));
    }
}
