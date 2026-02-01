//! Command registry for dispatching keyboard input to commands.
//!
//! The [`CommandRegistry`] provides a centralized place for mapping key events
//! to commands based on the current application context. It handles:
//! - Global bindings (always active)
//! - Modal bindings (folder picker, thread switcher, permissions)
//! - Focus-specific bindings (input vs threads panel)
//! - Screen-specific bindings (conversation vs command deck)

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::command::Command;
use super::context::{InputContext, ModalType};
use super::keybindings::{KeyCombo, KeybindingConfig};

/// Registry for dispatching key events to commands.
///
/// The registry uses a priority system to determine which command to dispatch:
/// 1. Global bindings (Ctrl+C, etc.) - checked first
/// 2. Modal bindings (when a modal is active) - highest priority when modal is open
/// 3. Focus bindings (input vs threads) - depends on current focus
/// 4. Screen bindings (conversation vs command deck) - screen-specific commands
/// 5. Character input (when focused on input) - lowest priority, catches all printable chars
#[derive(Debug, Clone)]
pub struct CommandRegistry {
    /// The keybinding configuration
    config: KeybindingConfig,
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandRegistry {
    /// Creates a new command registry with default keybindings.
    pub fn new() -> Self {
        Self {
            config: KeybindingConfig::new(),
        }
    }

    /// Creates a command registry with a custom keybinding configuration.
    pub fn with_config(config: KeybindingConfig) -> Self {
        Self { config }
    }

    /// Dispatches a key event to a command based on the current context.
    ///
    /// Returns `Some(Command)` if the key event maps to a command,
    /// or `None` if the key should be ignored.
    pub fn dispatch(&self, key: KeyEvent, context: &InputContext) -> Option<Command> {
        let combo = KeyCombo::new(key.code, key.modifiers);

        // Priority 1: Global bindings (Ctrl+C always works)
        // But check Ctrl+C specifically because it should always quit
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Some(Command::Quit);
        }

        // Priority 2: Modal bindings (modal takes over all input)
        // Exception: Permission modal only intercepts character keys (y/n/a)
        if context.is_modal_active() {
            if context.modal == ModalType::Permission {
                // Permission: only capture char keys for y/n/a handling
                if let KeyCode::Char(c) = key.code {
                    return Some(Command::HandlePermissionKey(c));
                }
                // Non-char keys (ESC, arrows, etc.) fall through to normal handling
            } else {
                return self.dispatch_modal(key, context);
            }
        }

        // Priority 3: Check remaining global bindings
        if let Some(cmd) = self.config.get_global(&combo) {
            return Some(self.resolve_global_command(cmd, context));
        }

        // Priority 4: Focus-specific bindings
        if context.is_input_focused() {
            self.dispatch_input(key, context)
        } else {
            self.dispatch_navigation(key, context)
        }
    }

