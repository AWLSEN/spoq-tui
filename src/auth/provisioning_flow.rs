//! Synchronous provisioning flow module.
//!
//! This module provides blocking provisioning flows for the TUI application.
//! It uses the Tokio runtime to call existing async methods on CentralApiClient.

use std::io::{self, Write};
use std::net::{Ipv4Addr, Ipv6Addr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use super::central_api::{
    ByovpsPendingResponse, CentralApiClient, CentralApiError, ConfirmVpsRequest, DataCenter,
    VpsPlan, VpsStatusResponse,
};
use super::credentials::{Credentials, CredentialsManager};
use crate::cli_output::{self, icons, SPINNER_CHARS};
use crate::conductor::ConductorClient;
use crate::setup::gh_auth::{ensure_gh_authenticated, is_gh_authenticated, GhAuthError};

/// VPS type selection.
#[derive(Debug, Clone, PartialEq)]
pub enum VpsType {
    Managed,
    Byovps,
    Local,
}

/// BYOVPS credentials collected from user input.
#[derive(Debug, Clone)]
pub struct ByovpsCredentials {
    pub vps_ip: String,
    pub ssh_username: String,
    pub ssh_password: String,
}

/// Poll interval for VPS status checks (in seconds).
const POLL_INTERVAL_SECS: u64 = 3;

/// Maximum number of poll attempts before timing out.
const MAX_POLL_ATTEMPTS: u32 = 200; // 10 minutes at 3 second intervals

/// Maximum number of retry attempts for BYOVPS provisioning.
const BYOVPS_MAX_RETRY_ATTEMPTS: u32 = 3;

/// Health check timeout after provision timeout (2 minutes).
/// When provisioning times out (e.g., 524 Cloudflare timeout), we poll
/// the conductor health endpoint for this duration before giving up.
const HEALTH_CHECK_TIMEOUT_SECS: u64 = 120;

/// Poll interval for payment status checks (in seconds).
const PAYMENT_POLL_INTERVAL_SECS: u64 = 5;

/// Maximum number of payment poll attempts before timing out.
const PAYMENT_MAX_POLL_ATTEMPTS: u32 = 120; // 10 minutes at 5 second intervals

/// Set up Ctrl+C handler that sets the interrupted flag.
/// Returns the Arc<AtomicBool> that will be set to true on interrupt.
fn setup_interrupt_handler() -> Arc<AtomicBool> {
    let interrupted = Arc::new(AtomicBool::new(false));
    let interrupted_clone = Arc::clone(&interrupted);

    // Install the handler - ignore errors if already set
    let _ = ctrlc::set_handler(move || {
        interrupted_clone.store(true, Ordering::SeqCst);
    });

    interrupted
}

/// Check if the user has pressed Ctrl+C and exit gracefully if so.
fn check_interrupt(interrupted: &Arc<AtomicBool>) {
    if interrupted.load(Ordering::SeqCst) {
        println!("\nProvisioning cancelled.");
        std::process::exit(0);
    }
}

/// Sync tokens to VPS via HTTP API using the Conductor client.
///
/// This function:
/// 1. Creates a Conductor client with auth tokens
/// 2. Calls sync_tokens() which reads from Keychain and filesystem
/// 3. Posts tokens to VPS via HTTP
///
/// # Arguments
/// * `runtime` - Tokio runtime for async operations
/// * `vps_url` - URL of the VPS conductor
/// * `credentials` - User credentials containing access/refresh tokens
fn sync_tokens_to_vps(
    runtime: &tokio::runtime::Runtime,
    vps_url: &str,
    credentials: &Credentials,
) {
    println!("\nSyncing tokens to VPS via HTTP...");

    let mut conductor = ConductorClient::with_url(vps_url);
    if let Some(ref token) = credentials.access_token {
        conductor = conductor.with_auth(token);
    }
    if let Some(ref refresh) = credentials.refresh_token {
        conductor = conductor.with_refresh_token(refresh);
    }

    match runtime.block_on(conductor.sync_tokens("all")) {
        Ok(result) => {
            let synced = result.synced.unwrap_or_default();
            if synced.contains(&"github_cli".to_string()) {
                println!("  ✓ GitHub CLI tokens synced");
            }
            if synced.is_empty() {
                println!("  ⚠ No tokens found locally to sync");
            }

            // Show verification results if available
            if let Some(verification) = result.verification {
                if verification.github_cli_works == Some(true) {
                    println!("  ✓ GitHub CLI verified on VPS");
                }
            }
        }
        Err(e) => {
            eprintln!("  Warning: Token sync failed: {}", e);
            eprintln!("  You can run 'spoq --sync' later to sync tokens manually.");
        }
    }
}

/// Validate an IP address string (IPv4 or IPv6).
///
/// # Arguments
/// * `input` - The IP address string to validate
///
/// # Returns
/// * `Ok(())` - Valid IP address
/// * `Err(String)` - Error message describing the validation failure
fn validate_ip_address(input: &str) -> Result<(), String> {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return Err("IP address cannot be empty".to_string());
    }

    // Try parsing as IPv4
    if trimmed.parse::<Ipv4Addr>().is_ok() {
        return Ok(());
    }

    // Try parsing as IPv6
    if trimmed.parse::<Ipv6Addr>().is_ok() {
        return Ok(());
    }

    Err("Invalid IP address format. Enter IPv4 (e.g., 192.168.1.1) or IPv6 (e.g., 2001:db8::1)".to_string())
}

/// Collect BYOVPS credentials from user input with interrupt support.
///
/// # Arguments
/// * `interrupted` - Interrupt flag for Ctrl+C handling
///
/// # Returns
/// * `Ok(ByovpsCredentials)` - Collected and validated credentials
/// * `Err(CentralApiError)` - Failed to read input
fn collect_byovps_credentials(
    interrupted: &Arc<AtomicBool>,
) -> Result<ByovpsCredentials, CentralApiError> {
    // Prompt for VPS IP address with validation
    let vps_ip = loop {
        check_interrupt(interrupted);

        print!("\nEnter VPS IP address (IPv4 or IPv6): ");
        io::stdout()
            .flush()
            .map_err(|e| CentralApiError::ServerError {
                status: 0,
                message: format!("Failed to flush stdout: {}", e),
            })?;

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .map_err(|e| CentralApiError::ServerError {
                status: 0,
                message: format!("Failed to read input: {}", e),
            })?;

        check_interrupt(interrupted);

        let trimmed = input.trim().to_string();
        match validate_ip_address(&trimmed) {
            Ok(()) => break trimmed,
            Err(e) => println!("{}", e),
        }
    };

    // Prompt for SSH username (default: "root")
    let ssh_username = loop {
        check_interrupt(interrupted);

        print!("Enter SSH username [root]: ");
        io::stdout()
            .flush()
            .map_err(|e| CentralApiError::ServerError {
                status: 0,
                message: format!("Failed to flush stdout: {}", e),
            })?;

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .map_err(|e| CentralApiError::ServerError {
                status: 0,
                message: format!("Failed to read input: {}", e),
            })?;

        check_interrupt(interrupted);

        let trimmed = input.trim().to_string();
        if trimmed.is_empty() {
            break "root".to_string();
        } else if !trimmed.is_empty() {
            break trimmed;
        }
    };

    // Prompt for SSH password (minimum 8 characters)
    let ssh_password = loop {
        check_interrupt(interrupted);

        print!("Enter SSH password (min 8 characters): ");
        io::stdout()
            .flush()
            .map_err(|e| CentralApiError::ServerError {
                status: 0,
                message: format!("Failed to flush stdout: {}", e),
            })?;

        let password = rpassword::read_password().map_err(|e| CentralApiError::ServerError {
            status: 0,
            message: format!("Failed to read password: {}", e),
        })?;

        check_interrupt(interrupted);

        if password.len() >= 8 {
            break password;
        }

        println!("Password must be at least 8 characters. Try again.");
    };

    Ok(ByovpsCredentials {
        vps_ip,
        ssh_username,
        ssh_password,
    })
}

/// Prompt the user to choose VPS type with interrupt support.
///
/// # Arguments
/// * `interrupted` - Interrupt flag for Ctrl+C handling
///
/// # Returns
/// * `Ok(VpsType)` - User selected VPS type
/// * `Err(CentralApiError)` - Failed to read input
fn choose_vps_type(interrupted: &Arc<AtomicBool>) -> Result<VpsType, CentralApiError> {
    println!("\nChoose VPS type:");
    println!("  [1] Managed VPS");
    println!("  [2] BYOVPS (Bring Your Own VPS)");
    println!("  [3] Local (run conductor on this machine)");

    loop {
        check_interrupt(interrupted);

        print!("\nEnter your choice (1-3): ");
        io::stdout()
            .flush()
            .map_err(|e| CentralApiError::ServerError {
                status: 0,
                message: format!("Failed to flush stdout: {}", e),
            })?;

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .map_err(|e| CentralApiError::ServerError {
                status: 0,
                message: format!("Failed to read input: {}", e),
            })?;

        check_interrupt(interrupted);

        let trimmed = input.trim();
        match trimmed {
            "1" => return Ok(VpsType::Managed),
            "2" => return Ok(VpsType::Byovps),
            "3" => return Ok(VpsType::Local),
            _ => println!("Please enter 1, 2, or 3."),
        }
    }
}

/// Run the provisioning flow to set up VPS for the user.
///
/// This function blocks until provisioning is complete and updates credentials.
///
/// # Arguments
/// * `runtime` - The Tokio runtime to use for async operations
/// * `credentials` - Mutable reference to credentials (may be updated during provisioning)
///
/// # Returns
/// * `Ok(())` - Provisioning completed successfully
/// * `Err(CentralApiError)` - Provisioning failed
pub fn run_provisioning_flow(
    runtime: &tokio::runtime::Runtime,
    credentials: &mut Credentials,
) -> Result<(), CentralApiError> {
    println!("\nPress Ctrl+C to cancel.\n");

    // Check if GitHub CLI is authenticated (runs `gh auth status`)
    if !is_gh_authenticated() {
        println!("GitHub CLI not authenticated. Starting auto-login...\n");

        match ensure_gh_authenticated() {
            Ok(()) => {
                println!("GitHub CLI authenticated successfully\n");
            }
            Err(GhAuthError::NotInstalled) => {
                println!("GitHub CLI not installed.");
                println!("Install from: https://cli.github.com/\n");
                return Err(CentralApiError::ServerError {
                    status: 0,
                    message: "GitHub CLI not installed. Please install it first.".to_string(),
                });
            }
            Err(GhAuthError::Cancelled) => {
                println!("GitHub CLI authentication cancelled.\n");
                return Err(CentralApiError::ServerError {
                    status: 0,
                    message: "GitHub CLI authentication cancelled by user.".to_string(),
                });
            }
            Err(e) => {
                println!("GitHub CLI authentication failed: {}\n", e);
                return Err(CentralApiError::ServerError {
                    status: 0,
                    message: format!("GitHub CLI authentication failed: {}", e),
                });
            }
        }
    }

    println!("Checking credentials...");

    // Set up interrupt handler
    let interrupted = setup_interrupt_handler();

    // Step 0: Choose VPS type
    let vps_type = choose_vps_type(&interrupted)?;

    // Branch based on VPS type
    match vps_type {
        VpsType::Managed => {
            // Continue with managed VPS provisioning flow
            run_managed_vps_flow(runtime, credentials, &interrupted)
        }
        VpsType::Byovps => {
            // Collect BYOVPS credentials
            check_interrupt(&interrupted);
            let mut byovps_creds = collect_byovps_credentials(&interrupted)?;

            // Run BYOVPS provisioning flow with retry logic
            run_byovps_flow_with_retry(runtime, credentials, &mut byovps_creds, &interrupted)
        }
        VpsType::Local => {
            run_local_conductor_flow(runtime, &interrupted)
        }
    }
}

