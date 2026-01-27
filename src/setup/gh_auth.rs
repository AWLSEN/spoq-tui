//! GitHub CLI authentication module.
//!
//! This module handles automated GitHub CLI installation and authentication
//! during the SPOQ setup flow. It uses PTY to interact with the `gh auth login`
//! process and capture the one-time device code.
//!
//! ## Flow
//!
//! 1. Check if `gh` CLI is installed
//! 2. Auto-install via Homebrew if not (macOS)
//! 3. Check if already authenticated
//! 4. Run `gh auth login` with PTY interaction
//! 5. Capture and display the one-time code
//! 6. Auto-open browser for authentication
//! 7. Wait for completion
//! 8. Fallback to manual instructions if auto fails

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use regex::Regex;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use thiserror::Error;

/// GitHub CLI install URL
const GH_INSTALL_URL: &str = "https://cli.github.com";

/// Timeout for the entire auth flow (3 minutes)
const AUTH_TIMEOUT_SECS: u64 = 180;

/// Timeout for waiting for prompts (10 seconds)
const PROMPT_TIMEOUT_SECS: u64 = 10;

#[derive(Debug, Error)]
pub enum GhAuthError {
    #[error("GitHub CLI not installed")]
    NotInstalled,

    #[error("Auto-install failed: {0}")]
    InstallFailed(String),

    #[error("Already authenticated")]
    AlreadyAuthenticated,

    #[error("PTY error: {0}")]
    PtyError(String),

    #[error("Timeout waiting for auth completion")]
    Timeout,

    #[error("Failed to parse device code from output")]
    CodeNotFound,

    #[error("User cancelled")]
    Cancelled,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result of GitHub CLI authentication attempt
#[derive(Debug, Clone)]
pub struct GhAuthResult {
    /// Whether authentication succeeded
    pub success: bool,
    /// The one-time device code (e.g., "9E41-5360")
    pub device_code: Option<String>,
    /// The verification URL
    pub verification_url: Option<String>,
    /// Error message if failed
    pub error: Option<String>,
}

impl GhAuthResult {
    fn success() -> Self {
        Self {
            success: true,
            device_code: None,
            verification_url: None,
            error: None,
        }
    }

    fn with_code(device_code: String, verification_url: String) -> Self {
        Self {
            success: false, // Not yet complete, waiting for user
            device_code: Some(device_code),
            verification_url: Some(verification_url),
            error: None,
        }
    }

