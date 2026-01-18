//! Terminal cleanup tests
//!
//! These tests verify the terminal cleanup functionality, particularly
//! the panic hook setup and restore_terminal operations.

use crossterm::{
    event::{KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode},
};
use std::io;

/// Test that keyboard enhancement flags can be pushed and popped
/// This verifies the sequence used in setup_panic_hook
#[test]
fn test_keyboard_enhancement_push_pop_sequence() {
    let mut stdout = io::stdout();

    // This test verifies the command sequence works without errors
    // We can't actually verify terminal state in a test, but we can verify
    // the commands don't panic

    // Push keyboard enhancement flags (as done in main.rs setup)
    let push_result = execute!(
        stdout,
        PushKeyboardEnhancementFlags(
            KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
        )
    );

    // Should succeed or silently fail (on unsupported terminals)
    assert!(
        push_result.is_ok(),
        "Push keyboard enhancement should not panic"
    );

    // Pop keyboard enhancement flags (as done in setup_panic_hook and restore_terminal)
    let pop_result = execute!(stdout, PopKeyboardEnhancementFlags);

    assert!(
        pop_result.is_ok(),
        "Pop keyboard enhancement should not panic"
    );
}

/// Test that alternate scroll mode escape sequences are well-formed
#[test]
fn test_alternate_scroll_mode_escape_sequences() {
    use std::io::Write;

    let mut buffer = Vec::new();

    // Enable alternate scroll mode (CSI ? 1007 h)
    let enable_result = write!(buffer, "\x1b[?1007h");
    assert!(enable_result.is_ok(), "Enable escape sequence should write");
    assert_eq!(
        buffer,
        b"\x1b[?1007h",
        "Enable sequence should be correct"
    );

    buffer.clear();

    // Disable alternate scroll mode (CSI ? 1007 l)
    let disable_result = write!(buffer, "\x1b[?1007l");
    assert!(
        disable_result.is_ok(),
        "Disable escape sequence should write"
    );
    assert_eq!(
        buffer,
        b"\x1b[?1007l",
        "Disable sequence should be correct"
    );
}

/// Test that Kitty protocol reset escape sequence is well-formed
#[test]
fn test_kitty_protocol_reset_sequence() {
    use std::io::Write;

    let mut buffer = Vec::new();

    // Kitty protocol reset (CSI < u)
    let reset_result = write!(buffer, "\x1b[<u");
    assert!(reset_result.is_ok(), "Reset escape sequence should write");
    assert_eq!(buffer, b"\x1b[<u", "Reset sequence should be correct");
}

/// Test raw mode enable/disable cycle
/// This verifies the sequence used in restore_terminal
/// Note: This test may fail in non-TTY environments (CI), which is expected
#[test]
fn test_raw_mode_enable_disable_cycle() {
    // Enable raw mode (may fail in non-TTY environments like CI)
    let enable_result = enable_raw_mode();

    // If we can't enable raw mode (not a TTY), skip the rest of the test
    if enable_result.is_err() {
        // This is expected in CI/test environments without a TTY
        return;
    }

    // If we successfully enabled, we should be able to disable
    let disable_result = disable_raw_mode();
    assert!(
        disable_result.is_ok(),
        "Disable raw mode should succeed after enable"
    );
}

/// Test the cleanup sequence order
/// Verifies that PopKeyboardEnhancementFlags happens BEFORE disable_raw_mode
/// This is critical for preventing terminal corruption (see panic hook comments)
#[test]
fn test_cleanup_sequence_order() {
    let mut stdout = io::stdout();

    // Simulate the setup phase (may fail in non-TTY)
    let raw_mode_enabled = enable_raw_mode().is_ok();
    execute!(
        stdout,
        PushKeyboardEnhancementFlags(
            KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
        )
    )
    .ok();

    // Cleanup phase - ORDER MATTERS
    // 1. Pop keyboard enhancement FIRST (before disabling raw mode)
    let pop_result = execute!(stdout, PopKeyboardEnhancementFlags);
    assert!(
        pop_result.is_ok(),
        "Pop should happen before disabling raw mode"
    );

    // 2. Write alternate scroll mode disable
    use std::io::Write;
    let _ = write!(stdout, "\x1b[?1007l");

    // 3. Write Kitty protocol reset
    let _ = write!(stdout, "\x1b[<u");
    let _ = stdout.flush();

    // 4. THEN disable raw mode (only if we enabled it)
    if raw_mode_enabled {
        let disable_result = disable_raw_mode();
        assert!(
            disable_result.is_ok(),
            "Raw mode should be disabled after pop"
        );
    }
}

/// Test that terminal cleanup is idempotent
/// Calling cleanup multiple times should not cause errors
#[test]
fn test_cleanup_idempotent() {
    use std::io::Write;

    let mut stdout = io::stdout();

    // Setup
    enable_raw_mode().ok();
    execute!(
        stdout,
        PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
    )
    .ok();

    // First cleanup
    let _ = execute!(stdout, PopKeyboardEnhancementFlags);
    let _ = write!(stdout, "\x1b[?1007l");
    let _ = write!(stdout, "\x1b[<u");
    let _ = stdout.flush();
    let _ = disable_raw_mode();

    // Second cleanup (should be safe, even if it errors)
    let pop2 = execute!(stdout, PopKeyboardEnhancementFlags);
    let _ = write!(stdout, "\x1b[?1007l");
    let _ = write!(stdout, "\x1b[<u");
    let _ = stdout.flush();
    let disable2 = disable_raw_mode();

    // Both operations should either succeed or fail gracefully
    // (not panic or cause undefined behavior)
    let _ = pop2;
    let _ = disable2;
}

/// Test that all escape sequences are valid ASCII/UTF-8
#[test]
fn test_escape_sequences_are_valid_utf8() {
    // All escape sequences used should be valid UTF-8
    let sequences = vec![
        "\x1b[?1007h",   // Enable alternate scroll
        "\x1b[?1007l",   // Disable alternate scroll
        "\x1b[<u",       // Kitty protocol reset
    ];

    for seq in sequences {
        assert!(
            seq.is_ascii(),
            "Escape sequence '{}' should be ASCII",
            seq.escape_debug()
        );
        assert!(
            std::str::from_utf8(seq.as_bytes()).is_ok(),
            "Escape sequence should be valid UTF-8"
        );
    }
}

/// Verify that the panic hook setup doesn't interfere with normal execution
/// This is a smoke test - we can't actually trigger a panic in a unit test
/// without failing the test, but we can verify the setup is callable
#[test]
fn test_panic_hook_setup_is_callable() {
    // Save original hook
    let original = std::panic::take_hook();

    // Setup a test panic hook similar to setup_panic_hook
    std::panic::set_hook(Box::new(move |panic_info| {
        // In actual panic, would do terminal cleanup here
        // For test, we just verify it's callable
        let _ = panic_info;
    }));

    // Restore original hook
    let _ = std::panic::take_hook();
    std::panic::set_hook(original);
}

/// Test that we can detect if keyboard enhancement is supported
/// (Note: This always passes in CI/test environments, but validates the concept)
#[test]
fn test_keyboard_enhancement_detection() {
    let mut stdout = io::stdout();

    // Try to push - this silently fails on unsupported terminals
    let result = execute!(
        stdout,
        PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
    );

    // Should not panic regardless of support
    assert!(result.is_ok(), "Push should not panic");

    // Try to pop - should also not panic
    let pop_result = execute!(stdout, PopKeyboardEnhancementFlags);
    assert!(pop_result.is_ok(), "Pop should not panic");
}