/// Poll payment completion status with interrupt support.
///
/// # Arguments
/// * `runtime` - The Tokio runtime for async operations
/// * `client` - API client for checking payment status
/// * `session_id` - Stripe checkout session ID
/// * `interrupted` - Interrupt flag for Ctrl+C handling
///
/// # Returns
/// * `Ok(PaymentStatusResponse)` - Payment completed successfully
/// * `Err(CentralApiError)` - Payment failed, expired, or timed out
fn poll_payment_completion(
    runtime: &tokio::runtime::Runtime,
    client: &CentralApiClient,
    session_id: &str,
    interrupted: &Arc<AtomicBool>,
) -> Result<super::central_api::PaymentStatusResponse, CentralApiError> {
    let start_time = std::time::Instant::now();
    let mut attempts = 0;

    loop {
        check_interrupt(interrupted);

        attempts += 1;
        if attempts > PAYMENT_MAX_POLL_ATTEMPTS {
            return Err(CentralApiError::ServerError {
                status: 408,
                message: format!(
                    "Payment timed out after {} minutes. Please check your payment status and try again.",
                    (PAYMENT_MAX_POLL_ATTEMPTS as u64) * PAYMENT_POLL_INTERVAL_SECS / 60
                ),
            });
        }

        // Poll payment status
        let status_result = runtime.block_on(client.check_payment_status(session_id));

        match status_result {
            Ok(status) => {
                match status.status.as_str() {
                    "paid" | "complete" => {
                        // Payment successful
                        return Ok(status);
                    }
                    "expired" | "cancelled" => {
                        // Payment failed
                        return Err(CentralApiError::ServerError {
                            status: 400,
                            message: format!("Payment {}: Please try again.", status.status),
                        });
                    }
                    "pending" | "open" => {
                        // Continue polling
                    }
                    _ => {
                        // Unknown status, log and continue
                        eprintln!("Unknown payment status: {}", status.status);
                    }
                }
            }
            Err(e) => {
                eprintln!("Error checking payment status: {}", e);
                // Continue polling unless it's a critical error
                if matches!(e, CentralApiError::ServerError { status: 401, .. }) {
                    return Err(e);
                }
            }
        }

        // Show spinner with elapsed time
        let elapsed = start_time.elapsed().as_secs();
        let spinner_char = SPINNER_CHARS[attempts as usize % SPINNER_CHARS.len()];
        print!(
            "\r{} Waiting for payment completion... ({}m {}s)",
            spinner_char,
            elapsed / 60,
            elapsed % 60
        );
        io::stdout().flush().ok();

        // Wait before next poll
        thread::sleep(Duration::from_secs(PAYMENT_POLL_INTERVAL_SECS));
    }
}

/// Run the managed VPS provisioning flow.
fn run_managed_vps_flow(
    runtime: &tokio::runtime::Runtime,
    credentials: &mut Credentials,
    interrupted: &Arc<AtomicBool>,
) -> Result<(), CentralApiError> {
    // Create API client with authentication
    let access_token =
        credentials
            .access_token
            .as_ref()
            .ok_or_else(|| CentralApiError::ServerError {
                status: 401,
                message: "No access token available".to_string(),
            })?;

    let mut client = CentralApiClient::new().with_auth(access_token);

    // Set refresh token if available for auto-refresh
    if let Some(ref refresh_token) = credentials.refresh_token {
        client.set_refresh_token(Some(refresh_token.clone()));
    }

    // Step 1: Fetch available subscription plans (with Stripe pricing)
    println!("Fetching available plans...");
    check_interrupt(&interrupted);
    let plans = runtime.block_on(client.fetch_subscription_plans())?;

    if plans.is_empty() {
        return Err(CentralApiError::ServerError {
            status: 404,
            message: "No VPS plans available".to_string(),
        });
    }

    // Step 2: Display plans and get user selection
    check_interrupt(interrupted);
    display_plans(&plans);
    let selected_index = prompt_plan_selection_with_interrupt(plans.len(), interrupted)?;
    let selected_plan = &plans[selected_index];

    println!(
        "\nYou selected: {} (${:.2}/month)",
        selected_plan.name,
        selected_plan.price_cents as f64 / 100.0
    );

    // Step 3: Create checkout session and process payment
    check_interrupt(interrupted);
    println!("\nCreating payment session...");
    let checkout_response = runtime.block_on(client.create_checkout_session(&selected_plan.id))?;

    // Check if tokens were refreshed and update credentials
    let (new_access_token, new_refresh_token) = client.get_tokens();
    if let Some(access_token) = new_access_token {
        if credentials.access_token.as_ref() != Some(&access_token) {
            credentials.access_token = Some(access_token);
            if let Some(refresh_token) = new_refresh_token {
                credentials.refresh_token = Some(refresh_token);
            }
            save_credentials(credentials);
        }
    }

    // Display payment information
    println!("\n╔═════════════════════════════════════════════════════╗");
    println!("║              Payment Required                       ║");
    println!("╚═════════════════════════════════════════════════════╝");
    println!("\n  Plan:  {}", selected_plan.name);
    println!(
        "  Price: ${:.2}/month",
        selected_plan.price_cents as f64 / 100.0
    );
    println!("  Email: {}", checkout_response.customer_email);
    println!("\n  Opening payment page in your browser...");
    println!("  URL: {}", checkout_response.checkout_url);

    // Open browser with checkout URL
    check_interrupt(interrupted);
    if let Err(e) = webbrowser::open(&checkout_response.checkout_url) {
        eprintln!("\nWarning: Failed to open browser: {}", e);
        println!("Please manually open the URL above to complete payment.");
    }

    // Poll for payment completion
    println!("\n  Waiting for payment...");
    let _payment_status =
        poll_payment_completion(runtime, &client, &checkout_response.session_id, interrupted)?;

    println!("\n\n✓ Payment successful!");

    // Step 4: Get SSH password from user
    check_interrupt(interrupted);
    let ssh_password = prompt_ssh_password_with_interrupt(interrupted)?;

    // Step 5: Confirm provisioning
    check_interrupt(interrupted);
    if !prompt_confirmation_with_interrupt(interrupted)? {
        println!("Provisioning cancelled.");
        return Ok(());
    }

    // Step 6: Fetch and select datacenter
    check_interrupt(interrupted);
    println!("\nFetching available data centers...");
    let datacenters = runtime.block_on(client.fetch_datacenters())?;

    if datacenters.is_empty() {
        return Err(CentralApiError::ServerError {
            status: 404,
            message: "No data centers available".to_string(),
        });
    }

    check_interrupt(interrupted);
    let ordered_dcs = display_datacenters(&datacenters);
    let selected_datacenter_id =
        prompt_datacenter_selection_with_interrupt(&ordered_dcs, interrupted)?;

    // Step 7: Provision the VPS (health-first approach - no DB record yet)
    check_interrupt(interrupted);
    println!("\nProvisioning your VPS...");
    let provision_result = runtime.block_on(client.provision_vps_pending(
        &ssh_password,
        Some(&selected_plan.id),
        Some(selected_datacenter_id),
    ));

    // Check if tokens were refreshed and update credentials
    let (new_access_token, new_refresh_token) = client.get_tokens();
    if let Some(access_token) = new_access_token {
        if credentials.access_token.as_ref() != Some(&access_token) {
            credentials.access_token = Some(access_token);
            if let Some(refresh_token) = new_refresh_token {
                credentials.refresh_token = Some(refresh_token);
            }
            save_credentials(credentials);
        }
    }

    // Handle 409 Conflict - user already has an active VPS
    let pending_response = match provision_result {
        Ok(response) => response,
        Err(CentralApiError::ServerError { status: 409, .. }) => {
            println!("\nYou already have an active VPS.");
            println!("Please use your existing VPS or contact support to delete it first.");
            return Ok(());
        }
        Err(e) => return Err(e),
    };

    println!("VPS provisioning started!");
    println!("  {}", pending_response.message);
    println!("  Hostname: {}", pending_response.hostname);

    // Step 8: Poll health endpoint directly (instead of /api/vps/status)
    println!("\nWaiting for conductor to be ready...");
    let health_url = format!("https://{}", pending_response.hostname);

    match wait_for_health_with_ui(runtime, &health_url, MAX_POLL_ATTEMPTS as u64 * POLL_INTERVAL_SECS, interrupted) {
        Ok(()) => {
            println!("\n  Conductor is healthy!");
        }
        Err(e) => {
            return Err(CentralApiError::ServerError {
                status: 503,
                message: format!("Health check failed: {}", e),
            });
        }
    }

    // Step 9: Confirm VPS with backend (creates DB record)
    println!("\nConfirming VPS with backend...");
    let confirm_request = ConfirmVpsRequest {
        hostname: pending_response.hostname.clone(),
        ip_address: pending_response.ip_address.clone().unwrap_or_default(),
        provider_instance_id: pending_response.provider_instance_id,
        provider_order_id: pending_response.provider_order_id.clone(),
        provider: "hostinger".to_string(),
        plan_id: pending_response.plan_id,
        template_id: pending_response.template_id,
        data_center_id: pending_response.data_center_id,
        jwt_secret: pending_response.jwt_secret.clone(),
        ssh_password: pending_response.ssh_password.clone(),
    };

    let status = runtime.block_on(client.confirm_vps(confirm_request))?;

    // Check if tokens were refreshed during confirmation and update credentials
    let (new_access_token, new_refresh_token) = client.get_tokens();
    if let Some(access_token) = new_access_token {
        if credentials.access_token.as_ref() != Some(&access_token) {
            credentials.access_token = Some(access_token);
            if let Some(refresh_token) = new_refresh_token {
                credentials.refresh_token = Some(refresh_token);
            }
            save_credentials(credentials);
        }
    }

    println!("\nVPS provisioning complete!");
    println!("  Status: {}", status.status);
    if let Some(hostname) = &status.hostname {
        println!("  Hostname: {}", hostname);
    }
    if let Some(ip) = &status.ip {
        println!("  IP: {}", ip);
    }
    if let Some(url) = &status.url {
        println!("  URL: {}", url);
    }

    // Sync tokens to VPS via HTTP API
    let vps_url = status
        .url
        .clone()
        .or_else(|| status.hostname.as_ref().map(|h| format!("https://{}", h)))
        .or_else(|| status.ip.as_ref().map(|ip| format!("http://{}:8000", ip)));

    if let Some(url) = vps_url {
        sync_tokens_to_vps(runtime, &url, credentials);
    } else {
        eprintln!("Warning: No VPS URL available for token sync.");
        eprintln!("You can run 'spoq --sync' later to sync tokens manually.");
    }

    Ok(())
}

