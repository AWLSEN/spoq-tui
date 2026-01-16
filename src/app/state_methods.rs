//! State accessor and utility methods for the App.

use std::sync::Arc;
use tokio::sync::mpsc;

use super::{App, AppMessage, ScrollBoundary};

impl App {
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
    }

    /// Reset scroll state to bottom (newest content)
    pub fn reset_scroll(&mut self) {
        self.conversation_scroll = 0;
        self.scroll_position = 0.0;
        self.scroll_velocity = 0.0;
    }

    /// Increment the tick counter for animations and update smooth scrolling
    pub fn tick(&mut self) {
        self.tick_count = self.tick_count.wrapping_add(1);

        // Update smooth scrolling with momentum
        self.update_smooth_scroll();
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
                return self.cache.toggle_message_reasoning(thread_id, idx);
            }
        }
        false
    }

    /// Dismiss the currently focused error for the active thread
    /// Returns true if an error was dismissed
    pub fn dismiss_focused_error(&mut self) -> bool {
        if let Some(thread_id) = &self.active_thread_id {
            self.cache.dismiss_focused_error(thread_id)
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
        }
    }
}
