//! Permission command handlers.
//!
//! Handles commands related to permission prompts, question dialogs,
//! folder picker, and thread switcher modals.

use crate::app::App;
use crate::input::Command;

/// Handles permission-related commands.
///
/// Returns `true` if the command was handled successfully.
pub fn handle_permission_command(app: &mut App, cmd: &Command) -> bool {
    match cmd {
        Command::ApprovePermission => {
            if let Some(ref perm) = app.session_state.pending_permission.clone() {
                app.approve_permission(&perm.permission_id);
                true
            } else {
                false
            }
        }

        Command::DenyPermission => {
            if let Some(ref perm) = app.session_state.pending_permission.clone() {
                app.deny_permission(&perm.permission_id);
                true
            } else {
                false
            }
        }

        Command::AlwaysAllowPermission => {
            if let Some(ref perm) = app.session_state.pending_permission.clone() {
                // Use allow_tool_always which adds to allowed list and approves
                app.allow_tool_always(&perm.tool_name, &perm.permission_id);
                true
            } else {
                false
            }
        }

        Command::HandlePermissionKey(c) => app.handle_permission_key(*c),

        _ => false,
    }
}

/// Handles question/AskUserQuestion-related commands (session-level, inline).
///
/// Returns `true` if the command was handled successfully.
pub fn handle_question_command(app: &mut App, cmd: &Command) -> bool {
    match cmd {
        Command::QuestionNextTab => {
            app.question_next_tab();
            true
        }

        Command::QuestionPrevOption => {
            app.question_prev_option();
            true
        }

        Command::QuestionNextOption => {
            app.question_next_option();
            true
        }

        Command::QuestionToggleOption => {
            app.question_toggle_option();
            true
        }

        Command::QuestionConfirm => {
            app.question_confirm();
            true
        }

        Command::QuestionCancelOther => {
            app.question_cancel_other();
            true
        }

        Command::QuestionBackspace => {
            app.question_backspace();
            true
        }

        Command::QuestionTypeChar(c) => {
            app.question_type_char(*c);
            true
        }

        _ => false,
    }
}

/// Handles dashboard question overlay commands.
///
/// These are for the overlay-based question dialogs on the CommandDeck,
/// which use `app.dashboard` state instead of session-level state.
///
/// Returns `true` if the command was handled successfully.
pub fn handle_dashboard_question_command(app: &mut App, cmd: &Command) -> bool {
    match cmd {
        Command::DashboardQuestionNextTab => {
            app.dashboard.question_next_tab();
            true
        }

        Command::DashboardQuestionPrevOption => {
            app.dashboard.question_prev_option();
            true
        }

        Command::DashboardQuestionNextOption => {
            app.dashboard.question_next_option();
            true
        }

        Command::DashboardQuestionToggleOption => {
            app.dashboard.question_toggle_option();
            true
        }

        Command::DashboardQuestionConfirm => {
            if let Some((thread_id, request_id, answers)) = app.dashboard.question_confirm() {
                app.submit_dashboard_question(&thread_id, &request_id, answers);
            }
            true
        }

        Command::DashboardQuestionClose => {
            app.dashboard.collapse_overlay();
            true
        }

        Command::DashboardQuestionCancelOther => {
            app.dashboard.question_cancel_other();
            true
        }

        Command::DashboardQuestionBackspace => {
            app.dashboard.question_backspace();
            true
        }

        Command::DashboardQuestionTypeChar(c) => {
            app.dashboard.question_type_char(*c);
            true
        }

        _ => false,
    }
}

