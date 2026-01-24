//! File-based credentials provider adapter.
//!
//! This module provides a credentials provider implementation that uses
//! the existing [`CredentialsManager`] for file-based storage.

use async_trait::async_trait;

use crate::auth::credentials::{Credentials, CredentialsManager};
use crate::traits::{CredentialsError, CredentialsProvider};

/// File-based credentials provider.
///
/// This adapter wraps the existing [`CredentialsManager`] and implements
/// the [`CredentialsProvider`] trait, providing async file-based credential
/// storage and retrieval.
///
/// Credentials are stored in `~/.spoq/.credentials.json`.
///
/// # Example
///
/// ```ignore
/// use spoq::adapters::FileCredentialsProvider;
/// use spoq::traits::CredentialsProvider;
///
/// let provider = FileCredentialsProvider::new()?;
///
/// // Load credentials
/// if let Some(creds) = provider.load().await? {
///     if creds.is_valid() {
///         println!("Using stored credentials");
///     }
/// }
///
/// // Save new credentials
/// let creds = Credentials { ... };
/// provider.save(&creds).await?;
/// ```
#[derive(Debug)]
pub struct FileCredentialsProvider {
    manager: CredentialsManager,
}

impl FileCredentialsProvider {
    /// Create a new file-based credentials provider.
    ///
    /// # Returns
    /// The provider, or an error if the home directory cannot be determined.
    pub fn new() -> Result<Self, CredentialsError> {
        CredentialsManager::new()
            .map(|manager| Self { manager })
            .ok_or_else(|| {
                CredentialsError::Other("Failed to determine home directory".to_string())
            })
    }

    /// Get a reference to the underlying credentials manager.
    pub fn manager(&self) -> &CredentialsManager {
        &self.manager
    }

    /// Get the path to the credentials file.
    pub fn credentials_path(&self) -> &std::path::PathBuf {
        self.manager.credentials_path()
    }
}

#[async_trait]
impl CredentialsProvider for FileCredentialsProvider {
    async fn load(&self) -> Result<Option<Credentials>, CredentialsError> {
        // CredentialsManager::load() returns default Credentials if file doesn't exist
        let creds = self.manager.load();

        // Check if credentials have any data (not just defaults)
        if creds.access_token.is_none()
            && creds.refresh_token.is_none()
            && creds.expires_at.is_none()
            && creds.user_id.is_none()
        {
            Ok(None)
        } else {
            Ok(Some(creds))
        }
    }

    async fn save(&self, creds: &Credentials) -> Result<(), CredentialsError> {
        if self.manager.save(creds) {
            Ok(())
        } else {
            Err(CredentialsError::SaveFailed(
                "Failed to write credentials file".to_string(),
            ))
        }
    }

    async fn clear(&self) -> Result<(), CredentialsError> {
        if self.manager.clear() {
            Ok(())
        } else {
            Err(CredentialsError::ClearFailed(
                "Failed to delete credentials file".to_string(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_credentials_provider_new() {
        // This test depends on having a home directory
        let result = FileCredentialsProvider::new();
        assert!(result.is_ok());
    }

    #[test]
    fn test_credentials_path() {
        let provider = FileCredentialsProvider::new().unwrap();
        let path = provider.credentials_path();
        // Path should end with .credentials.json
        assert!(path.ends_with(".credentials.json"));
    }

    #[test]
    fn test_credentials_error_display() {
        let err = CredentialsError::SaveFailed("disk full".to_string());
        assert!(err.to_string().contains("disk full"));

        let err = CredentialsError::ClearFailed("permission denied".to_string());
        assert!(err.to_string().contains("permission denied"));

        let err = CredentialsError::Other("unknown".to_string());
        assert!(err.to_string().contains("unknown"));
    }

    #[test]
    fn test_manager_accessor() {
        let provider = FileCredentialsProvider::new().unwrap();
        let _manager = provider.manager();
        // Just verify we can access the manager
    }

    // Note: Full integration tests for save/load/clear use the real file system
    // and are covered by the CredentialsManager tests in auth/credentials.rs
    // The InMemoryCredentials mock should be used for testing code that
    // depends on CredentialsProvider trait.
}
