//! Terminal management module with RAII pattern for automatic cleanup.
//!
//! This module provides a safe abstraction for terminal state management.
//! The `TerminalManager` ensures that terminal state is properly restored
//! when the application exits, whether normally or due to a panic.
//!
//! # Example
//!
//! ```no_run
//! use spoq::terminal::TerminalManager;
//!
//! fn main() -> color_eyre::Result<()> {
//!     // Terminal state is automatically managed
//!     let mut term_manager = TerminalManager::new()?;
//!
//!     // Get a reference to the terminal for drawing
//!     let terminal = term_manager.terminal();
//!
//!     // ... run your application ...
//!
//!     // Terminal is automatically restored when term_manager is dropped
//!     Ok(())
//! }
//! ```

mod enhancements;
mod panic;
mod setup;

pub use enhancements::{enable_keyboard_enhancements, push_keyboard_enhancements};
pub use panic::setup_panic_hook;
pub use setup::{enter_tui_mode, leave_tui_mode};

use color_eyre::Result;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, Stdout};

/// RAII guard that automatically restores terminal state on drop.
///
/// This guard is created by `TerminalManager` and should not be used directly.
/// When dropped, it performs the following cleanup:
/// 1. Pops keyboard enhancement flags (Kitty protocol)
/// 2. Disables raw mode
/// 3. Disables mouse capture
/// 4. Disables bracketed paste
/// 5. Leaves alternate screen
/// 6. Hard resets Kitty keyboard protocol
/// 7. Shows the cursor
pub struct TerminalGuard {
    /// Whether cleanup has already been performed
    cleaned_up: bool,
}

impl TerminalGuard {
    /// Create a new terminal guard.
    fn new() -> Self {
        Self { cleaned_up: false }
    }

    /// Manually perform cleanup.
    ///
    /// This is called by Drop, but can also be called manually if needed.
    /// Subsequent calls are no-ops.
    pub fn cleanup(&mut self) {
        if self.cleaned_up {
            return;
        }
        self.cleaned_up = true;

        // Perform cleanup in the correct order
        leave_tui_mode(&mut io::stdout());
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        self.cleanup();
    }
}

/// Manages terminal state with automatic cleanup via RAII.
///
/// `TerminalManager` sets up the terminal for TUI operation when created
/// and automatically restores the terminal to its original state when dropped.
/// This ensures the terminal is always left in a usable state, even if the
/// application panics.
///
/// # Panic Safety
///
/// The panic hook is installed by `setup_panic_hook()` which should be called
/// separately before creating the `TerminalManager`. The panic hook provides
/// additional safety by performing cleanup even in panic scenarios where the
/// normal Drop path might not execute.
pub struct TerminalManager {
    /// The ratatui terminal instance
    terminal: Terminal<CrosstermBackend<Stdout>>,
    /// RAII guard for cleanup
    _guard: TerminalGuard,
}

impl TerminalManager {
    /// Create a new terminal manager.
    ///
    /// This sets up the terminal for TUI operation:
    /// 1. Enables raw mode
    /// 2. Enters alternate screen
    /// 3. Enables bracketed paste
    /// 4. Enables mouse capture
    /// 5. Enables keyboard enhancements (Kitty protocol)
    /// 6. Clears the terminal
    ///
    /// # Errors
    ///
    /// Returns an error if terminal setup fails.
    pub fn new() -> Result<Self> {
        // Enable raw mode first
        enable_raw_mode()?;

        let mut stdout = io::stdout();

        // Enter TUI mode (alternate screen, bracketed paste, mouse capture)
        enter_tui_mode(&mut stdout)?;

        // Enable keyboard enhancements (Kitty protocol)
        // Silently fails on unsupported terminals
        push_keyboard_enhancements(&mut stdout);

        // Create the terminal
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Clear the terminal
        terminal.clear()?;

        // Create the guard (cleanup happens on drop)
        let guard = TerminalGuard::new();

        Ok(Self {
            terminal,
            _guard: guard,
        })
    }

    /// Get a mutable reference to the underlying terminal.
    pub fn terminal(&mut self) -> &mut Terminal<CrosstermBackend<Stdout>> {
        &mut self.terminal
    }

    /// Get the current terminal size.
    pub fn size(&self) -> Result<ratatui::prelude::Rect> {
        Ok(self.terminal.size()?.into())
    }

    /// Manually restore the terminal.
    ///
    /// This is called automatically on drop, but can be called manually
    /// if you need to restore the terminal before dropping the manager.
    ///
    /// After calling this, the terminal manager should be dropped.
    pub fn restore(&mut self) -> Result<()> {
        // Pop keyboard enhancement flags
        enhancements::pop_keyboard_enhancements(self.terminal.backend_mut());

        // Disable raw mode
        disable_raw_mode()?;

        // Leave TUI mode
        leave_tui_mode(self.terminal.backend_mut());

        // Show cursor
        self.terminal.show_cursor()?;

        Ok(())
    }
}
