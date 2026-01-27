//! Device Authorization Flow (RFC 8628) for Spoq CLI.
//!
//! This module implements the OAuth 2.0 Device Authorization Grant flow,
//! which allows CLI applications to authenticate users via a browser.
//!
//! Flow:
//! 1. CLI requests device code from server
//! 2. User visits verification URL and enters code
//! 3. CLI polls for token until authorized or timeout

use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;

use super::central_api::{
    get_jwt_expires_in, CentralApiClient, CentralApiError, DeviceCodeResponse, TokenResponse,
};
use super::credentials::{Credentials, CredentialsManager};

/// Error type for device flow operations.
#[derive(Debug)]
pub enum DeviceFlowError {
    /// Central API error
    Api(CentralApiError),
    /// Failed to initialize credentials manager
    CredentialsManager(String),
    /// Failed to save credentials
    SaveCredentials(String),
    /// User cancelled the flow
    Cancelled,
}

impl std::fmt::Display for DeviceFlowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeviceFlowError::Api(e) => write!(f, "API error: {}", e),
            DeviceFlowError::CredentialsManager(msg) => {
                write!(f, "Credentials manager error: {}", msg)
            }
            DeviceFlowError::SaveCredentials(msg) => {
                write!(f, "Failed to save credentials: {}", msg)
            }
            DeviceFlowError::Cancelled => write!(f, "Authentication cancelled by user"),
        }
    }
}

impl std::error::Error for DeviceFlowError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DeviceFlowError::Api(e) => Some(e),
            _ => None,
        }
    }
}

impl From<CentralApiError> for DeviceFlowError {
    fn from(e: CentralApiError) -> Self {
        DeviceFlowError::Api(e)
    }
}

/// Set up Ctrl+C handler that sets the interrupted flag.
/// Returns the Arc<AtomicBool> that will be set to true on interrupt.
fn setup_interrupt_handler() -> Arc<AtomicBool> {
    let interrupted = Arc::new(AtomicBool::new(false));
    let interrupted_clone = Arc::clone(&interrupted);

    // Install the handler - ignore errors if already set
    let _ = ctrlc::set_handler(move || {
        interrupted_clone.store(true, Ordering::SeqCst);
    });

    interrupted
}

/// Run the device authorization flow.
///
/// This function:
/// 1. Requests a device code from the server
/// 2. Displays the verification URL and code to the user
/// 3. Opens the browser automatically
/// 4. Polls for authorization until the user completes the flow
/// 5. Saves the credentials to disk
///
/// # Arguments
/// * `runtime` - Tokio runtime for async operations
///
/// # Returns
/// * `Ok(Credentials)` - Successfully authenticated credentials
/// * `Err(DeviceFlowError)` - Authentication failed
pub fn run_device_flow(runtime: &tokio::runtime::Runtime) -> Result<Credentials, DeviceFlowError> {
    println!("\nAuthentication required\n");
    println!("Press Ctrl+C to cancel.\n");

    // Set up interrupt handler
    let interrupted = setup_interrupt_handler();

    let client = CentralApiClient::new();

    // Request device code (blocking via runtime)
    print!("Requesting authorization... ");
    io::stdout().flush().ok();
    let device_code: DeviceCodeResponse = runtime.block_on(client.request_device_code())?;
    println!("done\n");

    // Show verification info - handle optional user_code
    let user_code_display = device_code.user_code.as_deref().unwrap_or("(see URL)");
    println!("Open this URL in your browser:");
    println!("  {}", device_code.verification_uri);
    println!("  Code: {}\n", user_code_display);

    // Try to open browser automatically
    if open::that(&device_code.verification_uri).is_ok() {
        println!("Browser opened automatically.\n");
    }

    // Poll for authorization (blocking, respects server interval)
    print!("Waiting for authorization... ");
    io::stdout().flush().ok();

    let tokens = poll_for_authorization(runtime, &client, &device_code, &interrupted)?;
    println!("done\n");

    // Save username for display before moving token fields
    let username_display = tokens.username.as_deref().unwrap_or("user").to_string();

    // Build credentials from token response
    let credentials = build_credentials_from_tokens(tokens);

    // Persist credentials
    let manager = CredentialsManager::new().ok_or_else(|| {
        DeviceFlowError::CredentialsManager("Failed to initialize credentials manager".to_string())
    })?;

    if !manager.save(&credentials) {
        return Err(DeviceFlowError::SaveCredentials(
            "Failed to save credentials to disk".to_string(),
        ));
    }

    println!("Signed in as {}\n", username_display);

    Ok(credentials)
}

