//! Setup module for Spoq TUI.
//!
//! This module handles the setup flow steps for the CLI:
//! - Flow orchestrator: Main entry point that runs all steps in sequence
//! - Pre-check: Determine if a VPS already exists for the user
//! - Provision: Create a new VPS via the Central API
//! - Health-wait: Wait for VPS to become healthy
//! - Creds-sync: Sync credentials to VPS via HTTP API (includes verification)
//! - GitHub Auth: Automated GitHub CLI installation and authentication

pub mod flow;
pub mod gh_auth;
pub mod health_wait;
pub mod keychain;
pub mod precheck;
pub mod provision;

// Legacy SSH-based sync modules - kept for backwards compatibility but not used
// Credential sync now uses HTTP via ConductorClient::sync_tokens()
#[deprecated(since = "0.2.0", note = "Use ConductorClient::sync_tokens() instead")]
pub mod creds_sync;
#[deprecated(since = "0.2.0", note = "Verification is now included in sync_tokens() response")]
pub mod creds_verify;

pub use flow::{run_setup_flow, SetupError, SetupResult, SetupStep, SetupSuccess};
pub use gh_auth::{ensure_gh_authenticated, is_gh_authenticated, is_gh_installed, GhAuthError};
pub use health_wait::{
    wait_for_health, wait_for_health_with_progress, HealthWaitError, DEFAULT_HEALTH_TIMEOUT_SECS,
};
pub use keychain::{extract_claude_credentials, extract_claude_credentials_simple, KeychainResult};
pub use precheck::{precheck, VpsStatus};
pub use provision::{provision, provision_with_options, ProvisionError, ProvisionResponse};
