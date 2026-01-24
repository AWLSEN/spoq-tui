//! Startup configuration types.
//!
//! This module defines configuration and result types for the startup process.

use crate::auth::central_api::VpsStatusResponse;
use crate::auth::credentials::Credentials;
use crate::debug::DebugEventSender;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

/// Configuration for the startup/preflight process.
///
/// Use the builder pattern to customize startup behavior.
///
/// # Example
///
/// ```ignore
/// use spoq::startup::StartupConfig;
///
/// let config = StartupConfig::default()
///     .with_skip_health_check(true)
///     .with_skip_update_check(false);
/// ```
#[derive(Debug, Clone)]
pub struct StartupConfig {
    /// Skip background update check (default: true for faster startup)
    pub skip_update_check: bool,
    /// Skip VPS verification (useful for offline development)
    pub skip_vps_check: bool,
    /// Skip health check loop (useful for testing)
    pub skip_health_check: bool,
    /// Enable debug server
    pub enable_debug: bool,
    /// Debug server port (default: 3030)
    pub debug_port: u16,
    /// Dev mode - skips auth and uses local conductor (set via SPOQ_DEV=1)
    pub dev_mode: bool,
    /// Override conductor URL (used in dev mode, defaults to http://localhost:8000)
    pub dev_conductor_url: Option<String>,
}

impl Default for StartupConfig {
    fn default() -> Self {
        Self {
            skip_update_check: true,
            skip_vps_check: false,
            skip_health_check: false,
            enable_debug: true,
            debug_port: 3030,
            dev_mode: false,
            dev_conductor_url: None,
        }
    }
}

impl StartupConfig {
    /// Create a new StartupConfig with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set whether to skip update check.
    pub fn with_skip_update_check(mut self, skip: bool) -> Self {
        self.skip_update_check = skip;
        self
    }

    /// Set whether to skip VPS check.
    pub fn with_skip_vps_check(mut self, skip: bool) -> Self {
        self.skip_vps_check = skip;
        self
    }

    /// Set whether to skip health check.
    pub fn with_skip_health_check(mut self, skip: bool) -> Self {
        self.skip_health_check = skip;
        self
    }

    /// Set whether to enable debug server.
    pub fn with_enable_debug(mut self, enable: bool) -> Self {
        self.enable_debug = enable;
        self
    }

    /// Set the debug server port.
    pub fn with_debug_port(mut self, port: u16) -> Self {
        self.debug_port = port;
        self
    }

    /// Enable dev mode (skips auth, uses local conductor).
    pub fn with_dev_mode(mut self, dev_mode: bool) -> Self {
        self.dev_mode = dev_mode;
        self
    }

    /// Set the dev conductor URL (defaults to http://localhost:8000).
    pub fn with_dev_conductor_url(mut self, url: impl Into<String>) -> Self {
        self.dev_conductor_url = Some(url.into());
        self
    }

    /// Create config from environment variable SPOQ_DEV.
    /// When SPOQ_DEV=1, enables dev mode with localhost:8000.
    pub fn from_env() -> Self {
        let dev_mode = std::env::var("SPOQ_DEV").is_ok();

        if dev_mode {
            Self::default()
                .with_dev_mode(true)
                .with_skip_vps_check(true)
                .with_skip_health_check(true)
                .with_dev_conductor_url("http://localhost:8000")
        } else {
            Self::default()
        }
    }
}

/// Result of successful preflight checks.
///
/// Contains all the resources needed to start the TUI.
pub struct StartupResult {
    /// Validated credentials (authenticated and not expired)
    pub credentials: Credentials,
    /// VPS state from API (if available)
    pub vps_state: Option<VpsStatusResponse>,
    /// VPS URL for connecting (if VPS exists)
    pub vps_url: Option<String>,
    /// Debug event sender (if debug is enabled)
    pub debug_tx: Option<DebugEventSender>,
    /// Debug server handle (if debug is enabled)
    pub debug_server_handle: Option<JoinHandle<()>>,
    /// State snapshot for debug server
    pub debug_state_snapshot: Option<Arc<RwLock<crate::debug::StateSnapshot>>>,
}

impl StartupResult {
    /// Create a new StartupResult.
    pub fn new(credentials: Credentials) -> Self {
        Self {
            credentials,
            vps_state: None,
            vps_url: None,
            debug_tx: None,
            debug_server_handle: None,
            debug_state_snapshot: None,
        }
    }

    /// Set VPS state.
    pub fn with_vps_state(mut self, vps: Option<VpsStatusResponse>) -> Self {
        self.vps_state = vps;
        self
    }

    /// Set VPS URL.
    pub fn with_vps_url(mut self, url: Option<String>) -> Self {
        self.vps_url = url;
        self
    }

    /// Set debug components.
    pub fn with_debug(
        mut self,
        tx: Option<DebugEventSender>,
        handle: Option<JoinHandle<()>>,
        snapshot: Option<Arc<RwLock<crate::debug::StateSnapshot>>>,
    ) -> Self {
        self.debug_tx = tx;
        self.debug_server_handle = handle;
        self.debug_state_snapshot = snapshot;
        self
    }

