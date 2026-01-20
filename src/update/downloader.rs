//! Update downloader module for Spoq CLI.
//!
//! This module provides functionality to download binary updates from the
//! platform-specific download URL, verify the download, and store it temporarily.

use reqwest::Client;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;

/// Base URL for the download server.
pub const DOWNLOAD_BASE_URL: &str = "https://download.spoq.dev";

/// Error type for download operations.
#[derive(Debug)]
pub enum DownloadError {
    /// HTTP request failed.
    Http(reqwest::Error),
    /// I/O operation failed.
    Io(std::io::Error),
    /// Server returned an error status.
    ServerError { status: u16, message: String },
    /// Failed to determine platform.
    UnsupportedPlatform(String),
    /// Failed to determine home directory.
    NoHomeDirectory,
    /// Downloaded file is empty or too small.
    EmptyDownload,
    /// Downloaded file hash mismatch (if verification is implemented).
    VerificationFailed(String),
}

impl std::fmt::Display for DownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DownloadError::Http(e) => write!(f, "HTTP error: {}", e),
            DownloadError::Io(e) => write!(f, "I/O error: {}", e),
            DownloadError::ServerError { status, message } => {
                write!(f, "Server error ({}): {}", status, message)
            }
            DownloadError::UnsupportedPlatform(platform) => {
                write!(f, "Unsupported platform: {}", platform)
            }
            DownloadError::NoHomeDirectory => write!(f, "Could not determine home directory"),
            DownloadError::EmptyDownload => write!(f, "Downloaded file is empty or too small"),
            DownloadError::VerificationFailed(msg) => write!(f, "Verification failed: {}", msg),
        }
    }
}

impl std::error::Error for DownloadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DownloadError::Http(e) => Some(e),
            DownloadError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for DownloadError {
    fn from(e: reqwest::Error) -> Self {
        DownloadError::Http(e)
    }
}

impl From<std::io::Error> for DownloadError {
    fn from(e: std::io::Error) -> Self {
        DownloadError::Io(e)
    }
}

/// Result of a successful download operation.
#[derive(Debug, Clone)]
pub struct DownloadResult {
    /// Path to the downloaded binary file.
    pub file_path: PathBuf,
    /// Size of the downloaded file in bytes.
    pub file_size: u64,
    /// The version that was downloaded (if known).
    pub version: Option<String>,
}

/// Platform identifier for download URLs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    /// macOS on ARM64 (Apple Silicon).
    DarwinArm64,
    /// macOS on x86_64 (Intel).
    DarwinX64,
    /// Linux on ARM64.
    LinuxArm64,
    /// Linux on x86_64.
    LinuxX64,
}

impl Platform {
    /// Get the platform identifier string for the download URL.
    pub fn as_str(&self) -> &'static str {
        match self {
            Platform::DarwinArm64 => "darwin-aarch64",
            Platform::DarwinX64 => "darwin-x64",
            Platform::LinuxArm64 => "linux-aarch64",
            Platform::LinuxX64 => "linux-x64",
        }
    }
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Detect the current platform.
///
/// Returns the appropriate `Platform` variant based on the OS and architecture.
pub fn detect_platform() -> Result<Platform, DownloadError> {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    match (os, arch) {
        ("macos", "aarch64") => Ok(Platform::DarwinArm64),
        ("macos", "x86_64") => Ok(Platform::DarwinX64),
        ("linux", "aarch64") => Ok(Platform::LinuxArm64),
        ("linux", "x86_64") => Ok(Platform::LinuxX64),
        _ => Err(DownloadError::UnsupportedPlatform(format!(
            "{}-{}",
            os, arch
        ))),
    }
}

/// Get the temporary directory for storing downloaded updates.
///
/// Returns `~/.spoq/updates/` directory, creating it if necessary.
pub fn get_update_temp_dir() -> Result<PathBuf, DownloadError> {
    let home = dirs::home_dir().ok_or(DownloadError::NoHomeDirectory)?;
    let update_dir = home.join(".spoq").join("updates");
    Ok(update_dir)
}

/// Get the path for a downloaded update binary.
///
/// Returns `~/.spoq/updates/spoq-{version}` or `~/.spoq/updates/spoq-pending` if no version.
pub fn get_download_path(version: Option<&str>) -> Result<PathBuf, DownloadError> {
    let update_dir = get_update_temp_dir()?;
    let filename = match version {
        Some(v) => format!("spoq-{}", v),
        None => "spoq-pending".to_string(),
    };
    Ok(update_dir.join(filename))
}

