use crate::cache::ThreadCache;
use crate::models::{MessageRole, StreamRequest};
use crate::state::{Task, Thread};
use crate::storage;
use crate::widgets::input_box::InputBox;
use color_eyre::Result;

/// Represents which screen is currently active
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Screen {
    #[default]
    CommandDeck,
    Conversation,
}

/// Represents which UI component has focus
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Focus {
    Notifications,
    Tasks,
    #[default]
    Threads,
    Input,
}

/// Main application state
#[derive(Debug)]
pub struct App {
    /// List of conversation threads (legacy - for storage compatibility)
    pub threads: Vec<Thread>,
    /// List of tasks
    pub tasks: Vec<Task>,
    /// Flag to track if the app should quit
    pub should_quit: bool,
    /// Current screen being displayed
    pub screen: Screen,
    /// ID of the active thread when in Conversation screen
    pub active_thread_id: Option<String>,
    /// Current focus panel
    pub focus: Focus,
    /// Selected index in notifications panel
    pub notifications_index: usize,
    /// Selected index in tasks panel
    pub tasks_index: usize,
    /// Selected index in threads panel
    pub threads_index: usize,
    /// Input box state
    pub input_box: InputBox,
    /// Migration/indexing progress (0-100), None when complete
    pub migration_progress: Option<u8>,
    /// Thread and message cache
    pub cache: ThreadCache,
}

impl App {
    /// Create a new App instance
    pub fn new() -> Result<Self> {
        // Initialize storage directories
        storage::init_storage()?;

        // Load existing data
        let threads = storage::load_threads().unwrap_or_default();
        let tasks = storage::load_tasks().unwrap_or_default();

        // Initialize cache with stub data for development
        let cache = ThreadCache::with_stub_data();

        Ok(Self {
            threads,
            tasks,
            should_quit: false,
            screen: Screen::default(),
            active_thread_id: None,
            focus: Focus::default(),
            notifications_index: 0,
            tasks_index: 0,
            threads_index: 0,
            input_box: InputBox::new(),
            migration_progress: Some(0),
            cache,
        })
    }

