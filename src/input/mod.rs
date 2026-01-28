//! Input handling module for keyboard and command processing.
//!
//! This module provides a command pattern implementation for handling keyboard input.
//! Instead of inline key handling scattered throughout the codebase, all input is:
//!
//! 1. Translated to a [`Command`] by the [`CommandRegistry`]
//! 2. Dispatched to appropriate handlers in the [`handlers`] module
//!
//! # Architecture
//!
//! ```text
//! KeyEvent -> CommandRegistry::dispatch() -> Command -> Handler -> App mutation
//! ```
//!
//! # Example
//!
//! ```ignore
//! use spoq::input::{CommandRegistry, InputContext, Command};
//!
//! let registry = CommandRegistry::new();
//! let context = InputContext::from_app(&app);
//!
//! if let Some(cmd) = registry.dispatch(key_event, &context) {
//!     app.execute_command(cmd);
//! }
//! ```
//!
//! # Modules
//!
//! - [`command`] - The [`Command`] enum with all possible user actions
//! - [`context`] - [`InputContext`] for tracking current UI state
//! - [`registry`] - [`CommandRegistry`] for mapping keys to commands
//! - [`keybindings`] - Default key binding configuration
//! - [`handlers`] - Command execution handlers

pub mod command;
pub mod context;
pub mod handlers;
pub mod keybindings;
pub mod registry;
pub mod slash_command;

pub use command::Command;
pub use context::{InputContext, ModalType};
pub use keybindings::{KeyCombo, KeybindingConfig};
pub use registry::CommandRegistry;
pub use slash_command::SlashCommand;

use crate::app::{App, Screen};
use crate::view_state::OverlayState;

/// Translates a base character to its shifted equivalent for US keyboard layout.
///
/// This function maps characters to what they become when Shift is pressed:
/// - Number keys: 1→!, 2→@, 3→#, etc.
/// - Symbol keys: -→_, =→+, [→{, etc.
/// - Letters are returned uppercase (already handled by caller)
/// - Unrecognized characters are returned unchanged
///
/// # Examples
///
/// ```
/// use spoq::input::translate_shifted_char;
///
/// assert_eq!(translate_shifted_char('1'), '!');
/// assert_eq!(translate_shifted_char('-'), '_');
/// assert_eq!(translate_shifted_char('a'), 'a'); // Letters unchanged (caller handles case)
/// ```
pub fn translate_shifted_char(c: char) -> char {
    match c {
        // Number row
        '1' => '!',
        '2' => '@',
        '3' => '#',
        '4' => '$',
        '5' => '%',
        '6' => '^',
        '7' => '&',
        '8' => '*',
        '9' => '(',
        '0' => ')',
        // Symbol keys
        '-' => '_',
        '=' => '+',
        '[' => '{',
        ']' => '}',
        '\\' => '|',
        ';' => ':',
        '\'' => '"',
        ',' => '<',
        '.' => '>',
        '/' => '?',
        '`' => '~',
        // Everything else (including letters) unchanged
        _ => c,
    }
}

