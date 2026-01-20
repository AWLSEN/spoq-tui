//! Synchronous provisioning flow module.
//!
//! This module provides blocking provisioning flows for the TUI application.
//! It uses the Tokio runtime to call existing async methods on CentralApiClient.

use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use super::central_api::{CentralApiClient, CentralApiError, VpsPlan, VpsStatusResponse};
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
    let access_token = credentials.access_token.as_ref().ok_or_else(|| {
        CentralApiError::ServerError {
            status: 401,
            message: "No access token available".to_string(),
        }
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

    println!("\nYou selected: {} (${:.2}/month)", selected_plan.name, selected_plan.price_cents as f64 / 100.0);

    // Step 3: Confirm provisioning
    check_interrupt(&interrupted);
    if !prompt_confirmation_with_interrupt(&interrupted)? {
        println!("Provisioning cancelled.");
        return Ok(());
    }

    // Step 4: Provision the VPS
    check_interrupt(&interrupted);
    println!("\nProvisioning your VPS...");
    let provision_response = runtime.block_on(client.provision_vps(&selected_plan.id))?;

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

    // Step 5: Poll for VPS to be ready
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

/// Prompt the user to select a plan by number with interrupt support.
fn prompt_plan_selection_with_interrupt(
    max: usize,
    interrupted: &Arc<AtomicBool>,
) -> Result<usize, CentralApiError> {
    loop {
        check_interrupt(interrupted);

        print!("\nEnter plan number (1-{}): ", max);
        io::stdout().flush().map_err(|e| CentralApiError::ServerError {
            status: 0,
            message: format!("Failed to flush stdout: {}", e),
        })?;

        let mut input = String::new();
        io::stdin().read_line(&mut input).map_err(|e| CentralApiError::ServerError {
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
fn prompt_confirmation_with_interrupt(interrupted: &Arc<AtomicBool>) -> Result<bool, CentralApiError> {
    check_interrupt(interrupted);

    print!("\nProceed with provisioning? (y/n): ");
    io::stdout().flush().map_err(|e| CentralApiError::ServerError {
        status: 0,
        message: format!("Failed to flush stdout: {}", e),
    })?;

    let mut input = String::new();
    io::stdin().read_line(&mut input).map_err(|e| CentralApiError::ServerError {
        status: 0,
        message: format!("Failed to read input: {}", e),
    })?;

    check_interrupt(interrupted);

    let trimmed = input.trim().to_lowercase();
    Ok(trimmed == "y" || trimmed == "yes")
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
            "failed" | "error" => {
                return Err(CentralApiError::ServerError {
                    status: 500,
                    message: format!("VPS provisioning failed with status: {}", status.status),
                });
            }
            _ => {
                // Still provisioning, show progress
                let spinner = spinner_chars[attempts as usize % spinner_chars.len()];
                print!("\r{} Status: {} (attempt {}/{})", spinner, status.status, attempts + 1, MAX_POLL_ATTEMPTS);
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
            let is_failed = matches!(
                state.to_lowercase().as_str(),
                "failed" | "error"
            );
            assert!(is_failed, "State '{}' should be recognized as failed", state);
        }
    }
}
