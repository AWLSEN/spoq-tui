//! Setup module for Spoq TUI.
//!
//! This module handles the setup flow steps for the CLI:
//! - Pre-check: Determine if a VPS already exists for the user
//! - Provision: Create a new VPS (future)
//! - Health-wait: Wait for VPS to become healthy
//! - Creds-sync: Sync credentials to VPS (future)
//! - Creds-verify: Verify credentials on VPS (future)

pub mod health_wait;
pub mod precheck;

pub use health_wait::{wait_for_health, wait_for_health_with_progress, HealthWaitError, DEFAULT_HEALTH_TIMEOUT_SECS};
pub use precheck::{precheck, VpsStatus};
