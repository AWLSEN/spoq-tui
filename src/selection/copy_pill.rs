//! Copy Pill UI Widget
//!
//! A floating "Copy" pill that appears at the end of a text selection.
//! Clicking the pill copies the selected text to the clipboard.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph, Widget},
};

/// State for the copy pill widget
#[derive(Debug, Clone, Default)]
pub struct CopyPillState {
    /// Whether the pill is visible
    pub visible: bool,
    /// Screen position of the pill (column, row)
    pub position: (u16, u16),
    /// Whether the pill is currently hovered (for click detection)
    pub hovered: bool,
    /// Tick when the pill was last shown (for auto-hide)
    pub shown_tick: u64,
    /// Whether the copy was successful (for feedback)
    pub copy_success: bool,
    /// Tick when copy succeeded (for feedback animation)
    pub success_tick: u64,
}

impl CopyPillState {
    /// Create a new hidden copy pill state
    pub fn new() -> Self {
        Self::default()
    }

    /// Show the pill at the given screen position
    pub fn show(&mut self, x: u16, y: u16, tick: u64) {
        self.visible = true;
        self.position = (x, y);
        self.hovered = false;
        self.shown_tick = tick;
        self.copy_success = false;
    }

    /// Hide the pill
    pub fn hide(&mut self) {
        self.visible = false;
        self.hovered = false;
    }

    /// Mark the copy as successful (for feedback animation)
    pub fn set_success(&mut self, tick: u64) {
        self.copy_success = true;
        self.success_tick = tick;
    }

    /// Check if the pill should auto-hide
    /// Auto-hides after 3 seconds (300 ticks at 100ms/tick)
    pub fn should_auto_hide(&self, current_tick: u64) -> bool {
        if !self.visible {
            return false;
        }

        // Auto-hide after 300 ticks (~30 seconds at 10 ticks/sec)
        current_tick.saturating_sub(self.shown_tick) > 300
    }

    /// Check if success feedback should hide
    /// Success feedback shows for 1 second (10 ticks)
    pub fn should_hide_success(&self, current_tick: u64) -> bool {
        if !self.copy_success {
            return false;
        }
        current_tick.saturating_sub(self.success_tick) > 10
    }

    /// Check if a screen position is within the pill bounds
    pub fn contains(&self, x: u16, y: u16) -> bool {
        if !self.visible {
            return false;
        }

        let (pill_x, pill_y) = self.position;
        let pill_width = PILL_WIDTH;
        let pill_height = PILL_HEIGHT;

        x >= pill_x
            && x < pill_x.saturating_add(pill_width)
            && y >= pill_y
            && y < pill_y.saturating_add(pill_height)
    }

    /// Get the pill bounds as a Rect
    pub fn bounds(&self) -> Rect {
        Rect::new(
            self.position.0,
            self.position.1,
            PILL_WIDTH,
            PILL_HEIGHT,
        )
    }
}

/// Width of the copy pill (in terminal cells)
pub const PILL_WIDTH: u16 = 8; // "[ Copy ]"

/// Height of the copy pill (in terminal cells)
pub const PILL_HEIGHT: u16 = 1;

/// Background color for the copy pill
pub const PILL_BG_COLOR: Color = Color::Rgb(60, 60, 80);

/// Hover background color for the copy pill
pub const PILL_HOVER_BG_COLOR: Color = Color::Rgb(80, 80, 120);

/// Success background color (after copying)
pub const PILL_SUCCESS_BG_COLOR: Color = Color::Rgb(40, 100, 60);

/// The Copy Pill widget
pub struct CopyPill<'a> {
    state: &'a CopyPillState,
}

impl<'a> CopyPill<'a> {
    /// Create a new CopyPill widget with the given state
    pub fn new(state: &'a CopyPillState) -> Self {
        Self { state }
    }
}

