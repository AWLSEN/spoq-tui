//! Update event logging for Spoq CLI.
//!
//! This module provides structured logging for all update operations,
//! including check attempts, download progress, installation results,
//! and error conditions.

use std::path::Path;
use std::time::{Duration, Instant};

use super::errors::UpdateError;

/// Log level for update events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateLogLevel {
    /// Debug information (detailed progress)
    Debug,
    /// Informational messages (normal operations)
    Info,
    /// Warnings (recoverable issues)
    Warn,
    /// Errors (operation failures)
    Error,
}

/// Types of update events that can be logged.
#[derive(Debug, Clone)]
pub enum UpdateEvent {
    // ========== Check Events ==========
    /// Starting an update check
    CheckStarted { current_version: String },
    /// Update check completed successfully
    CheckCompleted {
        current_version: String,
        latest_version: String,
        update_available: bool,
        duration: Duration,
    },
    /// Update check failed
    CheckFailed {
        error: String,
        error_code: String,
        duration: Duration,
    },

    // ========== Download Events ==========
    /// Starting a download
    DownloadStarted { version: String, url: String },
    /// Download progress update
    DownloadProgress {
        version: String,
        bytes_downloaded: u64,
        total_bytes: Option<u64>,
        percent: Option<f32>,
    },
    /// Download completed successfully
    DownloadCompleted {
        version: String,
        file_path: String,
        file_size: u64,
        duration: Duration,
    },
    /// Download failed
    DownloadFailed {
        version: String,
        error: String,
        error_code: String,
        duration: Duration,
    },

    // ========== Installation Events ==========
    /// Starting installation
    InstallStarted {
        version: String,
        target_path: String,
    },
    /// Backup created
    BackupCreated { backup_path: String },
    /// Installation completed successfully
    InstallCompleted {
        version: String,
        binary_path: String,
        backup_path: String,
        duration: Duration,
    },
    /// Installation failed
    InstallFailed {
        version: String,
        error: String,
        error_code: String,
        restored: bool,
        duration: Duration,
    },

    // ========== Rollback Events ==========
    /// Starting rollback
    RollbackStarted { backup_path: String },
    /// Rollback completed
    RollbackCompleted {
        backup_path: String,
        duration: Duration,
    },
    /// Rollback failed
    RollbackFailed { error: String, duration: Duration },

    // ========== Cleanup Events ==========
    /// Cleanup started
    CleanupStarted,
    /// Cleanup completed
    CleanupCompleted { files_removed: usize },
}

impl UpdateEvent {
    /// Get the log level for this event.
    pub fn level(&self) -> UpdateLogLevel {
        match self {
            UpdateEvent::CheckStarted { .. }
            | UpdateEvent::DownloadStarted { .. }
            | UpdateEvent::DownloadProgress { .. }
            | UpdateEvent::InstallStarted { .. }
            | UpdateEvent::BackupCreated { .. }
            | UpdateEvent::RollbackStarted { .. }
            | UpdateEvent::CleanupStarted => UpdateLogLevel::Debug,

            UpdateEvent::CheckCompleted { .. }
            | UpdateEvent::DownloadCompleted { .. }
            | UpdateEvent::InstallCompleted { .. }
            | UpdateEvent::RollbackCompleted { .. }
            | UpdateEvent::CleanupCompleted { .. } => UpdateLogLevel::Info,

            UpdateEvent::CheckFailed { .. }
            | UpdateEvent::DownloadFailed { .. }
            | UpdateEvent::InstallFailed { .. }
            | UpdateEvent::RollbackFailed { .. } => UpdateLogLevel::Error,
        }
    }

