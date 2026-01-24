//! Panic hook for terminal restoration.
//!
//! This module provides a panic hook that restores the terminal to a usable
//! state when the application panics. This ensures the user's terminal is
//! not left in an unusable state.

use super::setup::emergency_restore;
use std::panic;

/// Install a panic hook that restores the terminal.
///
/// This should be called early in main(), before creating the `TerminalManager`.
/// The panic hook will:
/// 1. Restore the terminal to a usable state
/// 2. Call the original panic hook (to print the panic message)
///
/// # Example
///
/// ```no_run
/// use spoq::terminal::setup_panic_hook;
///
/// fn main() {
///     // Install panic hook early
///     setup_panic_hook();
///
///     // ... rest of initialization ...
/// }
/// ```
pub fn setup_panic_hook() {
    let original_hook = panic::take_hook();

    panic::set_hook(Box::new(move |panic_info| {
        // Restore terminal state first
        emergency_restore();

        // Then call the original panic hook to display the panic message
        original_hook(panic_info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_setup_panic_hook_does_not_panic() {
        // This test verifies that setup_panic_hook can be called
        // Note: We can't easily test the actual panic behavior in a unit test
        // because it would require triggering a panic
        setup_panic_hook();

        // Reset to default hook to avoid affecting other tests
        let _ = panic::take_hook();
    }
}
