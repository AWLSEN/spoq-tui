//! Dashboard state management
//!
//! This module provides the state container for the multi-thread dashboard view,
//! managing thread data, computed views, and overlay states.

use crate::models::dashboard::{Aggregate, PlanSummary, ThreadStatus, WaitingFor};
use crate::models::Thread;
use crate::view_state::{
    FilterState, OverlayState, Progress, RenderContext, SystemStats, Theme, ThreadView,
};
use crate::websocket::messages::PhaseStatus;
use std::collections::{HashMap, HashSet};

// ============================================================================
// PhaseProgressData
// ============================================================================

/// Data for tracking phase progress during plan execution
///
/// This struct holds the current state of a phase for display in the dashboard.
#[derive(Debug, Clone)]
pub struct PhaseProgressData {
    /// Current phase index (0-based)
    pub phase_index: u32,
    /// Total number of phases in the plan
    pub total_phases: u32,
    /// Name of the current phase
    pub phase_name: String,
    /// Status of the phase (Starting, Running, Completed, Failed)
    pub status: PhaseStatus,
    /// Number of tools used in this phase
    pub tool_count: u32,
    /// Name of the last tool used
    pub last_tool: String,
    /// Last file modified (optional)
    pub last_file: Option<String>,
}

impl PhaseProgressData {
    /// Create a new PhaseProgressData instance
    pub fn new(
        phase_index: u32,
        total_phases: u32,
        phase_name: String,
        status: PhaseStatus,
        tool_count: u32,
        last_tool: String,
        last_file: Option<String>,
    ) -> Self {
        Self {
            phase_index,
            total_phases,
            phase_name,
            status,
            tool_count,
            last_tool,
            last_file,
        }
    }
}

// ============================================================================
// DashboardState
// ============================================================================

/// State container for the multi-thread dashboard view
///
/// Owns thread data and produces RenderContext for rendering.
/// Maintains computed views with dirty tracking for efficiency.
#[derive(Debug)]
pub struct DashboardState {
    /// Thread data indexed by thread_id
    threads: HashMap<String, Thread>,
    /// Agent state data by thread_id: (state, current_operation)
    /// Used for fallback status inference and current_operation display
    agent_states: HashMap<String, (String, Option<String>)>,
    /// What each thread is waiting for
    waiting_for: HashMap<String, WaitingFor>,
    /// Plan requests pending approval: thread_id -> (request_id, summary)
    plan_requests: HashMap<String, (String, PlanSummary)>,
    /// Thread IDs verified locally (backend fallback)
    locally_verified: HashSet<String>,
    /// Phase progress data by thread_id during plan execution
    phase_progress: HashMap<String, PhaseProgressData>,

    /// Current filter state (None means show all)
    filter: Option<FilterState>,
    /// Current overlay state (if an overlay is open)
    overlay: Option<OverlayState>,
    /// Cached aggregate statistics
    aggregate: Aggregate,

    /// Cached computed thread views (sorted: needs_action first, then by updated_at)
    thread_views: Vec<ThreadView>,
    /// True when threads/waiting_for changed and views need recomputation
    thread_views_dirty: bool,
}

impl Default for DashboardState {
    fn default() -> Self {
        Self::new()
    }
}

impl DashboardState {
    /// Create a new empty dashboard state
    pub fn new() -> Self {
        Self {
            threads: HashMap::new(),
            agent_states: HashMap::new(),
            waiting_for: HashMap::new(),
            plan_requests: HashMap::new(),
            locally_verified: HashSet::new(),
            phase_progress: HashMap::new(),
            filter: None,
            overlay: None,
            aggregate: Aggregate::new(),
            thread_views: Vec::new(),
            thread_views_dirty: true,
        }
    }

    // ========================================================================
    // Data Updates (from WS/REST handlers)
    // ========================================================================

    /// Replace all threads and recompute aggregate
    ///
    /// Called when receiving full thread list from backend.
    /// Note: agent_states here only contains state strings; current_operation
    /// will be populated by subsequent WebSocket updates.
    pub fn set_threads(&mut self, threads: Vec<Thread>, agent_states: &HashMap<String, String>) {
        self.threads.clear();
        for thread in threads {
            self.threads.insert(thread.id.clone(), thread);
        }
        // Convert HashMap<String, String> to HashMap<String, (String, Option<String>)>
        // with None for current_operation (will be populated by WebSocket updates)
        self.agent_states = agent_states
            .iter()
            .map(|(k, v)| (k.clone(), (v.clone(), None)))
            .collect();
        self.recompute_aggregate();
        self.thread_views_dirty = true;
    }

    /// Add a single thread (from WebSocket thread_created event)
    ///
    /// If the thread already exists, it will be replaced.
    pub fn add_thread(&mut self, thread: Thread) {
        self.threads.insert(thread.id.clone(), thread);
        self.recompute_aggregate();
        self.thread_views_dirty = true;
    }

    /// Update a single thread's status
    pub fn update_thread_status(
        &mut self,
        thread_id: &str,
        status: ThreadStatus,
        waiting_for: Option<WaitingFor>,
    ) {
        if let Some(thread) = self.threads.get_mut(thread_id) {
            thread.status = Some(status);
        } else {
            tracing::warn!("Received status update for unknown thread: {}", thread_id);
        }

        if let Some(wf) = waiting_for {
            self.waiting_for.insert(thread_id.to_string(), wf);
        } else {
            self.waiting_for.remove(thread_id);
        }

        self.recompute_aggregate();
        self.thread_views_dirty = true;
    }

