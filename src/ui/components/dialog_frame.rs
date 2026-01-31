//! Dialog Frame Component
//!
//! A centered dialog frame with rounded borders matching the thread_switcher style.
//! Handles background clearing and responsive sizing.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::Span,
    widgets::{Block, BorderType, Borders, Clear},
    Frame,
};

use crate::ui::layout::LayoutContext;
use crate::ui::theme::{COLOR_BORDER, COLOR_HEADER};

/// Configuration for rendering a dialog frame
#[derive(Debug, Clone)]
pub struct DialogFrameConfig<'a> {
    /// Title displayed in the border
    pub title: &'a str,
    /// Content height (not including borders)
    pub content_height: u16,
    /// Optional fixed width (otherwise responsive)
    pub fixed_width: Option<u16>,
    /// Minimum width
    pub min_width: u16,
    /// Maximum width
    pub max_width: u16,
}

impl<'a> DialogFrameConfig<'a> {
    /// Create a new dialog frame configuration
    pub fn new(title: &'a str, content_height: u16) -> Self {
        Self {
            title,
            content_height,
            fixed_width: None,
            min_width: 30,
            max_width: 60,
        }
    }

    /// Set a fixed width for the dialog
    pub fn fixed_width(mut self, width: u16) -> Self {
        self.fixed_width = Some(width);
        self
    }

    /// Set the minimum width
    pub fn min_width(mut self, width: u16) -> Self {
        self.min_width = width;
        self
    }

    /// Set the maximum width
    pub fn max_width(mut self, width: u16) -> Self {
        self.max_width = width;
        self
    }
}

/// Calculate dialog width based on terminal size and configuration
fn calculate_dialog_width(ctx: &LayoutContext, config: &DialogFrameConfig, area_width: u16) -> u16 {
    if let Some(fixed) = config.fixed_width {
        return fixed.min(area_width.saturating_sub(4));
    }

    if ctx.is_extra_small() {
        // Extra small: take most of the screen width, leave 2 cols margin
        area_width.saturating_sub(4).min(config.max_width)
    } else if ctx.is_narrow() {
        // Narrow: 80% of width, within bounds
        ctx.bounded_width(80, config.min_width, config.max_width)
    } else {
        // Normal: 50% of width, within bounds
        ctx.bounded_width(50, config.min_width, config.max_width)
    }
}

/// Render a dialog frame and return the inner content area
///
/// This function:
/// 1. Calculates responsive dimensions
/// 2. Centers the dialog on screen
/// 3. Clears the background
/// 4. Renders the border with title
/// 5. Returns the inner area for content
///
/// # Arguments
/// * `frame` - The frame to render to
/// * `area` - The full screen area
/// * `ctx` - Layout context for responsive sizing
/// * `config` - Dialog frame configuration
///
/// # Returns
/// The inner `Rect` where content should be rendered
pub fn render_dialog_frame(
    frame: &mut Frame,
    area: Rect,
    ctx: &LayoutContext,
    config: &DialogFrameConfig,
) -> Rect {
    // Calculate dimensions
    let dialog_width = calculate_dialog_width(ctx, config, area.width);
    let dialog_height = config.content_height + 2; // Add 2 for borders

    // Center the dialog
    let x = (area.width.saturating_sub(dialog_width)) / 2;
    let y = (area.height.saturating_sub(dialog_height)) / 2;

    let dialog_area = Rect {
        x,
        y,
        width: dialog_width,
        height: dialog_height,
    };

    // Clear the background behind the dialog
    frame.render_widget(Clear, dialog_area);

    // Create title with centered style matching thread_switcher
    let title = format!(" {} ", config.title);

    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default()
                .fg(COLOR_HEADER)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(COLOR_BORDER));

    frame.render_widget(block, dialog_area);

    // Return inner content area
    Rect {
        x: dialog_area.x + 1,
        y: dialog_area.y + 1,
        width: dialog_area.width.saturating_sub(2),
        height: dialog_area.height.saturating_sub(2),
    }
}

/// Calculate the total dialog height needed for given content
pub fn calculate_total_dialog_height(content_height: u16) -> u16 {
    content_height + 2 // Add borders
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dialog_frame_config_new() {
        let config = DialogFrameConfig::new("Test", 10);
        assert_eq!(config.title, "Test");
        assert_eq!(config.content_height, 10);
        assert!(config.fixed_width.is_none());
        assert_eq!(config.min_width, 30);
        assert_eq!(config.max_width, 60);
    }

    #[test]
    fn test_dialog_frame_config_builder() {
        let config = DialogFrameConfig::new("Test", 10)
            .fixed_width(50)
            .min_width(40)
            .max_width(70);

        assert_eq!(config.fixed_width, Some(50));
        assert_eq!(config.min_width, 40);
        assert_eq!(config.max_width, 70);
    }

    #[test]
    fn test_calculate_dialog_width_fixed() {
        let ctx = LayoutContext::new(100, 40);
        let config = DialogFrameConfig::new("Test", 10).fixed_width(50);
        let width = calculate_dialog_width(&ctx, &config, 100);
        assert_eq!(width, 50);
    }

    #[test]
    fn test_calculate_dialog_width_fixed_clamped() {
        let ctx = LayoutContext::new(40, 20);
        let config = DialogFrameConfig::new("Test", 10).fixed_width(50);
        // Should be clamped to area width - 4
        let width = calculate_dialog_width(&ctx, &config, 40);
        assert_eq!(width, 36);
    }

    #[test]
    fn test_calculate_dialog_width_extra_small() {
        let ctx = LayoutContext::new(50, 14);
        let config = DialogFrameConfig::new("Test", 10);
        let width = calculate_dialog_width(&ctx, &config, 50);
        // Should take most of width but respect max
        assert!(width <= 60);
        assert!(width >= 30);
    }

    #[test]
    fn test_calculate_dialog_width_normal() {
        let ctx = LayoutContext::new(120, 40);
        let config = DialogFrameConfig::new("Test", 10);
        let width = calculate_dialog_width(&ctx, &config, 120);
        // 50% of 120 = 60, clamped to max 60
        assert_eq!(width, 60);
    }

    #[test]
    fn test_calculate_total_dialog_height() {
        assert_eq!(calculate_total_dialog_height(10), 12);
        assert_eq!(calculate_total_dialog_height(5), 7);
    }
}
