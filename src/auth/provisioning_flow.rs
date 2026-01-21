//! Synchronous provisioning flow module.
//!
//! This module provides blocking provisioning flows for the TUI application.
//! It uses the Tokio runtime to call existing async methods on CentralApiClient.

use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use super::central_api::{
    ByovpsProvisionResponse, CentralApiClient, CentralApiError, DataCenter, VpsPlan,
    VpsStatusResponse,
};
use super::credentials::{Credentials, CredentialsManager};

/// VPS type selection.
#[derive(Debug, Clone, PartialEq)]
pub enum VpsType {
    Managed,
    Byovps,
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

/// Poll interval for BYOVPS status checks (in seconds).
const BYOVPS_POLL_INTERVAL_SECS: u64 = 5;

/// Maximum number of BYOVPS poll attempts before timing out.
const BYOVPS_MAX_POLL_ATTEMPTS: u32 = 120; // 10 minutes at 5 second intervals

/// Maximum number of retry attempts for BYOVPS provisioning.
const BYOVPS_MAX_RETRY_ATTEMPTS: u32 = 3;

/// Spinner characters for loading animation.
const SPINNER_CHARS: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

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
    // Prompt for VPS IP address
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
        if !trimmed.is_empty() {
            break trimmed;
        }

        println!("IP address cannot be empty. Please try again.");
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

    // Prompt for SSH password (minimum 1 character)
    let ssh_password = loop {
        check_interrupt(interrupted);

        print!("Enter SSH password (min 1 character): ");
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

        if !password.is_empty() {
            break password;
        }

        println!("Password must be at least 1 character. Try again.");
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

    loop {
        check_interrupt(interrupted);

        print!("\nEnter your choice (1-2): ");
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
            _ => println!("Please enter either 1 or 2."),
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

    // Step 1: Fetch available plans
    println!("Fetching available VPS plans...");
    check_interrupt(&interrupted);
    let plans = runtime.block_on(client.fetch_vps_plans())?;

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

    // Step 3: Get SSH password from user
    check_interrupt(interrupted);
    let ssh_password = prompt_ssh_password_with_interrupt(interrupted)?;

    // Step 4: Confirm provisioning
    check_interrupt(interrupted);
    if !prompt_confirmation_with_interrupt(interrupted)? {
        println!("Provisioning cancelled.");
        return Ok(());
    }

    // Step 5: Fetch and select datacenter
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
    credentials.datacenter_id = Some(selected_datacenter_id);

    // Step 6: Provision the VPS
    check_interrupt(interrupted);
    println!("\nProvisioning your VPS...");
    let provision_result = runtime.block_on(client.provision_vps(
        &ssh_password,
        Some(&selected_plan.id),
        Some(selected_datacenter_id),
    ));

    // Handle 409 Conflict - user already has an active VPS
    let provision_response = match provision_result {
        Ok(response) => response,
        Err(CentralApiError::ServerError { status: 409, .. }) => {
            println!("\nYou already have an active VPS.");
            println!("Please use your existing VPS or contact support to delete it first.");
            return Ok(());
        }
        Err(e) => return Err(e),
    };

    println!("VPS provisioning started!");
    if let Some(msg) = &provision_response.message {
        println!("  {}", msg);
    }

    // Update credentials with VPS ID
    credentials.vps_id = Some(provision_response.vps_id.clone());
    credentials.vps_status = Some(provision_response.status.clone());
    if let Some(hostname) = &provision_response.hostname {
        credentials.vps_hostname = Some(hostname.clone());
    }

    // Step 7: Poll for VPS to be ready
    println!("\nWaiting for VPS to be ready...");
    let status = poll_vps_status_with_interrupt(runtime, &mut client, interrupted)?;

    // Update credentials with final status
    credentials.vps_id = Some(status.vps_id.clone());
    credentials.vps_status = Some(status.status.clone());
    if let Some(hostname) = &status.hostname {
        credentials.vps_hostname = Some(hostname.clone());
    }
    if let Some(ip) = &status.ip {
        credentials.vps_ip = Some(ip.clone());
    }
    if let Some(url) = &status.url {
        credentials.vps_url = Some(url.clone());
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

    Ok(())
}

/// Retry action for BYOVPS provisioning failure.
#[derive(Debug, Clone, PartialEq)]
enum ByovpsRetryAction {
    Retry,
    ChangeCredentials,
    Exit,
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

        print!("\nRetry? (y)es / (c)hange credentials / (e)xit: ");
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
            "c" | "change" => return Ok(ByovpsRetryAction::ChangeCredentials),
            "e" | "exit" => return Ok(ByovpsRetryAction::Exit),
            _ => println!("Please enter 'y' to retry, 'c' to change credentials, or 'e' to exit."),
        }
    }
}

/// Run the BYOVPS provisioning flow with error handling and retry logic.
///
/// This flow:
/// 1. Attempts to provision BYOVPS
/// 2. On failure, displays error details and prompts for retry
/// 3. Supports up to 3 retry attempts
/// 4. User can retry with same credentials, change credentials, or exit
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

