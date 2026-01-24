//! Setup Flow Orchestrator for SPOQ CLI.
//!
//! This module implements the main orchestrator that runs Steps 0-5 of the setup flow
//! in sequence with proper state management and user feedback.
//!
//! ## Setup Steps
//!
//! - **Step 0 (AUTH)**: Ensure user is authenticated via `ensure_authenticated()`
//! - **Step 1 (PRE-CHECK)**: Check if VPS exists via `precheck()`
//! - **Step 2 (PROVISION)**: Create VPS if needed via `provision()`
//! - **Step 3 (HEALTH-WAIT)**: Wait for VPS to become healthy via `wait_for_health()`
//! - **Step 4 (CREDS-SYNC)**: Sync credentials to VPS via `sync_credentials()`
//! - **Step 5 (CREDS-VERIFY)**: Verify credentials work via `verify_credentials()`
//!
//! ## Usage
//!
//! ```no_run
//! use spoq::setup::flow::run_setup_flow;
//!
//! let runtime = tokio::runtime::Runtime::new().unwrap();
//! match run_setup_flow(&runtime) {
//!     Ok(result) => println!("Setup complete! VPS URL: {}", result.vps_url),
//!     Err(e) => eprintln!("Setup failed at step {:?}: {}", e.step, e.message),
//! }
//! ```

use crate::auth::{ensure_authenticated, Credentials};
use crate::auth::central_api::CentralApiClient;
use super::precheck::{precheck, VpsStatus};
use super::provision::{provision, ProvisionError};
use super::health_wait::{wait_for_health, HealthWaitError, DEFAULT_HEALTH_TIMEOUT_SECS};
use super::creds_sync::sync_credentials;
use super::creds_verify::verify_credentials;

use std::fmt;
use std::io::{self, Write};

/// The current step in the setup flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupStep {
    /// Step 0: Authentication
    Auth,
    /// Step 1: Pre-check for existing VPS
    PreCheck,
    /// Step 2: VPS provisioning
    Provision,
    /// Step 3: Waiting for VPS health
    HealthWait,
    /// Step 4: Credentials sync
    CredsSync,
    /// Step 5: Credentials verification
    CredsVerify,
}

impl SetupStep {
    /// Returns the step number (0-5).
    pub fn number(&self) -> u8 {
        match self {
            SetupStep::Auth => 0,
            SetupStep::PreCheck => 1,
            SetupStep::Provision => 2,
            SetupStep::HealthWait => 3,
            SetupStep::CredsSync => 4,
            SetupStep::CredsVerify => 5,
        }
    }

    /// Returns a human-readable description of the step.
    pub fn description(&self) -> &'static str {
        match self {
            SetupStep::Auth => "Authenticating",
            SetupStep::PreCheck => "Checking VPS status",
            SetupStep::Provision => "Provisioning VPS",
            SetupStep::HealthWait => "Waiting for VPS",
            SetupStep::CredsSync => "Syncing credentials",
            SetupStep::CredsVerify => "Verifying credentials",
        }
    }
}

impl fmt::Display for SetupStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Step {}: {}", self.number(), self.description())
    }
}

/// Successful result of the setup flow.
#[derive(Debug, Clone)]
pub struct SetupSuccess {
    /// The VPS URL for connecting to the conductor
    pub vps_url: String,
    /// The VPS hostname
    pub vps_hostname: Option<String>,
    /// The VPS IP address
    pub vps_ip: Option<String>,
    /// The VPS ID
    pub vps_id: String,
    /// Updated credentials after setup
    pub credentials: Credentials,
}

/// Error result of the setup flow.
#[derive(Debug)]
pub struct SetupError {
    /// The step where the error occurred
    pub step: SetupStep,
    /// Human-readable error message
    pub message: String,
    /// Whether this error is fatal and should block TUI access
    pub is_blocking: bool,
}

impl fmt::Display for SetupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.step, self.message)
    }
}

impl std::error::Error for SetupError {}

impl SetupError {
    /// Create a new SetupError.
    pub fn new(step: SetupStep, message: impl Into<String>, is_blocking: bool) -> Self {
        Self {
            step,
            message: message.into(),
            is_blocking,
        }
    }

