//! Setup module for Spoq TUI.
//!
//! This module provides GitHub CLI and Claude CLI authentication automation for the setup flow.

pub mod claude_auth;
pub mod gh_auth;

pub use claude_auth::{run_claude_setup_token, run_claude_setup_token_async, ClaudeAuthError, ClaudeAuthResult};
pub use gh_auth::{ensure_gh_authenticated, is_gh_authenticated, is_gh_installed, GhAuthError};
