//! Update module for Spoq CLI.
//!
//! This module provides functionality for auto-updating the CLI:
//! - Checking for available updates by comparing versions
//! - Downloading binary updates from platform-specific URLs
//! - Tracking update state
//!
//! # Example
//!
//! ```ignore
//! use spoq::update::{detect_platform, download_binary};
//!
//! let platform = detect_platform()?;
//! let result = download_binary(platform, Some("0.2.0")).await?;
//! println!("Downloaded {} bytes to {}", result.file_size, result.file_path.display());
//! ```

mod checker;
mod downloader;
pub mod state;

pub use checker::{
    check_for_update, compare_versions, UpdateCheckError, UpdateCheckResult, VersionInfo,
};
pub use downloader::{
    cleanup_old_updates, detect_platform, download_binary, download_binary_with_client,
    download_from_url, get_download_path, get_pending_update_path, get_update_temp_dir,
    has_pending_update, DownloadError, DownloadResult, Platform, DOWNLOAD_BASE_URL,
};
pub use state::{UpdateState, UpdateStateManager};
