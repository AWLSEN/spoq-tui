//! Editing command handlers.
//!
//! Handles commands related to text input, cursor movement,
//! and text manipulation in the textarea.

use crate::app::{App, Focus, Screen};
use crate::input::Command;
use crate::models::ThreadType;

/// Handles editing-related commands.
///
/// Returns `true` if the command was handled successfully.
pub fn handle_editing_command(app: &mut App, cmd: &Command) -> bool {
    eprintln!("DEBUG: handle_editing_command called with: {:?}", cmd);
    match cmd {
        Command::InsertChar(c) => {
            eprintln!("DEBUG: InsertChar received: '{}' (screen={:?})", c, app.screen);

            // Auto-focus to input if not already focused
            if app.focus != Focus::Input {
                app.focus = Focus::Input;
            }

            // Reset scroll to show input when typing
            if app.screen == Screen::Conversation {
                app.user_has_scrolled = false;
                app.unified_scroll = 0;
            }

            // Reset cursor blink on any character input
            app.reset_cursor_blink();

            // Check for @ trigger for unified picker (repos, threads, folders)
            if *c == '@' && app.screen == Screen::CommandDeck {
                let (row, col) = app.textarea.cursor();
                let lines = app.textarea.lines();
                let line_content = lines.get(row).map(|s| s.as_str()).unwrap_or("");

                if app.is_folder_picker_trigger(line_content, col) {
                    app.textarea.insert_char('@');
                    app.open_unified_picker();
                    return true;
                }
            }

            // Check for / trigger for slash command autocomplete (only on CommandDeck)
            // TEMP: Always trigger on / for debugging
            if *c == '/' && app.screen == Screen::CommandDeck {
                eprintln!("DEBUG: Slash autocomplete triggered!");
                app.textarea.insert_char('/');
                app.slash_autocomplete_visible = true;
                app.slash_autocomplete_query.clear();
                app.slash_autocomplete_cursor = 0;
                app.mark_dirty();
                eprintln!("DEBUG: slash_autocomplete_visible = {}", app.slash_autocomplete_visible);
                return true;
            }

            // Normal character insertion
            app.textarea.insert_char(*c);
            true
        }

        Command::InsertNewline => {
            app.reset_cursor_blink();
            app.textarea.insert_newline();
            true
        }

        Command::Backspace => {
            app.reset_cursor_blink();
            // Check if we should clear the folder chip instead of backspace
            if app.should_clear_folder_on_backspace() {
                app.clear_folder();
            } else {
                app.textarea.backspace();
            }
            true
        }

        Command::DeleteChar => {
            app.reset_cursor_blink();
            app.textarea.delete_char();
            true
        }

        Command::DeleteWordBackward => {
            app.reset_cursor_blink();
            app.textarea.delete_word_backward();
            true
        }

        Command::DeleteToLineStart => {
            app.reset_cursor_blink();
            app.textarea.delete_to_line_start();
            true
        }

        Command::MoveCursorLeft => {
            app.reset_cursor_blink();
            app.textarea.move_cursor_left();
            true
        }

        Command::MoveCursorRight => {
            app.reset_cursor_blink();
            app.textarea.move_cursor_right();
            true
        }

        Command::MoveCursorUp => {
            app.reset_cursor_blink();
            app.textarea.move_cursor_up();
            true
        }

        Command::MoveCursorDown => {
            app.reset_cursor_blink();
            app.textarea.move_cursor_down();
            true
        }

        Command::MoveCursorHome => {
            app.reset_cursor_blink();
            app.textarea.move_cursor_home();
            true
        }

        Command::MoveCursorEnd => {
            app.reset_cursor_blink();
            app.textarea.move_cursor_end();
            true
        }

        Command::MoveCursorWordLeft => {
            app.reset_cursor_blink();
            app.textarea.move_cursor_word_left();
            true
        }

        Command::MoveCursorWordRight => {
            app.reset_cursor_blink();
            app.textarea.move_cursor_word_right();
            true
        }

        Command::HistoryUp => {
            app.reset_cursor_blink();
            let current_content = app.textarea.content();
            if let Some(history_entry) = app.input_history.navigate_up(&current_content) {
                let entry = history_entry.to_string();
                app.textarea.set_content(&entry);
            }
            true
        }

        Command::HistoryDown => {
            app.reset_cursor_blink();
            if let Some(history_entry) = app.input_history.navigate_down() {
                let entry = history_entry.to_string();
                app.textarea.set_content(&entry);
            } else {
                // At bottom of history, restore original input
                let original = app.input_history.get_current_input().to_string();
                app.textarea.set_content(&original);
            }
            true
        }

        Command::SubmitInput(thread_type) => {
            app.submit_input(*thread_type);
            true
        }

        Command::SubmitAsProgramming => {
            if app.screen == Screen::CommandDeck && !app.textarea.is_empty() {
                app.submit_input(ThreadType::Programming);
                true
            } else {
                false
            }
        }

        Command::Paste(text) => {
            // Auto-focus to input if not already focused
            if app.focus != Focus::Input {
                app.focus = Focus::Input;
            }

            app.reset_cursor_blink();

            if app.should_summarize_paste(text) {
                app.textarea.insert_paste_token(text.clone());
            } else {
                for ch in text.chars() {
                    app.textarea.insert_char(ch);
                }
            }
            app.mark_dirty();
            true
        }

        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_app() -> App {
        App::default()
    }

    #[test]
    fn test_handle_insert_char() {
        let mut app = create_test_app();
        app.focus = Focus::Input;

        let handled = handle_editing_command(&mut app, &Command::InsertChar('a'));
        assert!(handled);
        assert!(app.textarea.content().contains('a'));
    }

    #[test]
    fn test_handle_insert_char_auto_focuses() {
        let mut app = create_test_app();
        app.focus = Focus::Threads;

        let handled = handle_editing_command(&mut app, &Command::InsertChar('a'));
        assert!(handled);
        assert_eq!(app.focus, Focus::Input);
    }

    #[test]
    fn test_handle_insert_newline() {
        let mut app = create_test_app();
        app.focus = Focus::Input;
        app.textarea.insert_char('a');

        let handled = handle_editing_command(&mut app, &Command::InsertNewline);
        assert!(handled);
        assert!(app.textarea.content().contains('\n'));
    }

    #[test]
    fn test_handle_backspace() {
        let mut app = create_test_app();
        app.focus = Focus::Input;
        app.textarea.insert_char('a');
        app.textarea.insert_char('b');

        let handled = handle_editing_command(&mut app, &Command::Backspace);
        assert!(handled);
        assert_eq!(app.textarea.content(), "a");
    }

    #[test]
    fn test_handle_delete_word_backward() {
        let mut app = create_test_app();
        app.focus = Focus::Input;
        app.textarea.set_content("hello world");
        // Move cursor to end
        app.textarea.move_cursor_end();

        let handled = handle_editing_command(&mut app, &Command::DeleteWordBackward);
        assert!(handled);
        assert!(!app.textarea.content().contains("world"));
    }

    #[test]
    fn test_handle_move_cursor() {
        let mut app = create_test_app();
        app.focus = Focus::Input;
        app.textarea.set_content("hello");
        app.textarea.move_cursor_end();
        let (_, initial_col) = app.textarea.cursor();

        let handled = handle_editing_command(&mut app, &Command::MoveCursorLeft);
        assert!(handled);
        let (_, new_col) = app.textarea.cursor();
        assert!(new_col < initial_col);
    }

    #[test]
    fn test_handle_history_up() {
        let mut app = create_test_app();
        app.focus = Focus::Input;
        // Add something to history first
        app.textarea.set_content("previous entry");
        app.input_history.add("previous entry".to_string());
        app.textarea.clear();

        let handled = handle_editing_command(&mut app, &Command::HistoryUp);
        assert!(handled);
        // History navigation was attempted
    }

    // Note: test_handle_submit_input requires async runtime for streaming
    // This behavior is tested in integration tests instead

    #[test]
    fn test_handle_paste() {
        let mut app = create_test_app();
        app.focus = Focus::Threads;

        let handled = handle_editing_command(&mut app, &Command::Paste("pasted text".to_string()));
        assert!(handled);
        assert_eq!(app.focus, Focus::Input);
        assert!(app.textarea.content().contains("pasted text"));
    }

    // =========================================================================
    // Cursor Blink Reset Tests
    // =========================================================================

    #[test]
    fn test_insert_char_resets_cursor_blink() {
        let mut app = create_test_app();
        app.focus = Focus::Input;

        // Move cursor to hidden phase
        app.cursor_blink.reset(0);
        app.tick_count = 50;
        app.cursor_blink.update(app.tick_count);
        assert!(!app.cursor_blink.is_visible(), "Setup: cursor should be hidden");

        // Insert a character
        handle_editing_command(&mut app, &Command::InsertChar('a'));

        // Cursor should be visible after reset
        assert!(app.cursor_blink.is_visible(), "Cursor should be visible after typing");
    }

    #[test]
    fn test_backspace_resets_cursor_blink() {
        let mut app = create_test_app();
        app.focus = Focus::Input;
        app.textarea.insert_char('x');

        // Move cursor to hidden phase
        app.cursor_blink.reset(0);
        app.tick_count = 50;
        app.cursor_blink.update(app.tick_count);
        assert!(!app.cursor_blink.is_visible(), "Setup: cursor should be hidden");

        // Backspace
        handle_editing_command(&mut app, &Command::Backspace);

        // Cursor should be visible after reset
        assert!(app.cursor_blink.is_visible(), "Cursor should be visible after backspace");
    }

    #[test]
    fn test_delete_char_resets_cursor_blink() {
        let mut app = create_test_app();
        app.focus = Focus::Input;
        app.textarea.insert_char('x');
        app.textarea.move_cursor_left();

        // Move cursor to hidden phase
        app.cursor_blink.reset(0);
        app.tick_count = 50;
        app.cursor_blink.update(app.tick_count);
        assert!(!app.cursor_blink.is_visible(), "Setup: cursor should be hidden");

        // Delete char
        handle_editing_command(&mut app, &Command::DeleteChar);

        // Cursor should be visible after reset
        assert!(app.cursor_blink.is_visible(), "Cursor should be visible after delete");
    }

    #[test]
    fn test_move_cursor_left_resets_blink() {
        let mut app = create_test_app();
        app.focus = Focus::Input;
        app.textarea.set_content("hello");
        app.textarea.move_cursor_end();

        // Move cursor to hidden phase
        app.cursor_blink.reset(0);
        app.tick_count = 50;
        app.cursor_blink.update(app.tick_count);
        assert!(!app.cursor_blink.is_visible(), "Setup: cursor should be hidden");

        // Move cursor
        handle_editing_command(&mut app, &Command::MoveCursorLeft);

        // Cursor should be visible after reset
        assert!(app.cursor_blink.is_visible(), "Cursor should be visible after cursor movement");
    }

    #[test]
    fn test_move_cursor_right_resets_blink() {
        let mut app = create_test_app();
        app.focus = Focus::Input;
        app.textarea.set_content("hello");

        // Move cursor to hidden phase
        app.cursor_blink.reset(0);
        app.tick_count = 50;
        app.cursor_blink.update(app.tick_count);
        assert!(!app.cursor_blink.is_visible(), "Setup: cursor should be hidden");

        // Move cursor
        handle_editing_command(&mut app, &Command::MoveCursorRight);

        // Cursor should be visible after reset
        assert!(app.cursor_blink.is_visible(), "Cursor should be visible after cursor movement");
    }

    #[test]
    fn test_paste_resets_cursor_blink() {
        let mut app = create_test_app();
        app.focus = Focus::Input;

        // Move cursor to hidden phase
        app.cursor_blink.reset(0);
        app.tick_count = 50;
        app.cursor_blink.update(app.tick_count);
        assert!(!app.cursor_blink.is_visible(), "Setup: cursor should be hidden");

        // Paste text
        handle_editing_command(&mut app, &Command::Paste("text".to_string()));

        // Cursor should be visible after reset
        assert!(app.cursor_blink.is_visible(), "Cursor should be visible after paste");
    }

    #[test]
    fn test_history_up_resets_cursor_blink() {
        let mut app = create_test_app();
        app.focus = Focus::Input;
        app.input_history.add("previous".to_string());

        // Move cursor to hidden phase
        app.cursor_blink.reset(0);
        app.tick_count = 50;
        app.cursor_blink.update(app.tick_count);
        assert!(!app.cursor_blink.is_visible(), "Setup: cursor should be hidden");

        // Navigate history
        handle_editing_command(&mut app, &Command::HistoryUp);

        // Cursor should be visible after reset
        assert!(app.cursor_blink.is_visible(), "Cursor should be visible after history navigation");
    }

    #[test]
    fn test_history_down_resets_cursor_blink() {
        let mut app = create_test_app();
        app.focus = Focus::Input;
        app.input_history.add("previous".to_string());
        app.input_history.navigate_up(""); // Move up in history

        // Move cursor to hidden phase
        app.cursor_blink.reset(0);
        app.tick_count = 50;
        app.cursor_blink.update(app.tick_count);
        assert!(!app.cursor_blink.is_visible(), "Setup: cursor should be hidden");

        // Navigate history down
        handle_editing_command(&mut app, &Command::HistoryDown);

        // Cursor should be visible after reset
        assert!(app.cursor_blink.is_visible(), "Cursor should be visible after history navigation");
    }
}
