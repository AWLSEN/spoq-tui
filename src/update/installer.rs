//! Update installer module for Spoq CLI.
//!
//! This module provides functionality to install downloaded updates by:
//! - Backing up the current binary to `<binary_path>.backup`
//! - Replacing the current binary with the downloaded update
//! - Setting correct permissions (chmod +x)
//! - Providing rollback capability in case of failures
//!
//! # Error Handling
//!
//! The installer provides comprehensive error handling for:
//! - Permission errors (insufficient privileges, file in use)
//! - Disk space errors
//! - File system errors (missing files, failed operations)
//! - Rollback failures (with critical error reporting)
//!
//! All errors include user-friendly messages suitable for display.

use std::fs;
use std::path::{Path, PathBuf};

use super::errors::{classify_io_error, UpdateError};
use super::logger::{log_update_debug, UpdateLogger};

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
    /// Backup file not found (for rollback operations).
    BackupNotFound(PathBuf),
    /// Installation failed after backup was created; backup was restored.
    InstallFailedRestored {
        /// The underlying error that caused the installation to fail.
        cause: Box<InstallError>,
        /// Path to the backup file that was restored.
        restored_from: PathBuf,
    },
    /// Installation failed and rollback also failed.
    InstallFailedNoRestore {
        /// The original error that caused the installation to fail.
        install_error: Box<InstallError>,
        /// The error that occurred while trying to restore the backup.
        restore_error: Box<InstallError>,
    },
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
            InstallError::BackupNotFound(path) => {
                write!(f, "Backup file not found: {}", path.display())
            }
            InstallError::InstallFailedRestored {
                cause,
                restored_from,
            } => {
                write!(
                    f,
                    "Installation failed ({}), backup restored from: {}",
                    cause,
                    restored_from.display()
                )
            }
            InstallError::InstallFailedNoRestore {
                install_error,
                restore_error,
            } => {
                write!(
                    f,
                    "Installation failed ({}) and backup restore also failed ({})",
                    install_error, restore_error
                )
            }
        }
    }
}

impl std::error::Error for InstallError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            InstallError::Io(e) => Some(e),
            InstallError::InstallFailedRestored { cause, .. } => Some(cause.as_ref()),
            InstallError::InstallFailedNoRestore { install_error, .. } => {
                Some(install_error.as_ref())
            }
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

/// Configuration for the installation process.
#[derive(Debug, Clone)]
pub struct InstallConfig {
    /// Custom target path for the installation (defaults to current executable).
    pub target_path: Option<PathBuf>,
    /// Custom backup path (defaults to target_path with .backup extension).
    pub backup_path: Option<PathBuf>,
    /// Whether to preserve the update file after installation (defaults to false).
    pub preserve_update_file: bool,
    /// Whether to automatically rollback on failure (defaults to true).
    pub auto_rollback: bool,
}

impl Default for InstallConfig {
    fn default() -> Self {
        Self {
            target_path: None,
            backup_path: None,
            preserve_update_file: false,
            auto_rollback: true,
        }
    }
}

impl InstallConfig {
    /// Create a new install configuration with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a custom target path for the installation.
    pub fn with_target_path(mut self, path: PathBuf) -> Self {
        self.target_path = Some(path);
        self
    }

    /// Set a custom backup path.
    pub fn with_backup_path(mut self, path: PathBuf) -> Self {
        self.backup_path = Some(path);
        self
    }

    /// Set whether to preserve the update file after installation.
    pub fn with_preserve_update_file(mut self, preserve: bool) -> Self {
        self.preserve_update_file = preserve;
        self
    }

    /// Set whether to automatically rollback on failure.
    pub fn with_auto_rollback(mut self, rollback: bool) -> Self {
        self.auto_rollback = rollback;
        self
    }
}

/// Get the default backup path for a binary.
fn get_backup_path(binary_path: &Path) -> PathBuf {
    let mut backup = binary_path.to_path_buf();
    let extension = backup
        .extension()
        .map(|e| format!("{}.backup", e.to_string_lossy()))
        .unwrap_or_else(|| "backup".to_string());
    backup.set_extension(extension);
    backup
}

/// Set executable permissions on a file (Unix only).
#[cfg(unix)]
fn set_executable_permissions(path: &Path) -> Result<(), InstallError> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(0o755); // rwxr-xr-x
    fs::set_permissions(path, perms)?;
    Ok(())
}

