//! Native OS notification support for task completion events.
//!
//! Sends macOS Notification Center banners when the TUI is not focused
//! and a task (stream) completes. Uses `notify-rust` on macOS, no-op elsewhere.

/// Send a native OS notification for task completion.
///
/// Dispatches to a blocking thread so `notify-rust`'s synchronous
/// `show()` call never blocks the async runtime. Errors are logged
/// and silently discarded (fire-and-forget).
pub fn notify_task_complete(thread_title: Option<&str>) {
    let body = match thread_title {
        Some(title) => format!("Task complete — {}", title),
        None => "Agent response finished".to_string(),
    };

    tokio::spawn(async move {
        let _ = tokio::task::spawn_blocking(move || {
            send_notification("spoq", &body);
        })
        .await;
    });
}

#[cfg(target_os = "macos")]
fn send_notification(title: &str, body: &str) {
    use notify_rust::Notification;

    if let Err(e) = Notification::new()
        .summary(title)
        .body(body)
        .sound_name("Glass")
        .show()
    {
        tracing::warn!("Failed to send OS notification: {}", e);
    }
}

#[cfg(not(target_os = "macos"))]
fn send_notification(_title: &str, _body: &str) {
    // No-op on non-macOS platforms for now
}
