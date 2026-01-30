//! Native OS notification support for task completion events.
//!
//! Sends macOS Notification Center banners when the TUI is not focused
//! and a task (stream) completes. Uses `osascript` on macOS for reliable
//! delivery from terminal apps (no bundle identifier or permissions needed).

/// Send a native OS notification for task completion.
///
/// Spawns a background task so the notification dispatch never blocks
/// the event loop. Errors are logged and silently discarded.
pub fn notify_task_complete(thread_title: Option<&str>) {
    let body = match thread_title {
        Some(title) if !title.is_empty() => format!("Task complete — {}", title),
        _ => "Agent response finished".to_string(),
    };

    tracing::debug!("Sending OS notification: {}", body);

    tokio::spawn(async move {
        let _ = tokio::task::spawn_blocking(move || {
            send_notification("spoq", &body);
        })
        .await;
    });
}

#[cfg(target_os = "macos")]
fn send_notification(title: &str, body: &str) {
    use std::process::Command;

    // Escape double quotes and backslashes for AppleScript string literals
    let escaped_title = title.replace('\\', "\\\\").replace('"', "\\\"");
    let escaped_body = body.replace('\\', "\\\\").replace('"', "\\\"");

    let script = format!(
        "display notification \"{}\" with title \"{}\" sound name \"Glass\"",
        escaped_body, escaped_title
    );

    match Command::new("osascript").arg("-e").arg(&script).output() {
        Ok(output) if !output.status.success() => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::warn!("osascript notification failed: {}", stderr.trim());
        }
        Err(e) => {
            tracing::warn!("Failed to spawn osascript: {}", e);
        }
        _ => {
            tracing::debug!("OS notification sent successfully");
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn send_notification(_title: &str, _body: &str) {
    // No-op on non-macOS platforms for now
}
