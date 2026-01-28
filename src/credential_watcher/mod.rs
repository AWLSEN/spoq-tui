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

// Submodules - will be implemented in subsequent phases
mod coordinator;
mod debouncer;
mod file_watcher;
mod keychain_poller;
mod state;
mod types;

// Public exports
pub use coordinator::{
    handle_credential_change, handle_debounce_expired, handle_sync_complete, handle_sync_failed,
};
pub use debouncer::Debouncer;
pub use file_watcher::spawn_file_watcher;
pub use keychain_poller::{get_current_hash, spawn_keychain_poller};
pub use state::{CredentialWatchState, ExponentialBackoff};
pub use types::{CredentialChangeEvent, CredentialSource};
