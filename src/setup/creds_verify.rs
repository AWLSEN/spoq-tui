//! Credentials verification module for SPOQ setup.
//!
//! This module implements Step 5 of the SPOQ setup flow:
//! verifying that synced credentials (GitHub CLI and Claude Code)
//! work correctly on the VPS.

use ssh2::Session;
use std::io::Read;
use std::net::TcpStream;
use thiserror::Error;

/// Error type for credentials verification.
#[derive(Debug, Error)]
pub enum VerifyError {
    /// Failed to establish TCP connection
    #[error("Failed to establish TCP connection to VPS: {0}")]
    TcpConnection(#[from] std::io::Error),

    /// SSH session error
    #[error("SSH error: {0}")]
    Ssh(#[from] ssh2::Error),

    /// Authentication failed
    #[error("SSH authentication failed - not authenticated after userauth_password")]
    AuthFailed,

    /// Credential verification failed
    #[error("Credential verification failed: {0}")]
    VerificationFailed(String),
}

/// Result of credentials verification on the VPS.
#[derive(Debug, Clone, PartialEq)]
pub struct VerifyResult {
    /// Whether GitHub CLI authentication succeeded
    pub github_ok: bool,
    /// Whether Claude Code authentication succeeded
    pub claude_ok: bool,
}

impl VerifyResult {
    /// Returns true if both GitHub and Claude are authenticated.
    pub fn all_ok(&self) -> bool {
        self.github_ok && self.claude_ok
    }
}

/// Verify that credentials work on the VPS.
///
/// This function connects to the VPS via SSH and runs:
/// 1. `gh auth status` - checks for "Logged in" in output
/// 2. `claude -p 'say OK'` - checks for successful exit code (uses script for TTY)
///
/// # Arguments
/// * `vps_host` - VPS hostname or IP address
/// * `user` - SSH username (typically "root")
/// * `password` - SSH password
/// * `port` - SSH port (typically 22)
///
/// # Returns
/// * `Ok(VerifyResult)` - Verification results for both tools
/// * `Err` - SSH connection or command execution failed
///
/// # Example
/// ```no_run
/// use spoq::setup::creds_verify::verify_credentials;
///
/// async fn example() {
///     match verify_credentials("vps.example.com", "root", "password", 22).await {
///         Ok(result) => {
///             if result.all_ok() {
///                 println!("All credentials verified!");
///             } else {
///                 if !result.github_ok {
///                     println!("GitHub CLI not authenticated");
///                 }
///                 if !result.claude_ok {
///                     println!("Claude Code not authenticated");
///                 }
///             }
///         }
///         Err(e) => eprintln!("Verification failed: {}", e),
///     }
/// }
/// ```
pub async fn verify_credentials(
    vps_host: &str,
    user: &str,
    password: &str,
    port: u16,
) -> Result<VerifyResult, VerifyError> {
    // Create SSH session
    let session = create_ssh_session(vps_host, user, password, port)?;

    // Test GitHub CLI
    let github_ok = verify_github_cli(&session)?;

    // Test Claude Code
    let claude_ok = verify_claude_code(&session)?;

    // Both must pass
    if !github_ok || !claude_ok {
        let mut errors = Vec::new();
        if !github_ok {
            errors.push("GitHub CLI not authenticated");
        }
        if !claude_ok {
            errors.push("Claude Code not authenticated");
        }
        return Err(VerifyError::VerificationFailed(errors.join(", ")));
    }

    Ok(VerifyResult {
        github_ok,
        claude_ok,
    })
}

/// Verify GitHub CLI authentication on the VPS.
///
/// Runs `gh auth status` and checks for "Logged in" in the output.
fn verify_github_cli(session: &Session) -> Result<bool, VerifyError> {
    let cmd = "gh auth status 2>&1";
    let (output, _exit_code) = run_ssh_command(session, cmd)?;

    // Check for authentication indicators
    let is_authenticated = output.contains("Logged in") || output.contains("✓");

    Ok(is_authenticated)
}

/// Verify Claude Code authentication on the VPS.
///
/// Runs `claude -p 'say OK'` with TTY emulation (using script command)
/// and checks for successful exit code.
fn verify_claude_code(session: &Session) -> Result<bool, VerifyError> {
    // Use script to fake TTY (Claude needs it) + timeout to prevent hanging
    // The script command provides a pseudo-TTY environment
    let cmd = "script -q /dev/null -c \"timeout 30 claude -p 'say OK'\" 2>&1";
    let (output, exit_code) = run_ssh_command(session, cmd)?;

    // Check for success:
    // - Exit code must be 0
    // - Output must not contain error indicators
    let is_authenticated = exit_code == 0
        && !output.to_lowercase().contains("invalid api key")
        && !output.to_lowercase().contains("not authenticated")
        && !output.to_lowercase().contains("unauthorized")
        && !output.to_lowercase().contains("/login")
        && !output.contains("[TIMEOUT or ERROR]");

    Ok(is_authenticated)
}

/// Create an SSH session to the VPS.
fn create_ssh_session(host: &str, user: &str, password: &str, port: u16) -> Result<Session, VerifyError> {
    let tcp = TcpStream::connect(format!("{}:{}", host, port))?;

    let mut session = Session::new()?;
    session.set_tcp_stream(tcp);
    session.handshake()?;
    session.userauth_password(user, password)?;

    if !session.authenticated() {
        return Err(VerifyError::AuthFailed);
    }

    Ok(session)
}

/// Run a command over SSH and return the output and exit code.
fn run_ssh_command(session: &Session, command: &str) -> Result<(String, i32), VerifyError> {
    let mut channel = session.channel_session()?;
    channel.exec(command)?;

    let mut stdout = String::new();
    channel.read_to_string(&mut stdout)?;

    // Also capture stderr
    let mut stderr = String::new();
    channel.stderr().read_to_string(&mut stderr).ok();

    // Combine stdout and stderr for analysis
    if !stderr.is_empty() {
        stdout.push_str(&stderr);
    }

    channel.wait_close().ok();
    let exit_status = channel.exit_status().unwrap_or(-1);

    Ok((stdout, exit_status))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_result_all_ok() {
        let result = VerifyResult {
            github_ok: true,
            claude_ok: true,
        };
        assert!(result.all_ok());
    }

    #[test]
    fn test_verify_result_github_fails() {
        let result = VerifyResult {
            github_ok: false,
            claude_ok: true,
        };
        assert!(!result.all_ok());
    }

    #[test]
    fn test_verify_result_claude_fails() {
        let result = VerifyResult {
            github_ok: true,
            claude_ok: false,
        };
        assert!(!result.all_ok());
    }

    #[test]
    fn test_verify_result_both_fail() {
        let result = VerifyResult {
            github_ok: false,
            claude_ok: false,
        };
        assert!(!result.all_ok());
    }

    #[tokio::test]
    async fn test_verify_credentials_invalid_host() {
        // Test with an invalid host to verify error handling
        let result = verify_credentials("127.0.0.1", "root", "invalid", 1).await;
        assert!(result.is_err());
    }
}
