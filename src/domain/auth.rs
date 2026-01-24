//! Authentication state management.
//!
//! This module provides [`AuthenticationState`], a domain object that encapsulates
//! all authentication-related state and operations, including token storage,
//! refresh logic, and client management.

use crate::auth::{
    central_api::{get_jwt_expires_in, CentralApiClient, CentralApiError},
    Credentials, CredentialsManager,
};
use crate::conductor::ConductorClient;
use chrono::Utc;
use std::sync::Arc;

/// Errors that can occur during authentication operations.
#[derive(Debug, Clone)]
pub enum AuthError {
    /// No credentials are stored.
    NoCredentials,
    /// No refresh token is available.
    NoRefreshToken,
    /// Central API client is not configured.
    NoCentralApi,
    /// Token refresh failed.
    RefreshFailed,
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::NoCredentials => write!(f, "No credentials stored"),
            AuthError::NoRefreshToken => write!(f, "No refresh token available"),
            AuthError::NoCentralApi => write!(f, "Central API client not configured"),
            AuthError::RefreshFailed => write!(f, "Token refresh failed"),
        }
    }
}

impl std::error::Error for AuthError {}

/// Authentication state encapsulating tokens, clients, and refresh logic.
///
/// This domain object manages all authentication-related concerns:
/// - Access and refresh tokens
/// - Token expiration tracking
/// - Central API client for authentication endpoints
/// - Credentials manager for persistent storage
/// - Conductor client with authentication
pub struct AuthenticationState {
    /// Current authentication credentials
    pub credentials: Credentials,
    /// Central API client for authenticated requests
    pub central_api: Option<Arc<CentralApiClient>>,
    /// Credentials manager for secure storage
    pub credentials_manager: Option<CredentialsManager>,
    /// VPS URL for the current session (fetched from API at startup)
    pub vps_url: Option<String>,
}

impl Default for AuthenticationState {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthenticationState {
    /// Create a new AuthenticationState with default values.
    pub fn new() -> Self {
        Self {
            credentials: Credentials::default(),
            central_api: None,
            credentials_manager: None,
            vps_url: None,
        }
    }

    /// Create a new AuthenticationState from loaded credentials.
    ///
    /// This initializes the credentials manager and central API client.
    pub fn from_credentials(credentials: Credentials, vps_url: Option<String>) -> Self {
        let credentials_manager = CredentialsManager::new();

        let central_api = if let Some(ref token) = credentials.access_token {
            Some(Arc::new(CentralApiClient::new().with_auth(token)))
        } else {
            Some(Arc::new(CentralApiClient::new()))
        };

        Self {
            credentials,
            central_api,
            credentials_manager,
            vps_url,
        }
    }

    /// Load authentication state from disk.
    pub fn load() -> Self {
        let credentials_manager = CredentialsManager::new();
        let credentials = credentials_manager
            .as_ref()
            .map(|cm| cm.load())
            .unwrap_or_default();

        let central_api = if let Some(ref token) = credentials.access_token {
            Some(Arc::new(CentralApiClient::new().with_auth(token)))
        } else {
            Some(Arc::new(CentralApiClient::new()))
        };

        Self {
            credentials,
            central_api,
            credentials_manager,
            vps_url: None,
        }
    }

    /// Check if the user has valid credentials.
    pub fn has_valid_credentials(&self) -> bool {
        self.credentials.access_token.is_some() && !self.credentials.is_expired()
    }

    /// Ensure we have a valid access token, refreshing if necessary.
    ///
    /// Returns the access token if valid, or attempts to refresh it.
    /// If refresh fails, returns an error.
    pub async fn ensure_valid_token(&mut self) -> Result<String, AuthError> {
        // Check if we have an access token
        let Some(ref token) = self.credentials.access_token else {
            return Err(AuthError::NoCredentials);
        };

        // Check expiration (with 5-minute buffer)
        if let Some(expires_at) = self.credentials.expires_at {
            let now = Utc::now().timestamp();
            let buffer = 5 * 60; // 5 minutes

            if now + buffer >= expires_at {
                // Token expired or about to expire, refresh it
                return self.refresh_token().await;
            }
        }

        Ok(token.clone())
    }

