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
    /// Email address of the authenticated account (if available)
    pub email: Option<String>,
    /// Error message if failed
    pub error: Option<String>,
}

impl ClaudeAuthResult {
    pub fn success(token: String) -> Self {
        // Try to read email from ~/.claude.json after successful auth
        let email = read_claude_email();
        Self {
            success: true,
            token: Some(token),
            email,
            error: None,
        }
    }

    pub fn failed(error: impl Into<String>) -> Self {
        Self {
            success: false,
            token: None,
            email: None,
            error: Some(error.into()),
        }
    }
}

/// Read the authenticated email from ~/.claude.json
///
/// After `claude setup-token` completes, the local Claude config is updated
/// with the OAuth account info including `oauthAccount.emailAddress`.
fn read_claude_email() -> Option<String> {
    let home = dirs::home_dir()?;
    let config_path = home.join(".claude.json");
    let contents = std::fs::read_to_string(&config_path).ok()?;
    let config: serde_json::Value = serde_json::from_str(&contents).ok()?;
    config
        .get("oauthAccount")
        .and_then(|acct| acct.get("emailAddress"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Fallback: check if ~/.claude.json was recently modified and contains a token.
///
/// This is useful when `setup-token` times out but the user may have actually
/// completed browser authentication — the CLI writes `~/.claude.json` with the
/// OAuth token on success, even if we failed to capture stdout.
///
/// Returns `Some(ClaudeAuthResult)` if a fresh token was written (modified
/// within the last 5 minutes), `None` otherwise.
fn check_claude_json_for_token() -> Option<ClaudeAuthResult> {
    let home = dirs::home_dir()?;
    let config_path = home.join(".claude.json");
    let metadata = std::fs::metadata(&config_path).ok()?;
    let modified = metadata.modified().ok()?;
    let age = std::time::SystemTime::now().duration_since(modified).ok()?;

    // Only trust the file if it was modified within the last 5 minutes
    if age > Duration::from_secs(300) {
        tracing::debug!("~/.claude.json too old ({:?} ago), skipping fallback", age);
        return None;
    }

    let contents = std::fs::read_to_string(&config_path).ok()?;
    let config: serde_json::Value = serde_json::from_str(&contents).ok()?;

    // Look for oauthAccount.accessToken or similar token fields
    let email = config
        .get("oauthAccount")
        .and_then(|acct| acct.get("emailAddress"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // If we found an email, the auth likely succeeded — but we don't have the
    // raw token from the file (Claude CLI doesn't store it in plain text).
    // We report this as a special result so the caller can inform the user.
    if email.is_some() {
        tracing::info!(
            "Fallback: found recent ~/.claude.json with email {:?}",
            email
        );
        Some(ClaudeAuthResult {
            success: true,
            token: None, // Token not extractable from config
            email,
            error: None,
        })
    } else {
        None
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

/// Events emitted during setup-token execution
#[derive(Debug, Clone)]
pub enum ClaudeSetupEvent {
    /// An OAuth URL was found in the output
    UrlFound(String),
}

/// Run `claude setup-token` and capture the OAuth token
///
/// This function:
/// 1. Spawns `claude setup-token` in a PTY
/// 2. Opens the browser for authentication
/// 3. Captures the OAuth token from stdout
/// 4. Returns the token for sending to Conductor
///
/// If `event_tx` is provided, intermediate events (like OAuth URLs) are sent
/// back in real-time while the function continues waiting for the token.
pub fn run_claude_setup_token() -> Result<ClaudeAuthResult, ClaudeAuthError> {
    run_claude_setup_token_with_events(None)
}

/// Run `claude setup-token` with real-time event channel for URL surfacing
pub fn run_claude_setup_token_with_events(
    event_tx: Option<mpsc::Sender<ClaudeSetupEvent>>,
) -> Result<ClaudeAuthResult, ClaudeAuthError> {
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
    // Regex to match OAuth URLs in output
    let url_regex = Regex::new(r#"(https://[^\s"<>\x1b]+)"#).unwrap();
    let mut url_sent = false;

    let mut captured_token: Option<String> = None;
    let mut full_output = String::new();

    // Main loop to read output and capture token
    loop {
        // Check timeout
        if start_time.elapsed() > timeout {
            let _ = child.kill();
            // Fallback: check if ~/.claude.json was updated (auth may have
            // succeeded even though we couldn't capture the token from stdout)
            if let Some(result) = check_claude_json_for_token() {
                tracing::info!("Timeout but fallback found recent auth in ~/.claude.json");
                return Ok(result);
            }
            return Err(ClaudeAuthError::Timeout);
        }

        // Try to receive output
        match rx.recv_timeout(output_timeout) {
            Ok(line) => {
                // Strip ANSI escape codes for pattern matching
                let clean_line = ansi_regex.replace_all(&line, "").to_string();
                full_output.push_str(&clean_line);

                // Check for OAuth URL in output (send back immediately)
                if !url_sent {
                    if let Some(caps) = url_regex.captures(&clean_line) {
                        let url = caps[1].to_string();
                        tracing::info!("Found OAuth URL in setup-token output: {}", url);
                        if let Some(ref tx) = event_tx {
                            let _ = tx.send(ClaudeSetupEvent::UrlFound(url));
                        }
                        url_sent = true;
                    }
                }

                // Check for token in output
                if let Some(caps) = token_regex.captures(&clean_line) {
                    captured_token = Some(caps[1].to_string());
                    tracing::info!("Captured Claude CLI token: token_length={}", caps[1].len());
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
        validate_token(&token)?;
        Ok(ClaudeAuthResult::success(token))
    } else {
        // Try one more time to find token in full output
        if let Some(caps) = token_regex.captures(&full_output) {
            let token = caps[1].to_string();
            validate_token(&token)?;
            Ok(ClaudeAuthResult::success(token))
        } else {
            Err(ClaudeAuthError::TokenNotFound)
        }
    }
}

/// Validate a captured token for security and correctness
fn validate_token(token: &str) -> Result<(), ClaudeAuthError> {
    const MIN_TOKEN_LENGTH: usize = 20;
    const MAX_TOKEN_LENGTH: usize = 500;

    if token.is_empty() {
        return Err(ClaudeAuthError::TokenNotFound);
    }

    if token.len() < MIN_TOKEN_LENGTH {
        return Err(ClaudeAuthError::ProcessError(
            format!("Token too short: {} chars (min {})", token.len(), MIN_TOKEN_LENGTH)
        ));
    }

    if token.len() > MAX_TOKEN_LENGTH {
        return Err(ClaudeAuthError::ProcessError(
            format!("Token too long: {} chars (max {})", token.len(), MAX_TOKEN_LENGTH)
        ));
    }

    if token.chars().any(|c| c.is_whitespace()) {
        return Err(ClaudeAuthError::ProcessError(
            "Token contains invalid whitespace".to_string()
        ));
    }

    Ok(())
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

    #[test]
    fn test_validate_token_empty() {
        let result = validate_token("");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ClaudeAuthError::TokenNotFound));
    }

    #[test]
    fn test_validate_token_too_short() {
        let token = "sk-ant-oat01-abc"; // Only 16 chars
        let result = validate_token(token);
        assert!(result.is_err());
        match result.unwrap_err() {
            ClaudeAuthError::ProcessError(msg) => {
                assert!(msg.contains("too short"));
                assert!(msg.contains("16 chars"));
            }
            _ => panic!("Expected ProcessError for too short token"),
        }
    }

    #[test]
    fn test_validate_token_too_long() {
        // Create a token that's 501 chars (> MAX_TOKEN_LENGTH of 500)
        let token = format!("sk-ant-oat01-{}", "a".repeat(488)); // 501 total
        let result = validate_token(&token);
        assert!(result.is_err());
        match result.unwrap_err() {
            ClaudeAuthError::ProcessError(msg) => {
                assert!(msg.contains("too long"));
                assert!(msg.contains("501 chars"));
            }
            _ => panic!("Expected ProcessError for too long token"),
        }
    }

    #[test]
    fn test_validate_token_with_whitespace() {
        let tokens_with_whitespace = vec![
            "sk-ant-oat01-abc def",      // space
            "sk-ant-oat01-abc\ndef",     // newline
            "sk-ant-oat01-abc\tdef",     // tab
            "sk-ant-oat01-abc\rdef",     // carriage return
        ];

        for token in tokens_with_whitespace {
            let result = validate_token(token);
            assert!(result.is_err(), "Token with whitespace should fail: {:?}", token);
            match result.unwrap_err() {
                ClaudeAuthError::ProcessError(msg) => {
                    assert!(msg.contains("whitespace"), "Error message should mention whitespace");
                }
                _ => panic!("Expected ProcessError for token with whitespace"),
            }
        }
    }

    #[test]
    fn test_validate_token_valid() {
        // Valid tokens of various lengths
        let valid_tokens = vec![
            "sk-ant-oat01-abcdefghij".to_string(),                           // 23 chars - minimum valid
            "sk-ant-oat01-abcdef_ghij-klmno".to_string(),                    // With underscores and hyphens
            "sk-ant-oat01-ABCDEF123456".to_string(),                         // With uppercase
            format!("sk-ant-oat01-{}", "x".repeat(200)),                     // Long but valid (214 chars)
            format!("sk-ant-oat01-{}", "a".repeat(486)),                     // Max valid length (500 chars)
        ];

        for token in valid_tokens {
            let result = validate_token(&token);
            assert!(result.is_ok(), "Valid token should pass: {}", token);
        }
    }
}
