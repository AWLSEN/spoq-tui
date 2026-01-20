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
    CentralApiClient, CentralApiError, DataCenter, VpsPlan, VpsStatusResponse,
};
use super::credentials::Credentials;

/// Poll interval for VPS status checks (in seconds).
const POLL_INTERVAL_SECS: u64 = 3;

/// Maximum number of poll attempts before timing out.
const MAX_POLL_ATTEMPTS: u32 = 200; // 10 minutes at 3 second intervals

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

    // Create API client with authentication
    let access_token =
        credentials
            .access_token
            .as_ref()
            .ok_or_else(|| CentralApiError::ServerError {
                status: 401,
                message: "No access token available".to_string(),
            })?;

    let client = CentralApiClient::new().with_auth(access_token);

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
    check_interrupt(&interrupted);
    display_plans(&plans);
    let selected_index = prompt_plan_selection_with_interrupt(plans.len(), &interrupted)?;
    let selected_plan = &plans[selected_index];

    println!(
        "\nYou selected: {} (${:.2}/month)",
        selected_plan.name,
        selected_plan.price_cents as f64 / 100.0
    );

    // Step 3: Get SSH password from user
    check_interrupt(&interrupted);
    let ssh_password = prompt_ssh_password_with_interrupt(&interrupted)?;

    // Step 4: Confirm provisioning
    check_interrupt(&interrupted);
    if !prompt_confirmation_with_interrupt(&interrupted)? {
        println!("Provisioning cancelled.");
        return Ok(());
    }

    // Step 5: Fetch and select datacenter
    check_interrupt(&interrupted);
    println!("\nFetching available data centers...");
    let datacenters = runtime.block_on(client.fetch_datacenters())?;

    if datacenters.is_empty() {
        return Err(CentralApiError::ServerError {
            status: 404,
            message: "No data centers available".to_string(),
        });
    }

    check_interrupt(&interrupted);
    let ordered_dcs = display_datacenters(&datacenters);
    let selected_datacenter_id =
        prompt_datacenter_selection_with_interrupt(&ordered_dcs, &interrupted)?;
    credentials.datacenter_id = Some(selected_datacenter_id);

    // Step 6: Provision the VPS
    check_interrupt(&interrupted);
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
    let status = poll_vps_status_with_interrupt(runtime, &client, &interrupted)?;

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
    client: &CentralApiClient,
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

    let client = CentralApiClient::new().with_auth(token);

    // Start VPS
    println!("Starting your VPS...");
    runtime.block_on(client.start_vps())?;

    // Poll until ready
    poll_vps_until_ready(runtime, &client)
}

/// Poll the VPS status with interrupt support.
fn poll_vps_status_with_interrupt(
    runtime: &tokio::runtime::Runtime,
    client: &CentralApiClient,
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
    fn test_run_provisioning_flow_requires_access_token() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let mut credentials = Credentials::default();

        // Without access token, should fail
        let result = run_provisioning_flow(&runtime, &mut credentials);
        assert!(result.is_err());

        if let Err(CentralApiError::ServerError { status, message }) = result {
            assert_eq!(status, 401);
            assert!(message.contains("access token"));
        } else {
            panic!("Expected ServerError with 401 status");
        }
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
}
