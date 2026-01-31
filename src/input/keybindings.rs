//! Default keybindings for the application.
//!
//! This module defines the default key bindings that map key combinations
//! to commands. These bindings can be customized in the future.

use crossterm::event::{KeyCode, KeyModifiers};
use std::collections::HashMap;

use super::command::Command;
use super::context::ModalType;
use crate::app::{Focus, Screen};
use crate::models::ThreadType;

/// Represents a key combination (key code + modifiers).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyCombo {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

impl KeyCombo {
    /// Creates a new key combo with the given code and modifiers.
    pub const fn new(code: KeyCode, modifiers: KeyModifiers) -> Self {
        Self { code, modifiers }
    }

    /// Creates a key combo with no modifiers.
    pub const fn plain(code: KeyCode) -> Self {
        Self::new(code, KeyModifiers::NONE)
    }

    /// Creates a key combo with Control modifier.
    pub const fn ctrl(code: KeyCode) -> Self {
        Self::new(code, KeyModifiers::CONTROL)
    }

    /// Creates a key combo with Shift modifier.
    pub const fn shift(code: KeyCode) -> Self {
        Self::new(code, KeyModifiers::SHIFT)
    }

    /// Creates a key combo with Alt modifier.
    pub const fn alt(code: KeyCode) -> Self {
        Self::new(code, KeyModifiers::ALT)
    }

    /// Creates a key combo with Super (Cmd on macOS) modifier.
    pub const fn super_key(code: KeyCode) -> Self {
        Self::new(code, KeyModifiers::SUPER)
    }
}

/// Keybinding configuration for the application.
#[derive(Debug, Clone)]
pub struct KeybindingConfig {
    /// Global keybindings (always active)
    pub global: HashMap<KeyCombo, Command>,
    /// Keybindings per modal type
    pub modal: HashMap<ModalType, HashMap<KeyCombo, Command>>,
    /// Keybindings per screen
    pub screen: HashMap<Screen, HashMap<KeyCombo, Command>>,
    /// Keybindings per focus state
    pub focus: HashMap<Focus, HashMap<KeyCombo, Command>>,
    /// Input-specific keybindings (when focus is Input)
    pub input_editing: HashMap<KeyCombo, Command>,
}

impl Default for KeybindingConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl KeybindingConfig {
    /// Creates a new keybinding configuration with default bindings.
    pub fn new() -> Self {
        let mut config = Self {
            global: HashMap::new(),
            modal: HashMap::new(),
            screen: HashMap::new(),
            focus: HashMap::new(),
            input_editing: HashMap::new(),
        };

        config.setup_global_bindings();
        config.setup_modal_bindings();
        config.setup_screen_bindings();
        config.setup_focus_bindings();
        config.setup_input_editing_bindings();

        config
    }

    /// Sets up global keybindings (always active).
    fn setup_global_bindings(&mut self) {
        // Ctrl+C: Quit
        self.global
            .insert(KeyCombo::ctrl(KeyCode::Char('c')), Command::Quit);

        // Shift+Escape: Navigate to CommandDeck
        self.global.insert(
            KeyCombo::shift(KeyCode::Esc),
            Command::NavigateToCommandDeck,
        );

        // Ctrl+W: Navigate to CommandDeck
        self.global.insert(
            KeyCombo::ctrl(KeyCode::Char('w')),
            Command::NavigateToCommandDeck,
        );

        // Shift+N: Create new thread
        self.global.insert(
            KeyCombo::shift(KeyCode::Char('N')),
            Command::CreateNewThread,
        );

        // Ctrl+N: Create new thread (alternative)
        self.global
            .insert(KeyCombo::ctrl(KeyCode::Char('n')), Command::CreateNewThread);

        // Alt+P: Submit as programming thread
        self.global.insert(
            KeyCombo::alt(KeyCode::Char('p')),
            Command::SubmitAsProgramming,
        );
    }