    /// Refresh the access token using the refresh token.
    async fn refresh_token(&mut self) -> Result<String, AuthError> {
        let refresh_token = self
            .credentials
            .refresh_token
            .clone()
            .ok_or(AuthError::NoRefreshToken)?;

        let central_api = self.central_api.as_ref().ok_or(AuthError::NoCentralApi)?;

        match central_api.refresh_token(&refresh_token).await {
            Ok(response) => {
                // Update credentials
                self.credentials.access_token = Some(response.access_token.clone());
                let expires_in = response
                    .expires_in
                    .or_else(|| get_jwt_expires_in(&response.access_token))
                    .unwrap_or(900);
                self.credentials.expires_at = Some(Utc::now().timestamp() + i64::from(expires_in));

                // Save to disk
                if let Some(ref manager) = self.credentials_manager {
                    let _ = manager.save(&self.credentials);
                }

                Ok(response.access_token)
            }
            Err(_) => Err(AuthError::RefreshFailed),
        }
    }

    /// Refresh the access token and update all clients with the new token.
    ///
    /// This method:
    /// 1. Gets the refresh token from credentials
    /// 2. Calls the Central API to refresh the access token
    /// 3. Updates credentials with the new token and expiration
    /// 4. Recreates CentralApiClient with the new token
    /// 5. Saves credentials to disk
    ///
    /// Returns the new ConductorClient if VPS URL is set, for the caller to update.
    pub async fn refresh_and_update_clients(
        &mut self,
    ) -> Result<Option<Arc<ConductorClient>>, CentralApiError> {
        let refresh_token = self.credentials.refresh_token.as_ref().ok_or_else(|| {
            CentralApiError::ServerError {
                status: 401,
                message: "No refresh token available".to_string(),
            }
        })?;

        // Create a temporary client for refresh (no auth needed for refresh endpoint)
        let api = CentralApiClient::new();
        let token_response = api.refresh_token(refresh_token).await?;

        // Update credentials with new token
        let now = Utc::now().timestamp();
        let expires_in = token_response
            .expires_in
            .or_else(|| get_jwt_expires_in(&token_response.access_token))
            .unwrap_or(900);
        self.credentials.access_token = Some(token_response.access_token.clone());
        self.credentials.expires_at = Some(now + i64::from(expires_in));

        // Update refresh token if server provides a new one
        if let Some(new_refresh_token) = token_response.refresh_token {
            if !new_refresh_token.is_empty() {
                self.credentials.refresh_token = Some(new_refresh_token);
            }
        }

        // Recreate CentralApiClient with new token
        self.central_api = Some(Arc::new(
            CentralApiClient::new().with_auth(&token_response.access_token),
        ));

        // Create new ConductorClient if VPS URL is set
        let new_client = if let Some(ref vps_url) = self.vps_url {
            Some(Arc::new(
                ConductorClient::with_url(vps_url).with_auth(&token_response.access_token),
            ))
        } else {
            None
        };

        // Save refreshed credentials to disk
        if let Some(ref manager) = self.credentials_manager {
            let _ = manager.save(&self.credentials);
        }

        Ok(new_client)
    }

    /// Execute an API call with automatic token refresh on 401.
    ///
    /// Only retries once to prevent infinite loops.
    /// Returns the new ConductorClient if token was refreshed and VPS URL is set.
    pub async fn with_auto_refresh<T, F, Fut>(
        &mut self,
        operation: F,
    ) -> Result<(T, Option<Arc<ConductorClient>>), CentralApiError>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T, CentralApiError>>,
    {
        match operation().await {
            Ok(result) => Ok((result, None)),
            Err(CentralApiError::ServerError { status: 401, .. }) => {
                // Try to refresh token
                let new_client = self.refresh_and_update_clients().await?;
                // Retry once with new token
                let result = operation().await?;
                Ok((result, new_client))
            }
            Err(e) => Err(e),
        }
    }

    /// Update credentials and save to disk.
    pub fn update_credentials(&mut self, credentials: Credentials) {
        self.credentials = credentials;
        if let Some(ref manager) = self.credentials_manager {
            let _ = manager.save(&self.credentials);
        }
    }

