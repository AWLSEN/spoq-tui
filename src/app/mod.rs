//! Application state and logic for the TUI.
//!
//! This module contains the core [`App`] struct and related types:
//! - [`Screen`] - Which screen is currently displayed
//! - [`Focus`] - Which UI component has focus
//! - [`ProgrammingMode`] - Current programming mode for Claude interactions
//! - [`AppMessage`] - Messages for async communication

mod handlers;
mod messages;
mod navigation;
mod permissions;
mod state_methods;
mod stream;

pub use messages::AppMessage;

use crate::cache::ThreadCache;
use crate::conductor::ConductorClient;
use crate::debug::{DebugEvent, DebugEventKind, DebugEventSender};
use crate::state::{SessionState, SubagentTracker, Task, Thread, Todo, ToolTracker};
use crate::widgets::input_box::InputBox;
use chrono::Utc;
use color_eyre::Result;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Truncate a string for debug output, adding "..." if truncated.
/// Uses char boundaries to avoid panicking on multi-byte UTF-8 characters.
pub(super) fn truncate_for_debug(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        // Find a valid char boundary at or before max_len - 3
        let target = max_len.saturating_sub(3);
        let boundary = s
            .char_indices()
            .take_while(|(i, _)| *i <= target)
            .last()
            .map(|(i, _)| i)
            .unwrap_or(0);
        format!("{}...", &s[..boundary])
    }
}

/// Log thread metadata updates to a dedicated file for debugging
pub(super) fn log_thread_update(message: &str) {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let log_path = format!("{}/spoq_thread.log", home);
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let _ = writeln!(file, "[{}] {}", timestamp, message);
        let _ = file.flush();
    }
}

/// Helper to emit a debug event if debug channel is available.
pub(super) fn emit_debug(
    debug_tx: &Option<DebugEventSender>,
    kind: DebugEventKind,
    thread_id: Option<&str>,
) {
    if let Some(ref tx) = debug_tx {
        let event = DebugEvent::with_context(kind, thread_id.map(String::from), None);
        let _ = tx.send(event);
    }
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

/// Represents the current programming mode for Claude interactions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProgrammingMode {
    /// Plan mode - Claude creates plans before executing
    PlanMode,
    /// Bypass permissions - skip confirmation prompts
    BypassPermissions,
    /// No special mode active
    #[default]
    None,
}

/// Thread switcher dialog state (Tab to open)
#[derive(Debug, Clone)]
pub struct ThreadSwitcher {
    /// Whether the thread switcher dialog is visible
    pub visible: bool,
    /// Currently selected index in the thread list (MRU order)
    pub selected_index: usize,
    /// Scroll offset for the thread list (first visible thread index)
    pub scroll_offset: usize,
    /// Timestamp of last navigation key press (for auto-confirm on release)
    pub last_nav_time: Option<std::time::Instant>,
}

impl Default for ThreadSwitcher {
    fn default() -> Self {
        Self {
            visible: false,
            selected_index: 0,
            scroll_offset: 0,
            last_nav_time: None,
        }
    }
}

/// Represents which scroll boundary was hit (for visual feedback)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollBoundary {
    Top,
    Bottom,
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
    /// Maximum scroll value (calculated during render, used for clamping)
    pub max_scroll: u16,
    /// Current programming mode for Claude interactions
    pub programming_mode: ProgrammingMode,
    /// Session-level state (skills, permissions, oauth, tokens)
    pub session_state: SessionState,
    /// Tool execution tracking per-thread (cleared on done event)
    pub tool_tracker: ToolTracker,
    /// Session-level todos from the assistant
    /// Subagent activity tracking (cleared on done event)
    pub subagent_tracker: SubagentTracker,
    pub todos: Vec<Todo>,
    /// Debug event sender for emitting internal events to debug server
    pub debug_tx: Option<DebugEventSender>,
    /// Timestamp when the current stream started
    pub stream_start_time: Option<std::time::Instant>,
    /// Timestamp of the last event
    pub last_event_time: Option<std::time::Instant>,
    /// Cumulative token count for the current stream
    pub cumulative_token_count: u64,
    /// Thread switcher dialog state (double-tap Tab to switch threads)
    pub thread_switcher: ThreadSwitcher,
    /// Timestamp of last Tab press (for double-tap detection)
    pub last_tab_press: Option<std::time::Instant>,
    /// Scroll boundary hit state (for visual feedback)
    pub scroll_boundary_hit: Option<ScrollBoundary>,
    /// Tick counter when boundary was hit (for timing the highlight)
    pub boundary_hit_tick: u64,
    /// Scroll velocity for momentum scrolling (lines per tick, positive = up/older)
    pub scroll_velocity: f32,
    /// Precise scroll position for smooth scrolling (fractional lines)
    pub scroll_position: f32,
    /// Current terminal width in columns
    pub terminal_width: u16,
    /// Current terminal height in rows
    pub terminal_height: u16,
}

impl App {
    /// Create a new App instance
    pub fn new() -> Result<Self> {
        Self::with_debug(None)
    }

    /// Create a new App instance with an optional debug event sender
    pub fn with_debug(debug_tx: Option<DebugEventSender>) -> Result<Self> {
        Self::with_client_and_debug(Arc::new(ConductorClient::new()), debug_tx)
    }

    /// Create a new App instance with a custom ConductorClient
    pub fn with_client(client: Arc<ConductorClient>) -> Result<Self> {
        Self::with_client_and_debug(client, None)
    }

    /// Create a new App instance with a custom ConductorClient and optional debug sender
    pub fn with_client_and_debug(
        client: Arc<ConductorClient>,
        debug_tx: Option<DebugEventSender>,
    ) -> Result<Self> {
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
            max_scroll: 0,
            programming_mode: ProgrammingMode::default(),
            session_state: SessionState::new(),
            tool_tracker: ToolTracker::new(),
            subagent_tracker: SubagentTracker::new(),
            todos: Vec::new(),
            debug_tx,
            stream_start_time: None,
            last_event_time: None,
            cumulative_token_count: 0,
            thread_switcher: ThreadSwitcher::default(),
            last_tab_press: None,
            scroll_boundary_hit: None,
            boundary_hit_tick: 0,
            scroll_velocity: 0.0,
            scroll_position: 0.0,
            terminal_width: 80,  // Default, will be updated on first render
            terminal_height: 24, // Default, will be updated on first render
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
                // Iterate in reverse because upsert_thread() inserts at front,
                // so we process oldest first to end up with newest at front
                for thread in threads.into_iter().rev() {
                    self.cache.upsert_thread(thread);
                }
                self.connection_status = true;
            }
            Err(e) => {
                // Server unreachable - start with empty state
                // Log the error for debugging
                log_thread_update(&format!("fetch_threads failed: {:?}", e));
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
        use crate::models::ThreadType;
        let mut app = App::default();
        let initial_cache_count = app.cache.thread_count();

        app.submit_input(ThreadType::Conversation);

        // Nothing should change with empty input
        assert_eq!(app.cache.thread_count(), initial_cache_count);
        assert_eq!(app.screen, Screen::CommandDeck);
        assert!(app.active_thread_id.is_none());
    }

    #[test]
    fn test_submit_input_with_whitespace_only_does_nothing() {
        use crate::models::ThreadType;
        let mut app = App::default();
        app.input_box.insert_char(' ');
        app.input_box.insert_char(' ');
        let initial_cache_count = app.cache.thread_count();

        app.submit_input(ThreadType::Conversation);

        // Whitespace-only input should be ignored
        assert_eq!(app.cache.thread_count(), initial_cache_count);
        assert_eq!(app.screen, Screen::CommandDeck);
        assert!(app.active_thread_id.is_none());
    }

    #[tokio::test]
    async fn test_submit_input_creates_thread_and_navigates() {
        use crate::models::ThreadType;
        let mut app = App::default();
        app.input_box.insert_char('H');
        app.input_box.insert_char('i');
        let initial_cache_count = app.cache.thread_count();

        app.submit_input(ThreadType::Conversation);

        // Should create a new thread
        assert_eq!(app.cache.thread_count(), initial_cache_count + 1);
        // Should navigate to conversation screen
        assert_eq!(app.screen, Screen::Conversation);
        // Should have an active thread ID that is a valid UUID
        assert!(app.active_thread_id.is_some());
        assert!(uuid::Uuid::parse_str(app.active_thread_id.as_ref().unwrap()).is_ok());
        // Input should be cleared
        assert!(app.input_box.is_empty());
    }