    fn failed(error: impl Into<String>) -> Self {
        Self {
            success: false,
            device_code: None,
            verification_url: None,
            error: Some(error.into()),
        }
    }
}

/// Check if GitHub CLI is installed
pub fn is_gh_installed() -> bool {
    Command::new("gh")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Check if GitHub CLI is authenticated
pub fn is_gh_authenticated() -> bool {
    Command::new("gh")
        .args(["auth", "status"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Attempt to auto-install GitHub CLI via Homebrew (macOS only)
pub fn auto_install_gh() -> Result<(), GhAuthError> {
    println!("  Attempting to install GitHub CLI...");

    // Check if brew is available
    let brew_available = Command::new("brew")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if !brew_available {
        return Err(GhAuthError::InstallFailed(
            "Homebrew not found. Please install GitHub CLI manually.".to_string(),
        ));
    }

    // Run brew install gh
    let output = Command::new("brew")
        .args(["install", "gh"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    if output.status.success() {
        println!("  GitHub CLI installed successfully!");
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(GhAuthError::InstallFailed(stderr.to_string()))
    }
}

/// Run the automated GitHub auth flow using PTY
///
/// This function:
/// 1. Spawns `gh auth login` in a PTY
/// 2. Sends Enter to accept defaults (GitHub.com, HTTPS, Web browser)
/// 3. Captures the one-time device code
/// 4. Displays it to the user
/// 5. Sends Enter to open the browser
/// 6. Waits for authentication to complete
pub fn run_gh_auth_pty() -> Result<GhAuthResult, GhAuthError> {
    let pty_system = native_pty_system();

    // Create PTY with reasonable size
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| GhAuthError::PtyError(e.to_string()))?;

    // Build the command
    let mut cmd = CommandBuilder::new("gh");
    cmd.args(["auth", "login"]);

    // Spawn the child process
    let mut child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| GhAuthError::PtyError(e.to_string()))?;

    // Get reader and writer
    let reader = pair.master.try_clone_reader()
        .map_err(|e| GhAuthError::PtyError(e.to_string()))?;
    let mut writer = pair.master.take_writer()
        .map_err(|e| GhAuthError::PtyError(e.to_string()))?;

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
    let timeout = Duration::from_secs(AUTH_TIMEOUT_SECS);
    let prompt_timeout = Duration::from_secs(PROMPT_TIMEOUT_SECS);

    let mut prompts_answered = 0;
    let mut device_code: Option<String> = None;
    let mut verification_url: Option<String> = None;
    let mut code_displayed = false;

    // Regex patterns - code pattern handles ANSI codes between characters
    let code_regex = Regex::new(r"one-time code:.*?([A-Z0-9]{4}-[A-Z0-9]{4})").unwrap();
    let url_regex = Regex::new(r"(https://github\.com/login/device)").unwrap();
    // Regex to strip ANSI escape sequences
    let ansi_regex = Regex::new(r"\x1b\[[0-9;]*[a-zA-Z]").unwrap();

    // Main loop to interact with the process
    loop {
        // Check timeout
        if start_time.elapsed() > timeout {
            let _ = child.kill();
            return Err(GhAuthError::Timeout);
        }

        // Try to receive output
        match rx.recv_timeout(prompt_timeout) {
            Ok(line) => {
                // Strip ANSI escape codes for pattern matching
                let clean_line = ansi_regex.replace_all(&line, "").to_string();

                // Check for prompts that need Enter
                let needs_enter = clean_line.contains("Where do you use GitHub")
                    || clean_line.contains("What is your preferred protocol")
                    || clean_line.contains("How would you like to authenticate")
                    || clean_line.contains("Authenticate Git with your GitHub credentials");

                if needs_enter && prompts_answered < 4 {
                    // Small delay before sending Enter
                    std::thread::sleep(Duration::from_millis(100));
                    let _ = writer.write_all(b"\n");
                    let _ = writer.flush();
                    prompts_answered += 1;
                }

                // Check for device code
                if let Some(caps) = code_regex.captures(&clean_line) {
                    device_code = Some(caps[1].to_string());
                }

                // Check for URL
                if let Some(caps) = url_regex.captures(&clean_line) {
                    verification_url = Some(caps[1].to_string());
                }

                // Check if we should display the code and press Enter to open browser
                if clean_line.contains("Press Enter to open") && !code_displayed {
                    if let (Some(ref code), Some(ref url)) = (&device_code, &verification_url) {
                        // Display to user
                        println!();
                        println!("  GitHub Authentication");
                        println!("  Code: {}", code);
                        println!("  URL:  {}", url);
                        println!("  Opening browser...");
                        println!();

                        code_displayed = true;

                        // Send Enter to open browser
                        std::thread::sleep(Duration::from_millis(500));
                        let _ = writer.write_all(b"\n");
                        let _ = writer.flush();
                    }
                }

                // If we have the code but haven't displayed yet, display it now
                // (in case "Press Enter" comes on a different line or we missed it)
                if device_code.is_some() && !code_displayed {
                    if let Some(ref code) = device_code {
                        let url = verification_url
                            .clone()
                            .unwrap_or_else(|| "https://github.com/login/device".to_string());

                        // Display to user
                        println!();
                        println!("  GitHub Authentication");
                        println!("  Code: {}", code);
                        println!("  URL:  {}", url);
                        println!("  Opening browser...");
                        println!();

                        code_displayed = true;

                        // Send Enter to open browser
                        std::thread::sleep(Duration::from_millis(500));
                        let _ = writer.write_all(b"\n");
                        let _ = writer.flush();
                    }
                }

                // Check for success
                if clean_line.contains("Logged in as") || clean_line.contains("Authentication complete") {
                    let _ = child.wait();
                    return Ok(GhAuthResult::success());
                }

                // Check for errors
                if clean_line.contains("error") || clean_line.contains("failed") {
                    if !clean_line.contains("Press Enter") {
                        let _ = child.kill();
                        return Ok(GhAuthResult::failed(clean_line.trim()));
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Check if child has exited
                if let Ok(Some(status)) = child.try_wait() {
                    if status.exit_code() == 0 {
                        return Ok(GhAuthResult::success());
                    } else {
                        return Ok(GhAuthResult::failed("gh auth login exited with error"));
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                // Reader thread finished
                break;
            }
        }
    }

    // Wait for child and check result
    let status = child.wait().map_err(|e| GhAuthError::PtyError(e.to_string()))?;

    if status.exit_code() == 0 {
        Ok(GhAuthResult::success())
    } else if let Some(code) = device_code {
        // We got the code but user didn't complete - might be waiting
        Ok(GhAuthResult::with_code(
            code,
            verification_url.unwrap_or_else(|| "https://github.com/login/device".to_string()),
        ))
    } else {
        Ok(GhAuthResult::failed("Authentication did not complete"))
    }
}

/// Display manual authentication instructions
pub fn display_manual_instructions() {
    println!();
    println!("  Manual GitHub CLI Setup Required");
    println!();
    println!("  1. Install GitHub CLI (if not installed):");
    println!("     {}", GH_INSTALL_URL);
    println!();
    println!("     macOS:   brew install gh");
    println!("     Linux:   See website for package manager");
    println!("     Windows: winget install GitHub.cli");
    println!();
    println!("  2. Authenticate (in a new terminal tab/window):");
    println!("     gh auth login");
    println!();
    println!("  3. Follow the prompts:");
    println!("     - Select GitHub.com");
    println!("     - Select HTTPS");
    println!("     - Select 'Login with a web browser'");
    println!("     - Copy the code and complete in browser");
    println!();
    println!("  4. Once done, press (r) here to retry");
    println!("     or (q) to quit");
    println!();
}

/// Wait for user to press 'r' to retry or 'q' to quit
///
/// Returns true if user pressed 'r' (retry), false if 'q' (quit)
pub fn wait_for_retry_input() -> bool {
    use std::io::Read;

    print!("  Press (r) to retry, (q) to quit: ");
    let _ = std::io::stdout().flush();

    // Read single character
    let mut buf = [0u8; 1];
    loop {
        if std::io::stdin().read(&mut buf).is_ok() {
            match buf[0] {
                b'r' | b'R' => {
                    println!();
                    return true;
                }
                b'q' | b'Q' => {
                    println!();
                    return false;
                }
                b'\n' | b'\r' => {
                    // Ignore Enter, wait for actual input
                    print!("  Press (r) to retry, (q) to quit: ");
                    let _ = std::io::stdout().flush();
                }
                _ => {
                    // Invalid input, prompt again
                    print!("\r  Press (r) to retry, (q) to quit: ");
                    let _ = std::io::stdout().flush();
                }
            }
        }
    }
}

/// Main entry point: Ensure GitHub CLI is installed and authenticated
///
/// This function orchestrates the entire GitHub CLI setup:
/// 1. Check if installed, auto-install if needed
/// 2. Check if authenticated
/// 3. Run automated auth flow
/// 4. Fallback to manual instructions with retry loop
///
/// # Returns
/// * `Ok(())` - GitHub CLI is installed and authenticated
/// * `Err(GhAuthError)` - User cancelled or fatal error
pub fn ensure_gh_authenticated() -> Result<(), GhAuthError> {
    println!("  Checking GitHub CLI...");

    // Step 1: Check if gh is installed
    if !is_gh_installed() {
        println!("  GitHub CLI not found.");

        // Try auto-install on macOS
        #[cfg(target_os = "macos")]
        {
            match auto_install_gh() {
                Ok(()) => {
                    // Verify installation
                    if !is_gh_installed() {
                        println!("  Auto-install completed but gh not found in PATH.");
                        display_manual_instructions();
                        return wait_for_retry_loop();
                    }
                }
                Err(e) => {
                    println!("  Auto-install failed: {}", e);
                    display_manual_instructions();
                    return wait_for_retry_loop();
                }
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            display_manual_instructions();
            return wait_for_retry_loop();
        }
    }

    println!("  GitHub CLI found.");

    // Step 2: Check if already authenticated
    if is_gh_authenticated() {
        println!("  GitHub CLI already authenticated.");
        return Ok(());
    }

    println!("  GitHub CLI not authenticated. Starting auth flow...");

    // Step 3: Try automated PTY flow
    match run_gh_auth_pty() {
        Ok(result) if result.success => {
            println!("  GitHub CLI authenticated successfully!");
            return Ok(());
        }
        Ok(result) => {
            if let Some(err) = result.error {
                println!("  Auth flow issue: {}", err);
            }
        }
        Err(e) => {
            println!("  Automated auth failed: {}", e);
        }
    }

    // Step 4: Verify authentication (user may have completed in browser)
    std::thread::sleep(Duration::from_secs(2));
    if is_gh_authenticated() {
        println!("  GitHub CLI authenticated successfully!");
        return Ok(());
    }

    // Step 5: Fall back to manual instructions
    display_manual_instructions();
    wait_for_retry_loop()
}

/// Retry loop for manual authentication
fn wait_for_retry_loop() -> Result<(), GhAuthError> {
    loop {
        if !wait_for_retry_input() {
            return Err(GhAuthError::Cancelled);
        }

        println!("  Checking GitHub CLI authentication...");

        if !is_gh_installed() {
            println!("  GitHub CLI still not installed.");
            display_manual_instructions();
            continue;
        }

        if is_gh_authenticated() {
            println!("  GitHub CLI authenticated successfully!");
            return Ok(());
        }

        println!("  GitHub CLI still not authenticated.");
        display_manual_instructions();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_regex() {
        let regex = Regex::new(r"one-time code:\s*([A-Z0-9]{4}-[A-Z0-9]{4})").unwrap();

        let text = "! First copy your one-time code: 9E41-5360";
        let caps = regex.captures(text).unwrap();
        assert_eq!(&caps[1], "9E41-5360");

        let text2 = "one-time code: ABCD-1234";
        let caps2 = regex.captures(text2).unwrap();
        assert_eq!(&caps2[1], "ABCD-1234");
    }

    #[test]
    fn test_url_regex() {
        let regex = Regex::new(r"(https://github\.com/login/device)").unwrap();

        let text = "Press Enter to open https://github.com/login/device in your browser";
        let caps = regex.captures(text).unwrap();
        assert_eq!(&caps[1], "https://github.com/login/device");
    }

    #[test]
    fn test_gh_auth_result_success() {
        let result = GhAuthResult::success();
        assert!(result.success);
        assert!(result.device_code.is_none());
        assert!(result.error.is_none());
    }

    #[test]
    fn test_gh_auth_result_with_code() {
        let result = GhAuthResult::with_code(
            "ABCD-1234".to_string(),
            "https://github.com/login/device".to_string(),
        );
        assert!(!result.success); // Not yet complete
        assert_eq!(result.device_code, Some("ABCD-1234".to_string()));
        assert_eq!(
            result.verification_url,
            Some("https://github.com/login/device".to_string())
        );
    }

    #[test]
    fn test_gh_auth_result_failed() {
        let result = GhAuthResult::failed("test error");
        assert!(!result.success);
        assert_eq!(result.error, Some("test error".to_string()));
    }
}
