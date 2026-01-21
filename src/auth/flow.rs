//! Authentication flow module - runs before TUI starts.
//!
//! This module provides a blocking authentication flow using terminal prompts.
//! It uses the Tokio runtime to call existing async methods on CentralApiClient.

use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;

use super::central_api::{
    get_jwt_expires_in, CentralApiClient, CentralApiError, DeviceCodeResponse, TokenResponse,
};
use super::credentials::{Credentials, CredentialsManager};

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

/// Run the interactive authentication flow in the terminal (not TUI).
/// Returns authenticated credentials on success.
pub fn run_auth_flow(runtime: &tokio::runtime::Runtime) -> Result<Credentials, CentralApiError> {
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

    // Calculate expiration from JWT or response
    // Token lifetime: Actual lifetime comes from API response, this is just a fallback default (15 min)
    let expires_in = tokens
        .expires_in
        .or_else(|| get_jwt_expires_in(&tokens.access_token))
        .unwrap_or(900); // Default 15 minutes
    let expires_at = Utc::now().timestamp() + expires_in as i64;

    // Build credentials from token response
    let mut credentials = Credentials::default();
    credentials.access_token = Some(tokens.access_token);
    credentials.refresh_token = tokens.refresh_token; // Already Option<String>
    credentials.expires_at = Some(expires_at);
    credentials.user_id = tokens.user_id;
    credentials.username = tokens.username;

    // Persist credentials
    let manager = CredentialsManager::new().ok_or_else(|| CentralApiError::ServerError {
        status: 0,
        message: "Failed to initialize credentials manager".to_string(),
    })?;
    if !manager.save(&credentials) {
        return Err(CentralApiError::ServerError {
            status: 0,
            message: "Failed to save credentials to disk".to_string(),
        });
    }

    println!(
        "Signed in as {}\n",
        credentials.username.as_deref().unwrap_or("user")
    );

    Ok(credentials)
}

fn poll_for_authorization(
    runtime: &tokio::runtime::Runtime,
    client: &CentralApiClient,
    device_code: &DeviceCodeResponse,
    interrupted: &Arc<AtomicBool>,
) -> Result<TokenResponse, CentralApiError> {
    let interval = Duration::from_secs(device_code.interval.max(1) as u64);
    let deadline = Instant::now() + Duration::from_secs(device_code.expires_in as u64);

    while Instant::now() < deadline {
        // Check for interrupt before sleeping
        if interrupted.load(Ordering::SeqCst) {
            println!("\nAuthentication cancelled.");
            std::process::exit(0);
        }

        std::thread::sleep(interval);

        // Check for interrupt after sleeping
        if interrupted.load(Ordering::SeqCst) {
            println!("\nAuthentication cancelled.");
            std::process::exit(0);
        }

        match runtime.block_on(client.poll_device_token(&device_code.device_code)) {
            Ok(tokens) => return Ok(tokens),
            Err(CentralApiError::AuthorizationPending) => {
                // User hasn't authorized yet, keep polling
                continue;
            }
            Err(CentralApiError::AccessDenied) => {
                println!("Denied");
                return Err(CentralApiError::AccessDenied);
            }
            Err(CentralApiError::AuthorizationExpired) => {
                println!("Expired");
                return Err(CentralApiError::AuthorizationExpired);
            }
            Err(e) => return Err(e),
        }
    }

    println!("Timed out");
    Err(CentralApiError::AuthorizationExpired)
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
    fn test_device_code_response_optional_user_code() {
        // Test that user_code being None is handled
        let device_code = DeviceCodeResponse {
            device_code: "test-device-code".to_string(),
            user_code: None,
            verification_uri: "https://example.com/verify".to_string(),
            expires_in: 600,
            interval: 5,
        };

        let display = device_code.user_code.as_deref().unwrap_or("(see URL)");
        assert_eq!(display, "(see URL)");

        // Test with Some value
        let device_code_with_code = DeviceCodeResponse {
            device_code: "test-device-code".to_string(),
            user_code: Some("ABC-123".to_string()),
            verification_uri: "https://example.com/verify".to_string(),
            expires_in: 600,
            interval: 5,
        };

        let display_with_code = device_code_with_code
            .user_code
            .as_deref()
            .unwrap_or("(see URL)");
        assert_eq!(display_with_code, "ABC-123");
    }

    #[test]
    fn test_interval_calculation() {
        let device_code = DeviceCodeResponse {
            device_code: "test".to_string(),
            user_code: None,
            verification_uri: "https://example.com".to_string(),
            expires_in: 600,
            interval: 5,
        };

        // Interval should be at least 1 second
        let interval = Duration::from_secs(device_code.interval.max(1) as u64);
        assert_eq!(interval, Duration::from_secs(5));

        // Test with interval of 0 (should become 1)
        let device_code_zero = DeviceCodeResponse {
            device_code: "test".to_string(),
            user_code: None,
            verification_uri: "https://example.com".to_string(),
            expires_in: 600,
            interval: 0,
        };

        let interval_zero = Duration::from_secs(device_code_zero.interval.max(1) as u64);
        assert_eq!(interval_zero, Duration::from_secs(1));
    }

    #[test]
    fn test_token_response_expires_in_fallback() {
        // Test that expires_in falls back to JWT parsing or default
        let token_response = TokenResponse {
            access_token: "test-token".to_string(),
            refresh_token: "test-refresh".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: None,
            user_id: None,
            username: None,
        };

        // With None expires_in and invalid JWT, should fall back to 900 (15 min)
        let expires_in = token_response
            .expires_in
            .or_else(|| get_jwt_expires_in(&token_response.access_token))
            .unwrap_or(900);
        assert_eq!(expires_in, 900);

        // With Some expires_in, should use that value
        let token_with_expires = TokenResponse {
            access_token: "test-token".to_string(),
            refresh_token: "test-refresh".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: Some(7200),
            user_id: None,
            username: None,
        };

        let expires_in_explicit = token_with_expires
            .expires_in
            .or_else(|| get_jwt_expires_in(&token_with_expires.access_token))
            .unwrap_or(900);
        assert_eq!(expires_in_explicit, 7200);
    }

    #[test]
    fn test_credentials_build_from_token_response() {
        let tokens = TokenResponse {
            access_token: "access-123".to_string(),
            refresh_token: "refresh-456".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: Some(3600),
            user_id: Some("user-789".to_string()),
            username: Some("testuser".to_string()),
        };

        let mut credentials = Credentials::default();
        credentials.access_token = Some(tokens.access_token.clone());
        credentials.refresh_token = Some(tokens.refresh_token.clone());
        credentials.user_id = tokens.user_id.clone();
        credentials.username = tokens.username.clone();

        assert_eq!(credentials.access_token, Some("access-123".to_string()));
        assert_eq!(credentials.refresh_token, Some("refresh-456".to_string()));
        assert_eq!(credentials.user_id, Some("user-789".to_string()));
        assert_eq!(credentials.username, Some("testuser".to_string()));
    }
}
