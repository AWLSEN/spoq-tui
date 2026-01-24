//! Sync command for Spoq CLI.
//!
//! Handles token synchronization from local machine to VPS.

use color_eyre::Result;
use std::process::Command;

use crate::auth::central_api::{CentralApiClient, VpsStatusResponse};
use crate::auth::credentials::Credentials;
use crate::auth::{detect_tokens, export_tokens, wait_for_claude_code_token, CredentialsManager};

/// Fetch VPS state from server (always fetch from API - never use local cache).
///
/// # Arguments
/// * `runtime` - Tokio runtime for async operations
/// * `credentials` - Current credentials (for auth token only)
///
/// # Returns
/// * `Ok(Some(vps))` - VPS exists on server
/// * `Ok(None)` - No VPS on server (provisioning needed)
/// * `Err(CentralApiError)` - Server check failed
fn fetch_vps_from_api(
    runtime: &tokio::runtime::Runtime,
    credentials: &Credentials,
) -> Result<Option<VpsStatusResponse>, crate::auth::central_api::CentralApiError> {
    let mut client = if let Some(ref token) = credentials.access_token {
        CentralApiClient::new().with_auth(token)
    } else {
        CentralApiClient::new()
    };

    runtime.block_on(client.fetch_user_vps())
}

/// Handle the /sync or --sync command for token migration to VPS.
///
/// This function runs the complete token migration flow:
/// 1. Verify credentials are loaded and VPS exists (via API)
/// 2. Detect available tokens (GitHub CLI, Claude Code, Codex)
/// 3. Check for Claude Code token (retry loop if missing)
/// 4. Export tokens to archive
/// 5. Transfer archive to VPS via SSH
/// 6. Exit with success or error message
///
/// # Errors
///
/// Returns an error if the tokio runtime cannot be created.
/// Other errors are handled internally with appropriate exit codes.
pub fn handle_sync_command() -> Result<()> {
    println!("Running token synchronization...\n");

    // Step 1: Load credentials and verify VPS exists
    println!("[1/5] Verifying credentials and VPS...");
    let manager = CredentialsManager::new().expect("Failed to initialize credentials manager");
    let credentials = manager.load();

    if !credentials.has_token() {
        eprintln!("Error: Not authenticated. Please run spoq to authenticate first.");
        std::process::exit(1);
    }

    // Fetch VPS state from API (single source of truth)
    let runtime = tokio::runtime::Runtime::new()?;
    let vps_state = match fetch_vps_from_api(&runtime, &credentials) {
        Ok(Some(vps)) => vps,
        Ok(None) => {
            eprintln!("Error: No VPS configured. Please run spoq to provision a VPS first.");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Error: Cannot verify VPS status: {}", e);
            std::process::exit(1);
        }
    };

    let vps_ip = vps_state.ip.as_ref().expect("VPS must have IP address");
    println!("  Credentials loaded");
    println!("  VPS found: {}", vps_ip);

    // Step 2: Detect tokens
    println!("\n[2/5] Detecting tokens...");
    let detection = match detect_tokens() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error detecting tokens: {}", e);
            std::process::exit(1);
        }
    };

    let mut tokens_found = Vec::new();
    if detection.github_cli {
        tokens_found.push("GitHub CLI");
    }
    if detection.claude_code {
        tokens_found.push("Claude Code");
    }
    if detection.codex {
        tokens_found.push("Codex");
    }

    if tokens_found.is_empty() {
        println!("  No tokens detected");
    } else {
        println!("  Tokens detected: {}", tokens_found.join(", "));
    }

    // Step 3: Check for Claude Code token with retry loop
    if !detection.claude_code {
        println!("\n[3/5] Waiting for Claude Code token...");
        if let Err(e) = wait_for_claude_code_token() {
            eprintln!("Error: {}", e);
            eprintln!("Token sync requires Claude Code token. Please login and try again.");
            std::process::exit(1);
        }
        println!("  Claude Code token detected");
    } else {
        println!("\n[3/5] Claude Code token verified");
    }

    // Step 4: Export tokens to archive
    println!("\n[4/5] Exporting tokens...");
    let export_result = match export_tokens() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error exporting tokens: {}", e);
            std::process::exit(1);
        }
    };

    println!(
        "  Archive created: {} ({} bytes)",
        export_result.archive_path.display(),
        export_result.size_bytes
    );

    // Step 5: Transfer to VPS via SSH
    println!("\n[5/5] Transferring to VPS...");
    let archive_path = export_result.archive_path;
    let remote_path = "/tmp/spoq-tokens.tar.gz";

    // Use scp to transfer the archive
    let scp_output = Command::new("scp")
        .arg("-o")
        .arg("StrictHostKeyChecking=no")
        .arg("-o")
        .arg("UserKnownHostsFile=/dev/null")
        .arg(&archive_path)
        .arg(format!("root@{}:{}", vps_ip, remote_path))
        .output();

    match scp_output {
        Ok(output) if output.status.success() => {
            println!("  Archive transferred to VPS: {}", remote_path);
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("Error: SCP transfer failed");
            eprintln!("{}", stderr);
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Error: Failed to execute scp: {}", e);
            eprintln!("Make sure scp is installed and SSH access to the VPS is configured.");
            std::process::exit(1);
        }
    }

    // Extract the archive on the VPS
    println!("Extracting archive on VPS...");
    let ssh_output = Command::new("ssh")
        .arg("-o")
        .arg("StrictHostKeyChecking=no")
        .arg("-o")
        .arg("UserKnownHostsFile=/dev/null")
        .arg(format!("root@{}", vps_ip))
        .arg("bash")
        .arg("-c")
        .arg(format!(
            "cd /tmp && tar -xzf {} && rm {}",
            remote_path, remote_path
        ))
        .output();

    match ssh_output {
        Ok(output) if output.status.success() => {
            println!("  Archive extracted successfully");
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("Warning: Archive extraction had issues");
            eprintln!("{}", stderr);
        }
        Err(e) => {
            eprintln!("Warning: Failed to extract archive: {}", e);
        }
    }

    // Clean up local archive
    if let Err(e) = std::fs::remove_file(&archive_path) {
        eprintln!("Warning: Failed to clean up local archive: {}", e);
    }

    println!("\nToken synchronization complete!");
    println!("Tokens have been transferred to your VPS at {}", vps_ip);

    Ok(())
}
