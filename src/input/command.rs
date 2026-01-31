//! Command definitions for keyboard input handling.
//!
//! This module defines all commands that can be triggered by keyboard input.
//! The [`Command`] enum provides a unified way to represent user actions,
//! decoupling key bindings from their effects.

use crate::models::ThreadType;

/// Represents all possible commands that can be triggered by keyboard input.
///
/// Commands are organized into categories:
/// - Global commands (quit, ctrl+c, new thread)
/// - Navigation commands (scroll, move, focus)
/// - Input/editing commands (character input, cursor movement, text manipulation)
/// - Permission commands (approve, deny, always allow)
/// - Modal commands (folder picker, thread switcher, question prompts)
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    // =========================================================================
    // Global Commands (always active)
    // =========================================================================
    /// Quit the application (Ctrl+C)
    Quit,
    /// Force quit (same as Quit, but explicit)
    ForceQuit,
    /// Navigate back to CommandDeck from Conversation (Shift+Esc, Ctrl+W)
    NavigateToCommandDeck,
    /// Create a new thread (Shift+N, Ctrl+N)
    CreateNewThread,
    /// Submit input as programming thread (Alt+P)
    SubmitAsProgramming,

    // =========================================================================
    // Screen/Focus Navigation
    // =========================================================================
    /// Move focus up (Up arrow in threads list)
    MoveUp,
    /// Move focus down (Down arrow in threads list)
    MoveDown,
    /// Open selected thread (Enter on threads)
    OpenSelectedThread,
    /// Cycle focus between panels (Tab)
    CycleFocus,
    /// Double-tap Tab to open thread switcher
    HandleTabPress,
    /// Cycle permission mode (Shift+Tab)
    CyclePermissionMode,

    // =========================================================================
    // Scroll Commands
    // =========================================================================
    /// Scroll page up in conversation
    ScrollPageUp,
    /// Scroll page down in conversation
    ScrollPageDown,
    /// Scroll up by specified lines (mouse scroll)
    ScrollUp(usize),
    /// Scroll down by specified lines (mouse scroll)
    ScrollDown(usize),

    // =========================================================================
    // Input/Editing Commands
    // =========================================================================
    /// Insert a character at cursor position
    InsertChar(char),
    /// Insert a newline (Shift+Enter, Alt+Enter, Ctrl+Enter, Ctrl+J)
    InsertNewline,
    /// Delete character before cursor (Backspace)
    Backspace,
    /// Delete character at cursor (Delete)
    DeleteChar,
    /// Delete word backward (Alt+Backspace)
    DeleteWordBackward,
    /// Delete to line start (Cmd+Backspace, Ctrl+U)
    DeleteToLineStart,
    /// Move cursor left
    MoveCursorLeft,
    /// Move cursor right
    MoveCursorRight,
    /// Move cursor up (or navigate history if on first line)
    MoveCursorUp,
    /// Move cursor down (or navigate history if on last line)
    MoveCursorDown,
    /// Move cursor to line start (Home, Cmd+Left)
    MoveCursorHome,
    /// Move cursor to line end (End, Cmd+Right)
    MoveCursorEnd,
    /// Move cursor word left (Alt+Left)
    MoveCursorWordLeft,
    /// Move cursor word right (Alt+Right)
    MoveCursorWordRight,
    /// Navigate input history up
    HistoryUp,
    /// Navigate input history down
    HistoryDown,
    /// Submit input with specified thread type (Enter)
    SubmitInput(ThreadType),
    /// Unfocus input (Escape when in input)
    UnfocusInput,
    /// Handle paste event
    Paste(String),

    // =========================================================================
    // Folder Picker Commands
    // =========================================================================
    /// Open folder picker (@ trigger)
    OpenFolderPicker,
    /// Close folder picker (Escape)
    CloseFolderPicker,
    /// Select folder from picker (Enter)
    SelectFolder,
    /// Type character in folder filter
    FolderPickerTypeChar(char),
    /// Backspace in folder filter
    FolderPickerBackspace,
    /// Move folder picker cursor up
    FolderPickerCursorUp,
    /// Move folder picker cursor down
    FolderPickerCursorDown,

    // =========================================================================
    // File Picker Commands (Conversation screen)
    // =========================================================================
    /// Close file picker (Escape)
    CloseFilePicker,
    /// Confirm file selection (Enter)
    FilePickerConfirm,
    /// Type character in file picker filter
    FilePickerTypeChar(char),
    /// Backspace in file picker filter
    FilePickerBackspace,
    /// Move file picker cursor up
    FilePickerCursorUp,
    /// Move file picker cursor down
    FilePickerCursorDown,
    /// Toggle file selection (Tab)
    FilePickerToggleSelect,
    /// Navigate into directory (Right arrow)
    FilePickerNavigateIn,
    /// Navigate to parent directory (Left arrow)
    FilePickerNavigateUp,

    // =========================================================================
    // Slash Command Autocomplete Commands
    // =========================================================================
    /// Open slash command autocomplete (/ trigger)
    OpenSlashAutocomplete,
    /// Close slash command autocomplete (Escape)
    CloseSlashAutocomplete,
    /// Select command from autocomplete (Enter)
    SelectSlashCommand,
    /// Type character in slash autocomplete query
    SlashAutocompleteTypeChar(char),
    /// Backspace in slash autocomplete query
    SlashAutocompleteBackspace,
    /// Move slash autocomplete cursor up
    SlashAutocompleteCursorUp,
    /// Move slash autocomplete cursor down
    SlashAutocompleteCursorDown,

    // =========================================================================
    // Thread Switcher Commands
    // =========================================================================
    /// Cycle thread switcher forward (Tab, Down)
    CycleSwitcherForward,
    /// Cycle thread switcher backward (Up)
    CycleSwitcherBackward,
    /// Close thread switcher (Escape)
    CloseSwitcher,
    /// Confirm thread switcher selection (Enter or any other key)
    ConfirmSwitcherSelection,

    // =========================================================================
    // Permission Commands
    // =========================================================================
    /// Approve permission (Y/y)
    ApprovePermission,
    /// Deny permission (N/n)
    DenyPermission,
    /// Always allow permission (A/a)
    AlwaysAllowPermission,
    /// Handle permission key (generic, will check Y/N/A)
    HandlePermissionKey(char),

    // =========================================================================
    // Question/AskUser Commands (Session-level, inline)
    // =========================================================================
    /// Next tab in question UI
    QuestionNextTab,
    /// Previous option in question UI
    QuestionPrevOption,
    /// Next option in question UI
    QuestionNextOption,
    /// Toggle option selection (Space)
    QuestionToggleOption,
    /// Confirm question response (Enter)
    QuestionConfirm,
    /// Cancel "Other" text input (Escape)
    QuestionCancelOther,
    /// Backspace in "Other" text input
    QuestionBackspace,
    /// Type char in "Other" text input
    QuestionTypeChar(char),

    // =========================================================================
    // Dashboard Question Overlay Commands
    // =========================================================================
    /// Next tab in dashboard question overlay
    DashboardQuestionNextTab,
    /// Previous option in dashboard question overlay
    DashboardQuestionPrevOption,
    /// Next option in dashboard question overlay
    DashboardQuestionNextOption,
    /// Toggle option in dashboard question overlay (Space)
    DashboardQuestionToggleOption,
    /// Confirm selection in dashboard question overlay (Enter)
    DashboardQuestionConfirm,
    /// Close dashboard question overlay (Escape)
    DashboardQuestionClose,
    /// Cancel "Other" text input in dashboard overlay (Escape)
    DashboardQuestionCancelOther,
    /// Backspace in dashboard overlay "Other" text input
    DashboardQuestionBackspace,
    /// Type char in dashboard overlay "Other" text input
    DashboardQuestionTypeChar(char),

    // =========================================================================
    // Plan Approval Commands
    // =========================================================================
    /// Scroll plan content up (see more of plan above)
    PlanScrollUp,
    /// Scroll plan content down (see more of plan below)
    PlanScrollDown,
    /// Approve plan (Y/y when plan approval is pending)
    ApprovePlan,
    /// Reject plan (N/n when plan approval is pending)
    RejectPlan,

    // =========================================================================
    // Rate Limit Commands
    // =========================================================================
    /// Continue with next account (Y/y when rate limit modal is showing)
    ContinueWithNextAccount,
    /// Cancel rate limit retry (N/n or Esc when rate limit modal is showing)
    CancelRateLimitRetry,

    // =========================================================================
    // Claude Login Commands
    // =========================================================================
    /// Open browser for Claude login (Enter)
    ClaudeLoginOpenBrowser,
    /// Mark Claude login as done (D/d)
    ClaudeLoginDone,
    /// Cancel Claude login (Escape)
    ClaudeLoginCancel,
    /// Retry Claude login after failure (R/r)
    ClaudeLoginRetry,

    // =========================================================================
    // Claude Accounts Commands
    // =========================================================================
    /// Add new Claude account (A/a)
    ClaudeAccountsAdd,
    /// Remove selected account (R/r)
    ClaudeAccountsRemove,
    /// Close Claude accounts overlay (Escape)
    ClaudeAccountsClose,
    /// Move selection up
    ClaudeAccountsMoveUp,
    /// Move selection down
    ClaudeAccountsMoveDown,

    // =========================================================================
    // VPS Config Commands (/vps overlay)
    // =========================================================================
    /// Move to next field (Tab/Down)
    VpsConfigNextField,
    /// Move to previous field (Shift+Tab/Up)
    VpsConfigPrevField,
    /// Type a character in the focused field
    VpsConfigTypeChar(char),
    /// Backspace in the focused field
    VpsConfigBackspace,
    /// Submit the form (Enter)
    VpsConfigSubmit,
    /// Toggle VPS config mode (Left/Right arrow on mode field)
    VpsConfigToggleMode,
    /// Close the VPS config overlay (Escape)
    VpsConfigClose,

    // =========================================================================
    // Conversation Screen Commands
    // =========================================================================
    /// Dismiss focused error (d key)
    DismissError,
    /// Toggle reasoning/thinking block (t key)
    ToggleReasoning,
    /// Open OAuth URL in browser (o key)
    OpenOAuthUrl,

    // =========================================================================
    // System Commands
    // =========================================================================
    /// Handle terminal resize
    Resize { width: u16, height: u16 },
    /// Tick for animations
    Tick,
    /// No operation (used when key should be ignored)
    Noop,
}