/// Poll the server for authorization completion.
///
/// This function respects the server-specified polling interval and
/// handles the various response states (pending, denied, expired).
fn poll_for_authorization(
    runtime: &tokio::runtime::Runtime,
    client: &CentralApiClient,
    device_code: &DeviceCodeResponse,
    interrupted: &Arc<AtomicBool>,
) -> Result<TokenResponse, DeviceFlowError> {
    let interval = Duration::from_secs(device_code.interval.max(1) as u64);
    let deadline = Instant::now() + Duration::from_secs(device_code.expires_in as u64);

    while Instant::now() < deadline {
        // Interruptible sleep: check every 100ms for Ctrl+C
        if interruptible_sleep(interval, interrupted) {
            println!("\nAuthentication cancelled.");
            return Err(DeviceFlowError::Cancelled);
        }

        match runtime.block_on(client.poll_device_token(&device_code.device_code)) {
            Ok(tokens) => return Ok(tokens),
            Err(CentralApiError::AuthorizationPending) => {
                // User hasn't authorized yet, keep polling
                continue;
            }
            Err(CentralApiError::AccessDenied) => {
                println!("Denied");
                return Err(DeviceFlowError::Api(CentralApiError::AccessDenied));
            }
            Err(CentralApiError::AuthorizationExpired) => {
                println!("Expired");
                return Err(DeviceFlowError::Api(CentralApiError::AuthorizationExpired));
            }
            Err(e) => return Err(DeviceFlowError::Api(e)),
        }
    }

    println!("Timed out");
    Err(DeviceFlowError::Api(CentralApiError::AuthorizationExpired))
}

/// Sleep for the given duration, but check for interrupts every 100ms.
/// Returns `true` if interrupted, `false` if sleep completed normally.
fn interruptible_sleep(duration: Duration, interrupted: &Arc<AtomicBool>) -> bool {
    const CHECK_INTERVAL: Duration = Duration::from_millis(100);
    let start = Instant::now();

    while start.elapsed() < duration {
        if interrupted.load(Ordering::SeqCst) {
            return true;
        }

        // Sleep for at most CHECK_INTERVAL or the remaining time
        let remaining = duration.saturating_sub(start.elapsed());
        let sleep_time = remaining.min(CHECK_INTERVAL);

        if sleep_time.is_zero() {
            break;
        }

        std::thread::sleep(sleep_time);
    }

    // Final check after sleep completes
    interrupted.load(Ordering::SeqCst)
}

