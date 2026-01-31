//! Tab Selector Component
//!
//! A horizontal tab selector matching the thread_switcher's visual style.
//! Uses `▶` marker for the selected item with responsive label sizing.

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use crate::ui::layout::LayoutContext;
use crate::ui::theme::{COLOR_ACCENT, COLOR_DIM};

/// A single tab item in the selector
#[derive(Debug, Clone)]
pub struct TabItem<'a> {
    /// Unique identifier for the tab
    pub id: &'a str,
    /// Full label displayed on normal-sized terminals
    pub label: &'a str,
    /// Short label displayed on compact terminals
    pub short_label: &'a str,
}

impl<'a> TabItem<'a> {
    /// Create a new tab item with the same label for both normal and compact modes
    pub fn new(id: &'a str, label: &'a str) -> Self {
        Self {
            id,
            label,
            short_label: label,
        }
    }

    /// Create a new tab item with different labels for normal and compact modes
    pub fn with_short_label(id: &'a str, label: &'a str, short_label: &'a str) -> Self {
        Self {
            id,
            label,
            short_label,
        }
    }
}

/// Render a horizontal tab selector
///
/// # Arguments
/// * `items` - The tab items to display
/// * `selected` - Index of the currently selected tab
/// * `focused` - Whether the tab selector is currently focused
/// * `ctx` - Layout context for responsive sizing
///
/// # Returns
/// A `Line` containing the rendered tab selector
///
/// # Example
/// ```ignore
/// let items = vec![
///     TabItem::with_short_label("remote", "Remote VPS", "Remote"),
///     TabItem::with_short_label("local", "Local", "Local"),
/// ];
/// let line = render_tab_selector(&items, 0, true, &ctx);
/// ```
pub fn render_tab_selector<'a>(
    items: &[TabItem<'a>],
    selected: usize,
    focused: bool,
    ctx: &LayoutContext,
) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();

    // Add leading padding
    spans.push(Span::raw("  "));

    for (idx, item) in items.iter().enumerate() {
        let is_selected = idx == selected;

        // Use short labels on compact screens
        let label = if ctx.is_compact() {
            item.short_label
        } else {
            item.label
        };

        if is_selected {
            // Selected item with marker
            let marker_style = if focused {
                Style::default()
                    .fg(COLOR_ACCENT)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(COLOR_DIM)
            };

            let text_style = if focused {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            spans.push(Span::styled("▶ ".to_string(), marker_style));
            spans.push(Span::styled(label.to_string(), text_style));
        } else {
            // Non-selected item (dimmed)
            let text_style = Style::default().fg(COLOR_DIM);
            spans.push(Span::styled("  ".to_string(), text_style));
            spans.push(Span::styled(label.to_string(), text_style));
        }

        // Add spacing between tabs (except after last)
        if idx < items.len() - 1 {
            let spacing = if ctx.is_extra_small() { "  " } else { "    " };
            spans.push(Span::raw(spacing.to_string()));
        }
    }

    Line::from(spans)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_items() -> Vec<TabItem<'static>> {
        vec![
            TabItem::with_short_label("remote", "Remote VPS", "Remote"),
            TabItem::with_short_label("local", "Local", "Local"),
        ]
    }

    #[test]
    fn test_tab_item_new() {
        let item = TabItem::new("test", "Test Label");
        assert_eq!(item.id, "test");
        assert_eq!(item.label, "Test Label");
        assert_eq!(item.short_label, "Test Label");
    }

    #[test]
    fn test_tab_item_with_short_label() {
        let item = TabItem::with_short_label("test", "Test Label", "Test");
        assert_eq!(item.id, "test");
        assert_eq!(item.label, "Test Label");
        assert_eq!(item.short_label, "Test");
    }

    #[test]
    fn test_render_first_selected() {
        let items = create_test_items();
        let ctx = LayoutContext::new(100, 40);
        let line = render_tab_selector(&items, 0, true, &ctx);

        // Should contain the marker for the first item
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("▶"));
        assert!(text.contains("Remote VPS"));
        assert!(text.contains("Local"));
    }

    #[test]
    fn test_render_second_selected() {
        let items = create_test_items();
        let ctx = LayoutContext::new(100, 40);
        let line = render_tab_selector(&items, 1, true, &ctx);

        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("▶"));
        // The marker should be before "Local", not "Remote VPS"
        let marker_pos = text.find("▶").unwrap();
        let local_pos = text.find("Local").unwrap();
        let remote_pos = text.find("Remote VPS").unwrap();
        assert!(marker_pos > remote_pos); // Marker comes after Remote
        assert!(marker_pos < local_pos); // Marker comes before Local
    }

    #[test]
    fn test_compact_uses_short_labels() {
        let items = create_test_items();
        let ctx = LayoutContext::new(50, 14); // Extra small terminal
        let line = render_tab_selector(&items, 0, true, &ctx);

        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        // Should use short labels
        assert!(text.contains("Remote"));
        assert!(!text.contains("Remote VPS")); // Full label should not appear
    }

    #[test]
    fn test_normal_uses_full_labels() {
        let items = create_test_items();
        let ctx = LayoutContext::new(120, 40); // Normal terminal
        let line = render_tab_selector(&items, 0, true, &ctx);

        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("Remote VPS")); // Full label should appear
    }

    #[test]
    fn test_unfocused_still_shows_selection() {
        let items = create_test_items();
        let ctx = LayoutContext::new(100, 40);
        let line = render_tab_selector(&items, 0, false, &ctx);

        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        // Should still show the marker even when unfocused
        assert!(text.contains("▶"));
    }
}
