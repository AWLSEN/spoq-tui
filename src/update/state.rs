//! Update state tracking for Spoq CLI.
//!
//! This module provides functionality for tracking update status
//! from `~/.spoq/update_state.json`.

use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Write};
use std::path::PathBuf;

/// The update state directory name.
const UPDATE_STATE_DIR: &str = ".spoq";

/// The update state file name.
const UPDATE_STATE_FILE: &str = "update_state.json";

/// Update state for the Spoq CLI.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateState {
    /// Unix timestamp of the last update check (seconds since epoch).
    pub last_check: Option<i64>,
    /// Path to the downloaded update binary, if available.
    pub pending_update_path: Option<String>,
    /// Version of the available update, if any.
    pub available_version: Option<String>,
}

impl UpdateState {
    /// Create new empty update state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if there is a pending update available.
    pub fn has_pending_update(&self) -> bool {
        self.pending_update_path.is_some() && self.available_version.is_some()
    }

    /// Clear the pending update information.
    pub fn clear_pending_update(&mut self) {
        self.pending_update_path = None;
        self.available_version = None;
    }
}

/// Manages update state storage and retrieval.
#[derive(Debug)]
pub struct UpdateStateManager {
    /// Path to the update state file.
    state_path: PathBuf,
}

impl UpdateStateManager {
    /// Create a new UpdateStateManager.
    ///
    /// Returns `None` if the home directory cannot be determined.
    pub fn new() -> Option<Self> {
        let home = dirs::home_dir()?;
        let state_path = home.join(UPDATE_STATE_DIR).join(UPDATE_STATE_FILE);
        Some(Self { state_path })
    }

    /// Get the path to the update state file.
    pub fn state_path(&self) -> &PathBuf {
        &self.state_path
    }

    /// Load update state from the state file.
    ///
    /// Returns default state if the file doesn't exist or can't be read.
    pub fn load(&self) -> UpdateState {
        if !self.state_path.exists() {
            return UpdateState::default();
        }

        let file = match File::open(&self.state_path) {
            Ok(f) => f,
            Err(_) => return UpdateState::default(),
        };

        let reader = BufReader::new(file);
        match serde_json::from_reader(reader) {
            Ok(state) => state,
            Err(_) => UpdateState::default(),
        }
    }

    /// Save update state to the state file.
    ///
    /// Creates the parent directory if it doesn't exist.
    /// Returns `true` if successful, `false` otherwise.
    pub fn save(&self, state: &UpdateState) -> bool {
        // Ensure the parent directory exists
        if let Some(parent) = self.state_path.parent() {
            if !parent.exists() && fs::create_dir_all(parent).is_err() {
                return false;
            }
        }

        let file = match File::create(&self.state_path) {
            Ok(f) => f,
            Err(_) => return false,
        };

        let mut writer = BufWriter::new(file);
        if serde_json::to_writer_pretty(&mut writer, state).is_err() {
            return false;
        }

        writer.flush().is_ok()
    }