/// Build Credentials from a TokenResponse.
///
/// Calculates expiration time from the JWT or response, and extracts
/// all relevant fields into the Credentials struct.
fn build_credentials_from_tokens(tokens: TokenResponse) -> Credentials {
    // Calculate expiration from JWT or response
    // Token lifetime: Actual lifetime comes from API response, this is just a fallback default (15 min)
    let expires_in = tokens
        .expires_in
        .or_else(|| get_jwt_expires_in(&tokens.access_token))
        .unwrap_or(900); // Default 15 minutes
    let expires_at = Utc::now().timestamp() + expires_in as i64;

    Credentials {
        access_token: Some(tokens.access_token),
        refresh_token: tokens.refresh_token,
        expires_at: Some(expires_at),
        user_id: tokens.user_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_setup_interrupt_handler() {
        // Should return an AtomicBool that starts as false
        let interrupted = setup_interrupt_handler();
        assert!(!interrupted.load(Ordering::SeqCst));
    }

    #[test]
    fn test_interrupt_flag_can_be_set() {
        let interrupted = Arc::new(AtomicBool::new(false));
        assert!(!interrupted.load(Ordering::SeqCst));

        interrupted.store(true, Ordering::SeqCst);
        assert!(interrupted.load(Ordering::SeqCst));
    }

    #[test]
    fn test_build_credentials_from_tokens() {
        let tokens = TokenResponse {
            access_token: "access-123".to_string(),
            refresh_token: Some("refresh-456".to_string()),
            token_type: Some("Bearer".to_string()),
            expires_in: Some(3600),
            user_id: Some("user-789".to_string()),
            username: Some("testuser".to_string()),
        };

        let creds = build_credentials_from_tokens(tokens);

        assert_eq!(creds.access_token, Some("access-123".to_string()));
        assert_eq!(creds.refresh_token, Some("refresh-456".to_string()));
        assert!(creds.expires_at.is_some());
        assert_eq!(creds.user_id, Some("user-789".to_string()));
    }

    #[test]
    fn test_build_credentials_from_tokens_without_expires_in() {
        let tokens = TokenResponse {
            access_token: "test-token".to_string(),
            refresh_token: Some("test-refresh".to_string()),
            token_type: Some("Bearer".to_string()),
            expires_in: None,
            user_id: None,
            username: None,
        };

        let creds = build_credentials_from_tokens(tokens);

        // Should fall back to 900 seconds (15 minutes)
        assert!(creds.expires_at.is_some());
        let now = Utc::now().timestamp();
        let expires_at = creds.expires_at.unwrap();
        // Should be roughly 15 minutes from now (allow 10 second tolerance)
        assert!(expires_at >= now + 890 && expires_at <= now + 910);
    }

    #[test]
    fn test_device_flow_error_display() {
        let err = DeviceFlowError::Cancelled;
        assert_eq!(format!("{}", err), "Authentication cancelled by user");

        let err = DeviceFlowError::CredentialsManager("test error".to_string());
        assert!(format!("{}", err).contains("test error"));

        let err = DeviceFlowError::SaveCredentials("save failed".to_string());
        assert!(format!("{}", err).contains("save failed"));
    }

    #[test]
    fn test_device_flow_error_from_central_api_error() {
        let api_err = CentralApiError::AccessDenied;
        let flow_err: DeviceFlowError = api_err.into();

        match flow_err {
            DeviceFlowError::Api(CentralApiError::AccessDenied) => {}
            _ => panic!("Expected DeviceFlowError::Api(AccessDenied)"),
        }
    }

    #[test]
    fn test_interruptible_sleep_completes_normally() {
        let interrupted = Arc::new(AtomicBool::new(false));
        let start = Instant::now();

        let was_interrupted = interruptible_sleep(Duration::from_millis(150), &interrupted);

        assert!(!was_interrupted);
        assert!(start.elapsed() >= Duration::from_millis(150));
    }

    #[test]
    fn test_interruptible_sleep_detects_interrupt() {
        let interrupted = Arc::new(AtomicBool::new(false));
        let interrupted_clone = Arc::clone(&interrupted);

        // Set interrupt after 50ms in another thread
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            interrupted_clone.store(true, Ordering::SeqCst);
        });

        let start = Instant::now();
        let was_interrupted = interruptible_sleep(Duration::from_secs(5), &interrupted);

        assert!(was_interrupted);
        // Should return much faster than 5 seconds (within ~200ms accounting for check interval)
        assert!(start.elapsed() < Duration::from_millis(500));
    }

    #[test]
    fn test_interruptible_sleep_pre_interrupted() {
        let interrupted = Arc::new(AtomicBool::new(true)); // Already interrupted
        let start = Instant::now();

        let was_interrupted = interruptible_sleep(Duration::from_secs(5), &interrupted);

        assert!(was_interrupted);
        // Should return almost immediately
        assert!(start.elapsed() < Duration::from_millis(50));
    }
}
