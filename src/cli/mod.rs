//! CLI module for Spoq.
//!
//! This module provides command-line interface functionality including:
//! - Argument parsing
//! - Version display
//! - Update checking and installation
//! - Token synchronization to VPS
//!
//! # Usage
//!
//! The CLI dispatcher should be called early in main() to handle command-line
//! flags before initializing the TUI:
//!
//! ```ignore
//! use spoq::cli::{parse_args, run_cli_command, CliCommand};
//!
//! let command = parse_args(std::env::args());
//! if let Some(result) = run_cli_command(command) {
//!     // CLI command was executed, exit with result
//!     if let Err(e) = result {
//!         eprintln!("Error: {}", e);
//!         std::process::exit(1);
//!     }
//!     std::process::exit(0);
//! }
//! // No CLI command, continue to TUI
//! ```

pub mod args;
pub mod sync;
pub mod update;
pub mod version;

pub use args::{parse_args, CliCommand};
pub use sync::handle_sync_command;
pub use update::handle_update_command;
pub use version::{handle_version_command, VERSION};

use color_eyre::Result;

/// Run a CLI command if applicable.
///
/// # Arguments
///
/// * `command` - The parsed CLI command
///
/// # Returns
///
/// * `None` - If the command is `RunTui` (no CLI action needed)
/// * `Some(Ok(()))` - If a CLI command executed successfully
/// * `Some(Err(e))` - If a CLI command failed
///
/// # Note
///
/// The `Version` command never returns as it calls `std::process::exit(0)`.
pub fn run_cli_command(command: CliCommand) -> Option<Result<()>> {
    match command {
        CliCommand::Version => {
            // This function never returns (calls exit)
            handle_version_command();
        }
        CliCommand::Update => Some(handle_update_command()),
        CliCommand::Sync => Some(handle_sync_command()),
        CliCommand::RunTui => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_tui_returns_none() {
        let result = run_cli_command(CliCommand::RunTui);
        assert!(result.is_none());
    }
}
