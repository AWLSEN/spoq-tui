use crate::cache::ThreadCache;
use crate::conductor::ConductorClient;
use crate::events::SseEvent;
use crate::models::StreamRequest;
use crate::state::{Task, Thread};
use crate::storage;
use crate::widgets::input_box::InputBox;
use color_eyre::Result;
use futures_util::StreamExt;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Messages received from async operations (streaming, connection status)
#[derive(Debug, Clone)]
pub enum AppMessage {
    /// A token received during streaming
    StreamToken { thread_id: String, token: String },
    /// Streaming completed successfully
    StreamComplete { thread_id: String, message_id: i64 },
    /// An error occurred during streaming
    StreamError { thread_id: String, error: String },
    /// Connection status changed
    ConnectionStatus(bool),
}

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
    /// Receiver for async messages (streaming tokens, connection status)
    pub message_rx: Option<mpsc::UnboundedReceiver<AppMessage>>,
    /// Sender for async messages (clone this to pass to async tasks)
    pub message_tx: mpsc::UnboundedSender<AppMessage>,
    /// Current connection status to the backend
    pub connection_status: bool,
    /// Last stream error for display
    pub stream_error: Option<String>,
    /// Conductor API client (shared across async tasks)
    pub client: Arc<ConductorClient>,
    /// Tick counter for animations (blinking cursor, etc.)
    pub tick_count: u64,
}

impl App {
    /// Create a new App instance
    pub fn new() -> Result<Self> {
        Self::with_client(Arc::new(ConductorClient::new()))
    }

