//! Credentials storage and management for Spoq TUI.
//!
//! This module provides functionality for storing and loading
//! authentication credentials from `~/.spoq/.credentials.json`.

use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Write};
use std::path::PathBuf;

/// The credentials directory name.
const CREDENTIALS_DIR: &str = ".spoq";

/// The credentials file name.
const CREDENTIALS_FILE: &str = ".credentials.json";

/// Authentication credentials for the Spoq platform.
///
/// NOTE: Only authentication tokens are stored locally.
/// VPS state is always fetched from the server via API.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Credentials {
    /// OAuth access token for API authentication.
    pub access_token: Option<String>,
    /// OAuth refresh token for obtaining new access tokens.
    pub refresh_token: Option<String>,
    /// Token expiration time as Unix timestamp (seconds since epoch).
    pub expires_at: Option<i64>,
    /// The authenticated user's ID.
    pub user_id: Option<String>,
}

impl Credentials {
    /// Create new empty credentials.
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if the credentials have an access token.
    pub fn has_token(&self) -> bool {
        self.access_token.is_some()
    }

    /// Check if the token is expired.
    ///
    /// Returns `true` if the token is expired or if there's no expiration time set.
    /// Returns `false` if the token is still valid.
    pub fn is_expired(&self) -> bool {
        match self.expires_at {
            Some(expires_at) => {
                let now = chrono::Utc::now().timestamp();
                now >= expires_at
            }
            None => true, // No expiration means we should consider it expired
        }
    }

    /// Check if the credentials are valid (has token and not expired).
    pub fn is_valid(&self) -> bool {
        self.has_token() && !self.is_expired()
    }
}

/// Manages credential storage and retrieval.
#[derive(Debug)]
pub struct CredentialsManager {
    /// Path to the credentials file.
    credentials_path: PathBuf,
}

impl CredentialsManager {
    /// Create a new CredentialsManager.
    ///
    /// Returns `None` if the home directory cannot be determined.
    pub fn new() -> Option<Self> {
        let home = dirs::home_dir()?;
        let credentials_path = home.join(CREDENTIALS_DIR).join(CREDENTIALS_FILE);
        Some(Self { credentials_path })
    }

    /// Get the path to the credentials file.
    pub fn credentials_path(&self) -> &PathBuf {
        &self.credentials_path
    }

    /// Load credentials from the credentials file.
    ///
    /// Returns default credentials if the file doesn't exist or can't be read.
    pub fn load(&self) -> Credentials {
        if !self.credentials_path.exists() {
            return Credentials::default();
        }

        let file = match File::open(&self.credentials_path) {
            Ok(f) => f,
            Err(_) => return Credentials::default(),
        };

        let reader = BufReader::new(file);
        match serde_json::from_reader(reader) {
            Ok(creds) => creds,
            Err(_) => Credentials::default(),
        }
    }

    /// Save credentials to the credentials file.
    ///
    /// Creates the parent directory if it doesn't exist.
    /// Returns `true` if successful, `false` otherwise.
    pub fn save(&self, credentials: &Credentials) -> bool {
        // Ensure the parent directory exists
        if let Some(parent) = self.credentials_path.parent() {
            if !parent.exists()
                && fs::create_dir_all(parent).is_err() {
                    return false;
                }
        }

        let file = match File::create(&self.credentials_path) {
            Ok(f) => f,
            Err(_) => return false,
        };

        let mut writer = BufWriter::new(file);
        if serde_json::to_writer_pretty(&mut writer, credentials).is_err() {
            return false;
        }

        writer.flush().is_ok()
    }

