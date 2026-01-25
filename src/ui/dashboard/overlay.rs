//! Overlay rendering for dashboard thread cards
//!
//! This module handles z-index overlay rendering for expanded thread cards.
//! Ratatui renders in call order, so overlays are rendered LAST to appear on top.
//!
//! ## Architecture
//!
//! The overlay system uses a specific render order to achieve proper z-indexing:
//! 1. Register CollapseOverlay hit area on list_area (click outside = close)
//! 2. Dim background behind the card
//! 3. Clear card area (solid background)
//! 4. Draw card border with rounded corners
//! 5. Delegate to content renderer based on overlay type
//!
//! ## Card Dimensions
//!
//! | Property | Value |
//! |----------|-------|
//! | **Width** | 80% of dashboard list area |
//! | **Height** | ~35% (10-12 rows, dynamic based on options count) |
//! | **Border** | Rounded Unicode: `╭╮╰╯─│` |

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::Clear,
    Frame,
};

use crate::models::dashboard::PlanSummary;
use crate::state::dashboard::DashboardQuestionState;
use crate::ui::dashboard::plan_card;
use crate::ui::dashboard::question_card::{self, QuestionRenderConfig};
use crate::ui::dashboard::{OverlayState, RenderContext};
use crate::ui::interaction::{ClickAction, HitAreaRegistry};

/// Maximum height for question card as percentage of list area
const QUESTION_CARD_MAX_HEIGHT_PERCENT: f32 = 0.35;

/// Minimum height for question card (rows)
const QUESTION_CARD_MIN_HEIGHT: u16 = 8;

/// Card width as percentage of list area
const CARD_WIDTH_PERCENT: f32 = 0.80;

/// Render an overlay card on top of the thread list.
///
/// This function handles the complete overlay rendering sequence:
/// 1. Calculates card dimensions based on overlay type
/// 2. Registers hit areas for click handling
/// 3. Dims the background
/// 4. Renders the card with appropriate content
///
/// # Arguments
///
/// * `frame` - The ratatui frame to render to
/// * `list_area` - The area of the thread list (used for positioning and backdrop)
/// * `overlay` - The overlay state containing card content
/// * `ctx` - Render context with theme and other settings
/// * `registry` - Hit area registry for click handling
#[allow(unused_variables)]
pub fn render(
    frame: &mut Frame,
    list_area: Rect,
    overlay: &OverlayState,
    ctx: &RenderContext,
    registry: &mut HitAreaRegistry,
) {
    // Calculate card dimensions based on overlay type
    let (anchor_y, card_height) = calculate_card_dimensions(overlay, list_area);

    // Card width is 80% of list area
    let card_width = ((list_area.width as f32) * CARD_WIDTH_PERCENT) as u16;
    let card_x = list_area.x + (list_area.width - card_width) / 2;
    let mut card_y = anchor_y + 1; // Below the anchor row

    // Clamp to list bounds
    if card_y + card_height > list_area.bottom() {
        card_y = list_area.bottom().saturating_sub(card_height);
    }

    let card_area = Rect::new(card_x, card_y, card_width, card_height);

    // Render sequence (z-index via call order):

    // 1. First: Register list_area as CollapseOverlay (click outside = close)
    //    This is registered FIRST so card hit areas take precedence
    registry.register(list_area, ClickAction::CollapseOverlay, None);

    // 2. Dim background behind card (semi-transparent overlay effect)
    //    Draw dim styling on rows above and below the card
    for y in list_area.y..list_area.bottom() {
        if y < card_y || y >= card_y + card_height {
            // These rows are "behind" the card - render them dimmed
            let dim_area = Rect::new(list_area.x, y, list_area.width, 1);
            frame.render_widget(
                ratatui::widgets::Block::default()
                    .style(Style::default().fg(Color::DarkGray)),
                dim_area,
            );
        }
    }

    // 3. Clear card area (solid background)
    frame.render_widget(Clear, card_area);

    // 4. Draw card border with rounded corners
    render_rounded_border(frame.buffer_mut(), card_area, Color::White, Color::Black);

    // 5. Delegate to content renderer based on overlay type
    let inner_area = Rect::new(
        card_area.x + 2, // 1 for border + 1 for padding
        card_area.y + 1,
        card_area.width.saturating_sub(4), // 2 for border + 2 for padding
        card_area.height.saturating_sub(2),
    );

    match overlay {
        OverlayState::Question {
            thread_id,
            thread_title,
            repository,
            question_data,
            ..
        } => {
            render_question_content(
                frame,
                inner_area,
                thread_id,
                thread_title,
                repository,
                question_data.as_ref(),
                ctx.question_state,
                registry,
            );
        }
        OverlayState::FreeForm {
            thread_id,
            thread_title,
            repository,
            question_data,
            input,
            cursor_pos,
            ..
        } => {
            // Extract question text from question_data for display context
            let question_text = question_data
                .as_ref()
                .and_then(|qd| qd.questions.first())
                .map(|q| q.question.clone())
                .unwrap_or_default();

            // FreeForm mode: pass Some((input, cursor_pos)) to render text input
            question_card::render(
                frame,
                inner_area,
                thread_id,
                thread_title,
                repository,
                &question_text,
                &[], // No options in FreeForm mode
                Some((input.as_str(), *cursor_pos)),
                registry,
            );
        }
        OverlayState::Plan {
            thread_id,
            thread_title,
            repository,
            request_id,
            summary,
            scroll_offset,
            ..
        } => {
            render_plan_content(
                frame,
                inner_area,
                thread_id,
                thread_title,
                repository,
                request_id,
                summary,
                *scroll_offset,
                registry,
            );
        }
    }
}

