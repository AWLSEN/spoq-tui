//! Backend coordinator for thread mode synchronization.
//!
//! This module handles the actual API calls to sync thread mode and permission mode
//! to the backend, with retry logic and graceful error handling.

use crate::conductor::{ConductorClient, ConductorError};
use crate::models::PermissionMode;
use std::time::Duration;
use tracing::{warn, error};

/// Error type for backend coordinator operations.
#[derive(Debug)]
pub enum BackendError {
    /// Error from the conductor client
    Conductor(ConductorError),
    /// Both endpoints failed
    BothFailed,
}

impl std::fmt::Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackendError::Conductor(e) => write!(f, "Conductor error: {}", e),
            BackendError::BothFailed => write!(f, "Both mode and permission updates failed"),
        }
    }
}

impl std::error::Error for BackendError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            BackendError::Conductor(e) => Some(e),
            BackendError::BothFailed => None,
        }
    }
}

impl From<ConductorError> for BackendError {
    fn from(e: ConductorError) -> Self {
        BackendError::Conductor(e)
    }
}

/// Synchronize thread mode and permission mode to the backend.
///
/// This function calls both `update_thread_mode` and `update_thread_permission`
/// with retry-once logic for transient errors.
///
/// # Arguments
/// * `conductor` - The conductor client for API calls
/// * `thread_id` - The ID of the thread to update
/// * `permission_mode` - The new permission mode
///
/// # Returns
/// * `Ok(())` - At least one endpoint succeeded, or both failed but we logged it
/// * `Err(BackendError)` - Only if we want to propagate (currently we always return Ok)
pub async fn sync_thread_mode(
    conductor: &ConductorClient,
    thread_id: &str,
    permission_mode: PermissionMode,
) -> Result<(), BackendError> {
    let (thread_mode, permission_mode_str) = map_permission_mode(permission_mode);

    // Try both API calls concurrently
    let (mode_result, perm_result) = tokio::join!(
        try_update_with_retry(conductor, thread_id, thread_mode, true),
        try_update_with_retry(conductor, thread_id, permission_mode_str, false),
    );

    // Log failures but don't propagate errors (fail-quietly)
    match (mode_result, perm_result) {
        (Ok(()), Ok(())) => {
            tracing::info!(
                thread_id = %thread_id,
                mode = %thread_mode,
                permission_mode = %permission_mode_str,
                "Successfully synced thread mode"
            );
        }
        (Ok(()), Err(e)) => {
            warn!(
                thread_id = %thread_id,
                mode = %thread_mode,
                permission_mode = %permission_mode_str,
                error = %e,
                "Thread mode synced but permission mode failed"
            );
        }
        (Err(e), Ok(())) => {
            warn!(
                thread_id = %thread_id,
                mode = %thread_mode,
                permission_mode = %permission_mode_str,
                error = %e,
                "Permission mode synced but thread mode failed"
            );
        }
        (Err(mode_err), Err(perm_err)) => {
            error!(
                thread_id = %thread_id,
                mode = %thread_mode,
                permission_mode = %permission_mode_str,
                mode_error = %mode_err,
                permission_error = %perm_err,
                "Both thread mode and permission mode failed to sync"
            );
        }
    }

    // Always return Ok - we fail-quietly and let local state remain authoritative
    Ok(())
}

/// Map PermissionMode to (ThreadMode, permission_mode string).
fn map_permission_mode(permission_mode: PermissionMode) -> (&'static str, &'static str) {
    match permission_mode {
        PermissionMode::Default => ("normal", "default"),
        PermissionMode::Plan => ("plan", "plan"),
        PermissionMode::Execution => ("exec", "execution"),
    }
}

/// Try to update with retry-once logic for transient errors.
async fn try_update_with_retry(
    conductor: &ConductorClient,
    thread_id: &str,
    mode: &str,
    is_thread_mode: bool,
) -> Result<(), ConductorError> {
    let result = if is_thread_mode {
        conductor.update_thread_mode(thread_id, mode).await
    } else {
        conductor.update_thread_permission(thread_id, mode).await
    };

    // Check if error is transient (network or 5xx) - retry once
    if let Err(ConductorError::ServerError { status, .. }) = &result {
        if is_transient_error(*status) {
            tracing::debug!(
                thread_id = %thread_id,
                mode = %mode,
                status = %status,
                "Transient error, retrying once"
            );
            tokio::time::sleep(Duration::from_millis(100)).await;

            return if is_thread_mode {
                conductor.update_thread_mode(thread_id, mode).await
            } else {
                conductor.update_thread_permission(thread_id, mode).await
            };
        }
    }

    // Also retry on network errors
    if let Err(ConductorError::Http(_)) = &result {
        tracing::debug!(
            thread_id = %thread_id,
            mode = %mode,
            "Network error, retrying once"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;

        return if is_thread_mode {
            conductor.update_thread_mode(thread_id, mode).await
        } else {
            conductor.update_thread_permission(thread_id, mode).await
        };
    }

    result
}

/// Check if an HTTP status code indicates a transient error that should be retried.
fn is_transient_error(status: u16) -> bool {
    // Retry on 5xx server errors and 408 (Request Timeout)
    status >= 500 || status == 408
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_permission_mode() {
        assert_eq!(map_permission_mode(PermissionMode::Default), ("normal", "default"));
        assert_eq!(map_permission_mode(PermissionMode::Plan), ("plan", "plan"));
        assert_eq!(map_permission_mode(PermissionMode::Execution), ("exec", "execution"));
    }

    #[test]
    fn test_is_transient_error() {
        assert!(is_transient_error(500));
        assert!(is_transient_error(502));
        assert!(is_transient_error(503));
        assert!(is_transient_error(408));
        assert!(!is_transient_error(400));
        assert!(!is_transient_error(404));
        assert!(!is_transient_error(401));
    }

    #[test]
    fn test_backend_error_display() {
        let err = BackendError::BothFailed;
        assert!(format!("{}", err).contains("Both"));
    }
}