    /// Sets up modal-specific keybindings.
    fn setup_modal_bindings(&mut self) {
        // Folder picker bindings
        let mut folder_picker = HashMap::new();
        folder_picker.insert(KeyCombo::plain(KeyCode::Esc), Command::CloseFolderPicker);
        folder_picker.insert(KeyCombo::plain(KeyCode::Enter), Command::SelectFolder);
        folder_picker.insert(KeyCombo::plain(KeyCode::Up), Command::FolderPickerCursorUp);
        folder_picker.insert(
            KeyCombo::plain(KeyCode::Down),
            Command::FolderPickerCursorDown,
        );
        folder_picker.insert(
            KeyCombo::plain(KeyCode::Backspace),
            Command::FolderPickerBackspace,
        );
        self.modal.insert(ModalType::FolderPicker, folder_picker);

        // File picker bindings (Conversation screen)
        let mut file_picker = HashMap::new();
        file_picker.insert(KeyCombo::plain(KeyCode::Esc), Command::CloseFilePicker);
        file_picker.insert(KeyCombo::plain(KeyCode::Enter), Command::FilePickerConfirm);
        file_picker.insert(KeyCombo::plain(KeyCode::Up), Command::FilePickerCursorUp);
        file_picker.insert(KeyCombo::plain(KeyCode::Down), Command::FilePickerCursorDown);
        file_picker.insert(
            KeyCombo::plain(KeyCode::Backspace),
            Command::FilePickerBackspace,
        );
        file_picker.insert(KeyCombo::plain(KeyCode::Tab), Command::FilePickerToggleSelect);
        file_picker.insert(KeyCombo::plain(KeyCode::Right), Command::FilePickerNavigateIn);
        file_picker.insert(KeyCombo::plain(KeyCode::Left), Command::FilePickerNavigateUp);
        self.modal.insert(ModalType::FilePicker, file_picker);

        // Slash command autocomplete bindings
        let mut slash_autocomplete = HashMap::new();
        slash_autocomplete.insert(
            KeyCombo::plain(KeyCode::Esc),
            Command::CloseSlashAutocomplete,
        );
        slash_autocomplete.insert(KeyCombo::plain(KeyCode::Enter), Command::SelectSlashCommand);
        slash_autocomplete.insert(
            KeyCombo::plain(KeyCode::Up),
            Command::SlashAutocompleteCursorUp,
        );
        slash_autocomplete.insert(
            KeyCombo::plain(KeyCode::Down),
            Command::SlashAutocompleteCursorDown,
        );
        slash_autocomplete.insert(
            KeyCombo::plain(KeyCode::Backspace),
            Command::SlashAutocompleteBackspace,
        );
        self.modal
            .insert(ModalType::SlashAutocomplete, slash_autocomplete);

        // Thread switcher bindings
        let mut thread_switcher = HashMap::new();
        thread_switcher.insert(KeyCombo::plain(KeyCode::Tab), Command::CycleSwitcherForward);
        thread_switcher.insert(
            KeyCombo::plain(KeyCode::Down),
            Command::CycleSwitcherForward,
        );
        thread_switcher.insert(KeyCombo::plain(KeyCode::Up), Command::CycleSwitcherBackward);
        thread_switcher.insert(KeyCombo::plain(KeyCode::Esc), Command::CloseSwitcher);
        thread_switcher.insert(
            KeyCombo::plain(KeyCode::Enter),
            Command::ConfirmSwitcherSelection,
        );
        self.modal
            .insert(ModalType::ThreadSwitcher, thread_switcher);

        // AskUserQuestion bindings
        let mut question = HashMap::new();
        question.insert(KeyCombo::plain(KeyCode::Tab), Command::QuestionNextTab);
        question.insert(KeyCombo::plain(KeyCode::Up), Command::QuestionPrevOption);
        question.insert(KeyCombo::plain(KeyCode::Down), Command::QuestionNextOption);
        question.insert(
            KeyCombo::plain(KeyCode::Char(' ')),
            Command::QuestionToggleOption,
        );
        question.insert(KeyCombo::plain(KeyCode::Enter), Command::QuestionConfirm);
        self.modal.insert(ModalType::AskUserQuestion, question);

        // AskUserQuestion "Other" text input bindings
        let mut question_other = HashMap::new();
        question_other.insert(KeyCombo::plain(KeyCode::Esc), Command::QuestionCancelOther);
        question_other.insert(KeyCombo::plain(KeyCode::Enter), Command::QuestionConfirm);
        question_other.insert(
            KeyCombo::plain(KeyCode::Backspace),
            Command::QuestionBackspace,
        );
        self.modal
            .insert(ModalType::AskUserQuestionOther, question_other);

        // Dashboard Question Overlay bindings
        let mut dashboard_question = HashMap::new();
        dashboard_question.insert(
            KeyCombo::plain(KeyCode::Tab),
            Command::DashboardQuestionNextTab,
        );
        dashboard_question.insert(
            KeyCombo::plain(KeyCode::Up),
            Command::DashboardQuestionPrevOption,
        );
        dashboard_question.insert(
            KeyCombo::plain(KeyCode::Down),
            Command::DashboardQuestionNextOption,
        );
        dashboard_question.insert(
            KeyCombo::plain(KeyCode::Char(' ')),
            Command::DashboardQuestionToggleOption,
        );
        dashboard_question.insert(
            KeyCombo::plain(KeyCode::Enter),
            Command::DashboardQuestionConfirm,
        );
        dashboard_question.insert(
            KeyCombo::plain(KeyCode::Esc),
            Command::DashboardQuestionClose,
        );
        self.modal
            .insert(ModalType::DashboardQuestionOverlay, dashboard_question);

        // Dashboard Question Overlay "Other" text input bindings
        let mut dashboard_question_other = HashMap::new();
        dashboard_question_other.insert(
            KeyCombo::plain(KeyCode::Esc),
            Command::DashboardQuestionCancelOther,
        );
        dashboard_question_other.insert(
            KeyCombo::plain(KeyCode::Enter),
            Command::DashboardQuestionConfirm,
        );
        dashboard_question_other.insert(
            KeyCombo::plain(KeyCode::Backspace),
            Command::DashboardQuestionBackspace,
        );
        self.modal
            .insert(ModalType::DashboardQuestionOverlayOther, dashboard_question_other);

        // Plan approval modal bindings
        // Up/Down for scrolling plan content, y/n for approve/reject
        let mut plan_approval = HashMap::new();
        plan_approval.insert(KeyCombo::plain(KeyCode::Up), Command::PlanScrollUp);
        plan_approval.insert(KeyCombo::plain(KeyCode::Down), Command::PlanScrollDown);
        plan_approval.insert(KeyCombo::plain(KeyCode::PageUp), Command::PlanScrollUp);
        plan_approval.insert(KeyCombo::plain(KeyCode::PageDown), Command::PlanScrollDown);
        plan_approval.insert(KeyCombo::plain(KeyCode::Char('y')), Command::ApprovePlan);
        plan_approval.insert(KeyCombo::plain(KeyCode::Char('Y')), Command::ApprovePlan);
        plan_approval.insert(KeyCombo::plain(KeyCode::Char('n')), Command::RejectPlan);
        plan_approval.insert(KeyCombo::plain(KeyCode::Char('N')), Command::RejectPlan);
        self.modal.insert(ModalType::PlanApproval, plan_approval);

        // Rate limit confirmation bindings
        let mut rate_limit_confirm = HashMap::new();
        rate_limit_confirm.insert(KeyCombo::plain(KeyCode::Char('y')), Command::ContinueWithNextAccount);
        rate_limit_confirm.insert(KeyCombo::plain(KeyCode::Char('Y')), Command::ContinueWithNextAccount);
        rate_limit_confirm.insert(KeyCombo::plain(KeyCode::Char('n')), Command::CancelRateLimitRetry);
        rate_limit_confirm.insert(KeyCombo::plain(KeyCode::Char('N')), Command::CancelRateLimitRetry);
        rate_limit_confirm.insert(KeyCombo::plain(KeyCode::Esc), Command::CancelRateLimitRetry);
        self.modal.insert(ModalType::RateLimitConfirm, rate_limit_confirm);
    }

