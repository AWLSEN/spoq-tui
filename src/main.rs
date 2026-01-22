use spoq::app::{start_websocket_with_config, App, AppMessage, Focus, Screen, ScrollBoundary};
use spoq::websocket::WsClientConfig;
use spoq::auth::{run_auth_flow, run_provisioning_flow, start_stopped_vps, CredentialsManager};
use spoq::auth::central_api::{CentralApiClient, VpsStatusResponse};
use spoq::auth::credentials::Credentials;
use spoq::debug::{
    create_debug_channel, start_debug_server, DebugEvent, DebugEventKind, StateChangeData, StateType,
};
use spoq::models;
use spoq::ui;

use color_eyre::Result;
use crossterm::{
    cursor::Show,
    event::{
        DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        Event, EventStream, KeyCode, KeyEventKind, KeyModifiers, KeyboardEnhancementFlags,
        MouseButton, MouseEventKind, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::StreamExt;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Background update check and download on startup.
///
/// This function runs non-blocking in the background:
/// 1. Load update state to check last check time
/// 2. Check for available updates (respecting rate limiting)
/// 3. Download the update if available
/// 4. Store the pending update path in state for next launch
///
/// Errors are silently ignored to avoid disrupting the user experience.
async fn check_and_download_update() {
    use spoq::update::{
        check_for_update, detect_platform, download_binary, UpdateStateManager,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    // Load update state to check when we last checked
    let state_manager = match UpdateStateManager::new() {
        Some(mgr) => mgr,
        None => return, // Can't determine home dir - skip update check
    };

    let mut state = state_manager.load();

    // Rate limit: only check for updates once per 24 hours
    const CHECK_INTERVAL_SECONDS: i64 = 24 * 60 * 60;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    if let Some(last_check) = state.last_check {
        if now - last_check < CHECK_INTERVAL_SECONDS {
            // Too soon since last check - skip
            return;
        }
    }

    // Update last check time
    state.last_check = Some(now);
    let _ = state_manager.save(&state);

    // Step 1: Check for updates
    let check_result = match check_for_update().await {
        Ok(result) => result,
        Err(_) => return, // Network error or API down - silently skip
    };

    if !check_result.update_available {
        // Already on latest version
        return;
    }

    // Step 2: Download the update
    let platform = match detect_platform() {
        Ok(p) => p,
        Err(_) => return, // Unsupported platform - skip
    };

    let download_result = match download_binary(platform, Some(&check_result.latest_version)).await
    {
        Ok(result) => result,
        Err(_) => return, // Download failed - silently skip
    };

    // Step 3: Store the pending update path in state
    state.pending_update_path = Some(download_result.file_path.to_string_lossy().to_string());
    state.available_version = Some(check_result.latest_version);
    let _ = state_manager.save(&state);

    // Update is now ready for installation on next launch
    // User will see notification in TUI or can run `spoq --update` manually
}

/// Handle the /sync or --sync command for token migration to VPS.
///
/// This function runs the complete token migration flow:
/// 1. Verify credentials are loaded and VPS exists
/// 2. Detect available tokens (GitHub CLI, Claude Code, Codex)
/// 3. Check for Claude Code token (retry loop if missing)
/// 4. Export tokens to archive
/// 5. Transfer archive to VPS via SSH
/// 6. Exit with success or error message
fn handle_sync_command() -> Result<()> {
    use spoq::auth::{detect_tokens, export_tokens, wait_for_claude_code_token, CredentialsManager};
    use std::process::Command;

    println!("Running token synchronization...\n");

    // Step 1: Load credentials and verify VPS exists
    println!("[1/5] Verifying credentials and VPS...");
    let manager = CredentialsManager::new().expect("Failed to initialize credentials manager");
    let credentials = manager.load();

    if !credentials.has_token() {
        eprintln!("Error: Not authenticated. Please run spoq to authenticate first.");
        std::process::exit(1);
    }

    if !credentials.has_vps() {
        eprintln!("Error: No VPS configured. Please run spoq to provision a VPS first.");
        std::process::exit(1);
    }

    let vps_ip = credentials.vps_ip.as_ref().expect("VPS IP should exist");
    println!("✓ Credentials loaded");
    println!("✓ VPS found: {}", vps_ip);

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
        println!("⚠ No tokens detected");
    } else {
        println!("✓ Tokens detected: {}", tokens_found.join(", "));
    }

    // Step 3: Check for Claude Code token with retry loop
    if !detection.claude_code {
        println!("\n[3/5] Waiting for Claude Code token...");
        if let Err(e) = wait_for_claude_code_token() {
            eprintln!("Error: {}", e);
            eprintln!("Token sync requires Claude Code token. Please login and try again.");
            std::process::exit(1);
        }
        println!("✓ Claude Code token detected");
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
        "✓ Archive created: {} ({} bytes)",
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
            println!("✓ Archive transferred to VPS: {}", remote_path);
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
            println!("✓ Archive extracted successfully");
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

    println!("\n✓ Token synchronization complete!");
    println!("Tokens have been transferred to your VPS at {}", vps_ip);

    Ok(())
}

/// Handle the --update flag for manual update check and installation.
///
/// This function runs the complete update flow:
/// 1. Check for available updates
/// 2. Download the update if available
/// 3. Install the update
/// 4. Exit with success or error message
fn handle_manual_update() -> Result<()> {
    use spoq::update::{
        check_for_update, cleanup_backup, detect_platform, download_binary, install_update,
    };

    // Create a runtime for async operations
    let runtime = tokio::runtime::Runtime::new()?;

    runtime.block_on(async {
        // Step 1: Check for updates
        println!("Checking for updates...");
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

/// Attempt to refresh an expired access token using the refresh token.
///
/// # Arguments
/// * `runtime` - Tokio runtime for async operations
/// * `credentials` - Current credentials with refresh_token
///
/// # Returns
/// * `Ok(Credentials)` - New credentials with refreshed tokens
/// * `Err(CentralApiError)` - Refresh failed (trigger re-auth)
fn attempt_token_refresh(
    runtime: &tokio::runtime::Runtime,
    credentials: &Credentials,
) -> Result<Credentials, spoq::auth::central_api::CentralApiError> {
    use spoq::auth::central_api::{CentralApiError, get_jwt_expires_in};

    let refresh_token = credentials.refresh_token.as_ref()
        .ok_or_else(|| CentralApiError::ServerError {
            status: 0,
            message: "No refresh token available".to_string(),
        })?;

    // Log refresh attempt with first 10 chars of refresh token for debugging
    let token_preview = if refresh_token.len() > 10 {
        &refresh_token[..10]
    } else {
        refresh_token
    };
    println!(
        "[TOKEN] Token expired, attempting refresh with refresh_token={}...",
        token_preview
    );

    let client = CentralApiClient::new();

    let refresh_response = runtime.block_on(client.refresh_token(refresh_token))?;

    // Build new credentials with refreshed tokens
    let mut new_credentials = credentials.clone();
    new_credentials.access_token = Some(refresh_response.access_token.clone());

    // Update refresh token if server provided a new one
    if let Some(new_refresh) = refresh_response.refresh_token {
        new_credentials.refresh_token = Some(new_refresh);
    }

    // Calculate expiration from response or JWT
    let expires_in = refresh_response
        .expires_in
        .or_else(|| get_jwt_expires_in(&refresh_response.access_token))
        .unwrap_or(900); // Default 15 minutes
    let new_expires_at = chrono::Utc::now().timestamp() + expires_in as i64;
    new_credentials.expires_at = Some(new_expires_at);

    println!(
        "[TOKEN] Token refresh successful, new expires_at={}, saved to credentials",
        new_expires_at
    );

    Ok(new_credentials)
}

/// Fetch VPS status from API for cases where vps_status field is missing.
///
/// # Arguments
/// * `runtime` - Tokio runtime for async operations
/// * `credentials` - Current credentials with access token
///
/// # Returns
/// * `Ok(VpsStatusResponse)` - VPS status from API
/// * `Err(CentralApiError)` - API call failed
fn fetch_vps_status(
    runtime: &tokio::runtime::Runtime,
    credentials: &Credentials,
) -> Result<VpsStatusResponse, spoq::auth::central_api::CentralApiError> {
    let mut client = if let Some(ref token) = credentials.access_token {
        CentralApiClient::new().with_auth(token)
    } else {
        CentralApiClient::new()
    };
    runtime.block_on(client.fetch_vps_status())
}

/// Sync VPS state between local credentials and server.
///
/// Checks if server has a VPS when local credentials don't, and syncs if needed.
///
/// # Arguments
/// * `runtime` - Tokio runtime for async operations
/// * `credentials` - Current credentials (will be modified if sync needed)
/// * `manager` - Credentials manager for saving
///
/// # Returns
/// * `Ok(true)` - Provisioning needed (no VPS on server)
/// * `Ok(false)` - VPS synced from server (no provisioning needed)
/// * `Err(CentralApiError)` - Server check failed
fn sync_vps_state(
    runtime: &tokio::runtime::Runtime,
    credentials: &mut Credentials,
    manager: &CredentialsManager,
) -> Result<bool, spoq::auth::central_api::CentralApiError> {
    println!("Checking VPS status...");

    let mut client = if let Some(ref token) = credentials.access_token {
        CentralApiClient::new().with_auth(token)
    } else {
        CentralApiClient::new()
    };

    match runtime.block_on(client.fetch_user_vps())? {
        Some(server_vps) => {
            // Server has VPS - sync local credentials
            println!("Found existing VPS on server. Syncing local credentials...");
            credentials.vps_id = Some(server_vps.vps_id.clone());

            // Always prefer hostname for HTTPS URL (server may return internal :8000 URL)
            credentials.vps_url = server_vps.hostname.as_ref()
                .map(|h| format!("https://{}", h))
                .or_else(|| server_vps.url.clone());

            credentials.vps_hostname = server_vps.hostname.clone();
            credentials.vps_ip = server_vps.ip.clone();
            credentials.vps_status = Some(server_vps.status.clone());

            if !manager.save(credentials) {
                eprintln!("Warning: Failed to save synced VPS credentials");
            }

            println!("VPS credentials synced successfully.");
            Ok(false) // No provisioning needed
        }
        None => {
            // Server confirms no VPS
            Ok(true) // Provisioning needed
        }
    }
}

fn main() -> Result<()> {
    // Handle --version flag before any initialization
    if std::env::args().any(|arg| arg == "--version") {
        println!("spoq {}", VERSION);
        std::process::exit(0);
    }

    // Handle --update flag for manual update check
    if std::env::args().any(|arg| arg == "--update") {
        return handle_manual_update();
    }

    // Handle /sync or --sync flag for token migration
    if std::env::args().any(|arg| arg == "/sync" || arg == "--sync") {
        return handle_sync_command();
    }

    color_eyre::install()?;

    // Setup panic hook to ensure terminal cleanup on panic
    setup_panic_hook();

    // Create Tokio runtime for the entire application
    // This runtime will be used for auth flows and then for TUI async operations
    let runtime = tokio::runtime::Runtime::new()?;

    // =========================================================
    // Pre-flight auth checks - run BEFORE TUI starts
    // =========================================================

    // Load or create credentials
    let manager = CredentialsManager::new().expect("Failed to initialize credentials manager");
    let mut credentials = manager.load();

    // =========================================================
    // Auth check - validate token and refresh if needed
    // =========================================================

    if credentials.access_token.is_none() {
        // No token at all - run full auth flow
        credentials = match run_auth_flow(&runtime) {
            Ok(creds) => creds,
            Err(e) => {
                eprintln!("Authentication failed: {}", e);
                std::process::exit(1);
            }
        };
        // Save credentials after auth
        if !manager.save(&credentials) {
            eprintln!("Warning: Failed to save credentials after authentication");
        }
    } else if credentials.is_expired() {
        // Token exists but expired - try to refresh
        let now = chrono::Utc::now().timestamp();
        let expires_at = credentials.expires_at.unwrap_or(0);
        println!(
            "[TOKEN] Checking token expiration: expires_at={}, current={}, expired=true",
            expires_at, now
        );

        match attempt_token_refresh(&runtime, &credentials) {
            Ok(refreshed) => {
                credentials = refreshed;
                if !manager.save(&credentials) {
                    eprintln!("Warning: Failed to save refreshed credentials");
                } else {
                    // CRITICAL: Reload credentials from disk to ensure health check uses fresh tokens
                    // This prevents TOCTOU race where health check reads stale in-memory credentials
                    credentials = manager.load();
                    println!("[TOKEN] Credentials reloaded from disk after refresh to prevent TOCTOU race");
                }
            }
            Err(e) => {
                println!("[TOKEN] Token refresh failed: {}, falling back to full auth flow", e);
                eprintln!("Token refresh failed: {}. Re-authenticating...", e);
                credentials = match run_auth_flow(&runtime) {
                    Ok(creds) => creds,
                    Err(e) => {
                        eprintln!("Authentication failed: {}", e);
                        std::process::exit(1);
                    }
                };
                if !manager.save(&credentials) {
                    eprintln!("Warning: Failed to save credentials after authentication");
                } else {
                    // Reload after re-auth as well to ensure consistency
                    credentials = manager.load();
                    println!("[TOKEN] Credentials reloaded from disk after re-authentication");
                }
            }
        }
    } else {
        // Token exists and is valid
        let now = chrono::Utc::now().timestamp();
        let expires_at = credentials.expires_at.unwrap_or(0);
        let time_remaining = expires_at - now;
        println!(
            "[TOKEN] Checking token expiration: expires_at={}, current={}, expired=false, time_remaining={}s",
            expires_at, now, time_remaining
        );
    }

    // =========================================================
    // Token verification - check Claude Code and GitHub CLI
    // =========================================================

    println!("Verifying local tokens...");
    match spoq::auth::verify_local_tokens() {
        Ok(verification) => {
            if verification.all_required_present {
                println!("✓ Required tokens verified (Claude Code, GitHub CLI)");
            } else {
                eprintln!("\n⚠️  Warning: Required tokens missing on local machine:");
                if !verification.claude_code_present {
                    eprintln!("  ✗ Claude Code - not found. Run: claude login");
                }
                if !verification.github_cli_present {
                    eprintln!("  ✗ GitHub CLI - not found. Run: gh auth login");
                }
                eprintln!("\nThese tokens are required for VPS provisioning.");
                eprintln!("You can continue, but provisioning will fail without them.\n");
            }
        }
        Err(e) => {
            eprintln!("⚠️  Warning: Could not verify local tokens: {}", e);
            eprintln!("Continuing anyway, but VPS provisioning may fail.\n");
        }
    }

    // =========================================================
    // VPS check - ensure VPS exists and is usable
    // =========================================================

    if !credentials.has_vps() {
        // Local credentials show no VPS - check server state first
        match sync_vps_state(&runtime, &mut credentials, &manager) {
            Ok(true) => {
                // Provisioning needed (server confirms no VPS)
                if let Err(e) = run_provisioning_flow(&runtime, &mut credentials) {
                    eprintln!("Provisioning failed: {}", e);
                    std::process::exit(1);
                }
                if !manager.save(&credentials) {
                    eprintln!("Warning: Failed to save credentials after provisioning");
                }
            }
            Ok(false) => {
                // VPS synced from server - continue to status checks below
            }
            Err(e) => {
                eprintln!("Error: Cannot verify VPS status: {}", e);
                eprintln!("Please check your network connection and try again.");
                std::process::exit(1);
            }
        }
    }

    if credentials.has_vps() {
        // VPS exists - check its status
        match credentials.vps_status.as_deref() {
            Some("ready") | Some("running") | Some("active") => {
                // Good to go - launch TUI
            }
            Some("stopped") => {
                // Auto-start existing VPS
                match start_stopped_vps(&runtime, &credentials) {
                    Ok(status) => {
                        credentials.vps_status = Some(status.status);
                        credentials.vps_ip = status.ip;
                        if !manager.save(&credentials) {
                            eprintln!("Warning: Failed to save credentials after starting VPS");
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to start VPS: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            Some("failed") | Some("terminated") => {
                // Failed VPS - don't auto-reprovision, show error
                eprintln!("Error: VPS is in failed state (status: {}).",
                    credentials.vps_status.as_deref().unwrap_or("unknown"));
                eprintln!("Your VPS cannot be started automatically.");
                eprintln!("Please contact support@spoq.dev for assistance.");
                std::process::exit(1);
            }
            None => {
                // VPS exists but status field missing (legacy credentials)
                // Fetch status from API instead of re-provisioning
                match fetch_vps_status(&runtime, &credentials) {
                    Ok(status) => {
                        credentials.vps_status = Some(status.status.clone());
                        if let Some(ip) = status.ip {
                            credentials.vps_ip = Some(ip);
                        }
                        if !manager.save(&credentials) {
                            eprintln!("Warning: Failed to save updated VPS status");
                        }
                        // Re-check status after fetching
                        match status.status.as_str() {
                            "ready" | "running" | "active" => {
                                // Good to go
                            }
                            "stopped" => {
                                // Need to start it
                                match start_stopped_vps(&runtime, &credentials) {
                                    Ok(status) => {
                                        credentials.vps_status = Some(status.status);
                                        credentials.vps_ip = status.ip;
                                        if !manager.save(&credentials) {
                                            eprintln!("Warning: Failed to save credentials");
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("Failed to start VPS: {}", e);
                                        std::process::exit(1);
                                    }
                                }
                            }
                            "failed" | "terminated" => {
                                eprintln!("Error: VPS is in failed state (status: {}).", status.status);
                                eprintln!("Your VPS cannot be started automatically.");
                                eprintln!("Please contact support@spoq.dev for assistance.");
                                std::process::exit(1);
                            }
                            other => {
                                eprintln!("VPS in unexpected state: {}. Please wait or contact support.", other);
                                std::process::exit(1);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Error: Cannot determine VPS status: {}", e);
                        eprintln!("Please check your network connection and try again.");
                        std::process::exit(1);
                    }
                }
            }
            Some(other) => {
                // Unknown state
                eprintln!("VPS in unexpected state: {}. Please wait or contact support.", other);
                std::process::exit(1);
            }
        }
    }

    // =========================================================
    // VPS health check - verify conductor and tokens
    // =========================================================
    if credentials.has_vps() {
        println!("\nRunning VPS health checks...\n");

        let mut first_attempt = true;

        // Keep checking until VPS is ready
        loop {
            // Run health checks
            let health_result = runtime.block_on(
                spoq::health_check::run_health_checks(&credentials)
            );

            // If tokens are missing on first attempt, try to auto-sync
            if first_attempt && health_result.should_block {
                first_attempt = false;

                println!("⚙️  Attempting to sync credentials to VPS...\n");

                // Attempt sync via conductor
                match &credentials.vps_url {
                    Some(url) => {
                        let mut conductor = spoq::conductor::ConductorClient::with_url(url);
                        if let Some(ref token) = credentials.access_token {
                            conductor = conductor.with_auth(token);
                        }
                        if let Some(ref refresh) = credentials.refresh_token {
                            conductor = conductor.with_refresh_token(refresh);
                        }

                        match runtime.block_on(conductor.sync_tokens("all")) {
                            Ok(_) => {
                                println!("✓ Sync initiated, verifying...\n");

                                // Reload credentials in case conductor auto-refreshed during sync
                                // This ensures we use the freshest tokens on retry
                                credentials = manager.load();
                                println!("[TOKEN] Credentials reloaded after sync (in case auto-refresh occurred)");

                                std::thread::sleep(std::time::Duration::from_secs(2));
                                continue; // Recheck immediately
                            }
                            Err(e) => {
                                println!("⚠️  Auto-sync failed: {}\n", e);
                            }
                        }
                    }
                    None => {}
                }
            }

            // Display results
            let vps_ip = credentials.vps_ip.as_deref();
            spoq::health_check::display_health_check_results(&health_result, vps_ip);

            // If all checks pass, break out of loop
            if !health_result.should_block {
                break;
            }

            // Wait for user input to retry
            println!("Press 'r' to retry verification, or Ctrl+C to exit.");

            use std::io::{self, BufRead};
            let stdin = io::stdin();
            let mut line = String::new();

            // Read user input
            match stdin.lock().read_line(&mut line) {
                Ok(_) => {
                    let input = line.trim().to_lowercase();
                    if input == "r" || input == "retry" {
                        // Reload credentials before retry in case user fixed tokens externally
                        credentials = manager.load();
                        println!("[TOKEN] Credentials reloaded before retry");
                        println!("\nRetrying VPS verification...\n");
                        continue;
                    } else {
                        println!("Invalid input. Press 'r' to retry.\n");
                    }
                }
                Err(_) => {
                    println!("Failed to read input. Exiting.\n");
                    std::process::exit(1);
                }
            }
        }
    }

    // =========================================================
    // Update check - run in background, non-blocking
    // =========================================================
    runtime.spawn(async {
        check_and_download_update().await;
    });

    // At this point, user is authenticated AND has a ready VPS
    println!("Starting SPOQ...\n");

    // =========================================================
    // TUI initialization - user is now authenticated
    // =========================================================

    // Run async initialization using the runtime
    let (debug_tx, debug_server_handle) = runtime.block_on(start_debug_system());

    // Open debug dashboard in browser (fire and forget)
    if debug_tx.is_some() {
        let _ = open::that("http://localhost:3030");
    }

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();

    // Enable keyboard enhancement for modern terminals (Kitty protocol)
    // This allows Ctrl+Enter and Shift+Enter to work properly
    // Silently fails on unsupported terminals (Terminal.app, Warp, etc.)
    let _ = execute!(
        stdout,
        PushKeyboardEnhancementFlags(
            KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
        )
    );

    // Enter alternate screen, enable bracketed paste, and mouse capture for scroll events
    // Note: Mouse capture is enabled but click events are ignored in the handler,
    // allowing scroll wheel to work while terminal handles text selection natively
    execute!(
        stdout,
        EnterAlternateScreen,
        EnableBracketedPaste,
        EnableMouseCapture
    )?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Clear the terminal
    terminal.clear()?;

    // Initialize application state with debug sender
    let mut app = App::with_debug(debug_tx)?;

    // Log initial auth state for debugging
    app.log_initial_auth_state();

    // Capture initial terminal dimensions
    let size = terminal.size()?;
    app.update_terminal_dimensions(size.width, size.height);

    // Initialize server connection - user is already authenticated with ready VPS
    // Login and Provisioning screens are handled by pre-flight checks above
    runtime.block_on(async {
        // Load threads from backend (async initialization)
        app.initialize().await;

        // Load folders for the folder picker (async, non-blocking)
        app.load_folders();

        // Connect WebSocket for real-time communication
        // Build config with token from credentials (not just env var)
        let ws_config = if let Some(ref token) = app.credentials.access_token {
            WsClientConfig::default().with_auth(token)
        } else {
            WsClientConfig::default()
        };

        // Emit debug event showing connection attempt
        if let Some(ref tx) = app.debug_tx {
            let _ = tx.send(DebugEvent::new(DebugEventKind::StateChange(
                StateChangeData::new(
                    StateType::WebSocket,
                    "WS_CONNECTING",
                    format!("Connecting to {} (has_token={})", ws_config.host, ws_config.auth_token.is_some()),
                ),
            )));
        }

        // If connection fails, app continues in SSE-only mode
        match start_websocket_with_config(app.message_tx.clone(), ws_config).await {
            Ok(sender) => {
                if let Some(ref tx) = app.debug_tx {
                    let _ = tx.send(DebugEvent::new(DebugEventKind::StateChange(
                        StateChangeData::new(StateType::WebSocket, "WS_INIT", "WebSocket connected successfully"),
                    )));
                }
                app.ws_sender = Some(sender);
            }
            Err(e) => {
                if let Some(ref tx) = app.debug_tx {
                    let _ = tx.send(DebugEvent::new(DebugEventKind::StateChange(
                        StateChangeData::new(StateType::WebSocket, "WS_INIT_FAILED", format!("WebSocket connection failed: {}", e)),
                    )));
                }
                app.ws_sender = None;
            }
        }
    });

    // Main event loop
    let result = runtime.block_on(run_app(&mut terminal, &mut app));

    // Before exiting, save input history
    app.input_history.save();

    // Restore terminal
    restore_terminal(&mut terminal)?;

    // Cleanup debug server if it was started
    if let Some(handle) = debug_server_handle {
        handle.abort();
    }

    result
}

/// Start the debug system (channel + server).
///
/// Returns the debug event sender and server handle if successful.
/// If the debug server fails to start, returns None for both - the app continues without debug.
async fn start_debug_system() -> (
    Option<spoq::debug::DebugEventSender>,
    Option<JoinHandle<()>>,
) {
    // Create debug channel with capacity for 1000 events
    let (debug_tx, _) = create_debug_channel(1000);

    // Try to start the debug server
    match start_debug_server(debug_tx.clone()).await {
        Ok((handle, _)) => {
            // Server started successfully
            (Some(debug_tx), Some(handle))
        }
        Err(_e) => {
            // Server failed to start - continue without debug
            // (e.g., port 3030 already in use)
            (None, None)
        }
    }
}

/// Setup panic hook to restore terminal on panic
fn setup_panic_hook() {
    use std::io::Write;
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        // Try to restore terminal state
        // Pop keyboard enhancement flags BEFORE disabling raw mode
        let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);

        let _ = disable_raw_mode();
        let _ = execute!(
            io::stdout(),
            DisableMouseCapture,
            DisableBracketedPaste,
            LeaveAlternateScreen
        );
        let _ = execute!(io::stdout(), Show);

        // CRITICAL: Hard reset Kitty keyboard protocol AFTER leaving alternate screen
        // Ghostty (and potentially other terminals) need this sent after leaving alternate screen
        // CSI = 0 u sets all keyboard enhancement flags to zero (non-stack based reset)
        let _ = write!(io::stdout(), "\x1b[=0u");
        let _ = io::stdout().flush();

        // Call the original panic hook
        original_hook(panic_info);
    }));
}

/// Restore terminal to normal mode
fn restore_terminal<B: ratatui::backend::Backend + std::io::Write>(
    terminal: &mut Terminal<B>,
) -> Result<()>
where
    B::Error: Send + Sync + 'static,
{
    // Pop keyboard enhancement flags (crossterm's standard approach)
    let _ = execute!(terminal.backend_mut(), PopKeyboardEnhancementFlags);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        DisableMouseCapture,
        DisableBracketedPaste,
        LeaveAlternateScreen
    )?;

    // CRITICAL: Hard reset Kitty keyboard protocol AFTER leaving alternate screen
    // Some terminals (Ghostty) need this sent after leaving alternate screen
    // CSI = 0 u sets all keyboard enhancement flags to zero (non-stack based reset)
    let _ = write!(terminal.backend_mut(), "\x1b[=0u");
    let _ = io::Write::flush(terminal.backend_mut());

    terminal.show_cursor()?;
    Ok(())
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<()>
where
    B::Error: Send + Sync + 'static,
{
    // Track migration progress animation
    let migration_start = tokio::time::Instant::now();
    const MIGRATION_DURATION_MS: u64 = 5000; // 5 seconds

    // Create async event stream for keyboard input
    let mut event_stream = EventStream::new();

    // Take the message receiver from the app (we need ownership for select!)
    let mut message_rx: Option<mpsc::UnboundedReceiver<AppMessage>> = app.message_rx.take();

    loop {
        // Update migration progress if it's running
        if app.migration_progress.is_some() {
            let elapsed_ms = migration_start.elapsed().as_millis() as u64;
            if elapsed_ms >= MIGRATION_DURATION_MS {
                // Migration complete, hide progress bar
                app.migration_progress = None;
                app.mark_dirty();
            } else {
                // Calculate progress percentage (0-100)
                let progress = ((elapsed_ms * 100) / MIGRATION_DURATION_MS) as u8;
                if app.migration_progress != Some(progress) {
                    app.migration_progress = Some(progress);
                    app.mark_dirty();
                }
            }
        }

        // Draw the UI only when needed (dirty flag or streaming)
        if app.needs_redraw || app.is_streaming() {
            terminal.draw(|f| {
                ui::render(f, &mut *app);
            })?;
            app.needs_redraw = false;
        }

        // Poll both keyboard events and message channel using tokio::select!
        // 16ms tick for smooth 60fps-like scrolling animation
        let timeout = tokio::time::sleep(std::time::Duration::from_millis(16));

        tokio::select! {
            // Handle timeout for UI updates (migration progress, animations, etc.)
            _ = timeout => {
                // Increment tick counter for animations (spinner, cursor blink)
                app.tick();

                // Check for thread switcher auto-confirm (Tab release simulation)
                app.check_switcher_timeout();
            }

            // Handle keyboard events
            event_result = event_stream.next() => {
                if let Some(Ok(event)) = event_result {
                    match event {
                        Event::Resize(width, height) => {
                            // Update app state with new terminal dimensions
                            app.update_terminal_dimensions(width, height);
                            // Redraw will happen on next loop iteration
                            continue;
                        }
                        Event::Key(key) if key.kind == KeyEventKind::Press => {
                            // Any key press likely changes state (input, navigation, etc.)
                            app.mark_dirty();

                            // DEBUG: Log ALL key events
                            app.emit_debug_state_change(
                                "KeyEvent",
                                &format!(
                                    "code={:?} mods={:?}",
                                    key.code, key.modifiers
                                ),
                                "",
                            );

                            // Global keybinds (always active)
                            match key.code {
                                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                    app.quit();
                                    return Ok(());
                                }
                                // Shift+Escape to return to CommandDeck from Conversation
                                // (kept for terminals that support it)
                                KeyCode::Esc if key.modifiers.contains(KeyModifiers::SHIFT) => {
                                    if app.screen == Screen::Conversation {
                                        app.navigate_to_command_deck();
                                    }
                                    continue;
                                }
                                // Ctrl+W to return to CommandDeck (explicit close/back binding)
                                KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                    if app.screen == Screen::Conversation {
                                        app.navigate_to_command_deck();
                                    }
                                    continue;
                                }
                                // Shift+N to create new thread
                                KeyCode::Char('N') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                                    app.create_new_thread();
                                    continue;
                                }
                                // CapsLock is tricky - use Ctrl+N as alternative
                                KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                    app.create_new_thread();
                                    continue;
                                }
                                // Alt+P to submit as Programming thread (from CommandDeck)
                                KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::ALT) => {
                                    if app.screen == Screen::CommandDeck && !app.textarea.is_empty() {
                                        app.submit_input(models::ThreadType::Programming);
                                    }
                                    continue;
                                }
                                _ => {}
                            }

                            // Handle permission prompt keys when a permission is pending
                            // This takes priority over all other key handling
                            if app.session_state.has_pending_permission() {
                                // Check if this is an AskUserQuestion prompt
                                // State is already initialized when permission is received
                                if app.is_ask_user_question_pending() {

                                    // Handle "Other" text input mode
                                    if app.question_state.other_active {
                                        match key.code {
                                            KeyCode::Esc => {
                                                app.question_cancel_other();
                                                continue;
                                            }
                                            KeyCode::Enter => {
                                                if app.question_confirm() {
                                                    continue;
                                                }
                                                continue;
                                            }
                                            KeyCode::Backspace => {
                                                app.question_backspace();
                                                continue;
                                            }
                                            KeyCode::Char(c) => {
                                                app.question_type_char(c);
                                                continue;
                                            }
                                            _ => continue,
                                        }
                                    }

                                    // Handle question navigation keys
                                    match key.code {
                                        KeyCode::Tab => {
                                            app.question_next_tab();
                                            continue;
                                        }
                                        KeyCode::Up => {
                                            app.question_prev_option();
                                            continue;
                                        }
                                        KeyCode::Down => {
                                            app.question_next_option();
                                            continue;
                                        }
                                        KeyCode::Char(' ') => {
                                            app.question_toggle_option();
                                            continue;
                                        }
                                        KeyCode::Enter => {
                                            app.question_confirm();
                                            continue;
                                        }
                                        KeyCode::Char('n') | KeyCode::Char('N') => {
                                            // Allow 'n' to deny/cancel
                                            if let Some(ref perm) = app.session_state.pending_permission.clone() {
                                                app.deny_permission(&perm.permission_id);
                                            }
                                            continue;
                                        }
                                        _ => continue,
                                    }
                                }

                                // Standard permission prompt (y/a/n)
                                if let KeyCode::Char(c) = key.code {
                                    // Debug: emit key press to debug system
                                    app.emit_debug_state_change(
                                        "permission_key",
                                        "Key pressed during permission",
                                        &format!("key: '{}', pending: true", c),
                                    );
                                    if app.handle_permission_key(c) {
                                        app.emit_debug_state_change(
                                            "permission_key",
                                            "Permission handled",
                                            &format!("key: '{}' -> handled", c),
                                        );
                                        continue;
                                    }
                                    app.emit_debug_state_change(
                                        "permission_key",
                                        "Key not handled",
                                        &format!("key: '{}' -> not Y/N/A", c),
                                    );
                                }
                                // When permission is pending, ignore all other keys except Ctrl+C
                                continue;
                            }

                            // =========================================================
                            // Folder Picker Key Handling (HIGHEST PRIORITY when visible)
                            // Must come BEFORE thread switcher to capture typed characters
                            // =========================================================
                            if app.folder_picker_visible {
                                match key.code {
                                    KeyCode::Esc => {
                                        // Close picker, remove @ + filter from input
                                        app.remove_at_and_filter_from_input();
                                        app.close_folder_picker();
                                        continue;
                                    }
                                    KeyCode::Enter => {
                                        // Select folder, close picker, clear @ + filter
                                        // The @ and filter text should be removed since we're selecting
                                        app.remove_at_and_filter_from_input();
                                        app.folder_picker_select();
                                        continue;
                                    }
                                    KeyCode::Backspace => {
                                        if app.folder_picker_backspace() {
                                            // Filter was empty, close picker and remove @
                                            app.textarea.backspace(); // Remove the @
                                            app.close_folder_picker();
                                        }
                                        continue;
                                    }
                                    KeyCode::Up => {
                                        app.folder_picker_cursor_up();
                                        continue;
                                    }
                                    KeyCode::Down => {
                                        app.folder_picker_cursor_down();
                                        continue;
                                    }
                                    KeyCode::Char(c) if !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER) => {
                                        // Append character to filter
                                        app.folder_picker_type_char(c);
                                        continue;
                                    }
                                    _ => {
                                        // Other keys are ignored while picker is open
                                        continue;
                                    }
                                }
                            }

                            // Thread switcher handling (takes priority when visible)
                            if app.thread_switcher.visible {
                                match key.code {
                                    KeyCode::Tab | KeyCode::Down => {
                                        app.cycle_switcher_forward();
                                        continue;
                                    }
                                    KeyCode::Up => {
                                        app.cycle_switcher_backward();
                                        continue;
                                    }
                                    KeyCode::Esc => {
                                        app.close_switcher();
                                        continue;
                                    }
                                    KeyCode::Enter => {
                                        app.confirm_switcher_selection();
                                        continue;
                                    }
                                    _ => {
                                        // Any other key closes and confirms
                                        app.confirm_switcher_selection();
                                        continue;
                                    }
                                }
                            }

                            // Handle OAuth consent 'o' key to open URL in browser
                            if let KeyCode::Char('o') = key.code {
                                if let Some(url) = &app.session_state.oauth_url {
                                    // Open URL in browser using the 'open' crate
                                    if let Err(_e) = open::that(url) {
                                        // Silently ignore errors - user can manually copy URL from UI
                                    }
                                    // Don't clear the URL yet - leave it until OAuth is completed
                                    continue;
                                }
                            }

                            // Auto-focus to Input when user starts typing
                            // (printable characters only, not Ctrl combinations)
                            if let KeyCode::Char(_) = key.code {
                                if !key.modifiers.contains(KeyModifiers::CONTROL) && app.focus != Focus::Input {
                                    app.focus = Focus::Input;
                                    // Character will be processed by input handling below
                                }
                            }

                            // Handle input-specific keys when Input is focused
                            if app.focus == Focus::Input {
                                // Check for Shift+Escape FIRST (before plain Escape)
                                // This ensures Shift+Escape goes back to CommandDeck even when typing
                                if key.code == KeyCode::Esc && key.modifiers.contains(KeyModifiers::SHIFT) {
                                    if app.screen == Screen::Conversation {
                                        app.navigate_to_command_deck();
                                    }
                                    continue;
                                }

                                // Shift+Tab cycles permission mode (works while typing, all threads)
                                if key.code == KeyCode::BackTab {
                                    if app.screen == Screen::Conversation || app.screen == Screen::CommandDeck {
                                        app.cycle_permission_mode();
                                    }
                                    continue;
                                }

                                // macOS-style text navigation shortcuts (modifier + key)
                                // Check these BEFORE plain key handlers
                                match key.code {
                                    // Alt+Backspace: Delete word backward
                                    KeyCode::Backspace if key.modifiers.contains(KeyModifiers::ALT) => {
                                        app.textarea.delete_word_backward();
                                        continue;
                                    }
                                    // Super+Backspace (Cmd+Backspace): Delete to line start
                                    // Note: Most terminals intercept this, so Ctrl+U is the reliable alternative
                                    KeyCode::Backspace if key.modifiers.contains(KeyModifiers::SUPER) => {
                                        app.textarea.delete_to_line_start();
                                        continue;
                                    }
                                    // Alt+Left: Move cursor word left
                                    KeyCode::Left if key.modifiers.contains(KeyModifiers::ALT) => {
                                        app.textarea.move_cursor_word_left();
                                        continue;
                                    }
                                    // Super+Left (Cmd+Left): Move cursor to line start
                                    KeyCode::Left if key.modifiers.contains(KeyModifiers::SUPER) => {
                                        app.textarea.move_cursor_home();
                                        continue;
                                    }
                                    // Alt+Right: Move cursor word right
                                    KeyCode::Right if key.modifiers.contains(KeyModifiers::ALT) => {
                                        app.textarea.move_cursor_word_right();
                                        continue;
                                    }
                                    // Super+Right (Cmd+Right): Move cursor to line end
                                    KeyCode::Right if key.modifiers.contains(KeyModifiers::SUPER) => {
                                        app.textarea.move_cursor_end();
                                        continue;
                                    }
                                    _ => {}
                                }

                                // Plain key handlers (without modifiers)
                                match key.code {
                                    // Ctrl+U = Unix "kill line" - delete to line start
                                    // Works in ALL terminals (unlike Cmd+Backspace which terminals intercept)
                                    KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                        app.textarea.delete_to_line_start();
                                        continue;
                                    }
                                    // Ctrl+J = ASCII LF (newline) - works in ALL terminals
                                    // MUST come before plain Char(c) handler
                                    KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                        app.textarea.insert_newline();
                                        continue;
                                    }
                                    // Plain characters (no modifiers or only SHIFT)
                                    KeyCode::Char(c) if !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER) => {
                                        // Reset scroll to show input when typing (unified scroll)
                                        if app.screen == Screen::Conversation {
                                            app.user_has_scrolled = false;
                                            app.unified_scroll = 0;
                                        }
                                        // Check for @ trigger for folder picker (only on CommandDeck)
                                        if c == '@' && app.screen == Screen::CommandDeck {
                                            // Get current line content and cursor position
                                            let (row, col) = app.textarea.cursor();
                                            let lines = app.textarea.lines();
                                            let line_content = lines.get(row).map(|s| s.as_str()).unwrap_or("");

                                            if app.is_folder_picker_trigger(line_content, col) {
                                                // Insert the @ character first
                                                app.textarea.insert_char('@');
                                                // Then open the folder picker
                                                app.open_folder_picker();
                                                continue;
                                            }
                                        }
                                        // Normal character insertion
                                        app.textarea.insert_char(c);
                                        continue;
                                    }
                                    KeyCode::Backspace => {
                                        // Check if we should clear the folder chip instead of backspace
                                        if app.should_clear_folder_on_backspace() {
                                            app.clear_folder();
                                        } else {
                                            app.textarea.backspace();
                                        }
                                        continue;
                                    }
                                    KeyCode::Delete => {
                                        app.textarea.delete_char();
                                        continue;
                                    }
                                    KeyCode::Left => {
                                        app.textarea.move_cursor_left();
                                        continue;
                                    }
                                    KeyCode::Right => {
                                        app.textarea.move_cursor_right();
                                        continue;
                                    }
                                    KeyCode::Up => {
                                        // If cursor is on first line, try to navigate history up
                                        if app.textarea.is_cursor_on_first_line() {
                                            let current_content = app.textarea.content();
                                            if let Some(history_entry) = app.input_history.navigate_up(&current_content) {
                                                let entry = history_entry.to_string();
                                                app.textarea.set_content(&entry);
                                            }
                                        } else {
                                            // Normal cursor movement
                                            app.textarea.move_cursor_up();
                                        }
                                        continue;
                                    }
                                    KeyCode::Down => {
                                        // If cursor is on last line and navigating history, go forward
                                        if app.textarea.is_cursor_on_last_line() {
                                            // Only handle history navigation if we're currently navigating
                                            if app.input_history.is_navigating() {
                                                if let Some(history_entry) = app.input_history.navigate_down() {
                                                    let entry = history_entry.to_string();
                                                    app.textarea.set_content(&entry);
                                                } else {
                                                    // At bottom of history, restore original input
                                                    let original = app.input_history.get_current_input().to_string();
                                                    app.textarea.set_content(&original);
                                                }
                                            }
                                            // If not navigating, Down on last line does nothing
                                        } else {
                                            // Normal cursor movement in multi-line input
                                            app.textarea.move_cursor_down();
                                        }
                                        continue;
                                    }
                                    KeyCode::Home => {
                                        app.textarea.move_cursor_home();
                                        continue;
                                    }
                                    KeyCode::End => {
                                        app.textarea.move_cursor_end();
                                        continue;
                                    }
                                    KeyCode::Enter if key.modifiers.contains(KeyModifiers::SHIFT) => {
                                        // Shift+Enter inserts a newline (works in Kitty protocol terminals)
                                        app.textarea.insert_newline();
                                        continue;
                                    }
                                    KeyCode::Enter if key.modifiers.contains(KeyModifiers::ALT) => {
                                        // Alt+Enter inserts a newline
                                        app.textarea.insert_newline();
                                        continue;
                                    }
                                    KeyCode::Enter if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                        // Ctrl+Enter inserts a newline (fallback - may not work in all terminals)
                                        app.textarea.insert_newline();
                                        continue;
                                    }
                                    KeyCode::Enter => {
                                        // Plain Enter = Conversation thread
                                        app.submit_input(models::ThreadType::Conversation);
                                        continue;
                                    }
                                    KeyCode::Esc => {
                                        // Plain Escape (no Shift) - depends on input state and screen
                                        if app.screen == Screen::Conversation {
                                            if app.textarea.is_empty() {
                                                // Empty input: go back to CommandDeck
                                                app.navigate_to_command_deck();
                                            } else {
                                                // Has content: just unfocus to allow navigation
                                                app.focus = Focus::Threads;
                                            }
                                        } else {
                                            // On CommandDeck: unfocus input
                                            app.focus = Focus::Threads;
                                        }
                                        continue;
                                    }
                                    _ => {}
                                }
                            }

                            // Panel navigation (when not typing in input)
                            match key.code {
                                KeyCode::Tab => {
                                    // Double-tap Tab opens thread switcher
                                    app.handle_tab_press();
                                }
                                KeyCode::BackTab => {
                                    // Shift+Tab in Conversation/CommandDeck screens: cycle permission mode (all threads)
                                    if app.screen == Screen::Conversation || app.screen == Screen::CommandDeck {
                                        app.cycle_permission_mode();
                                    }
                                }
                                KeyCode::Esc if app.focus != Focus::Input => {
                                    // Escape when not in input: go back to CommandDeck
                                    if app.screen == Screen::Conversation {
                                        app.navigate_to_command_deck();
                                    }
                                }
                                KeyCode::Enter if app.focus == Focus::Threads => {
                                    // Open selected thread when pressing Enter on Threads panel
                                    app.open_selected_thread();
                                }
                                // Page scroll keys for conversation (unified scroll)
                                KeyCode::PageUp if app.screen == Screen::Conversation => {
                                    // Page up = scroll up to see older content
                                    app.scroll_velocity = 0.0; // Reset momentum on user scroll
                                    app.user_has_scrolled = true;
                                    let new_scroll = (app.unified_scroll + 10).min(app.max_scroll);
                                    let needs_redraw = if new_scroll != app.unified_scroll {
                                        app.unified_scroll = new_scroll;
                                        app.scroll_position = app.unified_scroll as f32;
                                        true
                                    } else if app.max_scroll > 0 {
                                        app.scroll_boundary_hit = Some(ScrollBoundary::Top);
                                        app.boundary_hit_tick = app.tick_count;
                                        true
                                    } else {
                                        false
                                    };
                                    if needs_redraw {
                                        app.mark_dirty();
                                    }
                                }
                                KeyCode::PageDown if app.screen == Screen::Conversation => {
                                    // Page down = scroll down to see newer content / input
                                    app.scroll_velocity = 0.0; // Reset momentum on user scroll
                                    let new_scroll = app.unified_scroll.saturating_sub(10);
                                    let needs_redraw = if new_scroll != app.unified_scroll {
                                        app.unified_scroll = new_scroll;
                                        app.scroll_position = app.unified_scroll as f32;
                                        if app.unified_scroll == 0 {
                                            app.user_has_scrolled = false; // Back at bottom
                                        }
                                        true
                                    } else {
                                        app.scroll_boundary_hit = Some(ScrollBoundary::Bottom);
                                        app.boundary_hit_tick = app.tick_count;
                                        true
                                    };
                                    if needs_redraw {
                                        app.mark_dirty();
                                    }
                                }
                                KeyCode::Up => {
                                    app.move_up();
                                }
                                KeyCode::Down => {
                                    let max_threads = app.cache.threads().len();
                                    app.move_down(max_threads);
                                }
                                KeyCode::Char('q') if app.focus != Focus::Input => {
                                    app.quit();
                                    return Ok(());
                                }
                                // 'd' to dismiss focused error in Conversation screen
                                KeyCode::Char('d') if app.focus != Focus::Input && app.screen == Screen::Conversation => {
                                    if app.has_errors() {
                                        app.dismiss_focused_error();
                                    }
                                }
                                // 't' to toggle thinking/reasoning block in Conversation screen
                                KeyCode::Char('t') if app.focus != Focus::Input && app.screen == Screen::Conversation => {
                                    app.toggle_reasoning();
                                }
                                // Note: Custom mouse selection removed - native terminal selection now handles copy
                                _ => {}
                            }
                        }
                        Event::Mouse(mouse_event) => {
                            // Handle mouse events for scroll, click, and hover
                            match mouse_event.kind {
                                // Left click: check hit areas for interactive elements
                                MouseEventKind::Down(MouseButton::Left) => {
                                    if let Some(action) = app.hit_registry.hit_test(
                                        mouse_event.column,
                                        mouse_event.row,
                                    ) {
                                        ui::handle_click_action(app, action);
                                        app.mark_dirty();
                                    }
                                    // If no hit area was clicked, let terminal handle text selection
                                }
                                // Mouse move: update hover state for visual feedback
                                MouseEventKind::Moved => {
                                    if app.hit_registry.update_hover(
                                        mouse_event.column,
                                        mouse_event.row,
                                    ) {
                                        app.mark_dirty();
                                    }
                                }
                                // Simple line-based scrolling (like native terminal apps)
                                // Each scroll event moves 3 lines (unified scroll)
                                MouseEventKind::ScrollDown => {
                                    if app.screen == Screen::Conversation {
                                        // Scroll down = see newer content / input
                                        app.scroll_velocity = 0.0; // Reset momentum on user scroll
                                        let needs_redraw = if app.unified_scroll >= 3 {
                                            app.unified_scroll -= 3;
                                            app.scroll_position = app.unified_scroll as f32;
                                            true
                                        } else if app.unified_scroll > 0 {
                                            app.unified_scroll = 0;
                                            app.user_has_scrolled = false; // Back at bottom
                                            app.scroll_position = 0.0;
                                            true
                                        } else {
                                            app.scroll_boundary_hit = Some(ScrollBoundary::Bottom);
                                            app.boundary_hit_tick = app.tick_count;
                                            true
                                        };
                                        if needs_redraw {
                                            app.mark_dirty();
                                        }
                                    }
                                }
                                MouseEventKind::ScrollUp => {
                                    if app.screen == Screen::Conversation {
                                        // Scroll up = see older content
                                        app.scroll_velocity = 0.0; // Reset momentum on user scroll
                                        app.user_has_scrolled = true;
                                        let new_scroll = (app.unified_scroll + 3).min(app.max_scroll);
                                        let needs_redraw = if new_scroll != app.unified_scroll {
                                            app.unified_scroll = new_scroll;
                                            app.scroll_position = app.unified_scroll as f32;
                                            true
                                        } else if app.max_scroll > 0 {
                                            app.scroll_boundary_hit = Some(ScrollBoundary::Top);
                                            app.boundary_hit_tick = app.tick_count;
                                            true
                                        } else {
                                            false
                                        };
                                        if needs_redraw {
                                            app.mark_dirty();
                                        }
                                    }
                                }
                                // Ignore other mouse events (right click, drag, etc.)
                                // Terminal handles text selection natively
                                _ => {}
                            }
                            continue;
                        }
                        Event::Paste(text) => {
                            // Handle paste events from bracketed paste mode
                            // Auto-focus to input if not already focused
                            if app.focus != Focus::Input {
                                app.focus = Focus::Input;
                            }

                            if app.should_summarize_paste(&text) {
                                // Insert as atomic token
                                app.textarea.insert_paste_token(text);
                            } else {
                                // Insert normally character by character
                                for ch in text.chars() {
                                    app.textarea.insert_char(ch);
                                }
                            }

                            app.mark_dirty();
                            continue;
                        }
                        _ => {
                            // Ignore other events (focus, etc.)
                        }
                    }
                }
            }

            // Handle async messages from streaming/connection
            msg = async {
                match &mut message_rx {
                    Some(rx) => rx.recv().await,
                    None => std::future::pending().await,
                }
            } => {
                if let Some(msg) = msg {
                    app.handle_message(msg);
                }
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}
