//! Startup module for pre-flight initialization with dependency injection.
//!
//! This module extracts startup logic from main() into testable, modular components.
//! Each component accepts injected dependencies for testing and flexibility.
//!
//! # Components
//!
//! - [`config`] - Startup configuration types
//! - [`preflight`] - Main preflight orchestration
//! - [`auth`] - Authentication and credential validation
//! - [`vps`] - VPS verification and management
//! - [`health`] - Health check loop
//! - [`debug`] - Debug system initialization
//!
//! # Usage
//!
//! ```ignore
//! use spoq::startup::{StartupConfig, run_preflight_checks, StartupResult};
//!
//! let config = StartupConfig::default();
//! let result = run_preflight_checks(&runtime, config)?;
//! // result.credentials, result.vps_state are ready for TUI
//! ```

pub mod auth;
pub mod config;
pub mod debug;
pub mod health;
pub mod preflight;
pub mod vps;

pub use config::{SpoqConfig, StartupConfig, StartupResult};
pub use preflight::run_preflight_checks;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_startup_config_default() {
        let config = StartupConfig::default();
        assert!(config.skip_update_check);
        assert!(!config.skip_vps_check);
        assert!(!config.skip_health_check);
    }

    #[test]
    fn test_startup_config_builder() {
        let config = StartupConfig::default()
            .with_skip_update_check(false)
            .with_skip_vps_check(true);

        assert!(!config.skip_update_check);
        assert!(config.skip_vps_check);
    }
}
