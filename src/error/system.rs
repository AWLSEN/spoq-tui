//! System-related error types.
//!
//! This module defines errors related to system operations like
//! filesystem access, resource management, and OS interactions.

use std::fmt;
use std::path::PathBuf;

/// System-specific error variants.
///
/// These errors represent issues with the underlying system, including
/// filesystem operations, resource constraints, and OS-level errors.
#[derive(Debug, Clone)]
pub enum SystemError {
    /// File not found.
    FileNotFound { path: PathBuf },

    /// Directory not found.
    DirectoryNotFound { path: PathBuf },

    /// Permission denied for file/directory operation.
    PermissionDenied { path: PathBuf, operation: String },

    /// Failed to create directory.
    DirectoryCreationFailed { path: PathBuf, message: String },

    /// Insufficient disk space.
    InsufficientDiskSpace {
        path: PathBuf,
        required_bytes: Option<u64>,
        available_bytes: Option<u64>,
    },

    /// Disk quota exceeded.
    QuotaExceeded { path: PathBuf },

    /// Generic I/O error.
    IoError {
        operation: String,
        path: Option<PathBuf>,
        message: String,
    },

    /// Could not determine home directory.
    NoHomeDirectory,

    /// Could not determine configuration directory.
    NoConfigDirectory,

    /// Could not determine cache directory.
    NoCacheDirectory,

    /// Resource not available (e.g., file locked).
    ResourceBusy { resource: String, message: String },

    /// Environment variable not set or invalid.
    EnvironmentError { variable: String, message: String },

    /// Generic system error.
    Other { message: String },
}

impl SystemError {
    /// Check if this error might be transient (e.g., resource temporarily busy).
    pub fn is_transient(&self) -> bool {
        matches!(self, SystemError::ResourceBusy { .. })
    }

    /// Get a user-friendly error message.
    pub fn user_message(&self) -> String {
        match self {
            SystemError::FileNotFound { path } => {
                format!("File not found: '{}'", path.display())
            }
            SystemError::DirectoryNotFound { path } => {
                format!("Directory not found: '{}'", path.display())
            }
            SystemError::PermissionDenied { path, operation } => {
                format!(
                    "Permission denied: Cannot {} '{}'.\nTry checking file permissions or running with elevated privileges.",
                    operation,
                    path.display()
                )
            }
            SystemError::DirectoryCreationFailed { path, .. } => {
                format!(
                    "Failed to create directory: '{}'.\nPlease check permissions and try again.",
                    path.display()
                )
            }
            SystemError::InsufficientDiskSpace { path, required_bytes, available_bytes } => {
                let mut msg = format!(
                    "Not enough disk space at '{}'.",
                    path.display()
                );
                if let (Some(req), Some(avail)) = (required_bytes, available_bytes) {
                    let req_mb = *req as f64 / (1024.0 * 1024.0);
                    let avail_mb = *avail as f64 / (1024.0 * 1024.0);
                    msg.push_str(&format!(
                        "\nRequired: {:.1} MB, Available: {:.1} MB",
                        req_mb, avail_mb
                    ));
                }
                msg.push_str("\nPlease free up some disk space and try again.");
                msg
            }
            SystemError::QuotaExceeded { path } => {
                format!(
                    "Disk quota exceeded at '{}'.\nPlease free up some quota or contact your system administrator.",
                    path.display()
                )
            }
            SystemError::IoError { operation, path, .. } => {
                match path {
                    Some(p) => format!("Failed to {} '{}'", operation, p.display()),
                    None => format!("Failed to {}", operation),
                }
            }
            SystemError::NoHomeDirectory => {
                "Could not determine your home directory. Please check your environment configuration.".to_string()
            }
            SystemError::NoConfigDirectory => {
                "Could not determine configuration directory. Please check your environment.".to_string()
            }
            SystemError::NoCacheDirectory => {
                "Could not determine cache directory. Please check your environment.".to_string()
            }
            SystemError::ResourceBusy { resource, .. } => {
                format!(
                    "'{}' is currently in use. Please close any other applications using it and try again.",
                    resource
                )
            }
            SystemError::EnvironmentError { variable, message } => {
                format!("Environment variable '{}' error: {}", variable, message)
            }
            SystemError::Other { message } => {
                format!("System error: {}", message)
            }
        }
    }