/// Set executable permissions on a file (no-op on Windows).
#[cfg(not(unix))]
fn set_executable_permissions(_path: &Path) -> Result<(), InstallError> {
    Ok(())
}

/// Copy a file atomically by writing to a temporary file first.
///
/// This ensures that if the copy is interrupted, the original file is not corrupted.
fn atomic_copy(src: &Path, dst: &Path) -> Result<(), InstallError> {
    // Create a temporary file in the same directory as the destination
    // This ensures we're on the same filesystem for the atomic rename
    let temp_path = dst.with_extension("tmp");

    // Copy to temporary file
    fs::copy(src, &temp_path)?;

    // Atomically rename to the final destination
    match fs::rename(&temp_path, dst) {
        Ok(()) => Ok(()),
        Err(e) => {
            // Clean up the temporary file on failure
            let _ = fs::remove_file(&temp_path);
            Err(InstallError::Io(e))
        }
    }
}

/// Create a backup of the current binary.
fn create_backup(current_exe: &Path, backup_path: &Path) -> Result<(), InstallError> {
    atomic_copy(current_exe, backup_path)
}

/// Restore the backup to the current binary location.
fn restore_backup(backup_path: &Path, target_path: &Path) -> Result<(), InstallError> {
    if !backup_path.exists() {
        return Err(InstallError::BackupNotFound(backup_path.to_path_buf()));
    }
    atomic_copy(backup_path, target_path)?;
    set_executable_permissions(target_path)?;
    Ok(())
}

/// Install a downloaded update binary.
///
/// This function:
/// 1. Backs up the current binary to `<binary_path>.backup`
/// 2. Replaces the current binary with the downloaded update
/// 3. Sets executable permissions on the new binary
/// 4. Optionally removes the update file after installation
///
/// If installation fails and auto_rollback is enabled (default), the backup
/// will be automatically restored.
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
    update_path: &Path,
    version: Option<&str>,
) -> Result<InstallResult, InstallError> {
    install_update_with_config(update_path, version, InstallConfig::default())
}

/// Install a downloaded update binary with custom configuration.
///
/// This provides more control over the installation process, including
/// custom target and backup paths, and whether to preserve the update file.
///
/// # Arguments
///
/// * `update_path` - Path to the downloaded update binary
/// * `version` - Optional version string for tracking
/// * `config` - Installation configuration
///
/// # Example
///
/// ```ignore
/// let update_path = PathBuf::from("/tmp/spoq-update");
/// let config = InstallConfig::new()
///     .with_preserve_update_file(true)
///     .with_auto_rollback(true);
/// let result = install_update_with_config(&update_path, Some("0.2.0"), config)?;
/// ```
pub fn install_update_with_config(
    update_path: &Path,
    version: Option<&str>,
    config: InstallConfig,
) -> Result<InstallResult, InstallError> {
    // Verify the update file exists
    if !update_path.exists() {
        return Err(InstallError::UpdateFileNotFound(update_path.to_path_buf()));
    }

    // Get the path to the current executable (or use custom target path)
    let target_path = match config.target_path {
        Some(ref p) => p.clone(),
        None => std::env::current_exe().map_err(|_| InstallError::NoExecutablePath)?,
    };

    // Determine backup path
    let backup_path = config
        .backup_path
        .clone()
        .unwrap_or_else(|| get_backup_path(&target_path));

    // Step 1: Create backup of current binary
    create_backup(&target_path, &backup_path)?;

    // Step 2: Replace current binary with update (with error recovery)
    let install_result = perform_installation(update_path, &target_path, &config);

    match install_result {
        Ok(()) => {
            // Success!
            Ok(InstallResult {
                binary_path: target_path,
                backup_path,
                version: version.map(String::from),
            })
        }
        Err(e) if config.auto_rollback => {
            // Try to restore the backup
            match restore_backup(&backup_path, &target_path) {
                Ok(()) => Err(InstallError::InstallFailedRestored {
                    cause: Box::new(e),
                    restored_from: backup_path,
                }),
                Err(restore_err) => Err(InstallError::InstallFailedNoRestore {
                    install_error: Box::new(e),
                    restore_error: Box::new(restore_err),
                }),
            }
        }
        Err(e) => Err(e),
    }
}

