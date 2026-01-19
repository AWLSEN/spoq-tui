//! Authentication module for Spoq TUI.
//!
//! This module provides authentication functionality including:
//! - Credentials storage and management
//! - Central API client for authentication endpoints

pub mod central_api;
pub mod credentials;

pub use central_api::CentralApiClient;
pub use credentials::{Credentials, CredentialsManager};