    /// Sets up screen-specific keybindings.
    fn setup_screen_bindings(&mut self) {
        // Conversation screen bindings
        let mut conversation = HashMap::new();
        conversation.insert(KeyCombo::plain(KeyCode::PageUp), Command::ScrollPageUp);
        conversation.insert(KeyCombo::plain(KeyCode::PageDown), Command::ScrollPageDown);
        self.screen.insert(Screen::Conversation, conversation);
    }

    /// Sets up focus-specific keybindings.
    fn setup_focus_bindings(&mut self) {
        // Threads panel bindings
        let mut threads = HashMap::new();
        threads.insert(KeyCombo::plain(KeyCode::Up), Command::MoveUp);
        threads.insert(KeyCombo::plain(KeyCode::Down), Command::MoveDown);
        threads.insert(KeyCombo::plain(KeyCode::Enter), Command::OpenSelectedThread);
        threads.insert(KeyCombo::plain(KeyCode::Tab), Command::HandleTabPress);
        threads.insert(
            KeyCombo::plain(KeyCode::BackTab),
            Command::CyclePermissionMode,
        );
        threads.insert(KeyCombo::plain(KeyCode::Char('q')), Command::Quit);
        threads.insert(KeyCombo::plain(KeyCode::Char('d')), Command::DismissError);
        threads.insert(
            KeyCombo::plain(KeyCode::Char('t')),
            Command::ToggleReasoning,
        );
        threads.insert(KeyCombo::plain(KeyCode::Char('o')), Command::OpenOAuthUrl);
        threads.insert(
            KeyCombo::plain(KeyCode::Esc),
            Command::NavigateToCommandDeck,
        );
        self.focus.insert(Focus::Threads, threads);
    }