/// Handles folder picker-related commands.
///
/// Returns `true` if the command was handled successfully.
pub fn handle_folder_picker_command(app: &mut App, cmd: &Command) -> bool {
    match cmd {
        Command::OpenFolderPicker => {
            app.open_folder_picker();
            true
        }

        Command::CloseFolderPicker => {
            app.remove_at_and_filter_from_input();
            app.close_folder_picker();
            true
        }

        Command::SelectFolder => {
            app.remove_at_and_filter_from_input();
            app.folder_picker_select();
            true
        }

        Command::FolderPickerTypeChar(c) => {
            app.folder_picker_type_char(*c);
            true
        }

        Command::FolderPickerBackspace => {
            if app.folder_picker_backspace() {
                // Filter was empty, close picker and remove @
                app.textarea.backspace();
                app.close_folder_picker();
            }
            true
        }

        Command::FolderPickerCursorUp => {
            app.folder_picker_cursor_up();
            true
        }

        Command::FolderPickerCursorDown => {
            app.folder_picker_cursor_down();
            true
        }

        _ => false,
    }
}

/// Handles slash command autocomplete-related commands.
///
/// Returns `true` if the command was handled successfully.
pub fn handle_slash_autocomplete_command(app: &mut App, cmd: &Command) -> bool {
    match cmd {
        Command::OpenSlashAutocomplete => {
            app.slash_autocomplete_visible = true;
            app.slash_autocomplete_query.clear();
            app.slash_autocomplete_cursor = 0;
            app.mark_dirty();
            true
        }

        Command::CloseSlashAutocomplete => {
            app.remove_slash_and_query_from_input();
            app.slash_autocomplete_visible = false;
            app.slash_autocomplete_query.clear();
            app.slash_autocomplete_cursor = 0;
            app.mark_dirty();
            true
        }

        Command::SelectSlashCommand => {
            let filtered = app.filtered_slash_commands();
            if let Some(command) = filtered.get(app.slash_autocomplete_cursor) {
                // Replace the / and query with the selected command name
                app.remove_slash_and_query_from_input();
                let command_text = command.name();
                for ch in command_text.chars() {
                    app.textarea.insert_char(ch);
                }
                app.slash_autocomplete_visible = false;
                app.slash_autocomplete_query.clear();
                app.slash_autocomplete_cursor = 0;
                app.mark_dirty();
            }
            true
        }

        Command::SlashAutocompleteTypeChar(c) => {
            app.slash_autocomplete_query.push(*c);
            app.slash_autocomplete_cursor = 0; // Reset cursor when query changes
            app.mark_dirty();
            true
        }

        Command::SlashAutocompleteBackspace => {
            if app.slash_autocomplete_query.is_empty() {
                // Close autocomplete when backspacing with empty query
                app.textarea.backspace(); // Remove the /
                app.slash_autocomplete_visible = false;
                app.slash_autocomplete_cursor = 0;
            } else {
                // Remove last character from query
                app.slash_autocomplete_query.pop();
                app.slash_autocomplete_cursor = 0;
            }
            app.mark_dirty();
            true
        }

        Command::SlashAutocompleteCursorUp => {
            if app.slash_autocomplete_cursor > 0 {
                app.slash_autocomplete_cursor -= 1;
                app.mark_dirty();
            }
            true
        }

        Command::SlashAutocompleteCursorDown => {
            let filtered_count = app.filtered_slash_commands().len();
            if filtered_count > 0 && app.slash_autocomplete_cursor < filtered_count - 1 {
                app.slash_autocomplete_cursor += 1;
                app.mark_dirty();
            }
            true
        }

        _ => false,
    }
}

/// Handles thread switcher-related commands.
///
/// Returns `true` if the command was handled successfully.
pub fn handle_thread_switcher_command(app: &mut App, cmd: &Command) -> bool {
    match cmd {
        Command::CycleSwitcherForward => {
            app.cycle_switcher_forward();
            true
        }

        Command::CycleSwitcherBackward => {
            app.cycle_switcher_backward();
            true
        }

        Command::CloseSwitcher => {
            app.close_switcher();
            true
        }

        Command::ConfirmSwitcherSelection => {
            app.confirm_switcher_selection();
            true
        }

        _ => false,
    }
}

