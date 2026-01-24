//! Command handlers for executing commands.
//!
//! This module contains handler functions organized by category:
//! - [`navigation`] - Focus and screen navigation
//! - [`editing`] - Text input and editing
//! - [`permission`] - Permission prompt handling

pub mod editing;
pub mod navigation;
pub mod permission;

pub use editing::*;
pub use navigation::*;
pub use permission::*;
