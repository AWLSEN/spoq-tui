//! Claude CLI authentication module.
//!
//! This module handles automated Claude CLI setup-token authentication
//! for VPS environments where browser-based login is not available.
//!
//! ## Flow
//!
//! 1. Conductor requests a token via WebSocket (ClaudeAuthTokenRequest)
//! 2. TUI spawns `claude setup-token` in a PTY
//! 3. TUI captures the OAuth token from stdout
//! 4. TUI sends the token back via WebSocket (ClaudeAuthToken)
//! 5. Conductor stores the token encrypted and confirms (ClaudeAuthTokenStored)

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use regex::Regex;
use std::io::{BufRead, BufReader};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use thiserror::Error;

/// Timeout for the entire setup-token flow (3 minutes)
const SETUP_TOKEN_TIMEOUT_SECS: u64 = 180;

/// Timeout for waiting for output (30 seconds)
const OUTPUT_TIMEOUT_SECS: u64 = 30;

#[derive(Debug, Error)]
pub enum ClaudeAuthError {
    #[error("Claude CLI not installed")]
    NotInstalled,

    #[error("PTY error: {0}")]
    PtyError(String),

    #[error("Timeout waiting for token")]
    Timeout,

    #[error("Failed to capture token from output")]
    TokenNotFound,

    #[error("User cancelled")]
    Cancelled,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Process exited with error: {0}")]
    ProcessError(String),
}

/// Result of Claude CLI setup-token attempt
#[derive(Debug, Clone)]
pub struct ClaudeAuthResult {
    /// Whether authentication succeeded
    pub success: bool,
    /// The captured OAuth token (sk-ant-oat01-...)
    pub token: Option<String>,
    /// Error message if failed
    pub error: Option<String>,
}

impl ClaudeAuthResult {
    pub fn success(token: String) -> Self {
        Self {
            success: true,
            token: Some(token),
            error: None,
        }
    }

    pub fn failed(error: impl Into<String>) -> Self {
        Self {
            success: false,
            token: None,
            error: Some(error.into()),
        }
    }
}