    /// Clear all stored credentials.
    ///
    /// Removes the credentials file if it exists.
    /// Returns `true` if successful or file didn't exist, `false` otherwise.
    pub fn clear(&self) -> bool {
        if !self.credentials_path.exists() {
            return true;
        }

        fs::remove_file(&self.credentials_path).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // Helper to create a CredentialsManager with a custom path
    fn create_test_manager(temp_dir: &TempDir) -> CredentialsManager {
        let credentials_path = temp_dir.path().join(CREDENTIALS_DIR).join(CREDENTIALS_FILE);
        CredentialsManager { credentials_path }
    }

    #[test]
    fn test_credentials_default() {
        let creds = Credentials::default();
        assert!(creds.access_token.is_none());
        assert!(creds.refresh_token.is_none());
        assert!(creds.expires_at.is_none());
        assert!(creds.user_id.is_none());
    }

    #[test]
    fn test_credentials_new() {
        let creds = Credentials::new();
        assert_eq!(creds, Credentials::default());
    }

    #[test]
    fn test_credentials_has_token() {
        let mut creds = Credentials::default();
        assert!(!creds.has_token());

        creds.access_token = Some("test-token".to_string());
        assert!(creds.has_token());
    }

    #[test]
    fn test_credentials_is_expired_no_expiration() {
        let creds = Credentials::default();
        assert!(creds.is_expired());
    }

    #[test]
    fn test_credentials_is_expired_past() {
        let mut creds = Credentials::default();
        creds.expires_at = Some(0); // Unix epoch - definitely in the past
        assert!(creds.is_expired());
    }

    #[test]
    fn test_credentials_is_expired_future() {
        let mut creds = Credentials::default();
        // Set expiration to 1 hour in the future
        creds.expires_at = Some(chrono::Utc::now().timestamp() + 3600);
        assert!(!creds.is_expired());
    }

    #[test]
    fn test_credentials_is_valid() {
        let mut creds = Credentials::default();
        assert!(!creds.is_valid());

        creds.access_token = Some("test-token".to_string());
        assert!(!creds.is_valid()); // Still invalid - no expiration

        creds.expires_at = Some(chrono::Utc::now().timestamp() + 3600);
        assert!(creds.is_valid()); // Now valid
    }

    #[test]
    fn test_credentials_manager_new() {
        // This test depends on having a home directory, which should be available
        let manager = CredentialsManager::new();
        assert!(manager.is_some());
    }

    #[test]
    fn test_credentials_manager_load_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let manager = create_test_manager(&temp_dir);
        let creds = manager.load();
        assert_eq!(creds, Credentials::default());
    }

    #[test]
    fn test_credentials_manager_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let manager = create_test_manager(&temp_dir);

        let creds = Credentials {
            access_token: Some("test-access-token".to_string()),
            refresh_token: Some("test-refresh-token".to_string()),
            expires_at: Some(1234567890),
            user_id: Some("user-123".to_string()),
        };

        assert!(manager.save(&creds));

        let loaded = manager.load();
        assert_eq!(loaded, creds);
    }

    #[test]
    fn test_credentials_manager_clear() {
        let temp_dir = TempDir::new().unwrap();
        let manager = create_test_manager(&temp_dir);

        // Save some credentials first
        let creds = Credentials {
            access_token: Some("test-token".to_string()),
            ..Default::default()
        };
        assert!(manager.save(&creds));
        assert!(manager.credentials_path.exists());

        // Clear them
        assert!(manager.clear());
        assert!(!manager.credentials_path.exists());

        // Load should return default
        let loaded = manager.load();
        assert_eq!(loaded, Credentials::default());
    }

    #[test]
    fn test_credentials_manager_clear_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let manager = create_test_manager(&temp_dir);

        // Clear should succeed even if file doesn't exist
        assert!(manager.clear());
    }

    #[test]
    fn test_credentials_manager_creates_parent_dir() {
        let temp_dir = TempDir::new().unwrap();
        let manager = create_test_manager(&temp_dir);

        let creds = Credentials {
            access_token: Some("test-token".to_string()),
            ..Default::default()
        };

        // Parent directory doesn't exist yet
        assert!(!manager.credentials_path.parent().unwrap().exists());

        // Save should create it
        assert!(manager.save(&creds));
        assert!(manager.credentials_path.parent().unwrap().exists());
    }

    #[test]
    fn test_credentials_serialization() {
        let creds = Credentials {
            access_token: Some("token".to_string()),
            refresh_token: Some("refresh".to_string()),
            expires_at: Some(1234567890),
            user_id: Some("user-id".to_string()),
        };

        let json = serde_json::to_string(&creds).unwrap();
        let deserialized: Credentials = serde_json::from_str(&json).unwrap();
        assert_eq!(creds, deserialized);
    }

    #[test]
    fn test_credentials_load_invalid_json() {
        let temp_dir = TempDir::new().unwrap();
        let manager = create_test_manager(&temp_dir);

        // Create directory and write invalid JSON
        fs::create_dir_all(manager.credentials_path.parent().unwrap()).unwrap();
        fs::write(&manager.credentials_path, "not valid json").unwrap();

        // Should return default credentials
        let loaded = manager.load();
        assert_eq!(loaded, Credentials::default());
    }

    #[test]
    fn test_credentials_backward_compatibility() {
        // Test that old credentials.json with extra fields can still be loaded
        // (serde ignores unknown fields by default)
        let json_with_extra_fields = r#"{
            "access_token": "old-token",
            "refresh_token": "old-refresh",
            "expires_at": 9999999999,
            "user_id": "old-user",
            "vps_id": "old-vps",
            "vps_url": "http://old.example.com"
        }"#;

        let creds: Credentials = serde_json::from_str(json_with_extra_fields).unwrap();

        assert_eq!(creds.access_token, Some("old-token".to_string()));
        assert_eq!(creds.refresh_token, Some("old-refresh".to_string()));
        assert_eq!(creds.expires_at, Some(9999999999));
        assert_eq!(creds.user_id, Some("old-user".to_string()));
        // Extra fields are ignored
    }
}
