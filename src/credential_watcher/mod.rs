//! Credential change detection and auto-sync system.
//!
//! This module provides automatic detection of credential changes and
//! triggers syncs to keep the VPS updated.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────┐     ┌─────────────────┐
//! │  File Watcher   │     │ Keychain Poller │
//! │  (notify crate) │     │  (30s polling)  │
//! └────────┬────────┘     └────────┬────────┘
//!          │                       │
//!          └───────────┬───────────┘
//!                      ▼
//!              ┌───────────────┐
//!              │   Debouncer   │
//!              │   (500ms)     │
//!              └───────┬───────┘
//!                      ▼
//!              ┌───────────────┐
//!              │  Coordinator  │
//!              │  (backoff)    │
//!              └───────┬───────┘
//!                      ▼
//!              ┌───────────────┐
//!              │ TriggerSync   │
//!              └───────────────┘
//! ```
//!
//! # Usage
//!
//! ```rust,ignore
//! // In main.rs, after app initialization:
//! let file_watcher = spawn_file_watcher(app.message_tx.clone())?;
//! let keychain_poller = spawn_keychain_poller(app.message_tx.clone());
//!
//! // Store handles in App to keep them alive
//! app.credential_file_watcher = Some(file_watcher);
//! app.credential_keychain_poller = Some(keychain_poller);
//! ```

// Submodules
mod coordinator;
mod debouncer;
mod file_watcher;
mod keychain_poller;
mod keychain_provider;
mod state;
mod types;

// Public exports
pub use coordinator::{
    handle_credential_change, handle_debounce_expired, handle_sync_complete, handle_sync_failed,
};
pub use debouncer::Debouncer;
pub use file_watcher::spawn_file_watcher;
pub use keychain_poller::{
    compute_keychain_hash_with_provider, get_current_hash, get_current_hash_with_provider,
    spawn_keychain_poller, spawn_keychain_poller_with_provider, POLL_INTERVAL_SECS,
};
pub use keychain_provider::{KeychainProvider, MockKeychain, RealKeychain};
pub use state::{CredentialWatchState, ExponentialBackoff};
pub use types::{CredentialChangeEvent, CredentialSource};