impl App {
    /// Builds an InputContext from the current application state.
    ///
    /// This captures all relevant state needed for the command registry
    /// to determine which commands are available.
    pub fn build_input_context(&self) -> InputContext {
        let modal = if self.folder_picker_visible {
            ModalType::FolderPicker
        } else if self.file_picker.visible {
            ModalType::FilePicker
        } else if self.slash_autocomplete_visible {
            ModalType::SlashAutocomplete
        } else if self.thread_switcher.visible {
            ModalType::ThreadSwitcher
        } else if self.screen == Screen::CommandDeck {
            // On CommandDeck, check for dashboard overlay FIRST (takes priority)
            if let Some(OverlayState::Question { .. }) = self.dashboard.overlay() {
                if self.dashboard.is_question_other_active() {
                    ModalType::DashboardQuestionOverlayOther
                } else {
                    ModalType::DashboardQuestionOverlay
                }
            } else if let Some(overlay) = self.dashboard.overlay() {
                // Check for thread-scoped permission using overlay's thread_id
                let thread_id = overlay.thread_id();
                if let Some(perm) = self.dashboard.get_pending_permission(thread_id) {
                    // Check if this is an AskUserQuestion by tool_name
                    if perm.tool_name == "AskUserQuestion" && perm.tool_input.is_some() {
                        if self.question_state.other_active {
                            ModalType::AskUserQuestionOther
                        } else {
                            ModalType::AskUserQuestion
                        }
                    } else {
                        ModalType::Permission
                    }
                } else {
                    ModalType::None
                }
            } else {
                ModalType::None
            }
        } else if let Some(ref thread_id) = self.active_thread_id {
            // For Conversation screen, check thread-scoped permission or plan approval
            if let Some(perm) = self.dashboard.get_pending_permission(thread_id) {
                // Check if this is an AskUserQuestion by tool_name
                if perm.tool_name == "AskUserQuestion" && perm.tool_input.is_some() {
                    if self.question_state.other_active {
                        ModalType::AskUserQuestionOther
                    } else {
                        ModalType::AskUserQuestion
                    }
                } else {
                    ModalType::Permission
                }
            } else if self.dashboard.get_plan_request(thread_id).is_some() {
                // Plan approval is pending (e.g., ExitPlanMode converted to plan approval)
                ModalType::PlanApproval
            } else {
                ModalType::None
            }
        } else {
            ModalType::None
        };

        let (row, col) = self.textarea.cursor();
        let lines = self.textarea.lines();
        let line_content = lines.get(row).map(|s| s.to_string()).unwrap_or_default();

        InputContext {
            screen: self.screen,
            focus: self.focus,
            modal,
            input_is_empty: self.textarea.is_empty(),
            cursor_on_first_line: self.textarea.is_cursor_on_first_line(),
            cursor_on_last_line: self.textarea.is_cursor_on_last_line(),
            is_navigating_history: self.input_history.is_navigating(),
            has_oauth_url: self.session_state.oauth_url.is_some(),
            has_errors: self.has_errors(),
            current_line_content: line_content,
            cursor_column: col,
        }
    }