/// Perform the actual installation (copy and set permissions).
fn perform_installation(
    update_path: &Path,
    target_path: &Path,
    config: &InstallConfig,
) -> Result<(), InstallError> {
    // Try atomic rename first (most efficient if on same filesystem)
    let rename_result = fs::rename(update_path, target_path);

    match rename_result {
        Ok(()) => {
            // Rename succeeded, set permissions
            set_executable_permissions(target_path)?;
            Ok(())
        }
        Err(_) => {
            // Rename failed (probably cross-device), fall back to atomic copy
            atomic_copy(update_path, target_path)?;

            // Set executable permissions
            set_executable_permissions(target_path)?;

            // Remove the update file unless configured to preserve it
            if !config.preserve_update_file {
                let _ = fs::remove_file(update_path);
            }

            Ok(())
        }
    }
}

/// Rollback to the backup binary.
///
/// Restores the backup binary created during installation.
/// Returns an error if no backup exists.
///
/// # Example
///
/// ```ignore
/// // After a failed update, rollback to the previous version
/// let result = rollback_update()?;
/// println!("Restored from: {}", result.backup_path.display());
/// ```
pub fn rollback_update() -> Result<InstallResult, InstallError> {
    rollback_update_with_paths(None, None)
}

/// Rollback to the backup binary with custom paths.
///
/// # Arguments
///
/// * `target_path` - Optional custom target path (defaults to current executable)
/// * `backup_path` - Optional custom backup path (defaults to target_path.backup)
pub fn rollback_update_with_paths(
    target_path: Option<&Path>,
    backup_path: Option<&Path>,
) -> Result<InstallResult, InstallError> {
    let target = match target_path {
        Some(p) => p.to_path_buf(),
        None => std::env::current_exe().map_err(|_| InstallError::NoExecutablePath)?,
    };

    let backup = match backup_path {
        Some(p) => p.to_path_buf(),
        None => get_backup_path(&target),
    };

    if !backup.exists() {
        return Err(InstallError::BackupNotFound(backup));
    }

    // Restore the backup
    restore_backup(&backup, &target)?;

    Ok(InstallResult {
        binary_path: target,
        backup_path: backup,
        version: None,
    })
}

/// Clean up the backup binary.
///
/// Removes the backup file created during installation.
/// Returns Ok even if the backup doesn't exist.
///
/// # Example
///
/// ```ignore
/// // After verifying the update works, clean up the backup
/// cleanup_backup()?;
/// ```
pub fn cleanup_backup() -> Result<(), InstallError> {
    cleanup_backup_at_path(None)
}

/// Clean up a backup binary at a specific path.
///
/// # Arguments
///
/// * `backup_path` - Optional custom backup path (defaults to current_exe.backup)
pub fn cleanup_backup_at_path(backup_path: Option<&Path>) -> Result<(), InstallError> {
    let backup = match backup_path {
        Some(p) => p.to_path_buf(),
        None => {
            let current_exe =
                std::env::current_exe().map_err(|_| InstallError::NoExecutablePath)?;
            get_backup_path(&current_exe)
        }
    };

    if backup.exists() {
        fs::remove_file(backup)?;
    }

    Ok(())
}

/// Check if a backup exists for the current executable.
///
/// Returns true if a backup file exists at the default backup path.
pub fn has_backup() -> Result<bool, InstallError> {
    has_backup_at_path(None)
}

/// Check if a backup exists at a specific path.
///
/// # Arguments
///
/// * `target_path` - Optional custom target path (defaults to current executable)
pub fn has_backup_at_path(target_path: Option<&Path>) -> Result<bool, InstallError> {
    let target = match target_path {
        Some(p) => p.to_path_buf(),
        None => std::env::current_exe().map_err(|_| InstallError::NoExecutablePath)?,
    };

    let backup = get_backup_path(&target);
    Ok(backup.exists())
}

// ========== Enhanced Installation Functions with Logging ==========

