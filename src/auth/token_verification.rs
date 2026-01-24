use std::process::{Command, Stdio};
use tracing::{debug, info, warn};

use super::token_migration::detect_tokens;

/// Result of local token verification (before provisioning)
#[derive(Debug, Clone)]
pub struct LocalTokenVerification {
    pub claude_code_present: bool,
    pub github_cli_present: bool,
    pub codex_present: bool,
    pub all_required_present: bool,
}

/// Result of VPS token verification (after migration)
#[derive(Debug, Clone)]
pub struct VpsTokenVerification {
    pub claude_code_works: bool,
    pub github_cli_works: bool,
    pub ssh_error: Option<String>,
}

/// Error types for token verification
#[derive(Debug, Clone)]
pub enum TokenVerificationError {
    DetectionFailed(String),
    SshConnectionFailed(String),
    SshCommandTimeout(String),
    VerificationScriptFailed(String),
    SshpassNotInstalled(String),
}

impl std::fmt::Display for TokenVerificationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenVerificationError::DetectionFailed(msg) => {
                write!(f, "Token detection failed: {}", msg)
            }
            TokenVerificationError::SshConnectionFailed(msg) => {
                write!(f, "SSH connection failed: {}", msg)
            }
            TokenVerificationError::SshCommandTimeout(msg) => {
                write!(f, "SSH command timed out: {}", msg)
            }
            TokenVerificationError::VerificationScriptFailed(msg) => {
                write!(f, "Verification script failed: {}", msg)
            }
            TokenVerificationError::SshpassNotInstalled(msg) => {
                write!(f, "sshpass not installed: {}", msg)
            }
        }
    }
}

impl std::error::Error for TokenVerificationError {}

/// Verify required tokens exist locally before provisioning
///
/// This function checks if Claude Code and GitHub CLI tokens are present
/// on the local machine. Both are required for provisioning to proceed.
///
/// # Returns
/// * `Ok(LocalTokenVerification)` - Token status with `all_required_present` flag
/// * `Err(TokenVerificationError)` - If token detection fails
pub fn verify_local_tokens() -> Result<LocalTokenVerification, TokenVerificationError> {
    info!("Verifying local tokens before provisioning");

    // Use existing detect_tokens() function
    let detection = detect_tokens().map_err(|e| {
        TokenVerificationError::DetectionFailed(format!("Failed to detect tokens: {}", e))
    })?;

    // Check if all required tokens are present
    let all_required_present = detection.claude_code && detection.github_cli;

    let verification = LocalTokenVerification {
        claude_code_present: detection.claude_code,
        github_cli_present: detection.github_cli,
        codex_present: detection.codex,
        all_required_present,
    };

    if !verification.all_required_present {
        warn!(
            "Missing required tokens - Claude Code: {}, GitHub CLI: {}",
            verification.claude_code_present, verification.github_cli_present
        );
    } else {
        info!("All required tokens present locally");
    }

    Ok(verification)
}

/// SSH to VPS and verify tokens work by running commands
///
/// This function connects to the VPS via SSH and tests whether
/// Claude Code and GitHub CLI are installed and authenticated.
///
/// # Arguments
/// * `vps_ip` - IP address of the VPS
/// * `ssh_username` - SSH username for the VPS
/// * `ssh_password` - SSH password for authentication
///
/// # Returns
/// * `Ok(VpsTokenVerification)` - Status of each token on VPS
/// * `Err(TokenVerificationError)` - If SSH connection or verification fails
pub fn verify_vps_tokens(
    vps_ip: &str,
    ssh_username: &str,
    ssh_password: &str,
) -> Result<VpsTokenVerification, TokenVerificationError> {
    info!(
        "Verifying tokens on VPS {} as user {}",
        vps_ip, ssh_username
    );

    // Check sshpass is available
    if !check_sshpass_available() {
        return Err(TokenVerificationError::SshpassNotInstalled(
            "sshpass is required for SSH verification. Install with: brew install hudochenkov/sshpass/sshpass (macOS) or apt install sshpass (Linux)".to_string(),
        ));
    }

    // Verify Claude Code
    let claude_code_works = match run_ssh_command(
        vps_ip,
        ssh_username,
        ssh_password,
        "claude -p \"testing verification\"",
    ) {
        Ok(output) => {
            debug!("Claude Code verification output: {}", output);
            info!("Claude Code verified on VPS");
            true
        }
        Err(e) => {
            warn!("Claude Code verification failed: {}", e);
            false
        }
    };

    // Verify GitHub CLI
    let github_cli_works =
        match run_ssh_command(vps_ip, ssh_username, ssh_password, "gh auth status") {
            Ok(output) => {
                debug!("GitHub CLI verification output: {}", output);
                // Check for success indicators in output
                let success = output.contains("Logged in") || output.contains("✓");
                if success {
                    info!("GitHub CLI verified on VPS");
                } else {
                    warn!("GitHub CLI not authenticated on VPS");
                }
                success
            }
            Err(e) => {
                warn!("GitHub CLI verification failed: {}", e);
                false
            }
        };

    Ok(VpsTokenVerification {
        claude_code_works,
        github_cli_works,
        ssh_error: None,
    })
}