/// Calculate card dimensions based on overlay type
fn calculate_card_dimensions(overlay: &OverlayState, list_area: Rect) -> (u16, u16) {
    match overlay {
        OverlayState::Question {
            anchor_y,
            question_data,
            ..
        } => {
            // Get option count from question_data if available
            let option_count = question_data
                .as_ref()
                .and_then(|qd| qd.questions.first())
                .map(|q| q.options.len())
                .unwrap_or(0);

            // Calculate height:
            // - 1 row: header (title · repo)
            // - 1 row: blank
            // - 2 rows: question text (max)
            // - 1 row: blank
            // - N rows: options
            // - 1 row: Other option
            // - 1 row: blank
            // - 1 row: help text
            // - 2 rows: border (top + bottom)
            let content_height = 1 + 1 + 2 + 1 + option_count as u16 + 1 + 1 + 1;
            let total_height = content_height + 2; // +2 for borders

            // Apply max height constraint (~35% of list area)
            let max_height =
                ((list_area.height as f32) * QUESTION_CARD_MAX_HEIGHT_PERCENT) as u16;
            let card_height = total_height
                .max(QUESTION_CARD_MIN_HEIGHT)
                .min(max_height.max(QUESTION_CARD_MIN_HEIGHT));

            (*anchor_y, card_height)
        }
        OverlayState::FreeForm { anchor_y, .. } => {
            // Fixed height for free-form input
            // - 1 row: header
            // - 1 row: blank
            // - 1 row: question (truncated)
            // - 1 row: blank
            // - 3 rows: input box
            // - 1 row: blank
            // - 1 row: buttons
            // - 2 rows: border
            (*anchor_y, 11)
        }
        OverlayState::Plan {
            anchor_y, summary, ..
        } => {
            // Plan card height based on phases
            let phase_rows = summary.phases.len().min(5) as u16;
            (*anchor_y, 6 + phase_rows + 2)
        }
    }
}