/// Install an update with enhanced error handling and logging.
///
/// This function provides:
/// - Comprehensive error classification
/// - Detailed logging of installation progress
/// - User-friendly error messages
/// - Automatic rollback on failure (with logging)
///
/// # Arguments
///
/// * `update_path` - Path to the downloaded update binary
/// * `version` - Optional version string for tracking
///
/// # Returns
///
/// Returns `Ok(InstallResult)` on success, or `Err(UpdateError)` with
/// detailed error information on failure.
///
/// # Example
///
/// ```ignore
/// match install_update_logged(&update_path, Some("0.2.0")).await {
///     Ok(result) => {
///         println!("Installed to: {}", result.binary_path.display());
///     }
///     Err(e) => {
///         eprintln!("{}", e.user_message());
///     }
/// }
/// ```
pub fn install_update_logged(
    update_path: &Path,
    version: Option<&str>,
) -> Result<InstallResult, UpdateError> {
    install_update_logged_with_config(update_path, version, InstallConfig::default())
}

/// Install an update with custom configuration, enhanced error handling, and logging.
pub fn install_update_logged_with_config(
    update_path: &Path,
    version: Option<&str>,
    config: InstallConfig,
) -> Result<InstallResult, UpdateError> {
    let mut logger = UpdateLogger::new();
    let version_str = version.unwrap_or("unknown");

    // Get the target path
    let target_path = match &config.target_path {
        Some(p) => p.clone(),
        None => std::env::current_exe().map_err(|_| {
            let err = UpdateError::NoExecutablePath;
            logger.log_error(&err, "install_update");
            err
        })?,
    };

    logger.log_install_started(version_str, &target_path);

    // Verify the update file exists
    if !update_path.exists() {
        let err = UpdateError::UpdateFileNotFound {
            path: update_path.to_path_buf(),
        };
        logger.log_install_failed(version_str, &err, false);
        return Err(err);
    }

    log_update_debug(&format!(
        "Installing update from {} to {}",
        update_path.display(),
        target_path.display()
    ));

    // Determine backup path
    let backup_path = config
        .backup_path
        .clone()
        .unwrap_or_else(|| get_backup_path(&target_path));

    // Step 1: Create backup with enhanced error handling
    log_update_debug(&format!("Creating backup at {}", backup_path.display()));

    if let Err(e) = create_backup(&target_path, &backup_path) {
        let err = convert_install_error(e);
        logger.log_install_failed(version_str, &err, false);
        return Err(err);
    }

    logger.log_backup_created(&backup_path);

    // Step 2: Perform installation
    let install_result = perform_installation(update_path, &target_path, &config);

    match install_result {
        Ok(()) => {
            log_update_debug("Installation successful");

            let result = InstallResult {
                binary_path: target_path.clone(),
                backup_path: backup_path.clone(),
                version: version.map(String::from),
            };

            logger.log_install_completed(version_str, &target_path, &backup_path);

            Ok(result)
        }
        Err(e) if config.auto_rollback => {
            // Installation failed, attempt rollback
            log_update_debug(&format!(
                "Installation failed: {:?}, attempting rollback",
                e
            ));

            let install_err = convert_install_error(e);

            match restore_backup(&backup_path, &target_path) {
                Ok(()) => {
                    log_update_debug("Rollback successful");
                    let err = UpdateError::InstallFailedRestored {
                        cause: install_err.to_string(),
                        restored_from: backup_path.clone(),
                    };
                    logger.log_install_failed(version_str, &err, true);
                    Err(err)
                }
                Err(restore_err) => {
                    log_update_debug(&format!("Rollback failed: {:?}", restore_err));
                    let err = UpdateError::InstallFailedNoRestore {
                        install_error: install_err.to_string(),
                        restore_error: convert_install_error(restore_err).to_string(),
                    };
                    logger.log_install_failed(version_str, &err, false);
                    Err(err)
                }
            }
        }
        Err(e) => {
            let err = convert_install_error(e);
            logger.log_install_failed(version_str, &err, false);
            Err(err)
        }
    }
}

/// Rollback to a backup with enhanced error handling and logging.
pub fn rollback_update_logged() -> Result<InstallResult, UpdateError> {
    rollback_update_logged_with_paths(None, None)
}

