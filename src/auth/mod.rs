//! Authentication module for Spoq TUI.
//!
//! This module handles authentication before the TUI starts. Authentication flows
//! run synchronously during application startup to ensure users are authenticated
//! before entering the TUI interface.
//!
//! This module provides:
//! - Credentials storage and management
//! - Central API client for authentication endpoints
//! - Device authorization flow (RFC 8628)
//! - Pre-TUI authentication and provisioning flows

pub mod central_api;
pub mod credentials;
pub mod flow;
pub mod provisioning_flow;
pub mod token_migration;

pub use central_api::CentralApiClient;
pub use credentials::{Credentials, CredentialsManager};
pub use flow::run_auth_flow;
pub use provisioning_flow::{run_provisioning_flow, start_stopped_vps};
pub use token_migration::{
    detect_tokens, export_tokens, transfer_tokens_to_vps, transfer_tokens_with_credentials,
    wait_for_claude_code_token, SshTransferError, TokenDetectionResult, TokenExportResult,
    TokenTransferResult, VpsConnectionInfo,
};
