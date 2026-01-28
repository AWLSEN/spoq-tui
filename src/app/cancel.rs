//! Stream cancellation functionality for the App.
//!
//! This module provides methods to cancel an active streaming response
//! by calling the backend's `/v1/cancel` endpoint.

use super::{emit_debug, App, AppMessage};
use crate::debug::{DebugEventKind, ErrorData, ErrorSource, StreamLifecycleData, StreamPhase};

impl App {
    /// Cancel the active stream if one is running.
    ///
    /// This sends a cancel request to the backend, which will SIGTERM the
    /// Claude CLI process. The stream will then emit a `cancelled` event
    /// that triggers `StreamCancelled` handling.
    ///
    /// Guards:
    /// - Does nothing if `cancel_in_progress` is already true
    /// - Does nothing if no stream is active (`is_streaming()` returns false)
    /// - Does nothing if there's no active thread
    pub fn cancel_active_stream(&mut self) {
        // Guard: prevent double-cancel
        if self.cancel_in_progress {
            return;
        }

        // Guard: only cancel if actually streaming
        if !self.is_streaming() {
            return;
        }

        // Guard: need an active thread to cancel
        let Some(thread_id) = self.active_thread_id.clone() else {
            return;
        };

        self.cancel_in_progress = true;

        // Emit debug event
        emit_debug(
            &self.debug_tx,
            DebugEventKind::StreamLifecycle(StreamLifecycleData::with_details(
                StreamPhase::Completed,
                "User initiated cancel (Ctrl+C)".to_string(),
            )),
            Some(&thread_id),
        );

        let client = self.client.clone();
        let message_tx = self.message_tx.clone();
        let debug_tx = self.debug_tx.clone();
        let thread_id_for_task = thread_id.clone();

        // Spawn async task to call cancel endpoint
        tokio::spawn(async move {
            match client.cancel_stream(&thread_id_for_task).await {
                Ok(response) => {
                    if response.is_cancelled() {
                        tracing::info!("Stream cancelled: {}", response.message);
                    } else {
                        // not_found - stream may have already completed
                        tracing::debug!(
                            "Cancel returned not_found (stream may have completed): {}",
                            response.message
                        );
                    }
                    // The SSE stream will emit a Cancelled event which triggers StreamCancelled
                }
                Err(e) => {
                    emit_debug(
                        &debug_tx,
                        DebugEventKind::Error(ErrorData::new(
                            ErrorSource::ConductorApi,
                            format!("Cancel request failed: {}", e),
                        )),
                        Some(&thread_id_for_task),
                    );
                    // Send error so UI can display it and reset state
                    let _ = message_tx.send(AppMessage::StreamError {
                        thread_id: thread_id_for_task,
                        error: format!("Cancel failed: {}", e),
                    });
                }
            }
        });
    }

    /// Reset the cancel state.
    ///
    /// This should be called when:
    /// - `StreamComplete` is received
    /// - `StreamCancelled` is received
    /// - `StreamError` is received
    pub fn reset_cancel_state(&mut self) {
        self.cancel_in_progress = false;
    }
}

#[cfg(test)]
mod tests {
    // Note: Full integration tests require mocking the ConductorClient
    // which is beyond the scope of this unit test module.
    // The cancel functionality is tested via manual testing.
}