    /// Store a plan request for approval
    pub fn set_plan_request(&mut self, thread_id: &str, request_id: String, summary: PlanSummary) {
        self.plan_requests
            .insert(thread_id.to_string(), (request_id, summary));
    }

    /// Update agent state for a thread (fallback status inference)
    ///
    /// # Arguments
    /// * `thread_id` - The thread to update
    /// * `state` - The agent state (e.g., "running", "streaming", "idle")
    /// * `current_operation` - Optional description of what the agent is doing
    pub fn update_agent_state(
        &mut self,
        thread_id: &str,
        state: &str,
        current_operation: Option<&str>,
    ) {
        self.agent_states.insert(
            thread_id.to_string(),
            (state.to_string(), current_operation.map(|s| s.to_string())),
        );
        self.recompute_aggregate();
        self.thread_views_dirty = true;
    }

    /// Mark a thread as verified locally
    pub fn mark_verified_local(&mut self, thread_id: &str) {
        self.locally_verified.insert(thread_id.to_string());
        self.thread_views_dirty = true;
    }

    /// Remove a plan request (after approval/rejection)
    pub fn remove_plan_request(&mut self, thread_id: &str) {
        self.plan_requests.remove(thread_id);
    }

    /// Clear waiting_for state for a thread
    pub fn clear_waiting_for(&mut self, thread_id: &str) {
        self.waiting_for.remove(thread_id);
        self.thread_views_dirty = true;
    }

    /// Update a thread's title and/or description
    ///
    /// Called when receiving thread_updated events from WebSocket.
    pub fn update_thread_metadata(
        &mut self,
        thread_id: &str,
        title: Option<String>,
        description: Option<String>,
    ) {
        if let Some(thread) = self.threads.get_mut(thread_id) {
            if let Some(t) = title {
                thread.title = t;
            }
            if let Some(d) = description {
                thread.description = Some(d);
            }
            self.thread_views_dirty = true;
        }
    }

    /// Update a thread's mode (normal, plan, exec)
    ///
    /// Called when receiving thread mode updates from WebSocket.
    pub fn update_thread_mode(&mut self, thread_id: &str, mode: crate::models::ThreadMode) {
        if let Some(thread) = self.threads.get_mut(thread_id) {
            thread.mode = mode;
        }
        self.thread_views_dirty = true;
    }

    /// Update a thread's verification status
    ///
    /// Called when receiving thread verified events from WebSocket.
    pub fn update_thread_verified(
        &mut self,
        thread_id: &str,
        verified_at: chrono::DateTime<chrono::Utc>,
    ) {
        if let Some(thread) = self.threads.get_mut(thread_id) {
            thread.verified = Some(true);
            thread.verified_at = Some(verified_at);
        }
        self.thread_views_dirty = true;
    }

    /// Update phase progress for a thread during plan execution
    ///
    /// Called when receiving phase progress updates from WebSocket.
    pub fn update_phase_progress(&mut self, thread_id: &str, progress: PhaseProgressData) {
        self.phase_progress.insert(thread_id.to_string(), progress);
        self.thread_views_dirty = true;
    }

    /// Clear phase progress for a thread
    ///
    /// Called when a phase completes or the plan execution ends.
    pub fn clear_phase_progress(&mut self, thread_id: &str) {
        self.phase_progress.remove(thread_id);
        self.thread_views_dirty = true;
    }

    /// Get phase progress for a thread
    pub fn get_phase_progress(&self, thread_id: &str) -> Option<&PhaseProgressData> {
        self.phase_progress.get(thread_id)
    }

    // ========================================================================
    // UI State (from click handlers)
    // ========================================================================

    /// Toggle a filter on/off (set if different, clear if same)
    pub fn toggle_filter(&mut self, filter: FilterState) {
        if self.filter == Some(filter) {
            self.filter = None;
        } else {
            self.filter = Some(filter);
        }
    }

    /// Clear any active filter
    pub fn clear_filter(&mut self) {
        self.filter = None;
    }

    /// Get current filter state
    pub fn filter(&self) -> Option<FilterState> {
        self.filter
    }

    /// Expand a thread to show its overlay
    ///
    /// The overlay type depends on what the thread is waiting for:
    /// - PlanApproval -> Plan overlay
    /// - Permission -> No overlay (shown inline)
    /// - UserInput/None -> Question overlay
    pub fn expand_thread(&mut self, thread_id: &str, anchor_y: u16) {
        let thread = match self.threads.get(thread_id) {
            Some(t) => t,
            None => return,
        };

        let waiting_for = self.waiting_for.get(thread_id);

        self.overlay = match waiting_for {
            Some(WaitingFor::PlanApproval { .. }) => {
                if let Some((request_id, summary)) = self.plan_requests.get(thread_id) {
                    Some(OverlayState::Plan {
                        thread_id: thread_id.to_string(),
                        thread_title: thread.title.clone(),
                        repository: thread.display_repository(),
                        request_id: request_id.clone(),
                        summary: summary.clone(),
                        scroll_offset: 0,
                        anchor_y,
                    })
                } else {
                    // No plan details available, don't open overlay
                    return;
                }
            }
            Some(WaitingFor::Permission { .. }) => {
                // Permissions show inline, no overlay
                return;
            }
            Some(WaitingFor::UserInput) | None => {
                // Question overlay - thread.pending_question if available
                Some(OverlayState::Question {
                    thread_id: thread_id.to_string(),
                    thread_title: thread.title.clone(),
                    repository: thread.display_repository(),
                    question: String::new(), // Will be populated from thread data
                    options: vec![],
                    anchor_y,
                })
            }
        };
    }