    /// Get a human-readable message for this event.
    pub fn message(&self) -> String {
        match self {
            UpdateEvent::CheckStarted { current_version } => {
                format!("Checking for updates (current: v{})", current_version)
            }
            UpdateEvent::CheckCompleted {
                current_version,
                latest_version,
                update_available,
                duration,
            } => {
                if *update_available {
                    format!(
                        "Update available: v{} -> v{} (checked in {:.1}s)",
                        current_version,
                        latest_version,
                        duration.as_secs_f32()
                    )
                } else {
                    format!(
                        "Already up to date: v{} (checked in {:.1}s)",
                        current_version,
                        duration.as_secs_f32()
                    )
                }
            }
            UpdateEvent::CheckFailed {
                error,
                error_code,
                duration,
            } => {
                format!(
                    "Update check failed [{}]: {} (after {:.1}s)",
                    error_code,
                    error,
                    duration.as_secs_f32()
                )
            }
            UpdateEvent::DownloadStarted { version, url } => {
                format!("Starting download of v{} from {}", version, url)
            }
            UpdateEvent::DownloadProgress {
                version,
                bytes_downloaded,
                total_bytes,
                percent,
            } => {
                let bytes_str = format_bytes(*bytes_downloaded);
                match (total_bytes, percent) {
                    (Some(total), Some(pct)) => {
                        let total_str = format_bytes(*total);
                        format!(
                            "Downloading v{}: {} / {} ({:.1}%)",
                            version, bytes_str, total_str, pct
                        )
                    }
                    _ => format!("Downloading v{}: {}", version, bytes_str),
                }
            }
            UpdateEvent::DownloadCompleted {
                version,
                file_path,
                file_size,
                duration,
            } => {
                format!(
                    "Download complete: v{} ({}) saved to {} in {:.1}s",
                    version,
                    format_bytes(*file_size),
                    file_path,
                    duration.as_secs_f32()
                )
            }
            UpdateEvent::DownloadFailed {
                version,
                error,
                error_code,
                duration,
            } => {
                format!(
                    "Download of v{} failed [{}]: {} (after {:.1}s)",
                    version,
                    error_code,
                    error,
                    duration.as_secs_f32()
                )
            }
            UpdateEvent::InstallStarted {
                version,
                target_path,
            } => {
                format!("Installing v{} to {}", version, target_path)
            }
            UpdateEvent::BackupCreated { backup_path } => {
                format!("Backup created at {}", backup_path)
            }
            UpdateEvent::InstallCompleted {
                version,
                binary_path,
                backup_path: _,
                duration,
            } => {
                format!(
                    "Successfully installed v{} to {} in {:.1}s",
                    version,
                    binary_path,
                    duration.as_secs_f32()
                )
            }
            UpdateEvent::InstallFailed {
                version,
                error,
                error_code,
                restored,
                duration,
            } => {
                let restore_msg = if *restored {
                    " (previous version restored)"
                } else {
                    " (WARNING: restore failed)"
                };
                format!(
                    "Installation of v{} failed [{}]: {}{} (after {:.1}s)",
                    version,
                    error_code,
                    error,
                    restore_msg,
                    duration.as_secs_f32()
                )
            }
            UpdateEvent::RollbackStarted { backup_path } => {
                format!("Rolling back from backup: {}", backup_path)
            }
            UpdateEvent::RollbackCompleted {
                backup_path,
                duration,
            } => {
                format!(
                    "Rollback complete from {} in {:.1}s",
                    backup_path,
                    duration.as_secs_f32()
                )
            }
            UpdateEvent::RollbackFailed { error, duration } => {
                format!(
                    "Rollback failed: {} (after {:.1}s)",
                    error,
                    duration.as_secs_f32()
                )
            }
            UpdateEvent::CleanupStarted => "Starting cleanup of old update files".to_string(),
            UpdateEvent::CleanupCompleted { files_removed } => {
                format!("Cleanup complete: {} files removed", files_removed)
            }
        }
    }
}

/// Format bytes in a human-readable way.
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} bytes", bytes)
    }
}

/// Logger for update operations.
///
/// This struct provides methods to log update events using the tracing framework.
/// It also maintains timing information for operation duration tracking.
#[derive(Debug)]
pub struct UpdateLogger {
    /// Start time of the current operation
    operation_start: Option<Instant>,
    /// Name of the current operation
    operation_name: Option<String>,
}