    /// Executes a command, delegating to the appropriate handler.
    ///
    /// This is the main entry point for command execution after
    /// the registry has translated a key event to a command.
    ///
    /// Returns `true` if the command was handled.
    pub fn execute_command(&mut self, cmd: Command) -> bool {
        tracing::info!("execute_command: {:?}", cmd);

        // Mark dirty for most commands
        if cmd.marks_dirty() {
            self.mark_dirty();
        }

        // Emit debug event for the command
        self.emit_debug_state_change("Command", &format!("{:?}", cmd), "");

        // Try handlers in order of specificity
        // Modal handlers first (they have highest priority)
        let modal = self.build_input_context().modal;
        tracing::info!("execute_command: modal={:?}", modal);
        match modal {
            ModalType::FolderPicker => {
                if handlers::handle_folder_picker_command(self, &cmd) {
                    return true;
                }
            }
            ModalType::FilePicker => {
                if handlers::handle_file_picker_command(self, &cmd) {
                    return true;
                }
            }
            ModalType::SlashAutocomplete => {
                if handlers::handle_slash_autocomplete_command(self, &cmd) {
                    return true;
                }
            }
            ModalType::ThreadSwitcher => {
                if handlers::handle_thread_switcher_command(self, &cmd) {
                    return true;
                }
            }
            ModalType::DashboardQuestionOverlay
            | ModalType::DashboardQuestionOverlayOther => {
                if handlers::handle_dashboard_question_command(self, &cmd) {
                    return true;
                }
            }
            ModalType::Permission
            | ModalType::AskUserQuestion
            | ModalType::AskUserQuestionOther => {
                if handlers::handle_permission_command(self, &cmd) {
                    return true;
                }
                if handlers::handle_question_command(self, &cmd) {
                    return true;
                }
            }
            ModalType::PlanApproval => {
                if handlers::handle_plan_approval_command(self, &cmd) {
                    return true;
                }
            }
            ModalType::None => {}
        }

        // Try editing commands - skip if permission modal is active
        // (user must respond to permission before typing more input)
        if modal != ModalType::Permission && modal != ModalType::AskUserQuestion {
            tracing::debug!("About to call handle_editing_command for {:?}", cmd);
            if handlers::handle_editing_command(self, &cmd) {
                tracing::debug!("handle_editing_command returned true");
                return true;
            }
            tracing::debug!("handle_editing_command returned false");
        }

        // Try navigation commands
        if handlers::handle_navigation_command(self, &cmd) {
            return true;
        }

        // Try miscellaneous commands
        if handlers::handle_misc_command(self, &cmd) {
            return true;
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Focus, Screen};

    fn create_test_app() -> App {
        App::default()
    }

    #[test]
    fn test_build_input_context_default() {
        let app = create_test_app();
        let ctx = app.build_input_context();

        assert_eq!(ctx.screen, Screen::CommandDeck);
        assert_eq!(ctx.focus, Focus::Threads);
        assert_eq!(ctx.modal, ModalType::None);
        assert!(ctx.input_is_empty);
    }

    #[test]
    fn test_build_input_context_with_folder_picker() {
        let mut app = create_test_app();
        app.folder_picker_visible = true;

        let ctx = app.build_input_context();
        assert_eq!(ctx.modal, ModalType::FolderPicker);
    }

    #[test]
    fn test_build_input_context_with_thread_switcher() {
        let mut app = create_test_app();
        app.thread_switcher.visible = true;

        let ctx = app.build_input_context();
        assert_eq!(ctx.modal, ModalType::ThreadSwitcher);
    }

    #[test]
    fn test_build_input_context_conversation_screen() {
        let mut app = create_test_app();
        app.screen = Screen::Conversation;
        app.focus = Focus::Input;

        let ctx = app.build_input_context();
        assert_eq!(ctx.screen, Screen::Conversation);
        assert_eq!(ctx.focus, Focus::Input);
    }

    #[test]
    fn test_build_input_context_with_input() {
        let mut app = create_test_app();
        app.textarea.set_content("hello");

        let ctx = app.build_input_context();
        assert!(!ctx.input_is_empty);
    }

    #[test]
    fn test_execute_command_quit() {
        let mut app = create_test_app();
        assert!(!app.should_quit);

        let handled = app.execute_command(Command::Quit);
        assert!(handled);
        assert!(app.should_quit);
    }

    #[test]
    fn test_execute_command_insert_char() {
        let mut app = create_test_app();

        let handled = app.execute_command(Command::InsertChar('x'));
        assert!(handled);
        assert!(app.textarea.content().contains('x'));
    }

    #[test]
    fn test_execute_command_move_up() {
        let mut app = create_test_app();

        let handled = app.execute_command(Command::MoveUp);
        assert!(handled);
    }

    #[test]
    fn test_execute_command_noop() {
        let mut app = create_test_app();

        let handled = app.execute_command(Command::Noop);
        assert!(handled);
    }

    #[test]
    fn test_full_dispatch_and_execute() {
        use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

        let mut app = create_test_app();
        let registry = CommandRegistry::new();
        let context = app.build_input_context();

        let key = KeyEvent {
            code: KeyCode::Char('a'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };

        if let Some(cmd) = registry.dispatch(key, &context) {
            let handled = app.execute_command(cmd);
            assert!(handled);
            assert!(app.textarea.content().contains('a'));
        } else {
            panic!("Expected command to be dispatched");
        }
    }

    // Tests for translate_shifted_char function

    #[test]
    fn test_translate_shifted_char_number_keys() {
        assert_eq!(translate_shifted_char('1'), '!');
        assert_eq!(translate_shifted_char('2'), '@');
        assert_eq!(translate_shifted_char('3'), '#');
        assert_eq!(translate_shifted_char('4'), '$');
        assert_eq!(translate_shifted_char('5'), '%');
        assert_eq!(translate_shifted_char('6'), '^');
        assert_eq!(translate_shifted_char('7'), '&');
        assert_eq!(translate_shifted_char('8'), '*');
        assert_eq!(translate_shifted_char('9'), '(');
        assert_eq!(translate_shifted_char('0'), ')');
    }

    #[test]
    fn test_translate_shifted_char_symbol_keys() {
        assert_eq!(translate_shifted_char('-'), '_');
        assert_eq!(translate_shifted_char('='), '+');
        assert_eq!(translate_shifted_char('['), '{');
        assert_eq!(translate_shifted_char(']'), '}');
        assert_eq!(translate_shifted_char('\\'), '|');
        assert_eq!(translate_shifted_char(';'), ':');
        assert_eq!(translate_shifted_char('\''), '"');
        assert_eq!(translate_shifted_char(','), '<');
        assert_eq!(translate_shifted_char('.'), '>');
        assert_eq!(translate_shifted_char('/'), '?');
        assert_eq!(translate_shifted_char('`'), '~');
    }

    #[test]
    fn test_translate_shifted_char_letters_unchanged() {
        // Letters should be returned unchanged (uppercase handling is done by caller)
        assert_eq!(translate_shifted_char('a'), 'a');
        assert_eq!(translate_shifted_char('z'), 'z');
        assert_eq!(translate_shifted_char('A'), 'A');
        assert_eq!(translate_shifted_char('Z'), 'Z');
        assert_eq!(translate_shifted_char('m'), 'm');
    }

    #[test]
    fn test_translate_shifted_char_unrecognized() {
        // Unrecognized characters should be returned unchanged
        assert_eq!(translate_shifted_char('!'), '!');
        assert_eq!(translate_shifted_char('@'), '@');
        assert_eq!(translate_shifted_char(' '), ' ');
        assert_eq!(translate_shifted_char('\n'), '\n');
        assert_eq!(translate_shifted_char('€'), '€');
    }

    #[test]
    fn test_translate_shifted_char_all_number_mappings() {
        let number_mappings = [
            ('1', '!'),
            ('2', '@'),
            ('3', '#'),
            ('4', '$'),
            ('5', '%'),
            ('6', '^'),
            ('7', '&'),
            ('8', '*'),
            ('9', '('),
            ('0', ')'),
        ];

        for (input, expected) in number_mappings {
            assert_eq!(
                translate_shifted_char(input),
                expected,
                "Failed for input '{}', expected '{}', got '{}'",
                input,
                expected,
                translate_shifted_char(input)
            );
        }
    }

    #[test]
    fn test_translate_shifted_char_all_symbol_mappings() {
        let symbol_mappings = [
            ('-', '_'),
            ('=', '+'),
            ('[', '{'),
            (']', '}'),
            ('\\', '|'),
            (';', ':'),
            ('\'', '"'),
            (',', '<'),
            ('.', '>'),
            ('/', '?'),
            ('`', '~'),
        ];

        for (input, expected) in symbol_mappings {
            assert_eq!(
                translate_shifted_char(input),
                expected,
                "Failed for input '{}', expected '{}', got '{}'",
                input,
                expected,
                translate_shifted_char(input)
            );
        }
    }

    #[test]
    fn test_editing_blocked_when_permission_pending() {
        use crate::state::PermissionRequest;

        let mut app = create_test_app();

        // Set up conversation screen with an active thread
        app.screen = Screen::Conversation;
        app.active_thread_id = Some("test-thread-123".to_string());

        // Add a pending permission for this thread
        app.dashboard.set_pending_permission(
            "test-thread-123",
            PermissionRequest {
                permission_id: "perm-001".to_string(),
                thread_id: Some("test-thread-123".to_string()),
                tool_name: "Bash".to_string(),
                description: "Run command".to_string(),
                context: None,
                tool_input: None,
                received_at: std::time::Instant::now(),
            },
        );

        // Verify modal type is Permission
        let ctx = app.build_input_context();
        assert_eq!(ctx.modal, ModalType::Permission);

        // Clear textarea first to ensure it's empty
        app.textarea.clear();
        assert!(app.textarea.content().is_empty());

        // Try to insert a character - should be blocked
        let handled = app.execute_command(Command::InsertChar('x'));

        // The command should not be handled (falls through)
        assert!(!handled);

        // The textarea should remain empty (editing was blocked)
        assert!(
            app.textarea.content().is_empty(),
            "Textarea should be empty when permission is pending"
        );
    }

    #[test]
    fn test_editing_allowed_when_no_permission_pending() {
        let mut app = create_test_app();

        // Set up conversation screen with an active thread but NO permission
        app.screen = Screen::Conversation;
        app.active_thread_id = Some("test-thread-456".to_string());

        // Verify modal type is None
        let ctx = app.build_input_context();
        assert_eq!(ctx.modal, ModalType::None);

        // Clear textarea first
        app.textarea.clear();
        assert!(app.textarea.content().is_empty());

        // Insert a character - should work
        let handled = app.execute_command(Command::InsertChar('y'));

        // The command should be handled
        assert!(handled);

        // The textarea should contain the character
        assert!(
            app.textarea.content().contains('y'),
            "Textarea should contain 'y' when no permission is pending"
        );
    }
}
