//! Main preflight orchestration module.
//!
//! This module coordinates all startup checks and returns a StartupResult
//! ready for TUI initialization.

use super::auth::validate_credentials;
use super::config::{StartupConfig, StartupResult};
use super::debug::start_debug_system;
use super::health::run_health_check_loop;
use super::vps::{build_vps_url, verify_vps, VpsError};
use crate::auth::credentials::CredentialsManager;
use crate::auth::AuthError;

/// Error type for preflight checks.
#[derive(Debug)]
pub enum PreflightError {
    /// Credentials manager initialization failed
    CredentialsManager(String),
    /// Authentication failed
    Auth(AuthError),
    /// VPS verification failed
    Vps(VpsError),
    /// Health check failed
    HealthCheck(String),
}

impl std::fmt::Display for PreflightError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PreflightError::CredentialsManager(msg) => {
                write!(f, "Credentials manager error: {}", msg)
            }
            PreflightError::Auth(e) => write!(f, "Authentication error: {}", e),
            PreflightError::Vps(e) => write!(f, "VPS error: {}", e),
            PreflightError::HealthCheck(msg) => write!(f, "Health check error: {}", msg),
        }
    }
}

impl std::error::Error for PreflightError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PreflightError::Auth(e) => Some(e),
            PreflightError::Vps(e) => Some(e),
            _ => None,
        }
    }
}

impl From<AuthError> for PreflightError {
    fn from(e: AuthError) -> Self {
        PreflightError::Auth(e)
    }
}

impl From<VpsError> for PreflightError {
    fn from(e: VpsError) -> Self {
        PreflightError::Vps(e)
    }
}