impl Default for UpdateLogger {
    fn default() -> Self {
        Self::new()
    }
}

impl UpdateLogger {
    /// Create a new UpdateLogger.
    pub fn new() -> Self {
        Self {
            operation_start: None,
            operation_name: None,
        }
    }

    /// Start timing an operation.
    pub fn start_operation(&mut self, name: &str) {
        self.operation_start = Some(Instant::now());
        self.operation_name = Some(name.to_string());
    }

    /// Get the elapsed duration since the operation started.
    pub fn elapsed(&self) -> Duration {
        self.operation_start
            .map(|start| start.elapsed())
            .unwrap_or_default()
    }

    /// Log an update event.
    pub fn log(&self, event: &UpdateEvent) {
        let message = event.message();

        match event.level() {
            UpdateLogLevel::Debug => {
                tracing::debug!(
                    target: "spoq::update",
                    event_type = %format!("{:?}", std::mem::discriminant(event)),
                    "{}",
                    message
                );
            }
            UpdateLogLevel::Info => {
                tracing::info!(
                    target: "spoq::update",
                    event_type = %format!("{:?}", std::mem::discriminant(event)),
                    "{}",
                    message
                );
            }
            UpdateLogLevel::Warn => {
                tracing::warn!(
                    target: "spoq::update",
                    event_type = %format!("{:?}", std::mem::discriminant(event)),
                    "{}",
                    message
                );
            }
            UpdateLogLevel::Error => {
                tracing::error!(
                    target: "spoq::update",
                    event_type = %format!("{:?}", std::mem::discriminant(event)),
                    "{}",
                    message
                );
            }
        }
    }

    /// Log an UpdateError with full context.
    pub fn log_error(&self, error: &UpdateError, context: &str) {
        tracing::error!(
            target: "spoq::update",
            error_code = %error.error_code(),
            error_category = %error.category(),
            retryable = %error.is_retryable(),
            context = %context,
            "Update error: {}",
            error
        );
    }

    // ========== Convenience methods for common events ==========

    /// Log the start of an update check.
    pub fn log_check_started(&mut self, current_version: &str) {
        self.start_operation("check");
        self.log(&UpdateEvent::CheckStarted {
            current_version: current_version.to_string(),
        });
    }

    /// Log a successful update check.
    pub fn log_check_completed(
        &self,
        current_version: &str,
        latest_version: &str,
        update_available: bool,
    ) {
        self.log(&UpdateEvent::CheckCompleted {
            current_version: current_version.to_string(),
            latest_version: latest_version.to_string(),
            update_available,
            duration: self.elapsed(),
        });
    }

    /// Log a failed update check.
    pub fn log_check_failed(&self, error: &UpdateError) {
        self.log(&UpdateEvent::CheckFailed {
            error: error.to_string(),
            error_code: error.error_code().to_string(),
            duration: self.elapsed(),
        });
    }

    /// Log the start of a download.
    pub fn log_download_started(&mut self, version: &str, url: &str) {
        self.start_operation("download");
        self.log(&UpdateEvent::DownloadStarted {
            version: version.to_string(),
            url: url.to_string(),
        });
    }

    /// Log download progress.
    pub fn log_download_progress(
        &self,
        version: &str,
        bytes_downloaded: u64,
        total_bytes: Option<u64>,
    ) {
        let percent = total_bytes.map(|total| (bytes_downloaded as f32 / total as f32) * 100.0);
        self.log(&UpdateEvent::DownloadProgress {
            version: version.to_string(),
            bytes_downloaded,
            total_bytes,
            percent,
        });
    }

    /// Log a successful download.
    pub fn log_download_completed(&self, version: &str, file_path: &Path, file_size: u64) {
        self.log(&UpdateEvent::DownloadCompleted {
            version: version.to_string(),
            file_path: file_path.display().to_string(),
            file_size,
            duration: self.elapsed(),
        });
    }

