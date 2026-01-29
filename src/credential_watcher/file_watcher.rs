//! File-based credential change detection using the `notify` crate.
//!
//! Watches:
//! - ~/.config/gh/hosts.yml (GitHub CLI credentials)
//!
//! Uses FSEvents on macOS for instant (~50ms) change detection.
//! Uses content hashing to detect actual credential changes vs metadata-only updates.

use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::mpsc as std_mpsc;
use tokio::sync::mpsc;

use crate::app::AppMessage;

/// Compute a hash of a file's contents.
/// Returns None if the file can't be read.
fn compute_file_hash(path: &PathBuf) -> Option<u64> {
    let contents = fs::read_to_string(path).ok()?;
    let mut hasher = DefaultHasher::new();
    contents.hash(&mut hasher);
    Some(hasher.finish())
}

/// Paths to watch for credential changes.
///
/// Watches:
/// - ~/.config/gh/hosts.yml (GitHub CLI credentials)
fn get_watch_paths() -> Vec<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
    vec![
        // GitHub CLI credentials
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

    // Compute initial hashes for content-based change detection
    let mut initial_hashes: HashMap<PathBuf, Option<u64>> = HashMap::new();
    for path in &paths {
        if path.exists() {
            let hash = compute_file_hash(path);
            tracing::debug!("Initial hash for {}: {:?}", path.display(), hash);
            initial_hashes.insert(path.clone(), hash);
        }
    }

    // Spawn async task to bridge std channel → tokio channel
    tokio::spawn(async move {
        tracing::debug!("File watcher bridge task started");

        // Track content hashes to detect actual changes vs metadata-only updates
        let mut file_hashes: HashMap<PathBuf, Option<u64>> = initial_hashes;

        loop {
            match event_rx.recv() {
                Ok(event) => {
                    // Extract the path that changed
                    let path_buf = event.paths.first().cloned();
                    let path_str = path_buf
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "unknown".to_string());

                    // Compute new hash and compare with previous
                    if let Some(ref path) = path_buf {
                        let new_hash = compute_file_hash(path);
                        let old_hash = file_hashes.get(path).copied().flatten();

                        // Only trigger if hash actually changed
                        match (old_hash, new_hash) {
                            (Some(old), Some(new)) if old == new => {
                                // Content unchanged - skip (likely metadata-only update)
                                tracing::trace!(
                                    "File modified but content unchanged: {} (hash: {})",
                                    path_str,
                                    old
                                );
                                continue;
                            }
                            (Some(old), Some(new)) => {
                                // Content actually changed
                                tracing::info!(
                                    "Credential file content changed: {} (hash: {} -> {})",
                                    path_str,
                                    old,
                                    new
                                );
                                file_hashes.insert(path.clone(), Some(new));
                            }
                            (None, Some(new)) => {
                                // File appeared or first read succeeded
                                tracing::info!(
                                    "Credential file appeared: {} (hash: {})",
                                    path_str,
                                    new
                                );
                                file_hashes.insert(path.clone(), Some(new));
                            }
                            (Some(_), None) => {
                                // File became unreadable
                                tracing::warn!("Credential file became unreadable: {}", path_str);
                                file_hashes.insert(path.clone(), None);
                                continue; // Don't trigger sync for unreadable files
                            }
                            (None, None) => {
                                // Still unreadable
                                tracing::trace!("Credential file still unreadable: {}", path_str);
                                continue;
                            }
                        }
                    }

                    // Send to main app (only reached if content actually changed)
                    if message_tx
                        .send(AppMessage::CredentialFileChanged { path: path_str })
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
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_get_watch_paths_returns_expected_paths() {
        let paths = get_watch_paths();

        // Should have 1 path:
        // - ~/.config/gh/hosts.yml (GitHub CLI credentials)
        assert_eq!(paths.len(), 1);

        // Check path endings
        let path_strings: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
        assert!(path_strings.iter().any(|p| p.ends_with("hosts.yml")));
    }

    #[test]
    fn test_get_watch_paths_are_absolute() {
        let paths = get_watch_paths();

        for path in paths {
            assert!(path.is_absolute(), "Path should be absolute: {:?}", path);
        }
    }

    #[test]
    fn test_compute_file_hash_deterministic() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "test content").unwrap();
        file.flush().unwrap();

        let path = file.path().to_path_buf();
        let hash1 = compute_file_hash(&path);
        let hash2 = compute_file_hash(&path);

        assert!(hash1.is_some());
        assert_eq!(hash1, hash2, "Same content should produce same hash");
    }

    #[test]
    fn test_compute_file_hash_different_content() {
        let mut file1 = NamedTempFile::new().unwrap();
        writeln!(file1, "content v1").unwrap();
        file1.flush().unwrap();

        let mut file2 = NamedTempFile::new().unwrap();
        writeln!(file2, "content v2").unwrap();
        file2.flush().unwrap();

        let hash1 = compute_file_hash(&file1.path().to_path_buf());
        let hash2 = compute_file_hash(&file2.path().to_path_buf());

        assert!(hash1.is_some());
        assert!(hash2.is_some());
        assert_ne!(hash1, hash2, "Different content should produce different hash");
    }

    #[test]
    fn test_compute_file_hash_nonexistent_file() {
        let path = PathBuf::from("/nonexistent/path/file.json");
        let hash = compute_file_hash(&path);
        assert!(hash.is_none(), "Nonexistent file should return None");
    }
}