impl Command {
    /// Returns true if this command should mark the app as dirty (needs redraw).
    pub fn marks_dirty(&self) -> bool {
        !matches!(self, Command::Noop | Command::Tick)
    }

    /// Returns true if this command is a quit command.
    pub fn is_quit(&self) -> bool {
        matches!(self, Command::Quit | Command::ForceQuit)
    }

    /// Returns a human-readable description of the command.
    pub fn description(&self) -> &'static str {
        match self {
            Command::Quit => "Quit application",
            Command::ForceQuit => "Force quit application",
            Command::NavigateToCommandDeck => "Return to command deck",
            Command::CreateNewThread => "Create new thread",
            Command::SubmitAsProgramming => "Submit as programming thread",
            Command::MoveUp => "Move selection up",
            Command::MoveDown => "Move selection down",
            Command::OpenSelectedThread => "Open selected thread",
            Command::CycleFocus => "Cycle focus",
            Command::HandleTabPress => "Handle tab press",
            Command::CyclePermissionMode => "Cycle permission mode",
            Command::ScrollPageUp => "Scroll page up",
            Command::ScrollPageDown => "Scroll page down",
            Command::ScrollUp(_) => "Scroll up",
            Command::ScrollDown(_) => "Scroll down",
            Command::InsertChar(_) => "Insert character",
            Command::InsertNewline => "Insert newline",
            Command::Backspace => "Delete previous character",
            Command::DeleteChar => "Delete character",
            Command::DeleteWordBackward => "Delete word backward",
            Command::DeleteToLineStart => "Delete to line start",
            Command::MoveCursorLeft => "Move cursor left",
            Command::MoveCursorRight => "Move cursor right",
            Command::MoveCursorUp => "Move cursor up",
            Command::MoveCursorDown => "Move cursor down",
            Command::MoveCursorHome => "Move cursor to start",
            Command::MoveCursorEnd => "Move cursor to end",
            Command::MoveCursorWordLeft => "Move cursor word left",
            Command::MoveCursorWordRight => "Move cursor word right",
            Command::HistoryUp => "Previous history entry",
            Command::HistoryDown => "Next history entry",
            Command::SubmitInput(_) => "Submit input",
            Command::UnfocusInput => "Unfocus input",
            Command::Paste(_) => "Paste text",
            Command::OpenFolderPicker => "Open folder picker",
            Command::CloseFolderPicker => "Close folder picker",
            Command::SelectFolder => "Select folder",
            Command::FolderPickerTypeChar(_) => "Type in folder filter",
            Command::FolderPickerBackspace => "Backspace in folder filter",
            Command::FolderPickerCursorUp => "Folder picker cursor up",
            Command::FolderPickerCursorDown => "Folder picker cursor down",
            Command::CloseFilePicker => "Close file picker",
            Command::FilePickerConfirm => "Confirm file selection",
            Command::FilePickerTypeChar(_) => "Type in file picker filter",
            Command::FilePickerBackspace => "Backspace in file picker filter",
            Command::FilePickerCursorUp => "File picker cursor up",
            Command::FilePickerCursorDown => "File picker cursor down",
            Command::FilePickerToggleSelect => "Toggle file selection",
            Command::FilePickerNavigateIn => "Enter directory",
            Command::FilePickerNavigateUp => "Go to parent directory",
            Command::OpenSlashAutocomplete => "Open slash command autocomplete",
            Command::CloseSlashAutocomplete => "Close slash command autocomplete",
            Command::SelectSlashCommand => "Select slash command",
            Command::SlashAutocompleteTypeChar(_) => "Type in slash autocomplete",
            Command::SlashAutocompleteBackspace => "Backspace in slash autocomplete",
            Command::SlashAutocompleteCursorUp => "Slash autocomplete cursor up",
            Command::SlashAutocompleteCursorDown => "Slash autocomplete cursor down",
            Command::CycleSwitcherForward => "Next thread in switcher",
            Command::CycleSwitcherBackward => "Previous thread in switcher",
            Command::CloseSwitcher => "Close thread switcher",
            Command::ConfirmSwitcherSelection => "Confirm thread selection",
            Command::ApprovePermission => "Approve permission",
            Command::DenyPermission => "Deny permission",
            Command::AlwaysAllowPermission => "Always allow permission",
            Command::HandlePermissionKey(_) => "Handle permission key",
            Command::QuestionNextTab => "Next question tab",
            Command::QuestionPrevOption => "Previous question option",
            Command::QuestionNextOption => "Next question option",
            Command::QuestionToggleOption => "Toggle question option",
            Command::QuestionConfirm => "Confirm question response",
            Command::QuestionCancelOther => "Cancel other input",
            Command::QuestionBackspace => "Backspace in other input",
            Command::QuestionTypeChar(_) => "Type in other input",
            Command::DashboardQuestionNextTab => "Next dashboard question tab",
            Command::DashboardQuestionPrevOption => "Previous dashboard question option",
            Command::DashboardQuestionNextOption => "Next dashboard question option",
            Command::DashboardQuestionToggleOption => "Toggle dashboard question option",
            Command::DashboardQuestionConfirm => "Confirm dashboard question response",
            Command::DashboardQuestionClose => "Close dashboard question overlay",
            Command::DashboardQuestionCancelOther => "Cancel dashboard other input",
            Command::DashboardQuestionBackspace => "Backspace in dashboard other input",
            Command::DashboardQuestionTypeChar(_) => "Type in dashboard other input",
            Command::PlanScrollUp => "Scroll plan up",
            Command::PlanScrollDown => "Scroll plan down",
            Command::ApprovePlan => "Approve plan",
            Command::RejectPlan => "Reject plan",
            Command::ContinueWithNextAccount => "Continue with next account",
            Command::CancelRateLimitRetry => "Cancel rate limit retry",
            Command::ClaudeLoginOpenBrowser => "Open browser for Claude login",
            Command::ClaudeLoginDone => "Confirm Claude login done",
            Command::ClaudeLoginCancel => "Cancel Claude login",
            Command::ClaudeLoginRetry => "Retry Claude login",
            Command::ClaudeAccountsAdd => "Add new Claude account",
            Command::ClaudeAccountsRemove => "Remove selected account",
            Command::ClaudeAccountsClose => "Close Claude accounts",
            Command::ClaudeAccountsMoveUp => "Move selection up",
            Command::ClaudeAccountsMoveDown => "Move selection down",
            Command::VpsConfigNextField => "Next VPS config field",
            Command::VpsConfigPrevField => "Previous VPS config field",
            Command::VpsConfigTypeChar(_) => "Type in VPS config field",
            Command::VpsConfigBackspace => "Backspace in VPS config field",
            Command::VpsConfigSubmit => "Submit VPS config",
            Command::VpsConfigToggleMode => "Toggle VPS config mode",
            Command::VpsConfigClose => "Close VPS config",
            Command::DismissError => "Dismiss error",
            Command::ToggleReasoning => "Toggle reasoning view",
            Command::OpenOAuthUrl => "Open OAuth URL",
            Command::Resize { .. } => "Terminal resize",
            Command::Tick => "Animation tick",
            Command::Noop => "No operation",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_marks_dirty() {
        assert!(Command::InsertChar('a').marks_dirty());
        assert!(Command::Quit.marks_dirty());
        assert!(Command::ScrollPageUp.marks_dirty());
        assert!(!Command::Noop.marks_dirty());
        assert!(!Command::Tick.marks_dirty());
    }

    #[test]
    fn test_command_is_quit() {
        assert!(Command::Quit.is_quit());
        assert!(Command::ForceQuit.is_quit());
        assert!(!Command::InsertChar('a').is_quit());
        assert!(!Command::Noop.is_quit());
    }

    #[test]
    fn test_command_description() {
        assert_eq!(Command::Quit.description(), "Quit application");
        assert_eq!(Command::InsertChar('a').description(), "Insert character");
        assert_eq!(Command::Noop.description(), "No operation");
    }

    #[test]
    fn test_command_clone() {
        let cmd = Command::InsertChar('x');
        let cloned = cmd.clone();
        assert_eq!(cmd, cloned);
    }

    #[test]
    fn test_command_debug() {
        let cmd = Command::ScrollUp(3);
        let debug = format!("{:?}", cmd);
        assert!(debug.contains("ScrollUp"));
        assert!(debug.contains("3"));
    }
}
