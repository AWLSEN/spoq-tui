//! Hit area system for touch-first interactions.
//!
//! This module provides a registry-based approach to handling clickable regions
//! in the TUI. Components register hit areas during rendering, and the event
//! loop queries the registry to determine what action to take on mouse events.

use ratatui::layout::Rect;
use ratatui::style::Style;

/// Represents an action that can be triggered by clicking a hit area.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClickAction {
    // Filter actions (CommandDeck)
    /// Filter to show only "Working" threads
    FilterWorking,
    /// Filter to show only "Ready to Test" threads
    FilterReadyToTest,
    /// Filter to show only "Idle" threads
    FilterIdle,
    /// Clear all filters
    ClearFilter,

    // Overlay actions (Dashboard expanded thread)
    /// Expand a thread overlay at the given anchor position
    ExpandThread { thread_id: String, anchor_y: u16 },
    /// Collapse the currently expanded overlay
    CollapseOverlay,

    // Thread action buttons
    /// Approve a thread's work
    ApproveThread(String),
    /// Reject a thread's work
    RejectThread(String),
    /// Verify a thread's work
    VerifyThread(String),
    /// Archive a thread
    ArchiveThread(String),
    /// Resume a paused/archived thread
    ResumeThread(String),
    /// Delete a thread
    DeleteThread(String),
    /// Report an issue with a thread
    ReportIssue(String),

    // Question prompt interactions
    /// Select an option in a multi-choice question
    SelectOption { thread_id: String, index: usize },
    /// Show free-form text input for a question
    ShowFreeFormInput(String),
    /// Submit free-form text response
    SubmitFreeForm(String),
    /// Go back to options from free-form input
    BackToOptions(String),

    // Navigation
    /// View the full plan for a thread
    ViewFullPlan(String),
}

/// A clickable region with an associated action.
#[derive(Debug, Clone)]
pub struct HitArea {
    /// The rectangular region that responds to clicks
    pub rect: Rect,
    /// The action to trigger when this area is clicked
    pub action: ClickAction,
    /// Optional style to apply when hovering over this area
    pub hover_style: Option<Style>,
}

impl HitArea {
    /// Create a new hit area with the given rect and action.
    pub fn new(rect: Rect, action: ClickAction) -> Self {
        Self {
            rect,
            action,
            hover_style: None,
        }
    }

    /// Create a new hit area with a hover style.
    pub fn with_hover_style(rect: Rect, action: ClickAction, hover_style: Style) -> Self {
        Self {
            rect,
            action,
            hover_style: Some(hover_style),
        }
    }

    /// Check if a point is within this hit area.
    #[inline]
    pub fn contains(&self, x: u16, y: u16) -> bool {
        x >= self.rect.x
            && x < self.rect.x + self.rect.width
            && y >= self.rect.y
            && y < self.rect.y + self.rect.height
    }
}

/// Registry for managing hit areas across the UI.
///
/// Hit areas are registered during rendering and cleared at the start of each
/// render cycle. The registry supports hit testing (finding which area was clicked)
/// and hover tracking for visual feedback.
#[derive(Debug, Default)]
pub struct HitAreaRegistry {
    /// All registered hit areas (order matters for overlapping regions)
    areas: Vec<HitArea>,
    /// Index of the currently hovered area (if any)
    hovered: Option<usize>,
}

