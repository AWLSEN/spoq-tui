//! Version command for Spoq CLI.
//!
//! Displays the current version of the Spoq application.

/// The current version of Spoq, read from Cargo.toml at compile time.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Handle the --version command.
///
/// Prints the version string and exits successfully.
pub fn handle_version_command() -> ! {
    println!("spoq {}", VERSION);
    std::process::exit(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_is_not_empty() {
        assert!(!VERSION.is_empty());
    }

    #[test]
    fn test_version_format() {
        // Version should be in semver format (e.g., "0.1.0")
        let parts: Vec<&str> = VERSION.split('.').collect();
        assert!(parts.len() >= 2, "Version should have at least major.minor");
    }
}