/// Render the question content using the new QuestionRenderConfig
fn render_question_content(
    frame: &mut Frame,
    area: Rect,
    thread_id: &str,
    title: &str,
    repo: &str,
    question_data: Option<&crate::state::session::AskUserQuestionData>,
    question_state: Option<&DashboardQuestionState>,
    registry: &mut HitAreaRegistry,
) {
    // Get tab index from question state, default to 0
    let tab_index = question_state.map(|s| s.tab_index).unwrap_or(0);

    // Extract tab headers from all questions
    let tab_headers: Vec<String> = question_data
        .map(|qd| qd.questions.iter().map(|q| q.header.clone()).collect())
        .unwrap_or_default();

    // Extract question info for the current tab
    let (question_text, option_labels, multi_select) = question_data
        .and_then(|qd| qd.questions.get(tab_index))
        .map(|q| {
            let labels: Vec<String> = q.options.iter().map(|o| o.label.clone()).collect();
            (q.question.clone(), labels, q.multi_select)
        })
        .unwrap_or_else(|| (String::new(), vec![], false));

    // Get UI state for current question from question_state
    let selected_index = question_state
        .and_then(|s| s.selections.get(tab_index).copied())
        .flatten();

    let multi_selections_owned: Vec<bool> = question_state
        .and_then(|s| s.multi_selections.get(tab_index).cloned())
        .unwrap_or_default();

    let other_input = question_state
        .and_then(|s| s.other_texts.get(tab_index))
        .map(|s| s.as_str())
        .unwrap_or("");

    let other_selected = question_state
        .map(|s| s.selections.get(tab_index).copied().flatten().is_none())
        .unwrap_or(false);

    let other_active = question_state.map(|s| s.other_active).unwrap_or(false);

    let tabs_answered: Vec<bool> = question_state
        .map(|s| s.answered.clone())
        .unwrap_or_default();

    // Build the render config
    let config = QuestionRenderConfig {
        question: &question_text,
        options: &option_labels,
        selected_index: if other_selected { None } else { selected_index.or(Some(0)) },
        multi_select,
        multi_selections: &multi_selections_owned,
        other_input: if other_active { other_input } else { "" },
        other_selected,
        timer_seconds: None,
        tab_headers: &tab_headers,
        current_tab: tab_index,
        tabs_answered: &tabs_answered,
    };

    question_card::render_question(frame, area, thread_id, title, repo, &config, registry);
}

/// Render a rounded border using Unicode box-drawing characters
///
/// ```text
/// ╭───────────────────╮
/// │                   │
/// │                   │
/// ╰───────────────────╯
/// ```
fn render_rounded_border(buf: &mut Buffer, area: Rect, fg: Color, bg: Color) {
    if area.width < 2 || area.height < 2 {
        return;
    }

    let style = Style::default().fg(fg).bg(bg);

    // Top-left corner
    if let Some(cell) = buf.cell_mut((area.x, area.y)) {
        cell.set_char('\u{256d}').set_style(style);
    }

    // Top-right corner
    if let Some(cell) = buf.cell_mut((area.x + area.width - 1, area.y)) {
        cell.set_char('\u{256e}').set_style(style);
    }

    // Bottom-left corner
    if let Some(cell) = buf.cell_mut((area.x, area.y + area.height - 1)) {
        cell.set_char('\u{2570}').set_style(style);
    }

    // Bottom-right corner
    if let Some(cell) =
        buf.cell_mut((area.x + area.width - 1, area.y + area.height - 1))
    {
        cell.set_char('\u{256f}').set_style(style);
    }

    // Top and bottom horizontal lines
    for x in (area.x + 1)..(area.x + area.width - 1) {
        // Top
        if let Some(cell) = buf.cell_mut((x, area.y)) {
            cell.set_char('\u{2500}').set_style(style);
        }
        // Bottom
        if let Some(cell) = buf.cell_mut((x, area.y + area.height - 1)) {
            cell.set_char('\u{2500}').set_style(style);
        }
    }

    // Left and right vertical lines
    for y in (area.y + 1)..(area.y + area.height - 1) {
        // Left
        if let Some(cell) = buf.cell_mut((area.x, y)) {
            cell.set_char('\u{2502}').set_style(style);
        }
        // Right
        if let Some(cell) = buf.cell_mut((area.x + area.width - 1, y)) {
            cell.set_char('\u{2502}').set_style(style);
        }
    }

    // Fill interior with background
    for y in (area.y + 1)..(area.y + area.height - 1) {
        for x in (area.x + 1)..(area.x + area.width - 1) {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_char(' ').set_style(Style::default().bg(bg));
            }
        }
    }
}

