use crate::cache::ThreadCache;
use crate::conductor::ConductorClient;
use crate::events::SseEvent;
use crate::models::StreamRequest;
use crate::state::{Task, Thread};
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
    /// Thread created on backend - reconcile pending ID with real ID
    ThreadCreated {
        pending_id: String,
        real_id: String,
        title: Option<String>,
    },
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
    /// Scroll position for conversation view (0 = bottom/latest content)
    pub conversation_scroll: u16,
}

impl App {
    /// Create a new App instance
    pub fn new() -> Result<Self> {
        Self::with_client(Arc::new(ConductorClient::new()))
    }

    /// Create a new App instance with a custom ConductorClient
    pub fn with_client(client: Arc<ConductorClient>) -> Result<Self> {
        // Initialize empty cache - will be populated by initialize()
        let cache = ThreadCache::new();

        // Create the message channel for async communication
        let (message_tx, message_rx) = mpsc::unbounded_channel();

        Ok(Self {
            // Start with empty vectors - will be populated from server in initialize()
            threads: Vec::new(),
            tasks: Vec::new(),
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
            conversation_scroll: 0,
        })
    }

    /// Initialize the app by fetching data from the backend.
    ///
    /// Fetches threads and tasks from the server. If the server is unreachable
    /// or returns an error, the app starts with empty state and sets connection
    /// status to false.
    pub async fn initialize(&mut self) {
        // Fetch threads from server
        match self.client.fetch_threads().await {
            Ok(threads) => {
                // Populate cache with threads from server
                for thread in threads {
                    self.cache.upsert_thread(thread);
                }
                self.connection_status = true;
            }
            Err(_) => {
                // Server unreachable - start with empty state
                self.connection_status = false;
            }
        }

        // Fetch tasks from server (only if connected)
        if self.connection_status {
            match self.client.fetch_tasks().await {
                Ok(tasks) => {
                    self.tasks = tasks;
                }
                Err(_) => {
                    // Failed to fetch tasks - continue with empty tasks
                    self.tasks = Vec::new();
                }
            }
        }
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

    /// Submit the current input, create a streaming thread, and spawn async API call.
    ///
    /// This handles two distinct cases:
    /// 1. NEW thread: When `active_thread_id` is None, creates a new pending thread
    /// 2. CONTINUING thread: When `active_thread_id` exists, adds to the existing thread
    ///
    /// Edge case: If active_thread_id starts with "pending-", we block submission
    /// because we're still waiting for the backend to confirm the thread ID.
    pub fn submit_input(&mut self) {
        let content = self.input_box.content().to_string();
        if content.trim().is_empty() {
            return;
        }

        // CRITICAL: Determine if this is a NEW thread or CONTINUING existing
        let (request, thread_id) = if let Some(existing_id) = &self.active_thread_id {
            // Check if thread is still pending (waiting for backend ThreadInfo)
            if existing_id.starts_with("pending-") {
                // Block rapid second message - still waiting for ThreadInfo
                self.stream_error = Some(
                    "Please wait for the current response to complete before sending another message."
                        .to_string(),
                );
                return;
            }

            // CONTINUING existing thread
            let request = StreamRequest::with_thread(content.clone(), existing_id.clone());
            if !self.cache.add_streaming_message(existing_id, content) {
                // Thread doesn't exist in cache - might have been deleted
                self.stream_error = Some("Thread no longer exists.".to_string());
                return;
            }
            (request, existing_id.clone())
        } else {
            // NEW thread - create pending, will reconcile when backend responds
            let request = StreamRequest::new(content.clone());
            let pending_id = self.cache.create_pending_thread(content);
            self.active_thread_id = Some(pending_id.clone());
            self.screen = Screen::Conversation;
            // Reset scroll for new conversation
            self.conversation_scroll = 0;
            (request, pending_id)
        };

        self.input_box.clear();

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
                                    SseEvent::UserMessageSaved(event) => {
                                        // ThreadInfo event mapped to UserMessageSaved in conductor.rs
                                        // This provides the real backend thread_id
                                        let _ = message_tx.send(AppMessage::ThreadCreated {
                                            pending_id: thread_id_for_task.clone(),
                                            real_id: event.thread_id,
                                            title: None, // Title not available in this event
                                        });
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

    /// Mark the app to quit
    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    /// Navigate back to the CommandDeck screen
    pub fn navigate_to_command_deck(&mut self) {
        self.screen = Screen::CommandDeck;
        self.active_thread_id = None;  // Clear so next submit creates new thread
        self.input_box.clear();        // Clear any partial input
    }

    /// Open a specific thread by ID for conversation
    pub fn open_thread(&mut self, thread_id: String) {
        // Set active thread
        self.active_thread_id = Some(thread_id);

        // Navigate to conversation
        self.screen = Screen::Conversation;

        // Clear input box for fresh start
        self.input_box.clear();

        // Reset scroll to show latest content
        self.conversation_scroll = 0;

        // Messages should already be in cache from backend fetch
    }

    /// Open the currently selected thread from the threads panel
    pub fn open_selected_thread(&mut self) {
        let threads = self.cache.threads();

        // Check if selection is beyond thread list (e.g., "New Thread" button)
        if self.threads_index >= threads.len() {
            // No valid thread selected, just focus input
            self.focus = Focus::Input;
            return;
        }

        let thread_id = threads[self.threads_index].id.clone();
        self.open_thread(thread_id);
    }

    /// Handle an incoming async message
    pub fn handle_message(&mut self, msg: AppMessage) {
        match msg {
            AppMessage::StreamToken { thread_id, token } => {
                self.cache.append_to_message(&thread_id, &token);
                // Auto-scroll to bottom when new content arrives, but only for the active thread
                if self.active_thread_id.as_ref() == Some(&thread_id) {
                    self.conversation_scroll = 0;
                }
            }
            AppMessage::StreamComplete {
                thread_id,
                message_id,
            } => {
                self.cache.finalize_message(&thread_id, message_id);
                // Auto-scroll to bottom when stream completes, but only for the active thread
                if self.active_thread_id.as_ref() == Some(&thread_id) {
                    self.conversation_scroll = 0;
                }
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
            AppMessage::ThreadCreated {
                pending_id,
                real_id,
                title,
            } => {
                // Reconcile the pending local thread ID with the real backend ID
                self.cache
                    .reconcile_thread_id(&pending_id, &real_id, title);
                // Update active_thread_id if it matches the pending ID
                if self.active_thread_id.as_ref() == Some(&pending_id) {
                    self.active_thread_id = Some(real_id);
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
        app.active_thread_id = Some("thread-123".to_string());
        app.input_box.insert_char('T');
        app.input_box.insert_char('e');
        app.input_box.insert_char('s');
        app.input_box.insert_char('t');

        app.navigate_to_command_deck();

        assert_eq!(app.screen, Screen::CommandDeck);
        assert!(app.active_thread_id.is_none());
        assert!(app.input_box.is_empty());
    }

    #[test]
    fn test_navigate_to_command_deck_when_already_on_command_deck() {
        let mut app = App::default();
        assert_eq!(app.screen, Screen::CommandDeck);
        app.active_thread_id = Some("thread-456".to_string());
        app.input_box.insert_char('H');
        app.input_box.insert_char('i');

        app.navigate_to_command_deck();

        assert_eq!(app.screen, Screen::CommandDeck);
        assert!(app.active_thread_id.is_none());
        assert!(app.input_box.is_empty());
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
        // Should have an active thread ID that starts with "pending-"
        assert!(app.active_thread_id.is_some());
        assert!(app.active_thread_id.as_ref().unwrap().starts_with("pending-"));
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
    async fn test_submit_input_creates_pending_thread_at_front() {
        let mut app = App::default();
        app.input_box.insert_char('N');
        app.input_box.insert_char('e');
        app.input_box.insert_char('w');

        app.submit_input();

        let thread_id = app.active_thread_id.as_ref().unwrap();
        // The new thread should be at the front of the list and have pending- prefix
        assert_eq!(app.cache.threads()[0].id, *thread_id);
        assert!(thread_id.starts_with("pending-"));
    }

    // ============= New Thread vs Continuing Thread Tests =============

    #[tokio::test]
    async fn test_submit_input_new_thread_when_no_active_thread() {
        let mut app = App::default();
        assert!(app.active_thread_id.is_none());
        app.input_box.insert_char('H');
        app.input_box.insert_char('i');

        app.submit_input();

        // Should create a pending thread
        let thread_id = app.active_thread_id.as_ref().unwrap();
        assert!(thread_id.starts_with("pending-"));
        // Should navigate to conversation
        assert_eq!(app.screen, Screen::Conversation);
    }

    #[tokio::test]
    async fn test_submit_input_continues_existing_thread() {
        let mut app = App::default();

        // Create an existing thread with a real (non-pending) ID
        let existing_id = "real-thread-123".to_string();
        app.cache.upsert_thread(crate::models::Thread {
            id: existing_id.clone(),
            title: "Existing Thread".to_string(),
            preview: "Previous message".to_string(),
            updated_at: chrono::Utc::now(),
        });
        app.cache.add_message_simple(&existing_id, MessageRole::User, "Previous question".to_string());
        app.cache.add_message_simple(&existing_id, MessageRole::Assistant, "Previous answer".to_string());

        // Set as active thread
        app.active_thread_id = Some(existing_id.clone());
        app.screen = Screen::Conversation;

        let initial_msg_count = app.cache.get_messages(&existing_id).unwrap().len();

        // Submit follow-up
        app.input_box.insert_char('F');
        app.input_box.insert_char('o');
        app.input_box.insert_char('l');
        app.input_box.insert_char('l');
        app.input_box.insert_char('o');
        app.input_box.insert_char('w');
        app.submit_input();

        // Should NOT create a new thread
        assert_eq!(app.active_thread_id.as_ref().unwrap(), &existing_id);
        // Should add messages to existing thread
        let messages = app.cache.get_messages(&existing_id).unwrap();
        assert_eq!(messages.len(), initial_msg_count + 2); // +1 user, +1 streaming assistant

        // Last user message should be our follow-up
        let user_msgs: Vec<_> = messages.iter().filter(|m| m.role == MessageRole::User).collect();
        assert_eq!(user_msgs.last().unwrap().content, "Follow");
    }

    #[tokio::test]
    async fn test_submit_input_blocks_rapid_submit_on_pending_thread() {
        let mut app = App::default();

        // First submit creates pending thread
        app.input_box.insert_char('F');
        app.input_box.insert_char('i');
        app.input_box.insert_char('r');
        app.input_box.insert_char('s');
        app.input_box.insert_char('t');
        app.submit_input();

        let pending_id = app.active_thread_id.clone().unwrap();
        assert!(pending_id.starts_with("pending-"));

        // Try to submit again while still pending
        app.input_box.insert_char('S');
        app.input_box.insert_char('e');
        app.input_box.insert_char('c');
        app.input_box.insert_char('o');
        app.input_box.insert_char('n');
        app.input_box.insert_char('d');
        app.submit_input();

        // Should NOT create a new thread or add messages
        // Should set an error
        assert!(app.stream_error.is_some());
        assert!(app.stream_error.as_ref().unwrap().contains("wait"));

        // Input should NOT be cleared (submission was rejected)
        assert!(!app.input_box.is_empty());
        assert_eq!(app.input_box.content(), "Second");

        // Should still be on the pending thread
        assert_eq!(app.active_thread_id, Some(pending_id));
    }

    #[tokio::test]
    async fn test_submit_input_allows_submit_after_thread_reconciled() {
        let mut app = App::default();

        // First submit creates pending thread
        app.input_box.insert_char('F');
        app.input_box.insert_char('i');
        app.input_box.insert_char('r');
        app.input_box.insert_char('s');
        app.input_box.insert_char('t');
        app.submit_input();

        let pending_id = app.active_thread_id.clone().unwrap();

        // Simulate backend responding with ThreadCreated
        app.handle_message(AppMessage::ThreadCreated {
            pending_id: pending_id.clone(),
            real_id: "real-backend-id".to_string(),
            title: None,
        });

        // Verify reconciliation happened
        assert_eq!(app.active_thread_id, Some("real-backend-id".to_string()));

        // Finalize the first response
        app.cache.append_to_message("real-backend-id", "First response");
        app.cache.finalize_message("real-backend-id", 1);

        // Now second submit should work
        app.input_box.insert_char('S');
        app.input_box.insert_char('e');
        app.input_box.insert_char('c');
        app.input_box.insert_char('o');
        app.input_box.insert_char('n');
        app.input_box.insert_char('d');
        let before_count = app.cache.get_messages("real-backend-id").unwrap().len();
        app.submit_input();

        // Should add to existing thread
        let messages = app.cache.get_messages("real-backend-id").unwrap();
        assert_eq!(messages.len(), before_count + 2);
        assert!(app.input_box.is_empty());
    }

    #[tokio::test]
    async fn test_submit_input_handles_deleted_thread() {
        let mut app = App::default();

        // Set active thread to non-existent (simulates deleted thread)
        app.active_thread_id = Some("deleted-thread".to_string());
        app.screen = Screen::Conversation;

        app.input_box.insert_char('T');
        app.input_box.insert_char('e');
        app.input_box.insert_char('s');
        app.input_box.insert_char('t');
        app.submit_input();

        // Should show error about thread not existing
        assert!(app.stream_error.is_some());
        assert!(app.stream_error.as_ref().unwrap().contains("no longer exists"));

        // Input should NOT be cleared
        assert!(!app.input_box.is_empty());
    }

    #[tokio::test]
    async fn test_submit_input_full_conversation_workflow() {
        let mut app = App::default();

        // === Turn 1: New thread ===
        app.input_box.insert_char('H');
        app.input_box.insert_char('i');
        app.submit_input();

        let pending_id = app.active_thread_id.clone().unwrap();
        assert!(pending_id.starts_with("pending-"));
        assert_eq!(app.screen, Screen::Conversation);

        // Simulate backend response
        app.handle_message(AppMessage::ThreadCreated {
            pending_id: pending_id.clone(),
            real_id: "thread-abc".to_string(),
            title: Some("Greeting".to_string()),
        });
        app.cache.append_to_message("thread-abc", "Hello! How can I help?");
        app.cache.finalize_message("thread-abc", 100);

        assert_eq!(app.active_thread_id, Some("thread-abc".to_string()));

        // === Turn 2: Continue thread ===
        app.input_box.insert_char('T');
        app.input_box.insert_char('e');
        app.input_box.insert_char('l');
        app.input_box.insert_char('l');
        app.input_box.insert_char(' ');
        app.input_box.insert_char('m');
        app.input_box.insert_char('e');
        app.submit_input();

        // Should still be on same thread
        assert_eq!(app.active_thread_id, Some("thread-abc".to_string()));

        // Should have 4 messages: user1, assistant1, user2, assistant2(streaming)
        let messages = app.cache.get_messages("thread-abc").unwrap();
        assert_eq!(messages.len(), 4);
        assert_eq!(messages[2].content, "Tell me");
        assert!(messages[3].is_streaming);

        // === Navigate away and back ===
        app.navigate_to_command_deck();
        assert!(app.active_thread_id.is_none());
        assert_eq!(app.screen, Screen::CommandDeck);

        // === Turn 3: New thread after navigating away ===
        app.input_box.insert_char('N');
        app.input_box.insert_char('e');
        app.input_box.insert_char('w');
        app.submit_input();

        // Should be a NEW pending thread
        let new_pending = app.active_thread_id.clone().unwrap();
        assert!(new_pending.starts_with("pending-"));
        assert_ne!(new_pending, "thread-abc");

        // Cache should have both threads
        assert!(app.cache.get_thread("thread-abc").is_some());
        assert!(app.cache.get_thread(&new_pending).is_some());
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

    // ============= ThreadCreated Message Tests =============

    #[test]
    fn test_handle_message_thread_created_reconciles_cache() {
        let mut app = App::default();
        // Create a streaming thread first
        let pending_id = app.cache.create_streaming_thread("Test".to_string());

        // Handle ThreadCreated message
        app.handle_message(AppMessage::ThreadCreated {
            pending_id: pending_id.clone(),
            real_id: "real-backend-id".to_string(),
            title: None,
        });

        // Verify the thread was reconciled
        assert!(app.cache.get_thread(&pending_id).is_none());
        assert!(app.cache.get_thread("real-backend-id").is_some());
    }

    #[test]
    fn test_handle_message_thread_created_updates_active_thread() {
        let mut app = App::default();
        // Create a streaming thread and set it as active
        let pending_id = app.cache.create_streaming_thread("Test".to_string());
        app.active_thread_id = Some(pending_id.clone());

        // Handle ThreadCreated message
        app.handle_message(AppMessage::ThreadCreated {
            pending_id,
            real_id: "real-backend-id".to_string(),
            title: None,
        });

        // Verify active_thread_id was updated
        assert_eq!(app.active_thread_id, Some("real-backend-id".to_string()));
    }

    #[test]
    fn test_handle_message_thread_created_does_not_update_different_active_thread() {
        let mut app = App::default();
        // Create a streaming thread
        let pending_id = app.cache.create_streaming_thread("Test".to_string());
        // Set a different thread as active
        app.active_thread_id = Some("different-thread".to_string());

        // Handle ThreadCreated message
        app.handle_message(AppMessage::ThreadCreated {
            pending_id,
            real_id: "real-backend-id".to_string(),
            title: None,
        });

        // Verify active_thread_id was NOT updated (it's different)
        assert_eq!(app.active_thread_id, Some("different-thread".to_string()));
    }

    #[test]
    fn test_handle_message_thread_created_with_title() {
        let mut app = App::default();
        let pending_id = app.cache.create_streaming_thread("Original".to_string());

        app.handle_message(AppMessage::ThreadCreated {
            pending_id,
            real_id: "real-backend-id".to_string(),
            title: Some("New Title from Backend".to_string()),
        });

        let thread = app.cache.get_thread("real-backend-id").unwrap();
        assert_eq!(thread.title, "New Title from Backend");
    }

    #[test]
    fn test_handle_message_thread_created_messages_accessible_by_new_id() {
        let mut app = App::default();
        let pending_id = app.cache.create_streaming_thread("Test".to_string());

        // Append some tokens
        app.cache.append_to_message(&pending_id, "Response content");

        // Handle ThreadCreated message
        app.handle_message(AppMessage::ThreadCreated {
            pending_id: pending_id.clone(),
            real_id: "real-backend-id".to_string(),
            title: None,
        });

        // Messages should be accessible by the new ID
        let messages = app.cache.get_messages("real-backend-id");
        assert!(messages.is_some());
        let messages = messages.unwrap();
        assert_eq!(messages.len(), 2);

        // All messages should have the new thread_id
        for msg in messages {
            assert_eq!(msg.thread_id, "real-backend-id");
        }
    }

    // ============= Initialize Tests =============

    #[tokio::test]
    async fn test_initialize_sets_connection_status() {
        // Initialize just sets connection_status to true
        // (Thread loading is skipped since /v1/messages/recent returns messages not threads)
        let client = Arc::new(ConductorClient::with_base_url("http://127.0.0.1:1".to_string()));
        let mut app = App::with_client(client).unwrap();

        // Connection status should start as false
        assert!(!app.connection_status);

        app.initialize().await;

        // After initialization, connection_status should be true
        assert!(app.connection_status);
    }

    #[tokio::test]
    async fn test_initialize_starts_with_empty_cache() {
        let client = Arc::new(ConductorClient::with_base_url("http://127.0.0.1:1".to_string()));
        let app = App::with_client(client).unwrap();

        // Cache should start empty (no stub data)
        assert_eq!(app.cache.thread_count(), 0);
    }

    // ============= Scroll Behavior Tests =============

    #[test]
    fn test_stream_token_does_not_reset_scroll_for_non_active_thread() {
        let mut app = App::default();

        // Create two threads
        let thread1_id = app.cache.create_streaming_thread("Thread 1".to_string());
        let thread2_id = app.cache.create_streaming_thread("Thread 2".to_string());

        // Set thread 1 as active
        app.active_thread_id = Some(thread1_id.clone());

        // Set conversation scroll to a non-zero value (user has scrolled up)
        app.conversation_scroll = 5;

        // Receive token for thread 2 (non-active thread)
        app.handle_message(AppMessage::StreamToken {
            thread_id: thread2_id.clone(),
            token: "Hello from thread 2".to_string(),
        });

        // Scroll should NOT be reset (should still be 5)
        assert_eq!(app.conversation_scroll, 5);
    }

    #[test]
    fn test_stream_token_resets_scroll_for_active_thread() {
        let mut app = App::default();

        // Create a thread and set it as active
        let thread_id = app.cache.create_streaming_thread("Active thread".to_string());
        app.active_thread_id = Some(thread_id.clone());

        // Set conversation scroll to a non-zero value
        app.conversation_scroll = 10;

        // Receive token for the active thread
        app.handle_message(AppMessage::StreamToken {
            thread_id: thread_id.clone(),
            token: "Hello".to_string(),
        });

        // Scroll should be reset to 0 (auto-scroll to bottom)
        assert_eq!(app.conversation_scroll, 0);
    }

    #[test]
    fn test_stream_complete_does_not_reset_scroll_for_non_active_thread() {
        let mut app = App::default();

        // Create two threads
        let thread1_id = app.cache.create_streaming_thread("Thread 1".to_string());
        let thread2_id = app.cache.create_streaming_thread("Thread 2".to_string());

        // Set thread 1 as active
        app.active_thread_id = Some(thread1_id.clone());

        // Set conversation scroll to a non-zero value
        app.conversation_scroll = 7;

        // Complete stream for thread 2 (non-active thread)
        app.handle_message(AppMessage::StreamComplete {
            thread_id: thread2_id.clone(),
            message_id: 42,
        });

        // Scroll should NOT be reset
        assert_eq!(app.conversation_scroll, 7);
    }

    #[test]
    fn test_stream_complete_resets_scroll_for_active_thread() {
        let mut app = App::default();

        // Create a thread and set it as active
        let thread_id = app.cache.create_streaming_thread("Active thread".to_string());
        app.active_thread_id = Some(thread_id.clone());

        // Set conversation scroll to a non-zero value
        app.conversation_scroll = 15;

        // Complete stream for the active thread
        app.handle_message(AppMessage::StreamComplete {
            thread_id: thread_id.clone(),
            message_id: 99,
        });

        // Scroll should be reset to 0
        assert_eq!(app.conversation_scroll, 0);
    }
}
