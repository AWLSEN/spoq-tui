//! Authentication flow module - runs before TUI starts.
//!
//! This module provides a blocking authentication flow using terminal prompts.
//! It uses the Tokio runtime to call existing async methods on CentralApiClient.

use std::io::{self, Write};
use std::time::{Duration, Instant};

use chrono::Utc;

use super::central_api::{
    get_jwt_expires_in, CentralApiClient, CentralApiError, DeviceCodeResponse, TokenResponse,
};
use super::credentials::{Credentials, CredentialsManager};

/// Run the interactive authentication flow in the terminal (not TUI).
/// Returns authenticated credentials on success.
pub fn run_auth_flow(runtime: &tokio::runtime::Runtime) -> Result<Credentials, CentralApiError> {
    println!("\nAuthentication required\n");

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

    let tokens = poll_for_authorization(runtime, &client, &device_code)?;
    println!("done\n");

    // Calculate expiration from JWT or response
    let expires_in = tokens
        .expires_in
        .or_else(|| get_jwt_expires_in(&tokens.access_token))
        .unwrap_or(3600); // Default 1 hour
    let expires_at = Utc::now().timestamp() + expires_in as i64;

    // Build credentials from token response
    let mut credentials = Credentials::default();
    credentials.access_token = Some(tokens.access_token);
    credentials.refresh_token = Some(tokens.refresh_token);
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
) -> Result<TokenResponse, CentralApiError> {
    let interval = Duration::from_secs(device_code.interval.max(1) as u64);
    let deadline = Instant::now() + Duration::from_secs(device_code.expires_in as u64);

    while Instant::now() < deadline {
        std::thread::sleep(interval);

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
