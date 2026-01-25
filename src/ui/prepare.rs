//! Render preparation phase
//!
//! This module contains functions that prepare data for the render phase.
//! All mutations to app state happen here, ensuring the render phase is pure.
//!
//! ## Design Principle
//!
//! The UI rendering is split into two phases:
//! 1. **Prepare** (this module): Mutates app state, computes derived data
//! 2. **Render**: Pure function that only reads state and produces Frame
//!
//! This separation makes the code easier to reason about and test.

use crate::app::App;
use crate::ui::context::{MessageHeightInfo, RenderOutputs};

/// Prepare app state for the render phase.
///
/// This function performs all necessary mutations to app state before rendering.
/// After calling this, the render functions should not need to mutate app state.
///
/// # Mutations performed:
/// - Clears hit registry
/// - Invalidates caches if viewport width changed
/// - Updates height cache for message virtualization
///
/// # Arguments
/// * `app` - Mutable reference to app state
/// * `viewport_width` - Current viewport width in columns
pub fn prepare_render(app: &mut App, viewport_width: u16) {
    // Invalidate rendered lines cache if viewport width changed
    app.rendered_lines_cache
        .invalidate_if_width_changed(viewport_width);

    // Reset link visibility flag
    app.has_visible_links = false;

    // Prepare height cache if we're on the conversation screen
    if app.screen == crate::app::Screen::Conversation {
        prepare_message_heights(app, viewport_width as usize);
    }
}

/// Prepare message heights using incremental caching.
///
/// This function updates the height cache for message virtualization.
/// It handles cache invalidation, incremental updates, and cache rebuilds.
fn prepare_message_heights(app: &mut App, viewport_width: usize) {
    use super::messages::virtualization::estimate_message_height_fast;

    let current_thread_id = app.active_thread_id.clone();

    let cached_messages = current_thread_id.as_ref().and_then(|id| {
        crate::app::log_thread_update(&format!(
            "PREPARE: Looking for messages for thread_id: {}",
            id
        ));
        let msgs = app.cache.get_messages(id);
        crate::app::log_thread_update(&format!(
            "PREPARE: Found {} messages",
            msgs.map(|m| m.len()).unwrap_or(0)
        ));
        msgs
    });

    match (&current_thread_id, cached_messages) {
        (_, None) => {}
        (_, Some(messages)) if messages.is_empty() => {}
        (Some(thread_id), Some(messages)) => {
            // Check if we can use incremental updates on existing cache
            let cache_valid = app
                .height_cache
                .as_ref()
                .map(|c| c.is_valid_for(thread_id, viewport_width))
                .unwrap_or(false);

            if cache_valid {
                // Incremental update
                let cache = app.height_cache.as_mut().unwrap();
                let cached_len = cache.heights.len();
                let msg_len = messages.len();

                // Handle message removal
                if msg_len < cached_len {
                    cache.truncate(msg_len);
                }

                // Track earliest index where cumulative offsets need recalculation
                let mut first_changed_idx: Option<usize> = None;

                // Update existing entries where render_version changed
                for (i, message) in messages.iter().enumerate().take(cache.heights.len()) {
                    let cached_entry = &cache.heights[i];
                    if cached_entry.message_id != message.id
                        || cached_entry.render_version != message.render_version
                    {
                        let new_height = estimate_message_height_fast(message, viewport_width);
                        cache.heights[i].message_id = message.id;
                        cache.heights[i].render_version = message.render_version;
                        if cache.heights[i].visual_lines != new_height {
                            cache.heights[i].visual_lines = new_height;
                            if first_changed_idx.is_none() {
                                first_changed_idx = Some(i);
                            }
                        }
                    }
                }

                // Append new messages
                for message in messages.iter().skip(cache.heights.len()) {
                    let height = estimate_message_height_fast(message, viewport_width);
                    cache.append(message.id, message.render_version, height);
                }

                // Recalculate cumulative offsets if any heights changed
                if let Some(start_idx) = first_changed_idx {
                    cache.recalculate_offsets_from(start_idx);
                }
            } else {
                // Cache miss or invalid: build fresh cache
                let thread_id_arc = std::sync::Arc::new(thread_id.clone());
                let mut cache = crate::app::CachedHeights::new(thread_id_arc, viewport_width);

                for message in messages.iter() {
                    let height = estimate_message_height_fast(message, viewport_width);
                    cache.append(message.id, message.render_version, height);
                }

                app.height_cache = Some(cache);
            }
        }
        (None, Some(_)) => {}
    }
}

/// Get message heights from the cache.
///
/// Call this after prepare_render() to get the computed heights.
pub fn get_message_heights(app: &App) -> Vec<MessageHeightInfo> {
    app.height_cache
        .as_ref()
        .map(|cache| cache.into())
        .unwrap_or_default()
}

/// Get total visual lines from the height cache.
pub fn get_total_visual_lines(app: &App) -> usize {
    app.height_cache
        .as_ref()
        .map(|cache| cache.total_lines)
        .unwrap_or(0)
}

/// Apply render outputs back to app state.
///
/// After the render phase completes, this function applies any computed
/// values (like max_scroll, has_visible_links) back to app state.
pub fn apply_render_outputs(app: &mut App, outputs: RenderOutputs) {
    app.max_scroll = outputs.max_scroll;
    app.has_visible_links = outputs.has_visible_links;
    app.input_section_start = outputs.input_section_start;
    app.total_content_lines = outputs.total_content_lines;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prepare_render_resets_link_visibility() {
        let mut app = App::default();
        app.has_visible_links = true;

        prepare_render(&mut app, 80);

        assert!(!app.has_visible_links);
    }

    #[test]
    fn test_apply_render_outputs() {
        let mut app = App::default();

        let outputs = RenderOutputs {
            max_scroll: 42,
            has_visible_links: true,
            input_section_start: 10,
            total_content_lines: 50,
        };

        apply_render_outputs(&mut app, outputs);

        assert_eq!(app.max_scroll, 42);
        assert!(app.has_visible_links);
        assert_eq!(app.input_section_start, 10);
        assert_eq!(app.total_content_lines, 50);
    }

    #[test]
    fn test_get_message_heights_empty() {
        let app = App::default();

        let heights = get_message_heights(&app);

        assert!(heights.is_empty());
    }

    #[test]
    fn test_get_total_visual_lines_empty() {
        let app = App::default();

        let total = get_total_visual_lines(&app);

        assert_eq!(total, 0);
    }
}
