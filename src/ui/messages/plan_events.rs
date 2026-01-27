//! Plan mode event rendering for conversation view.
//!
//! Provides two render functions:
//! - `render_planning_indicator`: Shows spinner while Claude is planning
//! - `render_plan_approval`: Shows plan summary with approve/reject options

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use crate::models::dashboard::PlanSummary;

use super::super::helpers::SPINNER_FRAMES;
use super::super::layout::LayoutContext;

/// Color for plan mode elements (matches [PLAN] header)
const COLOR_PLAN: Color = Color::Magenta;

/// Color for phase numbers
const COLOR_PHASE: Color = Color::Cyan;

/// Render planning-in-progress indicator
///
/// Shows a spinner with "Planning..." text while Claude is actively planning.
///
/// # Display format
/// ```text
/// │
///   ◈ ⣾ Planning...
/// │
/// ```
pub fn render_planning_indicator(tick_count: u64) -> Vec<Line<'static>> {
    let frame_index = (tick_count % 10) as usize;
    let spinner = SPINNER_FRAMES[frame_index];

    vec![
        Line::raw("│"),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("◈ ", Style::default().fg(COLOR_PLAN)),
            Span::styled(format!("{} ", spinner), Style::default().fg(COLOR_PLAN)),
            Span::styled("Planning...", Style::default().fg(COLOR_PLAN)),
        ]),
        Line::raw("│"),
    ]
}

/// Render plan approval prompt
///
/// Shows plan summary with phases and approve/reject action hints.
///
/// # Display format
/// ```text
/// │
///   ◈ Plan ready · 5 phases · 8 files
/// │
/// │   1. First phase description
/// │   2. Second phase description
/// │   ... (max 5 shown)
/// │
/// │   [y] approve    [n] reject
/// │
/// ```
pub fn render_plan_approval(summary: &PlanSummary, ctx: &LayoutContext) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    // Header line
    lines.push(Line::raw("│"));
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled("◈ ", Style::default().fg(COLOR_PLAN)),
        Span::styled(
            format!(
                "Plan ready · {} phases · {} files",
                summary.phases.len(),
                summary.file_count
            ),
            Style::default().fg(COLOR_PLAN).add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::raw("│"));

    // Phase list (max 5 visible)
    let max_phases = 5.min(summary.phases.len());
    for (i, phase) in summary.phases.iter().take(max_phases).enumerate() {
        // Calculate responsive max length for phase description
        let max_len = ctx.text_wrap_width(2).saturating_sub(5) as usize;
        let display = if phase.len() > max_len && max_len > 3 {
            format!("{}...", &phase[..max_len.saturating_sub(3)])
        } else {
            phase.clone()
        };

        lines.push(Line::from(vec![
            Span::raw("│   "),
            Span::styled(format!("{}. ", i + 1), Style::default().fg(COLOR_PHASE)),
            Span::raw(display),
        ]));
    }

    // Show "... +N more" if there are more than 5 phases
    if summary.phases.len() > max_phases {
        lines.push(Line::from(vec![
            Span::raw("│   "),
            Span::styled(
                format!("... +{} more", summary.phases.len() - max_phases),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    }

    // Action hints
    lines.push(Line::raw("│"));
    lines.push(Line::from(vec![
        Span::raw("│   "),
        Span::styled("[y]", Style::default().fg(Color::Green)),
        Span::raw(" approve    "),
        Span::styled("[n]", Style::default().fg(Color::Red)),
        Span::raw(" reject"),
    ]));
    lines.push(Line::raw("│"));

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_planning_indicator_creates_lines() {
        let lines = render_planning_indicator(0);
        assert_eq!(lines.len(), 3);
        // First and last lines are just vertical bars
        assert_eq!(lines[0], Line::raw("│"));
        assert_eq!(lines[2], Line::raw("│"));
    }

    #[test]
    fn test_render_planning_indicator_cycles_spinner() {
        let lines_0 = render_planning_indicator(0);
        let lines_5 = render_planning_indicator(5);
        // The spinner frame should change with tick_count
        assert_ne!(
            lines_0[1].to_string(),
            lines_5[1].to_string(),
            "Spinner should animate"
        );
    }

    #[test]
    fn test_render_plan_approval_basic() {
        let summary = PlanSummary::new(
            "Test Plan".to_string(),
            vec!["Phase 1".to_string(), "Phase 2".to_string()],
            5,
            Some(1000),
        );
        let ctx = LayoutContext::new(100, 40);
        let lines = render_plan_approval(&summary, &ctx);

        // Should have: │, header, │, phase1, phase2, │, actions, │
        assert!(lines.len() >= 7);

        // Check header contains phase count
        let header_str = lines[1].to_string();
        assert!(header_str.contains("2 phases"));
        assert!(header_str.contains("5 files"));
    }

    #[test]
    fn test_render_plan_approval_truncates_phases() {
        let summary = PlanSummary::new(
            "Test Plan".to_string(),
            vec![
                "Phase 1".to_string(),
                "Phase 2".to_string(),
                "Phase 3".to_string(),
                "Phase 4".to_string(),
                "Phase 5".to_string(),
                "Phase 6".to_string(),
                "Phase 7".to_string(),
            ],
            10,
            None,
        );
        let ctx = LayoutContext::new(100, 40);
        let lines = render_plan_approval(&summary, &ctx);

        // Should show "... +2 more" for the 2 hidden phases
        let full_text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(full_text.contains("+2 more"));
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
        let lines = render_plan_approval(&summary, &ctx);

        let full_text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(full_text.contains("[y]"));
        assert!(full_text.contains("[n]"));
        assert!(full_text.contains("approve"));
        assert!(full_text.contains("reject"));
    }
}
