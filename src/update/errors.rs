//! Unified error handling for update operations.
//!
//! This module provides:
//! - Comprehensive error types for all update failure scenarios
//! - User-friendly error messages for display
//! - Error categorization for appropriate handling
//! - Conversion traits for underlying error types

use std::fmt;
use std::path::PathBuf;

/// Represents the category of an update error for handling decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateErrorCategory {
    /// Network-related errors (connection, DNS, timeout)
    Network,
    /// Server-side errors (HTTP 5xx, bad response)
    Server,
    /// Permission or access errors
    Permission,
    /// Disk space or filesystem errors
    DiskSpace,
    /// File system I/O errors (not permission or space related)
    FileSystem,
    /// Version or format errors
    Version,
    /// Platform compatibility errors
    Platform,
    /// Configuration errors
    Configuration,
    /// Verification failures (checksum, size mismatch)
    Verification,
}

impl UpdateErrorCategory {
    /// Returns true if this error category is likely transient and retryable.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            UpdateErrorCategory::Network | UpdateErrorCategory::Server
        )
    }

    /// Returns a short label for the category.
    pub fn as_str(&self) -> &'static str {
        match self {
            UpdateErrorCategory::Network => "network",
            UpdateErrorCategory::Server => "server",
            UpdateErrorCategory::Permission => "permission",
            UpdateErrorCategory::DiskSpace => "disk_space",
            UpdateErrorCategory::FileSystem => "filesystem",
            UpdateErrorCategory::Version => "version",
            UpdateErrorCategory::Platform => "platform",
            UpdateErrorCategory::Configuration => "configuration",
            UpdateErrorCategory::Verification => "verification",
        }
    }
}

impl fmt::Display for UpdateErrorCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Comprehensive error type for all update operations.
#[derive(Debug)]
pub enum UpdateError {
    // ========== Network Errors ==========
    /// Connection failed (network unreachable, connection refused)
    ConnectionFailed {
        url: String,
        message: String,
    },

    /// DNS resolution failed
    DnsResolutionFailed {
        host: String,
    },

    /// Request timed out
    Timeout {
        operation: String,
        duration_secs: u64,
    },

    /// TLS/SSL error
    TlsError {
        message: String,
    },

    // ========== Server Errors ==========
    /// Server returned an error status code
    ServerError {
        status: u16,
        message: String,
    },

    /// Server returned invalid or unexpected response
    InvalidResponse {
        message: String,
    },

    /// Rate limited by server
    RateLimited {
        retry_after_secs: Option<u64>,
    },

    // ========== Permission Errors ==========
    /// Insufficient permissions to read/write file
    PermissionDenied {
        path: PathBuf,
        operation: String,
    },

    /// Cannot modify the running executable
    ExecutableInUse {
        path: PathBuf,
    },

    /// Need elevated privileges (sudo/admin)
    ElevationRequired {
        path: PathBuf,
    },

    // ========== Disk Space Errors ==========
    /// Not enough disk space for download
    InsufficientDiskSpace {
        required_bytes: u64,
        available_bytes: u64,
        path: PathBuf,
    },

    /// Disk quota exceeded
    QuotaExceeded {
        path: PathBuf,
    },

    // ========== File System Errors ==========
    /// File not found
    FileNotFound {
        path: PathBuf,
    },

    /// Directory not found
    DirectoryNotFound {
        path: PathBuf,
    },

    /// Failed to create directory
    DirectoryCreationFailed {
        path: PathBuf,
        message: String,
    },

    /// Generic I/O error
    IoError {
        operation: String,
        path: Option<PathBuf>,
        message: String,
    },

    // ========== Version Errors ==========
    /// Invalid version format
    InvalidVersionFormat {
        version: String,
    },

    /// No update available (already on latest)
    AlreadyUpToDate {
        current_version: String,
    },

    // ========== Platform Errors ==========
    /// Unsupported operating system or architecture
    UnsupportedPlatform {
        os: String,
        arch: String,
    },

    /// Could not determine home directory
    NoHomeDirectory,

    /// Could not determine current executable path
    NoExecutablePath,

    // ========== Verification Errors ==========
    /// Downloaded file size doesn't match expected
    SizeMismatch {
        expected: u64,
        actual: u64,
    },