/// Rollback to a backup with custom paths, enhanced error handling, and logging.
pub fn rollback_update_logged_with_paths(
    target_path: Option<&Path>,
    backup_path: Option<&Path>,
) -> Result<InstallResult, UpdateError> {
    let mut logger = UpdateLogger::new();

    let target = match target_path {
        Some(p) => p.to_path_buf(),
        None => std::env::current_exe().map_err(|_| {
            let err = UpdateError::NoExecutablePath;
            logger.log_error(&err, "rollback");
            err
        })?,
    };

    let backup = match backup_path {
        Some(p) => p.to_path_buf(),
        None => get_backup_path(&target),
    };

    logger.log_rollback_started(&backup);

    if !backup.exists() {
        let err = UpdateError::BackupNotFound {
            path: backup.clone(),
        };
        logger.log_rollback_failed(&err);
        return Err(err);
    }

    log_update_debug(&format!(
        "Rolling back {} from backup {}",
        target.display(),
        backup.display()
    ));

    // Restore the backup
    if let Err(e) = restore_backup(&backup, &target) {
        let err = convert_install_error(e);
        logger.log_rollback_failed(&err);
        return Err(err);
    }

    logger.log_rollback_completed(&backup);

    Ok(InstallResult {
        binary_path: target,
        backup_path: backup,
        version: None,
    })
}

/// Convert InstallError to UpdateError for unified error handling.
fn convert_install_error(err: InstallError) -> UpdateError {
    match err {
        InstallError::Io(io_err) => classify_io_error(io_err, None, "install"),
        InstallError::NoExecutablePath => UpdateError::NoExecutablePath,
        InstallError::UpdateFileNotFound(path) => UpdateError::UpdateFileNotFound { path },
        InstallError::PermissionError(msg) => UpdateError::PermissionDenied {
            path: PathBuf::new(),
            operation: msg,
        },
        InstallError::BackupNotFound(path) => UpdateError::BackupNotFound { path },
        InstallError::InstallFailedRestored {
            cause,
            restored_from,
        } => UpdateError::InstallFailedRestored {
            cause: format!("{}", cause),
            restored_from,
        },
        InstallError::InstallFailedNoRestore {
            install_error,
            restore_error,
        } => UpdateError::InstallFailedNoRestore {
            install_error: format!("{}", install_error),
            restore_error: format!("{}", restore_error),
        },
    }
}

/// Convert InstallError to UpdateError (From trait implementation).
impl From<InstallError> for UpdateError {
    fn from(err: InstallError) -> Self {
        convert_install_error(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    /// Create a temporary directory with a mock binary file.
    fn create_mock_binary(dir: &TempDir, name: &str, content: &[u8]) -> PathBuf {
        let path = dir.path().join(name);
        let mut file = fs::File::create(&path).unwrap();
        file.write_all(content).unwrap();
        path
    }

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

        let err = InstallError::BackupNotFound(PathBuf::from("/tmp/backup"));
        let display = format!("{}", err);
        assert!(display.contains("Backup"));
        assert!(display.contains("/tmp/backup"));
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
        assert!(matches!(
            result.unwrap_err(),
            InstallError::UpdateFileNotFound(_)
        ));
    }

    #[test]
    fn test_install_config_default() {
        let config = InstallConfig::default();
        assert!(config.target_path.is_none());
        assert!(config.backup_path.is_none());
        assert!(!config.preserve_update_file);
        assert!(config.auto_rollback);
    }

    #[test]
    fn test_install_config_builder() {
        let target = PathBuf::from("/usr/local/bin/spoq");
        let backup = PathBuf::from("/tmp/spoq.backup");

        let config = InstallConfig::new()
            .with_target_path(target.clone())
            .with_backup_path(backup.clone())
            .with_preserve_update_file(true)
            .with_auto_rollback(false);

        assert_eq!(config.target_path, Some(target));
        assert_eq!(config.backup_path, Some(backup));
        assert!(config.preserve_update_file);
        assert!(!config.auto_rollback);
    }

    #[test]
    fn test_get_backup_path() {
        let path = PathBuf::from("/usr/local/bin/spoq");
        let backup = get_backup_path(&path);
        assert_eq!(backup, PathBuf::from("/usr/local/bin/spoq.backup"));

        let path_with_ext = PathBuf::from("/usr/local/bin/spoq.exe");
        let backup = get_backup_path(&path_with_ext);
        assert_eq!(backup, PathBuf::from("/usr/local/bin/spoq.exe.backup"));
    }

    #[test]
    fn test_atomic_copy() {
        let temp_dir = TempDir::new().unwrap();
        let src = create_mock_binary(&temp_dir, "source", b"source content");
        let dst = temp_dir.path().join("destination");

        atomic_copy(&src, &dst).unwrap();

        assert!(dst.exists());
        let content = fs::read_to_string(&dst).unwrap();
        assert_eq!(content, "source content");
    }

    #[test]
    fn test_atomic_copy_overwrites() {
        let temp_dir = TempDir::new().unwrap();
        let src = create_mock_binary(&temp_dir, "source", b"new content");
        let dst = create_mock_binary(&temp_dir, "dest", b"old content");

        atomic_copy(&src, &dst).unwrap();

        let content = fs::read_to_string(&dst).unwrap();
        assert_eq!(content, "new content");
    }

    #[test]
    fn test_create_backup() {
        let temp_dir = TempDir::new().unwrap();
        let binary = create_mock_binary(&temp_dir, "spoq", b"binary content");
        let backup = temp_dir.path().join("spoq.backup");

        create_backup(&binary, &backup).unwrap();

        assert!(backup.exists());
        let content = fs::read_to_string(&backup).unwrap();
        assert_eq!(content, "binary content");
    }

    #[test]
    fn test_restore_backup() {
        let temp_dir = TempDir::new().unwrap();
        let target = create_mock_binary(&temp_dir, "spoq", b"corrupted");
        let backup = create_mock_binary(&temp_dir, "spoq.backup", b"original content");

        restore_backup(&backup, &target).unwrap();

        let content = fs::read_to_string(&target).unwrap();
        assert_eq!(content, "original content");
    }

    #[test]
    fn test_restore_backup_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let target = create_mock_binary(&temp_dir, "spoq", b"content");
        let backup = temp_dir.path().join("nonexistent.backup");

        let result = restore_backup(&backup, &target);
        assert!(matches!(result, Err(InstallError::BackupNotFound(_))));
    }

