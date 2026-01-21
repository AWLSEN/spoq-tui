//! Update module for Spoq CLI.
//!
//! This module provides functionality for auto-updating the CLI:
//! - Checking for available updates by comparing versions
//! - Downloading binary updates from platform-specific URLs
//! - Installing updates with backup and rollback support
//! - Tracking update state
//! - Comprehensive error handling with user-friendly messages
//! - Detailed logging of update operations
//!
//! # Example
//!
//! ```ignore
//! use spoq::update::{detect_platform, download_binary_logged, UpdateError};
//!
//! let platform = detect_platform()?;
//! match download_binary_logged(platform, Some("0.2.0")).await {
//!     Ok(result) => {
//!         println!("Downloaded {} bytes to {}", result.file_size, result.file_path.display());
//!     }
//!     Err(e) => {
//!         // Show user-friendly error message
//!         eprintln!("{}", e.user_message());
//!     }
//! }
//! ```
//!
//! # Error Handling
//!
//! All update operations return detailed error types that can be:
//! - Classified by category (network, permission, disk space, etc.)
//! - Checked for retryability
//! - Converted to user-friendly messages
//!
//! ```ignore
//! use spoq::update::{check_for_update_logged, UpdateError};
//!
//! match check_for_update_logged().await {
//!     Ok(result) if result.update_available => {
//!         println!("Update available: {}", result.latest_version);
//!     }
//!     Ok(_) => println!("Already up to date"),
//!     Err(e) => {
//!         if e.is_retryable() {
//!             println!("Temporary error, please try again: {}", e.user_message());
//!         } else {
//!             println!("Error: {}", e.user_message());
//!         }
//!     }
//! }
//! ```

mod checker;
mod downloader;
pub mod errors;
mod installer;
pub mod logger;
pub mod state;

// Original exports (kept for backward compatibility)
pub use checker::{
    check_for_update, compare_versions, UpdateCheckError, UpdateCheckResult, VersionInfo,
};
pub use downloader::{
    cleanup_old_updates, detect_platform, download_binary, download_binary_with_client,
    download_from_url, get_download_path, get_pending_update_path, get_update_temp_dir,
    has_pending_update, DownloadError, DownloadResult, Platform, DOWNLOAD_BASE_URL,
};
pub use installer::{
    cleanup_backup, cleanup_backup_at_path, has_backup, has_backup_at_path, install_update,
    install_update_with_config, rollback_update, rollback_update_with_paths, InstallConfig,
    InstallError, InstallResult,
};
pub use state::{UpdateState, UpdateStateManager};

// Enhanced exports with logging and unified error handling
pub use checker::{check_for_update_logged, check_for_update_logged_with_url};
pub use downloader::{
    cleanup_old_updates_logged, download_binary_logged, download_binary_logged_with_client,
    download_from_url_logged,
};
pub use errors::{classify_io_error, classify_reqwest_error, UpdateError, UpdateErrorCategory};
pub use installer::{
    install_update_logged, install_update_logged_with_config, rollback_update_logged,
    rollback_update_logged_with_paths,
};
pub use logger::{
    log_update_debug, log_update_error, log_update_info, log_update_warn, UpdateEvent,
    UpdateLogLevel, UpdateLogger,
};