    /// Sets up input editing keybindings.
    fn setup_input_editing_bindings(&mut self) {
        // Navigation
        self.input_editing
            .insert(KeyCombo::plain(KeyCode::Left), Command::MoveCursorLeft);
        self.input_editing
            .insert(KeyCombo::plain(KeyCode::Right), Command::MoveCursorRight);
        self.input_editing
            .insert(KeyCombo::plain(KeyCode::Up), Command::MoveCursorUp);
        self.input_editing
            .insert(KeyCombo::plain(KeyCode::Down), Command::MoveCursorDown);
        self.input_editing
            .insert(KeyCombo::plain(KeyCode::Home), Command::MoveCursorHome);
        self.input_editing
            .insert(KeyCombo::plain(KeyCode::End), Command::MoveCursorEnd);

        // Word navigation (Alt+Arrow)
        self.input_editing
            .insert(KeyCombo::alt(KeyCode::Left), Command::MoveCursorWordLeft);
        self.input_editing
            .insert(KeyCombo::alt(KeyCode::Right), Command::MoveCursorWordRight);

        // Line navigation (Cmd/Super+Arrow)
        self.input_editing
            .insert(KeyCombo::super_key(KeyCode::Left), Command::MoveCursorHome);
        self.input_editing
            .insert(KeyCombo::super_key(KeyCode::Right), Command::MoveCursorEnd);

        // Deletion
        self.input_editing
            .insert(KeyCombo::plain(KeyCode::Backspace), Command::Backspace);
        self.input_editing
            .insert(KeyCombo::plain(KeyCode::Delete), Command::DeleteChar);
        self.input_editing.insert(
            KeyCombo::alt(KeyCode::Backspace),
            Command::DeleteWordBackward,
        );
        self.input_editing.insert(
            KeyCombo::super_key(KeyCode::Backspace),
            Command::DeleteToLineStart,
        );
        self.input_editing.insert(
            KeyCombo::ctrl(KeyCode::Char('u')),
            Command::DeleteToLineStart,
        );

        // Newline insertion
        self.input_editing
            .insert(KeyCombo::shift(KeyCode::Enter), Command::InsertNewline);
        self.input_editing
            .insert(KeyCombo::alt(KeyCode::Enter), Command::InsertNewline);
        self.input_editing
            .insert(KeyCombo::ctrl(KeyCode::Enter), Command::InsertNewline);
        self.input_editing
            .insert(KeyCombo::ctrl(KeyCode::Char('j')), Command::InsertNewline);

        // Submit
        self.input_editing.insert(
            KeyCombo::plain(KeyCode::Enter),
            Command::SubmitInput(ThreadType::Conversation),
        );

        // Escape handling (context-dependent, will be resolved in registry)
        self.input_editing
            .insert(KeyCombo::plain(KeyCode::Esc), Command::UnfocusInput);

        // Shift+Tab for permission mode cycling
        self.input_editing.insert(
            KeyCombo::plain(KeyCode::BackTab),
            Command::CyclePermissionMode,
        );
    }

    /// Gets the command for a key combo in the current context.
    /// This is a simple lookup; the registry handles priority.
    pub fn get_global(&self, combo: &KeyCombo) -> Option<&Command> {
        self.global.get(combo)
    }

    /// Gets the modal-specific command for a key combo.
    pub fn get_modal(&self, modal: ModalType, combo: &KeyCombo) -> Option<&Command> {
        self.modal.get(&modal).and_then(|m| m.get(combo))
    }

    /// Gets the screen-specific command for a key combo.
    pub fn get_screen(&self, screen: Screen, combo: &KeyCombo) -> Option<&Command> {
        self.screen.get(&screen).and_then(|m| m.get(combo))
    }

    /// Gets the focus-specific command for a key combo.
    pub fn get_focus(&self, focus: Focus, combo: &KeyCombo) -> Option<&Command> {
        self.focus.get(&focus).and_then(|m| m.get(combo))
    }