    /// Log a failed download.
    pub fn log_download_failed(&self, version: &str, error: &UpdateError) {
        self.log(&UpdateEvent::DownloadFailed {
            version: version.to_string(),
            error: error.to_string(),
            error_code: error.error_code().to_string(),
            duration: self.elapsed(),
        });
    }

    /// Log the start of an installation.
    pub fn log_install_started(&mut self, version: &str, target_path: &Path) {
        self.start_operation("install");
        self.log(&UpdateEvent::InstallStarted {
            version: version.to_string(),
            target_path: target_path.display().to_string(),
        });
    }

    /// Log backup creation.
    pub fn log_backup_created(&self, backup_path: &Path) {
        self.log(&UpdateEvent::BackupCreated {
            backup_path: backup_path.display().to_string(),
        });
    }

    /// Log a successful installation.
    pub fn log_install_completed(&self, version: &str, binary_path: &Path, backup_path: &Path) {
        self.log(&UpdateEvent::InstallCompleted {
            version: version.to_string(),
            binary_path: binary_path.display().to_string(),
            backup_path: backup_path.display().to_string(),
            duration: self.elapsed(),
        });
    }

    /// Log a failed installation.
    pub fn log_install_failed(&self, version: &str, error: &UpdateError, restored: bool) {
        self.log(&UpdateEvent::InstallFailed {
            version: version.to_string(),
            error: error.to_string(),
            error_code: error.error_code().to_string(),
            restored,
            duration: self.elapsed(),
        });
    }

    /// Log the start of a rollback.
    pub fn log_rollback_started(&mut self, backup_path: &Path) {
        self.start_operation("rollback");
        self.log(&UpdateEvent::RollbackStarted {
            backup_path: backup_path.display().to_string(),
        });
    }

    /// Log a successful rollback.
    pub fn log_rollback_completed(&self, backup_path: &Path) {
        self.log(&UpdateEvent::RollbackCompleted {
            backup_path: backup_path.display().to_string(),
            duration: self.elapsed(),
        });
    }

    /// Log a failed rollback.
    pub fn log_rollback_failed(&self, error: &UpdateError) {
        self.log(&UpdateEvent::RollbackFailed {
            error: error.to_string(),
            duration: self.elapsed(),
        });
    }

    /// Log cleanup start.
    pub fn log_cleanup_started(&mut self) {
        self.start_operation("cleanup");
        self.log(&UpdateEvent::CleanupStarted);
    }

    /// Log cleanup completion.
    pub fn log_cleanup_completed(&self, files_removed: usize) {
        self.log(&UpdateEvent::CleanupCompleted { files_removed });
    }
}

/// Global convenience function to log an update error.
pub fn log_update_error(error: &UpdateError, context: &str) {
    tracing::error!(
        target: "spoq::update",
        error_code = %error.error_code(),
        error_category = %error.category(),
        retryable = %error.is_retryable(),
        context = %context,
        "Update error: {}",
        error
    );
}

/// Global convenience function to log an update info message.
pub fn log_update_info(message: &str) {
    tracing::info!(target: "spoq::update", "{}", message);
}

/// Global convenience function to log an update debug message.
pub fn log_update_debug(message: &str) {
    tracing::debug!(target: "spoq::update", "{}", message);
}