    /// Downloaded file is empty or too small
    EmptyDownload,

    /// Checksum verification failed
    ChecksumMismatch {
        expected: String,
        actual: String,
    },

    // ========== Installation Errors ==========
    /// Update file not found for installation
    UpdateFileNotFound {
        path: PathBuf,
    },

    /// Backup not found for rollback
    BackupNotFound {
        path: PathBuf,
    },

    /// Installation failed but backup was restored
    InstallFailedRestored {
        cause: String,
        restored_from: PathBuf,
    },

    /// Installation failed and rollback also failed (critical)
    InstallFailedNoRestore {
        install_error: String,
        restore_error: String,
    },
}

impl UpdateError {
    /// Get the category of this error.
    pub fn category(&self) -> UpdateErrorCategory {
        match self {
            // Network
            UpdateError::ConnectionFailed { .. }
            | UpdateError::DnsResolutionFailed { .. }
            | UpdateError::Timeout { .. }
            | UpdateError::TlsError { .. } => UpdateErrorCategory::Network,

            // Server
            UpdateError::ServerError { .. }
            | UpdateError::InvalidResponse { .. }
            | UpdateError::RateLimited { .. } => UpdateErrorCategory::Server,

            // Permission
            UpdateError::PermissionDenied { .. }
            | UpdateError::ExecutableInUse { .. }
            | UpdateError::ElevationRequired { .. } => UpdateErrorCategory::Permission,

            // Disk Space
            UpdateError::InsufficientDiskSpace { .. } | UpdateError::QuotaExceeded { .. } => {
                UpdateErrorCategory::DiskSpace
            }

            // File System
            UpdateError::FileNotFound { .. }
            | UpdateError::DirectoryNotFound { .. }
            | UpdateError::DirectoryCreationFailed { .. }
            | UpdateError::IoError { .. } => UpdateErrorCategory::FileSystem,

            // Version
            UpdateError::InvalidVersionFormat { .. } | UpdateError::AlreadyUpToDate { .. } => {
                UpdateErrorCategory::Version
            }

            // Platform
            UpdateError::UnsupportedPlatform { .. }
            | UpdateError::NoHomeDirectory
            | UpdateError::NoExecutablePath => UpdateErrorCategory::Platform,

            // Verification
            UpdateError::SizeMismatch { .. }
            | UpdateError::EmptyDownload
            | UpdateError::ChecksumMismatch { .. } => UpdateErrorCategory::Verification,

            // Installation (mixed categories, but primarily filesystem)
            UpdateError::UpdateFileNotFound { .. } | UpdateError::BackupNotFound { .. } => {
                UpdateErrorCategory::FileSystem
            }

            UpdateError::InstallFailedRestored { .. }
            | UpdateError::InstallFailedNoRestore { .. } => UpdateErrorCategory::FileSystem,
        }
    }

    /// Check if this error is likely transient and the operation can be retried.
    pub fn is_retryable(&self) -> bool {
        self.category().is_retryable()
    }

