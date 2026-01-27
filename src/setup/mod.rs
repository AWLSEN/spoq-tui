//! Setup module for Spoq TUI.
//!
//! This module provides GitHub CLI authentication automation for the setup flow.

pub mod gh_auth;

pub use gh_auth::{ensure_gh_authenticated, is_gh_authenticated, is_gh_installed, GhAuthError};
