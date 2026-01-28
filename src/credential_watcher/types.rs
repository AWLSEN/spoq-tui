//! Shared types for credential change detection.
//!
//! Will be fully implemented in Phase 2.

use std::path::PathBuf;

/// Source of a credential change event
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CredentialSource {
    /// File-based credential (path included)
    File(PathBuf),
    /// macOS Keychain entry
    Keychain,
}

/// A detected credential change event
#[derive(Debug, Clone)]
pub struct CredentialChangeEvent {
    pub source: CredentialSource,
    pub timestamp: std::time::Instant,
}