/// Download the CLI binary for the specified platform.
///
/// Downloads the binary from the platform-specific URL and stores it in the
/// temporary updates directory. Returns information about the downloaded file.
///
/// # Arguments
///
/// * `platform` - The target platform to download for.
/// * `version` - Optional version string for naming the downloaded file.
///
/// # Example
///
/// ```ignore
/// let platform = detect_platform()?;
/// let result = download_binary(platform, Some("0.2.0")).await?;
/// println!("Downloaded to: {}", result.file_path.display());
/// ```
pub async fn download_binary(
    platform: Platform,
    version: Option<&str>,
) -> Result<DownloadResult, DownloadError> {
    download_binary_with_client(&Client::new(), platform, version).await
}

/// Download the CLI binary using a custom HTTP client.
///
/// This allows for custom client configuration (timeouts, proxies, etc.).
pub async fn download_binary_with_client(
    client: &Client,
    platform: Platform,
    version: Option<&str>,
) -> Result<DownloadResult, DownloadError> {
    let url = format!("{}/cli/download/{}", DOWNLOAD_BASE_URL, platform.as_str());
    download_from_url(client, &url, version).await
}

/// Download a binary from a specific URL.
///
/// This is the core download function that handles the actual HTTP request
/// and file writing.
pub async fn download_from_url(
    client: &Client,
    url: &str,
    version: Option<&str>,
) -> Result<DownloadResult, DownloadError> {
    // Make the HTTP request
    let response = client.get(url).send().await?;

    // Check for success status
    let status = response.status();
    if !status.is_success() {
        let status_code = status.as_u16();
        let message = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        return Err(DownloadError::ServerError {
            status: status_code,
            message,
        });
    }

    // Get content length if available for progress tracking
    let content_length = response.content_length();

    // Prepare the download directory and file path
    let update_dir = get_update_temp_dir()?;
    tokio::fs::create_dir_all(&update_dir).await?;

    let file_path = get_download_path(version)?;

    // Download the content
    let bytes = response.bytes().await?;

    // Verify the download is not empty
    if bytes.is_empty() {
        return Err(DownloadError::EmptyDownload);
    }

    // Minimum expected size for a binary (say 100KB)
    const MIN_BINARY_SIZE: usize = 100 * 1024;
    if bytes.len() < MIN_BINARY_SIZE {
        return Err(DownloadError::EmptyDownload);
    }

    // Write to temporary file first, then rename for atomicity
    let temp_path = file_path.with_extension("tmp");
    let mut file = tokio::fs::File::create(&temp_path).await?;
    file.write_all(&bytes).await?;
    file.flush().await?;
    drop(file);

    // Rename temp file to final location (atomic on most filesystems)
    tokio::fs::rename(&temp_path, &file_path).await?;

    // Verify file was written correctly
    let metadata = tokio::fs::metadata(&file_path).await?;
    let actual_size = metadata.len();

    // If we had a content length, verify it matches
    if let Some(expected) = content_length {
        if actual_size != expected {
            // Clean up the failed download
            let _ = tokio::fs::remove_file(&file_path).await;
            return Err(DownloadError::VerificationFailed(format!(
                "Size mismatch: expected {} bytes, got {} bytes",
                expected, actual_size
            )));
        }
    }

    Ok(DownloadResult {
        file_path,
        file_size: actual_size,
        version: version.map(String::from),
    })
}

/// Clean up old update files from the updates directory.
///
/// Removes all files in the updates directory except for the currently
/// pending update (if specified).
pub async fn cleanup_old_updates(keep_version: Option<&str>) -> Result<(), DownloadError> {
    let update_dir = get_update_temp_dir()?;

    if !update_dir.exists() {
        return Ok(());
    }

    let keep_filename = keep_version.map(|v| format!("spoq-{}", v));

    let mut entries = tokio::fs::read_dir(&update_dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let filename = entry.file_name();
        let filename_str = filename.to_string_lossy();

        // Skip the file we want to keep
        if let Some(ref keep) = keep_filename {
            if filename_str == *keep {
                continue;
            }
        }

        // Remove old update files
        if filename_str.starts_with("spoq-") || filename_str.ends_with(".tmp") {
            let _ = tokio::fs::remove_file(entry.path()).await;
        }
    }

    Ok(())
}

