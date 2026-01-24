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

pub use command::Command;
pub use context::{InputContext, ModalType};
pub use keybindings::{KeyCombo, KeybindingConfig};
pub use registry::CommandRegistry;

use crate::app::App;

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
        } else if self.thread_switcher.visible {
            ModalType::ThreadSwitcher
        } else if self.session_state.has_pending_permission() {
            if self.is_ask_user_question_pending() {
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
        // Mark dirty for most commands
        if cmd.marks_dirty() {
            self.mark_dirty();
        }

        // Emit debug event for the command
        self.emit_debug_state_change("Command", &format!("{:?}", cmd), "");

        // Try handlers in order of specificity
        // Modal handlers first (they have highest priority)
        match self.build_input_context().modal {
            ModalType::FolderPicker => {
                if handlers::handle_folder_picker_command(self, &cmd) {
                    return true;
                }
            }
            ModalType::ThreadSwitcher => {
                if handlers::handle_thread_switcher_command(self, &cmd) {
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
            ModalType::None => {}
        }

        // Try editing commands
        if handlers::handle_editing_command(self, &cmd) {
            return true;
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
}
