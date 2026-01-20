//! Update installer module for Spoq CLI.
//!
//! This module provides functionality to install downloaded updates by:
//! - Backing up the current binary to `<binary_path>.backup`
//! - Replacing the current binary with the downloaded update
//! - Setting correct permissions (chmod +x)

use std::fs;
use std::path::PathBuf;

/// Error type for installation operations.
#[derive(Debug)]
pub enum InstallError {
    /// I/O operation failed.
    Io(std::io::Error),
    /// Failed to determine the current executable path.
    NoExecutablePath,
    /// Update file not found at the specified path.
    UpdateFileNotFound(PathBuf),
    /// Failed to set executable permissions.
    PermissionError(String),
}

impl std::fmt::Display for InstallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstallError::Io(e) => write!(f, "I/O error: {}", e),
            InstallError::NoExecutablePath => {
                write!(f, "Could not determine current executable path")
            }
            InstallError::UpdateFileNotFound(path) => {
                write!(f, "Update file not found: {}", path.display())
            }
            InstallError::PermissionError(msg) => write!(f, "Permission error: {}", msg),
        }
    }
}

impl std::error::Error for InstallError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            InstallError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for InstallError {
    fn from(e: std::io::Error) -> Self {
        InstallError::Io(e)
    }
}

/// Result of a successful installation operation.
#[derive(Debug, Clone)]
pub struct InstallResult {
    /// Path to the installed binary.
    pub binary_path: PathBuf,
    /// Path to the backup of the old binary.
    pub backup_path: PathBuf,
    /// Version that was installed (if known).
    pub version: Option<String>,
}

/// Install a downloaded update binary.
///
/// This function:
/// 1. Backs up the current binary to `<binary_path>.backup`
/// 2. Replaces the current binary with the downloaded update
/// 3. Sets executable permissions on the new binary
///
/// # Arguments
///
/// * `update_path` - Path to the downloaded update binary
/// * `version` - Optional version string for tracking
///
/// # Example
///
/// ```ignore
/// let update_path = PathBuf::from("/tmp/spoq-update");
/// let result = install_update(&update_path, Some("0.2.0"))?;
/// println!("Installed to: {}", result.binary_path.display());
/// println!("Backup at: {}", result.backup_path.display());
/// ```
pub fn install_update(
    update_path: &PathBuf,
    version: Option<&str>,
) -> Result<InstallResult, InstallError> {
    // Verify the update file exists
    if !update_path.exists() {
        return Err(InstallError::UpdateFileNotFound(update_path.clone()));
    }

    // Get the path to the current executable
    let current_exe =
        std::env::current_exe().map_err(|_| InstallError::NoExecutablePath)?;

    // Create backup path
    let backup_path = current_exe.with_extension("backup");

    // Step 1: Create backup of current binary
    fs::copy(&current_exe, &backup_path)?;

    // Step 2: Replace current binary with update
    // Use atomic rename if possible, otherwise copy
    match fs::rename(update_path, &current_exe) {
        Ok(_) => {}
        Err(_) => {
            // Rename failed (maybe cross-device), try copy instead
            fs::copy(update_path, &current_exe)?;
            // Remove the update file after successful copy
            let _ = fs::remove_file(update_path);
        }
    }

    // Step 3: Set executable permissions (Unix only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&current_exe)?.permissions();
        perms.set_mode(0o755); // rwxr-xr-x
        fs::set_permissions(&current_exe, perms)?;
    }

    Ok(InstallResult {
        binary_path: current_exe,
        backup_path,
        version: version.map(String::from),
    })
}

/// Rollback to the backup binary.
///
/// Restores the backup binary created during installation.
/// Returns an error if no backup exists.
pub fn rollback_update() -> Result<InstallResult, InstallError> {
    let current_exe =
        std::env::current_exe().map_err(|_| InstallError::NoExecutablePath)?;
    let backup_path = current_exe.with_extension("backup");

    if !backup_path.exists() {
        return Err(InstallError::UpdateFileNotFound(backup_path));
    }

    // Replace current binary with backup
    fs::copy(&backup_path, &current_exe)?;

    // Set executable permissions (Unix only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&current_exe)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&current_exe, perms)?;
    }

    Ok(InstallResult {
        binary_path: current_exe,
        backup_path,
        version: None,
    })
}

/// Clean up the backup binary.
///
/// Removes the backup file created during installation.
/// Returns Ok even if the backup doesn't exist.
pub fn cleanup_backup() -> Result<(), InstallError> {
    let current_exe =
        std::env::current_exe().map_err(|_| InstallError::NoExecutablePath)?;
    let backup_path = current_exe.with_extension("backup");

    if backup_path.exists() {
        fs::remove_file(backup_path)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_install_error_display() {
        let err = InstallError::NoExecutablePath;
        let display = format!("{}", err);
        assert!(display.contains("executable path"));

        let err = InstallError::UpdateFileNotFound(PathBuf::from("/tmp/fake"));
        let display = format!("{}", err);
        assert!(display.contains("not found"));
        assert!(display.contains("/tmp/fake"));

        let err = InstallError::PermissionError("test".to_string());
        let display = format!("{}", err);
        assert!(display.contains("Permission"));
    }

    #[test]
    fn test_install_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let install_err: InstallError = io_err.into();
        assert!(matches!(install_err, InstallError::Io(_)));
    }

    #[test]
    fn test_install_result_clone() {
        let result = InstallResult {
            binary_path: PathBuf::from("/usr/local/bin/spoq"),
            backup_path: PathBuf::from("/usr/local/bin/spoq.backup"),
            version: Some("0.2.0".to_string()),
        };
        let cloned = result.clone();
        assert_eq!(cloned.binary_path, result.binary_path);
        assert_eq!(cloned.backup_path, result.backup_path);
        assert_eq!(cloned.version, result.version);
    }

    #[test]
    fn test_install_update_file_not_found() {
        let fake_path = PathBuf::from("/tmp/nonexistent-spoq-update-12345");
        let result = install_update(&fake_path, Some("0.2.0"));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), InstallError::UpdateFileNotFound(_)));
    }

    // Note: We can't easily test the full install_update function in unit tests
    // because it requires mocking std::env::current_exe() and file operations
    // on the actual binary. Integration tests would be more appropriate.
}