impl<'a> Widget for CopyPill<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if !self.state.visible {
            return;
        }

        // Clear the area first
        Clear.render(area, buf);

        // Choose background color based on state
        let bg_color = if self.state.copy_success {
            PILL_SUCCESS_BG_COLOR
        } else if self.state.hovered {
            PILL_HOVER_BG_COLOR
        } else {
            PILL_BG_COLOR
        };

        // Choose text based on state
        let text = if self.state.copy_success {
            "Copied!"
        } else {
            "  Copy "
        };

        let style = Style::default()
            .bg(bg_color)
            .fg(Color::White)
            .add_modifier(Modifier::BOLD);

        // Render the pill
        let line = Line::from(vec![Span::styled(text, style)]);
        let paragraph = Paragraph::new(line);

        // Render at the pill's position
        let pill_area = Rect::new(
            self.state.position.0.min(area.right().saturating_sub(PILL_WIDTH)),
            self.state.position.1.min(area.bottom().saturating_sub(PILL_HEIGHT)),
            PILL_WIDTH.min(area.width),
            PILL_HEIGHT.min(area.height),
        );

        paragraph.render(pill_area, buf);
    }
}

/// Render the copy pill on a frame
///
/// This is a convenience function for rendering the pill during the UI render pass.
///
/// # Arguments
/// * `state` - The copy pill state
/// * `frame` - The frame to render on
/// * `area` - The area where the pill can be rendered (typically the full terminal)
pub fn render_copy_pill(state: &CopyPillState, buf: &mut Buffer, area: Rect) {
    if !state.visible {
        return;
    }

    let widget = CopyPill::new(state);
    widget.render(area, buf);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_copy_pill_state_new() {
        let state = CopyPillState::new();
        assert!(!state.visible);
        assert!(!state.hovered);
        assert!(!state.copy_success);
    }

    #[test]
    fn test_copy_pill_state_show() {
        let mut state = CopyPillState::new();
        state.show(10, 20, 100);

        assert!(state.visible);
        assert_eq!(state.position, (10, 20));
        assert_eq!(state.shown_tick, 100);
    }

    #[test]
    fn test_copy_pill_state_hide() {
        let mut state = CopyPillState::new();
        state.show(10, 20, 100);
        state.hide();

        assert!(!state.visible);
    }

    #[test]
    fn test_copy_pill_state_success() {
        let mut state = CopyPillState::new();
        state.show(10, 20, 100);
        state.set_success(150);

        assert!(state.copy_success);
        assert_eq!(state.success_tick, 150);
    }

    #[test]
    fn test_copy_pill_state_contains() {
        let mut state = CopyPillState::new();
        state.show(10, 20, 100);

        // Inside pill
        assert!(state.contains(10, 20));
        assert!(state.contains(15, 20));

        // Outside pill
        assert!(!state.contains(9, 20));
        assert!(!state.contains(10, 19));
        assert!(!state.contains(20, 20));
    }

    #[test]
    fn test_copy_pill_state_contains_not_visible() {
        let state = CopyPillState::new();
        assert!(!state.contains(10, 20));
    }

    #[test]
    fn test_copy_pill_state_auto_hide() {
        let mut state = CopyPillState::new();
        state.show(10, 20, 100);

        assert!(!state.should_auto_hide(200)); // 100 ticks later
        assert!(state.should_auto_hide(500)); // 400 ticks later
    }

    #[test]
    fn test_copy_pill_state_success_hide() {
        let mut state = CopyPillState::new();
        state.show(10, 20, 100);
        state.set_success(150);

        assert!(!state.should_hide_success(155)); // 5 ticks later
        assert!(state.should_hide_success(165)); // 15 ticks later
    }

    #[test]
    fn test_copy_pill_bounds() {
        let mut state = CopyPillState::new();
        state.show(10, 20, 100);

        let bounds = state.bounds();
        assert_eq!(bounds.x, 10);
        assert_eq!(bounds.y, 20);
        assert_eq!(bounds.width, PILL_WIDTH);
        assert_eq!(bounds.height, PILL_HEIGHT);
    }
}
