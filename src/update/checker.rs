//! Update checker for Spoq CLI.
//!
//! This module calls the version API endpoint and compares the remote version
//! with the current version using semantic versioning comparison.

use reqwest::Client;
use serde::Deserialize;
use std::cmp::Ordering;

/// URL for the version API endpoint
pub const VERSION_API_URL: &str = "https://download.spoq.dev/cli/version";

/// Current CLI version (from Cargo.toml)
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Error type for update check operations
#[derive(Debug)]
pub enum UpdateCheckError {
    /// HTTP request failed
    Http(reqwest::Error),
    /// JSON deserialization failed
    Json(serde_json::Error),
    /// Server returned an error status
    ServerError { status: u16, message: String },
    /// Invalid version format
    InvalidVersion(String),
}

impl std::fmt::Display for UpdateCheckError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UpdateCheckError::Http(e) => write!(f, "HTTP error: {}", e),
            UpdateCheckError::Json(e) => write!(f, "JSON error: {}", e),
            UpdateCheckError::ServerError { status, message } => {
                write!(f, "Server error ({}): {}", status, message)
            }
            UpdateCheckError::InvalidVersion(v) => write!(f, "Invalid version format: {}", v),
        }
    }
}

impl std::error::Error for UpdateCheckError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            UpdateCheckError::Http(e) => Some(e),
            UpdateCheckError::Json(e) => Some(e),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for UpdateCheckError {
    fn from(e: reqwest::Error) -> Self {
        UpdateCheckError::Http(e)
    }
}

impl From<serde_json::Error> for UpdateCheckError {
    fn from(e: serde_json::Error) -> Self {
        UpdateCheckError::Json(e)
    }
}

/// Response from the version API endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct VersionInfo {
    /// Latest version available
    pub version: String,
    /// Download URL for the latest version (optional)
    #[serde(default)]
    pub download_url: Option<String>,
    /// Release notes or changelog (optional)
    #[serde(default)]
    pub release_notes: Option<String>,
    /// Whether this is a mandatory update (optional)
    #[serde(default)]
    pub mandatory: Option<bool>,
}

/// Result of checking for updates.
#[derive(Debug, Clone)]
pub struct UpdateCheckResult {
    /// Current version of the CLI
    pub current_version: String,
    /// Latest version available
    pub latest_version: String,
    /// Whether an update is available
    pub update_available: bool,
    /// Download URL (if provided by API)
    pub download_url: Option<String>,
    /// Release notes (if provided by API)
    pub release_notes: Option<String>,
    /// Whether this is a mandatory update
    pub mandatory: bool,
}

/// Parsed semantic version for comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
struct SemVer {
    major: u32,
    minor: u32,
    patch: u32,
    prerelease: Option<String>,
}

impl SemVer {
    /// Parse a version string into SemVer components.
    fn parse(version: &str) -> Result<Self, UpdateCheckError> {
        // Remove leading 'v' if present
        let version = version.strip_prefix('v').unwrap_or(version);

        // Split by '-' to separate prerelease
        let (version_part, prerelease) = match version.split_once('-') {
            Some((v, pre)) => (v, Some(pre.to_string())),
            None => (version, None),
        };

        // Split by '.' to get major.minor.patch
        let parts: Vec<&str> = version_part.split('.').collect();

        if parts.len() < 2 || parts.len() > 3 {
            return Err(UpdateCheckError::InvalidVersion(version.to_string()));
        }

        let major = parts[0]
            .parse::<u32>()
            .map_err(|_| UpdateCheckError::InvalidVersion(version.to_string()))?;

        let minor = parts[1]
            .parse::<u32>()
            .map_err(|_| UpdateCheckError::InvalidVersion(version.to_string()))?;

        let patch = if parts.len() == 3 {
            parts[2]
                .parse::<u32>()
                .map_err(|_| UpdateCheckError::InvalidVersion(version.to_string()))?
        } else {
            0
        };

        Ok(Self {
            major,
            minor,
            patch,
            prerelease,
        })
    }
}