    /// Create a new App instance with a custom ConductorClient
    pub fn with_client(client: Arc<ConductorClient>) -> Result<Self> {
        // Initialize storage directories
        storage::init_storage()?;

        // Load existing data
        let threads = storage::load_threads().unwrap_or_default();
        let tasks = storage::load_tasks().unwrap_or_default();

        // Initialize cache with stub data for development
        let cache = ThreadCache::with_stub_data();

        // Create the message channel for async communication
        let (message_tx, message_rx) = mpsc::unbounded_channel();

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
            message_rx: Some(message_rx),
            message_tx,
            connection_status: false,
            stream_error: None,
            client,
            tick_count: 0,
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

    /// Submit the current input, create a streaming thread, and spawn async API call
    pub fn submit_input(&mut self) {
        let content = self.input_box.content().to_string();
        if content.trim().is_empty() {
            return;
        }
        self.input_box.clear();

        // Build the stream request
        let request = StreamRequest::new(content.clone());

        // Create streaming thread in cache (with user message and placeholder assistant message)
        let thread_id = self.cache.create_streaming_thread(content);

        // Navigate to conversation immediately
        self.active_thread_id = Some(thread_id.clone());
        self.screen = Screen::Conversation;

        // Clone what we need for the async task
        let client = Arc::clone(&self.client);
        let message_tx = self.message_tx.clone();
        let thread_id_for_task = thread_id;

        // Spawn async task to stream the response
        tokio::spawn(async move {
            match client.stream(&request).await {
                Ok(mut stream) => {
                    while let Some(result) = stream.next().await {
                        match result {
                            Ok(event) => {
                                match event {
                                    SseEvent::Content(content_event) => {
                                        let _ = message_tx.send(AppMessage::StreamToken {
                                            thread_id: thread_id_for_task.clone(),
                                            token: content_event.text,
                                        });
                                    }
                                    SseEvent::Done(done_event) => {
                                        // Parse message_id from string to i64
                                        let message_id = done_event
                                            .message_id
                                            .parse::<i64>()
                                            .unwrap_or(0);
                                        let _ = message_tx.send(AppMessage::StreamComplete {
                                            thread_id: thread_id_for_task.clone(),
                                            message_id,
                                        });
                                        break;
                                    }
                                    SseEvent::Error(error_event) => {
                                        let _ = message_tx.send(AppMessage::StreamError {
                                            thread_id: thread_id_for_task.clone(),
                                            error: error_event.message,
                                        });
                                        break;
                                    }
                                    // Ignore other event types for now (reasoning, tool calls, etc.)
                                    _ => {}
                                }
                            }
                            Err(e) => {
                                let _ = message_tx.send(AppMessage::StreamError {
                                    thread_id: thread_id_for_task.clone(),
                                    error: e.to_string(),
                                });
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    let _ = message_tx.send(AppMessage::StreamError {
                        thread_id: thread_id_for_task,
                        error: e.to_string(),
                    });
                }
            }
        });
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

    /// Handle an incoming async message
    pub fn handle_message(&mut self, msg: AppMessage) {
        match msg {
            AppMessage::StreamToken { thread_id, token } => {
                self.cache.append_to_message(&thread_id, &token);
            }
            AppMessage::StreamComplete {
                thread_id,
                message_id,
            } => {
                self.cache.finalize_message(&thread_id, message_id);
            }
            AppMessage::StreamError { thread_id: _, error } => {
                self.stream_error = Some(error);
            }
            AppMessage::ConnectionStatus(connected) => {
                self.connection_status = connected;
                if connected {
                    // Clear any previous error when reconnected
                    self.stream_error = None;
                }
            }
        }
    }

    /// Get a clone of the message sender for passing to async tasks
    pub fn message_sender(&self) -> mpsc::UnboundedSender<AppMessage> {
        self.message_tx.clone()
    }

    /// Spawn an async task to check connection status.
    ///
    /// This calls the ConductorClient health_check and sends the result
    /// via the message channel. The App will update connection_status
    /// when the message is received.
    pub fn check_connection(&self) {
        let tx = self.message_tx.clone();
        let client = Arc::clone(&self.client);
        tokio::spawn(async move {
            let connected = match client.health_check().await {
                Ok(healthy) => healthy,
                Err(_) => false,
            };
            let _ = tx.send(AppMessage::ConnectionStatus(connected));
        });
    }

    /// Clear the current stream error
    pub fn clear_error(&mut self) {
        self.stream_error = None;
    }

    /// Increment the tick counter for animations
    pub fn tick(&mut self) {
        self.tick_count = self.tick_count.wrapping_add(1);
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
    use crate::models::MessageRole;

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

    #[tokio::test]
    async fn test_submit_input_creates_thread_and_navigates() {
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

    #[tokio::test]
    async fn test_submit_input_adds_messages_to_thread() {
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
        // Should have user message and streaming assistant placeholder
        assert_eq!(messages.len(), 2);

        // First message should be the user's input
        assert_eq!(messages[0].role, MessageRole::User);
        assert_eq!(messages[0].content, "Test");

        // Second message should be the streaming assistant placeholder
        assert_eq!(messages[1].role, MessageRole::Assistant);
        assert!(messages[1].is_streaming);
        assert!(messages[1].content.is_empty());
    }

    #[tokio::test]
    async fn test_submit_input_creates_thread_at_front() {
        let mut app = App::default();
        app.input_box.insert_char('N');
        app.input_box.insert_char('e');
        app.input_box.insert_char('w');

        app.submit_input();

        let thread_id = app.active_thread_id.as_ref().unwrap();
        // The new thread should be at the front of the list
        assert_eq!(app.cache.threads()[0].id, *thread_id);
    }

    #[test]
    fn test_handle_message_stream_token() {
        let mut app = App::default();
        // Create a streaming thread first
        let thread_id = app.cache.create_streaming_thread("Test".to_string());

        // Send a stream token
        app.handle_message(AppMessage::StreamToken {
            thread_id: thread_id.clone(),
            token: "Hello".to_string(),
        });

        // Verify the token was appended
        let messages = app.cache.get_messages(&thread_id).unwrap();
        let assistant_msg = messages.iter().find(|m| m.role == MessageRole::Assistant).unwrap();
        assert!(assistant_msg.content.contains("Hello") || assistant_msg.partial_content.contains("Hello"));
    }

    #[test]
    fn test_handle_message_stream_complete() {
        let mut app = App::default();
        // Create a streaming thread first
        let thread_id = app.cache.create_streaming_thread("Test".to_string());

        // Append some tokens
        app.handle_message(AppMessage::StreamToken {
            thread_id: thread_id.clone(),
            token: "Response".to_string(),
        });

        // Complete the stream
        app.handle_message(AppMessage::StreamComplete {
            thread_id: thread_id.clone(),
            message_id: 42,
        });

        // Verify the message was finalized with correct ID
        let messages = app.cache.get_messages(&thread_id).unwrap();
        let assistant_msg = messages.iter().find(|m| m.role == MessageRole::Assistant).unwrap();
        assert_eq!(assistant_msg.id, 42);
        assert!(!assistant_msg.is_streaming);
    }

    #[test]
    fn test_handle_message_stream_error() {
        let mut app = App::default();

        // Send a stream error
        app.handle_message(AppMessage::StreamError {
            thread_id: "thread-001".to_string(),
            error: "Connection failed".to_string(),
        });

        // Verify the error was stored
        assert!(app.stream_error.is_some());
        assert_eq!(app.stream_error.as_ref().unwrap(), "Connection failed");
    }

    #[test]
    fn test_handle_message_connection_status_connected() {
        let mut app = App::default();
        app.stream_error = Some("Previous error".to_string());
        assert!(!app.connection_status);

        // Send connection status update
        app.handle_message(AppMessage::ConnectionStatus(true));

        // Verify status updated and error cleared
        assert!(app.connection_status);
        assert!(app.stream_error.is_none());
    }

    #[test]
    fn test_handle_message_connection_status_disconnected() {
        let mut app = App::default();
        app.connection_status = true;

        // Send disconnection status
        app.handle_message(AppMessage::ConnectionStatus(false));

        // Verify status updated (error not cleared on disconnect)
        assert!(!app.connection_status);
    }

    #[test]
    fn test_message_sender_returns_clone() {
        let app = App::default();
        let _sender = app.message_sender();
        // Just verify it compiles and returns without panic
        // The sender should be usable for sending messages
    }

    #[test]
    fn test_clear_error() {
        let mut app = App::default();
        app.stream_error = Some("Test error".to_string());

        app.clear_error();

        assert!(app.stream_error.is_none());
    }

    #[test]
    fn test_clear_error_when_no_error() {
        let mut app = App::default();
        assert!(app.stream_error.is_none());

        app.clear_error();

        assert!(app.stream_error.is_none());
    }
}
