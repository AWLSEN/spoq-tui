//! Setup module for Spoq TUI.
//!
//! This module handles the setup flow steps for the CLI:
//! - Pre-check: Determine if a VPS already exists for the user
//! - Provision: Create a new VPS via the Central API
//! - Health-wait: Wait for VPS to become healthy
//! - Creds-sync: Sync credentials to VPS
//! - Creds-verify: Verify credentials on VPS

pub mod creds_sync;
pub mod creds_verify;
pub mod health_wait;
pub mod keychain;
pub mod precheck;
pub mod provision;

pub use creds_sync::{
    get_local_credentials_info, sync_and_verify_credentials, sync_credentials, CredsSyncError,
    CredsSyncResult,
};
pub use creds_verify::{verify_credentials, VerifyError, VerifyResult};
pub use health_wait::{
    wait_for_health, wait_for_health_with_progress, HealthWaitError, DEFAULT_HEALTH_TIMEOUT_SECS,
};
pub use keychain::{extract_claude_credentials, extract_claude_credentials_simple, KeychainResult};
pub use precheck::{precheck, VpsStatus};
pub use provision::{provision, provision_with_options, ProvisionError, ProvisionResponse};