    /// Get a short error code for logging.
    pub fn error_code(&self) -> &'static str {
        match self {
            SystemError::FileNotFound { .. } => "E_SYS_FILE_NOT_FOUND",
            SystemError::DirectoryNotFound { .. } => "E_SYS_DIR_NOT_FOUND",
            SystemError::PermissionDenied { .. } => "E_SYS_PERM",
            SystemError::DirectoryCreationFailed { .. } => "E_SYS_DIR_CREATE",
            SystemError::InsufficientDiskSpace { .. } => "E_SYS_DISK_SPACE",
            SystemError::QuotaExceeded { .. } => "E_SYS_QUOTA",
            SystemError::IoError { .. } => "E_SYS_IO",
            SystemError::NoHomeDirectory => "E_SYS_NO_HOME",
            SystemError::NoConfigDirectory => "E_SYS_NO_CONFIG",
            SystemError::NoCacheDirectory => "E_SYS_NO_CACHE",
            SystemError::ResourceBusy { .. } => "E_SYS_BUSY",
            SystemError::EnvironmentError { .. } => "E_SYS_ENV",
            SystemError::Other { .. } => "E_SYS_OTHER",
        }
    }
}

impl fmt::Display for SystemError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SystemError::FileNotFound { path } => {
                write!(f, "File not found: '{}'", path.display())
            }
            SystemError::DirectoryNotFound { path } => {
                write!(f, "Directory not found: '{}'", path.display())
            }
            SystemError::PermissionDenied { path, operation } => {
                write!(f, "Permission denied: {} '{}'", operation, path.display())
            }
            SystemError::DirectoryCreationFailed { path, message } => {
                write!(
                    f,
                    "Failed to create directory '{}': {}",
                    path.display(),
                    message
                )
            }
            SystemError::InsufficientDiskSpace {
                path,
                required_bytes,
                available_bytes,
            } => {
                write!(f, "Insufficient disk space at '{}'", path.display())?;
                if let (Some(req), Some(avail)) = (required_bytes, available_bytes) {
                    write!(f, " (need {} bytes, have {} bytes)", req, avail)?;
                }
                Ok(())
            }
            SystemError::QuotaExceeded { path } => {
                write!(f, "Disk quota exceeded at '{}'", path.display())
            }
            SystemError::IoError {
                operation,
                path,
                message,
            } => match path {
                Some(p) => write!(
                    f,
                    "I/O error during {} at '{}': {}",
                    operation,
                    p.display(),
                    message
                ),
                None => write!(f, "I/O error during {}: {}", operation, message),
            },
            SystemError::NoHomeDirectory => {
                write!(f, "Could not determine home directory")
            }
            SystemError::NoConfigDirectory => {
                write!(f, "Could not determine configuration directory")
            }
            SystemError::NoCacheDirectory => {
                write!(f, "Could not determine cache directory")
            }
            SystemError::ResourceBusy { resource, message } => {
                write!(f, "Resource '{}' is busy: {}", resource, message)
            }
            SystemError::EnvironmentError { variable, message } => {
                write!(f, "Environment variable '{}' error: {}", variable, message)
            }
            SystemError::Other { message } => {
                write!(f, "System error: {}", message)
            }
        }
    }
}

impl std::error::Error for SystemError {}