/// Check if Claude CLI is installed
pub fn is_claude_installed() -> bool {
    std::process::Command::new("claude")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Run `claude setup-token` and capture the OAuth token
///
/// This function:
/// 1. Spawns `claude setup-token` in a PTY
/// 2. Opens the browser for authentication
/// 3. Captures the OAuth token from stdout
/// 4. Returns the token for sending to Conductor
pub fn run_claude_setup_token() -> Result<ClaudeAuthResult, ClaudeAuthError> {
    if !is_claude_installed() {
        return Err(ClaudeAuthError::NotInstalled);
    }

    let pty_system = native_pty_system();

    // Create PTY with reasonable size
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| ClaudeAuthError::PtyError(e.to_string()))?;

    // Build the command
    let mut cmd = CommandBuilder::new("claude");
    cmd.args(["setup-token"]);

    // Spawn the child process
    let mut child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| ClaudeAuthError::PtyError(e.to_string()))?;

    // Get reader
    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| ClaudeAuthError::PtyError(e.to_string()))?;

    // Channel for output processing
    let (tx, rx) = mpsc::channel::<String>();

    // Spawn a thread to read output
    let _reader_handle = std::thread::spawn(move || {
        let mut buf_reader = BufReader::new(reader);
        let mut line = String::new();
        let mut full_output = String::new();

        loop {
            line.clear();
            match buf_reader.read_line(&mut line) {
                Ok(0) => break, // EOF
                Ok(_) => {
                    full_output.push_str(&line);
                    let _ = tx.send(line.clone());
                }
                Err(_) => break,
            }
        }
        full_output
    });

    let start_time = Instant::now();
    let timeout = Duration::from_secs(SETUP_TOKEN_TIMEOUT_SECS);
    let output_timeout = Duration::from_secs(OUTPUT_TIMEOUT_SECS);

    // Regex to match OAuth token - format: sk-ant-oat01-...
    // The token is typically on its own line or after "Token: " or similar
    let token_regex = Regex::new(r"(sk-ant-oat[a-zA-Z0-9_-]+)").unwrap();
    // Regex to strip ANSI escape sequences
    let ansi_regex = Regex::new(r"\x1b\[[0-9;]*[a-zA-Z]").unwrap();

    let mut captured_token: Option<String> = None;
    let mut full_output = String::new();

    // Main loop to read output and capture token
    loop {
        // Check timeout
        if start_time.elapsed() > timeout {
            let _ = child.kill();
            return Err(ClaudeAuthError::Timeout);
        }

        // Try to receive output
        match rx.recv_timeout(output_timeout) {
            Ok(line) => {
                // Strip ANSI escape codes for pattern matching
                let clean_line = ansi_regex.replace_all(&line, "").to_string();
                full_output.push_str(&clean_line);

                // Check for token in output
                if let Some(caps) = token_regex.captures(&clean_line) {
                    captured_token = Some(caps[1].to_string());
                    tracing::info!("Captured Claude CLI token: {}...", &caps[1][..std::cmp::min(20, caps[1].len())]);
                }

                // Check for errors in output
                if clean_line.to_lowercase().contains("error")
                    && !clean_line.contains("no error")
                    && captured_token.is_none()
                {
                    tracing::warn!("Claude setup-token error output: {}", clean_line.trim());
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Check if child has exited
                if let Ok(Some(status)) = child.try_wait() {
                    if status.exit_code() == 0 {
                        // Process completed successfully
                        break;
                    } else {
                        return Err(ClaudeAuthError::ProcessError(format!(
                            "setup-token exited with code {}",
                            status.exit_code()
                        )));
                    }
                }
                // Continue waiting if process is still running
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                // Reader thread finished
                break;
            }
        }
    }

    // Wait for child to finish
    let status = child
        .wait()
        .map_err(|e| ClaudeAuthError::PtyError(e.to_string()))?;

    if status.exit_code() != 0 {
        return Err(ClaudeAuthError::ProcessError(format!(
            "setup-token exited with code {}",
            status.exit_code()
        )));
    }

    // Return result
    if let Some(token) = captured_token {
        Ok(ClaudeAuthResult::success(token))
    } else {
        // Try one more time to find token in full output
        if let Some(caps) = token_regex.captures(&full_output) {
            Ok(ClaudeAuthResult::success(caps[1].to_string()))
        } else {
            Err(ClaudeAuthError::TokenNotFound)
        }
    }
}

/// Run setup-token in a background thread and return a channel receiver
/// for the result. This is useful for non-blocking operation.
pub fn run_claude_setup_token_async() -> mpsc::Receiver<Result<ClaudeAuthResult, ClaudeAuthError>> {
    let (tx, rx) = mpsc::channel();

    std::thread::spawn(move || {
        let result = run_claude_setup_token();
        let _ = tx.send(result);
    });

    rx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_regex() {
        let regex = Regex::new(r"(sk-ant-oat[a-zA-Z0-9_-]+)").unwrap();

        let test_cases = vec![
            ("sk-ant-oat01-abc123", Some("sk-ant-oat01-abc123")),
            ("Token: sk-ant-oat01-xyz789", Some("sk-ant-oat01-xyz789")),
            ("Your token is: sk-ant-oat01-test_token-123", Some("sk-ant-oat01-test_token-123")),
            ("no token here", None),
            ("sk-wrong-format", None),
        ];

        for (input, expected) in test_cases {
            let result = regex.captures(input).map(|c| c[1].to_string());
            assert_eq!(result.as_deref(), expected, "Input: {}", input);
        }
    }

    #[test]
    fn test_claude_auth_result_success() {
        let result = ClaudeAuthResult::success("sk-ant-oat01-test".to_string());
        assert!(result.success);
        assert_eq!(result.token, Some("sk-ant-oat01-test".to_string()));
        assert!(result.error.is_none());
    }

    #[test]
    fn test_claude_auth_result_failed() {
        let result = ClaudeAuthResult::failed("test error");
        assert!(!result.success);
        assert!(result.token.is_none());
        assert_eq!(result.error, Some("test error".to_string()));
    }
}