/// Run an SSH command on the VPS
///
/// # Arguments
/// * `vps_ip` - IP address of the VPS
/// * `ssh_username` - SSH username
/// * `ssh_password` - SSH password
/// * `command` - Command to execute on VPS
///
/// # Returns
/// * `Ok(String)` - Command output (stdout)
/// * `Err(TokenVerificationError)` - If SSH fails or command fails
fn run_ssh_command(
    vps_ip: &str,
    ssh_username: &str,
    ssh_password: &str,
    command: &str,
) -> Result<String, TokenVerificationError> {
    debug!(
        "Running SSH command: {} on {}@{}",
        command, ssh_username, vps_ip
    );

    let remote_host = format!("{}@{}", ssh_username, vps_ip);
    let escaped_password = ssh_password.replace("'", "'\\''"); // Escape single quotes

    let output = Command::new("sshpass")
        .arg("-p")
        .arg(&escaped_password)
        .arg("ssh")
        .arg("-o")
        .arg("StrictHostKeyChecking=no")
        .arg("-o")
        .arg("UserKnownHostsFile=/dev/null")
        .arg("-o")
        .arg("ConnectTimeout=30")
        .arg(&remote_host)
        .arg(command)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| {
            TokenVerificationError::SshConnectionFailed(format!(
                "Failed to execute SSH command: {}",
                e
            ))
        })?;

    // Check exit status
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(stdout)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        // Parse specific error types
        if stderr.to_lowercase().contains("connection refused") {
            Err(TokenVerificationError::SshConnectionFailed(
                "Connection refused. VPS may not be ready or SSH port is blocked.".to_string(),
            ))
        } else if stderr.to_lowercase().contains("permission denied") {
            Err(TokenVerificationError::SshConnectionFailed(
                "Authentication failed. Check SSH username and password.".to_string(),
            ))
        } else if stderr.to_lowercase().contains("timed out")
            || stderr.to_lowercase().contains("timeout")
        {
            Err(TokenVerificationError::SshCommandTimeout(
                "SSH connection timed out. VPS may be slow to respond.".to_string(),
            ))
        } else {
            Err(TokenVerificationError::VerificationScriptFailed(stderr))
        }
    }
}

/// Check if sshpass is available on the system
fn check_sshpass_available() -> bool {
    Command::new("which")
        .arg("sshpass")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Display error message for missing local tokens
///
/// Shows which required tokens are missing and provides
/// instructions for how to authenticate.
pub fn display_missing_tokens_error(verification: &LocalTokenVerification) {
    println!("\n⚠️  Required tokens missing:");

    if !verification.claude_code_present {
        println!("  ✗ Claude Code - not found");
    }
    if !verification.github_cli_present {
        println!("  ✗ GitHub CLI - not found");
    }

    println!("\nTo continue, please login:");

    if !verification.claude_code_present {
        println!("  1. Claude Code: Run 'claude', then type /login");
    }
    if !verification.github_cli_present {
        println!("  2. GitHub CLI: Run 'gh auth login'");
    }

    println!("\nAfter logging in, run this command again to provision your VPS.");
}

/// Display results of VPS token verification
///
/// Shows which tokens are working on the VPS and provides
/// troubleshooting steps for any failures.
pub fn display_vps_verification_results(verification: &VpsTokenVerification) {
    if verification.claude_code_works && verification.github_cli_works {
        // All tokens verified successfully
        println!("\n✓ Claude Code verified on VPS");
        println!("✓ GitHub CLI verified on VPS");
        println!("\nYour VPS is ready with working credentials!");
    } else {
        // Some tokens failed verification
        println!("\n⚠️  Warning: Could not verify all tokens on VPS\n");

        if verification.claude_code_works {
            println!("  ✓ Claude Code - verified successfully");
        } else {
            println!("  ✗ Claude Code - verification failed");
        }

        if verification.github_cli_works {
            println!("  ✓ GitHub CLI - verified successfully");
        } else {
            println!("  ✗ GitHub CLI - verification failed");
        }

        println!("\nYour VPS is ready, but you may need to manually login:");

        if !verification.claude_code_works {
            println!("  1. SSH to VPS: ssh spoq@[VPS_IP]");
            println!("  2. Run: claude, then type /login");
            println!("  3. Verify: claude -p \"test\"");
        }

        if !verification.github_cli_works {
            println!("  1. SSH to VPS: ssh spoq@[VPS_IP]");
            println!("  2. Run: gh auth login");
            println!("  3. Verify: gh auth status");
        }
    }
}
