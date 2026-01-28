//! File-based credential change detection using the `notify` crate.
//!
//! Watches:
//! - ~/.claude.json (Claude Code metadata)
//! - ~/.config/gh/hosts.yml (GitHub CLI credentials)
//!
//! Uses FSEvents on macOS for instant (~50ms) change detection.

use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc as std_mpsc;
use tokio::sync::mpsc;

use crate::app::AppMessage;

/// Paths to watch for credential changes.
fn get_watch_paths() -> Vec<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
    vec![
        PathBuf::from(&home).join(".claude.json"),
        PathBuf::from(&home).join(".config/gh/hosts.yml"),
    ]
}

/// Spawn the file watcher system.
///
/// Returns the watcher handle (MUST be kept alive - dropping it stops watching).
/// Spawns an async task that bridges notify events to AppMessage.
pub fn spawn_file_watcher(
    message_tx: mpsc::UnboundedSender<AppMessage>,
) -> notify::Result<RecommendedWatcher> {
    let paths = get_watch_paths();

    // Create a std channel for the notify callback (notify doesn't support async)
    let (event_tx, event_rx) = std_mpsc::channel::<Event>();

    // Create the watcher with our callback
    let mut watcher = RecommendedWatcher::new(
        move |result: Result<Event, notify::Error>| {
            if let Ok(event) = result {
                // Only care about modifications and creations
                match event.kind {
                    EventKind::Modify(_) | EventKind::Create(_) => {
                        let _ = event_tx.send(event);
                    }
                    _ => {} // Ignore access, remove, other events
                }
            }
        },
        Config::default(),
    )?;

    // Track how many paths we're watching
    let mut watched_count = 0;

    // Watch each path (if it exists)
    for path in &paths {
        if path.exists() {
            if let Err(e) = watcher.watch(path, RecursiveMode::NonRecursive) {
                tracing::warn!("Failed to watch {}: {}", path.display(), e);
            } else {
                tracing::debug!("Watching for changes: {}", path.display());
                watched_count += 1;
            }
        } else {
            tracing::debug!(
                "Path doesn't exist yet, skipping watch: {}",
                path.display()
            );
            // Note: Could watch parent directory and add watch when file is created
            // but for now we keep it simple - file needs to exist at startup
        }
    }

    // Spawn async task to bridge std channel → tokio channel
    tokio::spawn(async move {
        tracing::debug!("File watcher bridge task started");
        loop {
            match event_rx.recv() {
                Ok(event) => {
                    // Extract the path that changed
                    let path = event
                        .paths
                        .first()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "unknown".to_string());

                    tracing::info!("Credential file changed: {}", path);

                    // Send to main app
                    if message_tx
                        .send(AppMessage::CredentialFileChanged { path })
                        .is_err()
                    {
                        tracing::debug!("Message channel closed, stopping file watcher");
                        break;
                    }
                }
                Err(_) => {
                    // Channel closed (watcher dropped)
                    tracing::debug!("File watcher channel closed");
                    break;
                }
            }
        }
    });

    tracing::info!(
        "File watcher initialized for {} paths",
        watched_count
    );

    Ok(watcher)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_watch_paths_returns_expected_paths() {
        let paths = get_watch_paths();

        // Should have at least 2 paths
        assert!(paths.len() >= 2);

        // Check path endings
        let path_strings: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
        assert!(path_strings.iter().any(|p| p.ends_with(".claude.json")));
        assert!(path_strings.iter().any(|p| p.ends_with("hosts.yml")));
    }

    #[test]
    fn test_get_watch_paths_are_absolute() {
        let paths = get_watch_paths();

        for path in paths {
            assert!(path.is_absolute(), "Path should be absolute: {:?}", path);
        }
    }
}