/// Run all preflight checks before starting the TUI.
///
/// This function orchestrates:
/// 1. Credential validation (SPOQ auth flow if needed)
/// 2. VPS verification (provisioning + GH auto-login if needed)
/// 3. Health check loop (with credential sync and GH auto-login retry)
/// 4. Debug system startup
///
/// # Arguments
/// * `runtime` - Tokio runtime for async operations
/// * `config` - Startup configuration
///
/// # Returns
/// * `Ok(StartupResult)` - All checks passed, ready for TUI
/// * `Err(PreflightError)` - Startup failed
pub fn run_preflight_checks(
    runtime: &tokio::runtime::Runtime,
    config: StartupConfig,
) -> Result<StartupResult, PreflightError> {
    // Dev mode: skip auth and use localhost conductor
    if config.dev_mode {
        println!("🔧 Dev mode enabled - skipping authentication");
        let dev_url = config
            .dev_conductor_url
            .clone()
            .unwrap_or_else(|| "http://localhost:8000".to_string());
        println!("   Using conductor at: {}", dev_url);

        // Create dummy credentials for dev mode
        let credentials = crate::auth::credentials::Credentials {
            access_token: Some("dev-token".to_string()),
            refresh_token: Some("dev-refresh".to_string()),
            expires_at: None,
            user_id: Some("dev-user".to_string()),
        };

        // Start debug system
        let (debug_tx, debug_handle, debug_snapshot) = if config.enable_debug {
            let debug_result = runtime.block_on(start_debug_system(config.debug_port));
            (
                debug_result.tx,
                debug_result.server_handle,
                debug_result.state_snapshot,
            )
        } else {
            (None, None, None)
        };

        let result = StartupResult::new(credentials)
            .with_vps_url(Some(dev_url))
            .with_debug(debug_tx, debug_handle, debug_snapshot);

        println!("Starting SPOQ (dev mode)...\n");
        return Ok(result);
    }

    // Initialize credentials manager
    let manager = CredentialsManager::new().ok_or_else(|| {
        PreflightError::CredentialsManager("Failed to initialize credentials manager".to_string())
    })?;

    // Step 1: Validate credentials (SPOQ auth flow if needed)
    println!("Checking authentication...");
    let mut credentials = validate_credentials(runtime, &manager)?;

    // Check if local conductor mode is configured
    let spoq_config = crate::startup::config::SpoqConfig::load();
    if spoq_config.is_local() {
        use crate::conductor::local;

        let port = local::default_port();
        let conductor_url = spoq_config
            .conductor_url
            .clone()
            .unwrap_or_else(|| format!("http://127.0.0.1:{}", port));

        println!("Local conductor mode");

        // Check if already running
        let already_running = runtime.block_on(local::is_running(port));

        if !already_running {
            // Ensure binary exists
            if !local::conductor_exists() {
                return Err(PreflightError::HealthCheck(
                    "Conductor binary not found. Run /vps to set up again.".to_string(),
                ));
            }

            println!("   Starting conductor...");
            let owner_id = credentials
                .user_id
                .clone()
                .unwrap_or_else(|| "local-user".to_string());

            let _child = runtime
                .block_on(local::start_conductor(port, &owner_id))
                .map_err(|e| PreflightError::HealthCheck(e))?;

            runtime
                .block_on(local::wait_for_health(port, 30))
                .map_err(|e| PreflightError::HealthCheck(e))?;
        }

        println!("   Conductor ready at {}", conductor_url);

        // Start debug system
        let (debug_tx, debug_handle, debug_snapshot) = if config.enable_debug {
            let debug_result = runtime.block_on(start_debug_system(config.debug_port));
            (
                debug_result.tx,
                debug_result.server_handle,
                debug_result.state_snapshot,
            )
        } else {
            (None, None, None)
        };

        let result = StartupResult::new(credentials)
            .with_vps_url(Some(conductor_url))
            .with_debug(debug_tx, debug_handle, debug_snapshot);

        println!("Starting SPOQ...\n");
        return Ok(result);
    }

    // Step 2: VPS verification (unless skipped)
    let (vps_state, vps_url) = if config.skip_vps_check {
        (None, None)
    } else {
        let vps = verify_vps(runtime, &mut credentials, &manager)?;
        let url = build_vps_url(&vps).ok_or_else(|| {
            PreflightError::Vps(VpsError::StatusCheckFailed(
                "VPS has no hostname, url, or IP address".to_string(),
            ))
        })?;
        (Some(vps), Some(url))
    };

    // Step 3: Health check loop (unless skipped - for local dev)
    if !config.skip_health_check {
        if let (Some(ref vps), Some(ref url)) = (&vps_state, &vps_url) {
            run_health_check_loop(runtime, url, &mut credentials, &manager, vps.ip.as_deref())
                .map_err(PreflightError::HealthCheck)?;
        }
    }

    // Step 4: Start debug system (unless disabled)
    let (debug_tx, debug_handle, debug_snapshot) = if config.enable_debug {
        let debug_result = runtime.block_on(start_debug_system(config.debug_port));
        (
            debug_result.tx,
            debug_result.server_handle,
            debug_result.state_snapshot,
        )
    } else {
        (None, None, None)
    };

    // Build startup result
    let result = StartupResult::new(credentials)
        .with_vps_state(vps_state)
        .with_vps_url(vps_url)
        .with_debug(debug_tx, debug_handle, debug_snapshot);

    println!("Starting SPOQ...\n");

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preflight_error_display() {
        let err = PreflightError::CredentialsManager("test".to_string());
        assert!(err.to_string().contains("test"));

        let err = PreflightError::HealthCheck("failed".to_string());
        assert!(err.to_string().contains("failed"));
    }

    #[test]
    fn test_preflight_error_from_auth_error() {
        let auth_err = AuthError::RefreshFailed("test".to_string());
        let preflight_err: PreflightError = auth_err.into();

        match preflight_err {
            PreflightError::Auth(_) => {}
            _ => panic!("Expected Auth variant"),
        }
    }

    #[test]
    fn test_preflight_error_from_vps_error() {
        let vps_err = VpsError::StatusCheckFailed("test".to_string());
        let preflight_err: PreflightError = vps_err.into();

        match preflight_err {
            PreflightError::Vps(_) => {}
            _ => panic!("Expected Vps variant"),
        }
    }

    // Note: Full integration test for run_preflight_checks would require
    // extensive mocking of HTTP clients, file system, and user input.
    // These are better suited for integration tests with test fixtures.
}
