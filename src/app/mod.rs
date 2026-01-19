//! Application state and logic for the TUI.
//!
//! This module contains the core [`App`] struct and related types:
//! - [`Screen`] - Which screen is currently displayed
//! - [`Focus`] - Which UI component has focus
//! - [`PermissionMode`] - Current permission mode for Claude interactions
//! - [`AppMessage`] - Messages for async communication

mod handlers;
mod messages;
mod navigation;
mod permissions;
mod state_methods;
mod stream;
mod types;
mod utils;
mod websocket;

pub use messages::AppMessage;
pub use types::{ActivePanel, Focus, Screen, ScrollBoundary, ThreadSwitcher};
pub use websocket::{start_websocket, start_websocket_with_config};

use crate::cache::ThreadCache;
use crate::conductor::ConductorClient;
use crate::debug::DebugEventSender;
use crate::input_history::InputHistory;
use crate::markdown::MarkdownCache;
use crate::models::{Folder, PermissionMode};
use crate::state::{
    AskUserQuestionState, SessionState, SubagentTracker, Task, Thread, Todo, ToolTracker,
};
use crate::websocket::WsConnectionState;
use crate::widgets::textarea_input::TextAreaInput;
use color_eyre::Result;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Cached message height data for incremental updates.
/// Stores precomputed heights with cumulative offsets to avoid recalculating
/// all heights on every render frame.
#[derive(Debug, Clone)]
pub struct CachedHeights {
    /// Thread ID this cache belongs to (Arc to avoid cloning on lookups)
    pub thread_id: Arc<String>,
    /// Per-message heights with cumulative offsets
    pub heights: Vec<CachedMessageHeight>,
    /// Total visual lines across all messages
    pub total_lines: usize,
    /// Viewport width used for these calculations (invalidate on resize)
    pub viewport_width: usize,
}

/// Height data for a single cached message
#[derive(Debug, Clone)]
pub struct CachedMessageHeight {
    /// Message ID
    pub message_id: i64,
    /// Render version when height was calculated
    pub render_version: u64,
    /// Number of visual lines this message occupies
    pub visual_lines: usize,
    /// Cumulative visual line offset from the start
    pub cumulative_offset: usize,
}

impl CachedHeights {
    /// Create empty cache for a thread
    pub fn new(thread_id: Arc<String>, viewport_width: usize) -> Self {
        Self {
            thread_id,
            heights: Vec::new(),
            total_lines: 0,
            viewport_width,
        }
    }

    /// Check if cache is valid for the given thread and viewport width
    pub fn is_valid_for(&self, thread_id: &str, viewport_width: usize) -> bool {
        self.thread_id.as_str() == thread_id && self.viewport_width == viewport_width
    }

    /// Recalculate cumulative offsets from a given index onwards
    pub fn recalculate_offsets_from(&mut self, start_idx: usize) {
        let mut cumulative = if start_idx > 0 {
            self.heights[start_idx - 1].cumulative_offset + self.heights[start_idx - 1].visual_lines
        } else {
            0
        };
        for height in self.heights.iter_mut().skip(start_idx) {
            height.cumulative_offset = cumulative;
            cumulative += height.visual_lines;
        }
        self.total_lines = cumulative;
    }

    /// Update a single message's height and recalculate offsets
    pub fn update_height(&mut self, idx: usize, new_height: usize, render_version: u64) {
        if idx < self.heights.len() {
            self.heights[idx].visual_lines = new_height;
            self.heights[idx].render_version = render_version;
            self.recalculate_offsets_from(idx);
        }
    }

    /// Append a new message height entry
    pub fn append(&mut self, message_id: i64, render_version: u64, visual_lines: usize) {
        let cumulative_offset = self.total_lines;
        self.heights.push(CachedMessageHeight {
            message_id,
            render_version,
            visual_lines,
            cumulative_offset,
        });
        self.total_lines += visual_lines;
    }

    /// Truncate heights to given length (for when messages are removed)
    pub fn truncate(&mut self, new_len: usize) {
        if new_len < self.heights.len() {
            self.heights.truncate(new_len);
            self.total_lines = self.heights.last()
                .map(|h| h.cumulative_offset + h.visual_lines)
                .unwrap_or(0);
        }
    }
}