    /// Gets the input editing command for a key combo.
    pub fn get_input_editing(&self, combo: &KeyCombo) -> Option<&Command> {
        self.input_editing.get(combo)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_combo_plain() {
        let combo = KeyCombo::plain(KeyCode::Enter);
        assert_eq!(combo.code, KeyCode::Enter);
        assert_eq!(combo.modifiers, KeyModifiers::NONE);
    }

    #[test]
    fn test_key_combo_ctrl() {
        let combo = KeyCombo::ctrl(KeyCode::Char('c'));
        assert_eq!(combo.code, KeyCode::Char('c'));
        assert_eq!(combo.modifiers, KeyModifiers::CONTROL);
    }

    #[test]
    fn test_key_combo_shift() {
        let combo = KeyCombo::shift(KeyCode::Esc);
        assert_eq!(combo.code, KeyCode::Esc);
        assert_eq!(combo.modifiers, KeyModifiers::SHIFT);
    }

    #[test]
    fn test_key_combo_alt() {
        let combo = KeyCombo::alt(KeyCode::Char('p'));
        assert_eq!(combo.code, KeyCode::Char('p'));
        assert_eq!(combo.modifiers, KeyModifiers::ALT);
    }

    #[test]
    fn test_key_combo_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(KeyCombo::ctrl(KeyCode::Char('c')));
        set.insert(KeyCombo::plain(KeyCode::Enter));

        assert!(set.contains(&KeyCombo::ctrl(KeyCode::Char('c'))));
        assert!(set.contains(&KeyCombo::plain(KeyCode::Enter)));
        assert!(!set.contains(&KeyCombo::alt(KeyCode::Char('c'))));
    }

    #[test]
    fn test_default_config_has_global_bindings() {
        let config = KeybindingConfig::new();

        // Ctrl+C should quit
        let quit = config.get_global(&KeyCombo::ctrl(KeyCode::Char('c')));
        assert!(matches!(quit, Some(Command::Quit)));

        // Shift+N should create new thread
        let new_thread = config.get_global(&KeyCombo::shift(KeyCode::Char('N')));
        assert!(matches!(new_thread, Some(Command::CreateNewThread)));
    }

    #[test]
    fn test_default_config_has_modal_bindings() {
        let config = KeybindingConfig::new();

        // Folder picker escape should close
        let close = config.get_modal(ModalType::FolderPicker, &KeyCombo::plain(KeyCode::Esc));
        assert!(matches!(close, Some(Command::CloseFolderPicker)));

        // Thread switcher Tab should cycle forward
        let cycle = config.get_modal(ModalType::ThreadSwitcher, &KeyCombo::plain(KeyCode::Tab));
        assert!(matches!(cycle, Some(Command::CycleSwitcherForward)));
    }

    #[test]
    fn test_default_config_has_screen_bindings() {
        let config = KeybindingConfig::new();

        // Conversation PageUp should scroll
        let scroll = config.get_screen(Screen::Conversation, &KeyCombo::plain(KeyCode::PageUp));
        assert!(matches!(scroll, Some(Command::ScrollPageUp)));
    }

    #[test]
    fn test_default_config_has_focus_bindings() {
        let config = KeybindingConfig::new();

        // Threads panel Up should move up
        let move_up = config.get_focus(Focus::Threads, &KeyCombo::plain(KeyCode::Up));
        assert!(matches!(move_up, Some(Command::MoveUp)));

        // Threads panel q should quit
        let quit = config.get_focus(Focus::Threads, &KeyCombo::plain(KeyCode::Char('q')));
        assert!(matches!(quit, Some(Command::Quit)));
    }

    #[test]
    fn test_default_config_has_input_editing_bindings() {
        let config = KeybindingConfig::new();

        // Left arrow should move cursor left
        let left = config.get_input_editing(&KeyCombo::plain(KeyCode::Left));
        assert!(matches!(left, Some(Command::MoveCursorLeft)));

        // Alt+Backspace should delete word
        let delete_word = config.get_input_editing(&KeyCombo::alt(KeyCode::Backspace));
        assert!(matches!(delete_word, Some(Command::DeleteWordBackward)));

        // Enter should submit
        let submit = config.get_input_editing(&KeyCombo::plain(KeyCode::Enter));
        assert!(matches!(submit, Some(Command::SubmitInput(_))));
    }
}
