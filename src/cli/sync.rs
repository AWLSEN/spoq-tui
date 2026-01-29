//! Sync command for Spoq CLI.
//!
//! Handles token synchronization from local machine to VPS via HTTP API.
//! Note: Claude CLI uses server-side OAuth and is not synced from the client.

use color_eyre::Result;

use crate::auth::central_api::{CentralApiClient, VpsStatusResponse};
use crate::auth::credentials::Credentials;
use crate::auth::CredentialsManager;
use crate::conductor::ConductorClient;

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
/// This function syncs local tokens (GitHub CLI) to the VPS
/// via the Conductor HTTP API endpoint `/v1/tokens/sync`.
///
/// Note: Claude CLI uses server-side OAuth and is not synced from the client.
///
/// # Process
/// 1. Verify credentials are loaded and VPS exists (via API)
/// 2. Read local tokens (from filesystem for GitHub CLI)
/// 3. POST tokens to Conductor's `/v1/tokens/sync` endpoint
/// 4. Verify sync was successful
///
/// # Errors
///
/// Returns an error if the tokio runtime cannot be created.
/// Other errors are handled internally with appropriate exit codes.
pub fn handle_sync_command() -> Result<()> {
    println!("Running token synchronization...\n");

    // Step 1: Load credentials and verify VPS exists
    println!("[1/3] Verifying credentials and VPS...");
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

    // Get VPS URL for conductor
    let vps_url = if let Some(ref url) = vps_state.url {
        url.clone()
    } else if let Some(ref ip) = vps_state.ip {
        format!("http://{}:8000", ip)
    } else {
        eprintln!("Error: VPS has no URL or IP configured.");
        std::process::exit(1);
    };

    println!("  Credentials loaded");
    println!("  VPS found: {}", vps_url);

    // Step 2: Sync tokens via HTTP API
    println!("\n[2/3] Syncing tokens to VPS...");

    let mut conductor = ConductorClient::with_url(&vps_url);
    if let Some(ref token) = credentials.access_token {
        conductor = conductor.with_auth(token);
    }
    if let Some(ref refresh) = credentials.refresh_token {
        conductor = conductor.with_refresh_token(refresh);
    }

    let sync_result = match runtime.block_on(conductor.sync_tokens("all")) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("Error: Token sync failed: {}", e);
            std::process::exit(1);
        }
    };

    // Display sync results
    let synced_items = sync_result.synced.unwrap_or_default();
    let github_synced = synced_items.iter().any(|s| s == "github_cli");

    if github_synced {
        println!("  ✓ GitHub CLI tokens synced");
    } else {
        println!("  ⚠ GitHub CLI tokens not found locally");
    }

    // Show verification from sync response if available
    if let Some(verification) = &sync_result.verification {
        println!("\n[3/3] Verifying tokens on VPS...");

        if verification.github_cli_works == Some(true) {
            println!("  ✓ GitHub CLI tokens verified on VPS");
        } else if github_synced {
            println!("  ⚠ GitHub CLI tokens synced but not yet verified");
        }
    } else {
        // Fallback to explicit verification call
        println!("\n[3/3] Verifying tokens on VPS...");

        match runtime.block_on(conductor.verify_tokens()) {
            Ok(verify_result) => {
                if verify_result.github_cli.authenticated {
                    println!("  ✓ GitHub CLI tokens verified on VPS");
                } else {
                    println!("  ⚠ GitHub CLI tokens not valid on VPS");
                }
            }
            Err(e) => {
                eprintln!("  Warning: Could not verify tokens: {}", e);
            }
        }
    }

    println!("\nToken synchronization complete!");

    Ok(())
}
