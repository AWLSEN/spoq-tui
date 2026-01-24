//! Keyboard enhancement support (Kitty protocol).
//!
//! This module provides functions for enabling and disabling keyboard
//! enhancements, specifically the Kitty keyboard protocol which allows
//! terminals to distinguish between key combinations like Ctrl+Enter
//! and Shift+Enter.
//!
//! Not all terminals support the Kitty protocol. Functions in this module
//! silently fail on unsupported terminals, allowing the application to
//! work with reduced functionality.

use crossterm::{
    event::{KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags},
    execute,
};
use std::io::Write;

/// Enable keyboard enhancements (Kitty protocol).
///
/// This enables the following keyboard enhancement flags:
/// - `DISAMBIGUATE_ESCAPE_CODES`: Allows distinguishing between escape sequences
/// - `REPORT_ALL_KEYS_AS_ESCAPE_CODES`: Reports all keys including modifiers
///
/// These flags enable features like:
/// - Shift+Enter vs Enter
/// - Ctrl+Enter vs Enter
/// - Other modifier combinations
///
/// # Arguments
///
/// * `writer` - The output writer (typically stdout)
///
/// # Returns
///
/// Returns `true` if enhancements were enabled, `false` if the terminal
/// doesn't support them (fails silently).
pub fn enable_keyboard_enhancements<W: Write>(writer: &mut W) -> bool {
    push_keyboard_enhancements(writer)
}

/// Push keyboard enhancement flags onto the stack.
///
/// This uses crossterm's stack-based API for keyboard enhancements.
/// The flags can be popped with `pop_keyboard_enhancements`.
///
/// # Arguments
///
/// * `writer` - The output writer (typically stdout)
///
/// # Returns
///
/// Returns `true` if the push succeeded, `false` otherwise.
pub fn push_keyboard_enhancements<W: Write>(writer: &mut W) -> bool {
    execute!(
        writer,
        PushKeyboardEnhancementFlags(
            KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
        )
    )
    .is_ok()
}

/// Pop keyboard enhancement flags from the stack.
///
/// This restores the keyboard enhancement state to what it was before
/// the last `push_keyboard_enhancements` call.
///
/// # Arguments
///
/// * `writer` - The output writer (typically stdout)
///
/// # Returns
///
/// Returns `true` if the pop succeeded, `false` otherwise.
pub fn pop_keyboard_enhancements<W: Write>(writer: &mut W) -> bool {
    execute!(writer, PopKeyboardEnhancementFlags).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_and_pop_do_not_panic() {
        let mut buffer = Vec::new();
        // These may return false on non-supporting terminals, but shouldn't panic
        let _ = push_keyboard_enhancements(&mut buffer);
        let _ = pop_keyboard_enhancements(&mut buffer);
    }
}
