//! Update command for Spoq CLI.
//!
//! Handles manual update checking, downloading, and installation.

use crate::update::{
    check_for_update, cleanup_backup, detect_platform, download_binary, install_update,
};
use color_eyre::Result;

/// Handle the --update flag for manual update check and installation.
///
/// This function runs the complete update flow:
/// 1. Check for available updates
/// 2. Download the update if available
/// 3. Install the update
/// 4. Exit with success or error message
///
/// # Errors
///
/// Returns an error if the tokio runtime cannot be created.
/// Other errors are handled internally with appropriate exit codes.
pub fn handle_update_command() -> Result<()> {
    // Create a runtime for async operations
    let runtime = tokio::runtime::Runtime::new()?;

    runtime.block_on(async {
        // Step 1: Check for updates
        println!("Checking for updates...");
        let _ = std::io::Write::flush(&mut std::io::stdout());
        let check_result = match check_for_update().await {
            Ok(result) => result,
            Err(e) => {
                eprintln!("Error checking for updates: {}", e);
                std::process::exit(1);
            }
        };

        if !check_result.update_available {
            println!(
                "You are already running the latest version ({}).",
                check_result.current_version
            );
            std::process::exit(0);
        }

        println!(
            "Update available: {} -> {}",
            check_result.current_version, check_result.latest_version
        );

        // Step 2: Download the update
        println!("Downloading update...");
        let platform = match detect_platform() {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Error detecting platform: {}", e);
                std::process::exit(1);
            }
        };

        let download_result =
            match download_binary(platform, Some(&check_result.latest_version)).await {
                Ok(result) => result,
                Err(e) => {
                    eprintln!("Error downloading update: {}", e);
                    std::process::exit(1);
                }
            };

        println!(
            "Downloaded {} bytes to {}",
            download_result.file_size,
            download_result.file_path.display()
        );

        // Step 3: Install the update
        println!("Installing update...");
        let install_result = match install_update(
            &download_result.file_path,
            Some(&check_result.latest_version),
        ) {
            Ok(result) => result,
            Err(e) => {
                eprintln!("Error installing update: {}", e);
                eprintln!("The downloaded update is still available for manual installation.");
                std::process::exit(1);
            }
        };

        println!(
            "Successfully updated to version {}!",
            check_result.latest_version
        );
        println!("Backup saved to: {}", install_result.backup_path.display());
        println!("\nRestart spoq to use the new version.");

        // Clean up old backups (optional - keep the most recent)
        let _ = cleanup_backup();

        std::process::exit(0);
    })
}
