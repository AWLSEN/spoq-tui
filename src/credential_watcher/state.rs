//! State tracking for credential change detection and sync coordination.
//!
//! Will be fully implemented in Phase 2.

use std::time::{Duration, Instant};

/// Exponential backoff for sync failures
#[derive(Debug, Clone, Default)]
pub struct ExponentialBackoff {
    pub failure_count: u32,
}

impl ExponentialBackoff {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Main state for credential watching
#[derive(Debug, Default)]
pub struct CredentialWatchState {
    pub keychain_hash: Option<u64>,
    pub last_sync: Option<Instant>,
    pub backoff: ExponentialBackoff,
    pub sync_pending: bool,
    pub pending_change: Option<Instant>,
    pub enabled: bool,
}

impl CredentialWatchState {
    pub fn new() -> Self {
        Self {
            enabled: true,
            ..Default::default()
        }
    }
}