    /// Cycle focus to the next panel
    pub fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Notifications => Focus::Tasks,
            Focus::Tasks => Focus::Threads,
            Focus::Threads => Focus::Input,
            Focus::Input => Focus::Notifications,
        };
    }

    /// Move selection up in the current focused panel
    pub fn move_up(&mut self) {
        match self.focus {
            Focus::Notifications => {
                if self.notifications_index > 0 {
                    self.notifications_index -= 1;
                }
            }
            Focus::Tasks => {
                if self.tasks_index > 0 {
                    self.tasks_index -= 1;
                }
            }
            Focus::Threads => {
                if self.threads_index > 0 {
                    self.threads_index -= 1;
                }
            }
            Focus::Input => {}
        }
    }

    /// Move selection down in the current focused panel
    pub fn move_down(&mut self, max_notifications: usize, max_tasks: usize, max_threads: usize) {
        match self.focus {
            Focus::Notifications => {
                if max_notifications > 0 && self.notifications_index < max_notifications - 1 {
                    self.notifications_index += 1;
                }
            }
            Focus::Tasks => {
                if max_tasks > 0 && self.tasks_index < max_tasks - 1 {
                    self.tasks_index += 1;
                }
            }
            Focus::Threads => {
                if max_threads > 0 && self.threads_index < max_threads - 1 {
                    self.threads_index += 1;
                }
            }
            Focus::Input => {}
        }
    }

    /// Create a new thread placeholder
    pub fn create_new_thread(&mut self) {
        use crate::state::Thread;
        use chrono::Utc;

        let new_thread = Thread {
            id: format!("thread-{}", self.threads.len() + 1),
            title: "New Thread".to_string(),
            preview: "No messages yet...".to_string(),
            created_at: Utc::now(),
        };
        self.threads.insert(0, new_thread);
        self.threads_index = 0;
        self.focus = Focus::Threads;
    }

    /// Submit the current input, create a thread (stubbed), and navigate to conversation
    pub fn submit_input(&mut self) {
        let content = self.input_box.content().to_string();
        if content.trim().is_empty() {
            return;
        }
        self.input_box.clear();

        // Build request (for future backend call)
        let _request = StreamRequest::new(content.clone());

        // STUB: Create thread locally (will be replaced by backend call)
        let thread_id = self.cache.create_stub_thread(content.clone());

        // STUB: Add user message
        self.cache
            .add_message_simple(&thread_id, MessageRole::User, content);

        // STUB: Add AI response
        self.cache.add_message_simple(
            &thread_id,
            MessageRole::Assistant,
            "This is a stub response. Backend not connected.".into(),
        );

        // Navigate to conversation
        self.active_thread_id = Some(thread_id);
        self.screen = Screen::Conversation;
    }

    /// Save all data to storage
    pub fn save(&self) -> Result<()> {
        storage::save_threads(&self.threads)?;
        storage::save_tasks(&self.tasks)?;
        Ok(())
    }

    /// Mark the app to quit
    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    /// Navigate back to the CommandDeck screen
    pub fn navigate_to_command_deck(&mut self) {
        self.screen = Screen::CommandDeck;
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new().expect("Failed to create default App")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_screen_default_is_command_deck() {
        assert_eq!(Screen::default(), Screen::CommandDeck);
    }

    #[test]
    fn test_screen_equality() {
        assert_eq!(Screen::CommandDeck, Screen::CommandDeck);
        assert_eq!(Screen::Conversation, Screen::Conversation);
        assert_ne!(Screen::CommandDeck, Screen::Conversation);
    }

    #[test]
    fn test_screen_copy() {
        let screen = Screen::Conversation;
        let copied = screen;
        assert_eq!(screen, copied);
    }

    #[test]
    fn test_navigate_to_command_deck_from_conversation() {
        let mut app = App::default();
        app.screen = Screen::Conversation;
        app.navigate_to_command_deck();
        assert_eq!(app.screen, Screen::CommandDeck);
    }

    #[test]
    fn test_navigate_to_command_deck_when_already_on_command_deck() {
        let mut app = App::default();
        assert_eq!(app.screen, Screen::CommandDeck);
        app.navigate_to_command_deck();
        assert_eq!(app.screen, Screen::CommandDeck);
    }

    #[test]
    fn test_app_initializes_with_no_active_thread() {
        let app = App::default();
        assert!(app.active_thread_id.is_none());
    }

    #[test]
    fn test_app_initializes_on_command_deck() {
        let app = App::default();
        assert_eq!(app.screen, Screen::CommandDeck);
    }

    #[test]
    fn test_submit_input_with_empty_input_does_nothing() {
        let mut app = App::default();
        let initial_cache_count = app.cache.thread_count();

        app.submit_input();

        // Nothing should change with empty input
        assert_eq!(app.cache.thread_count(), initial_cache_count);
        assert_eq!(app.screen, Screen::CommandDeck);
        assert!(app.active_thread_id.is_none());
    }

    #[test]
    fn test_submit_input_with_whitespace_only_does_nothing() {
        let mut app = App::default();
        app.input_box.insert_char(' ');
        app.input_box.insert_char(' ');
        let initial_cache_count = app.cache.thread_count();

        app.submit_input();

        // Whitespace-only input should be ignored
        assert_eq!(app.cache.thread_count(), initial_cache_count);
        assert_eq!(app.screen, Screen::CommandDeck);
        assert!(app.active_thread_id.is_none());
    }

    #[test]
    fn test_submit_input_creates_thread_and_navigates() {
        let mut app = App::default();
        app.input_box.insert_char('H');
        app.input_box.insert_char('i');
        let initial_cache_count = app.cache.thread_count();

        app.submit_input();

        // Should create a new thread
        assert_eq!(app.cache.thread_count(), initial_cache_count + 1);
        // Should navigate to conversation screen
        assert_eq!(app.screen, Screen::Conversation);
        // Should have an active thread ID
        assert!(app.active_thread_id.is_some());
        // Input should be cleared
        assert!(app.input_box.is_empty());
    }

    #[test]
    fn test_submit_input_adds_messages_to_thread() {
        let mut app = App::default();
        app.input_box.insert_char('T');
        app.input_box.insert_char('e');
        app.input_box.insert_char('s');
        app.input_box.insert_char('t');

        app.submit_input();

        let thread_id = app.active_thread_id.as_ref().unwrap();
        let messages = app.cache.get_messages(thread_id);
        assert!(messages.is_some());

        let messages = messages.unwrap();
        // Should have user message and AI stub response
        assert_eq!(messages.len(), 2);

        // First message should be the user's input
        assert_eq!(messages[0].role, MessageRole::User);
        assert_eq!(messages[0].content, "Test");

        // Second message should be the AI stub
        assert_eq!(messages[1].role, MessageRole::Assistant);
        assert!(messages[1].content.contains("stub"));
    }

    #[test]
    fn test_submit_input_creates_thread_at_front() {
        let mut app = App::default();
        app.input_box.insert_char('N');
        app.input_box.insert_char('e');
        app.input_box.insert_char('w');

        app.submit_input();

        let thread_id = app.active_thread_id.as_ref().unwrap();
        // The new thread should be at the front of the list
        assert_eq!(app.cache.threads()[0].id, *thread_id);
    }
}
