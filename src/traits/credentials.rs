//! Credentials provider trait abstraction.
//!
//! Provides a trait-based abstraction for credentials storage and retrieval,
//! enabling dependency injection and mocking in tests.

use async_trait::async_trait;

use crate::auth::Credentials;

/// Credentials operation errors.
#[derive(Debug, Clone)]
pub enum CredentialsError {
    /// Failed to load credentials
    LoadFailed(String),
    /// Failed to save credentials
    SaveFailed(String),
    /// Failed to clear credentials
    ClearFailed(String),
    /// Credentials not found
    NotFound,
    /// IO error
    Io(String),
    /// Serialization/deserialization error
    Serialization(String),
    /// Other error
    Other(String),
}

impl std::fmt::Display for CredentialsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CredentialsError::LoadFailed(msg) => write!(f, "Failed to load credentials: {}", msg),
            CredentialsError::SaveFailed(msg) => write!(f, "Failed to save credentials: {}", msg),
            CredentialsError::ClearFailed(msg) => {
                write!(f, "Failed to clear credentials: {}", msg)
            }
            CredentialsError::NotFound => write!(f, "Credentials not found"),
            CredentialsError::Io(msg) => write!(f, "IO error: {}", msg),
            CredentialsError::Serialization(msg) => write!(f, "Serialization error: {}", msg),
            CredentialsError::Other(msg) => write!(f, "Credentials error: {}", msg),
        }
    }
}

impl std::error::Error for CredentialsError {}

/// Trait for credentials storage and retrieval.
///
/// This trait abstracts credentials operations to enable dependency injection
/// and mocking in tests. Implementations include the production file-based
/// storage and mock providers for testing.
///
/// # Example
///
/// ```ignore
/// use spoq::traits::CredentialsProvider;
/// use spoq::auth::Credentials;
///
/// async fn authenticate<P: CredentialsProvider>(provider: &P) -> Result<(), CredentialsError> {
///     // Try to load existing credentials
///     if let Some(creds) = provider.load().await? {
///         if creds.is_valid() {
///             return Ok(());
///         }
///     }
///
///     // Perform authentication and save new credentials
///     let new_creds = Credentials { ... };
///     provider.save(&new_creds).await?;
///     Ok(())
/// }
/// ```
#[async_trait]
pub trait CredentialsProvider: Send + Sync {
    /// Load credentials from storage.
    ///
    /// # Returns
    /// - `Ok(Some(credentials))` if credentials exist and were loaded successfully
    /// - `Ok(None)` if no credentials are stored
    /// - `Err(error)` if loading failed
    async fn load(&self) -> Result<Option<Credentials>, CredentialsError>;

    /// Save credentials to storage.
    ///
    /// # Arguments
    /// * `creds` - The credentials to save
    ///
    /// # Returns
    /// Ok(()) on success, or an error if saving failed
    async fn save(&self, creds: &Credentials) -> Result<(), CredentialsError>;

    /// Clear all stored credentials.
    ///
    /// # Returns
    /// Ok(()) on success, or an error if clearing failed
    async fn clear(&self) -> Result<(), CredentialsError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credentials_error_display() {
        assert_eq!(
            CredentialsError::LoadFailed("read error".to_string()).to_string(),
            "Failed to load credentials: read error"
        );
        assert_eq!(
            CredentialsError::SaveFailed("write error".to_string()).to_string(),
            "Failed to save credentials: write error"
        );
        assert_eq!(
            CredentialsError::ClearFailed("delete error".to_string()).to_string(),
            "Failed to clear credentials: delete error"
        );
        assert_eq!(
            CredentialsError::NotFound.to_string(),
            "Credentials not found"
        );
        assert_eq!(
            CredentialsError::Io("disk full".to_string()).to_string(),
            "IO error: disk full"
        );
        assert_eq!(
            CredentialsError::Serialization("invalid json".to_string()).to_string(),
            "Serialization error: invalid json"
        );
        assert_eq!(
            CredentialsError::Other("unknown".to_string()).to_string(),
            "Credentials error: unknown"
        );
    }

    #[test]
    fn test_credentials_error_clone() {
        let err = CredentialsError::LoadFailed("test".to_string());
        let cloned = err.clone();
        assert_eq!(err.to_string(), cloned.to_string());
    }

    #[test]
    fn test_credentials_error_implements_error_trait() {
        let err = CredentialsError::NotFound;
        let _: &dyn std::error::Error = &err;
    }
}