    /// Dispatches input when a modal is active.
    fn dispatch_modal(&self, key: KeyEvent, context: &InputContext) -> Option<Command> {
        let combo = KeyCombo::new(key.code, key.modifiers);

        match context.modal {
            ModalType::FolderPicker => {
                // Check modal-specific bindings first
                if let Some(cmd) = self.config.get_modal(ModalType::FolderPicker, &combo) {
                    return Some(cmd.clone());
                }

                // Handle character input for filter (no modifiers)
                if let KeyCode::Char(c) = key.code {
                    if !key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
                    {
                        return Some(Command::FolderPickerTypeChar(c));
                    }
                }

                // Ignore other keys in folder picker
                Some(Command::Noop)
            }

            ModalType::FilePicker => {
                // Check modal-specific bindings first
                if let Some(cmd) = self.config.get_modal(ModalType::FilePicker, &combo) {
                    return Some(cmd.clone());
                }

                // Handle character input for filter (no modifiers)
                if let KeyCode::Char(c) = key.code {
                    if !key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
                    {
                        return Some(Command::FilePickerTypeChar(c));
                    }
                }

                // Ignore other keys in file picker
                Some(Command::Noop)
            }

            ModalType::SlashAutocomplete => {
                // Check modal-specific bindings first
                if let Some(cmd) = self.config.get_modal(ModalType::SlashAutocomplete, &combo) {
                    return Some(cmd.clone());
                }

                // Handle character input for query (no modifiers)
                if let KeyCode::Char(c) = key.code {
                    if !key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
                    {
                        return Some(Command::SlashAutocompleteTypeChar(c));
                    }
                }

                // Ignore other keys in slash autocomplete
                Some(Command::Noop)
            }

            ModalType::ThreadSwitcher => {
                // Check modal-specific bindings first
                if let Some(cmd) = self.config.get_modal(ModalType::ThreadSwitcher, &combo) {
                    return Some(cmd.clone());
                }

                // Any other key confirms selection
                Some(Command::ConfirmSwitcherSelection)
            }

            ModalType::Permission => {
                // Permission is now handled in dispatch() before dispatch_modal is called.
                // This case is unreachable but kept for exhaustive match.
                unreachable!("Permission modal handled in dispatch()")
            }

            ModalType::AskUserQuestion => {
                // Check modal-specific bindings
                if let Some(cmd) = self.config.get_modal(ModalType::AskUserQuestion, &combo) {
                    return Some(cmd.clone());
                }

                // Handle N/n to deny
                if let KeyCode::Char('n') | KeyCode::Char('N') = key.code {
                    return Some(Command::DenyPermission);
                }

                // Ignore other keys
                Some(Command::Noop)
            }

            ModalType::AskUserQuestionOther => {
                // Check modal-specific bindings
                if let Some(cmd) = self
                    .config
                    .get_modal(ModalType::AskUserQuestionOther, &combo)
                {
                    return Some(cmd.clone());
                }

                // Handle character input
                if let KeyCode::Char(c) = key.code {
                    return Some(Command::QuestionTypeChar(c));
                }

                // Ignore other keys
                Some(Command::Noop)
            }

            ModalType::DashboardQuestionOverlay => {
                // Check modal-specific bindings
                if let Some(cmd) = self
                    .config
                    .get_modal(ModalType::DashboardQuestionOverlay, &combo)
                {
                    return Some(cmd.clone());
                }

                // Handle N/n to close (deny)
                if let KeyCode::Char('n') | KeyCode::Char('N') = key.code {
                    return Some(Command::DashboardQuestionClose);
                }

                // Ignore other keys
                Some(Command::Noop)
            }

            ModalType::DashboardQuestionOverlayOther => {
                // Check modal-specific bindings
                if let Some(cmd) = self
                    .config
                    .get_modal(ModalType::DashboardQuestionOverlayOther, &combo)
                {
                    return Some(cmd.clone());
                }

                // Handle character input for "Other" text field
                if let KeyCode::Char(c) = key.code {
                    return Some(Command::DashboardQuestionTypeChar(c));
                }

                // Ignore other keys
                Some(Command::Noop)
            }

            ModalType::PlanApproval => {
                // Check modal-specific bindings (Up/Down for scroll, y/n for approve/reject, f for feedback)
                if let Some(cmd) = self.config.get_modal(ModalType::PlanApproval, &combo) {
                    return Some(cmd.clone());
                }

                // Allow PageUp/PageDown to also scroll the conversation
                if matches!(key.code, KeyCode::PageUp | KeyCode::PageDown) {
                    return None; // Fall through to normal handling
                }

                // Ignore other keys
                Some(Command::Noop)
            }

            ModalType::PlanFeedback => {
                // Check modal-specific bindings (Esc, Enter, Backspace)
                if let Some(cmd) = self.config.get_modal(ModalType::PlanFeedback, &combo) {
                    return Some(cmd.clone());
                }

                // Handle character input for feedback text
                if let KeyCode::Char(c) = key.code {
                    if !key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
                    {
                        return Some(Command::PlanFeedbackTypeChar(c));
                    }
                }

                // Ignore other keys
                Some(Command::Noop)
            }

            ModalType::ClaudeLogin => {
                // Claude login dialog - handle Enter, D, Esc, R keys
                match key.code {
                    KeyCode::Enter => Some(Command::ClaudeLoginOpenBrowser),
                    KeyCode::Char('d') | KeyCode::Char('D') => Some(Command::ClaudeLoginDone),
                    KeyCode::Char('r') | KeyCode::Char('R') => Some(Command::ClaudeLoginRetry),
                    KeyCode::Esc => Some(Command::ClaudeLoginCancel),
                    _ => Some(Command::Noop), // Ignore other keys
                }
            }

            ModalType::ClaudeAccounts => {
                match key.code {
                    KeyCode::Char('a') | KeyCode::Char('A') => Some(Command::ClaudeAccountsAdd),
                    KeyCode::Char('r') | KeyCode::Char('R') => Some(Command::ClaudeAccountsRemove),
                    KeyCode::Char('t') | KeyCode::Char('T') => Some(Command::ClaudeAccountsPasteStart),
                    KeyCode::Up => Some(Command::ClaudeAccountsMoveUp),
                    KeyCode::Down => Some(Command::ClaudeAccountsMoveDown),
                    KeyCode::Esc => Some(Command::ClaudeAccountsClose),
                    _ => Some(Command::Noop),
                }
            }

            ModalType::ClaudeAccountsPaste => {
                // Paste-token text input mode — route chars/Enter/Esc/Backspace
                match key.code {
                    KeyCode::Enter => Some(Command::ClaudeAccountsPasteSubmit),
                    KeyCode::Esc => Some(Command::ClaudeAccountsPasteCancel),
                    KeyCode::Backspace => Some(Command::ClaudeAccountsPasteBackspace),
                    KeyCode::Char(c) => Some(Command::ClaudeAccountsPasteChar(c)),
                    _ => Some(Command::Noop),
                }
            }

            ModalType::RateLimitConfirm => {
                // Use modal-specific bindings from keybindings.rs (Y/N/Esc)
                if let Some(cmd) = self.config.get_modal(ModalType::RateLimitConfirm, &combo) {
                    return Some(cmd.clone());
                }
                // Ignore other keys
                Some(Command::Noop)
            }

            ModalType::VpsConfig => {
                // VPS config modal - handle Tab, Enter, Esc, R, and char input
                match key.code {
                    KeyCode::Tab | KeyCode::Down => Some(Command::VpsConfigNextField),
                    KeyCode::BackTab | KeyCode::Up => Some(Command::VpsConfigPrevField),
                    KeyCode::Enter => Some(Command::VpsConfigSubmit),
                    KeyCode::Esc => Some(Command::VpsConfigClose),
                    KeyCode::Backspace => Some(Command::VpsConfigBackspace),
                    KeyCode::Char(c) => Some(Command::VpsConfigTypeChar(c)),
                    _ => Some(Command::Noop),
                }
            }

            ModalType::AskUserQuestionPending => {
                // Only 'A'/'a' opens the overlay, other keys fall through to normal handling
                if let KeyCode::Char('a') | KeyCode::Char('A') = key.code {
                    return Some(Command::OpenQuestionOverlay);
                }
                None // Fall through for navigation (arrows), etc.
            }

            ModalType::None => {
                // Should not reach here, but handle gracefully
                None
            }
        }
    }

