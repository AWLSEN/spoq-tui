//! Authentication module for Spoq TUI.
//!
//! This module provides authentication functionality including:
//! - Credentials storage and management
//! - Central API client for authentication endpoints
//! - Device authorization flow (RFC 8628)

pub mod central_api;
pub mod credentials;
pub mod device_flow;

pub use central_api::CentralApiClient;
pub use credentials::{Credentials, CredentialsManager};
pub use device_flow::{DeviceFlowManager, DeviceFlowState};