    /// Build VPS URL from VpsStatusResponse.
    pub fn build_vps_url(vps: &VpsStatusResponse) -> Option<String> {
        vps.hostname
            .as_ref()
            .map(|h| format!("https://{}", h))
            .or_else(|| vps.url.clone())
            .or_else(|| vps.ip.as_ref().map(|ip| format!("http://{}:8000", ip)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_startup_config_default() {
        let config = StartupConfig::default();
        assert!(config.skip_update_check);
        assert!(!config.skip_vps_check);
        assert!(!config.skip_health_check);
        assert!(config.enable_debug);
        assert_eq!(config.debug_port, 3030);
        assert!(!config.dev_mode);
        assert!(config.dev_conductor_url.is_none());
    }

    #[test]
    fn test_startup_config_builder() {
        let config = StartupConfig::new()
            .with_skip_update_check(false)
            .with_skip_vps_check(true)
            .with_skip_health_check(true)
            .with_enable_debug(false)
            .with_debug_port(4040)
            .with_dev_mode(true)
            .with_dev_conductor_url("http://localhost:9000");

        assert!(!config.skip_update_check);
        assert!(config.skip_vps_check);
        assert!(config.skip_health_check);
        assert!(!config.enable_debug);
        assert_eq!(config.debug_port, 4040);
        assert!(config.dev_mode);
        assert_eq!(
            config.dev_conductor_url,
            Some("http://localhost:9000".to_string())
        );
    }

    #[test]
    fn test_startup_config_dev_mode_settings() {
        // Test that dev mode configures all the right flags
        let config = StartupConfig::default()
            .with_dev_mode(true)
            .with_skip_vps_check(true)
            .with_skip_health_check(true)
            .with_dev_conductor_url("http://localhost:8000");

        assert!(config.dev_mode);
        assert!(config.skip_vps_check);
        assert!(config.skip_health_check);
        assert_eq!(
            config.dev_conductor_url,
            Some("http://localhost:8000".to_string())
        );
    }

    #[test]
    fn test_startup_result_new() {
        let creds = Credentials::default();
        let result = StartupResult::new(creds);

        assert!(result.vps_state.is_none());
        assert!(result.vps_url.is_none());
        assert!(result.debug_tx.is_none());
    }

    #[test]
    fn test_startup_result_builder() {
        let creds = Credentials::default();
        let vps = VpsStatusResponse {
            vps_id: "test-vps".to_string(),
            status: "ready".to_string(),
            hostname: Some("test.spoq.dev".to_string()),
            ip: Some("1.2.3.4".to_string()),
            url: None,
            ssh_username: None,
            provider: None,
            plan_id: None,
            data_center_id: None,
            created_at: None,
            ready_at: None,
        };

        let result = StartupResult::new(creds)
            .with_vps_state(Some(vps.clone()))
            .with_vps_url(StartupResult::build_vps_url(&vps));

        assert!(result.vps_state.is_some());
        assert_eq!(result.vps_url, Some("https://test.spoq.dev".to_string()));
    }

    #[test]
    fn test_build_vps_url_hostname() {
        let vps = VpsStatusResponse {
            vps_id: "test".to_string(),
            status: "ready".to_string(),
            hostname: Some("test.spoq.dev".to_string()),
            ip: Some("1.2.3.4".to_string()),
            url: Some("http://custom.url".to_string()),
            ssh_username: None,
            provider: None,
            plan_id: None,
            data_center_id: None,
            created_at: None,
            ready_at: None,
        };

        // Hostname takes priority
        assert_eq!(
            StartupResult::build_vps_url(&vps),
            Some("https://test.spoq.dev".to_string())
        );
    }

    #[test]
    fn test_build_vps_url_url_fallback() {
        let vps = VpsStatusResponse {
            vps_id: "test".to_string(),
            status: "ready".to_string(),
            hostname: None,
            ip: Some("1.2.3.4".to_string()),
            url: Some("http://custom.url".to_string()),
            ssh_username: None,
            provider: None,
            plan_id: None,
            data_center_id: None,
            created_at: None,
            ready_at: None,
        };

        // url takes second priority
        assert_eq!(
            StartupResult::build_vps_url(&vps),
            Some("http://custom.url".to_string())
        );
    }

    #[test]
    fn test_build_vps_url_ip_fallback() {
        let vps = VpsStatusResponse {
            vps_id: "test".to_string(),
            status: "ready".to_string(),
            hostname: None,
            ip: Some("1.2.3.4".to_string()),
            url: None,
            ssh_username: None,
            provider: None,
            plan_id: None,
            data_center_id: None,
            created_at: None,
            ready_at: None,
        };

        // IP is last fallback
        assert_eq!(
            StartupResult::build_vps_url(&vps),
            Some("http://1.2.3.4:8000".to_string())
        );
    }

    #[test]
    fn test_build_vps_url_none() {
        let vps = VpsStatusResponse {
            vps_id: "test".to_string(),
            status: "ready".to_string(),
            hostname: None,
            ip: None,
            url: None,
            ssh_username: None,
            provider: None,
            plan_id: None,
            data_center_id: None,
            created_at: None,
            ready_at: None,
        };

        assert_eq!(StartupResult::build_vps_url(&vps), None);
    }
}