    /// Create a blocking error (fatal - blocks TUI access).
    pub fn blocking(step: SetupStep, message: impl Into<String>) -> Self {
        Self::new(step, message, true)
    }

    /// Create a non-blocking error (warning - allows TUI access with degraded functionality).
    #[allow(dead_code)]
    pub fn non_blocking(step: SetupStep, message: impl Into<String>) -> Self {
        Self::new(step, message, false)
    }
}

/// Result type for the setup flow.
pub type SetupResult = Result<SetupSuccess, SetupError>;

/// Central API base URL.
const CENTRAL_API_BASE: &str = "https://spoq.dev";

/// Default SSH port.
const SSH_PORT: u16 = 22;

/// Print progress indicator for a step.
fn print_step_progress(step: SetupStep, total_steps: u8) {
    print!(
        "\r[{}/{}] {}...",
        step.number() + 1,
        total_steps,
        step.description()
    );
    let _ = io::stdout().flush();
}

/// Print step completion.
fn print_step_complete(step: SetupStep, total_steps: u8) {
    println!(
        "\r[{}/{}] {} ✓",
        step.number() + 1,
        total_steps,
        step.description()
    );
}

/// Print step skipped message.
fn print_step_skipped(step: SetupStep, total_steps: u8, reason: &str) {
    println!(
        "\r[{}/{}] {} (skipped: {})",
        step.number() + 1,
        total_steps,
        step.description(),
        reason
    );
}