    /// Get a user-friendly error message suitable for display.
    pub fn user_message(&self) -> String {
        match self {
            // Network errors
            UpdateError::ConnectionFailed { .. } => {
                "Unable to connect to the update server. Please check your internet connection and try again.".to_string()
            }
            UpdateError::DnsResolutionFailed { host } => {
                format!("Could not resolve server address '{}'. Please check your internet connection or DNS settings.", host)
            }
            UpdateError::Timeout { operation, duration_secs } => {
                format!("The {} operation timed out after {} seconds. The server may be slow or unreachable.", operation, duration_secs)
            }
            UpdateError::TlsError { .. } => {
                "A secure connection could not be established. Please check your system's SSL/TLS configuration.".to_string()
            }

            // Server errors
            UpdateError::ServerError { status, .. } => {
                match *status {
                    404 => "The update was not found on the server. The version may not be available yet.".to_string(),
                    500..=599 => "The update server is experiencing issues. Please try again later.".to_string(),
                    _ => format!("The server returned an error (HTTP {}). Please try again later.", status),
                }
            }
            UpdateError::InvalidResponse { .. } => {
                "Received an invalid response from the update server. Please try again later.".to_string()
            }
            UpdateError::RateLimited { retry_after_secs } => {
                match retry_after_secs {
                    Some(secs) => format!("Too many update requests. Please wait {} seconds before trying again.", secs),
                    None => "Too many update requests. Please wait a moment before trying again.".to_string(),
                }
            }

            // Permission errors
            UpdateError::PermissionDenied { path, operation } => {
                format!(
                    "Permission denied while trying to {} '{}'.\nTry running with elevated privileges (sudo) or check file permissions.",
                    operation,
                    path.display()
                )
            }
            UpdateError::ExecutableInUse { path } => {
                format!(
                    "Cannot update '{}' because it is currently in use.\nPlease close any running instances and try again.",
                    path.display()
                )
            }
            UpdateError::ElevationRequired { path } => {
                format!(
                    "Administrator privileges are required to update '{}'.\nPlease run with sudo or as administrator.",
                    path.display()
                )
            }

            // Disk space errors
            UpdateError::InsufficientDiskSpace { required_bytes, available_bytes, path } => {
                let required_mb = *required_bytes as f64 / (1024.0 * 1024.0);
                let available_mb = *available_bytes as f64 / (1024.0 * 1024.0);
                format!(
                    "Not enough disk space for the update.\nRequired: {:.1} MB, Available: {:.1} MB\nPath: {}\nPlease free up some disk space and try again.",
                    required_mb, available_mb, path.display()
                )
            }
            UpdateError::QuotaExceeded { path } => {
                format!(
                    "Disk quota exceeded while writing to '{}'.\nPlease free up some quota or contact your system administrator.",
                    path.display()
                )
            }

            // File system errors
            UpdateError::FileNotFound { path } => {
                format!("File not found: '{}'", path.display())
            }
            UpdateError::DirectoryNotFound { path } => {
                format!("Directory not found: '{}'", path.display())
            }
            UpdateError::DirectoryCreationFailed { path, .. } => {
                format!("Failed to create directory: '{}'", path.display())
            }
            UpdateError::IoError { operation, path, .. } => {
                match path {
                    Some(p) => format!("Failed to {} file: '{}'", operation, p.display()),
                    None => format!("Failed to {}", operation),
                }
            }

            // Version errors
            UpdateError::InvalidVersionFormat { version } => {
                format!("Invalid version format: '{}'. Expected format: X.Y.Z", version)
            }
            UpdateError::AlreadyUpToDate { current_version } => {
                format!("You are already running the latest version ({}).", current_version)
            }

            // Platform errors
            UpdateError::UnsupportedPlatform { os, arch } => {
                format!("Unsupported platform: {}-{}. Updates are available for macOS and Linux on x64 and ARM64.", os, arch)
            }
            UpdateError::NoHomeDirectory => {
                "Could not determine your home directory. Please check your environment configuration.".to_string()
            }
            UpdateError::NoExecutablePath => {
                "Could not determine the current executable path. Please try specifying the target path manually.".to_string()
            }

            // Verification errors
            UpdateError::SizeMismatch { expected, actual } => {
                format!(
                    "Downloaded file size ({} bytes) doesn't match expected size ({} bytes).\nThe download may be corrupted. Please try again.",
                    actual, expected
                )
            }
            UpdateError::EmptyDownload => {
                "The downloaded update file is empty or too small.\nThe download may have been interrupted. Please try again.".to_string()
            }
            UpdateError::ChecksumMismatch { .. } => {
                "The downloaded file failed integrity verification.\nThe download may be corrupted. Please try again.".to_string()
            }

            // Installation errors
            UpdateError::UpdateFileNotFound { path } => {
                format!("Update file not found at: '{}'", path.display())
            }
            UpdateError::BackupNotFound { path } => {
                format!("Backup file not found at: '{}'. Cannot perform rollback.", path.display())
            }
            UpdateError::InstallFailedRestored { cause, restored_from } => {
                format!(
                    "Installation failed: {}\nThe previous version has been restored from backup: '{}'",
                    cause,
                    restored_from.display()
                )
            }
            UpdateError::InstallFailedNoRestore { install_error, restore_error } => {
                format!(
                    "CRITICAL: Installation failed ({}) and backup restoration also failed ({}).\nYour installation may be corrupted. Please reinstall manually.",
                    install_error, restore_error
                )
            }
        }
    }

