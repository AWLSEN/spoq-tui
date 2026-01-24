//! Terminal backend trait abstraction.
//!
//! Provides a trait-based abstraction for terminal rendering operations,
//! enabling dependency injection and mocking in tests.

use ratatui::backend::Backend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use std::io;

/// Terminal operation errors.
#[derive(Debug)]
pub enum TerminalError {
    /// IO error during terminal operation
    Io(io::Error),
    /// Terminal not available
    NotAvailable(String),
    /// Failed to initialize terminal
    InitFailed(String),
    /// Failed to restore terminal state
    RestoreFailed(String),
    /// Other error
    Other(String),
}

impl std::fmt::Display for TerminalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TerminalError::Io(err) => write!(f, "Terminal IO error: {}", err),
            TerminalError::NotAvailable(msg) => write!(f, "Terminal not available: {}", msg),
            TerminalError::InitFailed(msg) => write!(f, "Failed to initialize terminal: {}", msg),
            TerminalError::RestoreFailed(msg) => {
                write!(f, "Failed to restore terminal: {}", msg)
            }
            TerminalError::Other(msg) => write!(f, "Terminal error: {}", msg),
        }
    }
}

impl std::error::Error for TerminalError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            TerminalError::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for TerminalError {
    fn from(err: io::Error) -> Self {
        TerminalError::Io(err)
    }
}

/// Trait for terminal backend operations.
///
/// This trait abstracts terminal operations to enable dependency injection
/// and mocking in tests. It wraps the ratatui Backend trait with additional
/// lifecycle management.
///
/// # Example
///
/// ```ignore
/// use spoq::traits::TerminalBackend;
/// use ratatui::layout::Rect;
///
/// fn run_tui<B: TerminalBackend>(backend: &mut B) -> Result<(), TerminalError> {
///     backend.setup()?;
///
///     // Get terminal size
///     let size = backend.size()?;
///     println!("Terminal size: {}x{}", size.width, size.height);
///
///     // Draw frame
///     backend.draw(|frame| {
///         // Render widgets
///     })?;
///
///     backend.cleanup()?;
///     Ok(())
/// }
/// ```
pub trait TerminalBackend: Send {
    /// The underlying ratatui backend type.
    type Backend: Backend;

    /// Set up the terminal for TUI rendering.
    ///
    /// This typically involves:
    /// - Enabling raw mode
    /// - Entering alternate screen
    /// - Hiding the cursor
    ///
    /// # Returns
    /// Ok(()) on success, or an error if setup failed
    fn setup(&mut self) -> Result<(), TerminalError>;

    /// Clean up and restore the terminal to its original state.
    ///
    /// This typically involves:
    /// - Showing the cursor
    /// - Leaving alternate screen
    /// - Disabling raw mode
    ///
    /// # Returns
    /// Ok(()) on success, or an error if cleanup failed
    fn cleanup(&mut self) -> Result<(), TerminalError>;

    /// Get the current terminal size.
    ///
    /// # Returns
    /// The terminal size as a Rect, or an error
    fn size(&self) -> Result<Rect, TerminalError>;

    /// Get a mutable reference to the underlying terminal.
    ///
    /// This allows direct access to ratatui Terminal methods.
    fn terminal(&mut self) -> &mut Terminal<Self::Backend>;

    /// Draw a frame to the terminal.
    ///
    /// # Arguments
    /// * `f` - A closure that renders widgets to the frame
    ///
    /// # Returns
    /// Ok(()) on success, or an error if drawing failed
    fn draw<F>(&mut self, f: F) -> Result<(), TerminalError>
    where
        F: FnOnce(&mut ratatui::Frame);

    /// Clear the terminal screen.
    ///
    /// # Returns
    /// Ok(()) on success, or an error if clearing failed
    fn clear(&mut self) -> Result<(), TerminalError>;

    /// Force a full redraw of the terminal.
    ///
    /// This is useful when the terminal state may be corrupted
    /// or after resizing.
    fn force_redraw(&mut self) -> Result<(), TerminalError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn test_terminal_error_display() {
        let io_err = io::Error::new(io::ErrorKind::Other, "test error");
        assert!(TerminalError::Io(io_err).to_string().contains("IO error"));

        assert_eq!(
            TerminalError::NotAvailable("no tty".to_string()).to_string(),
            "Terminal not available: no tty"
        );
        assert_eq!(
            TerminalError::InitFailed("raw mode failed".to_string()).to_string(),
            "Failed to initialize terminal: raw mode failed"
        );
        assert_eq!(
            TerminalError::RestoreFailed("alternate screen".to_string()).to_string(),
            "Failed to restore terminal: alternate screen"
        );
        assert_eq!(
            TerminalError::Other("unknown".to_string()).to_string(),
            "Terminal error: unknown"
        );
    }

    #[test]
    fn test_terminal_error_from_io_error() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let term_err: TerminalError = io_err.into();
        assert!(matches!(term_err, TerminalError::Io(_)));
    }

    #[test]
    fn test_terminal_error_source() {
        let io_err = io::Error::new(io::ErrorKind::Other, "test");
        let term_err = TerminalError::Io(io_err);
        assert!(term_err.source().is_some());

        let term_err = TerminalError::NotAvailable("test".to_string());
        assert!(term_err.source().is_none());
    }
}