/// Classify an I/O error into a SystemError.
pub fn classify_io_error(
    err: std::io::Error,
    path: Option<PathBuf>,
    operation: &str,
) -> SystemError {
    use std::io::ErrorKind;

    match err.kind() {
        ErrorKind::NotFound => match &path {
            Some(p) => {
                if p.is_dir() || operation.contains("dir") {
                    SystemError::DirectoryNotFound { path: p.clone() }
                } else {
                    SystemError::FileNotFound { path: p.clone() }
                }
            }
            None => SystemError::IoError {
                operation: operation.to_string(),
                path: None,
                message: err.to_string(),
            },
        },
        ErrorKind::PermissionDenied => match &path {
            Some(p) => SystemError::PermissionDenied {
                path: p.clone(),
                operation: operation.to_string(),
            },
            None => SystemError::IoError {
                operation: operation.to_string(),
                path: None,
                message: "Permission denied".to_string(),
            },
        },
        _ if is_disk_space_error(&err) => match &path {
            Some(p) => SystemError::InsufficientDiskSpace {
                path: p.clone(),
                required_bytes: None,
                available_bytes: None,
            },
            None => SystemError::IoError {
                operation: operation.to_string(),
                path: None,
                message: "No space left on device".to_string(),
            },
        },
        _ if is_quota_error(&err) => match &path {
            Some(p) => SystemError::QuotaExceeded { path: p.clone() },
            None => SystemError::IoError {
                operation: operation.to_string(),
                path: None,
                message: "Disk quota exceeded".to_string(),
            },
        },
        _ => SystemError::IoError {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_not_found() {
        let err = SystemError::FileNotFound {
            path: PathBuf::from("/tmp/missing.txt"),
        };
        assert!(!err.is_transient());
        assert_eq!(err.error_code(), "E_SYS_FILE_NOT_FOUND");
        assert!(err.user_message().contains("/tmp/missing.txt"));
    }

    #[test]
    fn test_directory_not_found() {
        let err = SystemError::DirectoryNotFound {
            path: PathBuf::from("/var/missing"),
        };
        assert!(!err.is_transient());
        assert_eq!(err.error_code(), "E_SYS_DIR_NOT_FOUND");
    }

    #[test]
    fn test_permission_denied() {
        let err = SystemError::PermissionDenied {
            path: PathBuf::from("/root/secret"),
            operation: "read".to_string(),
        };
        assert!(!err.is_transient());
        assert_eq!(err.error_code(), "E_SYS_PERM");
        assert!(err.user_message().contains("Permission denied"));
        assert!(err.user_message().contains("read"));
    }

    #[test]
    fn test_insufficient_disk_space() {
        let err = SystemError::InsufficientDiskSpace {
            path: PathBuf::from("/tmp"),
            required_bytes: Some(100 * 1024 * 1024),
            available_bytes: Some(10 * 1024 * 1024),
        };
        assert!(!err.is_transient());
        assert_eq!(err.error_code(), "E_SYS_DISK_SPACE");
        assert!(err.user_message().contains("100.0 MB"));
        assert!(err.user_message().contains("10.0 MB"));
    }

    #[test]
    fn test_insufficient_disk_space_no_details() {
        let err = SystemError::InsufficientDiskSpace {
            path: PathBuf::from("/tmp"),
            required_bytes: None,
            available_bytes: None,
        };
        assert!(err.user_message().contains("Not enough disk space"));
    }

    #[test]
    fn test_quota_exceeded() {
        let err = SystemError::QuotaExceeded {
            path: PathBuf::from("/home/user"),
        };
        assert!(!err.is_transient());
        assert_eq!(err.error_code(), "E_SYS_QUOTA");
        assert!(err.user_message().contains("quota"));
    }

    #[test]
    fn test_resource_busy_is_transient() {
        let err = SystemError::ResourceBusy {
            resource: "config.json".to_string(),
            message: "file locked".to_string(),
        };
        assert!(err.is_transient());
        assert_eq!(err.error_code(), "E_SYS_BUSY");
    }

    #[test]
    fn test_no_home_directory() {
        let err = SystemError::NoHomeDirectory;
        assert!(!err.is_transient());
        assert_eq!(err.error_code(), "E_SYS_NO_HOME");
        assert!(err.user_message().contains("home directory"));
    }

    #[test]
    fn test_environment_error() {
        let err = SystemError::EnvironmentError {
            variable: "API_KEY".to_string(),
            message: "not set".to_string(),
        };
        assert!(!err.is_transient());
        assert_eq!(err.error_code(), "E_SYS_ENV");
        assert!(err.user_message().contains("API_KEY"));
    }

    #[test]
    fn test_io_error_with_path() {
        let err = SystemError::IoError {
            operation: "write".to_string(),
            path: Some(PathBuf::from("/tmp/file.txt")),
            message: "unexpected EOF".to_string(),
        };
        assert!(!err.is_transient());
        assert_eq!(err.error_code(), "E_SYS_IO");
        assert!(err.user_message().contains("write"));
        assert!(err.user_message().contains("file.txt"));
    }

    #[test]
    fn test_io_error_without_path() {
        let err = SystemError::IoError {
            operation: "connect".to_string(),
            path: None,
            message: "connection reset".to_string(),
        };
        let display = format!("{}", err);
        assert!(display.contains("connect"));
        assert!(!display.contains("at '"));
    }

    #[test]
    fn test_display_format() {
        let err = SystemError::DirectoryCreationFailed {
            path: PathBuf::from("/var/app"),
            message: "access denied".to_string(),
        };
        let display = format!("{}", err);
        assert!(display.contains("/var/app"));
        assert!(display.contains("access denied"));
    }

    #[test]
    fn test_classify_io_error_not_found() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let path = PathBuf::from("/tmp/test.txt");

        let err = classify_io_error(io_err, Some(path.clone()), "read");
        assert!(matches!(err, SystemError::FileNotFound { .. }));
    }

    #[test]
    fn test_classify_io_error_permission_denied() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let path = PathBuf::from("/root/secret");

        let err = classify_io_error(io_err, Some(path), "write");
        assert!(matches!(err, SystemError::PermissionDenied { .. }));
    }

    #[test]
    fn test_classify_io_error_other() {
        let io_err = std::io::Error::new(std::io::ErrorKind::Other, "unknown error");

        let err = classify_io_error(io_err, None, "operation");
        assert!(matches!(err, SystemError::IoError { .. }));
    }
}