impl HitAreaRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            areas: Vec::new(),
            hovered: None,
        }
    }

    /// Clear all registered areas and reset hover state.
    ///
    /// Call this at the start of each render cycle.
    pub fn clear(&mut self) {
        self.areas.clear();
        self.hovered = None;
    }

    /// Register a new hit area.
    ///
    /// Areas registered later take priority over earlier ones for overlapping
    /// regions (z-order: later = on top).
    pub fn register(&mut self, rect: Rect, action: ClickAction, hover_style: Option<Style>) {
        self.areas.push(HitArea {
            rect,
            action,
            hover_style,
        });
    }

    /// Register a hit area from an existing HitArea struct.
    pub fn register_area(&mut self, area: HitArea) {
        self.areas.push(area);
    }

    /// Perform a hit test at the given position.
    ///
    /// Returns the action for the topmost hit area containing the point,
    /// or None if no area was hit. Areas are checked in reverse order
    /// (last registered = highest priority).
    pub fn hit_test(&self, x: u16, y: u16) -> Option<ClickAction> {
        // Iterate in reverse to check topmost (last registered) areas first
        for area in self.areas.iter().rev() {
            if area.contains(x, y) {
                return Some(area.action.clone());
            }
        }
        None
    }

    /// Update the hover state based on mouse position.
    ///
    /// Returns true if the hover state changed (requiring a redraw).
    pub fn update_hover(&mut self, x: u16, y: u16) -> bool {
        let new_hovered = self.find_hovered_index(x, y);
        if new_hovered != self.hovered {
            self.hovered = new_hovered;
            true
        } else {
            false
        }
    }

    /// Find the index of the topmost area containing the given point.
    fn find_hovered_index(&self, x: u16, y: u16) -> Option<usize> {
        // Iterate in reverse to find topmost (last registered) area first
        for (i, area) in self.areas.iter().enumerate().rev() {
            if area.contains(x, y) {
                return Some(i);
            }
        }
        None
    }

    /// Get the hover style for a rect if it matches the currently hovered area.
    ///
    /// This allows render code to apply hover styling to elements without
    /// needing to track hover state themselves.
    pub fn get_hover_style(&self, rect: Rect) -> Option<Style> {
        let hovered_idx = self.hovered?;
        let hovered_area = self.areas.get(hovered_idx)?;

        // Check if the rect matches the hovered area's rect
        if hovered_area.rect == rect {
            hovered_area.hover_style
        } else {
            None
        }
    }

    /// Check if any area is currently hovered.
    pub fn is_hovering(&self) -> bool {
        self.hovered.is_some()
    }

    /// Get the currently hovered area (if any).
    pub fn get_hovered(&self) -> Option<&HitArea> {
        self.hovered.and_then(|idx| self.areas.get(idx))
    }

    /// Get the number of registered areas.
    pub fn len(&self) -> usize {
        self.areas.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.areas.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    fn make_rect(x: u16, y: u16, width: u16, height: u16) -> Rect {
        Rect::new(x, y, width, height)
    }

    #[test]
    fn test_hit_area_contains() {
        let area = HitArea::new(make_rect(10, 10, 20, 10), ClickAction::FilterWorking);

        // Inside the area
        assert!(area.contains(10, 10)); // Top-left corner
        assert!(area.contains(29, 10)); // Top-right edge (x + width - 1)
        assert!(area.contains(10, 19)); // Bottom-left edge (y + height - 1)
        assert!(area.contains(29, 19)); // Bottom-right corner
        assert!(area.contains(20, 15)); // Center

        // Outside the area
        assert!(!area.contains(9, 10)); // Left of area
        assert!(!area.contains(30, 10)); // Right of area (x + width is exclusive)
        assert!(!area.contains(10, 9)); // Above area
        assert!(!area.contains(10, 20)); // Below area (y + height is exclusive)
        assert!(!area.contains(0, 0)); // Origin
    }

    #[test]
    fn test_hit_area_zero_size() {
        let area = HitArea::new(make_rect(5, 5, 0, 0), ClickAction::ClearFilter);

        // Zero-size area should not contain any point
        assert!(!area.contains(5, 5));
        assert!(!area.contains(4, 4));
    }

    #[test]
    fn test_registry_clear() {
        let mut registry = HitAreaRegistry::new();

        registry.register(make_rect(0, 0, 10, 10), ClickAction::FilterWorking, None);
        registry.register(make_rect(10, 0, 10, 10), ClickAction::FilterIdle, None);
        assert_eq!(registry.len(), 2);

        // Set hover state
        registry.update_hover(5, 5);
        assert!(registry.is_hovering());

        registry.clear();
        assert_eq!(registry.len(), 0);
        assert!(!registry.is_hovering());
        assert!(registry.is_empty());
    }

    #[test]
    fn test_hit_test_basic() {
        let mut registry = HitAreaRegistry::new();

        registry.register(make_rect(0, 0, 10, 10), ClickAction::FilterWorking, None);
        registry.register(
            make_rect(20, 0, 10, 10),
            ClickAction::FilterReadyToTest,
            None,
        );
        registry.register(make_rect(40, 0, 10, 10), ClickAction::FilterIdle, None);

        // Hit each area
        assert_eq!(registry.hit_test(5, 5), Some(ClickAction::FilterWorking));
        assert_eq!(
            registry.hit_test(25, 5),
            Some(ClickAction::FilterReadyToTest)
        );
        assert_eq!(registry.hit_test(45, 5), Some(ClickAction::FilterIdle));

        // Miss all areas
        assert_eq!(registry.hit_test(15, 5), None);
        assert_eq!(registry.hit_test(100, 100), None);
    }

    #[test]
    fn test_hit_test_overlapping_areas() {
        let mut registry = HitAreaRegistry::new();

        // Register overlapping areas - later ones should take priority
        registry.register(make_rect(0, 0, 20, 20), ClickAction::FilterWorking, None); // Bottom layer
        registry.register(make_rect(5, 5, 10, 10), ClickAction::FilterIdle, None); // Top layer

        // Click in overlapping region - should hit top layer
        assert_eq!(registry.hit_test(10, 10), Some(ClickAction::FilterIdle));

        // Click outside inner area but inside outer - should hit bottom layer
        assert_eq!(registry.hit_test(2, 2), Some(ClickAction::FilterWorking));
        assert_eq!(registry.hit_test(18, 18), Some(ClickAction::FilterWorking));
    }

    #[test]
    fn test_hit_test_with_thread_id() {
        let mut registry = HitAreaRegistry::new();

        registry.register(
            make_rect(0, 0, 10, 10),
            ClickAction::ApproveThread("thread-123".to_string()),
            None,
        );

        let result = registry.hit_test(5, 5);
        assert_eq!(
            result,
            Some(ClickAction::ApproveThread("thread-123".to_string()))
        );
    }

    #[test]
    fn test_update_hover_returns_changed() {
        let mut registry = HitAreaRegistry::new();

        registry.register(make_rect(0, 0, 10, 10), ClickAction::FilterWorking, None);
        registry.register(make_rect(20, 0, 10, 10), ClickAction::FilterIdle, None);

        // Initial hover - should return true (changed from None)
        assert!(registry.update_hover(5, 5));

        // Same position - should return false (no change)
        assert!(!registry.update_hover(5, 5));

        // Still in same area, different position - should return false
        assert!(!registry.update_hover(8, 8));

        // Move to different area - should return true
        assert!(registry.update_hover(25, 5));

        // Move to no area - should return true
        assert!(registry.update_hover(100, 100));

        // Still in no area - should return false
        assert!(!registry.update_hover(200, 200));
    }

    #[test]
    fn test_get_hover_style() {
        let mut registry = HitAreaRegistry::new();

        let hover_style = Style::default().fg(Color::Yellow);
        let rect1 = make_rect(0, 0, 10, 10);
        let rect2 = make_rect(20, 0, 10, 10);

        registry.register(rect1, ClickAction::FilterWorking, Some(hover_style));
        registry.register(rect2, ClickAction::FilterIdle, None);

        // No hover yet
        assert_eq!(registry.get_hover_style(rect1), None);

        // Hover over first area
        registry.update_hover(5, 5);
        assert_eq!(registry.get_hover_style(rect1), Some(hover_style));
        assert_eq!(registry.get_hover_style(rect2), None);

        // Hover over second area (no hover style)
        registry.update_hover(25, 5);
        assert_eq!(registry.get_hover_style(rect1), None);
        assert_eq!(registry.get_hover_style(rect2), None); // Has no hover style

        // Different rect that matches position but not hovered rect
        let different_rect = make_rect(0, 0, 5, 5);
        registry.update_hover(5, 5);
        assert_eq!(registry.get_hover_style(different_rect), None);
    }

    #[test]
    fn test_get_hovered() {
        let mut registry = HitAreaRegistry::new();

        registry.register(make_rect(0, 0, 10, 10), ClickAction::FilterWorking, None);

        // No hover initially
        assert!(registry.get_hovered().is_none());

        // After hover
        registry.update_hover(5, 5);
        let hovered = registry.get_hovered().unwrap();
        assert_eq!(hovered.action, ClickAction::FilterWorking);

        // After hover moves away
        registry.update_hover(100, 100);
        assert!(registry.get_hovered().is_none());
    }

    #[test]
    fn test_boundary_conditions() {
        let mut registry = HitAreaRegistry::new();

        // Area at origin
        registry.register(make_rect(0, 0, 5, 5), ClickAction::FilterWorking, None);

        // Hit at origin
        assert_eq!(registry.hit_test(0, 0), Some(ClickAction::FilterWorking));

        // Hit at max u16 values (area at edge of screen)
        registry.clear();
        let max_x = u16::MAX - 10;
        let max_y = u16::MAX - 10;
        registry.register(make_rect(max_x, max_y, 5, 5), ClickAction::FilterIdle, None);
        assert_eq!(
            registry.hit_test(max_x + 2, max_y + 2),
            Some(ClickAction::FilterIdle)
        );
    }

    #[test]
    fn test_register_area() {
        let mut registry = HitAreaRegistry::new();

        let area = HitArea::with_hover_style(
            make_rect(10, 10, 20, 20),
            ClickAction::CollapseOverlay,
            Style::default().fg(Color::Red),
        );

        registry.register_area(area);
        assert_eq!(registry.len(), 1);
        assert_eq!(
            registry.hit_test(15, 15),
            Some(ClickAction::CollapseOverlay)
        );
    }

    #[test]
    fn test_select_option_action() {
        let mut registry = HitAreaRegistry::new();

        registry.register(
            make_rect(0, 0, 10, 10),
            ClickAction::SelectOption {
                thread_id: "t1".to_string(),
                index: 0,
            },
            None,
        );
        registry.register(
            make_rect(0, 10, 10, 10),
            ClickAction::SelectOption {
                thread_id: "t1".to_string(),
                index: 1,
            },
            None,
        );

        assert_eq!(
            registry.hit_test(5, 5),
            Some(ClickAction::SelectOption {
                thread_id: "t1".to_string(),
                index: 0
            })
        );
        assert_eq!(
            registry.hit_test(5, 15),
            Some(ClickAction::SelectOption {
                thread_id: "t1".to_string(),
                index: 1
            })
        );
    }
}