    /// Close the current overlay
    pub fn collapse_overlay(&mut self) {
        self.overlay = None;
    }

    /// Get current overlay state
    pub fn overlay(&self) -> Option<&OverlayState> {
        self.overlay.as_ref()
    }

    /// Switch from Question overlay to FreeForm input
    pub fn show_free_form(&mut self, thread_id: &str) {
        if let Some(OverlayState::Question {
            thread_id: tid,
            thread_title,
            repository,
            question,
            anchor_y,
            ..
        }) = self.overlay.take()
        {
            if tid == thread_id {
                self.overlay = Some(OverlayState::FreeForm {
                    thread_id: tid,
                    thread_title,
                    repository,
                    question,
                    input: String::new(),
                    cursor_pos: 0,
                    anchor_y,
                });
            }
        }
    }

    /// Switch from FreeForm back to Question overlay
    pub fn back_to_options(&mut self, thread_id: &str) {
        if let Some(OverlayState::FreeForm {
            thread_id: tid,
            thread_title,
            repository,
            question,
            anchor_y,
            ..
        }) = self.overlay.take()
        {
            if tid == thread_id {
                self.overlay = Some(OverlayState::Question {
                    thread_id: tid,
                    thread_title,
                    repository,
                    question,
                    options: vec![],
                    anchor_y,
                });
            }
        }
    }

    /// Update free form input text and cursor position
    pub fn update_free_form_input(&mut self, text: String, cursor_pos: usize) {
        if let Some(OverlayState::FreeForm {
            ref mut input,
            cursor_pos: ref mut pos,
            ..
        }) = self.overlay
        {
            *input = text;
            *pos = cursor_pos;
        }
    }

    /// Scroll plan overlay
    pub fn scroll_plan(&mut self, delta: i16) {
        if let Some(OverlayState::Plan {
            ref mut scroll_offset,
            ..
        }) = self.overlay
        {
            let new_offset = (*scroll_offset as i16).saturating_add(delta);
            *scroll_offset = new_offset.max(0) as usize;
        }
    }

    // ========================================================================
    // Computed Views (for rendering)
    // ========================================================================