/// Handles miscellaneous modal/dialog commands.
///
/// Returns `true` if the command was handled successfully.
pub fn handle_misc_command(app: &mut App, cmd: &Command) -> bool {
    match cmd {
        Command::DismissError => {
            if app.has_errors() {
                app.dismiss_focused_error();
                true
            } else {
                false
            }
        }

        Command::ToggleReasoning => {
            app.toggle_reasoning();
            true
        }

        Command::OpenOAuthUrl => {
            if let Some(url) = &app.session_state.oauth_url {
                let _ = open::that(url);
                true
            } else {
                false
            }
        }

        Command::CreateNewThread => {
            app.create_new_thread();
            true
        }

        Command::Quit | Command::ForceQuit => {
            app.quit();
            true
        }

        Command::Tick => {
            app.tick();
            app.check_switcher_timeout();
            true
        }

        Command::Resize { width, height } => {
            app.update_terminal_dimensions(*width, *height);
            true
        }

        Command::Noop => {
            // No operation, but still considered "handled"
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
    fn test_handle_dismiss_error_with_errors() {
        let mut app = create_test_app();
        // Set up an active thread and add an error to it
        app.active_thread_id = Some("test-thread".to_string());
        app.cache.add_error_simple(
            "test-thread",
            "test_error".to_string(),
            "Test message".to_string(),
        );

        let handled = handle_misc_command(&mut app, &Command::DismissError);
        assert!(handled);
    }

    #[test]
    fn test_handle_dismiss_error_without_errors() {
        let mut app = create_test_app();
        app.stream_error = None;

        let handled = handle_misc_command(&mut app, &Command::DismissError);
        assert!(!handled);
    }

    #[test]
    fn test_handle_toggle_reasoning() {
        let mut app = create_test_app();

        let handled = handle_misc_command(&mut app, &Command::ToggleReasoning);
        assert!(handled);
    }

    #[test]
    fn test_handle_create_new_thread() {
        let mut app = create_test_app();

        let handled = handle_misc_command(&mut app, &Command::CreateNewThread);
        assert!(handled);
    }

    #[test]
    fn test_handle_quit() {
        let mut app = create_test_app();
        assert!(!app.should_quit);

        let handled = handle_misc_command(&mut app, &Command::Quit);
        assert!(handled);
        assert!(app.should_quit);
    }

    #[test]
    fn test_handle_tick() {
        let mut app = create_test_app();
        let initial_tick = app.tick_count;

        let handled = handle_misc_command(&mut app, &Command::Tick);
        assert!(handled);
        assert!(app.tick_count > initial_tick);
    }

    #[test]
    fn test_handle_resize() {
        let mut app = create_test_app();

        let handled = handle_misc_command(
            &mut app,
            &Command::Resize {
                width: 120,
                height: 40,
            },
        );
        assert!(handled);
        assert_eq!(app.terminal_width, 120);
        assert_eq!(app.terminal_height, 40);
    }

    #[test]
    fn test_handle_noop() {
        let mut app = create_test_app();

        let handled = handle_misc_command(&mut app, &Command::Noop);
        assert!(handled);
    }

    #[test]
    fn test_folder_picker_close() {
        let mut app = create_test_app();
        app.folder_picker_visible = true;

        let handled = handle_folder_picker_command(&mut app, &Command::CloseFolderPicker);
        assert!(handled);
        assert!(!app.folder_picker_visible);
    }

    #[test]
    fn test_thread_switcher_close() {
        let mut app = create_test_app();
        app.thread_switcher.visible = true;

        let handled = handle_thread_switcher_command(&mut app, &Command::CloseSwitcher);
        assert!(handled);
        assert!(!app.thread_switcher.visible);
    }

    #[test]
    fn test_thread_switcher_cycle_forward() {
        let mut app = create_test_app();
        app.thread_switcher.visible = true;

        let handled = handle_thread_switcher_command(&mut app, &Command::CycleSwitcherForward);
        assert!(handled);
    }
}