    /// Dispatches input when the input field is focused.
    fn dispatch_input(&self, key: KeyEvent, context: &InputContext) -> Option<Command> {
        let combo = KeyCombo::new(key.code, key.modifiers);

        // Check for Shift+Escape first (navigate to command deck)
        if key.code == KeyCode::Esc
            && key.modifiers.contains(KeyModifiers::SHIFT)
            && context.is_conversation_screen()
        {
            return Some(Command::NavigateToCommandDeck);
        }

        // Check input editing bindings
        if let Some(cmd) = self.config.get_input_editing(&combo) {
            return Some(self.resolve_input_command(cmd, context));
        }

        // Handle character input (no modifiers except SHIFT for uppercase)
        if let KeyCode::Char(c) = key.code {
            if !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
            {
                // Check for @ trigger for folder picker (only on CommandDeck)
                if c == '@' && context.is_command_deck_screen() {
                    // The actual trigger check is context-dependent, handled by App
                    return Some(Command::InsertChar(c));
                }

                return Some(Command::InsertChar(c));
            }
        }

        // Fall through to screen-specific bindings for things like PageUp/Down
        if let Some(cmd) = self.config.get_screen(context.screen, &combo) {
            return Some(cmd.clone());
        }

        None
    }

    /// Dispatches input when navigating (not in input field).
    fn dispatch_navigation(&self, key: KeyEvent, context: &InputContext) -> Option<Command> {
        let combo = KeyCombo::new(key.code, key.modifiers);

        // Check focus-specific bindings (Threads panel)
        if let Some(cmd) = self.config.get_focus(context.focus, &combo) {
            return Some(self.resolve_navigation_command(cmd, context));
        }

        // Check screen-specific bindings
        if let Some(cmd) = self.config.get_screen(context.screen, &combo) {
            return Some(cmd.clone());
        }

        // Auto-focus to input when user starts typing
        if let KeyCode::Char(c) = key.code {
            if !key.modifiers.contains(KeyModifiers::CONTROL) {
                return Some(Command::InsertChar(c));
            }
        }

        None
    }

