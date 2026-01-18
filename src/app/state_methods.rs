//! State accessor and utility methods for the App.

use std::sync::Arc;
use tokio::sync::mpsc;

use super::{App, AppMessage, ScrollBoundary};

impl App {
    /// Mark the UI as needing a redraw.
    /// Call this method after any state mutation that affects the UI.
    #[inline]
    pub fn mark_dirty(&mut self) {
        self.needs_redraw = true;
    }
    /// Get a clone of the message sender for passing to async tasks
    pub fn message_sender(&self) -> mpsc::UnboundedSender<AppMessage> {
        self.message_tx.clone()
    }

    /// Spawn an async task to check connection status.
    ///
    /// This calls the ConductorClient health_check and sends the result
    /// via the message channel. The App will update connection_status
    /// when the message is received.
    pub fn check_connection(&self) {
        let tx = self.message_tx.clone();
        let client = Arc::clone(&self.client);
        tokio::spawn(async move {
            let connected: bool = (client.health_check().await).unwrap_or_default();
            let _ = tx.send(AppMessage::ConnectionStatus(connected));
        });
    }

    /// Clear the current stream error
    pub fn clear_error(&mut self) {
        self.stream_error = None;
        self.mark_dirty();
    }

    /// Reset scroll state to bottom (newest content)
    pub fn reset_scroll(&mut self) {
        self.conversation_scroll = 0;
        self.scroll_position = 0.0;
        self.scroll_velocity = 0.0;
        self.mark_dirty();
    }

    /// Increment the tick counter for animations and update smooth scrolling
    pub fn tick(&mut self) {
        self.tick_count = self.tick_count.wrapping_add(1);

        // Update smooth scrolling with momentum
        let had_velocity = self.scroll_velocity.abs() > 0.05;
        self.update_smooth_scroll();

        // Mark dirty if there are active animations:
        // - Scroll momentum (velocity > 0)
        // - Streaming (spinner animation)
        // - Boundary hit indicator (fades after a few ticks)
        if had_velocity || self.is_streaming() || self.scroll_boundary_hit.is_some() {
            self.mark_dirty();
        }

        // Clear boundary hit indicator after a few ticks (visual feedback duration)
        if let Some(_boundary) = self.scroll_boundary_hit {
            // Clear after 10 ticks (~160ms at 16ms/tick)
            if self.tick_count.saturating_sub(self.boundary_hit_tick) > 10 {
                self.scroll_boundary_hit = None;
                self.mark_dirty();
            }
        }
    }

    /// Update smooth scroll position with velocity and friction
    fn update_smooth_scroll(&mut self) {
        // Friction factor: lower = more friction, stops faster
        const FRICTION: f32 = 0.80;
        const VELOCITY_THRESHOLD: f32 = 0.05;

        // Skip if no velocity
        if self.scroll_velocity.abs() < VELOCITY_THRESHOLD {
            self.scroll_velocity = 0.0;
            return;
        }

        // Apply velocity to position
        let new_position = self.scroll_position + self.scroll_velocity;

        // Clamp to valid range [0, max_scroll]
        let max = self.max_scroll as f32;
        let clamped_position = new_position.clamp(0.0, max);

        // Check for boundary hits
        if new_position < 0.0 && self.scroll_position >= 0.0 {
            // Hit bottom boundary
            self.scroll_boundary_hit = Some(ScrollBoundary::Bottom);
            self.boundary_hit_tick = self.tick_count;
            self.scroll_velocity = 0.0; // Stop on boundary hit
        } else if new_position > max && self.scroll_position <= max && self.max_scroll > 0 {
            // Hit top boundary
            self.scroll_boundary_hit = Some(ScrollBoundary::Top);
            self.boundary_hit_tick = self.tick_count;
            self.scroll_velocity = 0.0; // Stop on boundary hit
        } else {
            // Apply friction when not hitting boundary
            self.scroll_velocity *= FRICTION;
        }

        // Update positions
        self.scroll_position = clamped_position;
        self.conversation_scroll = clamped_position.round() as u16;
    }

    /// Check if the currently active thread is a Programming thread
    pub fn is_active_thread_programming(&self) -> bool {
        if let Some(thread_id) = &self.active_thread_id {
            if let Some(thread) = self.cache.get_thread(thread_id) {
                return thread.thread_type == crate::models::ThreadType::Programming;
            }
        }
        false
    }

    /// Check if there is currently an active streaming message
    pub fn is_streaming(&self) -> bool {
        if let Some(thread_id) = &self.active_thread_id {
            self.cache.is_thread_streaming(thread_id)
        } else {
            false
        }
    }

    /// Toggle reasoning collapsed state for the last message with reasoning
    /// Returns true if a reasoning block was toggled
    pub fn toggle_reasoning(&mut self) -> bool {
        if let Some(thread_id) = &self.active_thread_id {
            if let Some(idx) = self.cache.find_last_reasoning_message_index(thread_id) {
                let toggled = self.cache.toggle_message_reasoning(thread_id, idx);
                if toggled {
                    self.mark_dirty();
                }
                return toggled;
            }
        }
        false
    }

    /// Dismiss the currently focused error for the active thread
    /// Returns true if an error was dismissed
    pub fn dismiss_focused_error(&mut self) -> bool {
        if let Some(thread_id) = &self.active_thread_id {
            let dismissed = self.cache.dismiss_focused_error(thread_id);
            if dismissed {
                self.mark_dirty();
            }
            dismissed
        } else {
            false
        }
    }

    /// Check if the active thread has any errors
    pub fn has_errors(&self) -> bool {
        if let Some(thread_id) = &self.active_thread_id {
            self.cache.error_count(thread_id) > 0
        } else {
            false
        }
    }

    /// Add an error to the active thread
    pub fn add_error_to_active_thread(&mut self, error_code: String, message: String) {
        if let Some(thread_id) = &self.active_thread_id {
            self.cache.add_error_simple(thread_id, error_code, message);
            self.mark_dirty();
        }
    }

    /// Update terminal dimensions
    ///
    /// Called when the terminal is resized or on initial setup.
    /// Updates both width and height in a single call.
    pub fn update_terminal_dimensions(&mut self, width: u16, height: u16) {
        if self.terminal_width != width || self.terminal_height != height {
            self.terminal_width = width;
            self.terminal_height = height;
            self.mark_dirty();
        }
    }

    /// Get the current terminal width
    pub fn terminal_width(&self) -> u16 {
        self.terminal_width
    }

    /// Get the current terminal height
    pub fn terminal_height(&self) -> u16 {
        self.terminal_height
    }

    /// Calculate the available content area width
    ///
    /// This accounts for borders and margins (2 cells on each side).
    pub fn content_width(&self) -> u16 {
        self.terminal_width.saturating_sub(4)
    }

    /// Calculate the available content area height
    ///
    /// This accounts for header, footer, and borders (approximately 6 rows).
    pub fn content_height(&self) -> u16 {
        self.terminal_height.saturating_sub(6)
    }

    /// Check if pasted text should be summarized
    pub fn should_summarize_paste(&self, text: &str) -> bool {
        let line_count = text.lines().count();
        let char_count = text.chars().count();
        line_count > 3 || char_count > 150
    }
}
