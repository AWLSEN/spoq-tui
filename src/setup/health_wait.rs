//! Health wait module for VPS provisioning flow.
//!
//! This module waits for the conductor to become healthy after VPS provisioning.
//! It polls the health endpoint until the conductor reports healthy status.

use reqwest::Client;
use serde::Deserialize;
use std::io::{self, Write};
use std::time::Duration;
use tokio::time::{sleep, Instant};

/// Default timeout for health wait (5 minutes)
pub const DEFAULT_HEALTH_TIMEOUT_SECS: u64 = 300;

/// Interval between health check polls (10 seconds)
const POLL_INTERVAL_SECS: u64 = 10;

/// Error type for health wait operations
#[derive(Debug)]
pub enum HealthWaitError {
    /// HTTP request failed
    Http(reqwest::Error),
    /// Timeout waiting for healthy status
    Timeout { waited_secs: u64 },
    /// Health check returned unhealthy status
    Unhealthy { message: String },
}

impl std::fmt::Display for HealthWaitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthWaitError::Http(e) => write!(f, "HTTP error: {}", e),
            HealthWaitError::Timeout { waited_secs } => {
                write!(
                    f,
                    "Timeout after {} seconds waiting for conductor to become healthy",
                    waited_secs
                )
            }
            HealthWaitError::Unhealthy { message } => {
                write!(f, "Conductor unhealthy: {}", message)
            }
        }
    }
}

impl std::error::Error for HealthWaitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            HealthWaitError::Http(e) => Some(e),
            HealthWaitError::Timeout { .. } => None,
            HealthWaitError::Unhealthy { .. } => None,
        }
    }
}

impl From<reqwest::Error> for HealthWaitError {
    fn from(e: reqwest::Error) -> Self {
        HealthWaitError::Http(e)
    }
}

/// Health check response from conductor
#[derive(Debug, Deserialize)]
pub struct HealthResponse {
    pub status: String,
}

/// Result of a single health check attempt
#[derive(Debug)]
pub enum HealthCheckStatus {
    /// Conductor is healthy
    Healthy,
    /// Conductor responded but is not healthy
    Unhealthy(String),
    /// Request failed (network error, timeout, etc.)
    Unreachable(String),
}

/// Perform a single health check against the conductor.
///
/// # Arguments
/// * `client` - HTTP client to use
/// * `vps_url` - Base URL of the VPS (e.g., "https://vps.example.com")
///
/// # Returns
/// The health check status
async fn check_health(client: &Client, vps_url: &str) -> HealthCheckStatus {
    let url = format!("{}/health", vps_url.trim_end_matches('/'));

    match client
        .get(&url)
        .timeout(Duration::from_secs(10))
        .send()
        .await
    {
        Ok(response) => {
            if response.status().is_success() {
                match response.json::<HealthResponse>().await {
                    Ok(health) => {
                        if health.status == "healthy" {
                            HealthCheckStatus::Healthy
                        } else {
                            HealthCheckStatus::Unhealthy(health.status)
                        }
                    }
                    Err(e) => HealthCheckStatus::Unhealthy(format!("Invalid response: {}", e)),
                }
            } else {
                HealthCheckStatus::Unhealthy(format!("HTTP {}", response.status()))
            }
        }
        Err(e) => HealthCheckStatus::Unreachable(e.to_string()),
    }
}

/// Wait for the conductor to become healthy.
///
/// Polls the health endpoint every 10 seconds until the conductor reports
/// healthy status or the timeout is reached.
///
/// # Arguments
/// * `vps_url` - Base URL of the VPS (e.g., "https://vps.example.com")
/// * `timeout_secs` - Maximum time to wait in seconds (default: 300)
///
/// # Returns
/// `Ok(())` when conductor is healthy, `Err` on timeout or error
///
/// # Example
/// ```no_run
/// use spoq::setup::health_wait::wait_for_health;
///
/// async fn example() -> Result<(), Box<dyn std::error::Error>> {
///     wait_for_health("https://my-vps.spoq.dev", 300).await?;
///     Ok(())
/// }
/// ```
pub async fn wait_for_health(vps_url: &str, timeout_secs: u64) -> Result<(), HealthWaitError> {
    let client = Client::builder()
        .timeout(Duration::from_secs(15))
        .build()?;

    let start = Instant::now();
    let timeout = Duration::from_secs(timeout_secs);

    print!("Waiting for VPS to become healthy");
    let _ = io::stdout().flush();

    loop {
        // Check if we've exceeded the timeout
        let elapsed = start.elapsed();
        if elapsed >= timeout {
            println!(" timeout!");
            return Err(HealthWaitError::Timeout {
                waited_secs: elapsed.as_secs(),
            });
        }

        // Perform health check
        match check_health(&client, vps_url).await {
            HealthCheckStatus::Healthy => {
                println!(" healthy!");
                return Ok(());
            }
            HealthCheckStatus::Unhealthy(msg) => {
                // Log but continue waiting - conductor might still be starting
                tracing::debug!("Health check returned unhealthy: {}", msg);
            }
            HealthCheckStatus::Unreachable(msg) => {
                // Log but continue waiting - VPS might still be booting
                tracing::debug!("Health check unreachable: {}", msg);
            }
        }

        // Show progress
        print!(".");
        let _ = io::stdout().flush();

        // Wait before next check
        sleep(Duration::from_secs(POLL_INTERVAL_SECS)).await;
    }
}