    /// Get a short error code suitable for logging.
    pub fn error_code(&self) -> &'static str {
        match self {
            UpdateError::ConnectionFailed { .. } => "E_CONN_FAILED",
            UpdateError::DnsResolutionFailed { .. } => "E_DNS_FAILED",
            UpdateError::Timeout { .. } => "E_TIMEOUT",
            UpdateError::TlsError { .. } => "E_TLS",
            UpdateError::ServerError { .. } => "E_SERVER",
            UpdateError::InvalidResponse { .. } => "E_INVALID_RESPONSE",
            UpdateError::RateLimited { .. } => "E_RATE_LIMITED",
            UpdateError::PermissionDenied { .. } => "E_PERMISSION",
            UpdateError::ExecutableInUse { .. } => "E_EXE_IN_USE",
            UpdateError::ElevationRequired { .. } => "E_ELEVATION",
            UpdateError::InsufficientDiskSpace { .. } => "E_DISK_SPACE",
            UpdateError::QuotaExceeded { .. } => "E_QUOTA",
            UpdateError::FileNotFound { .. } => "E_FILE_NOT_FOUND",
            UpdateError::DirectoryNotFound { .. } => "E_DIR_NOT_FOUND",
            UpdateError::DirectoryCreationFailed { .. } => "E_DIR_CREATE",
            UpdateError::IoError { .. } => "E_IO",
            UpdateError::InvalidVersionFormat { .. } => "E_INVALID_VERSION",
            UpdateError::AlreadyUpToDate { .. } => "E_UP_TO_DATE",
            UpdateError::UnsupportedPlatform { .. } => "E_PLATFORM",
            UpdateError::NoHomeDirectory => "E_NO_HOME",
            UpdateError::NoExecutablePath => "E_NO_EXE_PATH",
            UpdateError::SizeMismatch { .. } => "E_SIZE_MISMATCH",
            UpdateError::EmptyDownload => "E_EMPTY_DOWNLOAD",
            UpdateError::ChecksumMismatch { .. } => "E_CHECKSUM",
            UpdateError::UpdateFileNotFound { .. } => "E_UPDATE_NOT_FOUND",
            UpdateError::BackupNotFound { .. } => "E_BACKUP_NOT_FOUND",
            UpdateError::InstallFailedRestored { .. } => "E_INSTALL_RESTORED",
            UpdateError::InstallFailedNoRestore { .. } => "E_INSTALL_CRITICAL",
        }
    }
}

