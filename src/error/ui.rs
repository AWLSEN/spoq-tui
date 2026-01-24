//! UI-related error types.
//!
//! This module defines errors that occur during UI operations,
//! rendering, input handling, and display management.

use std::fmt;

/// UI-specific error variants.
///
/// These errors represent issues with terminal UI rendering,
/// input handling, and user interface operations.
#[derive(Debug, Clone)]
pub enum UiError {
    /// Terminal initialization failed.
    TerminalInitFailed {
        message: String,
    },

    /// Terminal restore failed.
    TerminalRestoreFailed {
        message: String,
    },

    /// Failed to get terminal size.
    TerminalSizeFailed {
        message: String,
    },

    /// Rendering error.
    RenderFailed {
        component: String,
        message: String,
    },

    /// Input handling error.
    InputError {
        message: String,
    },

    /// Clipboard operation failed.
    ClipboardError {
        operation: String,
        message: String,
    },

    /// Browser launch failed.
    BrowserLaunchFailed {
        url: String,
        message: String,
    },

    /// Event channel error.
    ChannelError {
        message: String,
    },

    /// Component state error.
    InvalidState {
        component: String,
        expected: String,
        actual: String,
    },

    /// Animation or transition error.
    AnimationError {
        message: String,
    },

    /// Generic UI error.
    Other {
        message: String,
    },
}

impl UiError {
    /// Check if this error is recoverable (UI can continue working).
    pub fn is_recoverable(&self) -> bool {
        !matches!(
            self,
            UiError::TerminalInitFailed { .. }
                | UiError::TerminalRestoreFailed { .. }
                | UiError::ChannelError { .. }
        )
    }

    /// Get a user-friendly error message.
    pub fn user_message(&self) -> String {
        match self {
            UiError::TerminalInitFailed { .. } => {
                "Failed to initialize the terminal. Please check your terminal settings.".to_string()
            }
            UiError::TerminalRestoreFailed { .. } => {
                "Failed to restore terminal. You may need to reset your terminal settings.".to_string()
            }
            UiError::TerminalSizeFailed { .. } => {
                "Could not determine terminal size. Please resize your terminal window.".to_string()
            }
            UiError::RenderFailed { component, .. } => {
                format!("Failed to render {}. Please try again.", component)
            }
            UiError::InputError { .. } => {
                "An error occurred while processing your input. Please try again.".to_string()
            }
            UiError::ClipboardError { operation, .. } => {
                format!(
                    "Failed to {} clipboard. Check clipboard permissions.",
                    operation
                )
            }
            UiError::BrowserLaunchFailed { url, .. } => {
                format!(
                    "Could not open browser. Please manually navigate to: {}",
                    url
                )
            }
            UiError::ChannelError { .. } => {
                "Internal communication error. Please restart the application.".to_string()
            }
            UiError::InvalidState { component, .. } => {
                format!(
                    "The {} is in an unexpected state. This may be a bug.",
                    component
                )
            }
            UiError::AnimationError { .. } => {
                "A visual glitch occurred. Please try refreshing the display.".to_string()
            }
            UiError::Other { message } => {
                format!("UI error: {}", message)
            }
        }
    }

    /// Get a short error code for logging.
    pub fn error_code(&self) -> &'static str {
        match self {
            UiError::TerminalInitFailed { .. } => "E_UI_TERM_INIT",
            UiError::TerminalRestoreFailed { .. } => "E_UI_TERM_RESTORE",
            UiError::TerminalSizeFailed { .. } => "E_UI_TERM_SIZE",
            UiError::RenderFailed { .. } => "E_UI_RENDER",
            UiError::InputError { .. } => "E_UI_INPUT",
            UiError::ClipboardError { .. } => "E_UI_CLIPBOARD",
            UiError::BrowserLaunchFailed { .. } => "E_UI_BROWSER",
            UiError::ChannelError { .. } => "E_UI_CHANNEL",
            UiError::InvalidState { .. } => "E_UI_STATE",
            UiError::AnimationError { .. } => "E_UI_ANIM",
            UiError::Other { .. } => "E_UI_OTHER",
        }
    }
}

