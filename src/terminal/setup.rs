//! Terminal setup and teardown functions.
//!
//! This module provides low-level functions for entering and leaving TUI mode.
//! These are used by `TerminalManager` but can also be used directly if needed.

use crossterm::{
    cursor::Show,
    event::{DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io::{self, Write};

/// Enter TUI mode.
///
/// This sets up the terminal for TUI operation:
/// - Enters alternate screen (preserves original terminal content)
/// - Enables bracketed paste (for handling multi-line pastes)
/// - Enables mouse capture (for scroll wheel and click events)
///
/// # Arguments
///
/// * `writer` - The output writer (typically stdout)
///
/// # Errors
///
/// Returns an error if any terminal commands fail.
pub fn enter_tui_mode<W: Write>(writer: &mut W) -> io::Result<()> {
    execute!(
        writer,
        EnterAlternateScreen,
        EnableBracketedPaste,
        EnableMouseCapture
    )
}

/// Leave TUI mode and restore terminal to normal state.
///
/// This performs cleanup in the correct order:
/// 1. Disables mouse capture
/// 2. Disables bracketed paste
/// 3. Leaves alternate screen (restores original terminal content)
/// 4. Hard resets Kitty keyboard protocol
/// 5. Shows the cursor
///
/// This function is designed to be safe to call multiple times and will
/// not panic on errors.
///
/// # Arguments
///
/// * `writer` - The output writer (typically stdout)
pub fn leave_tui_mode<W: Write>(writer: &mut W) {
    // Disable raw mode first to allow normal terminal operation
    let _ = disable_raw_mode();

    // Leave alternate screen and disable features
    let _ = execute!(
        writer,
        DisableMouseCapture,
        DisableBracketedPaste,
        LeaveAlternateScreen
    );

    // CRITICAL: Hard reset Kitty keyboard protocol AFTER leaving alternate screen
    // Some terminals (Ghostty, Kitty) need this sent after leaving alternate screen
    // CSI = 0 u sets all keyboard enhancement flags to zero (non-stack based reset)
    let _ = write!(writer, "\x1b[=0u");
    let _ = writer.flush();

    // Show the cursor
    let _ = execute!(writer, Show);
}

/// Restore terminal to a usable state after a panic or error.
///
/// This is a more aggressive cleanup function that attempts to restore
/// the terminal even in error conditions. It combines all cleanup steps
/// and ignores all errors.
pub fn emergency_restore() {
    let mut stdout = io::stdout();

    // Try to pop keyboard enhancements first
    use crossterm::event::PopKeyboardEnhancementFlags;
    let _ = execute!(stdout, PopKeyboardEnhancementFlags);

    // Then do the rest of the cleanup
    leave_tui_mode(&mut stdout);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_leave_tui_mode_does_not_panic() {
        // This test verifies that leave_tui_mode doesn't panic
        // even when called on a non-TUI terminal
        let mut buffer = Vec::new();
        leave_tui_mode(&mut buffer);
        // The buffer should contain some escape sequences
        // We don't verify the exact content since it depends on terminal state
    }

    #[test]
    fn test_emergency_restore_does_not_panic() {
        // This test verifies that emergency_restore doesn't panic
        emergency_restore();
    }
}
