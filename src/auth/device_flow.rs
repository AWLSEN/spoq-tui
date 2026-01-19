//! Device authorization flow state machine (RFC 8628).
//!
//! This module implements the OAuth 2.0 Device Authorization Grant flow,
//! which allows devices with limited input capabilities to obtain user authorization.

use std::sync::Arc;
use std::time::{Duration, Instant};

use super::central_api::{CentralApiClient, CentralApiError};

/// State of the device authorization flow.
#[derive(Debug)]
pub enum DeviceFlowState {
    /// Flow has not started yet.
    NotStarted,
    /// Waiting for user to authorize at verification URI.
    WaitingForUser {
        verification_uri: String,
        /// User code to display - may be None if embedded in verification_uri
        user_code: Option<String>,
        device_code: String,
        expires_at: Instant,
        interval: Duration,
        last_poll: Option<Instant>,
    },
    /// User has authorized, tokens received.
    Authorized {
        access_token: String,
        refresh_token: String,
        expires_in: i64,
    },
    /// User denied authorization.
    Denied,
    /// Device code expired before user authorized.
    Expired,
    /// An error occurred during the flow.
    Error(String),
}

/// Manager for the device authorization flow.
///
/// Handles the state machine for RFC 8628 device authorization,
/// including starting the flow, polling for tokens, and tracking state.
pub struct DeviceFlowManager {
    client: Arc<CentralApiClient>,
    state: DeviceFlowState,
}

impl DeviceFlowManager {
    /// Create a new DeviceFlowManager with the given API client.
    pub fn new(client: Arc<CentralApiClient>) -> Self {
        Self {
            client,
            state: DeviceFlowState::NotStarted,
        }
    }

    /// Get a reference to the current flow state.
    pub fn state(&self) -> &DeviceFlowState {
        &self.state
    }

    /// Start the device authorization flow.
    ///
    /// Calls the device authorization endpoint and transitions to WaitingForUser state.
    /// Returns an error if the flow has already started or if the API call fails.
    pub async fn start(&mut self) -> Result<(), CentralApiError> {
        // Only allow starting from NotStarted or Error states
        match &self.state {
            DeviceFlowState::NotStarted | DeviceFlowState::Error(_) => {}
            _ => {
                return Err(CentralApiError::ServerError {
                    status: 0,
                    message: "Device flow already started".to_string(),
                });
            }
        }

        match self.client.request_device_code().await {
            Ok(response) => {
                self.state = DeviceFlowState::WaitingForUser {
                    verification_uri: response.verification_uri,
                    user_code: response.user_code,
                    device_code: response.device_code,
                    expires_at: Instant::now() + Duration::from_secs(response.expires_in as u64),
                    interval: Duration::from_secs(response.interval as u64),
                    last_poll: None,
                };
                Ok(())
            }
            Err(e) => {
                self.state = DeviceFlowState::Error(e.to_string());
                Err(e)
            }
        }
    }