impl fmt::Display for UpdateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UpdateError::ConnectionFailed { url, message } => {
                write!(f, "Connection failed to '{}': {}", url, message)
            }
            UpdateError::DnsResolutionFailed { host } => {
                write!(f, "DNS resolution failed for '{}'", host)
            }
            UpdateError::Timeout {
                operation,
                duration_secs,
            } => {
                write!(f, "{} timed out after {} seconds", operation, duration_secs)
            }
            UpdateError::TlsError { message } => {
                write!(f, "TLS error: {}", message)
            }
            UpdateError::ServerError { status, message } => {
                write!(f, "Server error (HTTP {}): {}", status, message)
            }
            UpdateError::InvalidResponse { message } => {
                write!(f, "Invalid response: {}", message)
            }
            UpdateError::RateLimited { retry_after_secs } => match retry_after_secs {
                Some(secs) => write!(f, "Rate limited, retry after {} seconds", secs),
                None => write!(f, "Rate limited"),
            },
            UpdateError::PermissionDenied { path, operation } => {
                write!(
                    f,
                    "Permission denied: {} '{}'",
                    operation,
                    path.display()
                )
            }
            UpdateError::ExecutableInUse { path } => {
                write!(f, "Executable in use: '{}'", path.display())
            }
            UpdateError::ElevationRequired { path } => {
                write!(f, "Elevation required for: '{}'", path.display())
            }
            UpdateError::InsufficientDiskSpace {
                required_bytes,
                available_bytes,
                path,
            } => {
                write!(
                    f,
                    "Insufficient disk space at '{}': need {} bytes, have {} bytes",
                    path.display(),
                    required_bytes,
                    available_bytes
                )
            }
            UpdateError::QuotaExceeded { path } => {
                write!(f, "Disk quota exceeded at '{}'", path.display())
            }
            UpdateError::FileNotFound { path } => {
                write!(f, "File not found: '{}'", path.display())
            }
            UpdateError::DirectoryNotFound { path } => {
                write!(f, "Directory not found: '{}'", path.display())
            }
            UpdateError::DirectoryCreationFailed { path, message } => {
                write!(f, "Failed to create directory '{}': {}", path.display(), message)
            }
            UpdateError::IoError {
                operation,
                path,
                message,
            } => match path {
                Some(p) => write!(f, "I/O error during {} at '{}': {}", operation, p.display(), message),
                None => write!(f, "I/O error during {}: {}", operation, message),
            },
            UpdateError::InvalidVersionFormat { version } => {
                write!(f, "Invalid version format: '{}'", version)
            }
            UpdateError::AlreadyUpToDate { current_version } => {
                write!(f, "Already up to date (version {})", current_version)
            }
            UpdateError::UnsupportedPlatform { os, arch } => {
                write!(f, "Unsupported platform: {}-{}", os, arch)
            }
            UpdateError::NoHomeDirectory => {
                write!(f, "Could not determine home directory")
            }
            UpdateError::NoExecutablePath => {
                write!(f, "Could not determine current executable path")
            }
            UpdateError::SizeMismatch { expected, actual } => {
                write!(
                    f,
                    "Size mismatch: expected {} bytes, got {} bytes",
                    expected, actual
                )
            }
            UpdateError::EmptyDownload => {
                write!(f, "Downloaded file is empty or too small")
            }
            UpdateError::ChecksumMismatch { expected, actual } => {
                write!(
                    f,
                    "Checksum mismatch: expected '{}', got '{}'",
                    expected, actual
                )
            }
            UpdateError::UpdateFileNotFound { path } => {
                write!(f, "Update file not found: '{}'", path.display())
            }
            UpdateError::BackupNotFound { path } => {
                write!(f, "Backup not found: '{}'", path.display())
            }
            UpdateError::InstallFailedRestored {
                cause,
                restored_from,
            } => {
                write!(
                    f,
                    "Installation failed ({}), restored from '{}'",
                    cause,
                    restored_from.display()
                )
            }
            UpdateError::InstallFailedNoRestore {
                install_error,
                restore_error,
            } => {
                write!(
                    f,
                    "Installation failed ({}) and restore failed ({})",
                    install_error, restore_error
                )
            }
        }
    }
}

impl std::error::Error for UpdateError {}

/// Helper function to classify an I/O error into a more specific UpdateError.
pub fn classify_io_error(err: std::io::Error, path: Option<PathBuf>, operation: &str) -> UpdateError {
    use std::io::ErrorKind;

    match err.kind() {
        ErrorKind::NotFound => match &path {
            Some(p) => {
                if p.is_dir() || operation.contains("dir") {
                    UpdateError::DirectoryNotFound { path: p.clone() }
                } else {
                    UpdateError::FileNotFound { path: p.clone() }
                }
            }
            None => UpdateError::IoError {
                operation: operation.to_string(),
                path: None,
                message: err.to_string(),
            },
        },
        ErrorKind::PermissionDenied => match &path {
            Some(p) => UpdateError::PermissionDenied {
                path: p.clone(),
                operation: operation.to_string(),
            },
            None => UpdateError::IoError {
                operation: operation.to_string(),
                path: None,
                message: "Permission denied".to_string(),
            },
        },
        // StorageFull is unstable, so we check the raw OS error on Unix
        _ if is_disk_space_error(&err) => match &path {
            Some(p) => UpdateError::InsufficientDiskSpace {
                required_bytes: 0, // Unknown
                available_bytes: 0,
                path: p.clone(),
            },
            None => UpdateError::IoError {
                operation: operation.to_string(),
                path: None,
                message: "No space left on device".to_string(),
            },
        },
        _ if is_quota_error(&err) => match &path {
            Some(p) => UpdateError::QuotaExceeded { path: p.clone() },
            None => UpdateError::IoError {
                operation: operation.to_string(),
                path: None,
                message: "Disk quota exceeded".to_string(),
            },
        },
        _ => UpdateError::IoError {
            operation: operation.to_string(),
            path,
            message: err.to_string(),
        },
    }
}