    /// Clear all stored update state.
    ///
    /// Removes the state file if it exists.
    /// Returns `true` if successful or file didn't exist, `false` otherwise.
    pub fn clear(&self) -> bool {
        if !self.state_path.exists() {
            return true;
        }

        fs::remove_file(&self.state_path).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // Helper to create an UpdateStateManager with a custom path
    fn create_test_manager(temp_dir: &TempDir) -> UpdateStateManager {
        let state_path = temp_dir.path().join(UPDATE_STATE_DIR).join(UPDATE_STATE_FILE);
        UpdateStateManager { state_path }
    }

    #[test]
    fn test_update_state_default() {
        let state = UpdateState::default();
        assert!(state.last_check.is_none());
        assert!(state.pending_update_path.is_none());
        assert!(state.available_version.is_none());
    }

    #[test]
    fn test_update_state_new() {
        let state = UpdateState::new();
        assert_eq!(state, UpdateState::default());
    }

    #[test]
    fn test_update_state_has_pending_update() {
        let mut state = UpdateState::default();
        assert!(!state.has_pending_update());

        state.pending_update_path = Some("/tmp/spoq-update".to_string());
        assert!(!state.has_pending_update()); // Needs both path and version

        state.available_version = Some("1.2.3".to_string());
        assert!(state.has_pending_update());
    }

    #[test]
    fn test_update_state_clear_pending_update() {
        let mut state = UpdateState {
            last_check: Some(1234567890),
            pending_update_path: Some("/tmp/spoq-update".to_string()),
            available_version: Some("1.2.3".to_string()),
        };

        state.clear_pending_update();

        assert_eq!(state.last_check, Some(1234567890)); // Should not be cleared
        assert!(state.pending_update_path.is_none());
        assert!(state.available_version.is_none());
    }

    #[test]
    fn test_update_state_manager_new() {
        // This test depends on having a home directory, which should be available
        let manager = UpdateStateManager::new();
        assert!(manager.is_some());
    }

    #[test]
    fn test_update_state_manager_load_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let manager = create_test_manager(&temp_dir);
        let state = manager.load();
        assert_eq!(state, UpdateState::default());
    }

    #[test]
    fn test_update_state_manager_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let manager = create_test_manager(&temp_dir);

        let state = UpdateState {
            last_check: Some(1234567890),
            pending_update_path: Some("/tmp/spoq-update".to_string()),
            available_version: Some("1.2.3".to_string()),
        };

        assert!(manager.save(&state));

        let loaded = manager.load();
        assert_eq!(loaded, state);
    }

    #[test]
    fn test_update_state_manager_clear() {
        let temp_dir = TempDir::new().unwrap();
        let manager = create_test_manager(&temp_dir);

        // Save some state first
        let state = UpdateState {
            last_check: Some(1234567890),
            ..Default::default()
        };
        assert!(manager.save(&state));
        assert!(manager.state_path.exists());

        // Clear it
        assert!(manager.clear());
        assert!(!manager.state_path.exists());

        // Load should return default
        let loaded = manager.load();
        assert_eq!(loaded, UpdateState::default());
    }

    #[test]
    fn test_update_state_manager_clear_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let manager = create_test_manager(&temp_dir);

        // Clear should succeed even if file doesn't exist
        assert!(manager.clear());
    }

    #[test]
    fn test_update_state_manager_creates_parent_dir() {
        let temp_dir = TempDir::new().unwrap();
        let manager = create_test_manager(&temp_dir);

        let state = UpdateState {
            last_check: Some(1234567890),
            ..Default::default()
        };

        // Parent directory doesn't exist yet
        assert!(!manager.state_path.parent().unwrap().exists());

        // Save should create it
        assert!(manager.save(&state));
        assert!(manager.state_path.parent().unwrap().exists());
    }

    #[test]
    fn test_update_state_serialization() {
        let state = UpdateState {
            last_check: Some(1234567890),
            pending_update_path: Some("/tmp/spoq-update".to_string()),
            available_version: Some("1.2.3".to_string()),
        };

        let json = serde_json::to_string(&state).unwrap();
        let deserialized: UpdateState = serde_json::from_str(&json).unwrap();
        assert_eq!(state, deserialized);
    }

    #[test]
    fn test_update_state_load_invalid_json() {
        let temp_dir = TempDir::new().unwrap();
        let manager = create_test_manager(&temp_dir);

        // Create directory and write invalid JSON
        fs::create_dir_all(manager.state_path.parent().unwrap()).unwrap();
        fs::write(&manager.state_path, "not valid json").unwrap();

        // Should return default state
        let loaded = manager.load();
        assert_eq!(loaded, UpdateState::default());
    }

    #[test]
    fn test_update_state_partial_fields() {
        // Test that state can be loaded with only some fields present
        let json_partial = r#"{
            "last_check": 9999999999
        }"#;

        let state: UpdateState = serde_json::from_str(json_partial).unwrap();

        assert_eq!(state.last_check, Some(9999999999));
        assert_eq!(state.pending_update_path, None);
        assert_eq!(state.available_version, None);
    }
}