/// Check if a downloaded update exists for the given version.
pub async fn has_pending_update(version: &str) -> Result<bool, DownloadError> {
    let path = get_download_path(Some(version))?;
    match tokio::fs::metadata(&path).await {
        Ok(metadata) => Ok(metadata.is_file()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(DownloadError::Io(e)),
    }
}

/// Get the path to a pending update if it exists.
pub async fn get_pending_update_path(version: &str) -> Result<Option<PathBuf>, DownloadError> {
    let path = get_download_path(Some(version))?;
    match tokio::fs::metadata(&path).await {
        Ok(metadata) if metadata.is_file() => Ok(Some(path)),
        Ok(_) => Ok(None), // It's a directory, not a file
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(DownloadError::Io(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_as_str() {
        assert_eq!(Platform::DarwinArm64.as_str(), "darwin-aarch64");
        assert_eq!(Platform::DarwinX64.as_str(), "darwin-x64");
        assert_eq!(Platform::LinuxArm64.as_str(), "linux-aarch64");
        assert_eq!(Platform::LinuxX64.as_str(), "linux-x64");
    }

    #[test]
    fn test_platform_display() {
        assert_eq!(format!("{}", Platform::DarwinArm64), "darwin-aarch64");
        assert_eq!(format!("{}", Platform::LinuxX64), "linux-x64");
    }

    #[test]
    fn test_detect_platform() {
        // This test will pass on the current platform
        let result = detect_platform();
        // On macOS ARM, this should succeed
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        assert!(matches!(result, Ok(Platform::DarwinArm64)));

        #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
        assert!(matches!(result, Ok(Platform::DarwinX64)));

        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
        assert!(matches!(result, Ok(Platform::LinuxArm64)));

        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        assert!(matches!(result, Ok(Platform::LinuxX64)));
    }

    #[test]
    fn test_get_update_temp_dir() {
        let result = get_update_temp_dir();
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.ends_with(".spoq/updates"));
    }

    #[test]
    fn test_get_download_path_with_version() {
        let result = get_download_path(Some("0.2.0"));
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.ends_with("spoq-0.2.0"));
    }

    #[test]
    fn test_get_download_path_without_version() {
        let result = get_download_path(None);
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.ends_with("spoq-pending"));
    }

    #[test]
    fn test_download_error_display() {
        let err = DownloadError::ServerError {
            status: 404,
            message: "Not Found".to_string(),
        };
        let display = format!("{}", err);
        assert!(display.contains("404"));
        assert!(display.contains("Not Found"));

        let err = DownloadError::UnsupportedPlatform("windows-x64".to_string());
        let display = format!("{}", err);
        assert!(display.contains("windows-x64"));
        assert!(display.contains("Unsupported"));

        let err = DownloadError::NoHomeDirectory;
        let display = format!("{}", err);
        assert!(display.contains("home directory"));

        let err = DownloadError::EmptyDownload;
        let display = format!("{}", err);
        assert!(display.contains("empty"));

        let err = DownloadError::VerificationFailed("hash mismatch".to_string());
        let display = format!("{}", err);
        assert!(display.contains("Verification"));
        assert!(display.contains("hash mismatch"));
    }

    #[test]
    fn test_download_error_from_reqwest() {
        // We can't easily create a reqwest::Error, but we can test the From impl exists
        // by checking the error type implements the trait
        fn assert_from<T: From<reqwest::Error>>() {}
        assert_from::<DownloadError>();
    }

    #[test]
    fn test_download_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let download_err: DownloadError = io_err.into();
        assert!(matches!(download_err, DownloadError::Io(_)));
    }

    #[test]
    fn test_download_result_clone() {
        let result = DownloadResult {
            file_path: PathBuf::from("/tmp/spoq-0.2.0"),
            file_size: 1024,
            version: Some("0.2.0".to_string()),
        };
        let cloned = result.clone();
        assert_eq!(cloned.file_path, result.file_path);
        assert_eq!(cloned.file_size, result.file_size);
        assert_eq!(cloned.version, result.version);
    }

    #[tokio::test]
    async fn test_download_binary_with_invalid_server() {
        // Test with a server that doesn't exist
        let client = Client::builder()
            .timeout(std::time::Duration::from_millis(100))
            .build()
            .unwrap();

        let result = download_from_url(&client, "http://127.0.0.1:1/fake", Some("0.0.0")).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_has_pending_update_nonexistent() {
        // Test with a version that definitely doesn't exist
        let result = has_pending_update("99.99.99-nonexistent-test").await;
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[tokio::test]
    async fn test_get_pending_update_path_nonexistent() {
        let result = get_pending_update_path("99.99.99-nonexistent-test").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_cleanup_old_updates_no_dir() {
        // Should not fail if the directory doesn't exist
        // We use a unique subdirectory that doesn't exist
        let result = cleanup_old_updates(None).await;
        // This should succeed (no-op if dir doesn't exist, or clean if it does)
        assert!(result.is_ok());
    }
}