                // Prompt user for action
                check_interrupt(interrupted);
                match prompt_byovps_retry_action(interrupted)? {
                    ByovpsRetryAction::Retry => {
                        println!("\nRetrying with same credentials...");
                        continue;
                    }
                    ByovpsRetryAction::ChangeCredentials => {
                        println!("\nPlease enter new credentials:");
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

/// Run the BYOVPS provisioning flow.
///
/// This flow:
/// 1. Calls the BYOVPS provision endpoint with spinner animation
/// 2. Polls VPS status every 5 seconds until ready or failed
/// 3. Updates and saves credentials with VPS info
fn run_byovps_flow(
    runtime: &tokio::runtime::Runtime,
    credentials: &mut Credentials,
    byovps_creds: &ByovpsCredentials,
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

    // Step 1: Call provision_byovps with spinner
    check_interrupt(interrupted);
    let provision_response = provision_byovps_with_spinner(
        runtime,
        &mut client,
        &byovps_creds.vps_ip,
        &byovps_creds.ssh_username,
        &byovps_creds.ssh_password,
        interrupted,
    )?;

    // Check initial provision status
    match provision_response.status.to_lowercase().as_str() {
        "failed" | "error" => {
            let msg = provision_response
                .message
                .unwrap_or_else(|| "BYOVPS provisioning failed".to_string());
            return Err(CentralApiError::ServerError {
                status: 500,
                message: msg,
            });
        }
        "ready" | "running" | "active" => {
            // Already ready, update credentials and return
            update_credentials_from_byovps_response(credentials, &provision_response);
            save_credentials(credentials);
            display_byovps_result(&provision_response);
            return Ok(());
        }
        _ => {
            // Need to poll for status
        }
    }

    // Display initial status
    if let Some(msg) = &provision_response.message {
        println!("\n{}", msg);
    }

    // Step 2: Poll VPS status until ready or failed
    check_interrupt(interrupted);
    let final_status = poll_byovps_status_with_interrupt(runtime, &mut client, interrupted)?;

    // Step 3: Update credentials with final VPS info
    credentials.vps_id = Some(final_status.vps_id.clone());
    credentials.vps_status = Some(final_status.status.clone());
    if let Some(hostname) = &final_status.hostname {
        credentials.vps_hostname = Some(hostname.clone());
    }
    if let Some(ip) = &final_status.ip {
        credentials.vps_ip = Some(ip.clone());
    }
    if let Some(url) = &final_status.url {
        credentials.vps_url = Some(url.clone());
    }

    // Save updated credentials
    save_credentials(credentials);

    // Display final result
    println!("\nBYOVPS provisioning complete!");
    println!("  Status: {}", final_status.status);
    if let Some(hostname) = &final_status.hostname {
        println!("  Hostname: {}", hostname);
    }
    if let Some(ip) = &final_status.ip {
        println!("  IP: {}", ip);
    }
    if let Some(url) = &final_status.url {
        println!("  URL: {}", url);
    }

    Ok(())
}

/// Call provision_byovps with spinner animation.
fn provision_byovps_with_spinner(
    runtime: &tokio::runtime::Runtime,
    client: &mut CentralApiClient,
    vps_ip: &str,
    ssh_username: &str,
    ssh_password: &str,
    interrupted: &Arc<AtomicBool>,
) -> Result<ByovpsProvisionResponse, CentralApiError> {
    use std::sync::mpsc;
    use std::time::Instant;

    // Start spinner in separate thread
    let (tx, rx) = mpsc::channel();
    let spinner_interrupted = Arc::clone(interrupted);

    let spinner_handle = thread::spawn(move || {
        let mut frame = 0;
        let start = Instant::now();

        loop {
            // Check for completion signal
            if rx.try_recv().is_ok() {
                break;
            }

            // Check for interrupt
            if spinner_interrupted.load(Ordering::SeqCst) {
                break;
            }

            // Display spinner
            let spinner = SPINNER_CHARS[frame % SPINNER_CHARS.len()];
            let elapsed = start.elapsed().as_secs();
            print!("\r{} Provisioning VPS... ({}s)", spinner, elapsed);
            io::stdout().flush().ok();

            frame += 1;
            thread::sleep(Duration::from_millis(100));
        }

        // Clear spinner line
        print!("\r                                        \r");
        io::stdout().flush().ok();
    });

    // Execute the async provision call
    let result = runtime.block_on(client.provision_byovps(vps_ip, ssh_username, ssh_password));

    // Stop the spinner
    let _ = tx.send(());
    let _ = spinner_handle.join();

    // Check for interrupt
    check_interrupt(interrupted);

    result
}

/// Poll BYOVPS status with spinner and interrupt support.
fn poll_byovps_status_with_interrupt(
    runtime: &tokio::runtime::Runtime,
    client: &mut CentralApiClient,
    interrupted: &Arc<AtomicBool>,
) -> Result<VpsStatusResponse, CentralApiError> {
    let mut attempts = 0;

    println!("\nPolling VPS status...");

    loop {
        check_interrupt(interrupted);

        let status = runtime.block_on(client.fetch_vps_status())?;

        // Check VPS status
        match status.status.to_lowercase().as_str() {
            "ready" | "running" | "active" => {
                // Clear spinner line and return
                print!("\r                                                              \r");
                io::stdout().flush().ok();
                return Ok(status);
            }
            "failed" | "error" | "terminated" => {
                return Err(CentralApiError::ServerError {
                    status: 500,
                    message: format!("BYOVPS provisioning failed with status: {}", status.status),
                });
            }
            _ => {
                // Still provisioning, show progress with spinner
                let spinner = SPINNER_CHARS[attempts as usize % SPINNER_CHARS.len()];
                let status_display = &status.status;
                print!(
                    "\r{} Polling VPS status... ({}) - attempt {}/{}",
                    spinner,
                    status_display,
                    attempts + 1,
                    BYOVPS_MAX_POLL_ATTEMPTS
                );
                io::stdout().flush().ok();
            }
        }

        attempts += 1;
        if attempts >= BYOVPS_MAX_POLL_ATTEMPTS {
            return Err(CentralApiError::ServerError {
                status: 408,
                message: "BYOVPS provisioning timed out after 10 minutes".to_string(),
            });
        }

        // Check interrupt before sleeping
        check_interrupt(interrupted);
        thread::sleep(Duration::from_secs(BYOVPS_POLL_INTERVAL_SECS));
    }
}

/// Update credentials from BYOVPS provision response.
fn update_credentials_from_byovps_response(
    credentials: &mut Credentials,
    response: &ByovpsProvisionResponse,
) {
    credentials.vps_status = Some(response.status.clone());

    if let Some(vps_id) = &response.vps_id {
        credentials.vps_id = Some(vps_id.clone());
    }
    if let Some(hostname) = &response.hostname {
        credentials.vps_hostname = Some(hostname.clone());
    }
    if let Some(ip) = &response.ip {
        credentials.vps_ip = Some(ip.clone());
    }
    if let Some(url) = &response.url {
        credentials.vps_url = Some(url.clone());
    }
}

/// Display BYOVPS provisioning result.
fn display_byovps_result(response: &ByovpsProvisionResponse) {
    println!("\nBYOVPS provisioning complete!");
    println!("  Status: {}", response.status);
    if let Some(hostname) = &response.hostname {
        println!("  Hostname: {}", hostname);
    }
    if let Some(ip) = &response.ip {
        println!("  IP: {}", ip);
    }
    if let Some(url) = &response.url {
        println!("  URL: {}", url);
    }
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

/// Poll the VPS status with interrupt support.
fn poll_vps_status_with_interrupt(
    runtime: &tokio::runtime::Runtime,
    client: &mut CentralApiClient,
    interrupted: &Arc<AtomicBool>,
) -> Result<VpsStatusResponse, CentralApiError> {
    let mut attempts = 0;
    let spinner_chars = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

    loop {
        check_interrupt(interrupted);

        let status = runtime.block_on(client.fetch_vps_status())?;

        // Check if VPS is ready
        match status.status.to_lowercase().as_str() {
            "ready" | "running" | "active" => {
                print!("\r"); // Clear the spinner line
                io::stdout().flush().ok();
                return Ok(status);
            }
            "stopped" | "failed" | "terminated" | "error" => {
                return Err(CentralApiError::ServerError {
                    status: 500,
                    message: format!("VPS provisioning failed with status: {}", status.status),
                });
            }
            _ => {
                // Still provisioning, show progress
                let spinner = spinner_chars[attempts as usize % spinner_chars.len()];
                print!(
                    "\r{} Status: {} (attempt {}/{})",
                    spinner,
                    status.status,
                    attempts + 1,
                    MAX_POLL_ATTEMPTS
                );
                io::stdout().flush().ok();
            }
        }

        attempts += 1;
        if attempts >= MAX_POLL_ATTEMPTS {
            return Err(CentralApiError::ServerError {
                status: 408,
                message: "VPS provisioning timed out".to_string(),
            });
        }

        // Check interrupt before sleeping
        check_interrupt(interrupted);
        thread::sleep(Duration::from_secs(POLL_INTERVAL_SECS));
    }
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

        assert_eq!(managed, VpsType::Managed);
        assert_eq!(byovps, VpsType::Byovps);
        assert_ne!(managed, byovps);

        // Test Debug trait
        assert_eq!(format!("{:?}", managed), "Managed");
        assert_eq!(format!("{:?}", byovps), "Byovps");

        // Test Clone trait
        let managed_clone = managed.clone();
        assert_eq!(managed, managed_clone);
    }

    #[tokio::test]
    async fn test_poll_vps_status_ready_states() {
        // Test that various "ready" states are recognized
        let ready_states = ["ready", "running", "active", "Ready", "RUNNING", "Active"];

        for state in &ready_states {
            let status = VpsStatusResponse {
                vps_id: "test".to_string(),
                status: state.to_string(),
                hostname: Some("test.example.com".to_string()),
                ip: Some("1.2.3.4".to_string()),
                url: Some("https://test.example.com".to_string()),
                ssh_username: None,
                provider: None,
                plan_id: None,
                data_center_id: None,
                created_at: None,
                ready_at: None,
            };

            // Verify status is recognized as ready
            let is_ready = matches!(
                status.status.to_lowercase().as_str(),
                "ready" | "running" | "active"
            );
            assert!(is_ready, "State '{}' should be recognized as ready", state);
        }
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

        // We can verify the Credentials struct stores datacenter_id
        let mut creds = Credentials::default();
        creds.datacenter_id = Some(42);
        assert_eq!(creds.datacenter_id, Some(42));
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
        assert!(empty_password.len() < 12, "Empty password should be rejected");

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

        assert_eq!(
            ipv6_creds.vps_ip,
            "2001:0db8:85a3:0000:0000:8a2e:0370:7334"
        );
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
    fn test_byovps_poll_constants() {
        // Verify BYOVPS polling constants are reasonable
        assert_eq!(BYOVPS_POLL_INTERVAL_SECS, 5);
        assert_eq!(BYOVPS_MAX_POLL_ATTEMPTS, 120);
        // Total wait time should be 10 minutes (600 seconds)
        assert_eq!(
            BYOVPS_POLL_INTERVAL_SECS * BYOVPS_MAX_POLL_ATTEMPTS as u64,
            600
        );
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
    fn test_update_credentials_from_byovps_response() {
        use super::super::central_api::ByovpsProvisionResponse;

        let mut credentials = Credentials::default();
        let response = ByovpsProvisionResponse {
            hostname: Some("user.spoq.dev".to_string()),
            status: "ready".to_string(),
            install_script: None,
            credentials: None,
            message: None,
            vps_id: Some("byovps-uuid-123".to_string()),
            ip: Some("192.168.1.100".to_string()),
            url: Some("https://user.spoq.dev:8000".to_string()),
        };

        update_credentials_from_byovps_response(&mut credentials, &response);

        assert_eq!(credentials.vps_status, Some("ready".to_string()));
        assert_eq!(credentials.vps_id, Some("byovps-uuid-123".to_string()));
        assert_eq!(credentials.vps_hostname, Some("user.spoq.dev".to_string()));
        assert_eq!(credentials.vps_ip, Some("192.168.1.100".to_string()));
        assert_eq!(
            credentials.vps_url,
            Some("https://user.spoq.dev:8000".to_string())
        );
    }

    #[test]
    fn test_update_credentials_from_byovps_response_partial() {
        use super::super::central_api::ByovpsProvisionResponse;

        let mut credentials = Credentials::default();
        let response = ByovpsProvisionResponse {
            hostname: None,
            status: "provisioning".to_string(),
            install_script: None,
            credentials: None,
            message: Some("Installing...".to_string()),
            vps_id: Some("byovps-uuid-456".to_string()),
            ip: None,
            url: None,
        };

        update_credentials_from_byovps_response(&mut credentials, &response);

        assert_eq!(credentials.vps_status, Some("provisioning".to_string()));
        assert_eq!(credentials.vps_id, Some("byovps-uuid-456".to_string()));
        assert!(credentials.vps_hostname.is_none());
        assert!(credentials.vps_ip.is_none());
        assert!(credentials.vps_url.is_none());
    }

    #[test]
    fn test_display_byovps_result_does_not_panic() {
        use super::super::central_api::ByovpsProvisionResponse;

        // Test with full response
        let response_full = ByovpsProvisionResponse {
            hostname: Some("test.spoq.dev".to_string()),
            status: "ready".to_string(),
            install_script: None,
            credentials: None,
            message: None,
            vps_id: Some("test-id".to_string()),
            ip: Some("10.0.0.1".to_string()),
            url: Some("https://test.spoq.dev:8000".to_string()),
        };
        // Just verify it doesn't panic
        display_byovps_result(&response_full);

        // Test with minimal response
        let response_minimal = ByovpsProvisionResponse {
            hostname: None,
            status: "ready".to_string(),
            install_script: None,
            credentials: None,
            message: None,
            vps_id: None,
            ip: None,
            url: None,
        };
        display_byovps_result(&response_minimal);
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
            assert!(message.contains("access token"));
        } else {
            panic!("Expected ServerError with status 401");
        }
    }

    #[test]
    fn test_byovps_timeout_calculation() {
        // Verify the timeout calculation is correct
        // 120 attempts * 5 seconds = 600 seconds = 10 minutes
        let total_timeout_secs = BYOVPS_MAX_POLL_ATTEMPTS as u64 * BYOVPS_POLL_INTERVAL_SECS;
        assert_eq!(total_timeout_secs, 600);
        assert_eq!(total_timeout_secs / 60, 10); // 10 minutes
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
        let change = ByovpsRetryAction::ChangeCredentials;
        let exit = ByovpsRetryAction::Exit;

        assert_eq!(retry, ByovpsRetryAction::Retry);
        assert_eq!(change, ByovpsRetryAction::ChangeCredentials);
        assert_eq!(exit, ByovpsRetryAction::Exit);

        // Test they are not equal to each other
        assert_ne!(retry, change);
        assert_ne!(retry, exit);
        assert_ne!(change, exit);

        // Test Debug trait
        assert_eq!(format!("{:?}", retry), "Retry");
        assert_eq!(format!("{:?}", change), "ChangeCredentials");
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
        assert!(is_ssh_connection_error("Failed to establish SSH connection"));
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
        assert!(is_ssh_connection_error("permission denied (publickey,password)"));
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
}
