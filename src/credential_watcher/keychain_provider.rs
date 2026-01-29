//! Keychain provider trait for dependency injection.
//!
//! Allows testing keychain polling without modifying real credentials.
//!
//! # Usage
//!
//! ```rust,ignore
//! // Production: use RealKeychain
//! let provider = Arc::new(RealKeychain);
//! spawn_keychain_poller_with_provider(tx, provider);
//!
//! // Testing: use MockKeychain
//! let mock = Arc::new(MockKeychain::with_credentials("test_token"));
//! mock.set_credentials(Some("new_token".to_string())); // Simulate change
//! ```

use std::sync::{Arc, Mutex};

/// Trait for reading keychain credentials.
///
/// Implementations must be thread-safe (Send + Sync) to work with async polling.
pub trait KeychainProvider: Send + Sync {
    /// Read Claude Code credentials from keychain.
    ///
    /// Returns None if credentials don't exist or can't be read.
    fn read_credentials(&self) -> Option<String>;
}

/// Production implementation - reads real macOS Keychain.
#[cfg(target_os = "macos")]
pub struct RealKeychain;

#[cfg(target_os = "macos")]
impl KeychainProvider for RealKeychain {
    fn read_credentials(&self) -> Option<String> {
        crate::conductor::read_claude_keychain_credentials()
    }
}

/// Stub for non-macOS platforms.
#[cfg(not(target_os = "macos"))]
pub struct RealKeychain;

#[cfg(not(target_os = "macos"))]
impl KeychainProvider for RealKeychain {
    fn read_credentials(&self) -> Option<String> {
        None
    }
}

/// Mock implementation for testing.
///
/// Provides controllable credentials that can be changed during tests
/// to simulate keychain updates without touching real credentials.
///
/// # Thread Safety
///
/// Uses `Arc<Mutex<>>` internally for thread-safe access across async boundaries.
pub struct MockKeychain {
    credentials: Arc<Mutex<Option<String>>>,
}

impl MockKeychain {
    /// Create a new MockKeychain with no credentials.
    pub fn new() -> Self {
        Self {
            credentials: Arc::new(Mutex::new(None)),
        }
    }

    /// Create a new MockKeychain with initial credentials.
    pub fn with_credentials(creds: &str) -> Self {
        Self {
            credentials: Arc::new(Mutex::new(Some(creds.to_string()))),
        }
    }

    /// Set or clear credentials (simulates keychain change).
    pub fn set_credentials(&self, creds: Option<String>) {
        *self.credentials.lock().unwrap() = creds;
    }

    /// Get current credentials (for test assertions).
    pub fn get_credentials(&self) -> Option<String> {
        self.credentials.lock().unwrap().clone()
    }
}

impl KeychainProvider for MockKeychain {
    fn read_credentials(&self) -> Option<String> {
        self.credentials.lock().unwrap().clone()
    }
}

impl Default for MockKeychain {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_keychain_new_is_empty() {
        let mock = MockKeychain::new();
        assert!(mock.read_credentials().is_none());
    }

    #[test]
    fn test_mock_keychain_with_credentials() {
        let mock = MockKeychain::with_credentials("test_token");
        assert_eq!(mock.read_credentials(), Some("test_token".to_string()));
    }

    #[test]
    fn test_mock_keychain_set_credentials() {
        let mock = MockKeychain::new();
        assert!(mock.read_credentials().is_none());

        mock.set_credentials(Some("new_token".to_string()));
        assert_eq!(mock.read_credentials(), Some("new_token".to_string()));

        mock.set_credentials(None);
        assert!(mock.read_credentials().is_none());
    }

    #[test]
    fn test_mock_keychain_get_credentials() {
        let mock = MockKeychain::with_credentials("test");
        assert_eq!(mock.get_credentials(), Some("test".to_string()));
    }

    #[test]
    fn test_mock_keychain_thread_safe() {
        use std::thread;

        let mock = Arc::new(MockKeychain::with_credentials("initial"));
        let mock_clone = mock.clone();

        let handle = thread::spawn(move || {
            mock_clone.set_credentials(Some("updated".to_string()));
        });

        handle.join().unwrap();

        assert_eq!(mock.get_credentials(), Some("updated".to_string()));
    }

    #[test]
    fn test_mock_keychain_default() {
        let mock = MockKeychain::default();
        assert!(mock.read_credentials().is_none());
    }
}