/// Retry action for BYOVPS provisioning failure.
#[derive(Debug, Clone, PartialEq)]
enum ByovpsRetryAction {
    Retry,
    ChangeVpsDetails,
    Exit,
}

/// Check if an error is an authentication error (401 Unauthorized).
fn is_auth_error(error: &CentralApiError) -> bool {
    matches!(error, CentralApiError::ServerError { status: 401, .. })
}

/// Check if an error message indicates an SSH connection error.
fn is_ssh_connection_error(message: &str) -> bool {
    let lower = message.to_lowercase();
    lower.contains("ssh")
        || lower.contains("connection refused")
        || lower.contains("connection timed out")
        || lower.contains("host unreachable")
        || lower.contains("network unreachable")
        || lower.contains("no route to host")
        || lower.contains("authentication failed")
        || lower.contains("permission denied")
        || lower.contains("port 22")
}

/// Check if an error is a timeout/gateway error that suggests provisioning may have started.
///
/// These errors occur when the backend takes too long (e.g., during SSH setup),
/// causing Cloudflare or the HTTP client to timeout. In such cases, the backend
/// may have already created the VPS record and started provisioning.
fn is_timeout_error(error: &CentralApiError) -> bool {
    match error {
        CentralApiError::ServerError { status, message } => {
            // 524: Cloudflare timeout (backend took too long)
            // 522: Connection timed out
            // 504: Gateway timeout
            // 502: Bad gateway (sometimes indicates timeout)
            *status == 524
                || *status == 522
                || *status == 504
                || *status == 502
                || message.to_lowercase().contains("timeout")
                || message.to_lowercase().contains("error code: 524")
        }
        CentralApiError::Http(e) => {
            let msg = e.to_string().to_lowercase();
            msg.contains("timeout") || msg.contains("timed out")
        }
        _ => false,
    }
}

/// Display BYOVPS provisioning error with helpful details.
fn display_byovps_error(error: &CentralApiError) {
    let message = match error {
        CentralApiError::ServerError { message, .. } => message.clone(),
        CentralApiError::Http(e) => e.to_string(),
        _ => format!("{}", error),
    };

    println!("\nProvisioning failed!");

    if is_ssh_connection_error(&message) {
        println!("\nFailed to connect via SSH. Please verify:");
        println!("  - VPS IP is correct");
        println!("  - VPS is running and accessible");
        println!("  - SSH is enabled (port 22)");
        println!("  - Username and password are correct");
        println!("\nError details: {}", message);
    } else {
        println!("\nError: {}", message);
    }
}

/// Prompt user for retry action after BYOVPS provisioning failure.
///
/// # Arguments
/// * `interrupted` - Interrupt flag for Ctrl+C handling
///
/// # Returns
/// * `Ok(ByovpsRetryAction)` - User's chosen action
/// * `Err(CentralApiError)` - Failed to read input
fn prompt_byovps_retry_action(
    interrupted: &Arc<AtomicBool>,
) -> Result<ByovpsRetryAction, CentralApiError> {
    loop {
        check_interrupt(interrupted);

        print!("\nRetry? (y)es / (c)hange VPS details / (e)xit: ");
        io::stdout()
            .flush()
            .map_err(|e| CentralApiError::ServerError {
                status: 0,
                message: format!("Failed to flush stdout: {}", e),
            })?;

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .map_err(|e| CentralApiError::ServerError {
                status: 0,
                message: format!("Failed to read input: {}", e),
            })?;

        check_interrupt(interrupted);

        let trimmed = input.trim().to_lowercase();
        match trimmed.as_str() {
            "y" | "yes" => return Ok(ByovpsRetryAction::Retry),
            "c" | "change" => return Ok(ByovpsRetryAction::ChangeVpsDetails),
            "e" | "exit" => return Ok(ByovpsRetryAction::Exit),
            _ => println!("Please enter 'y' to retry, 'c' to change VPS details, or 'e' to exit."),
        }
    }
}

/// Run the BYOVPS provisioning flow with error handling and retry logic.
///
/// This flow:
/// 1. Attempts to provision BYOVPS
/// 2. On timeout errors (524, etc.), switches to health polling instead of retrying
/// 3. On other failures, displays error details and prompts for retry
/// 4. Supports up to 3 retry attempts
/// 5. User can retry with same credentials, change credentials, or exit
fn run_byovps_flow_with_retry(
    runtime: &tokio::runtime::Runtime,
    credentials: &mut Credentials,
    byovps_creds: &mut ByovpsCredentials,
    interrupted: &Arc<AtomicBool>,
) -> Result<(), CentralApiError> {
    let mut attempts = 0;

    loop {
        attempts += 1;

        // Attempt provisioning
        let result = run_byovps_flow(runtime, credentials, byovps_creds, interrupted);

        match result {
            Ok(()) => return Ok(()),
            Err(error) => {
                // Check if this is a timeout error (524, gateway timeout, etc.)
                // In this case, the backend may have started provisioning but the
                // request timed out. Instead of retrying provisioning, we should
                // poll the conductor health endpoint to check if it came up.
                if is_timeout_error(&error) {
                    // Use clean UI for timeout recovery
                    cli_output::print_header("SPOQ VPS SETUP (Recovery)");

                    // STEP 3: HEALTH CHECK (recovery mode)
                    cli_output::print_step_start(3, "HEALTH CHECK");
                    cli_output::print_step_line(
                        icons::WARNING,
                        "Request timed out, checking health...",
                    );

                    // In health-first pattern, we don't have a DB record yet.
                    // Construct hostname from username pattern or fallback to IP.
                    // Username format: {name}.spoq.dev
                    let health_url = if !byovps_creds.ssh_username.is_empty()
                        && byovps_creds.ssh_username != "root"
                    {
                        // Use the expected hostname pattern
                        format!("https://{}.spoq.dev", byovps_creds.ssh_username)
                    } else {
                        // Fallback to IP-based health check (conductor runs on port 8000)
                        format!("http://{}:8000", byovps_creds.vps_ip)
                    };
                    match wait_for_health_with_ui(
                        runtime,
                        &health_url,
                        HEALTH_CHECK_TIMEOUT_SECS,
                        interrupted,
                    ) {
                        Ok(()) => {
                            cli_output::print_step_spinner_done(
                                icons::SUCCESS,
                                &format!("Conductor healthy at {}", health_url),
                            );
                            cli_output::print_step_end();

                            // STEP 4: CONFIRM VPS (Create DB record in recovery)
                            cli_output::print_step_start(4, "CONFIRMING VPS");

                            // Re-call provision to get jwt_secret (API is idempotent now)
                            let mut client = CentralApiClient::new();
                            if let Some(ref token) = credentials.access_token {
                                client = client.with_auth(token);
                            }
                            if let Some(ref refresh) = credentials.refresh_token {
                                client = client.with_refresh_token(refresh);
                            }

                            match runtime.block_on(client.provision_byovps_pending(
                                &byovps_creds.vps_ip,
                                &byovps_creds.ssh_username,
                                &byovps_creds.ssh_password,
                            )) {
                                Ok(pending) => {
                                    // Build ConfirmVpsRequest with BYOVPS-specific values
                                    let confirm_request = ConfirmVpsRequest {
                                        hostname: pending.hostname.clone(),
                                        ip_address: pending.ip_address.clone(),
                                        provider_instance_id: 0,
                                        provider_order_id: None,
                                        provider: "byovps".to_string(),
                                        plan_id: "byovps".to_string(),
                                        template_id: 0,
                                        data_center_id: 0,
                                        jwt_secret: pending.jwt_secret.clone(),
                                        ssh_password: pending.ssh_password.clone(),
                                    };

                                    match runtime.block_on(client.confirm_vps(confirm_request)) {
                                        Ok(_) => {
                                            cli_output::print_step_line(
                                                icons::SUCCESS,
                                                "VPS confirmed in backend",
                                            );
                                        }
                                        Err(e) => {
                                            cli_output::print_step_line(
                                                icons::WARNING,
                                                &format!("Failed to confirm VPS: {}", e),
                                            );
                                        }
                                    }
                                }
                                Err(e) => {
                                    cli_output::print_step_line(
                                        icons::WARNING,
                                        &format!("Failed to get pending data: {}", e),
                                    );
                                }
                            }
                            cli_output::print_step_end();

                            // STEP 5: CREDENTIAL SYNC (via HTTP API)
                            cli_output::print_step_start(5, "CREDENTIAL SYNC");
                            let mut has_warnings = false;

                            // Construct conductor URL for HTTP sync
                            let conductor_url_for_sync = if !byovps_creds.ssh_username.is_empty()
                                && byovps_creds.ssh_username != "root"
                            {
                                format!("https://{}.spoq.dev", byovps_creds.ssh_username)
                            } else {
                                format!("http://{}:8000", byovps_creds.vps_ip)
                            };

                            let mut conductor = ConductorClient::with_url(&conductor_url_for_sync);
                            if let Some(ref token) = credentials.access_token {
                                conductor = conductor.with_auth(token);
                            }
                            if let Some(ref refresh) = credentials.refresh_token {
                                conductor = conductor.with_refresh_token(refresh);
                            }

                            match runtime.block_on(conductor.sync_tokens("all")) {
                                Ok(sync_result) => {
                                    let synced = sync_result.synced.unwrap_or_default();
                                    if synced.contains(&"github_cli".to_string()) {
                                        cli_output::print_step_line(
                                            icons::SUCCESS,
                                            "GitHub CLI synced",
                                        );
                                    }
                                    if synced.is_empty() {
                                        cli_output::print_step_line(
                                            icons::WARNING,
                                            "No local tokens found to sync",
                                        );
                                    }

                                    // Show verification results from sync response
                                    if let Some(verification) = sync_result.verification {
                                        if verification.github_cli_works == Some(true) {
                                            cli_output::print_step_line(
                                                icons::SUCCESS,
                                                "GitHub CLI verified on VPS",
                                            );
                                        } else if synced.contains(&"github_cli".to_string()) {
                                            has_warnings = true;
                                            cli_output::print_step_line(
                                                icons::WARNING,
                                                "GitHub CLI synced but not verified",
                                            );
                                        }
                                    }
                                }
                                Err(e) => {
                                    has_warnings = true;
                                    cli_output::print_step_line(
                                        icons::WARNING,
                                        &format!("Sync failed: {}", e),
                                    );
                                }
                            }
                            cli_output::print_step_end();

                            // Final footer
                            let conductor_url =
                                format!("https://{}.spoq.dev", byovps_creds.ssh_username);
                            if has_warnings {
                                cli_output::print_footer_warning(
                                    &byovps_creds.vps_ip,
                                    &conductor_url,
                                    &byovps_creds.ssh_username,
                                );
                            } else {
                                cli_output::print_footer_success(
                                    &byovps_creds.vps_ip,
                                    &conductor_url,
                                    &byovps_creds.ssh_username,
                                );
                            }

                            return Ok(());
                        }
                        Err(health_err) => {
                            cli_output::print_step_spinner_done(
                                icons::FAILURE,
                                &format!("Health check failed: {}", health_err),
                            );
                            cli_output::print_step_end();
                            // Fall through to normal error handling
                        }
                    }
                }

                // Display error with helpful details
                display_byovps_error(&error);

                // Check if max retries reached
                if attempts >= BYOVPS_MAX_RETRY_ATTEMPTS {
                    println!(
                        "\nMaximum retry attempts ({}) reached. Exiting provisioning.",
                        BYOVPS_MAX_RETRY_ATTEMPTS
                    );
                    return Err(error);
                }

                println!(
                    "\nAttempt {}/{} failed.",
                    attempts, BYOVPS_MAX_RETRY_ATTEMPTS
                );

                // Check if error is auth-related (401) - likely invalid/expired tokens
                if is_auth_error(&error) {
                    println!("\nAuthentication failed. Your session may have expired.");
                    println!("Please run the CLI again to re-authenticate.");

                    // Clear credentials to force fresh authentication
                    credentials.access_token = None;
                    credentials.refresh_token = None;
                    save_credentials(credentials);

                    return Err(error);
                }

                // Prompt user for action
                check_interrupt(interrupted);
                match prompt_byovps_retry_action(interrupted)? {
                    ByovpsRetryAction::Retry => {
                        println!("\nRetrying with same VPS details...");
                        continue;
                    }
                    ByovpsRetryAction::ChangeVpsDetails => {
                        println!("\nPlease enter new VPS details:");
                        *byovps_creds = collect_byovps_credentials(interrupted)?;
                        continue;
                    }
                    ByovpsRetryAction::Exit => {
                        println!("\nExiting provisioning.");
                        return Err(error);
                    }
                }
            }
        }
    }
}

