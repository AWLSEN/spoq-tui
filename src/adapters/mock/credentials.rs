//! In-memory credentials provider for testing.
//!
//! Provides a credentials provider that stores credentials in memory,
//! suitable for testing without file system access.

use async_trait::async_trait;
use std::sync::{Arc, Mutex};

use crate::auth::credentials::Credentials;
use crate::traits::{CredentialsError, CredentialsProvider};

/// In-memory credentials provider for testing.
///
/// This provider stores credentials in memory, allowing tests to
/// verify credential operations without touching the file system.
///
/// # Example
///
/// ```ignore
/// use spoq::adapters::mock::InMemoryCredentials;
/// use spoq::traits::CredentialsProvider;
/// use spoq::auth::Credentials;
///
/// let provider = InMemoryCredentials::new();
///
/// // Initially empty
/// assert!(provider.load().await?.is_none());
///
/// // Save credentials
/// let creds = Credentials {
///     access_token: Some("test-token".to_string()),
///     ..Default::default()
/// };
/// provider.save(&creds).await?;
///
/// // Load them back
/// let loaded = provider.load().await?.unwrap();
/// assert_eq!(loaded.access_token, Some("test-token".to_string()));
///
/// // Clear
/// provider.clear().await?;
/// assert!(provider.load().await?.is_none());
/// ```
#[derive(Debug, Clone)]
pub struct InMemoryCredentials {
    /// Stored credentials
    credentials: Arc<Mutex<Option<Credentials>>>,
    /// Whether save should fail
    save_should_fail: Arc<Mutex<bool>>,
    /// Whether load should fail
    load_should_fail: Arc<Mutex<bool>>,
    /// Whether clear should fail
    clear_should_fail: Arc<Mutex<bool>>,
}

impl InMemoryCredentials {
    /// Create a new in-memory credentials provider.
    pub fn new() -> Self {
        Self {
            credentials: Arc::new(Mutex::new(None)),
            save_should_fail: Arc::new(Mutex::new(false)),
            load_should_fail: Arc::new(Mutex::new(false)),
            clear_should_fail: Arc::new(Mutex::new(false)),
        }
    }

    /// Create a provider with initial credentials.
    pub fn with_credentials(creds: Credentials) -> Self {
        Self {
            credentials: Arc::new(Mutex::new(Some(creds))),
            save_should_fail: Arc::new(Mutex::new(false)),
            load_should_fail: Arc::new(Mutex::new(false)),
            clear_should_fail: Arc::new(Mutex::new(false)),
        }
    }

    /// Configure whether save should fail.
    pub fn set_save_should_fail(&self, should_fail: bool) {
        *self.save_should_fail.lock().unwrap() = should_fail;
    }

    /// Configure whether load should fail.
    pub fn set_load_should_fail(&self, should_fail: bool) {
        *self.load_should_fail.lock().unwrap() = should_fail;
    }

    /// Configure whether clear should fail.
    pub fn set_clear_should_fail(&self, should_fail: bool) {
        *self.clear_should_fail.lock().unwrap() = should_fail;
    }

    /// Get the current credentials synchronously (for testing).
    pub fn get_credentials(&self) -> Option<Credentials> {
        self.credentials.lock().unwrap().clone()
    }

    /// Set credentials synchronously (for testing).
    pub fn set_credentials(&self, creds: Option<Credentials>) {
        *self.credentials.lock().unwrap() = creds;
    }
}

impl Default for InMemoryCredentials {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CredentialsProvider for InMemoryCredentials {
    async fn load(&self) -> Result<Option<Credentials>, CredentialsError> {
        if *self.load_should_fail.lock().unwrap() {
            return Err(CredentialsError::LoadFailed("Mock load failure".to_string()));
        }

        Ok(self.credentials.lock().unwrap().clone())
    }

    async fn save(&self, creds: &Credentials) -> Result<(), CredentialsError> {
        if *self.save_should_fail.lock().unwrap() {
            return Err(CredentialsError::SaveFailed("Mock save failure".to_string()));
        }

        *self.credentials.lock().unwrap() = Some(creds.clone());
        Ok(())
    }

