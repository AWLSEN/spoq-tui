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
//! 4. Draw card border
//! 5. Delegate to content renderer based on overlay type

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Clear},
    Frame,
};

use crate::models::dashboard::PlanSummary;
use crate::ui::dashboard::context::{OverlayState, RenderContext};
use crate::ui::dashboard::plan_card;
use crate::ui::dashboard::question_card;
use crate::ui::interaction::{ClickAction, HitAreaRegistry};

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
    let (anchor_y, card_height) = match overlay {
        OverlayState::Question {
            anchor_y, options, ..
        } => (*anchor_y, 6 + options.len() as u16),
        OverlayState::FreeForm { anchor_y, .. } => (*anchor_y, 8),
        OverlayState::Plan {
            anchor_y, summary, ..
        } => (*anchor_y, 6 + summary.phases.len().min(5) as u16 + 2),
    };

    let card_width = (list_area.width as f32 * 0.90) as u16;
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
                Block::default().style(Style::default().fg(Color::DarkGray)),
                dim_area,
            );
        }
    }

    // 3. Clear card area (solid background)
    frame.render_widget(Clear, card_area);

    // 4. Draw card border (Block with borders)
    let card_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::White))
        .style(Style::default().bg(Color::Black));
    frame.render_widget(card_block, card_area);

    // 5. Delegate to content renderer based on overlay type
    let inner_area = Rect::new(
        card_area.x + 1,
        card_area.y + 1,
        card_area.width.saturating_sub(2),
        card_area.height.saturating_sub(2),
    );

    match overlay {
        OverlayState::Question {
            thread_id,
            thread_title,
            repository,
            question,
            options,
            ..
        } => {
            // Question mode: pass None for input to render option buttons
            question_card::render(
                frame,
                inner_area,
                thread_id,
                thread_title,
                repository,
                question,
                options,
                None,
                registry,
            );
        }
        OverlayState::FreeForm {
            thread_id,
            thread_title,
            repository,
            question,
            input,
            cursor_pos,
            ..
        } => {
            // FreeForm mode: pass Some((input, cursor_pos)) to render text input
            question_card::render(
                frame,
                inner_area,
                thread_id,
                thread_title,
                repository,
                question,
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

/// Calculate the bounding rectangle for an overlay card.
///
/// Given the list area, anchor position, and desired card height,
/// computes the card bounds with proper centering and clamping.
///
/// # Arguments
///
/// * `list_area` - The area of the thread list
/// * `anchor_y` - The Y position to anchor the card below
/// * `card_height` - The desired height of the card
///
/// # Returns
///
/// A `Rect` representing the card bounds, clamped to fit within the list area.
pub fn calculate_card_bounds(list_area: Rect, anchor_y: u16, card_height: u16) -> Rect {
    let card_width = (list_area.width as f32 * 0.90) as u16;
    let card_x = list_area.x + (list_area.width - card_width) / 2;
    let mut card_y = anchor_y + 1;

    if card_y + card_height > list_area.bottom() {
        card_y = list_area.bottom().saturating_sub(card_height);
    }

    Rect::new(card_x, card_y, card_width, card_height)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_rect(x: u16, y: u16, width: u16, height: u16) -> Rect {
        Rect::new(x, y, width, height)
    }

    // -------------------- calculate_card_bounds Tests --------------------

    #[test]
    fn test_calculate_card_bounds_basic() {
        // List area: 100 wide, starting at x=0, y=0, height 50
        let list_area = make_rect(0, 0, 100, 50);
        let anchor_y = 10;
        let card_height = 10;

        let bounds = calculate_card_bounds(list_area, anchor_y, card_height);

        // Width should be 90% of 100 = 90
        assert_eq!(bounds.width, 90);
        // X should be centered: (100 - 90) / 2 = 5
        assert_eq!(bounds.x, 5);
        // Y should be anchor + 1 = 11
        assert_eq!(bounds.y, 11);
        // Height should be as requested
        assert_eq!(bounds.height, 10);
    }

    #[test]
    fn test_calculate_card_bounds_with_offset_list() {
        // List area starting at x=10, y=5
        let list_area = make_rect(10, 5, 80, 40);
        let anchor_y = 15;
        let card_height = 8;

        let bounds = calculate_card_bounds(list_area, anchor_y, card_height);

        // Width should be 90% of 80 = 72
        assert_eq!(bounds.width, 72);
        // X should be: 10 + (80 - 72) / 2 = 10 + 4 = 14
        assert_eq!(bounds.x, 14);
        // Y should be anchor + 1 = 16
        assert_eq!(bounds.y, 16);
        assert_eq!(bounds.height, 8);
    }

    #[test]
    fn test_calculate_card_bounds_clamps_to_bottom() {
        // List area: y=0, height=30, bottom=30
        let list_area = make_rect(0, 0, 100, 30);
        // Anchor near the bottom
        let anchor_y = 25;
        let card_height = 10;

        let bounds = calculate_card_bounds(list_area, anchor_y, card_height);

        // Without clamping, card_y would be 26, which + 10 = 36 > 30
        // So it should clamp to bottom - height = 30 - 10 = 20
        assert_eq!(bounds.y, 20);
        assert_eq!(bounds.height, 10);
    }

    #[test]
    fn test_calculate_card_bounds_exact_fit() {
        // List area: y=0, height=30
        let list_area = make_rect(0, 0, 100, 30);
        // Anchor such that card exactly fits
        let anchor_y = 19; // card_y = 20, 20 + 10 = 30 = bottom
        let card_height = 10;

        let bounds = calculate_card_bounds(list_area, anchor_y, card_height);

        // Should fit exactly without clamping
        assert_eq!(bounds.y, 20);
    }

    #[test]
    fn test_calculate_card_bounds_narrow_list() {
        // Very narrow list area
        let list_area = make_rect(0, 0, 10, 50);
        let anchor_y = 5;
        let card_height = 10;

        let bounds = calculate_card_bounds(list_area, anchor_y, card_height);

        // Width should be 90% of 10 = 9
        assert_eq!(bounds.width, 9);
        // X should be centered: (10 - 9) / 2 = 0 (integer division)
        assert_eq!(bounds.x, 0);
    }

    #[test]
    fn test_calculate_card_bounds_large_card() {
        // Card larger than available space
        let list_area = make_rect(0, 10, 100, 20);
        let anchor_y = 15;
        let card_height = 25; // Larger than list height

        let bounds = calculate_card_bounds(list_area, anchor_y, card_height);

        // Bottom of list is 10 + 20 = 30
        // card_y should clamp to 30 - 25 = 5
        assert_eq!(bounds.y, 5);
        assert_eq!(bounds.height, 25);
    }

    #[test]
    fn test_calculate_card_bounds_zero_anchor() {
        let list_area = make_rect(0, 0, 100, 50);
        let anchor_y = 0;
        let card_height = 10;

        let bounds = calculate_card_bounds(list_area, anchor_y, card_height);

        // Y should be anchor + 1 = 1
        assert_eq!(bounds.y, 1);
    }

    #[test]
    fn test_calculate_card_bounds_list_with_nonzero_y() {
        // List doesn't start at y=0
        let list_area = make_rect(5, 10, 60, 30);
        let anchor_y = 35; // Near the bottom (list bottom = 10 + 30 = 40)
        let card_height = 8;

        let bounds = calculate_card_bounds(list_area, anchor_y, card_height);

        // card_y would be 36, 36 + 8 = 44 > 40
        // Clamp to 40 - 8 = 32
        assert_eq!(bounds.y, 32);

        // Width = 90% of 60 = 54
        assert_eq!(bounds.width, 54);
        // X = 5 + (60 - 54) / 2 = 5 + 3 = 8
        assert_eq!(bounds.x, 8);
    }
}
