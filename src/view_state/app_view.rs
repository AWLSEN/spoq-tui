//! Main view state struct for UI rendering
//!
//! This module provides the `AppViewState` struct, which contains all data
//! that UI components need to render without requiring access to the full App.

use crate::app::{Focus, Screen};
use crate::cache::ThreadCache;
use crate::markdown::MarkdownCache;
use crate::models::{Folder, Message, Thread};
use crate::rendered_lines_cache::RenderedLinesCache;
use crate::state::{SessionState, Todo};

use super::{DashboardViewState, ScrollState, SessionViewState, StreamingState};

// We use the UI's SystemStats directly since that's what App stores
use crate::ui::dashboard::SystemStats;

/// Complete view state for UI rendering.
///
/// This struct aggregates all data that UI components need to render,
/// using references to avoid unnecessary cloning. The lifetime `'a`
/// represents the borrow from the App struct.
///
/// ## Design Goals
///
/// 1. **Break circular dependency**: UI modules can import this struct
///    without importing App, eliminating the app ↔ ui cycle.
///
/// 2. **Pure rendering**: UI components become pure functions:
///    `fn render(view: &AppViewState, registry: &mut HitAreaRegistry) -> pixels`
///
/// 3. **Efficient**: Uses references wherever possible to avoid cloning.
///
/// 4. **Complete**: Contains everything needed to render any UI component.
pub struct AppViewState<'a> {
    // =========================================================================
    // Core Application State
    // =========================================================================
    /// Current screen being displayed
    pub screen: Screen,

    /// Current focus panel
    pub focus: Focus,

    /// Terminal dimensions
    pub terminal_width: u16,
    pub terminal_height: u16,

    /// Tick counter for animations (cursor blink, spinners)
    pub tick_count: u64,

    /// Flag indicating if the app should quit
    pub should_quit: bool,

    // =========================================================================
    // Thread Data
    // =========================================================================
    /// Thread and message cache (for read access)
    pub cache: &'a ThreadCache,

    /// Active thread ID
    pub active_thread_id: Option<&'a str>,

    /// All threads (for thread list)
    pub threads: &'a [crate::state::Thread],

    /// Selected thread index
    pub threads_index: usize,

    // =========================================================================
    // Scroll and Viewport
    // =========================================================================
    /// Scroll state
    pub scroll: ScrollState,

    // =========================================================================
    // Streaming State
    // =========================================================================
    /// Streaming state
    pub streaming: StreamingState,

    // =========================================================================
    // Session State (view)
    // =========================================================================
    /// Session view state
    pub session: SessionViewState,

    /// Full session state reference (for permission checks, etc.)
    pub session_state: &'a SessionState,

    // =========================================================================
    // Dashboard State (view)
    // =========================================================================
    /// Dashboard view state
    pub dashboard: DashboardViewState,

    /// Full dashboard state reference (for overlay rendering)
    pub dashboard_state: &'a crate::state::DashboardState,

    // =========================================================================
    // System Stats
    // =========================================================================
    /// System statistics
    pub system_stats: &'a SystemStats,

    // =========================================================================
    // Input State
    // =========================================================================
    /// Input textarea content
    pub input_content: String,

    /// Input cursor position
    pub input_cursor: (usize, usize),

    /// Whether input has content
    pub input_has_content: bool,

    // =========================================================================
    // Folder Picker State
    // =========================================================================
    /// Available folders for @ mentions
    pub folders: &'a [Folder],

    /// Currently selected folder
    pub selected_folder: Option<&'a Folder>,

    /// Folder picker visibility
    pub folder_picker_visible: bool,

    /// Folder picker filter text
    pub folder_picker_filter: &'a str,

    /// Folder picker cursor position
    pub folder_picker_cursor: usize,

    // =========================================================================
    // Todos
    // =========================================================================
    /// Current todos from assistant
    pub todos: &'a [Todo],

    // =========================================================================
    // Connection State
    // =========================================================================
    /// WebSocket connection status
    pub connection_status: bool,

    /// WebSocket connection state for detailed UI
    pub ws_connection_state: crate::websocket::WsConnectionState,

    // =========================================================================
    // UI Flags
    // =========================================================================
    /// Whether there are visible links in messages
    pub has_visible_links: bool,

    /// Migration progress (0-100), None when complete
    pub migration_progress: Option<u8>,

    // =========================================================================
    // Caches (mutable references for in-place updates)
    // =========================================================================
    /// Markdown cache
    pub markdown_cache: &'a mut MarkdownCache,

    /// Rendered lines cache
    pub rendered_lines_cache: &'a mut RenderedLinesCache,
}

impl<'a> AppViewState<'a> {
    /// Get the active thread if one is selected
    pub fn active_thread(&self) -> Option<&Thread> {
        self.active_thread_id
            .and_then(|id| self.cache.get_thread(id))
    }

    /// Get messages for the active thread
    pub fn active_messages(&self) -> Option<&[Message]> {
        self.active_thread_id
            .and_then(|id| self.cache.get_messages(id))
            .map(|v| v.as_slice())
    }

    /// Get errors for the active thread
    pub fn active_errors(&self) -> Option<&[crate::models::ErrorInfo]> {
        self.active_thread_id
            .and_then(|id| self.cache.get_errors(id))
            .map(|v| v.as_slice())
    }

    /// Check if there's a stream error
    pub fn has_stream_error(&self) -> bool {
        self.streaming.has_error()
    }

    /// Get the stream error message
    pub fn stream_error(&self) -> Option<&str> {
        self.streaming.stream_error.as_deref()
    }

    /// Check if streaming is active
    pub fn is_streaming(&self) -> bool {
        self.streaming.is_active()
    }

    /// Check if on conversation screen
    pub fn is_conversation_screen(&self) -> bool {
        self.screen == Screen::Conversation
    }

    /// Check if on command deck screen
    pub fn is_command_deck_screen(&self) -> bool {
        self.screen == Screen::CommandDeck
    }

    /// Get filtered folders based on current filter
    pub fn filtered_folders(&self) -> Vec<&Folder> {
        if self.folder_picker_filter.is_empty() {
            self.folders.iter().collect()
        } else {
            let filter_lower = self.folder_picker_filter.to_lowercase();
            self.folders
                .iter()
                .filter(|f| {
                    f.name.to_lowercase().contains(&filter_lower)
                        || f.path.to_lowercase().contains(&filter_lower)
                })
                .collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Full AppViewState tests require a complete App mock,
    // which is complex. Individual component tests are in their
    // respective modules.

    #[test]
    fn test_scroll_state_defaults() {
        let scroll = ScrollState::default();
        assert!(scroll.is_at_bottom());
        assert!(!scroll.user_has_scrolled);
    }

    #[test]
    fn test_streaming_state_defaults() {
        let streaming = StreamingState::default();
        assert!(!streaming.is_active());
        assert!(!streaming.has_error());
    }

    #[test]
    fn test_session_view_state_defaults() {
        let session = SessionViewState::default();
        assert_eq!(session.skills_count, 0);
        assert!(!session.has_pending_permission);
    }

    #[test]
    fn test_dashboard_view_state_defaults() {
        let dashboard = DashboardViewState::default();
        assert!(!dashboard.has_overlay);
        assert_eq!(dashboard.thread_count, 0);
    }
}
