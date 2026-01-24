//! View state construction for UI rendering.
//!
//! This module provides the `view_state()` method on App that constructs
//! an `AppViewState` containing all data needed for UI rendering.

use super::App;
use crate::view_state::{
    AppViewState, DashboardViewState, ScrollState, SessionViewState, StreamingState,
};

impl App {
    /// Create a view state for UI rendering.
    ///
    /// This method constructs an `AppViewState` that contains all the data
    /// UI components need to render, using references where possible to avoid
    /// cloning.
    ///
    /// The view state breaks the circular dependency between `app` and `ui`
    /// modules by providing a struct that UI can import without importing App.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let view = app.view_state();
    /// ui::render(frame, &view, &mut app.hit_registry);
    /// ```
    pub fn view_state(&mut self) -> AppViewState<'_> {
        // Build scroll state
        let scroll = ScrollState {
            unified_scroll: self.unified_scroll,
            max_scroll: self.max_scroll,
            user_has_scrolled: self.user_has_scrolled,
            scroll_velocity: self.scroll_velocity,
            scroll_position: self.scroll_position,
            scroll_boundary_hit: self.scroll_boundary_hit,
            boundary_hit_tick: self.boundary_hit_tick,
            input_section_start: self.input_section_start,
            total_content_lines: self.total_content_lines,
        };

        // Build streaming state
        let streaming = StreamingState {
            is_streaming: self
                .active_thread_id
                .as_ref()
                .and_then(|id| self.cache.get_messages(id))
                .map(|msgs| msgs.iter().any(|m| m.is_streaming))
                .unwrap_or(false),
            // Check if any message has non-empty reasoning content but is still streaming
            is_thinking: self
                .active_thread_id
                .as_ref()
                .and_then(|id| self.cache.get_messages(id))
                .map(|msgs| {
                    msgs.iter()
                        .any(|m| m.is_streaming && !m.reasoning_content.is_empty())
                })
                .unwrap_or(false),
            stream_error: self.stream_error.clone(),
            tick_count: self.tick_count,
        };

        // Build session view state
        let session = SessionViewState {
            skills_count: self.session_state.skills.len(),
            context_tokens_used: self.session_state.context_tokens_used,
            context_token_limit: self.session_state.context_token_limit,
            has_pending_permission: self.session_state.has_pending_permission(),
            needs_oauth: self.session_state.needs_oauth(),
        };

        // Build dashboard view state
        let dashboard = DashboardViewState {
            filter: self.dashboard.filter().and_then(|f| {
                // Convert from ui::dashboard::FilterState to view_state::dashboard_view::FilterState
                // Note: ui::dashboard::FilterState::All maps to None in the view state
                match f {
                    crate::ui::dashboard::FilterState::All => None,
                    crate::ui::dashboard::FilterState::Working => {
                        Some(crate::view_state::dashboard_view::FilterState::Working)
                    }
                    crate::ui::dashboard::FilterState::ReadyToTest => {
                        Some(crate::view_state::dashboard_view::FilterState::ReadyToTest)
                    }
                    crate::ui::dashboard::FilterState::Idle => {
                        Some(crate::view_state::dashboard_view::FilterState::Idle)
                    }
                }
            }),
            has_overlay: self.dashboard.overlay().is_some(),
            thread_count: self.dashboard.thread_count(),
            action_count: self
                .dashboard
                .aggregate()
                .count(crate::models::dashboard::ThreadStatus::Waiting)
                as usize,
        };

        // Build input state
        let (input_content, input_cursor) = {
            let content = self.textarea.lines().join("\n");
            let cursor = self.textarea.cursor();
            (content, cursor)
        };
        let input_has_content = !input_content.is_empty();

        AppViewState {
            // Core state
            screen: self.screen,
            focus: self.focus,
            terminal_width: self.terminal_width,
            terminal_height: self.terminal_height,
            tick_count: self.tick_count,
            should_quit: self.should_quit,

            // Thread data
            cache: &self.cache,
            active_thread_id: self.active_thread_id.as_deref(),
            threads: &self.threads,
            threads_index: self.threads_index,

            // Scroll and viewport
            scroll,

            // Streaming
            streaming,

            // Session
            session,
            session_state: &self.session_state,

            // Dashboard
            dashboard,
            dashboard_state: &self.dashboard,

            // System stats
            system_stats: &self.system_stats,

            // Input
            input_content,
            input_cursor,
            input_has_content,

            // Folder picker
            folders: &self.folders,
            selected_folder: self.selected_folder.as_ref(),
            folder_picker_visible: self.folder_picker_visible,
            folder_picker_filter: &self.folder_picker_filter,
            folder_picker_cursor: self.folder_picker_cursor,

            // Todos
            todos: &self.todos,

            // Connection
            connection_status: self.connection_status,
            ws_connection_state: self.ws_connection_state.clone(),

            // UI flags
            has_visible_links: self.has_visible_links,
            migration_progress: self.migration_progress,

            // Caches (mutable for in-place updates during rendering)
            markdown_cache: &mut self.markdown_cache,
            rendered_lines_cache: &mut self.rendered_lines_cache,
        }
    }
}