impl PartialOrd for SemVer {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SemVer {
    fn cmp(&self, other: &Self) -> Ordering {
        // Compare major, minor, patch
        match self.major.cmp(&other.major) {
            Ordering::Equal => {}
            ord => return ord,
        }
        match self.minor.cmp(&other.minor) {
            Ordering::Equal => {}
            ord => return ord,
        }
        match self.patch.cmp(&other.patch) {
            Ordering::Equal => {}
            ord => return ord,
        }

        // Prerelease versions have lower precedence than release versions
        // e.g., 1.0.0-alpha < 1.0.0
        match (&self.prerelease, &other.prerelease) {
            (None, None) => Ordering::Equal,
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (Some(a), Some(b)) => a.cmp(b),
        }
    }
}

/// Compare two version strings using semantic versioning.
///
/// Returns:
/// - `Ordering::Less` if `current` < `latest`
/// - `Ordering::Equal` if `current` == `latest`
/// - `Ordering::Greater` if `current` > `latest`
pub fn compare_versions(current: &str, latest: &str) -> Result<Ordering, UpdateCheckError> {
    let current_ver = SemVer::parse(current)?;
    let latest_ver = SemVer::parse(latest)?;
    Ok(current_ver.cmp(&latest_ver))
}

/// Check for available updates.
///
/// Calls the version API endpoint and compares the remote version
/// with the current version.
pub async fn check_for_update() -> Result<UpdateCheckResult, UpdateCheckError> {
    check_for_update_with_url(VERSION_API_URL).await
}

/// Check for available updates using a custom URL (for testing).
pub async fn check_for_update_with_url(url: &str) -> Result<UpdateCheckResult, UpdateCheckError> {
    let client = Client::new();

    let response = client.get(url).send().await?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(UpdateCheckError::ServerError {
            status,
            message: body,
        });
    }

    let version_info: VersionInfo = response.json().await?;

    let comparison = compare_versions(CURRENT_VERSION, &version_info.version)?;