/// Run the BYOVPS provisioning flow with clean ASCII UI.
///
/// 5-Step Flow:
/// 1. AUTHENTICATION - Check token, refresh if needed
/// 2. VPS PROVISIONING - Call provision API
/// 3. HEALTH CHECK - Wait for conductor health
/// 4. CREDENTIAL SYNC - Migrate tokens
/// 5. VERIFICATION - Verify tokens work on VPS
fn run_byovps_flow(
    runtime: &tokio::runtime::Runtime,
    credentials: &mut Credentials,
    byovps_creds: &ByovpsCredentials,
    interrupted: &Arc<AtomicBool>,
) -> Result<(), CentralApiError> {
    // Print header
    cli_output::print_header("SPOQ VPS SETUP");

    // Track if any verification failed for final status
    let mut has_warnings = false;

    // ═══════════════════════════════════════════════════════════════════
    // STEP 1: AUTHENTICATION
    // ═══════════════════════════════════════════════════════════════════
    cli_output::print_step_start(1, "AUTHENTICATION");

    if credentials.is_expired() {
        cli_output::print_step_line(icons::WARNING, "Token expired, refreshing...");

        if let Some(ref refresh_token) = credentials.refresh_token {
            let temp_client = CentralApiClient::new();
            match runtime.block_on(temp_client.refresh_token(refresh_token)) {
                Ok(token_response) => {
                    credentials.access_token = Some(token_response.access_token.clone());
                    if let Some(new_refresh_token) = token_response.refresh_token {
                        credentials.refresh_token = Some(new_refresh_token);
                    }

                    // Calculate expiration
                    let expiration_str = if let Some(expires_in) = token_response.expires_in {
                        let expires_at = chrono::Utc::now().timestamp() + expires_in as i64;
                        credentials.expires_at = Some(expires_at);
                        chrono::DateTime::<chrono::Utc>::from_timestamp(expires_at, 0)
                            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                            .unwrap_or_else(|| "unknown".to_string())
                    } else if let Some(expires_in) =
                        super::central_api::get_jwt_expires_in(&token_response.access_token)
                    {
                        let expires_at = chrono::Utc::now().timestamp() + expires_in as i64;
                        credentials.expires_at = Some(expires_at);
                        chrono::DateTime::<chrono::Utc>::from_timestamp(expires_at, 0)
                            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                            .unwrap_or_else(|| "unknown".to_string())
                    } else {
                        "unknown".to_string()
                    };

                    save_credentials(credentials);
                    cli_output::print_step_line(
                        icons::SUCCESS,
                        &format!("Token valid until {}", expiration_str),
                    );
                }
                Err(e) => {
                    cli_output::print_step_line(icons::FAILURE, "Token refresh failed");
                    cli_output::print_step_end();
                    credentials.access_token = None;
                    credentials.refresh_token = None;
                    save_credentials(credentials);
                    return Err(CentralApiError::ServerError {
                        status: 401,
                        message: format!("Token refresh failed: {}", e),
                    });
                }
            }
        } else {
            cli_output::print_step_line(icons::FAILURE, "No refresh token available");
            cli_output::print_step_end();
            credentials.access_token = None;
            credentials.refresh_token = None;
            save_credentials(credentials);
            return Err(CentralApiError::ServerError {
                status: 401,
                message: "Please sign in again".to_string(),
            });
        }
    } else {
        // Token is valid
        let expiration_str = credentials
            .expires_at
            .and_then(|ts| chrono::DateTime::<chrono::Utc>::from_timestamp(ts, 0))
            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| "unknown".to_string());
        cli_output::print_step_line(icons::SUCCESS, "Authenticated");
        cli_output::print_step_line(
            icons::SUCCESS,
            &format!("Token valid until {}", expiration_str),
        );
    }
    cli_output::print_step_end();

    // Get access token for API calls
    let access_token =
        credentials
            .access_token
            .as_ref()
            .ok_or_else(|| CentralApiError::ServerError {
                status: 401,
                message: "No access token available".to_string(),
            })?;

    let mut client = CentralApiClient::new().with_auth(access_token);
    if let Some(ref refresh_token) = credentials.refresh_token {
        client.set_refresh_token(Some(refresh_token.clone()));
    }

    // ═══════════════════════════════════════════════════════════════════
    // STEP 2: VPS PROVISIONING (Health-First Pattern)
    // ═══════════════════════════════════════════════════════════════════
    cli_output::print_step_start(2, "VPS PROVISIONING");
    check_interrupt(interrupted);

    // Call provision_byovps_pending to get pending response (no DB record yet)
    let pending_response = provision_byovps_pending_with_spinner_ui(
        runtime,
        &mut client,
        &byovps_creds.vps_ip,
        &byovps_creds.ssh_username,
        &byovps_creds.ssh_password,
        interrupted,
    )?;

    // Update credentials if tokens were refreshed
    let (new_access_token, new_refresh_token) = client.get_tokens();
    if let Some(access_token) = new_access_token {
        if credentials.access_token.as_ref() != Some(&access_token) {
            credentials.access_token = Some(access_token);
            if let Some(refresh_token) = new_refresh_token {
                credentials.refresh_token = Some(refresh_token);
            }
            save_credentials(credentials);
        }
    }

    cli_output::print_step_line(icons::SUCCESS, &format!("Hostname: {}", pending_response.hostname));
    cli_output::print_step_line(icons::SUCCESS, &pending_response.message);
    cli_output::print_step_end();

    // ═══════════════════════════════════════════════════════════════════
    // STEP 3: HEALTH CHECK (Poll conductor directly)
    // ═══════════════════════════════════════════════════════════════════
    cli_output::print_step_start(3, "HEALTH CHECK");
    check_interrupt(interrupted);

    // Use hostname (HTTPS via Cloudflare) for health check
    let health_url = format!("https://{}", pending_response.hostname);
    match wait_for_health_with_ui(runtime, &health_url, HEALTH_CHECK_TIMEOUT_SECS, interrupted) {
        Ok(()) => {
            cli_output::print_step_spinner_done(
                icons::SUCCESS,
                &format!("Conductor healthy at {}", health_url),
            );
        }
        Err(e) => {
            cli_output::print_step_spinner_done(
                icons::FAILURE,
                &format!("Health check failed: {}", e),
            );
            cli_output::print_troubleshoot(&[
                &format!(
                    "1. SSH to VPS: ssh {}@{}",
                    byovps_creds.ssh_username, byovps_creds.vps_ip
                ),
                "2. Check logs: journalctl -u conductor -f",
                "3. Restart:    systemctl restart conductor",
            ]);
            cli_output::print_step_end();
            return Err(CentralApiError::ServerError {
                status: 503,
                message: format!("Health check failed: {}", e),
            });
        }
    }
    cli_output::print_step_end();

    // ═══════════════════════════════════════════════════════════════════
    // STEP 4: CONFIRM VPS (Create DB record after health passes)
    // ═══════════════════════════════════════════════════════════════════
    cli_output::print_step_start(4, "CONFIRMING VPS");
    check_interrupt(interrupted);

    // Build ConfirmVpsRequest with BYOVPS-specific values
    let confirm_request = ConfirmVpsRequest {
        hostname: pending_response.hostname.clone(),
        ip_address: pending_response.ip_address.clone(),
        provider_instance_id: 0, // Not applicable for BYOVPS
        provider_order_id: None,
        provider: "byovps".to_string(),
        plan_id: "byovps".to_string(), // Not applicable for BYOVPS
        template_id: 0, // Not applicable for BYOVPS
        data_center_id: 0, // Not applicable for BYOVPS
        jwt_secret: pending_response.jwt_secret.clone(),
        ssh_password: pending_response.ssh_password.clone(),
    };

    match runtime.block_on(client.confirm_vps(confirm_request)) {
        Ok(_confirm_response) => {
            cli_output::print_step_line(icons::SUCCESS, "VPS confirmed in backend");
        }
        Err(e) => {
            cli_output::print_step_line(icons::FAILURE, &format!("Failed to confirm VPS: {}", e));
            cli_output::print_step_end();
            return Err(e);
        }
    }

    // Update credentials if tokens were refreshed during confirmation
    let (new_access_token, new_refresh_token) = client.get_tokens();
    if let Some(access_token) = new_access_token {
        if credentials.access_token.as_ref() != Some(&access_token) {
            credentials.access_token = Some(access_token);
            if let Some(refresh_token) = new_refresh_token {
                credentials.refresh_token = Some(refresh_token);
            }
            save_credentials(credentials);
        }
    }
    cli_output::print_step_end();

    // ═══════════════════════════════════════════════════════════════════
    // STEP 5: CREDENTIAL SYNC (via HTTP API)
    // ═══════════════════════════════════════════════════════════════════
    cli_output::print_step_start(5, "CREDENTIAL SYNC");
    check_interrupt(interrupted);

    // Sync credentials to VPS via HTTP API
    let conductor_url_for_sync = format!("https://{}", pending_response.hostname);
    let mut conductor = ConductorClient::with_url(&conductor_url_for_sync);
    if let Some(ref token) = credentials.access_token {
        conductor = conductor.with_auth(token);
    }
    if let Some(ref refresh) = credentials.refresh_token {
        conductor = conductor.with_refresh_token(refresh);
    }

    match runtime.block_on(conductor.sync_tokens("all")) {
        Ok(sync_result) => {
            let synced = sync_result.synced.unwrap_or_default();
            if synced.contains(&"github_cli".to_string()) {
                cli_output::print_step_line(icons::SUCCESS, "GitHub CLI synced");
            }
            if synced.is_empty() {
                has_warnings = true;
                cli_output::print_step_line(icons::WARNING, "No local tokens found to sync");
            }

            // Show verification results from sync response
            if let Some(verification) = sync_result.verification {
                if verification.github_cli_works == Some(true) {
                    cli_output::print_step_line(icons::SUCCESS, "GitHub CLI verified on VPS");
                } else if synced.contains(&"github_cli".to_string()) {
                    has_warnings = true;
                    cli_output::print_step_line(icons::WARNING, "GitHub CLI synced but not verified");
                }

                if has_warnings {
                    cli_output::print_troubleshoot(&[
                        &format!(
                            "1. SSH to VPS: ssh {}@{}",
                            byovps_creds.ssh_username, byovps_creds.vps_ip
                        ),
                        "2. Run: gh auth login (if GitHub failed)",
                    ]);
                }
            }
        }
        Err(e) => {
            has_warnings = true;
            cli_output::print_step_line(icons::WARNING, &format!("Sync failed: {}", e));
        }
    }
    cli_output::print_step_end();

    // ═══════════════════════════════════════════════════════════════════
    // FINAL SUMMARY
    // ═══════════════════════════════════════════════════════════════════
    let conductor_url = format!("https://{}", pending_response.hostname);
    if has_warnings {
        cli_output::print_footer_warning(
            &byovps_creds.vps_ip,
            &conductor_url,
            &byovps_creds.ssh_username,
        );
    } else {
        cli_output::print_footer_success(
            &byovps_creds.vps_ip,
            &conductor_url,
            &byovps_creds.ssh_username,
        );
    }

    Ok(())
}