    /// Resolves a global command based on context.
    fn resolve_global_command(&self, cmd: &Command, context: &InputContext) -> Command {
        match cmd {
            Command::NavigateToCommandDeck => {
                if context.is_conversation_screen() {
                    Command::NavigateToCommandDeck
                } else {
                    Command::Noop
                }
            }
            Command::SubmitAsProgramming => {
                if context.is_command_deck_screen() && !context.input_is_empty {
                    Command::SubmitAsProgramming
                } else {
                    Command::Noop
                }
            }
            _ => cmd.clone(),
        }
    }

    /// Resolves an input command based on context.
    fn resolve_input_command(&self, cmd: &Command, context: &InputContext) -> Command {
        match cmd {
            Command::MoveCursorUp => {
                // If on first line, navigate history up
                if context.cursor_on_first_line {
                    Command::HistoryUp
                } else {
                    Command::MoveCursorUp
                }
            }
            Command::MoveCursorDown => {
                // If on last line and navigating history, go forward in history
                if context.cursor_on_last_line && context.is_navigating_history {
                    Command::HistoryDown
                } else if context.cursor_on_last_line {
                    // On last line but not navigating - noop
                    Command::Noop
                } else {
                    Command::MoveCursorDown
                }
            }
            Command::UnfocusInput => {
                // Escape behavior depends on screen and input state
                if context.is_conversation_screen() {
                    if context.input_is_empty {
                        Command::NavigateToCommandDeck
                    } else {
                        Command::UnfocusInput
                    }
                } else {
                    Command::UnfocusInput
                }
            }
            _ => cmd.clone(),
        }
    }

    /// Resolves a navigation command based on context.
    fn resolve_navigation_command(&self, cmd: &Command, context: &InputContext) -> Command {
        match cmd {
            Command::DismissError => {
                if context.is_conversation_screen() && context.has_errors {
                    Command::DismissError
                } else {
                    Command::Noop
                }
            }
            Command::ToggleReasoning => {
                if context.is_conversation_screen() {
                    Command::ToggleReasoning
                } else {
                    Command::Noop
                }
            }
            Command::OpenOAuthUrl => {
                if context.has_oauth_url {
                    Command::OpenOAuthUrl
                } else {
                    Command::Noop
                }
            }
            Command::NavigateToCommandDeck => {
                if context.is_conversation_screen() {
                    Command::NavigateToCommandDeck
                } else {
                    Command::Noop
                }
            }
            _ => cmd.clone(),
        }
    }