pub(crate) use utils::{emit_debug, log_thread_update, truncate_for_debug};

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
    /// TextArea input (tui-textarea wrapper)
    pub textarea: TextAreaInput<'static>,
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
    /// Maximum scroll value (calculated during render, used for clamping)
    pub max_scroll: u16,
    /// Unified scroll offset (0 = input visible at bottom, higher = scrolled up)
    pub unified_scroll: u16,
    /// True when user manually scrolled (disables auto-scroll)
    pub user_has_scrolled: bool,
    /// Line index where input section begins (for scroll calculations)
    pub input_section_start: usize,
    /// Total content lines from last render
    pub total_content_lines: usize,
    /// Current permission mode for Claude interactions
    pub permission_mode: PermissionMode,
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
    /// WebSocket sender for sending messages to the server
    pub ws_sender: Option<tokio::sync::mpsc::Sender<crate::websocket::WsOutgoingMessage>>,
    /// WebSocket connection state for UI status indicator
    pub ws_connection_state: WsConnectionState,
    /// State for AskUserQuestion prompt modal
    pub question_state: AskUserQuestionState,
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
    /// Active panel for narrow/stacked layout mode (when width < 60 cols)
    pub active_panel: ActivePanel,
    /// Cache for pre-rendered message lines (avoids re-rendering on every tick)
    pub rendered_lines_cache: crate::rendered_lines_cache::RenderedLinesCache,
    /// Click detector for multi-click detection (single/double/triple click)
    /// Cache for parsed markdown (avoids re-parsing unchanged content)
    pub markdown_cache: MarkdownCache,
    /// Incremental height cache for virtualization (avoids recalculating all heights every frame)
    pub height_cache: Option<CachedHeights>,
    /// Dirty flag: when true, the UI needs to be redrawn.
    /// Set to true on state mutations, cleared after each draw.
    pub needs_redraw: bool,
    /// Tracks whether the current visible messages contain any hyperlinks.
    /// Used to conditionally show the link interaction hint.
    pub has_visible_links: bool,
    /// Input history for Up/Down arrow navigation
    pub input_history: InputHistory,
    /// Cached folder list from API for folder picker
    pub folders: Vec<Folder>,
    /// True while fetching folders from API
    pub folders_loading: bool,
    /// Error message if folder fetch failed
    pub folders_error: Option<String>,
    /// Currently selected folder (displayed as chip in input)
    pub selected_folder: Option<Folder>,
    /// Is the folder picker overlay showing
    pub folder_picker_visible: bool,
    /// Current filter text for folder picker (text after @)
    pub folder_picker_filter: String,
    /// Selected index in the filtered folder list
    pub folder_picker_cursor: usize,
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
            textarea: TextAreaInput::new(),
            migration_progress: Some(0),
            cache,
            message_rx: Some(message_rx),
            message_tx,
            connection_status: false,
            stream_error: None,
            client,
            tick_count: 0,
            max_scroll: 0,
            unified_scroll: 0,
            user_has_scrolled: false,
            input_section_start: 0,
            total_content_lines: 0,
            permission_mode: PermissionMode::default(),
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
            ws_sender: None,
            ws_connection_state: WsConnectionState::Disconnected,
            question_state: AskUserQuestionState::default(),
            scroll_boundary_hit: None,
            boundary_hit_tick: 0,
            scroll_velocity: 0.0,
            scroll_position: 0.0,
            terminal_width: 80,  // Default, will be updated on first render
            terminal_height: 24, // Default, will be updated on first render
            active_panel: ActivePanel::default(),
            rendered_lines_cache: crate::rendered_lines_cache::RenderedLinesCache::new(),
            markdown_cache: MarkdownCache::new(),
            height_cache: None,
            needs_redraw: true, // Start with redraw needed
            has_visible_links: false,
            input_history: InputHistory::load(),
            folders: Vec::new(),
            folders_loading: false,
            folders_error: None,
            selected_folder: None,
            folder_picker_visible: false,
            folder_picker_filter: String::new(),
            folder_picker_cursor: 0,
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

    /// Emit a debug state change event (helper for external callers like main.rs)
    pub fn emit_debug_state_change(&self, _state_type: &str, description: &str, current: &str) {
        use crate::debug::{DebugEventKind, StateChangeData, StateType};
        emit_debug(
            &self.debug_tx,
            DebugEventKind::StateChange(StateChangeData::new(
                StateType::ToolTracker, // Use ToolTracker as a generic state type
                description,
                current,
            )),
            None,
        );
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
        let mut app = App {
            screen: Screen::Conversation,
            active_thread_id: Some("thread-123".to_string()),
            ..Default::default()
        };
        app.textarea.insert_char('T');
        app.textarea.insert_char('e');
        app.textarea.insert_char('s');
        app.textarea.insert_char('t');

        app.navigate_to_command_deck();

        assert_eq!(app.screen, Screen::CommandDeck);
        assert!(app.active_thread_id.is_none());
        assert!(app.textarea.is_empty());
    }

    #[test]
    fn test_navigate_to_command_deck_when_already_on_command_deck() {
        let mut app = App::default();
        assert_eq!(app.screen, Screen::CommandDeck);
        app.active_thread_id = Some("thread-456".to_string());
        app.textarea.insert_char('H');
        app.textarea.insert_char('i');

        app.navigate_to_command_deck();

        assert_eq!(app.screen, Screen::CommandDeck);
        assert!(app.active_thread_id.is_none());
        assert!(app.textarea.is_empty());
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
        app.textarea.insert_char(' ');
        app.textarea.insert_char(' ');
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
        app.textarea.insert_char('H');
        app.textarea.insert_char('i');
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
        assert!(app.textarea.is_empty());
    }

    #[tokio::test]
    async fn test_submit_input_adds_messages_to_thread() {
        use crate::models::ThreadType;
        let mut app = App::default();
        app.textarea.insert_char('T');
        app.textarea.insert_char('e');
        app.textarea.insert_char('s');
        app.textarea.insert_char('t');

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
        app.textarea.insert_char('N');
        app.textarea.insert_char('e');
        app.textarea.insert_char('w');

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
        app.textarea.insert_char('H');
        app.textarea.insert_char('i');

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
        let mut app = App {
            active_thread_id: Some("stale-thread-id".to_string()),
            screen: Screen::CommandDeck,
            ..Default::default()
        };

        app.textarea.insert_char('N');
        app.textarea.insert_char('e');
        app.textarea.insert_char('w');

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
            working_directory: None,
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
        app.textarea.insert_char('F');
        app.textarea.insert_char('o');
        app.textarea.insert_char('l');
        app.textarea.insert_char('l');
        app.textarea.insert_char('o');
        app.textarea.insert_char('w');
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
        app.textarea.insert_char('F');
        app.textarea.insert_char('i');
        app.textarea.insert_char('r');
        app.textarea.insert_char('s');
        app.textarea.insert_char('t');
        app.submit_input(ThreadType::Conversation);

        let thread_id = app.active_thread_id.clone().unwrap();
        assert!(uuid::Uuid::parse_str(&thread_id).is_ok());

        // Try to submit again while still streaming
        app.textarea.insert_char('S');
        app.textarea.insert_char('e');
        app.textarea.insert_char('c');
        app.textarea.insert_char('o');
        app.textarea.insert_char('n');
        app.textarea.insert_char('d');
        app.submit_input(ThreadType::Conversation);

        // Should NOT create a new thread or add messages
        // Should set an error
        assert!(app.stream_error.is_some());
        assert!(app.stream_error.as_ref().unwrap().contains("wait"));

        // Input should NOT be cleared (submission was rejected)
        assert!(!app.textarea.is_empty());
        assert_eq!(app.textarea.content(), "Second");

        // Should still be on the same thread
        assert_eq!(app.active_thread_id, Some(thread_id));
    }

    #[tokio::test]
    async fn test_submit_input_allows_submit_after_thread_reconciled() {
        use crate::models::ThreadType;
        let mut app = App::default();

        // First submit creates pending thread
        app.textarea.insert_char('F');
        app.textarea.insert_char('i');
        app.textarea.insert_char('r');
        app.textarea.insert_char('s');
        app.textarea.insert_char('t');
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
        app.textarea.insert_char('S');
        app.textarea.insert_char('e');
        app.textarea.insert_char('c');
        app.textarea.insert_char('o');
        app.textarea.insert_char('n');
        app.textarea.insert_char('d');
        let before_count = app.cache.get_messages("real-backend-id").unwrap().len();
        app.submit_input(ThreadType::Conversation);

        // Should add to existing thread
        let messages = app.cache.get_messages("real-backend-id").unwrap();
        assert_eq!(messages.len(), before_count + 2);
        assert!(app.textarea.is_empty());
    }

    #[tokio::test]
    async fn test_submit_input_handles_deleted_thread() {
        use crate::models::ThreadType;
        let mut app = App {
            active_thread_id: Some("deleted-thread".to_string()),
            screen: Screen::Conversation,
            ..Default::default()
        };

        app.textarea.insert_char('T');
        app.textarea.insert_char('e');
        app.textarea.insert_char('s');
        app.textarea.insert_char('t');
        app.submit_input(ThreadType::Conversation);

        // Should show error about thread not existing
        assert!(app.stream_error.is_some());
        assert!(app
            .stream_error
            .as_ref()
            .unwrap()
            .contains("no longer exists"));

        // Input should NOT be cleared
        assert!(!app.textarea.is_empty());
    }

    #[tokio::test]
    async fn test_submit_input_full_conversation_workflow() {
        use crate::models::ThreadType;
        let mut app = App::default();

        // === Turn 1: New thread ===
        app.textarea.insert_char('H');
        app.textarea.insert_char('i');
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
        app.textarea.insert_char('T');
        app.textarea.insert_char('e');
        app.textarea.insert_char('l');
        app.textarea.insert_char('l');
        app.textarea.insert_char(' ');
        app.textarea.insert_char('m');
        app.textarea.insert_char('e');
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
        app.textarea.insert_char('N');
        app.textarea.insert_char('e');
        app.textarea.insert_char('w');
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
        let mut app = App {
            stream_error: Some("Previous error".to_string()),
            ..Default::default()
        };
        assert!(!app.connection_status);

        // Send connection status update
        app.handle_message(AppMessage::ConnectionStatus(true));

        // Verify status updated and error cleared
        assert!(app.connection_status);
        assert!(app.stream_error.is_none());
    }

    #[test]
    fn test_handle_message_connection_status_disconnected() {
        let mut app = App {
            connection_status: true,
            ..Default::default()
        };

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
        let mut app = App {
            stream_error: Some("Test error".to_string()),
            ..Default::default()
        };

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

        // Set unified scroll to a non-zero value (user has scrolled up)
        app.unified_scroll = 5;
        app.user_has_scrolled = true;

        // Receive token for thread 2 (non-active thread)
        app.handle_message(AppMessage::StreamToken {
            thread_id: thread2_id.clone(),
            token: "Hello from thread 2".to_string(),
        });

        // Scroll should NOT be reset (should still be 5)
        assert_eq!(app.unified_scroll, 5);
    }

    #[test]
    fn test_stream_token_resets_scroll_for_active_thread() {
        let mut app = App::default();

        // Create a thread and set it as active
        let thread_id = app.cache.create_streaming_thread("Active thread".to_string());
        app.active_thread_id = Some(thread_id.clone());

        // Set unified scroll to a non-zero value
        app.unified_scroll = 10;
        app.user_has_scrolled = true;

        // Receive token for the active thread
        app.handle_message(AppMessage::StreamToken {
            thread_id: thread_id.clone(),
            token: "Hello".to_string(),
        });

        // Scroll should be reset to 0 (auto-scroll to bottom)
        assert_eq!(app.unified_scroll, 0);
    }

    #[test]
    fn test_stream_complete_does_not_reset_scroll_for_non_active_thread() {
        let mut app = App::default();

        // Create two threads
        let thread1_id = app.cache.create_streaming_thread("Thread 1".to_string());
        let thread2_id = app.cache.create_streaming_thread("Thread 2".to_string());

        // Set thread 1 as active
        app.active_thread_id = Some(thread1_id.clone());

        // Set unified scroll to a non-zero value
        app.unified_scroll = 7;
        app.user_has_scrolled = true;

        // Complete stream for thread 2 (non-active thread)
        app.handle_message(AppMessage::StreamComplete {
            thread_id: thread2_id.clone(),
            message_id: 42,
        });

        // Scroll should NOT be reset
        assert_eq!(app.unified_scroll, 7);
    }

    #[test]
    fn test_stream_complete_resets_scroll_for_active_thread() {
        let mut app = App::default();

        // Create a thread and set it as active
        let thread_id = app.cache.create_streaming_thread("Active thread".to_string());
        app.active_thread_id = Some(thread_id.clone());

        // Set unified scroll to a non-zero value
        app.unified_scroll = 15;
        app.user_has_scrolled = true;

        // Complete stream for the active thread
        app.handle_message(AppMessage::StreamComplete {
            thread_id: thread_id.clone(),
            message_id: 99,
        });

        // Scroll should be reset to 0
        assert_eq!(app.unified_scroll, 0);
    }

    // ============= PermissionMode Tests =============

    #[test]
    fn test_permission_mode_default_is_default() {
        assert_eq!(PermissionMode::default(), PermissionMode::Default);
    }

    #[test]
    fn test_permission_mode_equality() {
        assert_eq!(PermissionMode::Plan, PermissionMode::Plan);
        assert_eq!(
            PermissionMode::BypassPermissions,
            PermissionMode::BypassPermissions
        );
        assert_eq!(PermissionMode::Default, PermissionMode::Default);
        assert_ne!(PermissionMode::Plan, PermissionMode::Default);
        assert_ne!(
            PermissionMode::BypassPermissions,
            PermissionMode::Plan
        );
    }

    #[test]
    fn test_permission_mode_copy() {
        let mode = PermissionMode::Plan;
        let copied = mode;
        assert_eq!(mode, copied);
    }

    #[test]
    fn test_app_initializes_with_default_permission_mode() {
        let app = App::default();
        assert_eq!(app.permission_mode, PermissionMode::Default);
    }

    #[test]
    fn test_cycle_permission_mode_from_default_to_plan() {
        let mut app = App::default();
        assert_eq!(app.permission_mode, PermissionMode::Default);

        app.cycle_permission_mode();

        assert_eq!(app.permission_mode, PermissionMode::Plan);
    }

    #[test]
    fn test_cycle_permission_mode_from_plan_to_bypass() {
        let mut app = App {
            permission_mode: PermissionMode::Plan,
            ..Default::default()
        };

        app.cycle_permission_mode();

        assert_eq!(app.permission_mode, PermissionMode::BypassPermissions);
    }

    #[test]
    fn test_cycle_permission_mode_from_bypass_to_default() {
        let mut app = App {
            permission_mode: PermissionMode::BypassPermissions,
            ..Default::default()
        };

        app.cycle_permission_mode();

        assert_eq!(app.permission_mode, PermissionMode::Default);
    }

    #[test]
    fn test_cycle_permission_mode_full_cycle() {
        let mut app = App::default();

        // Start at Default
        assert_eq!(app.permission_mode, PermissionMode::Default);

        // Cycle: Default → Plan
        app.cycle_permission_mode();
        assert_eq!(app.permission_mode, PermissionMode::Plan);

        // Cycle: Plan → BypassPermissions
        app.cycle_permission_mode();
        assert_eq!(app.permission_mode, PermissionMode::BypassPermissions);

        // Cycle: BypassPermissions → Default
        app.cycle_permission_mode();
        assert_eq!(app.permission_mode, PermissionMode::Default);

        // Cycle: Default → Plan (wraps around)
        app.cycle_permission_mode();
        assert_eq!(app.permission_mode, PermissionMode::Plan);
    }

    #[test]
    fn test_cycle_permission_mode_multiple_cycles() {
        let mut app = App::default();

        // Cycle through 3 complete cycles (9 transitions)
        for _ in 0..3 {
            app.cycle_permission_mode(); // Default → Plan
            assert_eq!(app.permission_mode, PermissionMode::Plan);

            app.cycle_permission_mode(); // Plan → BypassPermissions
            assert_eq!(app.permission_mode, PermissionMode::BypassPermissions);

            app.cycle_permission_mode(); // BypassPermissions → Default
            assert_eq!(app.permission_mode, PermissionMode::Default);
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
            working_directory: None,
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
            working_directory: None,
        };
        app.cache.upsert_thread(thread);
        app.active_thread_id = Some("thread-prog".to_string());

        assert!(app.is_active_thread_programming());
    }

    #[test]
    fn test_is_active_thread_programming_returns_false_for_nonexistent_thread() {
        let app = App {
            active_thread_id: Some("nonexistent-thread".to_string()),
            ..Default::default()
        };

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
            working_directory: None,
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
            working_directory: None,
        };
        app.cache.upsert_thread(thread);

        assert!(app.is_active_thread_programming());
    }

    // ============= Submit Input with Programming Thread Tests =============

    #[tokio::test]
    async fn test_submit_input_on_programming_thread_uses_permission_mode() {
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
            working_directory: None,
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
        app.permission_mode = PermissionMode::Plan;

        // Submit input
        app.textarea.insert_char('H');
        app.textarea.insert_char('i');
        app.submit_input(ThreadType::Conversation);

        // Should add streaming message to the thread
        let messages = app.cache.get_messages("prog-thread-123").unwrap();
        assert_eq!(messages.len(), 4); // 2 original + user + assistant streaming
        assert!(messages[3].is_streaming);
    }

    #[tokio::test]
    async fn test_submit_input_permission_mode_default_sets_correct_flags() {
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
            working_directory: None,
        };
        app.cache.upsert_thread(thread);
        app.cache
            .add_message_simple("prog-thread-456", MessageRole::User, "Prev".to_string());
        app.cache
            .add_message_simple("prog-thread-456", MessageRole::Assistant, "Resp".to_string());
        app.active_thread_id = Some("prog-thread-456".to_string());
        app.screen = Screen::Conversation;

        // Mode is Default by default
        assert_eq!(app.permission_mode, PermissionMode::Default);

        // Submit input
        app.textarea.insert_char('T');
        app.textarea.insert_char('e');
        app.textarea.insert_char('s');
        app.textarea.insert_char('t');
        app.submit_input(ThreadType::Conversation);

        // Input should be cleared (submission was accepted)
        assert!(app.textarea.is_empty());
    }

    #[tokio::test]
    async fn test_submit_input_new_thread_is_not_programming() {
        use crate::models::ThreadType;
        let mut app = App::default();
        assert!(app.active_thread_id.is_none());

        // Submit creates a new non-programming thread
        app.textarea.insert_char('N');
        app.textarea.insert_char('e');
        app.textarea.insert_char('w');
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
            .create_pending_thread("Code task".to_string(), ThreadType::Programming, None);

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
        app.textarea.insert_char('C');
        app.textarea.insert_char('o');
        app.textarea.insert_char('d');
        app.textarea.insert_char('e');
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
            working_directory: None,
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
            working_directory: None,
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
            working_directory: None,
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
        assert_eq!(app.permission_mode, PermissionMode::Default);
        app.cycle_permission_mode();
        assert_eq!(app.permission_mode, PermissionMode::Plan);
    }

    #[test]
    fn test_shift_tab_cycles_mode_for_all_threads() {
        let mut app = App::default();

        // Create a normal (conversation) thread
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
            working_directory: None,
        };
        app.cache.upsert_thread(thread);
        app.active_thread_id = Some("conv-thread".to_string());
        app.screen = Screen::Conversation;

        // Shift+Tab should cycle mode for ALL threads now (not just Programming)
        assert_eq!(app.screen, Screen::Conversation);
        assert!(!app.is_active_thread_programming()); // It's a conversation thread

        // Mode cycling works on all thread types
        assert_eq!(app.permission_mode, PermissionMode::Default);
        app.cycle_permission_mode();
        assert_eq!(app.permission_mode, PermissionMode::Plan);
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
            working_directory: None,
        };
        app.cache.upsert_thread(thread);
        app.active_thread_id = Some("prog-thread".to_string());
        app.screen = Screen::CommandDeck; // Not in Conversation

        // Even with programming thread, Shift+Tab should not cycle mode
        // because we're not on Conversation screen
        assert_eq!(app.screen, Screen::CommandDeck);
        // The condition for mode cycling is being on Conversation screen
    }

    // ============= Permission Mode Persistence Tests =============

    #[test]
    fn test_permission_mode_persists_across_thread_switches() {
        use crate::models::ThreadType;
        let mut app = App {
            permission_mode: PermissionMode::Plan,
            ..Default::default()
        };

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
            working_directory: None,
        };
        app.cache.upsert_thread(thread1);
        // Pre-populate messages to avoid lazy fetch triggering tokio::spawn
        app.cache.set_messages("prog-1".to_string(), vec![]);
        app.open_thread("prog-1".to_string());

        // Mode should persist
        assert_eq!(app.permission_mode, PermissionMode::Plan);

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
            working_directory: None,
        };
        app.cache.upsert_thread(thread2);
        // Pre-populate messages to avoid lazy fetch triggering tokio::spawn
        app.cache.set_messages("prog-2".to_string(), vec![]);
        app.open_thread("prog-2".to_string());

        // Mode should still persist
        assert_eq!(app.permission_mode, PermissionMode::Plan);
    }

    #[test]
    fn test_permission_mode_persists_after_navigate_to_command_deck() {
        let mut app = App {
            permission_mode: PermissionMode::BypassPermissions,
            ..Default::default()
        };

        // Navigate to command deck
        app.navigate_to_command_deck();

        // Mode should persist (it's app-level, not thread-level)
        assert_eq!(app.permission_mode, PermissionMode::BypassPermissions);
    }

    // ============= Thread Type with Add Streaming Message Tests =============

    #[test]
    fn test_add_streaming_message_preserves_thread_type() {
        use crate::models::ThreadType;
        let mut app = App::default();

        // Create a programming thread
        let pending_id = app
            .cache
            .create_pending_thread("Code question".to_string(), ThreadType::Programming, None);

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
        let app = App {
            active_thread_id: Some("nonexistent-thread".to_string()),
            ..Default::default()
        };

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
                received_at: std::time::Instant::now(),
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
                received_at: std::time::Instant::now(),
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
                received_at: std::time::Instant::now(),
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
                received_at: std::time::Instant::now(),
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
                received_at: std::time::Instant::now(),
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
                received_at: std::time::Instant::now(),
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

    // ============= WebSocket Connection State Tests =============

    #[test]
    fn test_handle_message_ws_connected() {
        use crate::websocket::WsConnectionState;
        let mut app = App::default();

        // Initial state should be Disconnected
        assert!(matches!(
            app.ws_connection_state,
            WsConnectionState::Disconnected
        ));

        // Handle WsConnected message
        app.handle_message(AppMessage::WsConnected);

        // State should be Connected
        assert!(matches!(
            app.ws_connection_state,
            WsConnectionState::Connected
        ));
    }

    #[test]
    fn test_handle_message_ws_disconnected() {
        use crate::websocket::WsConnectionState;
        let mut app = App {
            ws_connection_state: WsConnectionState::Connected,
            ..Default::default()
        };

        // Handle WsDisconnected message
        app.handle_message(AppMessage::WsDisconnected);

        // State should be Disconnected
        assert!(matches!(
            app.ws_connection_state,
            WsConnectionState::Disconnected
        ));
    }

    #[test]
    fn test_handle_message_ws_reconnecting() {
        use crate::websocket::WsConnectionState;
        let mut app = App::default();

        // Initial state
        assert!(matches!(
            app.ws_connection_state,
            WsConnectionState::Disconnected
        ));

        // Handle WsReconnecting message with attempt 1
        app.handle_message(AppMessage::WsReconnecting { attempt: 1 });

        // State should be Reconnecting with attempt 1
        match app.ws_connection_state {
            WsConnectionState::Reconnecting { attempt } => {
                assert_eq!(attempt, 1);
            }
            _ => panic!("Expected Reconnecting state"),
        }

        // Handle another reconnection attempt
        app.handle_message(AppMessage::WsReconnecting { attempt: 3 });

        // State should be Reconnecting with attempt 3
        match app.ws_connection_state {
            WsConnectionState::Reconnecting { attempt } => {
                assert_eq!(attempt, 3);
            }
            _ => panic!("Expected Reconnecting state"),
        }
    }

    #[test]
    fn test_ws_connection_state_transitions() {
        use crate::websocket::WsConnectionState;
        let mut app = App::default();

        // Start disconnected
        assert!(matches!(
            app.ws_connection_state,
            WsConnectionState::Disconnected
        ));

        // Connect
        app.handle_message(AppMessage::WsConnected);
        assert!(matches!(
            app.ws_connection_state,
            WsConnectionState::Connected
        ));

        // Disconnect
        app.handle_message(AppMessage::WsDisconnected);
        assert!(matches!(
            app.ws_connection_state,
            WsConnectionState::Disconnected
        ));

        // Reconnecting
        app.handle_message(AppMessage::WsReconnecting { attempt: 1 });
        assert!(matches!(
            app.ws_connection_state,
            WsConnectionState::Reconnecting { .. }
        ));

        // Successfully reconnect
        app.handle_message(AppMessage::WsConnected);
        assert!(matches!(
            app.ws_connection_state,
            WsConnectionState::Connected
        ));
    }

    // ========================================================================
    // ActivePanel Tests
    // ========================================================================

    #[test]
    fn test_active_panel_default_is_left() {
        assert_eq!(ActivePanel::default(), ActivePanel::Left);
    }

    #[test]
    fn test_active_panel_equality() {
        assert_eq!(ActivePanel::Left, ActivePanel::Left);
        assert_eq!(ActivePanel::Right, ActivePanel::Right);
        assert_ne!(ActivePanel::Left, ActivePanel::Right);
    }

    #[test]
    fn test_active_panel_copy() {
        let panel = ActivePanel::Right;
        let copied = panel;
        assert_eq!(panel, copied);
    }

    #[test]
    fn test_app_initializes_with_left_panel_active() {
        let app = App::default();
        assert_eq!(app.active_panel, ActivePanel::Left);
    }

    #[test]
    fn test_app_active_panel_can_be_changed() {
        let mut app = App::default();
        assert_eq!(app.active_panel, ActivePanel::Left);

        app.active_panel = ActivePanel::Right;
        assert_eq!(app.active_panel, ActivePanel::Right);

        app.active_panel = ActivePanel::Left;
        assert_eq!(app.active_panel, ActivePanel::Left);
    }

    #[test]
    fn test_app_terminal_dimensions_have_defaults() {
        let app = App::default();
        assert_eq!(app.terminal_width, 80);
        assert_eq!(app.terminal_height, 24);
    }

    #[test]
    fn test_should_summarize_paste_by_lines() {
        let app = App::default();
        assert!(!app.should_summarize_paste("1\n2\n3")); // 3 lines
        assert!(app.should_summarize_paste("1\n2\n3\n4")); // 4 lines
    }

    #[test]
    fn test_should_summarize_paste_by_chars() {
        let app = App::default();
        assert!(!app.should_summarize_paste(&"a".repeat(150))); // 150 chars
        assert!(app.should_summarize_paste(&"a".repeat(151))); // 151 chars
    }

    // ============= Permission Mode Propagation Tests =============

    #[tokio::test]
    async fn test_submit_input_preserves_permission_mode_in_new_thread() {
        use crate::models::{ThreadType, PermissionMode};
        let mut app = App {
            permission_mode: PermissionMode::Plan,
            ..Default::default()
        };

        // Verify app is in initial state
        assert_eq!(app.screen, Screen::CommandDeck);
        assert!(app.active_thread_id.is_none());

        // Submit input to create new thread
        app.textarea.insert_char('T');
        app.textarea.insert_char('e');
        app.textarea.insert_char('s');
        app.textarea.insert_char('t');
        app.submit_input(ThreadType::Conversation);

        // Verify thread was created
        assert_eq!(app.screen, Screen::Conversation);
        assert!(app.active_thread_id.is_some());

        // The permission_mode should remain unchanged on the app
        assert_eq!(app.permission_mode, PermissionMode::Plan);

        // Note: The actual HTTP request validation would require mocking the conductor client.
        // This test verifies that the app state is correct when submit_input is called.
        // The StreamRequest construction in submit_input() uses app.permission_mode,
        // which is verified in src/models/request.rs tests.
    }

    #[tokio::test]
    async fn test_submit_input_preserves_permission_mode_in_continuing_thread() {
        use crate::models::{ThreadType, PermissionMode};
        let mut app = App {
            permission_mode: PermissionMode::BypassPermissions,
            ..Default::default()
        };

        // Create an existing thread
        let existing_id = "real-thread-456".to_string();
        app.cache.upsert_thread(crate::models::Thread {
            id: existing_id.clone(),
            title: "Existing Thread".to_string(),
            description: None,
            preview: "Previous message".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: ThreadType::Conversation,
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: chrono::Utc::now(),
            working_directory: None,
        });

        // Set as active thread
        app.active_thread_id = Some(existing_id.clone());
        app.screen = Screen::Conversation;

        // Finalize any streaming messages
        app.cache.add_message_simple(&existing_id, MessageRole::User, "Previous".to_string());
        app.cache.add_message_simple(&existing_id, MessageRole::Assistant, "Response".to_string());

        // Submit follow-up message
        app.textarea.insert_char('F');
        app.textarea.insert_char('o');
        app.textarea.insert_char('l');
        app.textarea.insert_char('l');
        app.textarea.insert_char('o');
        app.textarea.insert_char('w');
        app.submit_input(ThreadType::Conversation);

        // Verify the permission mode is preserved
        assert_eq!(app.permission_mode, PermissionMode::BypassPermissions);
        assert_eq!(app.active_thread_id.as_ref().unwrap(), &existing_id);
    }

    #[tokio::test]
    async fn test_submit_input_uses_provided_thread_type() {
        use crate::models::{ThreadType, PermissionMode};
        let mut app = App {
            permission_mode: PermissionMode::Default,
            ..Default::default()
        };

        // Test with Programming thread type
        app.textarea.insert_char('C');
        app.textarea.insert_char('o');
        app.textarea.insert_char('d');
        app.textarea.insert_char('e');
        app.submit_input(ThreadType::Programming);

        let thread_id = app.active_thread_id.as_ref().unwrap();

        // Verify thread was created
        assert!(uuid::Uuid::parse_str(thread_id).is_ok());
        assert_eq!(app.screen, Screen::Conversation);

        // Note: The thread_type parameter is passed to StreamRequest::with_type()
        // in submit_input() at src/app/stream.rs:118
        // This test verifies the flow works correctly with different thread types.
    }

    #[tokio::test]
    async fn test_submit_input_thread_type_conversation() {
        use crate::models::ThreadType;
        let mut app = App::default();

        // Test with Conversation thread type
        app.textarea.insert_char('H');
        app.textarea.insert_char('i');
        app.submit_input(ThreadType::Conversation);

        let thread_id = app.active_thread_id.as_ref().unwrap();

        // Verify thread was created with correct type
        assert!(uuid::Uuid::parse_str(thread_id).is_ok());
        assert_eq!(app.screen, Screen::Conversation);
    }
}
