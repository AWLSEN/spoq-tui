//! Utility functions for the application.
//!
//! Contains helper functions for debugging and logging:
//! - [`truncate_for_debug`] - Truncate strings for debug output
//! - [`log_thread_update`] - Log thread metadata updates
//! - [`emit_debug`] - Emit debug events

use crate::debug::{DebugEvent, DebugEventKind, DebugEventSender};
use chrono::Utc;
use std::fs::OpenOptions;
use std::io::Write;

/// Truncate a string for debug output, adding "..." if truncated.
/// Uses char boundaries to avoid panicking on multi-byte UTF-8 characters.
pub(crate) fn truncate_for_debug(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        // Find a valid char boundary at or before max_len - 3
        let target = max_len.saturating_sub(3);
        let boundary = s
            .char_indices()
            .take_while(|(i, _)| *i <= target)
            .last()
            .map(|(i, _)| i)
            .unwrap_or(0);
        format!("{}...", &s[..boundary])
    }
}

/// Log thread metadata updates to a dedicated file for debugging
pub(crate) fn log_thread_update(message: &str) {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let log_path = format!("{}/spoq_thread.log", home);
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&log_path) {
        let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let _ = writeln!(file, "[{}] {}", timestamp, message);
        let _ = file.flush();
    }
}

/// Helper to emit a debug event if debug channel is available.
pub(crate) fn emit_debug(
    debug_tx: &Option<DebugEventSender>,
    kind: DebugEventKind,
    thread_id: Option<&str>,
) {
    if let Some(ref tx) = debug_tx {
        let event = DebugEvent::with_context(kind, thread_id.map(String::from), None);
        let _ = tx.send(event);
    }
}