    async fn clear(&self) -> Result<(), CredentialsError> {
        if *self.clear_should_fail.lock().unwrap() {
            return Err(CredentialsError::ClearFailed("Mock clear failure".to_string()));
        }

        *self.credentials.lock().unwrap() = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_in_memory_credentials_new() {
        let provider = InMemoryCredentials::new();
        assert!(provider.get_credentials().is_none());
    }

    #[test]
    fn test_in_memory_credentials_default() {
        let provider = InMemoryCredentials::default();
        assert!(provider.get_credentials().is_none());
    }

    #[test]
    fn test_with_credentials() {
        let creds = Credentials {
            access_token: Some("initial-token".to_string()),
            ..Default::default()
        };
        let provider = InMemoryCredentials::with_credentials(creds);

        let loaded = provider.get_credentials().unwrap();
        assert_eq!(loaded.access_token, Some("initial-token".to_string()));
    }

    #[tokio::test]
    async fn test_load_empty() {
        let provider = InMemoryCredentials::new();
        let result = provider.load().await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_save_and_load() {
        let provider = InMemoryCredentials::new();

        let creds = Credentials {
            access_token: Some("test-token".to_string()),
            refresh_token: Some("test-refresh".to_string()),
            expires_at: Some(9999999999),
            user_id: Some("user-123".to_string()),
        };

        provider.save(&creds).await.unwrap();

        let loaded = provider.load().await.unwrap().unwrap();
        assert_eq!(loaded.access_token, Some("test-token".to_string()));
        assert_eq!(loaded.refresh_token, Some("test-refresh".to_string()));
        assert_eq!(loaded.expires_at, Some(9999999999));
        assert_eq!(loaded.user_id, Some("user-123".to_string()));
    }

    #[tokio::test]
    async fn test_clear() {
        let provider = InMemoryCredentials::new();

        let creds = Credentials {
            access_token: Some("test-token".to_string()),
            ..Default::default()
        };

        provider.save(&creds).await.unwrap();
        assert!(provider.load().await.unwrap().is_some());

        provider.clear().await.unwrap();
        assert!(provider.load().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_load_failure() {
        let provider = InMemoryCredentials::new();
        provider.set_load_should_fail(true);

        let result = provider.load().await;
        assert!(result.is_err());
        assert!(matches!(result, Err(CredentialsError::LoadFailed(_))));
    }

    #[tokio::test]
    async fn test_save_failure() {
        let provider = InMemoryCredentials::new();
        provider.set_save_should_fail(true);

        let creds = Credentials::default();
        let result = provider.save(&creds).await;

        assert!(result.is_err());
        assert!(matches!(result, Err(CredentialsError::SaveFailed(_))));
    }

    #[tokio::test]
    async fn test_clear_failure() {
        let provider = InMemoryCredentials::new();
        provider.set_clear_should_fail(true);

        let result = provider.clear().await;
        assert!(result.is_err());
        assert!(matches!(result, Err(CredentialsError::ClearFailed(_))));
    }

    #[test]
    fn test_clone() {
        let provider = InMemoryCredentials::new();

        let creds = Credentials {
            access_token: Some("shared-token".to_string()),
            ..Default::default()
        };
        provider.set_credentials(Some(creds));

        let cloned = provider.clone();

        // Both should share the same credentials
        let loaded = cloned.get_credentials().unwrap();
        assert_eq!(loaded.access_token, Some("shared-token".to_string()));

        // Modifying one affects the other
        provider.set_credentials(None);
        assert!(cloned.get_credentials().is_none());
    }

    #[test]
    fn test_set_credentials() {
        let provider = InMemoryCredentials::new();

        provider.set_credentials(Some(Credentials {
            access_token: Some("direct-set".to_string()),
            ..Default::default()
        }));

        let loaded = provider.get_credentials().unwrap();
        assert_eq!(loaded.access_token, Some("direct-set".to_string()));
    }

    #[tokio::test]
    async fn test_overwrite_credentials() {
        let provider = InMemoryCredentials::new();

        let creds1 = Credentials {
            access_token: Some("token-1".to_string()),
            ..Default::default()
        };
        provider.save(&creds1).await.unwrap();

        let creds2 = Credentials {
            access_token: Some("token-2".to_string()),
            ..Default::default()
        };
        provider.save(&creds2).await.unwrap();

        let loaded = provider.load().await.unwrap().unwrap();
        assert_eq!(loaded.access_token, Some("token-2".to_string()));
    }

    #[tokio::test]
    async fn test_credentials_isolation() {
        // Test that different providers don't share state
        let provider1 = InMemoryCredentials::new();
        let provider2 = InMemoryCredentials::new();

        let creds = Credentials {
            access_token: Some("isolated-token".to_string()),
            ..Default::default()
        };
        provider1.save(&creds).await.unwrap();

        // provider2 should still be empty
        assert!(provider2.load().await.unwrap().is_none());
    }
}