/// Check if an I/O error is a disk space error.
fn is_disk_space_error(err: &std::io::Error) -> bool {
    #[cfg(unix)]
    {
        // ENOSPC = 28 on most Unix systems
        err.raw_os_error() == Some(28)
    }
    #[cfg(not(unix))]
    {
        // Check error message as fallback
        let msg = err.to_string().to_lowercase();
        msg.contains("no space") || msg.contains("disk full")
    }
}

/// Check if an I/O error is a quota error.
fn is_quota_error(err: &std::io::Error) -> bool {
    #[cfg(unix)]
    {
        // EDQUOT = 122 on Linux, 69 on macOS
        let raw = err.raw_os_error();
        raw == Some(122) || raw == Some(69)
    }
    #[cfg(not(unix))]
    {
        let msg = err.to_string().to_lowercase();
        msg.contains("quota")
    }
}

/// Helper to classify a reqwest error into UpdateError.
pub fn classify_reqwest_error(err: reqwest::Error, url: &str) -> UpdateError {
    if err.is_connect() {
        // Connection error - could be network unreachable or connection refused
        UpdateError::ConnectionFailed {
            url: url.to_string(),
            message: err.to_string(),
        }
    } else if err.is_timeout() {
        UpdateError::Timeout {
            operation: "HTTP request".to_string(),
            duration_secs: 30, // Default assumption
        }
    } else if err.is_status() {
        // HTTP status error
        if let Some(status) = err.status() {
            let status_code = status.as_u16();
            if status_code == 429 {
                UpdateError::RateLimited {
                    retry_after_secs: None,
                }
            } else {
                UpdateError::ServerError {
                    status: status_code,
                    message: err.to_string(),
                }
            }
        } else {
            UpdateError::ServerError {
                status: 0,
                message: err.to_string(),
            }
        }
    } else if err.is_decode() {
        UpdateError::InvalidResponse {
            message: format!("Failed to decode response: {}", err),
        }
    } else {
        // Check for TLS errors in the error chain
        let err_str = err.to_string().to_lowercase();
        if err_str.contains("tls") || err_str.contains("ssl") || err_str.contains("certificate") {
            UpdateError::TlsError {
                message: err.to_string(),
            }
        } else if err_str.contains("dns") || err_str.contains("resolve") {
            // Extract host from URL if possible
            let host = extract_host_from_url(url);
            UpdateError::DnsResolutionFailed { host }
        } else {
            UpdateError::ConnectionFailed {
                url: url.to_string(),
                message: err.to_string(),
            }
        }
    }
}