    #[tokio::test]
    async fn test_submit_input_adds_messages_to_thread() {
        use crate::models::ThreadType;
        let mut app = App::default();
        app.input_box.insert_char('T');
        app.input_box.insert_char('e');
        app.input_box.insert_char('s');
        app.input_box.insert_char('t');

        app.submit_input(ThreadType::Conversation);

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
        use crate::models::ThreadType;
        let mut app = App::default();
        app.input_box.insert_char('N');
        app.input_box.insert_char('e');
        app.input_box.insert_char('w');

        app.submit_input(ThreadType::Conversation);

        let thread_id = app.active_thread_id.as_ref().unwrap();
        // The new thread should be at the front of the list and be a valid UUID
        assert_eq!(app.cache.threads()[0].id, *thread_id);
        assert!(uuid::Uuid::parse_str(thread_id).is_ok());
    }

    // ============= New Thread vs Continuing Thread Tests =============

    #[tokio::test]
    async fn test_submit_input_new_thread_when_no_active_thread() {
        use crate::models::ThreadType;
        let mut app = App::default();
        assert!(app.active_thread_id.is_none());
        app.input_box.insert_char('H');
        app.input_box.insert_char('i');

        app.submit_input(ThreadType::Conversation);

        // Should create a thread with a valid UUID
        let thread_id = app.active_thread_id.as_ref().unwrap();
        assert!(uuid::Uuid::parse_str(thread_id).is_ok());
        // Should navigate to conversation
        assert_eq!(app.screen, Screen::Conversation);
    }

    #[tokio::test]
    async fn test_submit_from_command_deck_always_creates_new_thread() {
        use crate::models::ThreadType;
        // This is the key bug fix test: even if active_thread_id is set,
        // submitting from CommandDeck should create a NEW thread, not continue.
        let mut app = App::default();

        // Simulate a stale active_thread_id (e.g., leftover from previous session)
        app.active_thread_id = Some("stale-thread-id".to_string());
        // But we're on CommandDeck, not Conversation
        app.screen = Screen::CommandDeck;

        app.input_box.insert_char('N');
        app.input_box.insert_char('e');
        app.input_box.insert_char('w');

        app.submit_input(ThreadType::Conversation);

        // Should create a NEW pending thread, ignoring the stale active_thread_id
        let thread_id = app.active_thread_id.as_ref().unwrap();
        assert!(
            uuid::Uuid::parse_str(thread_id).is_ok(),
            "Expected new UUID thread, got: {}",
            thread_id
        );
        assert_ne!(
            thread_id, "stale-thread-id",
            "Should not reuse stale thread ID"
        );
        // Should navigate to conversation
        assert_eq!(app.screen, Screen::Conversation);
    }

    #[tokio::test]
    async fn test_submit_input_continues_existing_thread() {
        use crate::models::ThreadType;
        let mut app = App::default();

        // Create an existing thread with a real (non-pending) ID
        let existing_id = "real-thread-123".to_string();
        app.cache.upsert_thread(crate::models::Thread {
            id: existing_id.clone(),
            title: "Existing Thread".to_string(),
            description: None,
            preview: "Previous message".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::default(),
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: chrono::Utc::now(),
        });
        app.cache
            .add_message_simple(&existing_id, MessageRole::User, "Previous question".to_string());
        app.cache
            .add_message_simple(&existing_id, MessageRole::Assistant, "Previous answer".to_string());

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
        app.submit_input(ThreadType::Conversation);

        // Should NOT create a new thread
        assert_eq!(app.active_thread_id.as_ref().unwrap(), &existing_id);
        // Should add messages to existing thread
        let messages = app.cache.get_messages(&existing_id).unwrap();
        assert_eq!(messages.len(), initial_msg_count + 2); // +1 user, +1 streaming assistant

        // Last user message should be our follow-up
        let user_msgs: Vec<_> = messages
            .iter()
            .filter(|m| m.role == MessageRole::User)
            .collect();
        assert_eq!(user_msgs.last().unwrap().content, "Follow");
    }

    #[tokio::test]
    async fn test_submit_input_blocks_rapid_submit_while_streaming() {
        use crate::models::ThreadType;
        let mut app = App::default();

        // First submit creates thread with streaming response
        app.input_box.insert_char('F');
        app.input_box.insert_char('i');
        app.input_box.insert_char('r');
        app.input_box.insert_char('s');
        app.input_box.insert_char('t');
        app.submit_input(ThreadType::Conversation);

        let thread_id = app.active_thread_id.clone().unwrap();
        assert!(uuid::Uuid::parse_str(&thread_id).is_ok());

        // Try to submit again while still streaming
        app.input_box.insert_char('S');
        app.input_box.insert_char('e');
        app.input_box.insert_char('c');
        app.input_box.insert_char('o');
        app.input_box.insert_char('n');
        app.input_box.insert_char('d');
        app.submit_input(ThreadType::Conversation);

        // Should NOT create a new thread or add messages
        // Should set an error
        assert!(app.stream_error.is_some());
        assert!(app.stream_error.as_ref().unwrap().contains("wait"));

        // Input should NOT be cleared (submission was rejected)
        assert!(!app.input_box.is_empty());
        assert_eq!(app.input_box.content(), "Second");

        // Should still be on the same thread
        assert_eq!(app.active_thread_id, Some(thread_id));
    }

    #[tokio::test]
    async fn test_submit_input_allows_submit_after_thread_reconciled() {
        use crate::models::ThreadType;
        let mut app = App::default();

        // First submit creates pending thread
        app.input_box.insert_char('F');
        app.input_box.insert_char('i');
        app.input_box.insert_char('r');
        app.input_box.insert_char('s');
        app.input_box.insert_char('t');
        app.submit_input(ThreadType::Conversation);

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
        app.submit_input(ThreadType::Conversation);

        // Should add to existing thread
        let messages = app.cache.get_messages("real-backend-id").unwrap();
        assert_eq!(messages.len(), before_count + 2);
        assert!(app.input_box.is_empty());
    }

    #[tokio::test]
    async fn test_submit_input_handles_deleted_thread() {
        use crate::models::ThreadType;
        let mut app = App::default();

        // Set active thread to non-existent (simulates deleted thread)
        app.active_thread_id = Some("deleted-thread".to_string());
        app.screen = Screen::Conversation;

        app.input_box.insert_char('T');
        app.input_box.insert_char('e');
        app.input_box.insert_char('s');
        app.input_box.insert_char('t');
        app.submit_input(ThreadType::Conversation);

        // Should show error about thread not existing
        assert!(app.stream_error.is_some());
        assert!(app
            .stream_error
            .as_ref()
            .unwrap()
            .contains("no longer exists"));

        // Input should NOT be cleared
        assert!(!app.input_box.is_empty());
    }

    #[tokio::test]
    async fn test_submit_input_full_conversation_workflow() {
        use crate::models::ThreadType;
        let mut app = App::default();

        // === Turn 1: New thread ===
        app.input_box.insert_char('H');
        app.input_box.insert_char('i');
        app.submit_input(ThreadType::Conversation);

        let thread_id = app.active_thread_id.clone().unwrap();
        assert!(uuid::Uuid::parse_str(&thread_id).is_ok());
        assert_eq!(app.screen, Screen::Conversation);

        // Simulate backend response (backend echoes back the same UUID we sent)
        app.handle_message(AppMessage::ThreadCreated {
            pending_id: thread_id.clone(),
            real_id: thread_id.clone(), // Backend uses our client-generated UUID
            title: Some("Greeting".to_string()),
        });
        app.cache
            .append_to_message(&thread_id, "Hello! How can I help?");
        app.cache.finalize_message(&thread_id, 100);

        assert_eq!(app.active_thread_id, Some(thread_id.clone()));

        // === Turn 2: Continue thread ===
        app.input_box.insert_char('T');
        app.input_box.insert_char('e');
        app.input_box.insert_char('l');
        app.input_box.insert_char('l');
        app.input_box.insert_char(' ');
        app.input_box.insert_char('m');
        app.input_box.insert_char('e');
        app.submit_input(ThreadType::Conversation);

        // Should still be on same thread
        assert_eq!(app.active_thread_id, Some(thread_id.clone()));

        // Should have 4 messages: user1, assistant1, user2, assistant2(streaming)
        let messages = app.cache.get_messages(&thread_id).unwrap();
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
        app.submit_input(ThreadType::Conversation);

        // Should be a NEW thread with a valid UUID
        let new_thread_id = app.active_thread_id.clone().unwrap();
        assert!(uuid::Uuid::parse_str(&new_thread_id).is_ok());
        assert_ne!(new_thread_id, thread_id);

        // Cache should have both threads
        assert!(app.cache.get_thread(&thread_id).is_some());
        assert!(app.cache.get_thread(&new_thread_id).is_some());
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
        let assistant_msg = messages
            .iter()
            .find(|m| m.role == MessageRole::Assistant)
            .unwrap();
        assert!(
            assistant_msg.content.contains("Hello") || assistant_msg.partial_content.contains("Hello")
        );
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
        let assistant_msg = messages
            .iter()
            .find(|m| m.role == MessageRole::Assistant)
            .unwrap();
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
        // When server is unreachable, initialize sets connection_status to false
        let client = Arc::new(ConductorClient::with_base_url(
            "http://127.0.0.1:1".to_string(),
        ));
        let mut app = App::with_client(client).unwrap();

        // Connection status should start as false
        assert!(!app.connection_status);

        app.initialize().await;

        // After initialization with unreachable server, connection_status should remain false
        assert!(!app.connection_status);
    }