/// Wait for health with progress callback.
///
/// Like `wait_for_health` but allows a custom callback for progress updates.
/// This is useful for TUI applications that want to display progress differently.
///
/// # Arguments
/// * `vps_url` - Base URL of the VPS
/// * `timeout_secs` - Maximum time to wait in seconds
/// * `on_progress` - Callback invoked with (attempt_number, elapsed_secs, status_message)
///
/// # Returns
/// `Ok(())` when conductor is healthy, `Err` on timeout or error
pub async fn wait_for_health_with_progress<F>(
    vps_url: &str,
    timeout_secs: u64,
    mut on_progress: F,
) -> Result<(), HealthWaitError>
where
    F: FnMut(u32, u64, &str),
{
    let client = Client::builder()
        .timeout(Duration::from_secs(15))
        .build()?;

    let start = Instant::now();
    let timeout = Duration::from_secs(timeout_secs);
    let mut attempt: u32 = 0;

    loop {
        attempt += 1;
        let elapsed = start.elapsed();

        // Check if we've exceeded the timeout
        if elapsed >= timeout {
            on_progress(attempt, elapsed.as_secs(), "Timeout");
            return Err(HealthWaitError::Timeout {
                waited_secs: elapsed.as_secs(),
            });
        }

        // Perform health check
        match check_health(&client, vps_url).await {
            HealthCheckStatus::Healthy => {
                on_progress(attempt, elapsed.as_secs(), "Healthy");
                return Ok(());
            }
            HealthCheckStatus::Unhealthy(msg) => {
                on_progress(attempt, elapsed.as_secs(), &format!("Unhealthy: {}", msg));
            }
            HealthCheckStatus::Unreachable(msg) => {
                on_progress(attempt, elapsed.as_secs(), &format!("Unreachable: {}", msg));
            }
        }

        // Wait before next check
        sleep(Duration::from_secs(POLL_INTERVAL_SECS)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_wait_error_display() {
        let err = HealthWaitError::Timeout { waited_secs: 300 };
        let display = format!("{}", err);
        assert!(display.contains("300"));
        assert!(display.contains("Timeout"));

        let err = HealthWaitError::Unhealthy {
            message: "starting".to_string(),
        };
        let display = format!("{}", err);
        assert!(display.contains("starting"));
    }

    #[test]
    fn test_health_check_status_debug() {
        let status = HealthCheckStatus::Healthy;
        assert!(format!("{:?}", status).contains("Healthy"));

        let status = HealthCheckStatus::Unhealthy("starting".to_string());
        assert!(format!("{:?}", status).contains("starting"));

        let status = HealthCheckStatus::Unreachable("connection refused".to_string());
        assert!(format!("{:?}", status).contains("connection refused"));
    }

    #[tokio::test]
    async fn test_check_health_unreachable() {
        let client = Client::new();
        // Use an invalid URL that will fail to connect
        let status = check_health(&client, "http://127.0.0.1:1").await;
        assert!(matches!(status, HealthCheckStatus::Unreachable(_)));
    }

    #[tokio::test]
    async fn test_wait_for_health_timeout() {
        // Very short timeout should fail immediately
        let result = wait_for_health("http://127.0.0.1:1", 1).await;
        assert!(matches!(result, Err(HealthWaitError::Timeout { .. })));
    }

    #[tokio::test]
    async fn test_wait_for_health_with_progress_timeout() {
        let mut progress_calls = 0;
        let result = wait_for_health_with_progress("http://127.0.0.1:1", 1, |_, _, _| {
            progress_calls += 1;
        })
        .await;

        assert!(matches!(result, Err(HealthWaitError::Timeout { .. })));
        assert!(progress_calls >= 1); // Should have at least one progress call
    }

    #[test]
    fn test_default_timeout() {
        assert_eq!(DEFAULT_HEALTH_TIMEOUT_SECS, 300);
    }
}