/// Run the complete setup flow.
///
/// Orchestrates Steps 0-5 in sequence:
/// 1. Ensure user is authenticated
/// 2. Check if VPS exists (pre-check)
/// 3. Provision VPS if needed
/// 4. Wait for VPS to become healthy
/// 5. Sync credentials to VPS
/// 6. Verify credentials work on VPS
///
/// # Arguments
/// * `runtime` - Tokio runtime for async operations
///
/// # Returns
/// * `Ok(SetupSuccess)` - Setup completed successfully with VPS URL
/// * `Err(SetupError)` - Setup failed with step and error message
///
/// # Example
/// ```no_run
/// use spoq::setup::flow::run_setup_flow;
///
/// let runtime = tokio::runtime::Runtime::new().unwrap();
/// match run_setup_flow(&runtime) {
///     Ok(result) => {
///         println!("Setup complete!");
///         println!("VPS URL: {}", result.vps_url);
///     }
///     Err(e) => {
///         eprintln!("Setup failed at {}: {}", e.step, e.message);
///         if e.is_blocking {
///             std::process::exit(1);
///         }
///     }
/// }
/// ```
pub fn run_setup_flow(runtime: &tokio::runtime::Runtime) -> SetupResult {
    // Total steps in the flow (for progress display)
    const TOTAL_STEPS: u8 = 6;

    println!("Starting SPOQ setup...\n");

    // =========================================================
    // Step 0: AUTH - Ensure user is authenticated
    // =========================================================
    print_step_progress(SetupStep::Auth, TOTAL_STEPS);

    let credentials = ensure_authenticated(runtime).map_err(|e| {
        SetupError::blocking(SetupStep::Auth, format!("Authentication failed: {}", e))
    })?;

    print_step_complete(SetupStep::Auth, TOTAL_STEPS);

    // =========================================================
    // Step 1: PRE-CHECK - Check if VPS exists
    // =========================================================
    print_step_progress(SetupStep::PreCheck, TOTAL_STEPS);

    let access_token = credentials.access_token.as_ref().ok_or_else(|| {
        SetupError::blocking(
            SetupStep::PreCheck,
            "No access token available after authentication",
        )
    })?;

    let mut client = CentralApiClient::new().with_auth(access_token);

    let vps_status = runtime
        .block_on(precheck(&mut client))
        .map_err(|e| SetupError::blocking(SetupStep::PreCheck, format!("VPS check failed: {}", e)))?;

    // Determine if we need to provision or just continue with existing VPS
    let (vps_id, vps_hostname, vps_ip, vps_url, needs_provision, needs_health_wait) = match &vps_status {
        VpsStatus::None => {
            print_step_complete(SetupStep::PreCheck, TOTAL_STEPS);
            (None, None, None, None, true, true)
        }
        VpsStatus::Provisioning { vps_id } => {
            print_step_complete(SetupStep::PreCheck, TOTAL_STEPS);
            println!("  VPS {} is currently provisioning...", vps_id);
            // VPS exists but still provisioning - skip provision step, wait for health
            (Some(vps_id.clone()), None, None, None, false, true)
        }
        VpsStatus::Ready { vps_id, hostname, ip, url, .. } => {
            print_step_skipped(SetupStep::PreCheck, TOTAL_STEPS, "VPS already exists");
            // VPS is ready - skip both provision and health wait
            (
                Some(vps_id.clone()),
                hostname.clone(),
                ip.clone(),
                url.clone(),
                false,
                false,
            )
        }
        VpsStatus::Other { vps_id, status } => {
            // Unknown status - try to continue with health wait
            print_step_complete(SetupStep::PreCheck, TOTAL_STEPS);
            println!("  VPS {} has status '{}', checking health...", vps_id, status);
            (Some(vps_id.clone()), None, None, None, false, true)
        }
    };

    // Track VPS info for later steps
    let mut final_vps_id = vps_id;
    let mut final_hostname = vps_hostname;
    let final_ip = vps_ip;
    let mut final_url = vps_url;

    // =========================================================
    // Step 2: PROVISION - Create VPS if needed
    // =========================================================
    if needs_provision {
        print_step_progress(SetupStep::Provision, TOTAL_STEPS);

        let provision_response = runtime
            .block_on(provision(access_token, CENTRAL_API_BASE))
            .map_err(|e| {
                let message = match e {
                    ProvisionError::Unauthorized => {
                        "Session expired - please restart and sign in again".to_string()
                    }
                    ProvisionError::PaymentRequired => {
                        "Payment required - please subscribe at https://spoq.dev/subscribe".to_string()
                    }
                    ProvisionError::AlreadyHasVps => {
                        "You already have a VPS provisioned".to_string()
                    }
                    ProvisionError::QuotaExceeded => {
                        "VPS quota exceeded - please contact support@spoq.dev".to_string()
                    }
                    _ => format!("Provisioning failed: {}", e),
                };
                SetupError::blocking(SetupStep::Provision, message)
            })?;

        final_vps_id = Some(provision_response.vps_id.clone());
        final_hostname = provision_response.hostname.clone();
        // Domain can be used as hostname
        if final_hostname.is_none() {
            final_hostname = provision_response.domain.clone();
        }

        print_step_complete(SetupStep::Provision, TOTAL_STEPS);
        if let Some(ref domain) = provision_response.get_domain() {
            println!("  VPS domain: {}", domain);
        }
    } else if !needs_health_wait {
        // Skip provision step entirely - VPS already ready
        print_step_skipped(SetupStep::Provision, TOTAL_STEPS, "VPS already provisioned");
    }

    // =========================================================
    // Step 3: HEALTH-WAIT - Wait for VPS to become healthy
    // =========================================================
    if needs_health_wait {
        print_step_progress(SetupStep::HealthWait, TOTAL_STEPS);

        // Build health check URL from hostname
        let health_url = if let Some(ref hostname) = final_hostname {
            format!("https://{}", hostname)
        } else {
            // Try to get hostname from credentials or use API
            return Err(SetupError::blocking(
                SetupStep::HealthWait,
                "No VPS hostname available for health check",
            ));
        };

        runtime
            .block_on(wait_for_health(&health_url, DEFAULT_HEALTH_TIMEOUT_SECS))
            .map_err(|e| {
                let message = match e {
                    HealthWaitError::Timeout { waited_secs } => {
                        format!(
                            "VPS did not become healthy within {} seconds. \
                             Please check VPS status at https://spoq.dev/dashboard",
                            waited_secs
                        )
                    }
                    HealthWaitError::Unhealthy { message } => {
                        format!("VPS health check failed: {}", message)
                    }
                    HealthWaitError::Http(e) => {
                        format!("Network error during health check: {}", e)
                    }
                };
                SetupError::blocking(SetupStep::HealthWait, message)
            })?;

        // Set the VPS URL now that we know it's healthy
        if final_url.is_none() {
            if let Some(ref hostname) = final_hostname {
                final_url = Some(format!("https://{}", hostname));
            }
        }

        print_step_complete(SetupStep::HealthWait, TOTAL_STEPS);
    } else {
        print_step_skipped(SetupStep::HealthWait, TOTAL_STEPS, "VPS already healthy");
    }

    // At this point we must have VPS info
    let vps_id = final_vps_id.ok_or_else(|| {
        SetupError::blocking(SetupStep::HealthWait, "No VPS ID available after health check")
    })?;

    let vps_url = final_url.ok_or_else(|| {
        SetupError::blocking(SetupStep::HealthWait, "No VPS URL available after health check")
    })?;

    // Get VPS IP for SSH operations - need to fetch from API if not available
    let vps_ip = if let Some(ip) = final_ip {
        ip
    } else {
        // Try to fetch VPS info from API to get IP
        let vps_response = runtime
            .block_on(client.fetch_user_vps())
            .map_err(|e| SetupError::blocking(SetupStep::CredsSync, format!("Failed to get VPS info: {}", e)))?
            .ok_or_else(|| {
                SetupError::blocking(SetupStep::CredsSync, "VPS not found in API after provisioning")
            })?;

        vps_response.ip.ok_or_else(|| {
            SetupError::blocking(
                SetupStep::CredsSync,
                "VPS IP address not available - cannot sync credentials via SSH",
            )
        })?
    };

    // Get SSH password from environment variable
    // SSH password is not stored in credentials for security reasons
    // It's typically provided during provisioning and passed via environment
    let ssh_password = std::env::var("SPOQ_VPS_SSH_PASSWORD").unwrap_or_default();

    // =========================================================
    // Step 4: CREDS-SYNC - Sync credentials to VPS
    // =========================================================
    print_step_progress(SetupStep::CredsSync, TOTAL_STEPS);

    if ssh_password.is_empty() {
        // SSH password not available - skip credential sync
        // This can happen for existing VPS where password was not stored
        print_step_skipped(
            SetupStep::CredsSync,
            TOTAL_STEPS,
            "SSH password not available",
        );
    } else {
        let sync_result = runtime
            .block_on(sync_credentials(&vps_ip, "root", &ssh_password, SSH_PORT))
            .map_err(|e| {
                // Credential sync failure is blocking
                SetupError::blocking(SetupStep::CredsSync, format!("Credential sync failed: {}", e))
            })?;

        if sync_result.any_synced() {
            print_step_complete(SetupStep::CredsSync, TOTAL_STEPS);
            if sync_result.claude_synced {
                println!("  Claude credentials synced ({} bytes)", sync_result.claude_bytes);
            }
            if sync_result.github_synced {
                println!("  GitHub credentials synced ({} bytes)", sync_result.github_bytes);
            }
        } else {
            // No credentials found locally to sync
            print_step_skipped(
                SetupStep::CredsSync,
                TOTAL_STEPS,
                "No local credentials found",
            );
        }
    }

    // =========================================================
    // Step 5: CREDS-VERIFY - Verify credentials work on VPS
    // =========================================================
    print_step_progress(SetupStep::CredsVerify, TOTAL_STEPS);

    if ssh_password.is_empty() {
        // Can't verify without SSH access
        print_step_skipped(
            SetupStep::CredsVerify,
            TOTAL_STEPS,
            "SSH password not available",
        );
    } else {
        let verify_result = runtime
            .block_on(verify_credentials(&vps_ip, "root", &ssh_password, SSH_PORT))
            .map_err(|e| {
                // Credential verification failure is BLOCKING
                SetupError::blocking(
                    SetupStep::CredsVerify,
                    format!(
                        "Credential verification failed: {}. \
                         Please ensure Claude Code and GitHub CLI are authenticated locally, \
                         then run 'spoq --sync' to sync credentials.",
                        e
                    ),
                )
            })?;

        if verify_result.all_ok() {
            print_step_complete(SetupStep::CredsVerify, TOTAL_STEPS);
            println!("  Claude Code: ✓");
            println!("  GitHub CLI: ✓");
        } else {
            // Partial verification - this is blocking
            let mut failures = Vec::new();
            if !verify_result.github_ok {
                failures.push("GitHub CLI not authenticated");
            }
            if !verify_result.claude_ok {
                failures.push("Claude Code not authenticated");
            }
            return Err(SetupError::blocking(
                SetupStep::CredsVerify,
                format!(
                    "Credential verification failed: {}. \
                     Please run 'gh auth login' and 'claude' (then /login) locally, \
                     then run 'spoq --sync' to sync credentials.",
                    failures.join(", ")
                ),
            ));
        }
    }

    // =========================================================
    // Setup Complete - Build final credentials
    // =========================================================
    println!("\n✓ Setup complete!");

    // NOTE: VPS info is NOT stored in credentials anymore.
    // VPS state is always fetched from the API (single source of truth).
    // The SetupSuccess struct contains the VPS info for the current session.

    Ok(SetupSuccess {
        vps_url,
        vps_hostname: final_hostname,
        vps_ip: Some(vps_ip),
        vps_id,
        credentials: credentials.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_setup_step_number() {
        assert_eq!(SetupStep::Auth.number(), 0);
        assert_eq!(SetupStep::PreCheck.number(), 1);
        assert_eq!(SetupStep::Provision.number(), 2);
        assert_eq!(SetupStep::HealthWait.number(), 3);
        assert_eq!(SetupStep::CredsSync.number(), 4);
        assert_eq!(SetupStep::CredsVerify.number(), 5);
    }

    #[test]
    fn test_setup_step_description() {
        assert_eq!(SetupStep::Auth.description(), "Authenticating");
        assert_eq!(SetupStep::PreCheck.description(), "Checking VPS status");
        assert_eq!(SetupStep::Provision.description(), "Provisioning VPS");
        assert_eq!(SetupStep::HealthWait.description(), "Waiting for VPS");
        assert_eq!(SetupStep::CredsSync.description(), "Syncing credentials");
        assert_eq!(SetupStep::CredsVerify.description(), "Verifying credentials");
    }

    #[test]
    fn test_setup_step_display() {
        assert_eq!(format!("{}", SetupStep::Auth), "Step 0: Authenticating");
        assert_eq!(format!("{}", SetupStep::CredsVerify), "Step 5: Verifying credentials");
    }

    #[test]
    fn test_setup_error_blocking() {
        let err = SetupError::blocking(SetupStep::Auth, "test error");
        assert!(err.is_blocking);
        assert_eq!(err.step, SetupStep::Auth);
        assert_eq!(err.message, "test error");
    }

    #[test]
    fn test_setup_error_non_blocking() {
        let err = SetupError::non_blocking(SetupStep::CredsSync, "warning");
        assert!(!err.is_blocking);
        assert_eq!(err.step, SetupStep::CredsSync);
        assert_eq!(err.message, "warning");
    }

    #[test]
    fn test_setup_error_display() {
        let err = SetupError::blocking(SetupStep::Provision, "test message");
        let display = format!("{}", err);
        assert!(display.contains("Step 2"));
        assert!(display.contains("Provisioning VPS"));
        assert!(display.contains("test message"));
    }

    #[test]
    fn test_setup_success_fields() {
        let success = SetupSuccess {
            vps_url: "https://test.spoq.dev".to_string(),
            vps_hostname: Some("test.spoq.dev".to_string()),
            vps_ip: Some("1.2.3.4".to_string()),
            vps_id: "vps-123".to_string(),
            credentials: Credentials::default(),
        };

        assert_eq!(success.vps_url, "https://test.spoq.dev");
        assert_eq!(success.vps_hostname, Some("test.spoq.dev".to_string()));
        assert_eq!(success.vps_ip, Some("1.2.3.4".to_string()));
        assert_eq!(success.vps_id, "vps-123");
    }
}
