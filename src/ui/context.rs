//! Render context for pure UI rendering
//!
//! Contains pre-computed data prepared before the render phase.
//! All rendering functions receive immutable references to this context,
//! ensuring the render phase is pure (no mutations).

use crate::app::CachedHeights;

/// Height information for a single message.
#[derive(Debug, Clone)]
pub struct MessageHeightInfo {
    /// Message ID
    pub message_id: i64,
    /// Number of visual lines
    pub visual_lines: usize,
    /// Cumulative offset from start
    pub cumulative_offset: usize,
}

impl From<&CachedHeights> for Vec<MessageHeightInfo> {
    fn from(cache: &CachedHeights) -> Self {
        cache
            .heights
            .iter()
            .map(|h| MessageHeightInfo {
                message_id: h.message_id,
                visual_lines: h.visual_lines,
                cumulative_offset: h.cumulative_offset,
            })
            .collect()
    }
}

/// Mutable outputs from the render phase.
///
/// Since the render phase should be pure, any values that need to be
/// communicated back to the app (like scroll calculations) are captured here.
#[derive(Debug, Default)]
pub struct RenderOutputs {
    /// Calculated maximum scroll value
    pub max_scroll: u16,

    /// Whether visible content contains hyperlinks
    pub has_visible_links: bool,

    /// Where the input section starts (line index)
    pub input_section_start: usize,

    /// Total content lines rendered
    pub total_content_lines: usize,
}

impl RenderOutputs {
    /// Create new empty render outputs
    pub fn new() -> Self {
        Self::default()
    }
}
