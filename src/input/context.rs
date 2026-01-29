//! Input context for determining which commands are available.
//!
//! The [`InputContext`] captures the current application state relevant to
//! input handling, allowing the command registry to dispatch appropriate
//! commands based on the current modal, focus, and screen.

use crate::app::{Focus, Screen};

/// The type of modal dialog currently active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ModalType {
    /// No modal dialog is active
    #[default]
    None,
    /// Folder picker is visible (CommandDeck)
    FolderPicker,
    /// File picker is visible (Conversation screen)
    FilePicker,
    /// Slash command autocomplete is visible
    SlashAutocomplete,
    /// Thread switcher is visible
    ThreadSwitcher,
    /// Permission prompt is pending
    Permission,
    /// Plan approval prompt is pending (ExitPlanMode converted to plan approval)
    PlanApproval,
    /// AskUserQuestion prompt is active (session-level, inline)
    AskUserQuestion,
    /// AskUserQuestion "Other" text input mode (session-level)
    AskUserQuestionOther,
    /// Dashboard question overlay is open
    DashboardQuestionOverlay,
    /// Dashboard question overlay "Other" text input mode
    DashboardQuestionOverlayOther,
    /// Claude login dialog is active
    ClaudeLogin,
}

/// Context information for input handling.
///
/// This struct captures all the state needed to determine which commands
/// are available and how key events should be interpreted.
#[derive(Debug, Clone)]
pub struct InputContext {
    /// Current screen (CommandDeck or Conversation)
    pub screen: Screen,
    /// Current focus (Threads or Input)
    pub focus: Focus,
    /// Current modal type (if any)
    pub modal: ModalType,
    /// Whether input is empty (affects Escape behavior)
    pub input_is_empty: bool,
    /// Whether cursor is on first line (affects Up arrow behavior)
    pub cursor_on_first_line: bool,
    /// Whether cursor is on last line (affects Down arrow behavior)
    pub cursor_on_last_line: bool,
    /// Whether history navigation is active
    pub is_navigating_history: bool,
    /// Whether there's an OAuth URL to open
    pub has_oauth_url: bool,
    /// Whether there are errors to dismiss
    pub has_errors: bool,
    /// Current line content (for folder picker trigger detection)
    pub current_line_content: String,
    /// Cursor column position
    pub cursor_column: usize,
}

impl Default for InputContext {
    fn default() -> Self {
        Self {
            screen: Screen::default(),
            focus: Focus::default(),
            modal: ModalType::None,
            input_is_empty: true,
            cursor_on_first_line: true,
            cursor_on_last_line: true,
            is_navigating_history: false,
            has_oauth_url: false,
            has_errors: false,
            current_line_content: String::new(),
            cursor_column: 0,
        }
    }
}

impl InputContext {
    /// Creates a new InputContext with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder method to set the screen.
    pub fn with_screen(mut self, screen: Screen) -> Self {
        self.screen = screen;
        self
    }

    /// Builder method to set the focus.
    pub fn with_focus(mut self, focus: Focus) -> Self {
        self.focus = focus;
        self
    }

    /// Builder method to set the modal type.
    pub fn with_modal(mut self, modal: ModalType) -> Self {
        self.modal = modal;
        self
    }

    /// Builder method to set input empty state.
    pub fn with_input_empty(mut self, is_empty: bool) -> Self {
        self.input_is_empty = is_empty;
        self
    }

    /// Builder method to set cursor position flags.
    pub fn with_cursor_position(mut self, on_first: bool, on_last: bool) -> Self {
        self.cursor_on_first_line = on_first;
        self.cursor_on_last_line = on_last;
        self
    }

    /// Builder method to set history navigation state.
    pub fn with_history_navigation(mut self, is_navigating: bool) -> Self {
        self.is_navigating_history = is_navigating;
        self
    }

    /// Builder method to set OAuth URL state.
    pub fn with_oauth_url(mut self, has_url: bool) -> Self {
        self.has_oauth_url = has_url;
        self
    }

    /// Builder method to set error state.
    pub fn with_errors(mut self, has_errors: bool) -> Self {
        self.has_errors = has_errors;
        self
    }