/// Content renderer for plan approval overlays.
///
/// Delegates to plan_card::render() for the full plan approval preview.
#[allow(clippy::too_many_arguments)]
fn render_plan_content(
    frame: &mut Frame,
    area: Rect,
    thread_id: &str,
    title: &str,
    repo: &str,
    request_id: &str,
    summary: &PlanSummary,
    scroll_offset: usize,
    registry: &mut HitAreaRegistry,
) {
    plan_card::render(
        frame,
        area,
        thread_id,
        title,
        repo,
        request_id,
        summary,
        scroll_offset,
        registry,
    );
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::session::{AskUserQuestionData, Question, QuestionOption};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use std::collections::HashMap;

    fn create_test_question_data(option_count: usize, multi_select: bool) -> AskUserQuestionData {
        let options: Vec<QuestionOption> = (0..option_count)
            .map(|i| QuestionOption {
                label: format!("Option {}", i + 1),
                description: format!("Description {}", i + 1),
            })
            .collect();

        AskUserQuestionData {
            questions: vec![Question {
                question: "Which option should I use?".to_string(),
                header: "Choice".to_string(),
                options,
                multi_select,
            }],
            answers: HashMap::new(),
        }
    }

    // -------------------- Card Dimension Tests --------------------

    #[test]
    fn test_calculate_card_dimensions_question_basic() {
        let list_area = Rect::new(0, 0, 100, 40);
        let question_data = Some(create_test_question_data(3, false));

        let overlay = OverlayState::Question {
            thread_id: "t1".to_string(),
            thread_title: "Test".to_string(),
            repository: "~/repo".to_string(),
            question_data,
            anchor_y: 10,
        };

        let (anchor, height) = calculate_card_dimensions(&overlay, list_area);

        assert_eq!(anchor, 10);
        // Height should be reasonable for 3 options
        assert!(height >= QUESTION_CARD_MIN_HEIGHT);
        // Should not exceed 35% of list area
        let max_expected = ((40.0 * QUESTION_CARD_MAX_HEIGHT_PERCENT) as u16).max(QUESTION_CARD_MIN_HEIGHT);
        assert!(height <= max_expected);
    }

    #[test]
    fn test_calculate_card_dimensions_question_many_options() {
        let list_area = Rect::new(0, 0, 100, 30);
        let question_data = Some(create_test_question_data(10, false));

        let overlay = OverlayState::Question {
            thread_id: "t1".to_string(),
            thread_title: "Test".to_string(),
            repository: "~/repo".to_string(),
            question_data,
            anchor_y: 5,
        };

        let (_, height) = calculate_card_dimensions(&overlay, list_area);

        // Should be capped at ~35% of list area (30 * 0.35 = 10.5)
        let max_expected = ((30.0 * QUESTION_CARD_MAX_HEIGHT_PERCENT) as u16).max(QUESTION_CARD_MIN_HEIGHT);
        assert!(height <= max_expected);
    }

    #[test]
    fn test_calculate_card_dimensions_free_form() {
        let list_area = Rect::new(0, 0, 100, 40);

        let overlay = OverlayState::FreeForm {
            thread_id: "t1".to_string(),
            thread_title: "Test".to_string(),
            repository: "~/repo".to_string(),
            question_data: None,
            input: String::new(),
            cursor_pos: 0,
            anchor_y: 15,
        };

        let (anchor, height) = calculate_card_dimensions(&overlay, list_area);

        assert_eq!(anchor, 15);
        assert_eq!(height, 11); // Fixed height for free-form
    }

    #[test]
    fn test_calculate_card_dimensions_plan() {
        let list_area = Rect::new(0, 0, 100, 40);
        let summary = crate::models::dashboard::PlanSummary::new(
            "Test Plan".to_string(),
            vec![
                "Phase 1".to_string(),
                "Phase 2".to_string(),
                "Phase 3".to_string(),
            ],
            5,
            Some(10000),
        );

        let overlay = OverlayState::Plan {
            thread_id: "t1".to_string(),
            thread_title: "Test".to_string(),
            repository: "~/repo".to_string(),
            request_id: "req-1".to_string(),
            summary,
            scroll_offset: 0,
            anchor_y: 8,
        };

        let (anchor, height) = calculate_card_dimensions(&overlay, list_area);

        assert_eq!(anchor, 8);
        // 6 base + 3 phases (capped at 5) + 2
        assert_eq!(height, 6 + 3 + 2);
    }

    // -------------------- Rounded Border Tests --------------------

    #[test]
    fn test_render_rounded_border_basic() {
        let backend = TestBackend::new(20, 10);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                let area = Rect::new(2, 2, 10, 5);
                render_rounded_border(frame.buffer_mut(), area, Color::White, Color::Black);
            })
            .unwrap();

        let buf = terminal.backend().buffer();

        // Check corners
        assert_eq!(buf.cell((2, 2)).unwrap().symbol(), "\u{256d}"); // Top-left
        assert_eq!(buf.cell((11, 2)).unwrap().symbol(), "\u{256e}"); // Top-right
        assert_eq!(buf.cell((2, 6)).unwrap().symbol(), "\u{2570}"); // Bottom-left
        assert_eq!(buf.cell((11, 6)).unwrap().symbol(), "\u{256f}"); // Bottom-right
    }

    #[test]
    fn test_render_rounded_border_horizontal_lines() {
        let backend = TestBackend::new(20, 10);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                let area = Rect::new(0, 0, 10, 5);
                render_rounded_border(frame.buffer_mut(), area, Color::White, Color::Black);
            })
            .unwrap();

        let buf = terminal.backend().buffer();

        // Check horizontal lines (top and bottom, excluding corners)
        for x in 1..9 {
            assert_eq!(buf.cell((x, 0)).unwrap().symbol(), "\u{2500}"); // Top
            assert_eq!(buf.cell((x, 4)).unwrap().symbol(), "\u{2500}"); // Bottom
        }
    }

    #[test]
    fn test_render_rounded_border_vertical_lines() {
        let backend = TestBackend::new(20, 10);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                let area = Rect::new(0, 0, 10, 5);
                render_rounded_border(frame.buffer_mut(), area, Color::White, Color::Black);
            })
            .unwrap();

        let buf = terminal.backend().buffer();

        // Check vertical lines (left and right, excluding corners)
        for y in 1..4 {
            assert_eq!(buf.cell((0, y)).unwrap().symbol(), "\u{2502}"); // Left
            assert_eq!(buf.cell((9, y)).unwrap().symbol(), "\u{2502}"); // Right
        }
    }

    #[test]
    fn test_render_rounded_border_too_small() {
        let backend = TestBackend::new(5, 5);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                // Area too small - should bail out gracefully
                let area = Rect::new(0, 0, 1, 1);
                render_rounded_border(frame.buffer_mut(), area, Color::White, Color::Black);
            })
            .unwrap();

        // Should not panic
    }

    // -------------------- Full Render Tests --------------------

    #[test]
    fn test_render_overlay_question() {
        let backend = TestBackend::new(80, 30);
        let mut terminal = Terminal::new(backend).unwrap();

        let question_data = Some(create_test_question_data(3, false));
        let overlay = OverlayState::Question {
            thread_id: "t1".to_string(),
            thread_title: "Implement feature".to_string(),
            repository: "my-project".to_string(),
            question_data,
            anchor_y: 5,
        };

        let threads = vec![];
        let aggregate = crate::models::dashboard::Aggregate::default();
        let system_stats = crate::view_state::SystemStats::default();
        let theme = crate::view_state::Theme::default();
        let ctx = crate::view_state::RenderContext {
            threads: &threads,
            aggregate: &aggregate,
            filter: None,
            overlay: Some(&overlay),
            system_stats: &system_stats,
            theme: &theme,
            question_state: None,
        };

        let mut registry = HitAreaRegistry::new();

        terminal
            .draw(|frame| {
                let list_area = Rect::new(0, 0, 80, 30);
                render(frame, list_area, &overlay, &ctx, &mut registry);
            })
            .unwrap();

        // Should have registered hit areas:
        // - CollapseOverlay for background
        // - options (may be limited by card height)
        // - 1 Other (if space allows)
        // Minimum: CollapseOverlay + at least 1 option
        assert!(registry.len() >= 2, "Expected at least 2 hit areas, got {}", registry.len());
    }

    #[test]
    fn test_render_overlay_multi_select() {
        let backend = TestBackend::new(80, 30);
        let mut terminal = Terminal::new(backend).unwrap();

        let question_data = Some(create_test_question_data(4, true));
        let overlay = OverlayState::Question {
            thread_id: "t1".to_string(),
            thread_title: "Setup CI".to_string(),
            repository: "my-repo".to_string(),
            question_data,
            anchor_y: 10,
        };

        let threads = vec![];
        let aggregate = crate::models::dashboard::Aggregate::default();
        let system_stats = crate::view_state::SystemStats::default();
        let theme = crate::view_state::Theme::default();
        let ctx = crate::view_state::RenderContext {
            threads: &threads,
            aggregate: &aggregate,
            filter: None,
            overlay: Some(&overlay),
            system_stats: &system_stats,
            theme: &theme,
            question_state: None,
        };

        let mut registry = HitAreaRegistry::new();

        terminal
            .draw(|frame| {
                let list_area = Rect::new(0, 0, 80, 30);
                render(frame, list_area, &overlay, &ctx, &mut registry);
            })
            .unwrap();

        // Should have registered hit areas for multi-select options
        // Minimum: CollapseOverlay + at least 1 option
        assert!(registry.len() >= 2, "Expected at least 2 hit areas, got {}", registry.len());
    }

    #[test]
    fn test_render_overlay_free_form() {
        let backend = TestBackend::new(80, 30);
        let mut terminal = Terminal::new(backend).unwrap();

        let overlay = OverlayState::FreeForm {
            thread_id: "t1".to_string(),
            thread_title: "Enter input".to_string(),
            repository: "my-repo".to_string(),
            question_data: None,
            input: "Hello world".to_string(),
            cursor_pos: 5,
            anchor_y: 8,
        };

        let threads = vec![];
        let aggregate = crate::models::dashboard::Aggregate::default();
        let system_stats = crate::view_state::SystemStats::default();
        let theme = crate::view_state::Theme::default();
        let ctx = crate::view_state::RenderContext {
            threads: &threads,
            aggregate: &aggregate,
            filter: None,
            overlay: Some(&overlay),
            system_stats: &system_stats,
            theme: &theme,
            question_state: None,
        };

        let mut registry = HitAreaRegistry::new();

        terminal
            .draw(|frame| {
                let list_area = Rect::new(0, 0, 80, 30);
                render(frame, list_area, &overlay, &ctx, &mut registry);
            })
            .unwrap();

        // Should have: CollapseOverlay + back button + send button
        assert!(registry.len() >= 3);
    }

    #[test]
    fn test_render_overlay_card_position_clamped() {
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        // Anchor near bottom - card should be clamped to fit
        let question_data = Some(create_test_question_data(3, false));
        let overlay = OverlayState::Question {
            thread_id: "t1".to_string(),
            thread_title: "Test".to_string(),
            repository: "repo".to_string(),
            question_data,
            anchor_y: 18, // Near bottom
        };

        let threads = vec![];
        let aggregate = crate::models::dashboard::Aggregate::default();
        let system_stats = crate::view_state::SystemStats::default();
        let theme = crate::view_state::Theme::default();
        let ctx = crate::view_state::RenderContext {
            threads: &threads,
            aggregate: &aggregate,
            filter: None,
            overlay: Some(&overlay),
            system_stats: &system_stats,
            theme: &theme,
            question_state: None,
        };

        let mut registry = HitAreaRegistry::new();

        terminal
            .draw(|frame| {
                let list_area = Rect::new(0, 0, 80, 20);
                render(frame, list_area, &overlay, &ctx, &mut registry);
            })
            .unwrap();

        // Should render without panic (card clamped to fit)
    }
}