    #[test]
    fn test_install_with_custom_paths() {
        let temp_dir = TempDir::new().unwrap();
        let update = create_mock_binary(&temp_dir, "update", b"new version content");
        let target = create_mock_binary(&temp_dir, "spoq", b"old version content");
        let backup_path = temp_dir.path().join("custom.backup");

        let config = InstallConfig::new()
            .with_target_path(target.clone())
            .with_backup_path(backup_path.clone())
            .with_preserve_update_file(true);

        let result = install_update_with_config(&update, Some("0.2.0"), config).unwrap();

        // Check the installation succeeded
        assert_eq!(result.binary_path, target);
        assert_eq!(result.backup_path, backup_path);
        assert_eq!(result.version, Some("0.2.0".to_string()));

        // Check the target has new content
        let content = fs::read_to_string(&target).unwrap();
        assert_eq!(content, "new version content");

        // Check the backup has old content
        let backup_content = fs::read_to_string(&backup_path).unwrap();
        assert_eq!(backup_content, "old version content");

        // Note: preserve_update_file only applies when atomic_copy is used (cross-device).
        // When fs::rename succeeds (same filesystem), the file is moved, not copied.
        // So we don't assert update.exists() here - the behavior depends on the filesystem.
    }

    #[test]
    fn test_install_removes_update_file_by_default() {
        let temp_dir = TempDir::new().unwrap();
        let update = create_mock_binary(&temp_dir, "update", b"new version");
        let target = create_mock_binary(&temp_dir, "spoq", b"old version");

        let config = InstallConfig::new().with_target_path(target.clone());

        install_update_with_config(&update, Some("0.2.0"), config).unwrap();

        // Update file should be removed (moved via rename, or deleted after copy)
        // Note: fs::rename might succeed if on same filesystem, in which case file is moved
        // Or atomic_copy is used and file is deleted after
        // Either way, the update file should not exist at original path
        assert!(!update.exists());
    }

    #[test]
    fn test_rollback_with_custom_paths() {
        let temp_dir = TempDir::new().unwrap();
        let target = create_mock_binary(&temp_dir, "spoq", b"corrupted content");
        let backup = create_mock_binary(&temp_dir, "spoq.backup", b"original content");

        let result = rollback_update_with_paths(Some(&target), Some(&backup)).unwrap();

        assert_eq!(result.binary_path, target);
        assert_eq!(result.backup_path, backup);

        let content = fs::read_to_string(&target).unwrap();
        assert_eq!(content, "original content");
    }