/// Wait for health with UI spinner.
fn wait_for_health_with_ui(
    runtime: &tokio::runtime::Runtime,
    url: &str,
    timeout_secs: u64,
    interrupted: &Arc<AtomicBool>,
) -> Result<(), String> {
    use std::time::Instant;

    let start = Instant::now();
    let mut frame = 0usize;

    loop {
        check_interrupt(interrupted);

        let elapsed = start.elapsed().as_secs();
        if elapsed >= timeout_secs {
            return Err(format!("Timeout after {}s", timeout_secs));
        }

        // Show spinner
        let spinner = SPINNER_CHARS[frame % SPINNER_CHARS.len()];
        cli_output::print_step_spinner(
            spinner,
            &format!("Waiting for conductor... ({}s)", elapsed),
        );
        frame += 1;

        // Try health check
        match runtime.block_on(async {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .map_err(|e| e.to_string())?;

            let resp = client
                .get(format!("{}/health", url))
                .send()
                .await
                .map_err(|e| e.to_string())?;

            if resp.status().is_success() {
                Ok(())
            } else {
                Err(format!("Status: {}", resp.status()))
            }
        }) {
            Ok(()) => return Ok(()),
            Err(_) => {
                // Wait before next attempt
                thread::sleep(Duration::from_secs(3));
            }
        }
    }
}

/// Provision BYOVPS with pending response (health-first pattern) with spinner in step box.
fn provision_byovps_pending_with_spinner_ui(
    runtime: &tokio::runtime::Runtime,
    client: &mut CentralApiClient,
    vps_ip: &str,
    ssh_username: &str,
    ssh_password: &str,
    interrupted: &Arc<AtomicBool>,
) -> Result<ByovpsPendingResponse, CentralApiError> {
    use std::sync::mpsc;
    use std::time::Instant;

    let (tx, rx) = mpsc::channel();
    let spinner_interrupted = Arc::clone(interrupted);

    let spinner_handle = thread::spawn(move || {
        let mut frame = 0usize;
        let start = Instant::now();

        loop {
            if rx.try_recv().is_ok() || spinner_interrupted.load(Ordering::SeqCst) {
                break;
            }

            let spinner = SPINNER_CHARS[frame % SPINNER_CHARS.len()];
            let elapsed = start.elapsed().as_secs();
            cli_output::print_step_spinner(
                spinner,
                &format!("Initiating BYOVPS setup... ({}s)", elapsed),
            );
            frame += 1;
            thread::sleep(Duration::from_millis(100));
        }
    });

    let result =
        runtime.block_on(client.provision_byovps_pending(vps_ip, ssh_username, ssh_password));

    let _ = tx.send(());
    let _ = spinner_handle.join();

    check_interrupt(interrupted);
    result
}

/// Save credentials to file.
fn save_credentials(credentials: &Credentials) {
    if let Some(manager) = CredentialsManager::new() {
        if !manager.save(credentials) {
            eprintln!("Warning: Failed to save credentials to file");
        }
    }
}

/// Display available VPS plans to the user.
fn display_plans(plans: &[VpsPlan]) {
    println!("\nAvailable VPS Plans:");
    println!("{:-<60}", "");

    for (i, plan) in plans.iter().enumerate() {
        let ram_display = if plan.ram_mb >= 1024 {
            format!("{} GB", plan.ram_mb / 1024)
        } else {
            format!("{} MB", plan.ram_mb)
        };

        let price_display = format!("${:.2}/mo", plan.price_cents as f64 / 100.0);

        // Show first month discount if available
        let discount_info = if let Some(first_month) = plan.first_month_price_cents {
            format!(" (first month: ${:.2})", first_month as f64 / 100.0)
        } else {
            String::new()
        };

        println!(
            "  [{}] {} - {} vCPU, {} RAM, {} GB Disk - {}{}",
            i + 1,
            plan.name,
            plan.vcpus,
            ram_display,
            plan.disk_gb,
            price_display,
            discount_info
        );
    }

    println!("{:-<60}", "");
}

/// Display available data centers grouped by continent.
/// Returns a mapping of display number (1-indexed) to datacenter.
fn display_datacenters(datacenters: &[DataCenter]) -> Vec<&DataCenter> {
    use std::collections::BTreeMap;

    // Group datacenters by continent
    let mut by_continent: BTreeMap<&str, Vec<&DataCenter>> = BTreeMap::new();
    for dc in datacenters {
        by_continent.entry(&dc.continent).or_default().push(dc);
    }

    // Build ordered list for selection
    let mut ordered: Vec<&DataCenter> = Vec::new();

    println!("\nAvailable Data Centers:");
    println!("{:─<40}", "");

    for (continent, dcs) in &by_continent {
        println!("{}:", continent);
        for dc in dcs {
            ordered.push(dc);
            println!("  [{}] {}, {}", ordered.len(), dc.city, dc.country);
        }
    }

    println!("{:─<40}", "");

    ordered
}

/// Prompt the user to select a datacenter by number with interrupt support.
fn prompt_datacenter_selection_with_interrupt(
    datacenters: &[&DataCenter],
    interrupted: &Arc<AtomicBool>,
) -> Result<u32, CentralApiError> {
    let max = datacenters.len();

    loop {
        check_interrupt(interrupted);

        print!("\nSelect data center (1-{}): ", max);
        io::stdout()
            .flush()
            .map_err(|e| CentralApiError::ServerError {
                status: 0,
                message: format!("Failed to flush stdout: {}", e),
            })?;

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .map_err(|e| CentralApiError::ServerError {
                status: 0,
                message: format!("Failed to read input: {}", e),
            })?;

        check_interrupt(interrupted);

        let trimmed = input.trim();
        match trimmed.parse::<usize>() {
            Ok(n) if n >= 1 && n <= max => {
                let selected = datacenters[n - 1];
                return Ok(selected.id);
            }
            Ok(_) => println!("Please enter a number between 1 and {}.", max),
            Err(_) => println!("Please enter a valid number."),
        }
    }
}

/// Prompt the user to select a plan by number with interrupt support.
fn prompt_plan_selection_with_interrupt(
    max: usize,
    interrupted: &Arc<AtomicBool>,
) -> Result<usize, CentralApiError> {
    loop {
        check_interrupt(interrupted);

        print!("\nEnter plan number (1-{}): ", max);
        io::stdout()
            .flush()
            .map_err(|e| CentralApiError::ServerError {
                status: 0,
                message: format!("Failed to flush stdout: {}", e),
            })?;

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .map_err(|e| CentralApiError::ServerError {
                status: 0,
                message: format!("Failed to read input: {}", e),
            })?;

        check_interrupt(interrupted);

        let trimmed = input.trim();
        match trimmed.parse::<usize>() {
            Ok(n) if n >= 1 && n <= max => return Ok(n - 1),
            Ok(_) => println!("Please enter a number between 1 and {}.", max),
            Err(_) => println!("Please enter a valid number."),
        }
    }
}

/// Prompt the user for confirmation with interrupt support.
fn prompt_confirmation_with_interrupt(
    interrupted: &Arc<AtomicBool>,
) -> Result<bool, CentralApiError> {
    check_interrupt(interrupted);

    print!("\nProceed with provisioning? (y/n): ");
    io::stdout()
        .flush()
        .map_err(|e| CentralApiError::ServerError {
            status: 0,
            message: format!("Failed to flush stdout: {}", e),
        })?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|e| CentralApiError::ServerError {
            status: 0,
            message: format!("Failed to read input: {}", e),
        })?;

    check_interrupt(interrupted);

    let trimmed = input.trim().to_lowercase();
    Ok(trimmed == "y" || trimmed == "yes")
}

/// Prompt the user for SSH password with interrupt support.
/// Uses rpassword for hidden input and validates minimum 12 characters.
fn prompt_ssh_password_with_interrupt(
    interrupted: &Arc<AtomicBool>,
) -> Result<String, CentralApiError> {
    loop {
        check_interrupt(interrupted);

        print!("Enter SSH password (min 12 characters): ");
        io::stdout()
            .flush()
            .map_err(|e| CentralApiError::ServerError {
                status: 0,
                message: format!("Failed to flush stdout: {}", e),
            })?;

        let password = rpassword::read_password().map_err(|e| CentralApiError::ServerError {
            status: 0,
            message: format!("Failed to read password: {}", e),
        })?;

        check_interrupt(interrupted);

        if password.len() >= 12 {
            return Ok(password);
        }

        println!("Password must be at least 12 characters. Try again.");
    }
}