    /// Gets the keybinding configuration (for UI display).
    pub fn config(&self) -> &KeybindingConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Focus, Screen};
    use crate::models::ThreadType;
    use crossterm::event::KeyEventKind;

    fn make_key_event(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        }
    }

    #[test]
    fn test_registry_default() {
        let registry = CommandRegistry::new();
        assert!(registry
            .config()
            .global
            .contains_key(&KeyCombo::ctrl(KeyCode::Char('c'))));
    }

    #[test]
    fn test_dispatch_ctrl_c_always_quits() {
        let registry = CommandRegistry::new();
        let context = InputContext::new();

        let key = make_key_event(KeyCode::Char('c'), KeyModifiers::CONTROL);
        let cmd = registry.dispatch(key, &context);

        assert!(matches!(cmd, Some(Command::Quit)));
    }

    #[test]
    fn test_dispatch_ctrl_c_quits_even_in_modal() {
        let registry = CommandRegistry::new();
        let context = InputContext::new().with_modal(ModalType::FolderPicker);

        let key = make_key_event(KeyCode::Char('c'), KeyModifiers::CONTROL);
        let cmd = registry.dispatch(key, &context);

        assert!(matches!(cmd, Some(Command::Quit)));
    }

    #[test]
    fn test_dispatch_folder_picker_escape() {
        let registry = CommandRegistry::new();
        let context = InputContext::new().with_modal(ModalType::FolderPicker);

        let key = make_key_event(KeyCode::Esc, KeyModifiers::NONE);
        let cmd = registry.dispatch(key, &context);

        assert!(matches!(cmd, Some(Command::CloseFolderPicker)));
    }

    #[test]
    fn test_dispatch_folder_picker_char() {
        let registry = CommandRegistry::new();
        let context = InputContext::new().with_modal(ModalType::FolderPicker);

        let key = make_key_event(KeyCode::Char('a'), KeyModifiers::NONE);
        let cmd = registry.dispatch(key, &context);

        assert!(matches!(cmd, Some(Command::FolderPickerTypeChar('a'))));
    }

    #[test]
    fn test_dispatch_thread_switcher_tab() {
        let registry = CommandRegistry::new();
        let context = InputContext::new().with_modal(ModalType::ThreadSwitcher);

        let key = make_key_event(KeyCode::Tab, KeyModifiers::NONE);
        let cmd = registry.dispatch(key, &context);

        assert!(matches!(cmd, Some(Command::CycleSwitcherForward)));
    }

    #[test]
    fn test_dispatch_thread_switcher_any_key_confirms() {
        let registry = CommandRegistry::new();
        let context = InputContext::new().with_modal(ModalType::ThreadSwitcher);

        let key = make_key_event(KeyCode::Char('x'), KeyModifiers::NONE);
        let cmd = registry.dispatch(key, &context);

        assert!(matches!(cmd, Some(Command::ConfirmSwitcherSelection)));
    }

    #[test]
    fn test_dispatch_permission_handles_char() {
        let registry = CommandRegistry::new();
        let context = InputContext::new().with_modal(ModalType::Permission);

        let key = make_key_event(KeyCode::Char('y'), KeyModifiers::NONE);
        let cmd = registry.dispatch(key, &context);

        assert!(matches!(cmd, Some(Command::HandlePermissionKey('y'))));
    }

    #[test]
    fn test_dispatch_input_focused_char() {
        let registry = CommandRegistry::new();
        let context = InputContext::new().with_focus(Focus::Input);

        let key = make_key_event(KeyCode::Char('a'), KeyModifiers::NONE);
        let cmd = registry.dispatch(key, &context);

        assert!(matches!(cmd, Some(Command::InsertChar('a'))));
    }

    #[test]
    fn test_dispatch_input_focused_backspace() {
        let registry = CommandRegistry::new();
        let context = InputContext::new().with_focus(Focus::Input);

        let key = make_key_event(KeyCode::Backspace, KeyModifiers::NONE);
        let cmd = registry.dispatch(key, &context);

        assert!(matches!(cmd, Some(Command::Backspace)));
    }

    #[test]
    fn test_dispatch_input_focused_alt_backspace() {
        let registry = CommandRegistry::new();
        let context = InputContext::new().with_focus(Focus::Input);

        let key = make_key_event(KeyCode::Backspace, KeyModifiers::ALT);
        let cmd = registry.dispatch(key, &context);

        assert!(matches!(cmd, Some(Command::DeleteWordBackward)));
    }

    #[test]
    fn test_dispatch_input_up_on_first_line_navigates_history() {
        let registry = CommandRegistry::new();
        let context = InputContext::new()
            .with_focus(Focus::Input)
            .with_cursor_position(true, false);

        let key = make_key_event(KeyCode::Up, KeyModifiers::NONE);
        let cmd = registry.dispatch(key, &context);

        assert!(matches!(cmd, Some(Command::HistoryUp)));
    }

    #[test]
    fn test_dispatch_input_up_not_on_first_line_moves_cursor() {
        let registry = CommandRegistry::new();
        let context = InputContext::new()
            .with_focus(Focus::Input)
            .with_cursor_position(false, false);

        let key = make_key_event(KeyCode::Up, KeyModifiers::NONE);
        let cmd = registry.dispatch(key, &context);

        assert!(matches!(cmd, Some(Command::MoveCursorUp)));
    }

    #[test]
    fn test_dispatch_threads_panel_up() {
        let registry = CommandRegistry::new();
        let context = InputContext::new().with_focus(Focus::Threads);

        let key = make_key_event(KeyCode::Up, KeyModifiers::NONE);
        let cmd = registry.dispatch(key, &context);

        assert!(matches!(cmd, Some(Command::MoveUp)));
    }

    #[test]
    fn test_dispatch_threads_panel_q_quits() {
        let registry = CommandRegistry::new();
        let context = InputContext::new().with_focus(Focus::Threads);

        let key = make_key_event(KeyCode::Char('q'), KeyModifiers::NONE);
        let cmd = registry.dispatch(key, &context);

        assert!(matches!(cmd, Some(Command::Quit)));
    }

    #[test]
    fn test_dispatch_conversation_page_up() {
        let registry = CommandRegistry::new();
        let context = InputContext::new()
            .with_screen(Screen::Conversation)
            .with_focus(Focus::Input);

        let key = make_key_event(KeyCode::PageUp, KeyModifiers::NONE);
        let cmd = registry.dispatch(key, &context);

        assert!(matches!(cmd, Some(Command::ScrollPageUp)));
    }

    #[test]
    fn test_dispatch_escape_in_input_unfocuses() {
        let registry = CommandRegistry::new();
        let context = InputContext::new()
            .with_focus(Focus::Input)
            .with_screen(Screen::CommandDeck);

        let key = make_key_event(KeyCode::Esc, KeyModifiers::NONE);
        let cmd = registry.dispatch(key, &context);

        assert!(matches!(cmd, Some(Command::UnfocusInput)));
    }

    #[test]
    fn test_dispatch_escape_in_input_empty_navigates_back() {
        let registry = CommandRegistry::new();
        let context = InputContext::new()
            .with_focus(Focus::Input)
            .with_screen(Screen::Conversation)
            .with_input_empty(true);

        let key = make_key_event(KeyCode::Esc, KeyModifiers::NONE);
        let cmd = registry.dispatch(key, &context);

        assert!(matches!(cmd, Some(Command::NavigateToCommandDeck)));
    }

    #[test]
    fn test_dispatch_shift_escape_navigates_back() {
        let registry = CommandRegistry::new();
        let context = InputContext::new()
            .with_focus(Focus::Input)
            .with_screen(Screen::Conversation);

        let key = make_key_event(KeyCode::Esc, KeyModifiers::SHIFT);
        let cmd = registry.dispatch(key, &context);

        assert!(matches!(cmd, Some(Command::NavigateToCommandDeck)));
    }

    #[test]
    fn test_dispatch_ctrl_n_creates_thread() {
        let registry = CommandRegistry::new();
        let context = InputContext::new();

        let key = make_key_event(KeyCode::Char('n'), KeyModifiers::CONTROL);
        let cmd = registry.dispatch(key, &context);

        assert!(matches!(cmd, Some(Command::CreateNewThread)));
    }

    #[test]
    fn test_dispatch_shift_n_creates_thread() {
        let registry = CommandRegistry::new();
        let context = InputContext::new();

        let key = make_key_event(KeyCode::Char('N'), KeyModifiers::SHIFT);
        let cmd = registry.dispatch(key, &context);

        assert!(matches!(cmd, Some(Command::CreateNewThread)));
    }

    #[test]
    fn test_dispatch_enter_submits() {
        let registry = CommandRegistry::new();
        let context = InputContext::new().with_focus(Focus::Input);

        let key = make_key_event(KeyCode::Enter, KeyModifiers::NONE);
        let cmd = registry.dispatch(key, &context);

        assert!(matches!(
            cmd,
            Some(Command::SubmitInput(ThreadType::Conversation))
        ));
    }

    #[test]
    fn test_dispatch_shift_enter_inserts_newline() {
        let registry = CommandRegistry::new();
        let context = InputContext::new().with_focus(Focus::Input);

        let key = make_key_event(KeyCode::Enter, KeyModifiers::SHIFT);
        let cmd = registry.dispatch(key, &context);

        assert!(matches!(cmd, Some(Command::InsertNewline)));
    }

    #[test]
    fn test_dispatch_auto_focus_on_typing() {
        let registry = CommandRegistry::new();
        let context = InputContext::new().with_focus(Focus::Threads);

        let key = make_key_event(KeyCode::Char('h'), KeyModifiers::NONE);
        let cmd = registry.dispatch(key, &context);

        // Should insert the character (which will trigger auto-focus in handler)
        assert!(matches!(cmd, Some(Command::InsertChar('h'))));
    }
}