    /// Poll for the device token.
    ///
    /// Respects the polling interval from the server. If called too soon,
    /// returns Ok(false) without making an API call. Returns Ok(true) when
    /// the state has changed (authorized, denied, or expired).
    pub async fn poll(&mut self) -> Result<bool, CentralApiError> {
        let (device_code, interval, expires_at, last_poll) = match &self.state {
            DeviceFlowState::WaitingForUser {
                device_code,
                interval,
                expires_at,
                last_poll,
                ..
            } => (
                device_code.clone(),
                *interval,
                *expires_at,
                *last_poll,
            ),
            DeviceFlowState::Authorized { .. } => return Ok(true),
            DeviceFlowState::Denied => return Ok(true),
            DeviceFlowState::Expired => return Ok(true),
            _ => {
                return Err(CentralApiError::ServerError {
                    status: 0,
                    message: "Device flow not in WaitingForUser state".to_string(),
                });
            }
        };

        let now = Instant::now();

        // Check if device code has expired
        if now >= expires_at {
            self.state = DeviceFlowState::Expired;
            return Ok(true);
        }

        // Respect polling interval
        if let Some(last) = last_poll {
            if now.duration_since(last) < interval {
                return Ok(false);
            }
        }

        // Update last_poll time before making the request
        if let DeviceFlowState::WaitingForUser { last_poll, .. } = &mut self.state {
            *last_poll = Some(now);
        }

        // Poll for the token
        match self.client.poll_device_token(&device_code).await {
            Ok(response) => {
                self.state = DeviceFlowState::Authorized {
                    access_token: response.access_token,
                    refresh_token: response.refresh_token,
                    expires_in: response.expires_in as i64,
                };
                Ok(true)
            }
            Err(CentralApiError::AuthorizationPending) => {
                // Stay in WaitingForUser state, already updated last_poll
                Ok(false)
            }
            Err(CentralApiError::AccessDenied) => {
                self.state = DeviceFlowState::Denied;
                Ok(true)
            }
            Err(CentralApiError::AuthorizationExpired) => {
                self.state = DeviceFlowState::Expired;
                Ok(true)
            }
            Err(e) => {
                self.state = DeviceFlowState::Error(e.to_string());
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_client() -> Arc<CentralApiClient> {
        // Use an invalid server for unit tests
        Arc::new(CentralApiClient::with_base_url("http://127.0.0.1:1".to_string()))
    }

    #[test]
    fn test_device_flow_manager_new() {
        let client = create_test_client();
        let manager = DeviceFlowManager::new(client);

        assert!(matches!(manager.state(), DeviceFlowState::NotStarted));
    }

    #[test]
    fn test_device_flow_state_debug() {
        let state = DeviceFlowState::NotStarted;
        let debug_str = format!("{:?}", state);
        assert!(debug_str.contains("NotStarted"));

        let state = DeviceFlowState::Denied;
        let debug_str = format!("{:?}", state);
        assert!(debug_str.contains("Denied"));

        let state = DeviceFlowState::Expired;
        let debug_str = format!("{:?}", state);
        assert!(debug_str.contains("Expired"));

        let state = DeviceFlowState::Error("test error".to_string());
        let debug_str = format!("{:?}", state);
        assert!(debug_str.contains("Error"));
        assert!(debug_str.contains("test error"));
    }

    #[test]
    fn test_device_flow_state_waiting_for_user() {
        let state = DeviceFlowState::WaitingForUser {
            verification_uri: "https://example.com/verify".to_string(),
            user_code: Some("ABCD-1234".to_string()),
            device_code: "device-code-123".to_string(),
            expires_at: Instant::now() + Duration::from_secs(900),
            interval: Duration::from_secs(5),
            last_poll: None,
        };

        let debug_str = format!("{:?}", state);
        assert!(debug_str.contains("WaitingForUser"));
        assert!(debug_str.contains("ABCD-1234"));
    }

    #[test]
    fn test_device_flow_state_authorized() {
        let state = DeviceFlowState::Authorized {
            access_token: "access-token-123".to_string(),
            refresh_token: "refresh-token-456".to_string(),
            expires_in: 3600,
        };

        let debug_str = format!("{:?}", state);
        assert!(debug_str.contains("Authorized"));
    }

    #[tokio::test]
    async fn test_device_flow_start_fails_with_invalid_server() {
        let client = create_test_client();
        let mut manager = DeviceFlowManager::new(client);

        let result = manager.start().await;
        assert!(result.is_err());

        // State should be Error after failed start
        assert!(matches!(manager.state(), DeviceFlowState::Error(_)));
    }

    #[tokio::test]
    async fn test_device_flow_poll_not_started() {
        let client = create_test_client();
        let mut manager = DeviceFlowManager::new(client);

        let result = manager.poll().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_device_flow_can_restart_after_error() {
        let client = create_test_client();
        let mut manager = DeviceFlowManager::new(client);

        // First start fails
        let _ = manager.start().await;
        assert!(matches!(manager.state(), DeviceFlowState::Error(_)));

        // Can try to start again from error state (will fail again due to invalid server)
        let result = manager.start().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_device_flow_poll_returns_true_for_terminal_states() {
        let client = create_test_client();

        // Test Authorized state
        let mut manager = DeviceFlowManager::new(Arc::clone(&client));
        manager.state = DeviceFlowState::Authorized {
            access_token: "token".to_string(),
            refresh_token: "refresh".to_string(),
            expires_in: 3600,
        };
        let result = manager.poll().await;
        assert!(result.is_ok());
        assert!(result.unwrap());

        // Test Denied state
        let mut manager = DeviceFlowManager::new(Arc::clone(&client));
        manager.state = DeviceFlowState::Denied;
        let result = manager.poll().await;
        assert!(result.is_ok());
        assert!(result.unwrap());

        // Test Expired state
        let mut manager = DeviceFlowManager::new(Arc::clone(&client));
        manager.state = DeviceFlowState::Expired;
        let result = manager.poll().await;
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn test_device_flow_poll_respects_interval() {
        let client = create_test_client();
        let mut manager = DeviceFlowManager::new(client);

        // Manually set state to WaitingForUser with a recent poll
        manager.state = DeviceFlowState::WaitingForUser {
            verification_uri: "https://example.com/verify".to_string(),
            user_code: Some("ABCD-1234".to_string()),
            device_code: "device-code-123".to_string(),
            expires_at: Instant::now() + Duration::from_secs(900),
            interval: Duration::from_secs(60), // Long interval
            last_poll: Some(Instant::now()),   // Just polled
        };

        // Poll should return Ok(false) because interval hasn't elapsed
        let result = manager.poll().await;
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[tokio::test]
    async fn test_device_flow_poll_detects_expiry() {
        let client = create_test_client();
        let mut manager = DeviceFlowManager::new(client);

        // Set state to WaitingForUser but already expired
        manager.state = DeviceFlowState::WaitingForUser {
            verification_uri: "https://example.com/verify".to_string(),
            user_code: Some("ABCD-1234".to_string()),
            device_code: "device-code-123".to_string(),
            expires_at: Instant::now() - Duration::from_secs(1), // Already expired
            interval: Duration::from_secs(5),
            last_poll: None,
        };

        // Poll should detect expiry and transition to Expired state
        let result = manager.poll().await;
        assert!(result.is_ok());
        assert!(result.unwrap()); // State changed

        assert!(matches!(manager.state(), DeviceFlowState::Expired));
    }
}