    /// Builder method to set current line content and cursor column.
    pub fn with_line_context(mut self, content: String, column: usize) -> Self {
        self.current_line_content = content;
        self.cursor_column = column;
        self
    }

    /// Returns true if input has focus.
    pub fn is_input_focused(&self) -> bool {
        self.focus == Focus::Input
    }

    /// Returns true if we're in a modal state.
    pub fn is_modal_active(&self) -> bool {
        self.modal != ModalType::None
    }

    /// Returns true if we're on the conversation screen.
    pub fn is_conversation_screen(&self) -> bool {
        self.screen == Screen::Conversation
    }

    /// Returns true if we're on the command deck screen.
    pub fn is_command_deck_screen(&self) -> bool {
        self.screen == Screen::CommandDeck
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_context_default() {
        let ctx = InputContext::default();
        assert_eq!(ctx.screen, Screen::CommandDeck);
        assert_eq!(ctx.focus, Focus::Threads);
        assert_eq!(ctx.modal, ModalType::None);
        assert!(ctx.input_is_empty);
        assert!(ctx.cursor_on_first_line);
        assert!(ctx.cursor_on_last_line);
        assert!(!ctx.is_navigating_history);
        assert!(!ctx.has_oauth_url);
        assert!(!ctx.has_errors);
    }

    #[test]
    fn test_input_context_builder() {
        let ctx = InputContext::new()
            .with_screen(Screen::Conversation)
            .with_focus(Focus::Input)
            .with_modal(ModalType::FolderPicker)
            .with_input_empty(false)
            .with_cursor_position(false, true)
            .with_history_navigation(true)
            .with_oauth_url(true)
            .with_errors(true);

        assert_eq!(ctx.screen, Screen::Conversation);
        assert_eq!(ctx.focus, Focus::Input);
        assert_eq!(ctx.modal, ModalType::FolderPicker);
        assert!(!ctx.input_is_empty);
        assert!(!ctx.cursor_on_first_line);
        assert!(ctx.cursor_on_last_line);
        assert!(ctx.is_navigating_history);
        assert!(ctx.has_oauth_url);
        assert!(ctx.has_errors);
    }

    #[test]
    fn test_is_input_focused() {
        let ctx = InputContext::new().with_focus(Focus::Input);
        assert!(ctx.is_input_focused());

        let ctx = InputContext::new().with_focus(Focus::Threads);
        assert!(!ctx.is_input_focused());
    }

    #[test]
    fn test_is_modal_active() {
        let ctx = InputContext::new();
        assert!(!ctx.is_modal_active());

        let ctx = InputContext::new().with_modal(ModalType::FolderPicker);
        assert!(ctx.is_modal_active());

        let ctx = InputContext::new().with_modal(ModalType::ThreadSwitcher);
        assert!(ctx.is_modal_active());
    }

    #[test]
    fn test_screen_helpers() {
        let ctx = InputContext::new().with_screen(Screen::Conversation);
        assert!(ctx.is_conversation_screen());
        assert!(!ctx.is_command_deck_screen());

        let ctx = InputContext::new().with_screen(Screen::CommandDeck);
        assert!(!ctx.is_conversation_screen());
        assert!(ctx.is_command_deck_screen());
    }

    #[test]
    fn test_modal_type_default() {
        assert_eq!(ModalType::default(), ModalType::None);
    }

    #[test]
    fn test_with_line_context() {
        let ctx = InputContext::new().with_line_context("Hello @".to_string(), 7);
        assert_eq!(ctx.current_line_content, "Hello @");
        assert_eq!(ctx.cursor_column, 7);
    }

    #[test]
    fn test_dashboard_question_overlay_modal() {
        let ctx = InputContext::new().with_modal(ModalType::DashboardQuestionOverlay);
        assert!(ctx.is_modal_active());
        assert_eq!(ctx.modal, ModalType::DashboardQuestionOverlay);

        let ctx = InputContext::new().with_modal(ModalType::DashboardQuestionOverlayOther);
        assert!(ctx.is_modal_active());
        assert_eq!(ctx.modal, ModalType::DashboardQuestionOverlayOther);
    }
}
