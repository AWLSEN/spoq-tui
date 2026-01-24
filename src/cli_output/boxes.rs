//! Simple line-based CLI output utilities.

use std::io::{self, Write};

/// Line width for separators.
const LINE_WIDTH: usize = 60;

/// Print the main header.
///
/// ```text
/// SPOQ VPS SETUP
/// ════════════════════════════════════════════════════════════
/// ```
pub fn print_header(title: &str) {
    println!();
    println!("{}", title);
    println!("{}", "═".repeat(LINE_WIDTH));
    println!();
}

/// Print the start of a step.
///
/// ```text
/// STEP 1: AUTHENTICATION
/// ────────────────────────────────────────────────────────────
/// ```
pub fn print_step_start(step: u8, title: &str) {
    println!("STEP {}: {}", step, title);
    println!("{}", "─".repeat(LINE_WIDTH));
}

/// Print a line within a step.
///
/// ```text
///   ✓ Authenticated as nidhish
/// ```
pub fn print_step_line(icon: &str, message: &str) {
    println!("  {} {}", icon, message);
}

/// Print a spinner line (overwrites current line).
pub fn print_step_spinner(spinner_char: char, message: &str) {
    print!("\r  {} {}", spinner_char, message);
    // Pad with spaces to clear any previous longer message
    print!("                    ");
    print!("\r  {} {}", spinner_char, message);
    io::stdout().flush().ok();
}

/// Clear the spinner line and print a final status line.
pub fn print_step_spinner_done(icon: &str, message: &str) {
    print!("\r");
    // Clear the line
    print!("                                                            ");
    print!("\r");
    print_step_line(icon, message);
}

/// Print the end of a step (just a blank line).
pub fn print_step_end() {
    println!();
}

/// Print troubleshooting lines.
pub fn print_troubleshoot(lines: &[&str]) {
    println!();
    for line in lines {
        println!("    {}", line);
    }
}

/// Print the success footer.
///
/// ```text
/// ════════════════════════════════════════════════════════════
/// ✓ SETUP COMPLETE
///
///   VPS:       123.45.67.89
///   Conductor: http://123.45.67.89:8080
///   SSH:       ssh root@123.45.67.89
/// ════════════════════════════════════════════════════════════
/// ```
pub fn print_footer_success(vps_ip: &str, conductor_url: &str, ssh_user: &str) {
    println!("{}", "═".repeat(LINE_WIDTH));
    println!("✓ SETUP COMPLETE");
    println!();
    println!("  VPS:       {}", vps_ip);
    println!("  Conductor: {}", conductor_url);
    println!("  SSH:       ssh {}@{}", ssh_user, vps_ip);
    println!("{}", "═".repeat(LINE_WIDTH));
}

/// Print the warning footer (partial success).
pub fn print_footer_warning(vps_ip: &str, conductor_url: &str, ssh_user: &str) {
    println!("{}", "═".repeat(LINE_WIDTH));
    println!("⚠ SETUP COMPLETE (with warnings)");
    println!();
    println!("  VPS:       {}", vps_ip);
    println!("  Conductor: {}", conductor_url);
    println!("  SSH:       ssh {}@{}", ssh_user, vps_ip);
    println!("{}", "═".repeat(LINE_WIDTH));
}

/// Status icons
pub mod icons {
    pub const SUCCESS: &str = "✓";
    pub const FAILURE: &str = "✗";
    pub const WARNING: &str = "⚠";
}

/// Spinner characters for loading animation.
pub const SPINNER_CHARS: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