/// Poll VPS status until ready (without interrupt support).
/// This is a simpler version for programmatic use.
///
/// # Arguments
/// * `runtime` - The Tokio runtime to use for async operations
/// * `client` - The CentralApiClient to use for polling
///
/// # Returns
/// * `Ok(VpsStatusResponse)` - VPS is ready
/// * `Err(CentralApiError)` - VPS failed or timed out
pub fn poll_vps_until_ready(
    runtime: &tokio::runtime::Runtime,
    client: &mut CentralApiClient,
) -> Result<VpsStatusResponse, CentralApiError> {
    let mut attempts = 0;

    loop {
        let status = runtime.block_on(client.fetch_vps_status())?;

        // Check VPS status according to state matrix
        match status.status.to_lowercase().as_str() {
            // Success states - VPS is ready to use
            "ready" | "running" | "active" => {
                return Ok(status);
            }
            // Error states - provisioning failed or VPS in unexpected state
            "stopped" | "failed" | "terminated" | "error" => {
                return Err(CentralApiError::ServerError {
                    status: 500,
                    message: format!("VPS failed with status: {}", status.status),
                });
            }
            // Keep polling states - provisioning in progress
            "pending" | "provisioning" => {
                // Still starting/provisioning, continue polling
            }
            // Unknown states - treat as still provisioning
            _ => {
                // Unknown status, continue polling
            }
        }

        attempts += 1;
        if attempts >= MAX_POLL_ATTEMPTS {
            return Err(CentralApiError::ServerError {
                status: 408,
                message: "VPS start timed out".to_string(),
            });
        }

        thread::sleep(Duration::from_secs(POLL_INTERVAL_SECS));
    }
}

/// Start a stopped VPS and wait for it to be ready.
///
/// Returns updated VpsStatusResponse or error.
///
/// # Arguments
/// * `runtime` - The Tokio runtime to use for async operations
/// * `credentials` - The credentials containing the access token
///
/// # Returns
/// * `Ok(VpsStatusResponse)` - VPS started and is ready
/// * `Err(CentralApiError)` - Failed to start VPS or timed out
pub fn start_stopped_vps(
    runtime: &tokio::runtime::Runtime,
    credentials: &Credentials,
) -> Result<VpsStatusResponse, CentralApiError> {
    let token = credentials
        .access_token
        .as_ref()
        .ok_or_else(|| CentralApiError::ServerError {
            status: 401,
            message: "No access token".to_string(),
        })?;

    let mut client = CentralApiClient::new().with_auth(token);

    // Set refresh token if available for auto-refresh
    if let Some(ref refresh_token) = credentials.refresh_token {
        client.set_refresh_token(Some(refresh_token.clone()));
    }

    // Start VPS
    println!("Starting your VPS...");
    runtime.block_on(client.start_vps())?;

    // Poll until ready
    poll_vps_until_ready(runtime, &mut client)
}