    Ok(UpdateCheckResult {
        current_version: CURRENT_VERSION.to_string(),
        latest_version: version_info.version,
        update_available: comparison == Ordering::Less,
        download_url: version_info.download_url,
        release_notes: version_info.release_notes,
        mandatory: version_info.mandatory.unwrap_or(false),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // SemVer parsing tests

    #[test]
    fn test_semver_parse_basic() {
        let ver = SemVer::parse("1.2.3").unwrap();
        assert_eq!(ver.major, 1);
        assert_eq!(ver.minor, 2);
        assert_eq!(ver.patch, 3);
        assert!(ver.prerelease.is_none());
    }

    #[test]
    fn test_semver_parse_with_v_prefix() {
        let ver = SemVer::parse("v1.2.3").unwrap();
        assert_eq!(ver.major, 1);
        assert_eq!(ver.minor, 2);
        assert_eq!(ver.patch, 3);
    }

    #[test]
    fn test_semver_parse_two_parts() {
        let ver = SemVer::parse("1.2").unwrap();
        assert_eq!(ver.major, 1);
        assert_eq!(ver.minor, 2);
        assert_eq!(ver.patch, 0);
    }

    #[test]
    fn test_semver_parse_with_prerelease() {
        let ver = SemVer::parse("1.2.3-alpha.1").unwrap();
        assert_eq!(ver.major, 1);
        assert_eq!(ver.minor, 2);
        assert_eq!(ver.patch, 3);
        assert_eq!(ver.prerelease, Some("alpha.1".to_string()));
    }

    #[test]
    fn test_semver_parse_invalid_single_part() {
        assert!(SemVer::parse("1").is_err());
    }

    #[test]
    fn test_semver_parse_invalid_non_numeric() {
        assert!(SemVer::parse("1.x.3").is_err());
    }

    // Version comparison tests

    #[test]
    fn test_compare_versions_equal() {
        assert_eq!(
            compare_versions("1.0.0", "1.0.0").unwrap(),
            Ordering::Equal
        );
        assert_eq!(compare_versions("0.1.4", "0.1.4").unwrap(), Ordering::Equal);
    }

    #[test]
    fn test_compare_versions_major_diff() {
        assert_eq!(compare_versions("1.0.0", "2.0.0").unwrap(), Ordering::Less);
        assert_eq!(
            compare_versions("2.0.0", "1.0.0").unwrap(),
            Ordering::Greater
        );
    }

    #[test]
    fn test_compare_versions_minor_diff() {
        assert_eq!(compare_versions("1.0.0", "1.1.0").unwrap(), Ordering::Less);
        assert_eq!(
            compare_versions("1.2.0", "1.1.0").unwrap(),
            Ordering::Greater
        );
    }

    #[test]
    fn test_compare_versions_patch_diff() {
        assert_eq!(compare_versions("1.0.0", "1.0.1").unwrap(), Ordering::Less);
        assert_eq!(
            compare_versions("1.0.5", "1.0.3").unwrap(),
            Ordering::Greater
        );
    }

    #[test]
    fn test_compare_versions_prerelease() {
        // Prerelease < release
        assert_eq!(
            compare_versions("1.0.0-alpha", "1.0.0").unwrap(),
            Ordering::Less
        );
        assert_eq!(
            compare_versions("1.0.0", "1.0.0-alpha").unwrap(),
            Ordering::Greater
        );
    }

    #[test]
    fn test_compare_versions_prerelease_order() {
        // alpha < beta
        assert_eq!(
            compare_versions("1.0.0-alpha", "1.0.0-beta").unwrap(),
            Ordering::Less
        );
    }

    #[test]
    fn test_compare_versions_with_v_prefix() {
        assert_eq!(compare_versions("v1.0.0", "1.0.0").unwrap(), Ordering::Equal);
        assert_eq!(compare_versions("1.0.0", "v1.0.1").unwrap(), Ordering::Less);
    }

    #[test]
    fn test_compare_versions_two_part() {
        assert_eq!(compare_versions("1.2", "1.2.0").unwrap(), Ordering::Equal);
        assert_eq!(compare_versions("1.2", "1.2.1").unwrap(), Ordering::Less);
    }

    // Current version test

    #[test]
    fn test_current_version_is_valid() {
        assert!(SemVer::parse(CURRENT_VERSION).is_ok());
    }

    // VersionInfo deserialization tests

    #[test]
    fn test_version_info_deserialize_minimal() {
        let json = r#"{"version": "1.0.0"}"#;
        let info: VersionInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.version, "1.0.0");
        assert!(info.download_url.is_none());
        assert!(info.release_notes.is_none());
        assert!(info.mandatory.is_none());
    }

    #[test]
    fn test_version_info_deserialize_full() {
        let json = r#"{
            "version": "2.0.0",
            "download_url": "https://download.spoq.dev/cli/spoq-2.0.0",
            "release_notes": "New features and bug fixes",
            "mandatory": true
        }"#;
        let info: VersionInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.version, "2.0.0");
        assert_eq!(
            info.download_url,
            Some("https://download.spoq.dev/cli/spoq-2.0.0".to_string())
        );
        assert_eq!(
            info.release_notes,
            Some("New features and bug fixes".to_string())
        );
        assert_eq!(info.mandatory, Some(true));
    }

    // UpdateCheckResult tests

    #[test]
    fn test_update_check_result_update_available() {
        let result = UpdateCheckResult {
            current_version: "0.1.4".to_string(),
            latest_version: "0.2.0".to_string(),
            update_available: true,
            download_url: Some("https://download.spoq.dev/cli/spoq".to_string()),
            release_notes: None,
            mandatory: false,
        };
        assert!(result.update_available);
        assert_eq!(result.current_version, "0.1.4");
        assert_eq!(result.latest_version, "0.2.0");
    }

    #[test]
    fn test_update_check_result_no_update() {
        let result = UpdateCheckResult {
            current_version: "0.2.0".to_string(),
            latest_version: "0.2.0".to_string(),
            update_available: false,
            download_url: None,
            release_notes: None,
            mandatory: false,
        };
        assert!(!result.update_available);
    }

    // Error display tests

    #[test]
    fn test_update_check_error_display_server_error() {
        let err = UpdateCheckError::ServerError {
            status: 500,
            message: "Internal Server Error".to_string(),
        };
        let display = format!("{}", err);
        assert!(display.contains("500"));
        assert!(display.contains("Internal Server Error"));
    }

    #[test]
    fn test_update_check_error_display_invalid_version() {
        let err = UpdateCheckError::InvalidVersion("not-a-version".to_string());
        let display = format!("{}", err);
        assert!(display.contains("Invalid version"));
        assert!(display.contains("not-a-version"));
    }

    // Async tests with invalid server

    #[tokio::test]
    async fn test_check_for_update_with_invalid_server() {
        let result = check_for_update_with_url("http://127.0.0.1:1/version").await;
        assert!(result.is_err());
    }

    // Edge case tests

    #[test]
    fn test_semver_ordering() {
        // Test various orderings
        let v0_1_0 = SemVer::parse("0.1.0").unwrap();
        let v0_1_4 = SemVer::parse("0.1.4").unwrap();
        let v0_2_0 = SemVer::parse("0.2.0").unwrap();
        let v1_0_0 = SemVer::parse("1.0.0").unwrap();

        assert!(v0_1_0 < v0_1_4);
        assert!(v0_1_4 < v0_2_0);
        assert!(v0_2_0 < v1_0_0);
    }

    #[test]
    fn test_current_version_comparison() {
        // Ensure we can compare the current version
        let result = compare_versions(CURRENT_VERSION, "999.0.0");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Ordering::Less);
    }
}