/// Global convenience function to log an update warning.
pub fn log_update_warn(message: &str) {
    tracing::warn!(target: "spoq::update", "{}", message);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500 bytes");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1536), "1.50 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.00 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.00 GB");
    }

    #[test]
    fn test_event_levels() {
        let check_started = UpdateEvent::CheckStarted {
            current_version: "1.0.0".to_string(),
        };
        assert_eq!(check_started.level(), UpdateLogLevel::Debug);

        let check_completed = UpdateEvent::CheckCompleted {
            current_version: "1.0.0".to_string(),
            latest_version: "1.1.0".to_string(),
            update_available: true,
            duration: Duration::from_secs(1),
        };
        assert_eq!(check_completed.level(), UpdateLogLevel::Info);

        let check_failed = UpdateEvent::CheckFailed {
            error: "Network error".to_string(),
            error_code: "E_CONN_FAILED".to_string(),
            duration: Duration::from_secs(5),
        };
        assert_eq!(check_failed.level(), UpdateLogLevel::Error);
    }

    #[test]
    fn test_event_messages() {
        let event = UpdateEvent::CheckStarted {
            current_version: "1.0.0".to_string(),
        };
        assert!(event.message().contains("1.0.0"));

        let event = UpdateEvent::DownloadProgress {
            version: "1.1.0".to_string(),
            bytes_downloaded: 1024 * 1024,
            total_bytes: Some(10 * 1024 * 1024),
            percent: Some(10.0),
        };
        let msg = event.message();
        assert!(msg.contains("1.00 MB"));
        assert!(msg.contains("10.0%"));

        let event = UpdateEvent::InstallCompleted {
            version: "1.1.0".to_string(),
            binary_path: "/usr/local/bin/spoq".to_string(),
            backup_path: "/usr/local/bin/spoq.backup".to_string(),
            duration: Duration::from_millis(1500),
        };
        let msg = event.message();
        assert!(msg.contains("Successfully installed"));
        assert!(msg.contains("1.5s"));
    }

    #[test]
    fn test_logger_timing() {
        let mut logger = UpdateLogger::new();

        // Initially no operation
        assert!(logger.elapsed().is_zero());

        // Start an operation
        logger.start_operation("test");

        // Let some time pass
        std::thread::sleep(Duration::from_millis(10));

        // Elapsed should be > 0
        let elapsed = logger.elapsed();
        assert!(elapsed >= Duration::from_millis(10));
    }

    #[test]
    fn test_update_available_message() {
        let event = UpdateEvent::CheckCompleted {
            current_version: "1.0.0".to_string(),
            latest_version: "2.0.0".to_string(),
            update_available: true,
            duration: Duration::from_millis(500),
        };
        let msg = event.message();
        assert!(msg.contains("Update available"));
        assert!(msg.contains("v1.0.0 -> v2.0.0"));
    }

    #[test]
    fn test_no_update_message() {
        let event = UpdateEvent::CheckCompleted {
            current_version: "2.0.0".to_string(),
            latest_version: "2.0.0".to_string(),
            update_available: false,
            duration: Duration::from_millis(500),
        };
        let msg = event.message();
        assert!(msg.contains("Already up to date"));
    }

    #[test]
    fn test_install_failed_messages() {
        let event_restored = UpdateEvent::InstallFailed {
            version: "1.1.0".to_string(),
            error: "Permission denied".to_string(),
            error_code: "E_PERMISSION".to_string(),
            restored: true,
            duration: Duration::from_secs(2),
        };
        let msg = event_restored.message();
        assert!(msg.contains("previous version restored"));

        let event_not_restored = UpdateEvent::InstallFailed {
            version: "1.1.0".to_string(),
            error: "Permission denied".to_string(),
            error_code: "E_PERMISSION".to_string(),
            restored: false,
            duration: Duration::from_secs(2),
        };
        let msg = event_not_restored.message();
        assert!(msg.contains("WARNING: restore failed"));
    }

    #[test]
    fn test_download_progress_without_total() {
        let event = UpdateEvent::DownloadProgress {
            version: "1.1.0".to_string(),
            bytes_downloaded: 5 * 1024 * 1024,
            total_bytes: None,
            percent: None,
        };
        let msg = event.message();
        assert!(msg.contains("5.00 MB"));
        assert!(!msg.contains("%"));
    }

    #[test]
    fn test_logger_default() {
        let logger = UpdateLogger::default();
        assert!(logger.operation_start.is_none());
        assert!(logger.operation_name.is_none());
    }
}