    /// Build a render context for the dashboard
    ///
    /// This method ensures thread views are fresh before returning the context.
    /// If the views are dirty, it recomputes them first.
    pub fn build_render_context<'a>(
        &'a mut self,
        system_stats: &'a SystemStats,
        theme: &'a Theme,
    ) -> RenderContext<'a> {
        // Ensure thread views are fresh before building context
        self.compute_thread_views();

        RenderContext::new(&self.thread_views, &self.aggregate, system_stats, theme)
            .with_filter(self.filter)
            .with_overlay(self.overlay.as_ref())
    }

    /// Compute and cache thread views if dirty
    ///
    /// Returns a reference to the cached views.
    /// Views are sorted: needs_action first, then by updated_at (most recent first).
    pub fn compute_thread_views(&mut self) -> &[ThreadView] {
        if self.thread_views_dirty {
            self.thread_views = self.build_thread_views();
            self.thread_views_dirty = false;
        }
        &self.thread_views
    }

    /// Get what a thread is waiting for
    pub fn get_waiting_for(&self, thread_id: &str) -> Option<&WaitingFor> {
        self.waiting_for.get(thread_id)
    }

    /// Get plan request ID for a thread
    pub fn get_plan_request_id(&self, thread_id: &str) -> Option<&str> {
        self.plan_requests.get(thread_id).map(|(id, _)| id.as_str())
    }

    /// Get a thread by ID
    pub fn get_thread(&self, thread_id: &str) -> Option<&Thread> {
        self.threads.get(thread_id)
    }

    /// Get all threads as an iterator
    pub fn threads(&self) -> impl Iterator<Item = &Thread> {
        self.threads.values()
    }

    /// Get the number of threads
    pub fn thread_count(&self) -> usize {
        self.threads.len()
    }

    /// Get aggregate statistics
    pub fn aggregate(&self) -> &Aggregate {
        &self.aggregate
    }

    /// Check if a thread is locally verified
    pub fn is_locally_verified(&self, thread_id: &str) -> bool {
        self.locally_verified.contains(thread_id)
    }

    // ========================================================================
    // Private Helpers
    // ========================================================================

    /// Recompute aggregate statistics from current thread data
    fn recompute_aggregate(&mut self) {
        let mut aggregate = Aggregate::new();

        for thread in self.threads.values() {
            let status = thread.effective_status(&self.agent_states);
            aggregate.increment(status);
        }

        self.aggregate = aggregate;
    }

    /// Build thread views from current data
    fn build_thread_views(&self) -> Vec<ThreadView> {
        // Progress is now imported at the top from view_state

        let mut views: Vec<ThreadView> =
            self.threads
                .values()
                .map(|thread| {
                    let status = thread.effective_status(&self.agent_states);
                    let waiting_for = self.waiting_for.get(&thread.id).cloned();

                    // Use thread.mode directly from the Thread model
                    let mode = thread.mode;

                    // Look up phase progress and create Progress if status is Running or Starting
                    let progress =
                        self.get_phase_progress(&thread.id)
                            .and_then(|phase_data| match phase_data.status {
                                PhaseStatus::Running | PhaseStatus::Starting => Some(
                                    Progress::new(phase_data.phase_index, phase_data.total_phases),
                                ),
                                _ => None,
                            });

                    // Get current_operation from agent state
                    let current_operation = thread
                        .current_operation(&self.agent_states)
                        .map(|s| s.to_string());

                    ThreadView::new(
                        thread.id.clone(),
                        thread.title.clone(),
                        thread.display_repository(),
                    )
                    .with_mode(mode)
                    .with_status(status)
                    .with_waiting_for(waiting_for)
                    .with_progress(progress)
                    .with_duration(thread.display_duration())
                    .with_current_operation(current_operation)
                })
                .collect();

        // Sort: needs_action first, then by updated_at (most recent first)
        views.sort_by(|a, b| {
            match (a.needs_action, b.needs_action) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => {
                    // Both have same needs_action status, sort by updated_at
                    // We need to get the original threads to compare dates
                    let a_thread = self.threads.get(&a.id);
                    let b_thread = self.threads.get(&b.id);
                    match (a_thread, b_thread) {
                        (Some(at), Some(bt)) => bt.updated_at.cmp(&at.updated_at),
                        _ => std::cmp::Ordering::Equal,
                    }
                }
            }
        });

        views
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_thread(id: &str, title: &str) -> Thread {
        Thread {
            id: id.to_string(),
            title: title.to_string(),
            description: None,
            preview: String::new(),
            updated_at: Utc::now(),
            thread_type: crate::models::ThreadType::Programming,
            mode: crate::models::ThreadMode::default(),
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: Utc::now(),
            working_directory: Some("/Users/test/project".to_string()),
            status: None,
            verified: None,
            verified_at: None,
        }
    }

    // -------------------- Filter Tests --------------------

    #[test]
    fn test_toggle_filter_sets_filter() {
        let mut state = DashboardState::new();
        assert_eq!(state.filter(), None);

        state.toggle_filter(FilterState::Working);
        assert_eq!(state.filter(), Some(FilterState::Working));
    }

    #[test]
    fn test_toggle_filter_clears_same_filter() {
        let mut state = DashboardState::new();
        state.toggle_filter(FilterState::Working);
        assert_eq!(state.filter(), Some(FilterState::Working));

        state.toggle_filter(FilterState::Working);
        assert_eq!(state.filter(), None);
    }

    #[test]
    fn test_toggle_filter_switches_filter() {
        let mut state = DashboardState::new();
        state.toggle_filter(FilterState::Working);
        assert_eq!(state.filter(), Some(FilterState::Working));

        state.toggle_filter(FilterState::Idle);
        assert_eq!(state.filter(), Some(FilterState::Idle));
    }

    #[test]
    fn test_clear_filter() {
        let mut state = DashboardState::new();
        state.toggle_filter(FilterState::Working);
        state.clear_filter();
        assert_eq!(state.filter(), None);
    }

    // -------------------- Overlay Tests --------------------

    #[test]
    fn test_expand_thread_no_waiting_creates_question_overlay() {
        let mut state = DashboardState::new();
        let thread = make_thread("t1", "Test Thread");
        state.threads.insert("t1".to_string(), thread);

        state.expand_thread("t1", 10);

        assert!(state.overlay().is_some());
        if let Some(OverlayState::Question {
            thread_id,
            anchor_y,
            ..
        }) = state.overlay()
        {
            assert_eq!(thread_id, "t1");
            assert_eq!(*anchor_y, 10);
        } else {
            panic!("Expected Question overlay");
        }
    }

    #[test]
    fn test_expand_thread_with_plan_creates_plan_overlay() {
        let mut state = DashboardState::new();
        let thread = make_thread("t1", "Test Thread");
        state.threads.insert("t1".to_string(), thread);
        state.waiting_for.insert(
            "t1".to_string(),
            WaitingFor::PlanApproval {
                request_id: "req-123".to_string(),
            },
        );
        state.plan_requests.insert(
            "t1".to_string(),
            (
                "req-123".to_string(),
                PlanSummary::new("Test".to_string(), vec!["Phase 1".to_string()], 3, Some(1000)),
            ),
        );

        state.expand_thread("t1", 5);

        assert!(state.overlay().is_some());
        if let Some(OverlayState::Plan {
            thread_id,
            request_id,
            ..
        }) = state.overlay()
        {
            assert_eq!(thread_id, "t1");
            assert_eq!(request_id, "req-123");
        } else {
            panic!("Expected Plan overlay");
        }
    }

    #[test]
    fn test_expand_thread_with_permission_does_not_create_overlay() {
        let mut state = DashboardState::new();
        let thread = make_thread("t1", "Test Thread");
        state.threads.insert("t1".to_string(), thread);
        state.waiting_for.insert(
            "t1".to_string(),
            WaitingFor::Permission {
                request_id: "req-1".to_string(),
                tool_name: "Bash".to_string(),
            },
        );

        state.expand_thread("t1", 10);

        assert!(state.overlay().is_none());
    }

    #[test]
    fn test_collapse_overlay() {
        let mut state = DashboardState::new();
        let thread = make_thread("t1", "Test Thread");
        state.threads.insert("t1".to_string(), thread);
        state.expand_thread("t1", 10);
        assert!(state.overlay().is_some());

        state.collapse_overlay();
        assert!(state.overlay().is_none());
    }

    #[test]
    fn test_show_free_form() {
        let mut state = DashboardState::new();
        let thread = make_thread("t1", "Test Thread");
        state.threads.insert("t1".to_string(), thread);
        state.expand_thread("t1", 10);

        state.show_free_form("t1");

        if let Some(OverlayState::FreeForm {
            thread_id,
            input,
            cursor_pos,
            ..
        }) = state.overlay()
        {
            assert_eq!(thread_id, "t1");
            assert!(input.is_empty());
            assert_eq!(*cursor_pos, 0);
        } else {
            panic!("Expected FreeForm overlay");
        }
    }

    #[test]
    fn test_back_to_options() {
        let mut state = DashboardState::new();
        let thread = make_thread("t1", "Test Thread");
        state.threads.insert("t1".to_string(), thread);
        state.expand_thread("t1", 10);
        state.show_free_form("t1");

        state.back_to_options("t1");

        if let Some(OverlayState::Question { thread_id, .. }) = state.overlay() {
            assert_eq!(thread_id, "t1");
        } else {
            panic!("Expected Question overlay");
        }
    }

    #[test]
    fn test_update_free_form_input() {
        let mut state = DashboardState::new();
        let thread = make_thread("t1", "Test Thread");
        state.threads.insert("t1".to_string(), thread);
        state.expand_thread("t1", 10);
        state.show_free_form("t1");

        state.update_free_form_input("Hello world".to_string(), 5);

        if let Some(OverlayState::FreeForm {
            input, cursor_pos, ..
        }) = state.overlay()
        {
            assert_eq!(input, "Hello world");
            assert_eq!(*cursor_pos, 5);
        } else {
            panic!("Expected FreeForm overlay");
        }
    }

    #[test]
    fn test_scroll_plan() {
        let mut state = DashboardState::new();
        let thread = make_thread("t1", "Test Thread");
        state.threads.insert("t1".to_string(), thread);
        state.waiting_for.insert(
            "t1".to_string(),
            WaitingFor::PlanApproval {
                request_id: "req-1".to_string(),
            },
        );
        state.plan_requests.insert(
            "t1".to_string(),
            (
                "req-1".to_string(),
                PlanSummary::new("Test".to_string(), vec![], 0, None),
            ),
        );
        state.expand_thread("t1", 10);

        state.scroll_plan(5);

        if let Some(OverlayState::Plan { scroll_offset, .. }) = state.overlay() {
            assert_eq!(*scroll_offset, 5);
        } else {
            panic!("Expected Plan overlay");
        }
    }

    #[test]
    fn test_scroll_plan_negative_clamped() {
        let mut state = DashboardState::new();
        let thread = make_thread("t1", "Test Thread");
        state.threads.insert("t1".to_string(), thread);
        state.waiting_for.insert(
            "t1".to_string(),
            WaitingFor::PlanApproval {
                request_id: "req-1".to_string(),
            },
        );
        state.plan_requests.insert(
            "t1".to_string(),
            (
                "req-1".to_string(),
                PlanSummary::new("Test".to_string(), vec![], 0, None),
            ),
        );
        state.expand_thread("t1", 10);

        state.scroll_plan(-10);

        if let Some(OverlayState::Plan { scroll_offset, .. }) = state.overlay() {
            assert_eq!(*scroll_offset, 0);
        } else {
            panic!("Expected Plan overlay");
        }
    }

    // -------------------- Thread Data Tests --------------------

    #[test]
    fn test_set_threads() {
        let mut state = DashboardState::new();
        let threads = vec![make_thread("t1", "Thread 1"), make_thread("t2", "Thread 2")];
        let agent_states = HashMap::new();

        state.set_threads(threads, &agent_states);

        assert_eq!(state.thread_count(), 2);
        assert!(state.get_thread("t1").is_some());
        assert!(state.get_thread("t2").is_some());
    }

    #[test]
    fn test_update_thread_status() {
        let mut state = DashboardState::new();
        let thread = make_thread("t1", "Test Thread");
        state.threads.insert("t1".to_string(), thread);

        state.update_thread_status("t1", ThreadStatus::Waiting, Some(WaitingFor::UserInput));

        let thread = state.get_thread("t1").unwrap();
        assert_eq!(thread.status, Some(ThreadStatus::Waiting));
        assert!(state.get_waiting_for("t1").is_some());
    }

    #[test]
    fn test_set_plan_request() {
        let mut state = DashboardState::new();
        let summary = PlanSummary::new(
            "Test Plan".to_string(),
            vec!["Phase 1".to_string()],
            5,
            Some(10000),
        );

        state.set_plan_request("t1", "req-123".to_string(), summary);

        assert_eq!(state.get_plan_request_id("t1"), Some("req-123"));
    }

    #[test]
    fn test_mark_verified_local() {
        let mut state = DashboardState::new();
        assert!(!state.is_locally_verified("t1"));

        state.mark_verified_local("t1");
        assert!(state.is_locally_verified("t1"));
    }

    // -------------------- Thread Views Tests --------------------

    #[test]
    fn test_compute_thread_views_sorts_by_needs_action() {
        let mut state = DashboardState::new();

        let mut t1 = make_thread("t1", "Thread 1");
        t1.status = Some(ThreadStatus::Idle);
        t1.updated_at = Utc::now();

        let mut t2 = make_thread("t2", "Thread 2");
        t2.status = Some(ThreadStatus::Waiting);
        t2.updated_at = Utc::now();

        state.threads.insert("t1".to_string(), t1);
        state.threads.insert("t2".to_string(), t2);

        let views = state.compute_thread_views();

        // Waiting thread should come first (needs action)
        assert_eq!(views[0].id, "t2");
        assert_eq!(views[1].id, "t1");
    }

    #[test]
    fn test_compute_thread_views_cached() {
        let mut state = DashboardState::new();
        let thread = make_thread("t1", "Test Thread");
        state.threads.insert("t1".to_string(), thread);

        // First call computes
        let _ = state.compute_thread_views();
        assert!(!state.thread_views_dirty);

        // Second call uses cache
        let views = state.compute_thread_views();
        assert_eq!(views.len(), 1);
    }

    #[test]
    fn test_aggregate_updated_on_set_threads() {
        let mut state = DashboardState::new();
        let mut agent_states = HashMap::new();
        agent_states.insert("t1".to_string(), "running".to_string());
        agent_states.insert("t2".to_string(), "waiting".to_string());

        let threads = vec![make_thread("t1", "Thread 1"), make_thread("t2", "Thread 2")];

        state.set_threads(threads, &agent_states);

        assert_eq!(state.aggregate().working(), 2);
    }

    // -------------------- Phase Progress Tests --------------------

    #[test]
    fn test_update_phase_progress() {
        let mut state = DashboardState::new();
        let progress = PhaseProgressData::new(
            1,
            5,
            "Add WebSocket handlers".to_string(),
            PhaseStatus::Running,
            10,
            "Edit".to_string(),
            Some("/src/websocket/handlers.rs".to_string()),
        );

        state.update_phase_progress("t1", progress);

        let result = state.get_phase_progress("t1");
        assert!(result.is_some());
        let p = result.unwrap();
        assert_eq!(p.phase_index, 1);
        assert_eq!(p.total_phases, 5);
        assert_eq!(p.phase_name, "Add WebSocket handlers");
        assert_eq!(p.status, PhaseStatus::Running);
        assert_eq!(p.tool_count, 10);
        assert_eq!(p.last_tool, "Edit");
        assert_eq!(p.last_file, Some("/src/websocket/handlers.rs".to_string()));
    }

    #[test]
    fn test_update_phase_progress_marks_dirty() {
        let mut state = DashboardState::new();
        state.thread_views_dirty = false;

        let progress = PhaseProgressData::new(
            0,
            3,
            "Setup".to_string(),
            PhaseStatus::Starting,
            0,
            "".to_string(),
            None,
        );

        state.update_phase_progress("t1", progress);
        assert!(state.thread_views_dirty);
    }

    #[test]
    fn test_clear_phase_progress() {
        let mut state = DashboardState::new();
        let progress = PhaseProgressData::new(
            2,
            5,
            "Test phase".to_string(),
            PhaseStatus::Completed,
            15,
            "Bash".to_string(),
            None,
        );

        state.update_phase_progress("t1", progress);
        assert!(state.get_phase_progress("t1").is_some());

        state.clear_phase_progress("t1");
        assert!(state.get_phase_progress("t1").is_none());
    }

    #[test]
    fn test_clear_phase_progress_marks_dirty() {
        let mut state = DashboardState::new();
        let progress = PhaseProgressData::new(
            0,
            1,
            "Single phase".to_string(),
            PhaseStatus::Running,
            5,
            "Read".to_string(),
            None,
        );
        state.update_phase_progress("t1", progress);
        state.thread_views_dirty = false;

        state.clear_phase_progress("t1");
        assert!(state.thread_views_dirty);
    }

    #[test]
    fn test_get_phase_progress_nonexistent() {
        let state = DashboardState::new();
        assert!(state.get_phase_progress("nonexistent").is_none());
    }

    #[test]
    fn test_phase_progress_data_new() {
        let progress = PhaseProgressData::new(
            3,
            10,
            "Implement feature".to_string(),
            PhaseStatus::Failed,
            25,
            "Write".to_string(),
            Some("/src/feature.rs".to_string()),
        );

        assert_eq!(progress.phase_index, 3);
        assert_eq!(progress.total_phases, 10);
        assert_eq!(progress.phase_name, "Implement feature");
        assert_eq!(progress.status, PhaseStatus::Failed);
        assert_eq!(progress.tool_count, 25);
        assert_eq!(progress.last_tool, "Write");
        assert_eq!(progress.last_file, Some("/src/feature.rs".to_string()));
    }

    #[test]
    fn test_phase_progress_multiple_threads() {
        let mut state = DashboardState::new();

        let progress1 = PhaseProgressData::new(
            1,
            3,
            "Phase A".to_string(),
            PhaseStatus::Running,
            5,
            "Edit".to_string(),
            None,
        );
        let progress2 = PhaseProgressData::new(
            2,
            4,
            "Phase B".to_string(),
            PhaseStatus::Completed,
            10,
            "Bash".to_string(),
            Some("/tests/test.rs".to_string()),
        );

        state.update_phase_progress("t1", progress1);
        state.update_phase_progress("t2", progress2);

        let p1 = state.get_phase_progress("t1").unwrap();
        let p2 = state.get_phase_progress("t2").unwrap();

        assert_eq!(p1.phase_name, "Phase A");
        assert_eq!(p2.phase_name, "Phase B");
        assert_eq!(p1.phase_index, 1);
        assert_eq!(p2.phase_index, 2);
    }

    #[test]
    fn test_update_phase_progress_replaces_existing() {
        let mut state = DashboardState::new();

        let progress1 = PhaseProgressData::new(
            0,
            3,
            "Initial".to_string(),
            PhaseStatus::Starting,
            0,
            "".to_string(),
            None,
        );
        state.update_phase_progress("t1", progress1);

        let progress2 = PhaseProgressData::new(
            0,
            3,
            "Initial".to_string(),
            PhaseStatus::Running,
            5,
            "Edit".to_string(),
            Some("/src/main.rs".to_string()),
        );
        state.update_phase_progress("t1", progress2);

        let result = state.get_phase_progress("t1").unwrap();
        assert_eq!(result.status, PhaseStatus::Running);
        assert_eq!(result.tool_count, 5);
        assert_eq!(result.last_tool, "Edit");
    }

    // -------------------- Round 6: Progress and Mode Tests --------------------

    #[test]
    fn test_build_thread_views_includes_mode() {
        use crate::models::ThreadMode;

        let mut state = DashboardState::new();
        let mut thread = make_thread("t1", "Test Thread");
        thread.mode = ThreadMode::Plan; // Set a specific mode
        state.threads.insert("t1".to_string(), thread);

        let views = state.compute_thread_views();

        assert_eq!(views.len(), 1);
        let view = &views[0];
        assert_eq!(view.mode, ThreadMode::Plan);
    }

    #[test]
    fn test_build_thread_views_includes_progress_when_running() {
        let mut state = DashboardState::new();
        let mut thread = make_thread("t1", "Test Thread");
        thread.status = Some(ThreadStatus::Running);
        state.threads.insert("t1".to_string(), thread);

        // Add phase progress with Running status
        let progress = PhaseProgressData::new(
            2,
            5,
            "Phase 2".to_string(),
            PhaseStatus::Running,
            10,
            "Edit".to_string(),
            None,
        );
        state.update_phase_progress("t1", progress);

        let views = state.compute_thread_views();

        assert_eq!(views.len(), 1);
        let view = &views[0];
        assert!(view.progress.is_some());
        let prog = view.progress.as_ref().unwrap();
        assert_eq!(prog.current, 2);
        assert_eq!(prog.total, 5);
    }

    #[test]
    fn test_build_thread_views_includes_progress_when_starting() {
        let mut state = DashboardState::new();
        let mut thread = make_thread("t1", "Test Thread");
        thread.status = Some(ThreadStatus::Running);
        state.threads.insert("t1".to_string(), thread);

        // Add phase progress with Starting status
        let progress = PhaseProgressData::new(
            0,
            3,
            "Phase 0".to_string(),
            PhaseStatus::Starting,
            0,
            "".to_string(),
            None,
        );
        state.update_phase_progress("t1", progress);

        let views = state.compute_thread_views();

        assert_eq!(views.len(), 1);
        let view = &views[0];
        assert!(view.progress.is_some());
    }

    #[test]
    fn test_build_thread_views_no_progress_when_completed() {
        let mut state = DashboardState::new();
        let mut thread = make_thread("t1", "Test Thread");
        thread.status = Some(ThreadStatus::Done);
        state.threads.insert("t1".to_string(), thread);

        // Add phase progress with Completed status (should not create Progress)
        let progress = PhaseProgressData::new(
            4,
            5,
            "Phase 4".to_string(),
            PhaseStatus::Completed,
            20,
            "Bash".to_string(),
            None,
        );
        state.update_phase_progress("t1", progress);

        let views = state.compute_thread_views();

        assert_eq!(views.len(), 1);
        let view = &views[0];
        // Progress should be None because status is Completed, not Running/Starting
        assert!(view.progress.is_none());
    }

    #[test]
    fn test_build_thread_views_no_progress_when_failed() {
        let mut state = DashboardState::new();
        let mut thread = make_thread("t1", "Test Thread");
        thread.status = Some(ThreadStatus::Error);
        state.threads.insert("t1".to_string(), thread);

        // Add phase progress with Failed status (should not create Progress)
        let progress = PhaseProgressData::new(
            2,
            5,
            "Phase 2".to_string(),
            PhaseStatus::Failed,
            10,
            "Edit".to_string(),
            None,
        );
        state.update_phase_progress("t1", progress);

        let views = state.compute_thread_views();

        assert_eq!(views.len(), 1);
        let view = &views[0];
        // Progress should be None because status is Failed, not Running/Starting
        assert!(view.progress.is_none());
    }

    #[test]
    fn test_build_thread_views_no_progress_without_phase_data() {
        let mut state = DashboardState::new();
        let mut thread = make_thread("t1", "Test Thread");
        thread.status = Some(ThreadStatus::Running);
        state.threads.insert("t1".to_string(), thread);

        // No phase progress added

        let views = state.compute_thread_views();

        assert_eq!(views.len(), 1);
        let view = &views[0];
        assert!(view.progress.is_none());
    }

    #[test]
    fn test_build_thread_views_with_mode_and_progress() {
        use crate::models::ThreadMode;

        let mut state = DashboardState::new();
        let mut thread = make_thread("t1", "Test Thread");
        thread.mode = ThreadMode::Exec;
        thread.status = Some(ThreadStatus::Running);
        state.threads.insert("t1".to_string(), thread);

        // Add phase progress
        let progress = PhaseProgressData::new(
            1,
            4,
            "Exec Phase 1".to_string(),
            PhaseStatus::Running,
            5,
            "Bash".to_string(),
            None,
        );
        state.update_phase_progress("t1", progress);

        let views = state.compute_thread_views();

        assert_eq!(views.len(), 1);
        let view = &views[0];
        assert_eq!(view.mode, ThreadMode::Exec);
        assert!(view.progress.is_some());
        let prog = view.progress.as_ref().unwrap();
        assert_eq!(prog.current, 1);
        assert_eq!(prog.total, 4);
    }

    #[test]
    fn test_build_thread_views_multiple_threads_with_progress() {
        use crate::models::ThreadMode;

        let mut state = DashboardState::new();

        // Thread 1: Plan mode with progress
        let mut t1 = make_thread("t1", "Thread 1");
        t1.mode = ThreadMode::Plan;
        t1.status = Some(ThreadStatus::Running);
        state.threads.insert("t1".to_string(), t1);

        let p1 = PhaseProgressData::new(
            0,
            2,
            "Phase 1".to_string(),
            PhaseStatus::Starting,
            0,
            "".to_string(),
            None,
        );
        state.update_phase_progress("t1", p1);

        // Thread 2: Exec mode with progress
        let mut t2 = make_thread("t2", "Thread 2");
        t2.mode = ThreadMode::Exec;
        t2.status = Some(ThreadStatus::Running);
        state.threads.insert("t2".to_string(), t2);

        let p2 = PhaseProgressData::new(
            2,
            5,
            "Phase 2".to_string(),
            PhaseStatus::Running,
            10,
            "Edit".to_string(),
            None,
        );
        state.update_phase_progress("t2", p2);

        // Thread 3: Normal mode, no progress
        let mut t3 = make_thread("t3", "Thread 3");
        t3.mode = ThreadMode::default();
        t3.status = Some(ThreadStatus::Idle);
        state.threads.insert("t3".to_string(), t3);

        let views = state.compute_thread_views();

        assert_eq!(views.len(), 3);

        // Find each thread and verify properties
        let v1 = views.iter().find(|v| v.id == "t1").unwrap();
        assert_eq!(v1.mode, ThreadMode::Plan);
        assert!(v1.progress.is_some());

        let v2 = views.iter().find(|v| v.id == "t2").unwrap();
        assert_eq!(v2.mode, ThreadMode::Exec);
        assert!(v2.progress.is_some());

        let v3 = views.iter().find(|v| v.id == "t3").unwrap();
        assert!(v3.progress.is_none());
    }

    // -------------------- build_render_context Cache Tests --------------------

    #[test]
    fn test_build_render_context_refreshes_stale_views() {
        use crate::view_state::{SystemStats, Theme};

        let mut state = DashboardState::new();

        // Add initial thread
        let t1 = make_thread("t1", "Thread 1");
        state.threads.insert("t1".to_string(), t1);

        // Compute views to cache them
        let views = state.compute_thread_views();
        assert_eq!(views.len(), 1);
        assert!(!state.thread_views_dirty);

        // Add another thread (makes views dirty)
        let t2 = make_thread("t2", "Thread 2");
        state.threads.insert("t2".to_string(), t2);
        state.recompute_aggregate();
        state.thread_views_dirty = true;

        // Verify views are dirty
        assert!(state.thread_views_dirty);

        // build_render_context should refresh the views
        let stats = SystemStats::default();
        let theme = Theme::default();
        let ctx = state.build_render_context(&stats, &theme);

        // Context should have 2 threads (the updated view)
        assert_eq!(ctx.threads.len(), 2);
        // And the dirty flag should be cleared
        assert!(!state.thread_views_dirty);
    }

    #[test]
    fn test_build_render_context_uses_cache_when_not_dirty() {
        use crate::view_state::{SystemStats, Theme};

        let mut state = DashboardState::new();

        // Add thread
        let t1 = make_thread("t1", "Thread 1");
        state.threads.insert("t1".to_string(), t1);

        // Compute views to cache them
        let _ = state.compute_thread_views();
        assert!(!state.thread_views_dirty);

        // build_render_context should use cached views
        let stats = SystemStats::default();
        let theme = Theme::default();
        let ctx = state.build_render_context(&stats, &theme);

        // Still 1 thread
        assert_eq!(ctx.threads.len(), 1);
        // Still not dirty
        assert!(!state.thread_views_dirty);
    }

    #[test]
    fn test_build_render_context_reflects_status_updates() {
        use crate::view_state::{SystemStats, Theme};

        let mut state = DashboardState::new();

        // Add thread with idle status
        let mut t1 = make_thread("t1", "Thread 1");
        t1.status = Some(ThreadStatus::Idle);
        state.threads.insert("t1".to_string(), t1);

        // Build initial context
        let stats = SystemStats::default();
        let theme = Theme::default();
        let ctx = state.build_render_context(&stats, &theme);
        assert_eq!(ctx.threads.len(), 1);

        // Update thread status (marks dirty)
        state.update_thread_status("t1", ThreadStatus::Waiting, Some(WaitingFor::UserInput));

        // Verify dirty
        assert!(state.thread_views_dirty);

        // build_render_context should reflect the update
        let ctx = state.build_render_context(&stats, &theme);
        let view = &ctx.threads[0];
        assert!(view.needs_action); // Waiting threads need action
    }
}