impl fmt::Display for UiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UiError::TerminalInitFailed { message } => {
                write!(f, "Terminal initialization failed: {}", message)
            }
            UiError::TerminalRestoreFailed { message } => {
                write!(f, "Terminal restore failed: {}", message)
            }
            UiError::TerminalSizeFailed { message } => {
                write!(f, "Failed to get terminal size: {}", message)
            }
            UiError::RenderFailed { component, message } => {
                write!(f, "Render failed for '{}': {}", component, message)
            }
            UiError::InputError { message } => {
                write!(f, "Input error: {}", message)
            }
            UiError::ClipboardError { operation, message } => {
                write!(f, "Clipboard {} failed: {}", operation, message)
            }
            UiError::BrowserLaunchFailed { url, message } => {
                write!(f, "Failed to open browser for '{}': {}", url, message)
            }
            UiError::ChannelError { message } => {
                write!(f, "Event channel error: {}", message)
            }
            UiError::InvalidState { component, expected, actual } => {
                write!(
                    f,
                    "Invalid state for '{}': expected {}, got {}",
                    component, expected, actual
                )
            }
            UiError::AnimationError { message } => {
                write!(f, "Animation error: {}", message)
            }
            UiError::Other { message } => {
                write!(f, "UI error: {}", message)
            }
        }
    }
}

impl std::error::Error for UiError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_init_failed_not_recoverable() {
        let err = UiError::TerminalInitFailed {
            message: "no tty".to_string(),
        };
        assert!(!err.is_recoverable());
        assert_eq!(err.error_code(), "E_UI_TERM_INIT");
    }

    #[test]
    fn test_terminal_restore_failed_not_recoverable() {
        let err = UiError::TerminalRestoreFailed {
            message: "failed".to_string(),
        };
        assert!(!err.is_recoverable());
        assert_eq!(err.error_code(), "E_UI_TERM_RESTORE");
    }

    #[test]
    fn test_terminal_size_failed_is_recoverable() {
        let err = UiError::TerminalSizeFailed {
            message: "ioctl failed".to_string(),
        };
        assert!(err.is_recoverable());
        assert_eq!(err.error_code(), "E_UI_TERM_SIZE");
    }

    #[test]
    fn test_render_failed_is_recoverable() {
        let err = UiError::RenderFailed {
            component: "message_list".to_string(),
            message: "out of bounds".to_string(),
        };
        assert!(err.is_recoverable());
        assert_eq!(err.error_code(), "E_UI_RENDER");
        assert!(err.user_message().contains("message_list"));
    }

    #[test]
    fn test_input_error_is_recoverable() {
        let err = UiError::InputError {
            message: "invalid key sequence".to_string(),
        };
        assert!(err.is_recoverable());
        assert_eq!(err.error_code(), "E_UI_INPUT");
    }

    #[test]
    fn test_clipboard_error_is_recoverable() {
        let err = UiError::ClipboardError {
            operation: "copy".to_string(),
            message: "no clipboard provider".to_string(),
        };
        assert!(err.is_recoverable());
        assert_eq!(err.error_code(), "E_UI_CLIPBOARD");
        assert!(err.user_message().contains("copy"));
    }

    #[test]
    fn test_browser_launch_failed_is_recoverable() {
        let err = UiError::BrowserLaunchFailed {
            url: "https://example.com".to_string(),
            message: "no browser found".to_string(),
        };
        assert!(err.is_recoverable());
        assert_eq!(err.error_code(), "E_UI_BROWSER");
        assert!(err.user_message().contains("https://example.com"));
    }

    #[test]
    fn test_channel_error_not_recoverable() {
        let err = UiError::ChannelError {
            message: "receiver dropped".to_string(),
        };
        assert!(!err.is_recoverable());
        assert_eq!(err.error_code(), "E_UI_CHANNEL");
    }

    #[test]
    fn test_invalid_state_is_recoverable() {
        let err = UiError::InvalidState {
            component: "thread_list".to_string(),
            expected: "loaded".to_string(),
            actual: "loading".to_string(),
        };
        assert!(err.is_recoverable());
        assert_eq!(err.error_code(), "E_UI_STATE");
        assert!(err.user_message().contains("thread_list"));
    }

    #[test]
    fn test_animation_error_is_recoverable() {
        let err = UiError::AnimationError {
            message: "timing error".to_string(),
        };
        assert!(err.is_recoverable());
        assert_eq!(err.error_code(), "E_UI_ANIM");
    }

    #[test]
    fn test_display_format() {
        let err = UiError::RenderFailed {
            component: "sidebar".to_string(),
            message: "buffer overflow".to_string(),
        };
        let display = format!("{}", err);
        assert!(display.contains("sidebar"));
        assert!(display.contains("buffer overflow"));
    }

    #[test]
    fn test_user_messages() {
        let err_init = UiError::TerminalInitFailed {
            message: "test".to_string(),
        };
        assert!(err_init.user_message().contains("terminal settings"));

        let err_channel = UiError::ChannelError {
            message: "test".to_string(),
        };
        assert!(err_channel.user_message().contains("restart"));
    }
}
