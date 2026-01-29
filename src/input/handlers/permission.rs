//! Permission command handlers.
//!
//! Handles commands related to permission prompts, question dialogs,
//! folder picker, and thread switcher modals.

use crate::app::{App, Screen};
use crate::input::Command;

/// Gets the current thread ID for permission handling.
///
/// The thread ID is determined based on the current screen and context:
/// - On CommandDeck with an overlay: uses the overlay's thread_id
/// - On Conversation screen: uses the active_thread_id
/// - Otherwise: returns None (no permission context)
fn get_current_thread_id(app: &App) -> Option<String> {
    if app.screen == Screen::CommandDeck {
        // On CommandDeck, check for overlay first (takes priority)
        if let Some(overlay) = app.dashboard.overlay() {
            return Some(overlay.thread_id().to_string());
        }
        // No overlay - no permission context on CommandDeck
        None
    } else {
        // On Conversation screen, use active_thread_id
        app.active_thread_id.clone()
    }
}

/// Handles permission-related commands.
///
/// Uses thread context to look up pending permissions from DashboardState.
/// The permission is looked up by thread_id, but the response is sent using
/// the permission_id which uniquely identifies the request.
///
/// Returns `true` if the command was handled successfully.
pub fn handle_permission_command(app: &mut App, cmd: &Command) -> bool {
    match cmd {
        Command::ApprovePermission => {
            if let Some(thread_id) = get_current_thread_id(app) {
                if let Some(perm) = app.dashboard.get_pending_permission(&thread_id).cloned() {
                    app.approve_permission(&perm.permission_id);
                    return true;
                }
            }
            false
        }

        Command::DenyPermission => {
            if let Some(thread_id) = get_current_thread_id(app) {
                if let Some(perm) = app.dashboard.get_pending_permission(&thread_id).cloned() {
                    app.deny_permission(&perm.permission_id);
                    return true;
                }
            }
            false
        }

        Command::AlwaysAllowPermission => {
            if let Some(thread_id) = get_current_thread_id(app) {
                if let Some(perm) = app.dashboard.get_pending_permission(&thread_id).cloned() {
                    // Use allow_tool_always which adds to allowed list and approves
                    app.allow_tool_always(&perm.tool_name, &perm.permission_id);
                    return true;
                }
            }
            false
        }

        Command::HandlePermissionKey(c) => {
            // Handle permission key press for the current thread context
            if let Some(thread_id) = get_current_thread_id(app) {
                if let Some(perm) = app.dashboard.get_pending_permission(&thread_id).cloned() {
                    match c {
                        'y' | 'Y' => {
                            app.approve_permission(&perm.permission_id);
                            true
                        }
                        'a' | 'A' => {
                            app.allow_tool_always(&perm.tool_name, &perm.permission_id);
                            true
                        }
                        'n' | 'N' => {
                            app.deny_permission(&perm.permission_id);
                            true
                        }
                        _ => false,
                    }
                } else {
                    false
                }
            } else {
                false
            }
        }

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

/// Handles file picker-related commands (Conversation screen).
///
/// Returns `true` if the command was handled successfully.
pub fn handle_file_picker_command(app: &mut App, cmd: &Command) -> bool {
    match cmd {
        Command::CloseFilePicker => {
            // Remove the @ and any filter text from input
            app.remove_at_and_filter_from_input_file_picker();
            app.cancel_file_picker();
            true
        }

        Command::FilePickerConfirm => {
            // If on a directory, navigate into it
            if let Some(item) = app.file_picker.selected_item() {
                if item.is_dir {
                    if item.name == ".." {
                        app.file_picker.navigate_up();
                        // Load files from parent directory
                        let path = app.file_picker.current_path_str();
                        app.load_files(&path);
                        app.mark_dirty();
                    } else {
                        let dir_name = item.name.clone();
                        app.file_picker.navigate_into(&dir_name);
                        // Load files from new directory
                        let path = app.file_picker.current_path_str();
                        app.load_files(&path);
                        app.mark_dirty();
                    }
                    return true;
                }
            }

            // If we have selected files or cursor on a file, confirm selection
            app.confirm_file_picker_selection();
            true
        }

        Command::FilePickerTypeChar(c) => {
            // Add to query filter
            let mut query = app.file_picker.query.clone();
            query.push(*c);
            app.file_picker.set_query(query);
            app.mark_dirty();
            true
        }

        Command::FilePickerBackspace => {
            if app.file_picker.query.is_empty() {
                // Close picker when backspacing with empty query
                app.textarea.backspace(); // Remove the @
                app.cancel_file_picker();
            } else {
                // Remove last character from query
                let mut query = app.file_picker.query.clone();
                query.pop();
                app.file_picker.set_query(query);
            }
            app.mark_dirty();
            true
        }

        Command::FilePickerCursorUp => {
            app.file_picker.move_up();
            app.mark_dirty();
            true
        }

        Command::FilePickerCursorDown => {
            app.file_picker.move_down();
            app.mark_dirty();
            true
        }

        Command::FilePickerToggleSelect => {
            app.file_picker.toggle_selection();
            app.mark_dirty();
            true
        }

        Command::FilePickerNavigateIn => {
            // Navigate into directory if selected item is a directory
            if let Some(item) = app.file_picker.selected_item() {
                if item.is_dir && item.name != ".." {
                    let dir_name = item.name.clone();
                    app.file_picker.navigate_into(&dir_name);
                    // Load files from new directory
                    let path = app.file_picker.current_path_str();
                    app.load_files(&path);
                    app.mark_dirty();
                }
            }
            true
        }

        Command::FilePickerNavigateUp => {
            if app.file_picker.can_go_up() {
                app.file_picker.navigate_up();
                // Load files from parent directory
                let path = app.file_picker.current_path_str();
                app.load_files(&path);
                app.mark_dirty();
            }
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
                // Clone the command before modifying app state
                let cmd_to_execute = command.clone();

                // Close autocomplete and clear input
                app.remove_slash_and_query_from_input();
                app.textarea.clear();
                app.slash_autocomplete_visible = false;
                app.slash_autocomplete_query.clear();
                app.slash_autocomplete_cursor = 0;

                // Execute the command immediately (no second Enter needed)
                tracing::info!("SelectSlashCommand: executing {:?}", cmd_to_execute);
                app.execute_slash_command(cmd_to_execute);
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

/// Handles Claude login dialog commands.
///
/// Returns `true` if the command was handled successfully.
pub fn handle_claude_login_command(app: &mut App, cmd: &Command) -> bool {
    use crate::view_state::{ClaudeLoginState, OverlayState};

    // Check that we have a Claude login overlay
    let overlay = match app.dashboard.overlay() {
        Some(OverlayState::ClaudeLogin { state, .. }) => state.clone(),
        _ => return false,
    };

    match cmd {
        Command::ClaudeLoginOpenBrowser => {
            // Open browser and update state
            if let Some(auth_url) = app.dashboard.claude_login_auth_url() {
                let _ = open::that(auth_url);
                app.dashboard
                    .update_claude_login_state(ClaudeLoginState::ShowingUrl {
                        browser_opened: true,
                    });
            }
            true
        }

        Command::ClaudeLoginDone => {
            // Only allow Done from ShowingUrl state (either state)
            if matches!(overlay, ClaudeLoginState::ShowingUrl { .. }) {
                // Update state to Verifying
                app.dashboard
                    .update_claude_login_state(ClaudeLoginState::Verifying);

                // Send response to backend
                if let Some(request_id) = app.dashboard.claude_login_request_id() {
                    app.send_claude_login_response(request_id.to_string(), true);
                }
            }
            true
        }

        Command::ClaudeLoginCancel => {
            // Allow cancel from ShowingUrl or VerificationFailed states
            match overlay {
                ClaudeLoginState::ShowingUrl { .. }
                | ClaudeLoginState::VerificationFailed { .. } => {
                    // Send cancel response to backend
                    if let Some(request_id) = app.dashboard.claude_login_request_id() {
                        app.send_claude_login_response(request_id.to_string(), false);
                    }
                    // Close overlay
                    app.dashboard.collapse_overlay();
                }
                ClaudeLoginState::VerificationSuccess { .. } => {
                    // Just close overlay (success state - don't send cancel)
                    app.dashboard.collapse_overlay();
                }
                ClaudeLoginState::Verifying => {
                    // Can't cancel while verifying - do nothing
                }
            }
            true
        }

        Command::ClaudeLoginRetry => {
            // Only allow retry from VerificationFailed state
            if matches!(overlay, ClaudeLoginState::VerificationFailed { .. }) {
                // Reset to ShowingUrl state with browser already opened
                app.dashboard
                    .update_claude_login_state(ClaudeLoginState::ShowingUrl {
                        browser_opened: true,
                    });
            }
            true
        }

        _ => false,
    }
}

/// Handles plan approval-related commands (scroll, approve, reject).
///
/// Returns `true` if the command was handled successfully.
pub fn handle_plan_approval_command(app: &mut App, cmd: &Command) -> bool {
    // Get current thread for plan approval
    let thread_id = match &app.active_thread_id {
        Some(id) => id.clone(),
        None => return false,
    };

    // Verify there's a plan pending for this thread
    if app.dashboard.get_plan_request(&thread_id).is_none() {
        return false;
    }

    match cmd {
        Command::PlanScrollUp => {
            // Scroll up in conversation to see plan content above
            app.scroll_velocity = 0.0;
            app.user_has_scrolled = true;
            let new_scroll = (app.unified_scroll + 3).min(app.max_scroll);
            if new_scroll != app.unified_scroll {
                app.unified_scroll = new_scroll;
                app.scroll_position = app.unified_scroll as f32;
                app.mark_dirty();
            }
            true
        }

        Command::PlanScrollDown => {
            // Scroll down in conversation
            app.scroll_velocity = 0.0;
            if app.unified_scroll >= 3 {
                app.unified_scroll -= 3;
                app.scroll_position = app.unified_scroll as f32;
                if app.unified_scroll == 0 {
                    app.user_has_scrolled = false;
                }
                app.mark_dirty();
            } else if app.unified_scroll > 0 {
                app.unified_scroll = 0;
                app.scroll_position = 0.0;
                app.user_has_scrolled = false;
                app.mark_dirty();
            }
            true
        }

        Command::ApprovePlan => {
            // Approve plan using existing permission key handler
            // It will check for plan approval after finding no pending permission
            app.handle_permission_key('y')
        }

        Command::RejectPlan => {
            // Reject plan using existing permission key handler
            // It will check for plan approval after finding no pending permission
            app.handle_permission_key('n')
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

    // ============= Thread-Aware Permission Tests =============

    #[test]
    fn test_get_current_thread_id_conversation_screen() {
        let mut app = create_test_app();
        app.screen = Screen::Conversation;
        app.active_thread_id = Some("test-thread-123".to_string());

        let thread_id = get_current_thread_id(&app);
        assert_eq!(thread_id, Some("test-thread-123".to_string()));
    }

    #[test]
    fn test_get_current_thread_id_conversation_no_active() {
        let mut app = create_test_app();
        app.screen = Screen::Conversation;
        app.active_thread_id = None;

        let thread_id = get_current_thread_id(&app);
        assert_eq!(thread_id, None);
    }

    #[test]
    fn test_get_current_thread_id_commanddeck_no_overlay() {
        let mut app = create_test_app();
        app.screen = Screen::CommandDeck;

        let thread_id = get_current_thread_id(&app);
        assert_eq!(thread_id, None);
    }

    #[test]
    fn test_handle_permission_no_thread_context() {
        let mut app = create_test_app();
        app.screen = Screen::CommandDeck;
        // No overlay, no active_thread_id

        let handled = handle_permission_command(&mut app, &Command::ApprovePermission);
        assert!(!handled);
    }

    #[test]
    fn test_handle_permission_no_pending_permission() {
        let mut app = create_test_app();
        app.screen = Screen::Conversation;
        app.active_thread_id = Some("test-thread".to_string());
        // No pending permission for this thread

        let handled = handle_permission_command(&mut app, &Command::ApprovePermission);
        assert!(!handled);
    }

    #[test]
    fn test_handle_permission_key_no_thread_context() {
        let mut app = create_test_app();
        app.screen = Screen::CommandDeck;
        // No overlay

        let handled = handle_permission_command(&mut app, &Command::HandlePermissionKey('y'));
        assert!(!handled);
    }

    #[test]
    fn test_handle_permission_key_invalid_key() {
        use crate::state::PermissionRequest;
        use std::time::Instant;

        let mut app = create_test_app();
        app.screen = Screen::Conversation;
        app.active_thread_id = Some("test-thread".to_string());

        // Set up a pending permission for this thread
        app.dashboard.set_pending_permission(
            "test-thread",
            PermissionRequest {
                permission_id: "perm-123".to_string(),
                thread_id: Some("test-thread".to_string()),
                tool_name: "Bash".to_string(),
                description: "Run command".to_string(),
                context: None,
                tool_input: None,
                received_at: Instant::now(),
            },
        );

        // Invalid key should not handle
        let handled = handle_permission_command(&mut app, &Command::HandlePermissionKey('x'));
        assert!(!handled);

        // Permission should still be pending
        assert!(app.dashboard.get_pending_permission("test-thread").is_some());
    }
}