    /// Set the VPS URL.
    pub fn set_vps_url(&mut self, url: Option<String>) {
        self.vps_url = url;
    }
}

impl std::fmt::Debug for AuthenticationState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthenticationState")
            .field("credentials", &self.credentials)
            .field("has_central_api", &self.central_api.is_some())
            .field("has_credentials_manager", &self.credentials_manager.is_some())
            .field("vps_url", &self.vps_url)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_error_display() {
        assert_eq!(AuthError::NoCredentials.to_string(), "No credentials stored");
        assert_eq!(
            AuthError::NoRefreshToken.to_string(),
            "No refresh token available"
        );
        assert_eq!(
            AuthError::NoCentralApi.to_string(),
            "Central API client not configured"
        );
        assert_eq!(AuthError::RefreshFailed.to_string(), "Token refresh failed");
    }

    #[test]
    fn test_authentication_state_new() {
        let state = AuthenticationState::new();
        assert!(state.credentials.access_token.is_none());
        assert!(state.central_api.is_none());
        assert!(state.credentials_manager.is_none());
        assert!(state.vps_url.is_none());
    }

    #[test]
    fn test_authentication_state_default() {
        let state = AuthenticationState::default();
        assert!(state.credentials.access_token.is_none());
    }

    #[test]
    fn test_has_valid_credentials_no_token() {
        let state = AuthenticationState::new();
        assert!(!state.has_valid_credentials());
    }

    #[test]
    fn test_has_valid_credentials_expired() {
        let mut state = AuthenticationState::new();
        state.credentials.access_token = Some("test-token".to_string());
        state.credentials.expires_at = Some(0); // Expired
        assert!(!state.has_valid_credentials());
    }

    #[test]
    fn test_has_valid_credentials_valid() {
        let mut state = AuthenticationState::new();
        state.credentials.access_token = Some("test-token".to_string());
        state.credentials.expires_at = Some(Utc::now().timestamp() + 3600);
        assert!(state.has_valid_credentials());
    }

    #[test]
    fn test_from_credentials_with_token() {
        let mut creds = Credentials::default();
        creds.access_token = Some("test-token".to_string());

        let state = AuthenticationState::from_credentials(creds, Some("http://vps.example.com".to_string()));

        assert!(state.credentials.access_token.is_some());
        assert!(state.central_api.is_some());
        assert!(state.credentials_manager.is_some());
        assert_eq!(state.vps_url, Some("http://vps.example.com".to_string()));
    }

    #[test]
    fn test_from_credentials_without_token() {
        let creds = Credentials::default();
        let state = AuthenticationState::from_credentials(creds, None);

        assert!(state.credentials.access_token.is_none());
        assert!(state.central_api.is_some()); // Still creates unauthenticated client
        assert!(state.vps_url.is_none());
    }

    #[test]
    fn test_set_vps_url() {
        let mut state = AuthenticationState::new();
        assert!(state.vps_url.is_none());

        state.set_vps_url(Some("http://example.com".to_string()));
        assert_eq!(state.vps_url, Some("http://example.com".to_string()));

        state.set_vps_url(None);
        assert!(state.vps_url.is_none());
    }

    #[tokio::test]
    async fn test_ensure_valid_token_no_credentials() {
        let mut state = AuthenticationState::new();
        let result = state.ensure_valid_token().await;
        assert!(matches!(result, Err(AuthError::NoCredentials)));
    }

    #[tokio::test]
    async fn test_ensure_valid_token_valid() {
        let mut state = AuthenticationState::new();
        state.credentials.access_token = Some("valid-token".to_string());
        state.credentials.expires_at = Some(Utc::now().timestamp() + 3600);

        let result = state.ensure_valid_token().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "valid-token");
    }

    #[test]
    fn test_update_credentials() {
        let mut state = AuthenticationState::new();
        let mut new_creds = Credentials::default();
        new_creds.access_token = Some("new-token".to_string());

        state.update_credentials(new_creds.clone());

        assert_eq!(state.credentials.access_token, new_creds.access_token);
    }
}