/// Extract the host portion from a URL string.
fn extract_host_from_url(url: &str) -> String {
    // Simple URL parsing without external crate
    // Try to extract host from "https://host/..." or "http://host/..."
    let url_lower = url.to_lowercase();
    let without_scheme = if url_lower.starts_with("https://") {
        &url[8..]
    } else if url_lower.starts_with("http://") {
        &url[7..]
    } else {
        url
    };

    // Take everything up to the first '/' or ':'
    without_scheme
        .split(&['/', ':'][..])
        .next()
        .unwrap_or(url)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_category_retryable() {
        assert!(UpdateErrorCategory::Network.is_retryable());
        assert!(UpdateErrorCategory::Server.is_retryable());
        assert!(!UpdateErrorCategory::Permission.is_retryable());
        assert!(!UpdateErrorCategory::DiskSpace.is_retryable());
        assert!(!UpdateErrorCategory::FileSystem.is_retryable());
    }

    #[test]
    fn test_error_category_as_str() {
        assert_eq!(UpdateErrorCategory::Network.as_str(), "network");
        assert_eq!(UpdateErrorCategory::Permission.as_str(), "permission");
        assert_eq!(UpdateErrorCategory::DiskSpace.as_str(), "disk_space");
    }

    #[test]
    fn test_connection_failed_error() {
        let err = UpdateError::ConnectionFailed {
            url: "https://example.com".to_string(),
            message: "Connection refused".to_string(),
        };

        assert_eq!(err.category(), UpdateErrorCategory::Network);
        assert!(err.is_retryable());
        assert_eq!(err.error_code(), "E_CONN_FAILED");
        assert!(err.user_message().contains("internet connection"));
    }

    #[test]
    fn test_permission_denied_error() {
        let err = UpdateError::PermissionDenied {
            path: PathBuf::from("/usr/local/bin/spoq"),
            operation: "write".to_string(),
        };

        assert_eq!(err.category(), UpdateErrorCategory::Permission);
        assert!(!err.is_retryable());
        assert_eq!(err.error_code(), "E_PERMISSION");
        assert!(err.user_message().contains("Permission denied"));
        assert!(err.user_message().contains("sudo"));
    }

    #[test]
    fn test_disk_space_error() {
        let err = UpdateError::InsufficientDiskSpace {
            required_bytes: 100 * 1024 * 1024, // 100 MB
            available_bytes: 10 * 1024 * 1024,  // 10 MB
            path: PathBuf::from("/tmp"),
        };

        assert_eq!(err.category(), UpdateErrorCategory::DiskSpace);
        assert!(!err.is_retryable());
        assert_eq!(err.error_code(), "E_DISK_SPACE");
        assert!(err.user_message().contains("Not enough disk space"));
        assert!(err.user_message().contains("100.0 MB"));
    }

    #[test]
    fn test_server_error_messages() {
        let err_404 = UpdateError::ServerError {
            status: 404,
            message: "Not Found".to_string(),
        };
        assert!(err_404.user_message().contains("not found"));

        let err_500 = UpdateError::ServerError {
            status: 500,
            message: "Internal Server Error".to_string(),
        };
        assert!(err_500.user_message().contains("experiencing issues"));
    }

    #[test]
    fn test_install_failed_restored_error() {
        let err = UpdateError::InstallFailedRestored {
            cause: "Permission denied".to_string(),
            restored_from: PathBuf::from("/usr/local/bin/spoq.backup"),
        };

        assert!(err.user_message().contains("Installation failed"));
        assert!(err.user_message().contains("restored from backup"));
    }

    #[test]
    fn test_install_failed_no_restore_error() {
        let err = UpdateError::InstallFailedNoRestore {
            install_error: "Write failed".to_string(),
            restore_error: "Backup not found".to_string(),
        };

        assert!(err.user_message().contains("CRITICAL"));
        assert!(err.user_message().contains("reinstall manually"));
    }

    #[test]
    fn test_error_display() {
        let err = UpdateError::Timeout {
            operation: "download".to_string(),
            duration_secs: 30,
        };

        let display = format!("{}", err);
        assert!(display.contains("download"));
        assert!(display.contains("30 seconds"));
    }

    #[test]
    fn test_classify_io_error_not_found() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let path = PathBuf::from("/tmp/test.txt");

        let err = classify_io_error(io_err, Some(path.clone()), "read");
        assert!(matches!(err, UpdateError::FileNotFound { .. }));
    }

    #[test]
    fn test_classify_io_error_permission_denied() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let path = PathBuf::from("/usr/local/bin/spoq");

        let err = classify_io_error(io_err, Some(path.clone()), "write");
        assert!(matches!(err, UpdateError::PermissionDenied { .. }));
    }

    #[test]
    fn test_rate_limited_error() {
        let err_with_retry = UpdateError::RateLimited {
            retry_after_secs: Some(60),
        };
        assert!(err_with_retry.user_message().contains("60 seconds"));

        let err_without_retry = UpdateError::RateLimited {
            retry_after_secs: None,
        };
        assert!(err_without_retry.user_message().contains("wait a moment"));
    }

    #[test]
    fn test_already_up_to_date() {
        let err = UpdateError::AlreadyUpToDate {
            current_version: "1.2.3".to_string(),
        };

        assert_eq!(err.category(), UpdateErrorCategory::Version);
        assert!(!err.is_retryable());
        assert!(err.user_message().contains("latest version"));
        assert!(err.user_message().contains("1.2.3"));
    }

    #[test]
    fn test_unsupported_platform() {
        let err = UpdateError::UnsupportedPlatform {
            os: "windows".to_string(),
            arch: "x86".to_string(),
        };

        assert_eq!(err.category(), UpdateErrorCategory::Platform);
        assert!(err.user_message().contains("Unsupported platform"));
        assert!(err.user_message().contains("macOS and Linux"));
    }
}