    #[tokio::test]
    async fn test_initialize_starts_with_empty_cache() {
        let client = Arc::new(ConductorClient::with_base_url(
            "http://127.0.0.1:1".to_string(),
        ));
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

    // ============= ProgrammingMode Tests =============

    #[test]
    fn test_programming_mode_default_is_none() {
        assert_eq!(ProgrammingMode::default(), ProgrammingMode::None);
    }

    #[test]
    fn test_programming_mode_equality() {
        assert_eq!(ProgrammingMode::PlanMode, ProgrammingMode::PlanMode);
        assert_eq!(
            ProgrammingMode::BypassPermissions,
            ProgrammingMode::BypassPermissions
        );
        assert_eq!(ProgrammingMode::None, ProgrammingMode::None);
        assert_ne!(ProgrammingMode::PlanMode, ProgrammingMode::None);
        assert_ne!(
            ProgrammingMode::BypassPermissions,
            ProgrammingMode::PlanMode
        );
    }

    #[test]
    fn test_programming_mode_copy() {
        let mode = ProgrammingMode::PlanMode;
        let copied = mode;
        assert_eq!(mode, copied);
    }

    #[test]
    fn test_app_initializes_with_no_programming_mode() {
        let app = App::default();
        assert_eq!(app.programming_mode, ProgrammingMode::None);
    }

    #[test]
    fn test_cycle_programming_mode_from_none_to_plan() {
        let mut app = App::default();
        assert_eq!(app.programming_mode, ProgrammingMode::None);

        app.cycle_programming_mode();

        assert_eq!(app.programming_mode, ProgrammingMode::PlanMode);
    }

    #[test]
    fn test_cycle_programming_mode_from_plan_to_bypass() {
        let mut app = App::default();
        app.programming_mode = ProgrammingMode::PlanMode;

        app.cycle_programming_mode();

        assert_eq!(app.programming_mode, ProgrammingMode::BypassPermissions);
    }

    #[test]
    fn test_cycle_programming_mode_from_bypass_to_none() {
        let mut app = App::default();
        app.programming_mode = ProgrammingMode::BypassPermissions;

        app.cycle_programming_mode();

        assert_eq!(app.programming_mode, ProgrammingMode::None);
    }

    #[test]
    fn test_cycle_programming_mode_full_cycle() {
        let mut app = App::default();

        // Start at None (default)
        assert_eq!(app.programming_mode, ProgrammingMode::None);

        // Cycle: None → PlanMode
        app.cycle_programming_mode();
        assert_eq!(app.programming_mode, ProgrammingMode::PlanMode);

        // Cycle: PlanMode → BypassPermissions
        app.cycle_programming_mode();
        assert_eq!(app.programming_mode, ProgrammingMode::BypassPermissions);

        // Cycle: BypassPermissions → None
        app.cycle_programming_mode();
        assert_eq!(app.programming_mode, ProgrammingMode::None);

        // Cycle: None → PlanMode (wraps around)
        app.cycle_programming_mode();
        assert_eq!(app.programming_mode, ProgrammingMode::PlanMode);
    }

    #[test]
    fn test_cycle_programming_mode_multiple_cycles() {
        let mut app = App::default();

        // Cycle through 3 complete cycles (9 transitions)
        for _ in 0..3 {
            app.cycle_programming_mode(); // None → PlanMode
            assert_eq!(app.programming_mode, ProgrammingMode::PlanMode);

            app.cycle_programming_mode(); // PlanMode → BypassPermissions
            assert_eq!(app.programming_mode, ProgrammingMode::BypassPermissions);

            app.cycle_programming_mode(); // BypassPermissions → None
            assert_eq!(app.programming_mode, ProgrammingMode::None);
        }
    }

    // ============= is_active_thread_programming Tests =============

    #[test]
    fn test_is_active_thread_programming_returns_false_when_no_active_thread() {
        let app = App::default();
        assert!(app.active_thread_id.is_none());
        assert!(!app.is_active_thread_programming());
    }

    #[test]
    fn test_is_active_thread_programming_returns_false_for_normal_thread() {
        let mut app = App::default();

        // Create a normal thread
        let thread = crate::models::Thread {
            id: "thread-conv".to_string(),
            title: "Normal Thread".to_string(),
            description: None,
            preview: "Just talking".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Conversation,
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: chrono::Utc::now(),
        };
        app.cache.upsert_thread(thread);
        app.active_thread_id = Some("thread-conv".to_string());

        assert!(!app.is_active_thread_programming());
    }

    #[test]
    fn test_is_active_thread_programming_returns_true_for_programming_thread() {
        let mut app = App::default();

        // Create a programming thread
        let thread = crate::models::Thread {
            id: "thread-prog".to_string(),
            title: "Programming Thread".to_string(),
            description: None,
            preview: "Code review".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Programming,
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: chrono::Utc::now(),
        };
        app.cache.upsert_thread(thread);
        app.active_thread_id = Some("thread-prog".to_string());

        assert!(app.is_active_thread_programming());
    }

    #[test]
    fn test_is_active_thread_programming_returns_false_for_nonexistent_thread() {
        let mut app = App::default();
        app.active_thread_id = Some("nonexistent-thread".to_string());

        assert!(!app.is_active_thread_programming());
    }

    #[test]
    fn test_is_active_thread_programming_after_thread_type_change() {
        let mut app = App::default();

        // Create a normal thread
        let thread = crate::models::Thread {
            id: "thread-1".to_string(),
            title: "Thread".to_string(),
            description: None,
            preview: "Content".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Conversation,
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: chrono::Utc::now(),
        };
        app.cache.upsert_thread(thread);
        app.active_thread_id = Some("thread-1".to_string());

        assert!(!app.is_active_thread_programming());

        // Update to programming thread
        let thread = crate::models::Thread {
            id: "thread-1".to_string(),
            title: "Thread".to_string(),
            description: None,
            preview: "Content".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Programming,
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: chrono::Utc::now(),
        };
        app.cache.upsert_thread(thread);

        assert!(app.is_active_thread_programming());
    }

    // ============= Submit Input with Programming Thread Tests =============

    #[tokio::test]
    async fn test_submit_input_on_programming_thread_uses_programming_mode() {
        use crate::models::ThreadType;
        let mut app = App::default();

        // Create a programming thread
        let thread = crate::models::Thread {
            id: "prog-thread-123".to_string(),
            title: "Programming Thread".to_string(),
            description: None,
            preview: "Code discussion".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: ThreadType::Programming,
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: chrono::Utc::now(),
        };
        app.cache.upsert_thread(thread);
        app.cache
            .add_message_simple("prog-thread-123", MessageRole::User, "Previous".to_string());
        app.cache.add_message_simple(
            "prog-thread-123",
            MessageRole::Assistant,
            "Response".to_string(),
        );
        app.active_thread_id = Some("prog-thread-123".to_string());
        app.screen = Screen::Conversation;

        // Set plan mode
        app.programming_mode = ProgrammingMode::PlanMode;

        // Submit input
        app.input_box.insert_char('H');
        app.input_box.insert_char('i');
        app.submit_input(ThreadType::Conversation);

        // Should add streaming message to the thread
        let messages = app.cache.get_messages("prog-thread-123").unwrap();
        assert_eq!(messages.len(), 4); // 2 original + user + assistant streaming
        assert!(messages[3].is_streaming);
    }

    #[tokio::test]
    async fn test_submit_input_programming_mode_none_sets_correct_flags() {
        use crate::models::ThreadType;
        let mut app = App::default();

        // Create a programming thread
        let thread = crate::models::Thread {
            id: "prog-thread-456".to_string(),
            title: "Programming Thread".to_string(),
            description: None,
            preview: "Code discussion".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: ThreadType::Programming,
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: chrono::Utc::now(),
        };
        app.cache.upsert_thread(thread);
        app.cache
            .add_message_simple("prog-thread-456", MessageRole::User, "Prev".to_string());
        app.cache
            .add_message_simple("prog-thread-456", MessageRole::Assistant, "Resp".to_string());
        app.active_thread_id = Some("prog-thread-456".to_string());
        app.screen = Screen::Conversation;

        // Mode is None by default
        assert_eq!(app.programming_mode, ProgrammingMode::None);

        // Submit input
        app.input_box.insert_char('T');
        app.input_box.insert_char('e');
        app.input_box.insert_char('s');
        app.input_box.insert_char('t');
        app.submit_input(ThreadType::Conversation);

        // Input should be cleared (submission was accepted)
        assert!(app.input_box.is_empty());
    }

    #[tokio::test]
    async fn test_submit_input_new_thread_is_not_programming() {
        use crate::models::ThreadType;
        let mut app = App::default();
        assert!(app.active_thread_id.is_none());

        // Submit creates a new non-programming thread
        app.input_box.insert_char('N');
        app.input_box.insert_char('e');
        app.input_box.insert_char('w');
        app.submit_input(ThreadType::Conversation);

        // New thread should be at front with a valid UUID
        let thread_id = app.active_thread_id.as_ref().unwrap();
        assert!(uuid::Uuid::parse_str(thread_id).is_ok());

        // The new thread should NOT be a programming thread
        assert!(!app.is_active_thread_programming());
    }

    #[test]
    fn test_create_pending_thread_uses_thread_type_parameter() {
        use crate::models::ThreadType;
        let mut app = App::default();

        // Create a programming pending thread via cache directly
        let pending_id = app
            .cache
            .create_pending_thread("Code task".to_string(), ThreadType::Programming);

        // Thread should have programming type
        let thread = app.cache.get_thread(&pending_id).unwrap();
        assert_eq!(thread.thread_type, ThreadType::Programming);
    }

    #[tokio::test]
    async fn test_submit_input_creates_programming_thread_when_specified() {
        use crate::models::ThreadType;
        let mut app = App::default();
        assert!(app.active_thread_id.is_none());

        // Submit with Programming thread type (like Shift+Enter on CommandDeck)
        app.input_box.insert_char('C');
        app.input_box.insert_char('o');
        app.input_box.insert_char('d');
        app.input_box.insert_char('e');
        app.submit_input(ThreadType::Programming);

        // New thread should be created with a valid UUID
        let thread_id = app.active_thread_id.as_ref().unwrap();
        assert!(uuid::Uuid::parse_str(thread_id).is_ok());

        // The new thread SHOULD be a programming thread
        assert!(app.is_active_thread_programming());

        // Verify thread type in cache
        let thread = app.cache.get_thread(thread_id).unwrap();
        assert_eq!(thread.thread_type, ThreadType::Programming);
    }

    // ============= Mode Indicator Visibility Tests =============
    // Mode indicator should only be visible for programming threads

    #[test]
    fn test_mode_indicator_visibility_logic_programming_thread() {
        let mut app = App::default();

        // Create a programming thread
        let thread = crate::models::Thread {
            id: "prog-thread".to_string(),
            title: "Programming".to_string(),
            description: None,
            preview: "Code".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Programming,
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: chrono::Utc::now(),
        };
        app.cache.upsert_thread(thread);
        app.active_thread_id = Some("prog-thread".to_string());
        app.screen = Screen::Conversation;

        // is_active_thread_programming determines mode indicator visibility
        // For programming threads, it should be true (indicator visible)
        assert!(app.is_active_thread_programming());
    }

    #[test]
    fn test_mode_indicator_visibility_logic_normal_thread() {
        let mut app = App::default();

        // Create a normal thread
        let thread = crate::models::Thread {
            id: "conv-thread".to_string(),
            title: "Normal".to_string(),
            description: None,
            preview: "Chat".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Conversation,
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: chrono::Utc::now(),
        };
        app.cache.upsert_thread(thread);
        app.active_thread_id = Some("conv-thread".to_string());
        app.screen = Screen::Conversation;

        // For normal threads, indicator should NOT be visible
        assert!(!app.is_active_thread_programming());
    }

    #[test]
    fn test_mode_indicator_visibility_logic_no_thread() {
        let app = App::default();

        // No active thread means no indicator
        assert!(app.active_thread_id.is_none());
        assert!(!app.is_active_thread_programming());
    }

    // ============= Shift+Tab Behavior Tests =============
    // Tests for the conditions that determine Shift+Tab behavior

    #[test]
    fn test_shift_tab_should_cycle_mode_for_programming_thread_in_conversation() {
        let mut app = App::default();

        // Create a programming thread
        let thread = crate::models::Thread {
            id: "prog-thread".to_string(),
            title: "Programming".to_string(),
            description: None,
            preview: "Code".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Programming,
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: chrono::Utc::now(),
        };
        app.cache.upsert_thread(thread);
        app.active_thread_id = Some("prog-thread".to_string());
        app.screen = Screen::Conversation;

        // Condition for Shift+Tab to cycle mode:
        // 1. Screen is Conversation
        // 2. Active thread is Programming type
        assert_eq!(app.screen, Screen::Conversation);
        assert!(app.is_active_thread_programming());

        // Mode cycling should work
        assert_eq!(app.programming_mode, ProgrammingMode::None);
        app.cycle_programming_mode();
        assert_eq!(app.programming_mode, ProgrammingMode::PlanMode);
    }

    #[test]
    fn test_shift_tab_should_not_cycle_mode_for_normal_thread() {
        let mut app = App::default();

        // Create a normal thread
        let thread = crate::models::Thread {
            id: "conv-thread".to_string(),
            title: "Normal".to_string(),
            description: None,
            preview: "Chat".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Conversation,
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: chrono::Utc::now(),
        };
        app.cache.upsert_thread(thread);
        app.active_thread_id = Some("conv-thread".to_string());
        app.screen = Screen::Conversation;

        // Shift+Tab should NOT cycle mode for conversation threads
        // (it should cycle focus instead, but that logic is in main.rs)
        assert_eq!(app.screen, Screen::Conversation);
        assert!(!app.is_active_thread_programming());
    }

    #[test]
    fn test_shift_tab_should_not_cycle_mode_on_command_deck() {
        let mut app = App::default();

        // Create a programming thread but stay on CommandDeck
        let thread = crate::models::Thread {
            id: "prog-thread".to_string(),
            title: "Programming".to_string(),
            description: None,
            preview: "Code".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Programming,
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: chrono::Utc::now(),
        };
        app.cache.upsert_thread(thread);
        app.active_thread_id = Some("prog-thread".to_string());
        app.screen = Screen::CommandDeck; // Not in Conversation

        // Even with programming thread, Shift+Tab should not cycle mode
        // because we're not on Conversation screen
        assert_eq!(app.screen, Screen::CommandDeck);
        // The condition for mode cycling is both screen AND thread type
    }

    // ============= Programming Mode Persistence Tests =============

    #[test]
    fn test_programming_mode_persists_across_thread_switches() {
        use crate::models::ThreadType;
        let mut app = App::default();

        // Set programming mode
        app.programming_mode = ProgrammingMode::PlanMode;

        // Create and switch to a programming thread
        let thread1 = crate::models::Thread {
            id: "prog-1".to_string(),
            title: "Programming 1".to_string(),
            description: None,
            preview: "Code 1".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: ThreadType::Programming,
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: chrono::Utc::now(),
        };
        app.cache.upsert_thread(thread1);
        // Pre-populate messages to avoid lazy fetch triggering tokio::spawn
        app.cache.set_messages("prog-1".to_string(), vec![]);
        app.open_thread("prog-1".to_string());

        // Mode should persist
        assert_eq!(app.programming_mode, ProgrammingMode::PlanMode);

        // Switch to another thread
        let thread2 = crate::models::Thread {
            id: "prog-2".to_string(),
            title: "Programming 2".to_string(),
            description: None,
            preview: "Code 2".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: ThreadType::Programming,
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: chrono::Utc::now(),
        };
        app.cache.upsert_thread(thread2);
        // Pre-populate messages to avoid lazy fetch triggering tokio::spawn
        app.cache.set_messages("prog-2".to_string(), vec![]);
        app.open_thread("prog-2".to_string());

        // Mode should still persist
        assert_eq!(app.programming_mode, ProgrammingMode::PlanMode);
    }

    #[test]
    fn test_programming_mode_persists_after_navigate_to_command_deck() {
        let mut app = App::default();

        // Set programming mode
        app.programming_mode = ProgrammingMode::BypassPermissions;

        // Navigate to command deck
        app.navigate_to_command_deck();

        // Mode should persist (it's app-level, not thread-level)
        assert_eq!(app.programming_mode, ProgrammingMode::BypassPermissions);
    }

    // ============= Thread Type with Add Streaming Message Tests =============

    #[test]
    fn test_add_streaming_message_preserves_thread_type() {
        use crate::models::ThreadType;
        let mut app = App::default();

        // Create a programming thread
        let pending_id = app
            .cache
            .create_pending_thread("Code question".to_string(), ThreadType::Programming);

        // Verify initial type
        let thread = app.cache.get_thread(&pending_id).unwrap();
        assert_eq!(thread.thread_type, ThreadType::Programming);

        // Finalize first response
        app.cache.append_to_message(&pending_id, "Answer");
        app.cache.finalize_message(&pending_id, 1);

        // Reconcile with backend
        app.cache
            .reconcile_thread_id(&pending_id, "real-thread-123", None);

        // Add follow-up message
        app.cache
            .add_streaming_message("real-thread-123", "Follow-up".to_string());

        // Thread type should be preserved
        let thread = app.cache.get_thread("real-thread-123").unwrap();
        assert_eq!(thread.thread_type, ThreadType::Programming);
    }

    // ============= State Module Integration Tests =============

    #[test]
    fn test_app_initializes_session_state() {
        let app = App::default();

        // SessionState should be initialized
        assert!(app.session_state.skills.is_empty());
        assert!(app.session_state.context_tokens_used.is_none());
        assert!(app.session_state.pending_permission.is_none());
        assert!(app.session_state.oauth_required.is_none());
    }

    #[test]
    fn test_app_initializes_tool_tracker() {
        let app = App::default();

        // ToolTracker should be initialized and empty
        assert_eq!(app.tool_tracker.total_count(), 0);
        assert!(!app.tool_tracker.has_active_tools());
    }

    #[test]
    fn test_session_state_persists_across_operations() {
        let mut app = App::default();

        // Modify session state
        app.session_state.add_skill("git".to_string());
        app.session_state.set_context_tokens(5000);

        // Navigate around
        app.navigate_to_command_deck();

        // Session state should persist
        assert!(app.session_state.has_skill("git"));
        assert_eq!(app.session_state.context_tokens_used, Some(5000));
    }

    #[test]
    fn test_tool_tracker_can_track_tools() {
        use crate::state::tools::{ToolCallState, ToolCallStatus};

        let mut app = App::default();

        // Register a tool call
        let state = ToolCallState::new("Bash".to_string());
        app.tool_tracker.register_tool("tool-1".to_string(), state);

        // Verify tracking
        assert_eq!(app.tool_tracker.total_count(), 1);
        assert!(app.tool_tracker.contains("tool-1"));

        // Start the tool
        app.tool_tracker.start_tool("tool-1");
        let tool_state = app.tool_tracker.get_tool("tool-1").unwrap();
        assert_eq!(tool_state.status, ToolCallStatus::Running);

        // Complete the tool
        app.tool_tracker
            .complete_tool("tool-1", Some("output".to_string()));
        let tool_state = app.tool_tracker.get_tool("tool-1").unwrap();
        assert_eq!(tool_state.status, ToolCallStatus::Completed);
        assert!(!app.tool_tracker.has_active_tools());
    }

    #[test]
    fn test_tool_tracker_independent_per_app() {
        let mut app1 = App::default();
        let app2 = App::default();

        // Add tool to app1
        app1.tool_tracker.register_tool(
            "tool-1".to_string(),
            crate::state::tools::ToolCallState::new("Bash".to_string()),
        );

        // app2 should not see it
        assert_eq!(app1.tool_tracker.total_count(), 1);
        assert_eq!(app2.tool_tracker.total_count(), 0);
    }

    // ============= is_streaming() Tests =============

    #[test]
    fn test_is_streaming_returns_false_when_no_active_thread() {
        let app = App::default();
        assert!(app.active_thread_id.is_none());
        assert!(!app.is_streaming());
    }

    #[test]
    fn test_is_streaming_returns_true_when_thread_is_streaming() {
        let mut app = App::default();

        // Create a streaming thread
        let thread_id = app.cache.create_streaming_thread("Test message".to_string());
        app.active_thread_id = Some(thread_id.clone());

        // Should detect streaming
        assert!(app.is_streaming());
    }

    #[test]
    fn test_is_streaming_returns_false_when_thread_not_streaming() {
        let mut app = App::default();

        // Create a streaming thread and finalize it
        let thread_id = app.cache.create_streaming_thread("Test message".to_string());
        app.cache.append_to_message(&thread_id, "Response");
        app.cache.finalize_message(&thread_id, 1);
        app.active_thread_id = Some(thread_id.clone());

        // Should NOT detect streaming (message is finalized)
        assert!(!app.is_streaming());
    }

    #[test]
    fn test_is_streaming_returns_false_for_nonexistent_thread() {
        let mut app = App::default();
        app.active_thread_id = Some("nonexistent-thread".to_string());

        assert!(!app.is_streaming());
    }

    #[test]
    fn test_is_streaming_updates_when_stream_completes() {
        let mut app = App::default();

        // Create a streaming thread
        let thread_id = app.cache.create_streaming_thread("Question".to_string());
        app.active_thread_id = Some(thread_id.clone());

        // Initially streaming
        assert!(app.is_streaming());

        // Finalize the stream
        app.cache.append_to_message(&thread_id, "Answer");
        app.cache.finalize_message(&thread_id, 42);

        // Should no longer be streaming
        assert!(!app.is_streaming());
    }

    // ============= TodosUpdated Tests =============

    #[test]
    fn test_app_initializes_with_empty_todos() {
        let app = App::default();
        assert!(app.todos.is_empty());
    }

    #[test]
    fn test_handle_message_todos_updated() {
        use crate::state::{Todo, TodoStatus};

        let mut app = App::default();

        let todos = vec![
            Todo {
                content: "Fix the bug".to_string(),
                active_form: "Fixing the bug".to_string(),
                status: TodoStatus::Pending,
            },
            Todo {
                content: "Run tests".to_string(),
                active_form: "Running tests".to_string(),
                status: TodoStatus::InProgress,
            },
        ];

        app.handle_message(AppMessage::TodosUpdated {
            todos: todos.clone(),
        });

        assert_eq!(app.todos.len(), 2);
        assert_eq!(app.todos[0].content, "Fix the bug");
        assert_eq!(app.todos[0].status, TodoStatus::Pending);
        assert_eq!(app.todos[1].content, "Run tests");
        assert_eq!(app.todos[1].status, TodoStatus::InProgress);
    }

    #[test]
    fn test_todos_updated_replaces_previous_todos() {
        use crate::state::{Todo, TodoStatus};

        let mut app = App::default();

        // Set initial todos
        let initial_todos = vec![
            Todo {
                content: "Old task 1".to_string(),
                active_form: "Old task 1".to_string(),
                status: TodoStatus::Pending,
            },
            Todo {
                content: "Old task 2".to_string(),
                active_form: "Old task 2".to_string(),
                status: TodoStatus::Pending,
            },
        ];
        app.handle_message(AppMessage::TodosUpdated {
            todos: initial_todos,
        });
        assert_eq!(app.todos.len(), 2);

        // Update with new todos
        let new_todos = vec![Todo {
            content: "New task".to_string(),
            active_form: "New task".to_string(),
            status: TodoStatus::InProgress,
        }];
        app.handle_message(AppMessage::TodosUpdated { todos: new_todos });

        // Should replace the old todos
        assert_eq!(app.todos.len(), 1);
        assert_eq!(app.todos[0].content, "New task");
        assert_eq!(app.todos[0].status, TodoStatus::InProgress);
    }

    #[test]
    fn test_todos_updated_with_empty_list() {
        use crate::state::{Todo, TodoStatus};

        let mut app = App::default();

        // Set initial todos
        let initial_todos = vec![Todo {
            content: "Task".to_string(),
            active_form: "Task".to_string(),
            status: TodoStatus::Pending,
        }];
        app.handle_message(AppMessage::TodosUpdated {
            todos: initial_todos,
        });
        assert_eq!(app.todos.len(), 1);

        // Clear todos
        app.handle_message(AppMessage::TodosUpdated { todos: Vec::new() });

        assert!(app.todos.is_empty());
    }

    #[test]
    fn test_todos_updated_preserves_active_form() {
        use crate::state::{Todo, TodoStatus};

        let mut app = App::default();

        let todos = vec![Todo {
            content: "Build the project".to_string(),
            active_form: "Building the project".to_string(),
            status: TodoStatus::InProgress,
        }];

        app.handle_message(AppMessage::TodosUpdated {
            todos: todos.clone(),
        });

        assert_eq!(app.todos[0].content, "Build the project");
        assert_eq!(app.todos[0].active_form, "Building the project");
    }

    // ============= Inline Error Management Tests =============

    #[test]
    fn test_has_errors_returns_false_when_no_active_thread() {
        let app = App::default();
        assert!(app.active_thread_id.is_none());
        assert!(!app.has_errors());
    }

    #[test]
    fn test_has_errors_returns_false_when_no_errors() {
        let mut app = App::default();
        let thread_id = app.cache.create_streaming_thread("Hello".to_string());
        app.active_thread_id = Some(thread_id);

        assert!(!app.has_errors());
    }

    #[test]
    fn test_has_errors_returns_true_when_errors_exist() {
        let mut app = App::default();
        let thread_id = app.cache.create_streaming_thread("Hello".to_string());
        app.active_thread_id = Some(thread_id.clone());

        app.cache
            .add_error_simple(&thread_id, "error".to_string(), "message".to_string());

        assert!(app.has_errors());
    }

    #[test]
    fn test_add_error_to_active_thread() {
        let mut app = App::default();
        let thread_id = app.cache.create_streaming_thread("Hello".to_string());
        app.active_thread_id = Some(thread_id.clone());

        app.add_error_to_active_thread("test_error".to_string(), "Test message".to_string());

        assert!(app.has_errors());
        let errors = app.cache.get_errors(&thread_id).unwrap();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].error_code, "test_error");
        assert_eq!(errors[0].message, "Test message");
    }

    #[test]
    fn test_add_error_to_active_thread_when_no_active_thread() {
        let mut app = App::default();
        assert!(app.active_thread_id.is_none());

        // Should not panic
        app.add_error_to_active_thread("error".to_string(), "message".to_string());

        // No errors should be added anywhere
        assert!(!app.has_errors());
    }

    #[test]
    fn test_dismiss_focused_error_when_no_active_thread() {
        let mut app = App::default();
        assert!(app.active_thread_id.is_none());

        let dismissed = app.dismiss_focused_error();
        assert!(!dismissed);
    }

    #[test]
    fn test_dismiss_focused_error_when_no_errors() {
        let mut app = App::default();
        let thread_id = app.cache.create_streaming_thread("Hello".to_string());
        app.active_thread_id = Some(thread_id);

        let dismissed = app.dismiss_focused_error();
        assert!(!dismissed);
    }

    #[test]
    fn test_dismiss_focused_error_removes_error() {
        let mut app = App::default();
        let thread_id = app.cache.create_streaming_thread("Hello".to_string());
        app.active_thread_id = Some(thread_id.clone());

        app.cache
            .add_error_simple(&thread_id, "error1".to_string(), "First".to_string());
        app.cache
            .add_error_simple(&thread_id, "error2".to_string(), "Second".to_string());
        assert!(app.has_errors());
        assert_eq!(app.cache.error_count(&thread_id), 2);

        let dismissed = app.dismiss_focused_error();
        assert!(dismissed);
        assert_eq!(app.cache.error_count(&thread_id), 1);

        // Should still have one error
        assert!(app.has_errors());

        // Dismiss the remaining error
        let dismissed = app.dismiss_focused_error();
        assert!(dismissed);
        assert_eq!(app.cache.error_count(&thread_id), 0);

        // No more errors
        assert!(!app.has_errors());
    }

    #[test]
    fn test_error_persists_across_navigate_to_command_deck() {
        let mut app = App::default();
        let thread_id = app.cache.create_streaming_thread("Hello".to_string());
        app.active_thread_id = Some(thread_id.clone());
        app.screen = Screen::Conversation;

        app.add_error_to_active_thread("error".to_string(), "message".to_string());
        assert!(app.has_errors());

        // Navigate away
        app.navigate_to_command_deck();
        assert!(app.active_thread_id.is_none());

        // Error should still exist in cache
        assert_eq!(app.cache.error_count(&thread_id), 1);
    }

    #[test]
    fn test_multiple_errors_on_active_thread() {
        let mut app = App::default();
        let thread_id = app.cache.create_streaming_thread("Hello".to_string());
        app.active_thread_id = Some(thread_id.clone());

        app.add_error_to_active_thread("error1".to_string(), "First error".to_string());
        app.add_error_to_active_thread("error2".to_string(), "Second error".to_string());
        app.add_error_to_active_thread("error3".to_string(), "Third error".to_string());

        assert_eq!(app.cache.error_count(&thread_id), 3);
        assert!(app.has_errors());
    }

    // ============= Permission Handling Tests =============

    #[test]
    fn test_handle_permission_key_returns_false_when_no_pending() {
        let mut app = App::default();
        assert!(!app.session_state.has_pending_permission());

        // Should return false when no permission is pending
        assert!(!app.handle_permission_key('y'));
        assert!(!app.handle_permission_key('a'));
        assert!(!app.handle_permission_key('n'));
    }

    #[test]
    fn test_handle_permission_key_y_approves_and_clears() {
        let mut app = App::default();

        // Set up a pending permission
        use crate::state::PermissionRequest;
        app.session_state
            .set_pending_permission(PermissionRequest {
                permission_id: "perm-123".to_string(),
                tool_name: "Bash".to_string(),
                description: "Run npm install".to_string(),
                context: None,
                tool_input: None,
            });
        assert!(app.session_state.has_pending_permission());

        // Press 'y' to approve
        let handled = app.handle_permission_key('y');
        assert!(handled);

        // Permission should be cleared
        assert!(!app.session_state.has_pending_permission());
        // Tool should NOT be added to allowed list (that's only for 'a')
        assert!(!app.session_state.is_tool_allowed("Bash"));
    }

    #[test]
    fn test_handle_permission_key_a_allows_always_and_clears() {
        let mut app = App::default();

        // Set up a pending permission
        use crate::state::PermissionRequest;
        app.session_state
            .set_pending_permission(PermissionRequest {
                permission_id: "perm-456".to_string(),
                tool_name: "Write".to_string(),
                description: "Write file".to_string(),
                context: Some("/home/user/test.rs".to_string()),
                tool_input: None,
            });
        assert!(app.session_state.has_pending_permission());

        // Press 'a' to allow always
        let handled = app.handle_permission_key('a');
        assert!(handled);

        // Permission should be cleared
        assert!(!app.session_state.has_pending_permission());
        // Tool SHOULD be added to allowed list
        assert!(app.session_state.is_tool_allowed("Write"));
    }

    #[test]
    fn test_handle_permission_key_n_denies_and_clears() {
        let mut app = App::default();

        // Set up a pending permission
        use crate::state::PermissionRequest;
        app.session_state
            .set_pending_permission(PermissionRequest {
                permission_id: "perm-789".to_string(),
                tool_name: "Edit".to_string(),
                description: "Edit file".to_string(),
                context: None,
                tool_input: None,
            });
        assert!(app.session_state.has_pending_permission());

        // Press 'n' to deny
        let handled = app.handle_permission_key('n');
        assert!(handled);

        // Permission should be cleared
        assert!(!app.session_state.has_pending_permission());
        // Tool should NOT be added to allowed list
        assert!(!app.session_state.is_tool_allowed("Edit"));
    }

    #[test]
    fn test_handle_permission_key_uppercase_works() {
        let mut app = App::default();

        use crate::state::PermissionRequest;
        app.session_state
            .set_pending_permission(PermissionRequest {
                permission_id: "perm-abc".to_string(),
                tool_name: "Bash".to_string(),
                description: "Run command".to_string(),
                context: None,
                tool_input: None,
            });

        // Uppercase 'Y' should also work
        let handled = app.handle_permission_key('Y');
        assert!(handled);
        assert!(!app.session_state.has_pending_permission());
    }

    #[test]
    fn test_handle_permission_key_invalid_returns_false() {
        let mut app = App::default();

        use crate::state::PermissionRequest;
        app.session_state
            .set_pending_permission(PermissionRequest {
                permission_id: "perm-def".to_string(),
                tool_name: "Bash".to_string(),
                description: "Run command".to_string(),
                context: None,
                tool_input: None,
            });

        // Invalid keys should return false and NOT clear permission
        assert!(!app.handle_permission_key('x'));
        assert!(!app.handle_permission_key('q'));
        assert!(!app.handle_permission_key(' '));

        // Permission should still be pending
        assert!(app.session_state.has_pending_permission());
    }

    #[test]
    fn test_permission_auto_approve_when_tool_allowed() {
        let mut app = App::default();

        // Pre-allow the Bash tool
        app.session_state.allow_tool("Bash".to_string());
        assert!(app.session_state.is_tool_allowed("Bash"));

        // Receive a permission request for Bash
        app.handle_message(AppMessage::PermissionRequested {
            permission_id: "perm-auto".to_string(),
            tool_name: "Bash".to_string(),
            description: "Run npm install".to_string(),
            tool_input: None,
        });

        // Permission should NOT be set as pending (auto-approved)
        assert!(!app.session_state.has_pending_permission());
    }

    #[test]
    fn test_permission_request_stored_when_tool_not_allowed() {
        let mut app = App::default();

        // Bash is NOT pre-allowed
        assert!(!app.session_state.is_tool_allowed("Bash"));

        // Receive a permission request for Bash
        app.handle_message(AppMessage::PermissionRequested {
            permission_id: "perm-store".to_string(),
            tool_name: "Bash".to_string(),
            description: "Run npm install".to_string(),
            tool_input: Some(serde_json::json!({"command": "npm install"})),
        });

        // Permission SHOULD be set as pending
        assert!(app.session_state.has_pending_permission());

        // Verify the stored permission data
        let perm = app.session_state.pending_permission.as_ref().unwrap();
        assert_eq!(perm.permission_id, "perm-store");
        assert_eq!(perm.tool_name, "Bash");
        assert_eq!(perm.description, "Run npm install");
        assert!(perm.tool_input.is_some());
    }

    #[test]
    fn test_allow_always_persists_for_subsequent_requests() {
        let mut app = App::default();

        // First request: user presses 'a' to allow always
        use crate::state::PermissionRequest;
        app.session_state
            .set_pending_permission(PermissionRequest {
                permission_id: "perm-first".to_string(),
                tool_name: "Read".to_string(),
                description: "Read file".to_string(),
                context: None,
                tool_input: None,
            });
        app.handle_permission_key('a');

        // Verify Read is now allowed
        assert!(app.session_state.is_tool_allowed("Read"));

        // Second request for Read tool (simulated)
        app.handle_message(AppMessage::PermissionRequested {
            permission_id: "perm-second".to_string(),
            tool_name: "Read".to_string(),
            description: "Read another file".to_string(),
            tool_input: None,
        });

        // Should be auto-approved (no pending permission)
        assert!(!app.session_state.has_pending_permission());
    }

    #[test]
    fn test_skills_injected_message() {
        let mut app = App::default();
        assert!(app.session_state.skills.is_empty());

        app.handle_message(AppMessage::SkillsInjected {
            skills: vec!["commit".to_string(), "review".to_string()],
        });

        assert_eq!(app.session_state.skills.len(), 2);
        assert!(app.session_state.has_skill("commit"));
        assert!(app.session_state.has_skill("review"));
    }

    #[test]
    fn test_skills_injected_deduplication() {
        let mut app = App::default();

        app.handle_message(AppMessage::SkillsInjected {
            skills: vec!["commit".to_string()],
        });
        app.handle_message(AppMessage::SkillsInjected {
            skills: vec!["commit".to_string(), "review".to_string()],
        });

        assert_eq!(app.session_state.skills.len(), 2);
    }

    #[test]
    fn test_oauth_consent_required_message() {
        let mut app = App::default();
        assert!(!app.session_state.needs_oauth());
        assert!(app.session_state.oauth_url.is_none());

        app.handle_message(AppMessage::OAuthConsentRequired {
            provider: "github".to_string(),
            url: Some("https://github.com/oauth".to_string()),
            skill_name: Some("git-commit".to_string()),
        });

        assert!(app.session_state.needs_oauth());
        assert_eq!(
            app.session_state.oauth_required,
            Some(("github".to_string(), "git-commit".to_string()))
        );
        assert_eq!(
            app.session_state.oauth_url,
            Some("https://github.com/oauth".to_string())
        );
    }

    #[test]
    fn test_oauth_consent_without_url() {
        let mut app = App::default();

        app.handle_message(AppMessage::OAuthConsentRequired {
            provider: "google".to_string(),
            url: None,
            skill_name: Some("calendar".to_string()),
        });

        assert!(app.session_state.needs_oauth());
        assert!(app.session_state.oauth_url.is_none());
    }

    #[test]
    fn test_context_compacted_message() {
        let mut app = App::default();
        assert!(app.session_state.context_tokens_used.is_none());
        assert!(app.session_state.context_token_limit.is_none());

        app.handle_message(AppMessage::ContextCompacted {
            tokens_used: Some(45_000),
            token_limit: Some(100_000),
        });

        assert_eq!(app.session_state.context_tokens_used, Some(45_000));
        assert_eq!(app.session_state.context_token_limit, Some(100_000));
    }

    #[test]
    fn test_context_compacted_updates_existing() {
        let mut app = App::default();
        app.session_state.set_context_tokens(30_000);
        app.session_state.set_context_token_limit(100_000);

        app.handle_message(AppMessage::ContextCompacted {
            tokens_used: Some(50_000),
            token_limit: None, // Don't update limit
        });

        assert_eq!(app.session_state.context_tokens_used, Some(50_000));
        assert_eq!(app.session_state.context_token_limit, Some(100_000));
    }

    #[test]
    fn test_thread_metadata_updated_updates_thread() {
        let mut app = App::default();

        // Create a thread
        let thread_id = app
            .cache
            .create_streaming_thread("Original Title".to_string());

        // Update metadata via message
        app.handle_message(AppMessage::ThreadMetadataUpdated {
            thread_id: thread_id.clone(),
            title: Some("Updated Title".to_string()),
            description: Some("New Description".to_string()),
        });

        // Verify the thread was updated
        let thread = app.cache.get_thread(&thread_id).unwrap();
        assert_eq!(thread.title, "Updated Title");
        assert_eq!(thread.description, Some("New Description".to_string()));
    }

    #[test]
    fn test_thread_metadata_updated_partial_update() {
        let mut app = App::default();

        // Create a thread
        let thread_id = app
            .cache
            .create_streaming_thread("Original Title".to_string());

        // Update only description
        app.handle_message(AppMessage::ThreadMetadataUpdated {
            thread_id: thread_id.clone(),
            title: None,
            description: Some("Just a description".to_string()),
        });

        // Verify title unchanged, description updated
        let thread = app.cache.get_thread(&thread_id).unwrap();
        assert_eq!(thread.title, "Original Title");
        assert_eq!(
            thread.description,
            Some("Just a description".to_string())
        );
    }

    #[test]
    fn test_thread_metadata_updated_nonexistent_thread() {
        let mut app = App::default();

        // Try to update a thread that doesn't exist
        app.handle_message(AppMessage::ThreadMetadataUpdated {
            thread_id: "nonexistent-thread".to_string(),
            title: Some("Title".to_string()),
            description: Some("Description".to_string()),
        });

        // Should not panic, just do nothing
        assert!(app.cache.get_thread("nonexistent-thread").is_none());
    }

    // ============= Subagent Handler Tests =============

    #[test]
    fn test_handle_subagent_started() {
        use crate::models::SubagentEventStatus;

        let mut app = App::default();
        let thread_id = app.cache.create_streaming_thread("Test".to_string());
        app.active_thread_id = Some(thread_id.clone());

        app.handle_message(AppMessage::SubagentStarted {
            task_id: "task-123".to_string(),
            description: "Exploring codebase".to_string(),
            subagent_type: "Explore".to_string(),
        });

        // Verify subagent event was added to streaming message
        let messages = app.cache.get_messages(&thread_id).unwrap();
        let assistant_msg = messages
            .iter()
            .find(|m| m.role == MessageRole::Assistant)
            .unwrap();

        let subagent = assistant_msg.get_subagent_event("task-123");
        assert!(subagent.is_some());
        let subagent = subagent.unwrap();
        assert_eq!(subagent.description, "Exploring codebase");
        assert_eq!(subagent.subagent_type, "Explore");
        assert_eq!(subagent.status, SubagentEventStatus::Running);
    }

    #[test]
    fn test_handle_subagent_started_without_active_thread() {
        let mut app = App::default();
        // No active thread set

        // Should not panic when handling SubagentStarted without active thread
        app.handle_message(AppMessage::SubagentStarted {
            task_id: "task-orphan".to_string(),
            description: "Orphan task".to_string(),
            subagent_type: "general-purpose".to_string(),
        });

        // No crash means success - subagent event simply not stored
    }

    #[test]
    fn test_handle_subagent_progress() {
        let mut app = App::default();
        let thread_id = app.cache.create_streaming_thread("Test".to_string());
        app.active_thread_id = Some(thread_id.clone());

        // Start subagent first
        app.handle_message(AppMessage::SubagentStarted {
            task_id: "task-456".to_string(),
            description: "Running tests".to_string(),
            subagent_type: "test-agent".to_string(),
        });

        // Send progress update
        app.handle_message(AppMessage::SubagentProgress {
            task_id: "task-456".to_string(),
            message: "Scanning test files".to_string(),
        });

        // Verify progress was updated
        let messages = app.cache.get_messages(&thread_id).unwrap();
        let assistant_msg = messages
            .iter()
            .find(|m| m.role == MessageRole::Assistant)
            .unwrap();

        let subagent = assistant_msg.get_subagent_event("task-456").unwrap();
        assert_eq!(subagent.progress_message, Some("Scanning test files".to_string()));
    }

    #[test]
    fn test_handle_subagent_progress_multiple_updates() {
        let mut app = App::default();
        let thread_id = app.cache.create_streaming_thread("Test".to_string());
        app.active_thread_id = Some(thread_id.clone());

        // Start subagent
        app.handle_message(AppMessage::SubagentStarted {
            task_id: "task-multi".to_string(),
            description: "Multi-step task".to_string(),
            subagent_type: "Explore".to_string(),
        });

        // Send multiple progress updates
        app.handle_message(AppMessage::SubagentProgress {
            task_id: "task-multi".to_string(),
            message: "Step 1".to_string(),
        });
        app.handle_message(AppMessage::SubagentProgress {
            task_id: "task-multi".to_string(),
            message: "Step 2".to_string(),
        });
        app.handle_message(AppMessage::SubagentProgress {
            task_id: "task-multi".to_string(),
            message: "Step 3".to_string(),
        });

        // Verify last progress message is stored
        let messages = app.cache.get_messages(&thread_id).unwrap();
        let assistant_msg = messages
            .iter()
            .find(|m| m.role == MessageRole::Assistant)
            .unwrap();

        let subagent = assistant_msg.get_subagent_event("task-multi").unwrap();
        assert_eq!(subagent.progress_message, Some("Step 3".to_string()));
    }

    #[test]
    fn test_handle_subagent_completed_with_summary() {
        use crate::models::SubagentEventStatus;

        let mut app = App::default();
        let thread_id = app.cache.create_streaming_thread("Test".to_string());
        app.active_thread_id = Some(thread_id.clone());

        // Start subagent
        app.handle_message(AppMessage::SubagentStarted {
            task_id: "task-789".to_string(),
            description: "Code analysis".to_string(),
            subagent_type: "general-purpose".to_string(),
        });

        // Complete subagent with summary
        app.handle_message(AppMessage::SubagentCompleted {
            task_id: "task-789".to_string(),
            summary: "Found 5 issues".to_string(),
            tool_call_count: Some(12),
        });

        // Verify subagent was completed
        let messages = app.cache.get_messages(&thread_id).unwrap();
        let assistant_msg = messages
            .iter()
            .find(|m| m.role == MessageRole::Assistant)
            .unwrap();

        let subagent = assistant_msg.get_subagent_event("task-789").unwrap();
        assert_eq!(subagent.status, SubagentEventStatus::Complete);
        assert_eq!(subagent.summary, Some("Found 5 issues".to_string()));
        assert_eq!(subagent.tool_call_count, 12);
    }

    #[test]
    fn test_handle_subagent_completed_without_summary() {
        use crate::models::SubagentEventStatus;

        let mut app = App::default();
        let thread_id = app.cache.create_streaming_thread("Test".to_string());
        app.active_thread_id = Some(thread_id.clone());

        // Start subagent
        app.handle_message(AppMessage::SubagentStarted {
            task_id: "task-no-summary".to_string(),
            description: "Quick task".to_string(),
            subagent_type: "Bash".to_string(),
        });

        // Complete subagent without summary
        app.handle_message(AppMessage::SubagentCompleted {
            task_id: "task-no-summary".to_string(),
            summary: String::new(), // Empty summary
            tool_call_count: None,
        });

        // Verify subagent was completed
        let messages = app.cache.get_messages(&thread_id).unwrap();
        let assistant_msg = messages
            .iter()
            .find(|m| m.role == MessageRole::Assistant)
            .unwrap();

        let subagent = assistant_msg.get_subagent_event("task-no-summary").unwrap();
        assert_eq!(subagent.status, SubagentEventStatus::Complete);
        // Empty summary is converted to None by the handler
        assert!(subagent.summary.is_none());
        assert_eq!(subagent.tool_call_count, 0);
    }

    #[test]
    fn test_handle_subagent_full_lifecycle() {
        use crate::models::SubagentEventStatus;

        let mut app = App::default();
        let thread_id = app.cache.create_streaming_thread("Test".to_string());
        app.active_thread_id = Some(thread_id.clone());

        // Start
        app.handle_message(AppMessage::SubagentStarted {
            task_id: "lifecycle-task".to_string(),
            description: "Full lifecycle test".to_string(),
            subagent_type: "Explore".to_string(),
        });

        // Verify started state
        let messages = app.cache.get_messages(&thread_id).unwrap();
        let assistant_msg = messages
            .iter()
            .find(|m| m.role == MessageRole::Assistant)
            .unwrap();
        let subagent = assistant_msg.get_subagent_event("lifecycle-task").unwrap();
        assert_eq!(subagent.status, SubagentEventStatus::Running);
        assert!(subagent.progress_message.is_none());
        assert!(subagent.summary.is_none());

        // Progress
        app.handle_message(AppMessage::SubagentProgress {
            task_id: "lifecycle-task".to_string(),
            message: "In progress".to_string(),
        });

        let messages = app.cache.get_messages(&thread_id).unwrap();
        let assistant_msg = messages
            .iter()
            .find(|m| m.role == MessageRole::Assistant)
            .unwrap();
        let subagent = assistant_msg.get_subagent_event("lifecycle-task").unwrap();
        assert_eq!(subagent.status, SubagentEventStatus::Running);
        assert_eq!(subagent.progress_message, Some("In progress".to_string()));

        // Complete
        app.handle_message(AppMessage::SubagentCompleted {
            task_id: "lifecycle-task".to_string(),
            summary: "Task completed successfully".to_string(),
            tool_call_count: Some(5),
        });

        let messages = app.cache.get_messages(&thread_id).unwrap();
        let assistant_msg = messages
            .iter()
            .find(|m| m.role == MessageRole::Assistant)
            .unwrap();
        let subagent = assistant_msg.get_subagent_event("lifecycle-task").unwrap();
        assert_eq!(subagent.status, SubagentEventStatus::Complete);
        assert_eq!(subagent.summary, Some("Task completed successfully".to_string()));
        assert_eq!(subagent.tool_call_count, 5);
    }

    #[test]
    fn test_handle_multiple_subagents() {
        use crate::models::SubagentEventStatus;

        let mut app = App::default();
        let thread_id = app.cache.create_streaming_thread("Test".to_string());
        app.active_thread_id = Some(thread_id.clone());

        // Start multiple subagents
        app.handle_message(AppMessage::SubagentStarted {
            task_id: "task-a".to_string(),
            description: "Task A".to_string(),
            subagent_type: "Explore".to_string(),
        });
        app.handle_message(AppMessage::SubagentStarted {
            task_id: "task-b".to_string(),
            description: "Task B".to_string(),
            subagent_type: "general-purpose".to_string(),
        });

        // Progress on task B
        app.handle_message(AppMessage::SubagentProgress {
            task_id: "task-b".to_string(),
            message: "B progress".to_string(),
        });

        // Complete task A
        app.handle_message(AppMessage::SubagentCompleted {
            task_id: "task-a".to_string(),
            summary: "A done".to_string(),
            tool_call_count: Some(3),
        });

        // Verify independent state
        let messages = app.cache.get_messages(&thread_id).unwrap();
        let assistant_msg = messages
            .iter()
            .find(|m| m.role == MessageRole::Assistant)
            .unwrap();

        let subagent_a = assistant_msg.get_subagent_event("task-a").unwrap();
        assert_eq!(subagent_a.status, SubagentEventStatus::Complete);
        assert_eq!(subagent_a.summary, Some("A done".to_string()));

        let subagent_b = assistant_msg.get_subagent_event("task-b").unwrap();
        assert_eq!(subagent_b.status, SubagentEventStatus::Running);
        assert_eq!(subagent_b.progress_message, Some("B progress".to_string()));
    }
}