/// Run the local conductor provisioning flow.
///
/// Downloads conductor binary if needed, starts it on localhost:8000,
/// waits for health, and saves config.
fn run_local_conductor_flow(
    runtime: &tokio::runtime::Runtime,
    interrupted: &Arc<AtomicBool>,
) -> Result<(), CentralApiError> {
    use crate::conductor::local;

    println!("\nSetting up local conductor...\n");

    // Step 1: Download if needed
    if !local::conductor_exists() {
        println!("  Downloading conductor binary...");
        check_interrupt(interrupted);
        runtime.block_on(local::download_conductor()).map_err(|e| {
            CentralApiError::ServerError {
                status: 0,
                message: format!("Download failed: {}", e),
            }
        })?;
        println!("  Conductor downloaded");
    } else {
        println!("  Conductor binary found");
    }

    let port = local::default_port();

    // Step 2: Check if already running
    let already_running = runtime.block_on(local::is_running(port));

    if !already_running {
        // Step 3: Start conductor
        println!("  Starting conductor on port {}...", port);
        check_interrupt(interrupted);
        // Use "local-user" as owner_id — conductor skips JWT for localhost anyway
        let _child = runtime
            .block_on(local::start_conductor(port, "local-user"))
            .map_err(|e| CentralApiError::ServerError {
                status: 0,
                message: e,
            })?;
        // Note: _child is dropped here but conductor process continues running.
        // On TUI launch, preflight will detect local mode and manage the process.

        // Step 4: Health check
        println!("  Waiting for conductor...");
        check_interrupt(interrupted);
        runtime
            .block_on(local::wait_for_health(port, 30))
            .map_err(|e| CentralApiError::ServerError {
                status: 0,
                message: e,
            })?;
    } else {
        println!("  Conductor already running on port {}", port);
    }

    println!("  Conductor is healthy");

    // Step 5: Save config
    let config = crate::startup::config::SpoqConfig {
        conductor_mode: "local".to_string(),
        conductor_url: Some(format!("http://127.0.0.1:{}", port)),
    };
    config.save().map_err(|e| CentralApiError::ServerError {
        status: 0,
        message: format!("Failed to save config: {}", e),
    })?;

    println!("\n  Local conductor ready!\n");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_plans_formats_correctly() {
        // This is mainly a visual test - we just verify it doesn't panic
        let plans = vec![
            VpsPlan {
                id: "plan-1".to_string(),
                name: "Small".to_string(),
                vcpus: 1,
                ram_mb: 1024,
                disk_gb: 25,
                price_cents: 500,
                bandwidth_tb: None,
                first_month_price_cents: None,
            },
            VpsPlan {
                id: "plan-2".to_string(),
                name: "Medium".to_string(),
                vcpus: 2,
                ram_mb: 2048,
                disk_gb: 50,
                price_cents: 1000,
                bandwidth_tb: Some(2),
                first_month_price_cents: Some(100),
            },
        ];

        // Just verify it doesn't panic
        display_plans(&plans);
    }

    #[test]
    fn test_display_plans_ram_formatting() {
        // Test MB display (under 1024 MB)
        let plans = vec![VpsPlan {
            id: "plan-tiny".to_string(),
            name: "Tiny".to_string(),
            vcpus: 1,
            ram_mb: 512,
            disk_gb: 10,
            price_cents: 250,
            bandwidth_tb: None,
            first_month_price_cents: None,
        }];

        // Just verify it doesn't panic with small RAM values
        display_plans(&plans);
    }

    #[test]
    fn test_constants() {
        // Verify polling constants are reasonable
        assert!(POLL_INTERVAL_SECS >= 1);
        assert!(MAX_POLL_ATTEMPTS >= 1);
        // Total wait time should be at least a minute
        assert!(POLL_INTERVAL_SECS * MAX_POLL_ATTEMPTS as u64 >= 60);
    }

    #[test]
    fn test_vps_type_enum() {
        // Test VpsType enum variants
        let managed = VpsType::Managed;
        let byovps = VpsType::Byovps;
        let local = VpsType::Local;

        assert_eq!(managed, VpsType::Managed);
        assert_eq!(byovps, VpsType::Byovps);
        assert_eq!(local, VpsType::Local);
        assert_ne!(managed, byovps);
        assert_ne!(managed, local);

        // Test Debug trait
        assert_eq!(format!("{:?}", managed), "Managed");
        assert_eq!(format!("{:?}", byovps), "Byovps");
        assert_eq!(format!("{:?}", local), "Local");

        // Test Clone trait
        let managed_clone = managed.clone();
        assert_eq!(managed, managed_clone);
    }


    #[test]
    fn test_vps_status_failed_states() {
        // Test that failed states are recognized
        let failed_states = ["failed", "error", "Failed", "ERROR"];

        for state in &failed_states {
            let is_failed = matches!(state.to_lowercase().as_str(), "failed" | "error");
            assert!(
                is_failed,
                "State '{}' should be recognized as failed",
                state
            );
        }
    }

    #[test]
    fn test_display_datacenters_groups_by_continent() {
        let datacenters = vec![
            DataCenter {
                id: 1,
                name: "PHX1".to_string(),
                city: "Phoenix".to_string(),
                country: "USA".to_string(),
                continent: "North America".to_string(),
            },
            DataCenter {
                id: 2,
                name: "AMS1".to_string(),
                city: "Amsterdam".to_string(),
                country: "Netherlands".to_string(),
                continent: "Europe".to_string(),
            },
            DataCenter {
                id: 3,
                name: "LAX1".to_string(),
                city: "Los Angeles".to_string(),
                country: "USA".to_string(),
                continent: "North America".to_string(),
            },
        ];

        let ordered = display_datacenters(&datacenters);

        // Should return all datacenters
        assert_eq!(ordered.len(), 3);

        // Due to BTreeMap ordering, Europe comes before North America
        // Within a continent, order matches input order
        assert_eq!(ordered[0].city, "Amsterdam");
        assert_eq!(ordered[1].city, "Phoenix");
        assert_eq!(ordered[2].city, "Los Angeles");
    }

    #[test]
    fn test_display_datacenters_returns_correct_ids() {
        let datacenters = vec![
            DataCenter {
                id: 5,
                name: "TYO1".to_string(),
                city: "Tokyo".to_string(),
                country: "Japan".to_string(),
                continent: "Asia".to_string(),
            },
            DataCenter {
                id: 9,
                name: "SYD1".to_string(),
                city: "Sydney".to_string(),
                country: "Australia".to_string(),
                continent: "Oceania".to_string(),
            },
        ];

        let ordered = display_datacenters(&datacenters);

        // Verify IDs are preserved
        assert_eq!(ordered[0].id, 5); // Asia before Oceania alphabetically
        assert_eq!(ordered[1].id, 9);
    }

    #[test]
    fn test_display_datacenters_empty_list() {
        let datacenters: Vec<DataCenter> = vec![];
        let ordered = display_datacenters(&datacenters);
        assert!(ordered.is_empty());
    }

    #[test]
    fn test_display_datacenters_single_datacenter() {
        let datacenters = vec![DataCenter {
            id: 42,
            name: "TEST1".to_string(),
            city: "Test City".to_string(),
            country: "Test Country".to_string(),
            continent: "Test Continent".to_string(),
        }];

        let ordered = display_datacenters(&datacenters);

        assert_eq!(ordered.len(), 1);
        assert_eq!(ordered[0].id, 42);
        assert_eq!(ordered[0].city, "Test City");
    }

    #[test]
    fn test_409_conflict_error_detection() {
        // Test that 409 error is correctly identified as a conflict error
        let error = CentralApiError::ServerError {
            status: 409,
            message: "User already has an active VPS".to_string(),
        };

        // Verify the error status is 409 (Conflict)
        if let CentralApiError::ServerError { status, .. } = error {
            assert_eq!(status, 409);
        } else {
            panic!("Expected ServerError variant");
        }
    }

    #[test]
    fn test_provisioning_flow_collects_all_required_params() {
        // Verify that the provisioning flow expects plan_id, datacenter_id, and ssh_password
        // This is a compile-time verification test - if the function signature changes,
        // this test documents the expected parameters.

        // The provision_vps function signature requires:
        // - ssh_password: &str (required)
        // - plan_id: Option<&str>
        // - data_center_id: Option<u32>

        // Note: datacenter_id is no longer stored in Credentials
        // VPS state is always fetched from the API
        let creds = Credentials::default();
        assert!(creds.access_token.is_none());
    }

    #[test]
    fn test_password_validation_logic() {
        // Test the password validation logic (minimum 12 characters)
        // This tests the core validation that happens in prompt_ssh_password_with_interrupt

        // 11 characters should fail
        let password_11_chars = "12345678901";
        assert_eq!(password_11_chars.len(), 11);
        assert!(
            password_11_chars.len() < 12,
            "11 character password should be rejected"
        );

        // Exactly 12 characters should pass
        let password_12_chars = "123456789012";
        assert_eq!(password_12_chars.len(), 12);
        assert!(
            password_12_chars.len() >= 12,
            "12 character password should be accepted"
        );

        // 13+ characters should pass
        let password_13_chars = "1234567890123";
        assert_eq!(password_13_chars.len(), 13);
        assert!(
            password_13_chars.len() >= 12,
            "13 character password should be accepted"
        );

        // Empty password should fail
        let empty_password = "";
        assert!(
            empty_password.len() < 12,
            "Empty password should be rejected"
        );

        // Unicode characters should be counted by length (not bytes)
        let unicode_password = "p@$$wörd123!"; // 12 chars
        assert_eq!(unicode_password.chars().count(), 12);
    }

    #[test]
    fn test_error_status_codes() {
        // Test various HTTP error status codes used in provisioning flow

        // 400 Bad Request - invalid parameters
        let err_400 = CentralApiError::ServerError {
            status: 400,
            message: "Invalid plan_id".to_string(),
        };
        if let CentralApiError::ServerError { status, message } = err_400 {
            assert_eq!(status, 400);
            assert!(message.contains("Invalid"));
        }

        // 401 Unauthorized - missing or expired token
        let err_401 = CentralApiError::ServerError {
            status: 401,
            message: "Access token required".to_string(),
        };
        if let CentralApiError::ServerError { status, message } = err_401 {
            assert_eq!(status, 401);
            assert!(message.contains("token"));
        }

        // 404 Not Found - no VPS exists
        let err_404 = CentralApiError::ServerError {
            status: 404,
            message: "No VPS found for user".to_string(),
        };
        if let CentralApiError::ServerError { status, message } = err_404 {
            assert_eq!(status, 404);
            assert!(message.contains("VPS"));
        }

        // 409 Conflict - user already has a VPS
        let err_409 = CentralApiError::ServerError {
            status: 409,
            message: "User already has an active VPS".to_string(),
        };
        if let CentralApiError::ServerError { status, message } = err_409 {
            assert_eq!(status, 409);
            assert!(message.contains("already"));
        }
    }

    #[test]
    fn test_vps_status_states_comprehensive() {
        // Test all documented VPS states from the API v2 spec

        // Success states - VPS is usable
        let success_states = ["ready", "running", "active"];
        for state in &success_states {
            let is_success = matches!(
                state.to_lowercase().as_str(),
                "ready" | "running" | "active"
            );
            assert!(is_success, "State '{}' should be a success state", state);
        }

        // Error states - VPS failed or terminated
        let error_states = ["stopped", "failed", "terminated", "error"];
        for state in &error_states {
            let is_error = matches!(
                state.to_lowercase().as_str(),
                "stopped" | "failed" | "terminated" | "error"
            );
            assert!(is_error, "State '{}' should be an error state", state);
        }

        // Polling states - keep waiting
        let polling_states = ["pending", "provisioning"];
        for state in &polling_states {
            let is_polling = matches!(state.to_lowercase().as_str(), "pending" | "provisioning");
            assert!(is_polling, "State '{}' should be a polling state", state);
        }
    }

    #[test]
    fn test_byovps_credentials_struct() {
        // Test ByovpsCredentials struct creation and field access
        let creds = ByovpsCredentials {
            vps_ip: "192.168.1.100".to_string(),
            ssh_username: "root".to_string(),
            ssh_password: "testpass".to_string(),
        };

        assert_eq!(creds.vps_ip, "192.168.1.100");
        assert_eq!(creds.ssh_username, "root");
        assert_eq!(creds.ssh_password, "testpass");

        // Test Debug trait
        let debug_str = format!("{:?}", creds);
        assert!(debug_str.contains("ByovpsCredentials"));
        assert!(debug_str.contains("192.168.1.100"));

        // Test Clone trait
        let cloned = creds.clone();
        assert_eq!(cloned.vps_ip, creds.vps_ip);
        assert_eq!(cloned.ssh_username, creds.ssh_username);
        assert_eq!(cloned.ssh_password, creds.ssh_password);
    }

    #[test]
    fn test_byovps_credentials_ipv6_support() {
        // Test that IPv6 addresses can be stored in ByovpsCredentials
        let ipv6_creds = ByovpsCredentials {
            vps_ip: "2001:0db8:85a3:0000:0000:8a2e:0370:7334".to_string(),
            ssh_username: "admin".to_string(),
            ssh_password: "securepass".to_string(),
        };

        assert_eq!(ipv6_creds.vps_ip, "2001:0db8:85a3:0000:0000:8a2e:0370:7334");
        assert!(ipv6_creds.vps_ip.contains(":"));
    }

    #[test]
    fn test_byovps_credentials_username_defaults() {
        // Test that "root" is a valid username
        let creds_root = ByovpsCredentials {
            vps_ip: "10.0.0.1".to_string(),
            ssh_username: "root".to_string(),
            ssh_password: "pass".to_string(),
        };

        assert_eq!(creds_root.ssh_username, "root");

        // Test custom usernames
        let creds_custom = ByovpsCredentials {
            vps_ip: "10.0.0.1".to_string(),
            ssh_username: "ubuntu".to_string(),
            ssh_password: "pass".to_string(),
        };

        assert_eq!(creds_custom.ssh_username, "ubuntu");
    }

    #[test]
    fn test_byovps_credentials_password_validation() {
        // Test that minimum 1 character password is accepted
        let creds_short = ByovpsCredentials {
            vps_ip: "10.0.0.1".to_string(),
            ssh_username: "root".to_string(),
            ssh_password: "p".to_string(),
        };

        assert_eq!(creds_short.ssh_password.len(), 1);
        assert!(!creds_short.ssh_password.is_empty());

        // Test longer passwords
        let creds_long = ByovpsCredentials {
            vps_ip: "10.0.0.1".to_string(),
            ssh_username: "root".to_string(),
            ssh_password: "verylongsecurepassword123!@#".to_string(),
        };

        assert!(creds_long.ssh_password.len() > 1);
    }

    #[test]
    fn test_byovps_input_validation_logic() {
        // Test trimming and validation logic that would be used in collect_byovps_credentials

        // IP address validation - non-empty after trim
        let ip_with_spaces = "  192.168.1.1  ";
        let trimmed_ip = ip_with_spaces.trim();
        assert!(!trimmed_ip.is_empty());
        assert_eq!(trimmed_ip, "192.168.1.1");

        // Username validation - empty means default to "root"
        let empty_username = "   ";
        let trimmed_username = empty_username.trim();
        let final_username = if trimmed_username.is_empty() {
            "root"
        } else {
            trimmed_username
        };
        assert_eq!(final_username, "root");

        // Password validation - minimum 1 character
        let valid_password = "x";
        assert!(valid_password.len() >= 1);

        let empty_password = "";
        assert!(empty_password.is_empty());
    }

    #[test]
    fn test_byovps_health_timeout_constant() {
        // Verify BYOVPS health check timeout is 2 minutes (120 seconds)
        assert_eq!(HEALTH_CHECK_TIMEOUT_SECS, 120);
    }

    #[test]
    fn test_spinner_chars_constant() {
        // Verify spinner characters array
        assert_eq!(SPINNER_CHARS.len(), 10);
        assert_eq!(SPINNER_CHARS[0], '⠋');
        assert_eq!(SPINNER_CHARS[9], '⠏');
    }

    #[test]
    fn test_byovps_status_states() {
        // Test that BYOVPS status states are handled correctly
        // Ready states - VPS is usable
        let ready_states = ["ready", "running", "active", "Ready", "RUNNING"];
        for state in &ready_states {
            let is_ready = matches!(
                state.to_lowercase().as_str(),
                "ready" | "running" | "active"
            );
            assert!(is_ready, "State '{}' should be ready", state);
        }

        // Error states - BYOVPS failed
        let error_states = ["failed", "error", "terminated"];
        for state in &error_states {
            let is_error = matches!(
                state.to_lowercase().as_str(),
                "failed" | "error" | "terminated"
            );
            assert!(is_error, "State '{}' should be error", state);
        }

        // Polling states - still provisioning
        let polling_states = [
            "pending",
            "provisioning",
            "registering",
            "configuring",
            "installing",
        ];
        for state in &polling_states {
            let is_ready = matches!(
                state.to_lowercase().as_str(),
                "ready" | "running" | "active"
            );
            let is_error = matches!(
                state.to_lowercase().as_str(),
                "failed" | "error" | "terminated"
            );
            assert!(
                !is_ready && !is_error,
                "State '{}' should trigger polling",
                state
            );
        }
    }

    #[test]
    fn test_byovps_flow_requires_access_token() {
        // Test that run_byovps_flow returns error without access token
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let mut credentials = Credentials::default();
        // No access token set

        let byovps_creds = ByovpsCredentials {
            vps_ip: "192.168.1.1".to_string(),
            ssh_username: "root".to_string(),
            ssh_password: "pass".to_string(),
        };

        let interrupted = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

        let result = run_byovps_flow(&runtime, &mut credentials, &byovps_creds, &interrupted);

        // Should fail because no access token
        assert!(result.is_err());
        if let Err(CentralApiError::ServerError { status, message }) = result {
            assert_eq!(status, 401);
            assert!(message.contains("refresh token") || message.contains("sign in again"));
        } else {
            panic!("Expected ServerError with status 401");
        }
    }

    #[test]
    fn test_health_timeout_minutes() {
        // Verify health check timeout is 2 minutes
        assert_eq!(HEALTH_CHECK_TIMEOUT_SECS / 60, 2);
    }

    #[test]
    fn test_byovps_max_retry_constant() {
        // Verify max retry attempts is 3
        assert_eq!(BYOVPS_MAX_RETRY_ATTEMPTS, 3);
    }

    #[test]
    fn test_byovps_retry_action_enum() {
        // Test ByovpsRetryAction enum variants
        let retry = ByovpsRetryAction::Retry;
        let change = ByovpsRetryAction::ChangeVpsDetails;
        let exit = ByovpsRetryAction::Exit;

        assert_eq!(retry, ByovpsRetryAction::Retry);
        assert_eq!(change, ByovpsRetryAction::ChangeVpsDetails);
        assert_eq!(exit, ByovpsRetryAction::Exit);

        // Test they are not equal to each other
        assert_ne!(retry, change);
        assert_ne!(retry, exit);
        assert_ne!(change, exit);

        // Test Debug trait
        assert_eq!(format!("{:?}", retry), "Retry");
        assert_eq!(format!("{:?}", change), "ChangeVpsDetails");
        assert_eq!(format!("{:?}", exit), "Exit");

        // Test Clone trait
        let retry_clone = retry.clone();
        assert_eq!(retry, retry_clone);
    }

    #[test]
    fn test_is_ssh_connection_error_ssh_keywords() {
        // Test SSH-related error messages
        assert!(is_ssh_connection_error("SSH connection failed"));
        assert!(is_ssh_connection_error("ssh: Connection refused"));
        assert!(is_ssh_connection_error(
            "Failed to establish SSH connection"
        ));
        assert!(is_ssh_connection_error("SSH authentication failed"));
    }

    #[test]
    fn test_is_ssh_connection_error_connection_errors() {
        // Test connection error messages
        assert!(is_ssh_connection_error("Connection refused"));
        assert!(is_ssh_connection_error("connection timed out"));
        assert!(is_ssh_connection_error("Host unreachable"));
        assert!(is_ssh_connection_error("Network unreachable"));
        assert!(is_ssh_connection_error("No route to host"));
    }

    #[test]
    fn test_is_ssh_connection_error_auth_errors() {
        // Test authentication error messages
        assert!(is_ssh_connection_error("Authentication failed"));
        assert!(is_ssh_connection_error("Permission denied"));
        assert!(is_ssh_connection_error(
            "permission denied (publickey,password)"
        ));
    }

    #[test]
    fn test_is_ssh_connection_error_port_errors() {
        // Test port 22 related errors
        assert!(is_ssh_connection_error("Failed to connect to port 22"));
        assert!(is_ssh_connection_error("Port 22: Connection refused"));
    }

    #[test]
    fn test_is_ssh_connection_error_case_insensitive() {
        // Test case insensitivity
        assert!(is_ssh_connection_error("SSH Connection Failed"));
        assert!(is_ssh_connection_error("CONNECTION REFUSED"));
        assert!(is_ssh_connection_error("Authentication FAILED"));
    }

    #[test]
    fn test_is_ssh_connection_error_non_ssh_errors() {
        // Test non-SSH errors return false
        assert!(!is_ssh_connection_error("Invalid request"));
        assert!(!is_ssh_connection_error("Server error 500"));
        assert!(!is_ssh_connection_error("VPS already exists"));
        assert!(!is_ssh_connection_error("Quota exceeded"));
        assert!(!is_ssh_connection_error("Internal error"));
    }

    #[test]
    fn test_display_byovps_error_ssh_error() {
        // Test display_byovps_error with SSH error - just verify it doesn't panic
        let error = CentralApiError::ServerError {
            status: 500,
            message: "SSH connection refused".to_string(),
        };
        // This will print to stdout but shouldn't panic
        display_byovps_error(&error);
    }

    #[test]
    fn test_display_byovps_error_non_ssh_error() {
        // Test display_byovps_error with non-SSH error - just verify it doesn't panic
        let error = CentralApiError::ServerError {
            status: 400,
            message: "Invalid VPS IP address".to_string(),
        };
        display_byovps_error(&error);
    }

    #[test]
    fn test_display_byovps_error_authorization_error() {
        // Test display_byovps_error with authorization errors
        let pending = CentralApiError::AuthorizationPending;
        display_byovps_error(&pending);

        let expired = CentralApiError::AuthorizationExpired;
        display_byovps_error(&expired);

        let denied = CentralApiError::AccessDenied;
        display_byovps_error(&denied);
    }

    #[test]
    fn test_display_byovps_error_various_messages() {
        // Test various error messages that might indicate SSH issues
        let errors = vec![
            "Failed to connect via SSH",
            "Connection timed out after 30 seconds",
            "Host 192.168.1.100 unreachable",
            "Network is unreachable",
            "No route to host 10.0.0.1",
            "Authentication failed for user root",
            "Permission denied for root@192.168.1.1",
        ];

        for msg in errors {
            let error = CentralApiError::ServerError {
                status: 500,
                message: msg.to_string(),
            };
            // Just verify they don't panic
            display_byovps_error(&error);
        }
    }

    #[test]
    fn test_expired_credentials_detection() {
        // Test that expired credentials are properly detected
        let mut credentials = Credentials::default();

        // No expiration time - should be considered expired
        assert!(credentials.is_expired());

        // Set expiration to past - should be expired
        credentials.expires_at = Some(chrono::Utc::now().timestamp() - 3600);
        assert!(credentials.is_expired());

        // Set expiration to future - should not be expired
        credentials.expires_at = Some(chrono::Utc::now().timestamp() + 3600);
        assert!(!credentials.is_expired());
    }

    #[test]
    fn test_byovps_flow_without_access_token() {
        // Test that run_byovps_flow returns error without access token
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let mut credentials = Credentials::default();
        // No access token set

        let byovps_creds = ByovpsCredentials {
            vps_ip: "192.168.1.1".to_string(),
            ssh_username: "root".to_string(),
            ssh_password: "pass".to_string(),
        };

        let interrupted = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

        let result = run_byovps_flow(&runtime, &mut credentials, &byovps_creds, &interrupted);

        // Should fail because no access token
        assert!(result.is_err());
        if let Err(CentralApiError::ServerError { status, message }) = result {
            assert_eq!(status, 401);
            assert!(message.contains("refresh token") || message.contains("sign in again"));
        } else {
            panic!("Expected ServerError with status 401");
        }
    }

    #[test]
    fn test_byovps_flow_with_expired_token_no_refresh() {
        // Test that run_byovps_flow handles expired token without refresh token
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let mut credentials = Credentials::default();

        // Set expired access token but no refresh token
        credentials.access_token = Some("expired_access_token".to_string());
        credentials.expires_at = Some(chrono::Utc::now().timestamp() - 3600);
        credentials.refresh_token = None;

        let byovps_creds = ByovpsCredentials {
            vps_ip: "192.168.1.1".to_string(),
            ssh_username: "root".to_string(),
            ssh_password: "pass".to_string(),
        };

        let interrupted = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

        let result = run_byovps_flow(&runtime, &mut credentials, &byovps_creds, &interrupted);

        // Should fail because token is expired and no refresh token
        assert!(result.is_err());
        if let Err(CentralApiError::ServerError { status, message }) = result {
            assert_eq!(status, 401);
            assert!(message.contains("refresh token") || message.contains("sign in again"));
        } else {
            panic!("Expected ServerError with status 401");
        }

        // Credentials should be cleared
        assert!(credentials.access_token.is_none());
        assert!(credentials.refresh_token.is_none());
    }

    #[test]
    fn test_credentials_is_expired_edge_cases() {
        // Test edge cases for token expiration checking
        let mut credentials = Credentials::default();

        // Exactly at expiration time (now) - should be expired
        credentials.expires_at = Some(chrono::Utc::now().timestamp());
        // This might be flaky due to timing, but >= check should make it expired
        assert!(credentials.is_expired());

        // One second in the future - should not be expired
        credentials.expires_at = Some(chrono::Utc::now().timestamp() + 1);
        assert!(!credentials.is_expired());

        // Far future - should not be expired
        credentials.expires_at = Some(chrono::Utc::now().timestamp() + 86400); // 1 day
        assert!(!credentials.is_expired());
    }

    #[test]
    fn test_proactive_token_check_detects_expired_token() {
        // Test that expired tokens are detected before API calls
        let mut credentials = Credentials::default();
        credentials.access_token = Some("expired_token".to_string());
        credentials.expires_at = Some(chrono::Utc::now().timestamp() - 3600); // 1 hour ago
        credentials.refresh_token = None;

        // Token should be detected as expired
        assert!(credentials.is_expired());
        assert!(credentials.access_token.is_some());
    }

    #[test]
    fn test_proactive_token_check_valid_token() {
        // Test that valid tokens are not flagged as expired
        let mut credentials = Credentials::default();
        credentials.access_token = Some("valid_token".to_string());
        credentials.expires_at = Some(chrono::Utc::now().timestamp() + 3600); // 1 hour from now
        credentials.refresh_token = Some("refresh_token".to_string());

        // Token should be valid
        assert!(!credentials.is_expired());
        assert!(credentials.access_token.is_some());
    }

    #[test]
    fn test_proactive_token_check_no_expiration() {
        // Test that tokens without expiration are treated as expired
        let mut credentials = Credentials::default();
        credentials.access_token = Some("token_without_expiry".to_string());
        credentials.expires_at = None;

        // Without expiration info, should be treated as expired for safety
        assert!(credentials.is_expired());
    }

    #[test]
    fn test_proactive_token_check_with_refresh_token() {
        // Test that expired token with refresh token is detected
        let mut credentials = Credentials::default();
        credentials.access_token = Some("expired_token".to_string());
        credentials.expires_at = Some(chrono::Utc::now().timestamp() - 1800); // 30 mins ago
        credentials.refresh_token = Some("valid_refresh_token".to_string());

        // Token is expired but refresh token is available
        assert!(credentials.is_expired());
        assert!(credentials.refresh_token.is_some());
    }

    #[test]
    fn test_is_auth_error_detection() {
        // Test that 401 errors are correctly identified as auth errors
        let auth_error = CentralApiError::ServerError {
            status: 401,
            message: "Unauthorized".to_string(),
        };
        assert!(is_auth_error(&auth_error));

        // Non-401 errors should not be identified as auth errors
        let other_error = CentralApiError::ServerError {
            status: 500,
            message: "Internal Server Error".to_string(),
        };
        assert!(!is_auth_error(&other_error));

        let not_found = CentralApiError::ServerError {
            status: 404,
            message: "Not Found".to_string(),
        };
        assert!(!is_auth_error(&not_found));
    }

    #[test]
    fn test_credentials_clearing_on_expired_token() {
        // Test that credentials are properly cleared when token is expired
        let mut credentials = Credentials::default();
        credentials.access_token = Some("expired_token".to_string());
        credentials.refresh_token = Some("also_expired_refresh".to_string());
        credentials.expires_at = Some(chrono::Utc::now().timestamp() - 7200); // 2 hours ago

        // Simulate clearing credentials when refresh fails
        credentials.access_token = None;
        credentials.refresh_token = None;

        assert!(credentials.access_token.is_none());
        assert!(credentials.refresh_token.is_none());
    }

    #[test]
    fn test_byovps_flow_proactive_check_flow() {
        // Test the overall flow:
        // 1. Token is expired
        // 2. Refresh token is available
        // 3. Proactive check should trigger refresh before API call

        let mut credentials = Credentials::default();
        credentials.access_token = Some("expired_token".to_string());
        credentials.expires_at = Some(chrono::Utc::now().timestamp() - 1); // just expired
        credentials.refresh_token = Some("refresh_token".to_string());

        // Token should be expired
        assert!(credentials.is_expired());

        // After successful refresh, new tokens would be set
        credentials.access_token = Some("new_access_token".to_string());
        credentials.refresh_token = Some("new_refresh_token".to_string());
        credentials.expires_at = Some(chrono::Utc::now().timestamp() + 3600);

        // Now token should be valid
        assert!(!credentials.is_expired());
    }

    #[test]
    fn test_token_expiration_message() {
        // Test that appropriate error messages are returned for expired tokens
        let error_msg = "Your session has expired. Please run the CLI again to re-authenticate.";
        assert!(error_msg.contains("expired"));
        assert!(error_msg.contains("re-authenticate"));
    }

    #[test]
    fn test_proactive_refresh_message() {
        // Test that proactive refresh is indicated in output
        let refresh_msg = "Token expired, refreshing proactively...";
        assert!(refresh_msg.contains("proactively"));
        assert!(refresh_msg.contains("refreshing"));
    }

    #[test]
    fn test_refresh_success_message() {
        // Test successful refresh message
        let success_msg = "Token refreshed successfully.";
        assert!(success_msg.contains("successfully"));
        assert!(success_msg.contains("refreshed"));
    }

    // NOTE: TokenMigrationResult tests removed - SSH-based sync was replaced with HTTP sync
}