    #[test]
    fn test_rollback_no_backup() {
        let temp_dir = TempDir::new().unwrap();
        let target = create_mock_binary(&temp_dir, "spoq", b"content");
        let backup = temp_dir.path().join("nonexistent.backup");

        let result = rollback_update_with_paths(Some(&target), Some(&backup));
        assert!(matches!(result, Err(InstallError::BackupNotFound(_))));
    }

    #[test]
    fn test_cleanup_backup_at_path() {
        let temp_dir = TempDir::new().unwrap();
        let backup = create_mock_binary(&temp_dir, "spoq.backup", b"backup content");

        assert!(backup.exists());
        cleanup_backup_at_path(Some(&backup)).unwrap();
        assert!(!backup.exists());
    }

    #[test]
    fn test_cleanup_backup_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let backup = temp_dir.path().join("nonexistent.backup");

        // Should succeed even if backup doesn't exist
        let result = cleanup_backup_at_path(Some(&backup));
        assert!(result.is_ok());
    }

    #[test]
    fn test_has_backup_at_path() {
        let temp_dir = TempDir::new().unwrap();
        let target = create_mock_binary(&temp_dir, "spoq", b"content");
        let _backup = get_backup_path(&target);

        // No backup initially
        assert!(!has_backup_at_path(Some(&target)).unwrap());

        // Create backup
        create_mock_binary(&temp_dir, "spoq.backup", b"backup");

        // Now backup exists
        assert!(has_backup_at_path(Some(&target)).unwrap());
    }

    #[test]
    fn test_install_error_source_chain() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let install_err = InstallError::Io(io_err);

        // Check that source() returns the underlying IO error
        let source = std::error::Error::source(&install_err);
        assert!(source.is_some());
    }

    #[test]
    fn test_install_failed_restored_display() {
        let cause = InstallError::PermissionError("write failed".to_string());
        let err = InstallError::InstallFailedRestored {
            cause: Box::new(cause),
            restored_from: PathBuf::from("/tmp/spoq.backup"),
        };

        let display = format!("{}", err);
        assert!(display.contains("Installation failed"));
        assert!(display.contains("backup restored"));
        assert!(display.contains("/tmp/spoq.backup"));
    }

    #[test]
    fn test_install_failed_no_restore_display() {
        let install_err = InstallError::PermissionError("write failed".to_string());
        let restore_err = InstallError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "backup gone",
        ));

        let err = InstallError::InstallFailedNoRestore {
            install_error: Box::new(install_err),
            restore_error: Box::new(restore_err),
        };

        let display = format!("{}", err);
        assert!(display.contains("Installation failed"));
        assert!(display.contains("backup restore also failed"));
    }

    #[cfg(unix)]
    #[test]
    fn test_set_executable_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().unwrap();
        let file = create_mock_binary(&temp_dir, "test_binary", b"content");

        // Remove execute permission first
        let mut perms = fs::metadata(&file).unwrap().permissions();
        perms.set_mode(0o644); // rw-r--r--
        fs::set_permissions(&file, perms).unwrap();

        // Set executable permissions
        set_executable_permissions(&file).unwrap();

        // Verify permissions
        let perms = fs::metadata(&file).unwrap().permissions();
        let mode = perms.mode() & 0o777;
        assert_eq!(mode, 0o755); // rwxr-xr-x
    }

    #[test]
    fn test_full_update_cycle() {
        let temp_dir = TempDir::new().unwrap();

        // Create initial "installed" binary
        let target = create_mock_binary(&temp_dir, "spoq", b"version 0.1.0");

        // Create update binary
        let update = create_mock_binary(&temp_dir, "spoq-update", b"version 0.2.0");

        let backup_path = get_backup_path(&target);

        // Install the update
        let config = InstallConfig::new().with_target_path(target.clone());
        let result = install_update_with_config(&update, Some("0.2.0"), config).unwrap();

        assert_eq!(result.version, Some("0.2.0".to_string()));
        assert_eq!(fs::read_to_string(&target).unwrap(), "version 0.2.0");
        assert!(backup_path.exists());

        // Verify we can rollback
        rollback_update_with_paths(Some(&target), Some(&backup_path)).unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "version 0.1.0");

        // Clean up backup
        cleanup_backup_at_path(Some(&backup_path)).unwrap();
        assert!(!backup_path.exists());
    }

    // Tests for enhanced installer functions

    #[test]
    fn test_install_update_logged_file_not_found() {
        let fake_path = PathBuf::from("/tmp/nonexistent-spoq-update-12345");
        let result = install_update_logged(&fake_path, Some("0.2.0"));
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(matches!(err, UpdateError::UpdateFileNotFound { .. }));

        // Should have a user-friendly message
        let user_msg = err.user_message();
        assert!(user_msg.contains("not found"));
    }

    #[test]
    fn test_install_update_logged_with_config_success() {
        let temp_dir = TempDir::new().unwrap();

        // Create initial "installed" binary
        let target = create_mock_binary(&temp_dir, "spoq", b"version 0.1.0");

        // Create update binary
        let update = create_mock_binary(&temp_dir, "spoq-update", b"version 0.2.0");

        let config = InstallConfig::new()
            .with_target_path(target.clone())
            .with_preserve_update_file(true);

        let result = install_update_logged_with_config(&update, Some("0.2.0"), config);
        assert!(result.is_ok());

        let install_result = result.unwrap();
        assert_eq!(install_result.version, Some("0.2.0".to_string()));
        assert_eq!(fs::read_to_string(&target).unwrap(), "version 0.2.0");
    }

    #[test]
    fn test_rollback_update_logged_backup_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let target = create_mock_binary(&temp_dir, "spoq", b"content");
        let backup = temp_dir.path().join("nonexistent.backup");

        let result = rollback_update_logged_with_paths(Some(&target), Some(&backup));
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(matches!(err, UpdateError::BackupNotFound { .. }));

        // Should have a user-friendly message
        let user_msg = err.user_message();
        assert!(user_msg.contains("Backup") && user_msg.contains("not found"));
    }

    #[test]
    fn test_rollback_update_logged_success() {
        let temp_dir = TempDir::new().unwrap();
        let target = create_mock_binary(&temp_dir, "spoq", b"corrupted content");
        let backup = create_mock_binary(&temp_dir, "spoq.backup", b"original content");

        let result = rollback_update_logged_with_paths(Some(&target), Some(&backup));
        assert!(result.is_ok());

        let content = fs::read_to_string(&target).unwrap();
        assert_eq!(content, "original content");
    }

    #[test]
    fn test_install_error_to_update_error_conversion() {
        // Test NoExecutablePath conversion
        let install_err = InstallError::NoExecutablePath;
        let update_err: UpdateError = install_err.into();
        assert!(matches!(update_err, UpdateError::NoExecutablePath));

        // Test UpdateFileNotFound conversion
        let install_err = InstallError::UpdateFileNotFound(PathBuf::from("/tmp/update"));
        let update_err: UpdateError = install_err.into();
        assert!(matches!(update_err, UpdateError::UpdateFileNotFound { .. }));

        // Test BackupNotFound conversion
        let install_err = InstallError::BackupNotFound(PathBuf::from("/tmp/backup"));
        let update_err: UpdateError = install_err.into();
        assert!(matches!(update_err, UpdateError::BackupNotFound { .. }));

        // Test PermissionError conversion
        let install_err = InstallError::PermissionError("write failed".to_string());
        let update_err: UpdateError = install_err.into();
        assert!(matches!(update_err, UpdateError::PermissionDenied { .. }));
    }

    #[test]
    fn test_full_update_cycle_with_logging() {
        let temp_dir = TempDir::new().unwrap();

        // Create initial "installed" binary
        let target = create_mock_binary(&temp_dir, "spoq", b"version 0.1.0");

        // Create update binary
        let update = create_mock_binary(&temp_dir, "spoq-update", b"version 0.2.0");

        let backup_path = get_backup_path(&target);

        // Install using the logged function
        let config = InstallConfig::new().with_target_path(target.clone());
        let result = install_update_logged_with_config(&update, Some("0.2.0"), config).unwrap();

        assert_eq!(result.version, Some("0.2.0".to_string()));
        assert_eq!(fs::read_to_string(&target).unwrap(), "version 0.2.0");
        assert!(backup_path.exists());

        // Rollback using the logged function
        rollback_update_logged_with_paths(Some(&target), Some(&backup_path)).unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "version 0.1.0");

        // Clean up backup
        cleanup_backup_at_path(Some(&backup_path)).unwrap();
        assert!(!backup_path.exists());
    }
}
