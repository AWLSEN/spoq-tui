//! Command-line argument parsing for Spoq CLI.
//!
//! This module handles parsing command-line arguments and determining
//! which CLI command to execute.

/// Parsed CLI command to execute.
#[derive(Debug, Clone, PartialEq)]
pub enum CliCommand {
    /// Show version information
    Version,
    /// Check for and install updates
    Update,
    /// Sync tokens to VPS
    Sync,
    /// Run the TUI application (default)
    RunTui,
}

/// Parse command-line arguments and return the appropriate command.
///
/// # Arguments
///
/// * `args` - Iterator of command-line arguments (typically `std::env::args()`)
///
/// # Returns
///
/// The `CliCommand` to execute based on the arguments.
///
/// # Examples
///
/// ```
/// use spoq::cli::args::{parse_args, CliCommand};
///
/// let args = vec!["spoq".to_string(), "--version".to_string()];
/// assert_eq!(parse_args(args.into_iter()), CliCommand::Version);
/// ```
pub fn parse_args<I>(args: I) -> CliCommand
where
    I: Iterator<Item = String>,
{
    for arg in args.skip(1) {
        // Skip the program name
        match arg.as_str() {
            "--version" | "-V" => return CliCommand::Version,
            "--update" => return CliCommand::Update,
            "--sync" | "/sync" => return CliCommand::Sync,
            _ => {}
        }
    }
    CliCommand::RunTui
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_version_flag() {
        let args = vec!["spoq".to_string(), "--version".to_string()];
        assert_eq!(parse_args(args.into_iter()), CliCommand::Version);
    }

    #[test]
    fn test_parse_version_short_flag() {
        let args = vec!["spoq".to_string(), "-V".to_string()];
        assert_eq!(parse_args(args.into_iter()), CliCommand::Version);
    }

    #[test]
    fn test_parse_update_flag() {
        let args = vec!["spoq".to_string(), "--update".to_string()];
        assert_eq!(parse_args(args.into_iter()), CliCommand::Update);
    }

    #[test]
    fn test_parse_sync_flag() {
        let args = vec!["spoq".to_string(), "--sync".to_string()];
        assert_eq!(parse_args(args.into_iter()), CliCommand::Sync);
    }

    #[test]
    fn test_parse_sync_slash_flag() {
        let args = vec!["spoq".to_string(), "/sync".to_string()];
        assert_eq!(parse_args(args.into_iter()), CliCommand::Sync);
    }

    #[test]
    fn test_parse_no_args() {
        let args = vec!["spoq".to_string()];
        assert_eq!(parse_args(args.into_iter()), CliCommand::RunTui);
    }

    #[test]
    fn test_parse_unknown_flag() {
        let args = vec!["spoq".to_string(), "--unknown".to_string()];
        assert_eq!(parse_args(args.into_iter()), CliCommand::RunTui);
    }
}
