//! Authentication module for Spoq TUI.
//!
//! This module provides authentication functionality including:
//! - Credentials storage and management
//! - Central API client for authentication endpoints
//! - Device authorization flow (RFC 8628)
//! - Synchronous auth and provisioning flows

pub mod central_api;
pub mod credentials;
pub mod flow;
pub mod provisioning_flow;

pub use central_api::CentralApiClient;
pub use credentials::{Credentials, CredentialsManager};
pub use flow::run_auth_flow;
pub use provisioning_flow::run_provisioning_flow;
