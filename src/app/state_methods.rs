//! State accessor and utility methods for the App.

use std::sync::Arc;
use tokio::sync::mpsc;

use super::{App, AppMessage};

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
            let connected = match client.health_check().await {
                Ok(healthy) => healthy,
                Err(_) => false,
            };
            let _ = tx.send(AppMessage::ConnectionStatus(connected));
        });
    }

    /// Clear the current stream error
    pub fn clear_error(&mut self) {
        self.stream_error = None;
    }

    /// Increment the tick counter for animations
    pub fn tick(&mut self) {
        self.tick_count = self.tick_count.wrapping_add(1);
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
