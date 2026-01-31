//! Dashboard state management
//!
//! This module provides the state container for the multi-thread dashboard view,
//! managing thread data, computed views, and overlay states.

use crate::models::dashboard::{Aggregate, PlanRequest, ThreadStatus, WaitingFor};
use crate::models::{Thread, ThreadMode};
use crate::state::session::{AskUserQuestionData, PermissionRequest};
use crate::view_state::{
    OverlayState, Progress, RenderContext, SystemStats, Theme, ThreadView,
};
use crate::websocket::messages::PhaseStatus;
use std::collections::{HashMap, HashSet};
use tracing::info;

// ============================================================================
// DashboardQuestionState
// ============================================================================

/// Navigation state for question overlays in the dashboard
///
/// This struct manages the UI navigation state when a question overlay is displayed
/// from the dashboard. It tracks the current option selection, tab index for
/// multi-question prompts, multi-select toggles, and "Other" text input state.
///
/// Note: This is dashboard-specific state that complements the `AskUserQuestionData`
/// stored in `pending_questions`. The data (questions, options) comes from
/// `AskUserQuestionData`, while this struct tracks navigation/selection state.
#[derive(Debug, Clone, Default)]
pub struct DashboardQuestionState {
    /// Current question tab index (0-based) for multi-question prompts
    pub tab_index: usize,
    /// Currently highlighted option index per question (None = "Other" highlighted)
    pub selections: Vec<Option<usize>>,
    /// For multi-select questions: which options are toggled per question
    /// Each inner Vec corresponds to a question's options
    pub multi_selections: Vec<Vec<bool>>,
    /// "Other" text content per question
    pub other_texts: Vec<String>,
    /// Whether "Other" text input is active (cursor in text field)
    pub other_active: bool,
    /// Tracks which questions have been answered (for multi-question flow)
    pub answered: Vec<bool>,
}

impl DashboardQuestionState {
    /// Create a new DashboardQuestionState for the given question data
    ///
    /// Initializes navigation state based on the number of questions and options.
    pub fn from_question_data(data: &AskUserQuestionData) -> Self {
        let num_questions = data.questions.len();
        let options_per_question: Vec<usize> =
            data.questions.iter().map(|q| q.options.len()).collect();

        Self {
            tab_index: 0,
            selections: vec![Some(0); num_questions],
            multi_selections: options_per_question
                .iter()
                .map(|&count| vec![false; count])
                .collect(),
            other_texts: vec![String::new(); num_questions],
            other_active: false,
            answered: vec![false; num_questions],
        }
    }

    /// Reset all navigation state
    pub fn reset(&mut self) {
        self.tab_index = 0;
        self.selections.clear();
        self.multi_selections.clear();
        self.other_texts.clear();
        self.other_active = false;
        self.answered.clear();
    }

    /// Get the currently selected option index for the current tab
    ///
    /// Returns None if "Other" is selected or if tab_index is out of bounds.
    pub fn current_selection(&self) -> Option<usize> {
        self.selections.get(self.tab_index).copied().flatten()
    }

    /// Set the selection for the current tab
    pub fn set_current_selection(&mut self, selection: Option<usize>) {
        if self.tab_index < self.selections.len() {
            self.selections[self.tab_index] = selection;
        }
    }

    /// Move to the previous option in the current question
    ///
    /// Wraps from first option to "Other" and from "Other" to last option.
    pub fn prev_option(&mut self, option_count: usize) {
        if let Some(current) = self.current_selection() {
            if current > 0 {
                self.set_current_selection(Some(current - 1));
            } else {
                // Wrap to "Other" (None)
                self.set_current_selection(None);
            }
        } else {
            // Currently on "Other", move to last option
            if option_count > 0 {
                self.set_current_selection(Some(option_count - 1));
            }
        }
    }

    /// Move to the next option in the current question
    ///
    /// Wraps from last option to "Other" and from "Other" to first option.
    pub fn next_option(&mut self, option_count: usize) {
        if let Some(current) = self.current_selection() {
            if current < option_count.saturating_sub(1) {
                self.set_current_selection(Some(current + 1));
            } else {
                // Wrap to "Other" (None)
                self.set_current_selection(None);
            }
        } else {
            // Currently on "Other", wrap to first option
            self.set_current_selection(Some(0));
        }
    }

    /// Move to the next tab (wraps around)
    pub fn next_tab(&mut self, num_questions: usize) {
        if num_questions > 1 {
            self.tab_index = (self.tab_index + 1) % num_questions;
        }
    }

    /// Toggle a multi-select option for the current tab
    pub fn toggle_multi_selection(&mut self, option_index: usize) {
        if let Some(options) = self.multi_selections.get_mut(self.tab_index) {
            if option_index < options.len() {
                options[option_index] = !options[option_index];
            }
        }
    }

    /// Check if a multi-select option is selected for the current tab
    pub fn is_multi_selected(&self, option_index: usize) -> bool {
        self.multi_selections
            .get(self.tab_index)
            .and_then(|options| options.get(option_index))
            .copied()
            .unwrap_or(false)
    }

    /// Get the "Other" text for the current tab
    pub fn current_other_text(&self) -> &str {
        self.other_texts
            .get(self.tab_index)
            .map(|s| s.as_str())
            .unwrap_or("")
    }

    /// Append a character to the current tab's "Other" text
    pub fn push_other_char(&mut self, c: char) {
        if let Some(text) = self.other_texts.get_mut(self.tab_index) {
            text.push(c);
        }
    }

    /// Remove the last character from the current tab's "Other" text
    pub fn pop_other_char(&mut self) {
        if let Some(text) = self.other_texts.get_mut(self.tab_index) {
            text.pop();
        }
    }

    /// Mark the current question as answered
    pub fn mark_current_answered(&mut self) {
        if self.tab_index < self.answered.len() {
            self.answered[self.tab_index] = true;
        }
    }

    /// Check if all questions have been answered
    pub fn all_answered(&self) -> bool {
        !self.answered.is_empty() && self.answered.iter().all(|&a| a)
    }

    /// Advance to the next unanswered question
    ///
    /// Returns true if moved to a new tab, false if no unanswered questions.
    pub fn advance_to_next_unanswered(&mut self, num_questions: usize) -> bool {
        if num_questions == 0 {
            return false;
        }

        for offset in 1..=num_questions {
            let idx = (self.tab_index + offset) % num_questions;
            if !self.answered.get(idx).copied().unwrap_or(true) {
                self.tab_index = idx;
                return true;
            }
        }
        false
    }
}

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
    /// Plan requests pending approval: thread_id -> PlanRequest
    plan_requests: HashMap<String, PlanRequest>,
    /// Thread IDs verified locally (backend fallback)
    locally_verified: HashSet<String>,
    /// Phase progress data by thread_id during plan execution
    phase_progress: HashMap<String, PhaseProgressData>,
    /// Pending question data by thread_id (for AskUserQuestion tool)
    /// Stores (request_id, question_data) tuple for WebSocket response
    pending_questions: HashMap<String, (String, AskUserQuestionData)>,
    /// Pending permission requests by thread_id (for permission prompts)
    /// Each thread can have at most one pending permission at a time
    pending_permissions: HashMap<String, PermissionRequest>,

    /// Threads currently in plan mode (actively planning)
    /// Set when ThreadModeUpdate { mode: Plan } received, cleared on exit
    planning_threads: HashSet<String>,

    /// Current overlay state (if an overlay is open)
    overlay: Option<OverlayState>,
    /// Navigation state for question overlay (when Question overlay is open)
    question_state: Option<DashboardQuestionState>,
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
            pending_questions: HashMap::new(),
            pending_permissions: HashMap::new(),
            planning_threads: HashSet::new(),
            overlay: None,
            question_state: None,
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
    ///
    /// This method also handles cleanup of pending questions and permissions when:
    /// - Thread status changes to Done, Error, or Idle (thread completed/dismissed)
    /// - Thread is no longer waiting for UserInput (questions) or Permission (permissions)
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

        // Check if we should clear pending question data:
        // 1. Thread completed (Done, Error) - question is no longer relevant
        // 2. Thread no longer waiting for UserInput - question was answered or cancelled
        let should_clear_question = match status {
            ThreadStatus::Done | ThreadStatus::Error => true,
            _ => {
                // Check if waiting_for changed from UserInput to something else
                let was_waiting_for_user_input = self
                    .waiting_for
                    .get(thread_id)
                    .map(|wf| matches!(wf, WaitingFor::UserInput))
                    .unwrap_or(false);
                let is_now_waiting_for_user_input = waiting_for
                    .as_ref()
                    .map(|wf| matches!(wf, WaitingFor::UserInput))
                    .unwrap_or(false);
                was_waiting_for_user_input && !is_now_waiting_for_user_input
            }
        };

        if should_clear_question && self.pending_questions.contains_key(thread_id) {
            tracing::debug!(
                "Clearing pending question for thread {} due to status change to {:?}",
                thread_id,
                status
            );
            self.pending_questions.remove(thread_id);
        }

        // Check if we should clear pending permission data:
        // 1. Thread completed (Done, Error) - permission is no longer relevant
        // 2. Thread no longer waiting for Permission - permission was answered or cancelled
        let should_clear_permission = match status {
            ThreadStatus::Done | ThreadStatus::Error => true,
            _ => {
                // Check if waiting_for changed from Permission to something else
                let was_waiting_for_permission = self
                    .waiting_for
                    .get(thread_id)
                    .map(|wf| matches!(wf, WaitingFor::Permission { .. }))
                    .unwrap_or(false);
                let is_now_waiting_for_permission = waiting_for
                    .as_ref()
                    .map(|wf| matches!(wf, WaitingFor::Permission { .. }))
                    .unwrap_or(false);
                was_waiting_for_permission && !is_now_waiting_for_permission
            }
        };

        if should_clear_permission && self.pending_permissions.contains_key(thread_id) {
            tracing::debug!(
                "Clearing pending permission for thread {} due to status change to {:?}",
                thread_id,
                status
            );
            self.pending_permissions.remove(thread_id);
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
    pub fn set_plan_request(&mut self, thread_id: &str, request: PlanRequest) {
        self.plan_requests.insert(thread_id.to_string(), request);
    }

    /// Check if a plan request originated from a permission request
    ///
    /// Returns true if the plan should send a permission_response instead of plan_approval_response
    pub fn is_plan_from_permission(&self, thread_id: &str) -> bool {
        self.plan_requests
            .get(thread_id)
            .map(|req| req.from_permission)
            .unwrap_or(false)
    }

    /// Check if a thread is currently in planning mode
    ///
    /// Returns true when the thread has received ThreadModeUpdate { mode: Plan }
    /// and hasn't yet exited plan mode (via PlanApprovalRequest).
    pub fn is_thread_planning(&self, thread_id: &str) -> bool {
        self.planning_threads.contains(thread_id)
    }

    /// Set or clear the planning state for a thread
    ///
    /// Called when ThreadModeUpdate events are received:
    /// - `planning=true` when mode is Plan
    /// - `planning=false` when mode is Normal or Exec
    pub fn set_thread_planning(&mut self, thread_id: &str, planning: bool) {
        if planning {
            self.planning_threads.insert(thread_id.to_string());
        } else {
            self.planning_threads.remove(thread_id);
        }
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

    /// Update only the current_operation for a thread without changing the state
    ///
    /// This is used when ToolExecuting events provide display_name - we want to
    /// show the tool's display name as the activity without overwriting the agent state.
    ///
    /// # Arguments
    /// * `thread_id` - The thread to update
    /// * `current_operation` - Description of what the agent is doing (e.g., "Read: main.rs")
    pub fn update_current_operation(&mut self, thread_id: &str, current_operation: Option<&str>) {
        if let Some((state, _)) = self.agent_states.get(thread_id) {
            // Preserve existing state, update operation
            let state = state.clone();
            self.agent_states.insert(
                thread_id.to_string(),
                (state, current_operation.map(|s| s.to_string())),
            );
        } else {
            // No existing state - default to "running" since we only get tool events when running
            self.agent_states.insert(
                thread_id.to_string(),
                ("running".to_string(), current_operation.map(|s| s.to_string())),
            );
        }
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

    /// Store pending question data for a thread
    ///
    /// Called when receiving an AskUserQuestion permission request from WebSocket.
    /// This stores the question data so it can be displayed in the UI when the
    /// user interacts with the thread.
    ///
    /// # Arguments
    /// * `thread_id` - The thread this question belongs to
    /// * `request_id` - The WebSocket request ID for sending the response
    /// * `question_data` - The question data structure
    pub fn set_pending_question(
        &mut self,
        thread_id: &str,
        request_id: String,
        question_data: AskUserQuestionData,
    ) {
        self.pending_questions
            .insert(thread_id.to_string(), (request_id, question_data));
        self.thread_views_dirty = true;
    }

    /// Get pending question data for a thread
    pub fn get_pending_question(&self, thread_id: &str) -> Option<&AskUserQuestionData> {
        self.pending_questions
            .get(thread_id)
            .map(|(_, data)| data)
    }

    /// Get pending question request ID for a thread
    pub fn get_pending_question_request_id(&self, thread_id: &str) -> Option<&str> {
        self.pending_questions
            .get(thread_id)
            .map(|(request_id, _)| request_id.as_str())
    }

    /// Clear pending question data for a thread
    ///
    /// Called after the user has answered the question or the request is cancelled.
    pub fn clear_pending_question(&mut self, thread_id: &str) {
        self.pending_questions.remove(thread_id);
        self.thread_views_dirty = true;
    }

    /// Store a pending permission request for a thread
    ///
    /// Called when receiving a permission request from WebSocket.
    /// Each thread can have at most one pending permission at a time.
    /// If a new permission arrives for a thread that already has one,
    /// it replaces the old one.
    ///
    /// # Arguments
    /// * `thread_id` - The thread this permission belongs to
    /// * `request` - The permission request data
    pub fn set_pending_permission(&mut self, thread_id: &str, request: PermissionRequest) {
        self.pending_permissions
            .insert(thread_id.to_string(), request);
        self.thread_views_dirty = true;
    }

    /// Get pending permission request for a thread
    pub fn get_pending_permission(&self, thread_id: &str) -> Option<&PermissionRequest> {
        self.pending_permissions.get(thread_id)
    }

    /// Clear pending permission for a thread
    ///
    /// Called after the user has responded to the permission or the request is cancelled.
    pub fn clear_pending_permission(&mut self, thread_id: &str) {
        self.pending_permissions.remove(thread_id);
        self.thread_views_dirty = true;
    }

    /// Find a pending permission by its permission_id across all threads
    ///
    /// Returns the thread_id and permission reference if found.
    /// This is useful when you have a permission_id but don't know which thread it belongs to.
    pub fn find_permission_by_id(&self, permission_id: &str) -> Option<(&str, &PermissionRequest)> {
        for (thread_id, perm) in &self.pending_permissions {
            if perm.permission_id == permission_id {
                return Some((thread_id.as_str(), perm));
            }
        }
        None
    }

    /// Clear a pending permission by its permission_id (searches across all threads)
    ///
    /// Returns the thread_id if found and cleared.
    pub fn clear_permission_by_id(&mut self, permission_id: &str) -> Option<String> {
        let thread_id = self
            .pending_permissions
            .iter()
            .find(|(_, perm)| perm.permission_id == permission_id)
            .map(|(tid, _)| tid.clone());

        if let Some(ref tid) = thread_id {
            self.pending_permissions.remove(tid);
            self.thread_views_dirty = true;
        }

        thread_id
    }

    /// Iterate over all pending permissions
    ///
    /// Returns an iterator of (thread_id, permission) pairs.
    pub fn pending_permissions_iter(
        &self,
    ) -> impl Iterator<Item = (&String, &PermissionRequest)> {
        self.pending_permissions.iter()
    }

    /// Check if there are any pending permissions
    pub fn has_pending_permission(&self) -> bool {
        !self.pending_permissions.is_empty()
    }

    // ========================================================================
    // UI State (from click handlers)
    // ========================================================================

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
                if let Some(plan_request) = self.plan_requests.get(thread_id) {
                    // Clear question state when opening Plan overlay
                    self.question_state = None;
                    Some(OverlayState::Plan {
                        thread_id: thread_id.to_string(),
                        thread_title: thread.title.clone(),
                        repository: thread.display_repository(),
                        request_id: plan_request.request_id.clone(),
                        summary: plan_request.summary.clone(),
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
                // Question overlay - lookup pending question data (extract just the data, not request_id)
                let question_data = self
                    .pending_questions
                    .get(thread_id)
                    .map(|(_, data)| data.clone());
                // Initialize question navigation state if we have question data
                self.question_state = question_data
                    .as_ref()
                    .map(DashboardQuestionState::from_question_data);
                Some(OverlayState::Question {
                    thread_id: thread_id.to_string(),
                    thread_title: thread.title.clone(),
                    repository: thread.display_repository(),
                    question_data,
                    anchor_y,
                })
            }
        };
    }

    /// Close the current overlay
    pub fn collapse_overlay(&mut self) {
        self.overlay = None;
        self.question_state = None;
    }

    /// Get current overlay state
    pub fn overlay(&self) -> Option<&OverlayState> {
        self.overlay.as_ref()
    }

    /// Get mutable reference to current overlay state
    pub fn overlay_mut(&mut self) -> Option<&mut OverlayState> {
        self.overlay.as_mut()
    }

    /// Show the Claude accounts management overlay
    pub fn show_claude_accounts(&mut self) {
        self.overlay = Some(OverlayState::ClaudeAccounts {
            accounts: Vec::new(),
            selected_index: 0,
            anchor_y: 10,
            adding: false,
            add_request_id: None,
            status_message: None,
        });
    }

    /// Switch from Question overlay to FreeForm input
    pub fn show_free_form(&mut self, thread_id: &str) {
        if let Some(OverlayState::Question {
            thread_id: tid,
            thread_title,
            repository,
            question_data,
            anchor_y,
        }) = self.overlay.take()
        {
            if tid == thread_id {
                self.overlay = Some(OverlayState::FreeForm {
                    thread_id: tid,
                    thread_title,
                    repository,
                    question_data,
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
            question_data,
            anchor_y,
            ..
        }) = self.overlay.take()
        {
            if tid == thread_id {
                self.overlay = Some(OverlayState::Question {
                    thread_id: tid,
                    thread_title,
                    repository,
                    question_data,
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
    // Claude Login Overlay
    // ========================================================================

    /// Show the Claude login overlay
    pub fn show_claude_login(&mut self, request_id: String, auth_url: String, auto_opened: bool) {
        use crate::view_state::ClaudeLoginState;

        self.overlay = Some(OverlayState::ClaudeLogin {
            request_id,
            auth_url,
            state: ClaudeLoginState::ShowingUrl {
                browser_opened: auto_opened,
            },
            anchor_y: 5, // Center-ish in the list area
        });
    }

    /// Update the Claude login overlay state
    pub fn update_claude_login_state(&mut self, new_state: crate::view_state::ClaudeLoginState) {
        if let Some(OverlayState::ClaudeLogin { ref mut state, .. }) = self.overlay {
            *state = new_state;
        }
    }

    /// Get the current Claude login request ID if a login overlay is open
    pub fn claude_login_request_id(&self) -> Option<&str> {
        if let Some(OverlayState::ClaudeLogin { request_id, .. }) = &self.overlay {
            Some(request_id)
        } else {
            None
        }
    }

    /// Get the current Claude login auth URL if a login overlay is open
    pub fn claude_login_auth_url(&self) -> Option<&str> {
        if let Some(OverlayState::ClaudeLogin { auth_url, .. }) = &self.overlay {
            Some(auth_url)
        } else {
            None
        }
    }

    // ========================================================================
    // VPS Config Overlay
    // ========================================================================

    /// Show the VPS config overlay
    pub fn show_vps_config(&mut self) {
        use crate::view_state::VpsConfigState;

        self.overlay = Some(OverlayState::VpsConfig {
            state: VpsConfigState::InputFields {
                ip: String::new(),
                username: "root".to_string(),
                password: String::new(),
                field_focus: 0,
                error: None,
            },
            anchor_y: 5,
        });
    }

    /// Update the VPS config overlay state
    pub fn update_vps_config_state(&mut self, new_state: crate::view_state::VpsConfigState) {
        if let Some(OverlayState::VpsConfig { ref mut state, .. }) = self.overlay {
            *state = new_state;
        }
    }

    /// Move to the next field in VPS config
    pub fn vps_config_next_field(&mut self) {
        use crate::view_state::VpsConfigState;

        if let Some(OverlayState::VpsConfig { ref mut state, .. }) = self.overlay {
            if let VpsConfigState::InputFields { ref mut field_focus, .. } = state {
                *field_focus = (*field_focus + 1) % 3;
            }
        }
    }

    /// Move to the previous field in VPS config
    pub fn vps_config_prev_field(&mut self) {
        use crate::view_state::VpsConfigState;

        if let Some(OverlayState::VpsConfig { ref mut state, .. }) = self.overlay {
            if let VpsConfigState::InputFields { ref mut field_focus, .. } = state {
                *field_focus = if *field_focus == 0 { 2 } else { *field_focus - 1 };
            }
        }
    }

    /// Type a character in the current VPS config field
    pub fn vps_config_type_char(&mut self, c: char) {
        use crate::view_state::VpsConfigState;

        if let Some(OverlayState::VpsConfig { ref mut state, .. }) = self.overlay {
            if let VpsConfigState::InputFields { ref mut ip, ref mut username, ref mut password, field_focus, ref mut error } = state {
                // Clear any validation error when user types
                *error = None;
                match field_focus {
                    0 => ip.push(c),
                    1 => username.push(c),
                    2 => password.push(c),
                    _ => {}
                }
            }
        }
    }

    /// Backspace in the current VPS config field
    pub fn vps_config_backspace(&mut self) {
        use crate::view_state::VpsConfigState;

        if let Some(OverlayState::VpsConfig { ref mut state, .. }) = self.overlay {
            if let VpsConfigState::InputFields { ref mut ip, ref mut username, ref mut password, field_focus, ref mut error } = state {
                // Clear any validation error when user types
                *error = None;
                match field_focus {
                    0 => { ip.pop(); }
                    1 => { username.pop(); }
                    2 => { password.pop(); }
                    _ => {}
                }
            }
        }
    }

    /// Set a validation error on the VPS config overlay
    pub fn vps_config_set_error(&mut self, error_msg: String) {
        use crate::view_state::VpsConfigState;

        if let Some(OverlayState::VpsConfig { ref mut state, .. }) = self.overlay {
            if let VpsConfigState::InputFields { ref mut error, .. } = state {
                *error = Some(error_msg);
            }
        }
    }

    /// Retry from error state - reset to InputFields keeping values
    pub fn vps_config_retry(&mut self) {
        use crate::view_state::VpsConfigState;

        if let Some(OverlayState::VpsConfig { ref mut state, .. }) = self.overlay {
            // Preserve the InputFields state if we're in Error
            if matches!(state, VpsConfigState::Error { .. }) {
                *state = VpsConfigState::InputFields {
                    ip: String::new(),
                    username: "root".to_string(),
                    password: String::new(),
                    field_focus: 0,
                    error: None,
                };
            }
        }
    }

    // ========================================================================
    // Question Navigation (for Question overlay)
    // ========================================================================

    /// Get the question state (immutable)
    pub fn question_state(&self) -> Option<&DashboardQuestionState> {
        self.question_state.as_ref()
    }

    /// Get the question state (mutable)
    pub fn question_state_mut(&mut self) -> Option<&mut DashboardQuestionState> {
        self.question_state.as_mut()
    }

    /// Get the number of options in the current question
    ///
    /// Returns 0 if no question overlay is open or no question data available.
    fn get_current_option_count(&self) -> usize {
        if let Some(OverlayState::Question { question_data, .. }) = &self.overlay {
            if let Some(data) = question_data {
                if let Some(state) = &self.question_state {
                    if let Some(question) = data.questions.get(state.tab_index) {
                        return question.options.len();
                    }
                }
            }
        }
        0
    }

    /// Get the number of questions in the overlay
    fn get_question_count(&self) -> usize {
        if let Some(OverlayState::Question { question_data, .. }) = &self.overlay {
            if let Some(data) = question_data {
                return data.questions.len();
            }
        }
        0
    }

    /// Check if the current question is multi-select
    fn is_current_question_multi_select(&self) -> bool {
        if let Some(OverlayState::Question { question_data, .. }) = &self.overlay {
            if let Some(data) = question_data {
                if let Some(state) = &self.question_state {
                    if let Some(question) = data.questions.get(state.tab_index) {
                        return question.multi_select;
                    }
                }
            }
        }
        false
    }

    /// Move to the previous option in the question overlay
    pub fn question_prev_option(&mut self) {
        let option_count = self.get_current_option_count();
        if let Some(state) = &mut self.question_state {
            state.prev_option(option_count);
        }
    }

    /// Move to the next option in the question overlay
    pub fn question_next_option(&mut self) {
        let option_count = self.get_current_option_count();
        if let Some(state) = &mut self.question_state {
            state.next_option(option_count);
        }
    }

    /// Move to the next question tab
    pub fn question_next_tab(&mut self) {
        let num_questions = self.get_question_count();
        if let Some(state) = &mut self.question_state {
            state.next_tab(num_questions);
        }
    }

    /// Toggle the current option in multi-select mode
    pub fn question_toggle_option(&mut self) {
        if self.is_current_question_multi_select() {
            if let Some(state) = &mut self.question_state {
                if let Some(idx) = state.current_selection() {
                    state.toggle_multi_selection(idx);
                }
            }
        }
    }

    /// Check if "Other" text input is active in question overlay
    pub fn is_question_other_active(&self) -> bool {
        self.question_state
            .as_ref()
            .map(|s| s.other_active)
            .unwrap_or(false)
    }

    /// Activate "Other" text input mode
    pub fn question_activate_other(&mut self) {
        if let Some(state) = &mut self.question_state {
            state.other_active = true;
        }
    }

    /// Deactivate "Other" text input mode
    pub fn question_deactivate_other(&mut self) {
        if let Some(state) = &mut self.question_state {
            state.other_active = false;
        }
    }

    /// Cancel "Other" text input mode and clear text
    pub fn question_cancel_other(&mut self) {
        if let Some(state) = &mut self.question_state {
            if state.other_active {
                state.other_active = false;
                if let Some(text) = state.other_texts.get_mut(state.tab_index) {
                    text.clear();
                }
            }
        }
    }

    /// Type a character in "Other" text input
    pub fn question_type_char(&mut self, c: char) {
        if let Some(state) = &mut self.question_state {
            if state.other_active {
                state.push_other_char(c);
            }
        }
    }

    /// Backspace in "Other" text input
    pub fn question_backspace(&mut self) {
        if let Some(state) = &mut self.question_state {
            if state.other_active {
                state.pop_other_char();
            }
        }
    }

    /// Handle Enter key in question overlay
    ///
    /// For single questions: returns Some((thread_id, request_id, answers)) to submit
    /// For multiple questions: marks current as answered and advances to next
    ///                         Returns Some only when all questions are answered
    ///
    /// Returns None if not ready to submit.
    pub fn question_confirm(
        &mut self,
    ) -> Option<(String, String, std::collections::HashMap<String, String>)> {
        let num_questions = self.get_question_count();

        // Check if "Other" is selected and not in text input mode
        let should_activate_other = self.question_state
            .as_ref()
            .map(|s| s.current_selection().is_none() && !s.other_active)
            .unwrap_or(false);

        if should_activate_other {
            self.question_activate_other();
            return None;
        }

        // If in "Other" text input mode, validate text
        if let Some(state) = &self.question_state {
            if state.other_active {
                let other_text = state.current_other_text();
                if other_text.is_empty() {
                    return None;
                }
            }
        }

        // Deactivate "Other" mode as we're confirming
        self.question_deactivate_other();

        // For single question, submit immediately
        if num_questions == 1 {
            return self.build_question_answers();
        }

        // Multiple questions: mark current as answered and advance
        if let Some(state) = &mut self.question_state {
            state.mark_current_answered();

            // Check if all questions are now answered
            if state.all_answered() {
                return self.build_question_answers();
            }

            // Advance to next unanswered question
            state.advance_to_next_unanswered(num_questions);
        }

        None
    }

    /// Build the answers map from current question state
    ///
    /// Returns (thread_id, request_id, answers) tuple for WebSocket response.
    fn build_question_answers(
        &self,
    ) -> Option<(String, String, std::collections::HashMap<String, String>)> {
        let (thread_id, question_data) = match &self.overlay {
            Some(OverlayState::Question {
                thread_id,
                question_data,
                ..
            }) => (thread_id.clone(), question_data.as_ref()?),
            _ => return None,
        };

        // Get the request_id from pending_questions
        let request_id = self.get_pending_question_request_id(&thread_id)?.to_string();

        let state = self.question_state.as_ref()?;
        let mut answers = std::collections::HashMap::new();

        for (i, question) in question_data.questions.iter().enumerate() {
            let answer = if question.multi_select {
                // Collect all selected options for multi-select
                let selected: Vec<String> = question
                    .options
                    .iter()
                    .enumerate()
                    .filter(|(opt_idx, _)| {
                        state
                            .multi_selections
                            .get(i)
                            .map(|s| s.get(*opt_idx).copied().unwrap_or(false))
                            .unwrap_or(false)
                    })
                    .map(|(_, opt)| opt.label.clone())
                    .collect();

                // Also check if "Other" has text for this question
                let other_text = state.other_texts.get(i).cloned().unwrap_or_default();
                if !other_text.is_empty() {
                    let mut with_other = selected;
                    with_other.push(other_text);
                    with_other.join(", ")
                } else if selected.is_empty() {
                    continue;
                } else {
                    selected.join(", ")
                }
            } else {
                // Single select
                if let Some(selection) = state.selections.get(i).copied().flatten() {
                    if let Some(opt) = question.options.get(selection) {
                        opt.label.clone()
                    } else {
                        continue;
                    }
                } else {
                    // "Other" selected - use the text
                    let other_text = state.other_texts.get(i).cloned().unwrap_or_default();
                    if other_text.is_empty() {
                        continue;
                    }
                    other_text
                }
            };

            answers.insert(question.question.clone(), answer);
        }

        Some((thread_id, request_id, answers))
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
        repos: &'a [crate::models::GitHubRepo],
    ) -> RenderContext<'a> {
        // Ensure thread views are fresh before building context
        self.compute_thread_views();

        RenderContext::new(&self.thread_views, &self.aggregate, system_stats, theme, repos)
            .with_overlay(self.overlay.as_ref())
            .with_question_state(self.question_state.as_ref())
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
        self.plan_requests
            .get(thread_id)
            .map(|req| req.request_id.as_str())
    }

    /// Get the full plan request for a thread
    ///
    /// Returns None if no plan approval is pending for this thread.
    pub fn get_plan_request(&self, thread_id: &str) -> Option<&PlanRequest> {
        self.plan_requests.get(thread_id)
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

    /// Find the first thread waiting for user input
    ///
    /// Returns the thread_id of the first thread with `WaitingFor::UserInput` status,
    /// following the same pattern as permission handling - topmost "needs action" thread.
    /// Threads are sorted by needs_action first, then by updated_at (most recent first).
    pub fn find_first_user_input_thread(&self) -> Option<String> {
        // Iterate through thread views (already sorted: needs_action first, then by updated_at)
        for view in &self.thread_views {
            // Check if this thread is waiting for user input
            if let Some(wf) = self.waiting_for.get(&view.id) {
                if matches!(wf, WaitingFor::UserInput) {
                    return Some(view.id.clone());
                }
            }
        }
        None
    }

    /// Get the top thread that needs action
    ///
    /// Returns the first thread that needs action and its waiting type.
    /// Threads are sorted by needs_action first, then by updated_at (most recent first).
    pub fn get_top_needs_action_thread(&self) -> Option<(String, WaitingFor)> {
        for view in &self.thread_views {
            if view.needs_action {
                if let Some(wf) = self.waiting_for.get(&view.id) {
                    info!("top_needs_action: id={} wf={:?}", view.id, wf);
                    return Some((view.id.clone(), wf.clone()));
                }
            }
        }
        None
    }

    /// Get the pending permission for the top needs-action thread only
    pub fn get_top_pending_permission(&self) -> Option<&PermissionRequest> {
        if let Some((thread_id, _)) = self.get_top_needs_action_thread() {
            return self.get_pending_permission(&thread_id);
        }
        None
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
                            .and_then(|phase_data| {
                                // Always populate progress for Exec mode, or when phase is actively running
                                if thread.mode == ThreadMode::Exec
                                    || matches!(phase_data.status, PhaseStatus::Running | PhaseStatus::Starting)
                                {
                                    Some(Progress::new(phase_data.phase_index, phase_data.total_phases))
                                } else {
                                    None
                                }
                            });

                    // Get current_operation from agent state
                    let current_operation = thread
                        .current_operation(&self.agent_states)
                        .map(|s| s.to_string());

                    // Compute activity_text based on thread state
                    // - Running + tool active: tool's display_name (e.g., "Read: main.rs")
                    // - Running + no tool: "Thinking..."
                    // - Idle: "idle"
                    // - Done: "done"
                    // - Error: "error"
                    // - Waiting: None (uses old layout with status column + actions)
                    let activity_text = match status {
                        ThreadStatus::Running => {
                            if let Some(ref op) = current_operation {
                                Some(op.clone())
                            } else {
                                Some("Thinking...".to_string())
                            }
                        }
                        ThreadStatus::Done => Some("ready".to_string()),
                        ThreadStatus::Error => Some("error".to_string()),
                        ThreadStatus::Waiting => None, // Uses old layout with status + actions
                    };

                    // Check if this thread has a pending permission (needs action)
                    let has_pending_permission = self.pending_permissions.contains_key(&thread.id);

                    let mut view = ThreadView::new(
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
                    .with_activity_text(activity_text);

                    // If thread has a pending permission, mark as needing action
                    if has_pending_permission {
                        view.needs_action = true;
                    }

                    view
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
    use crate::models::dashboard::PlanSummary;
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
            PlanRequest::new(
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
            PlanRequest::new(
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
            PlanRequest::new(
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

        state.set_plan_request("t1", PlanRequest::new("req-123".to_string(), summary));

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
        t1.status = Some(ThreadStatus::Done);
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
        t3.status = Some(ThreadStatus::Done);
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

    // -------------------- activity_text Tests --------------------

    #[test]
    fn test_activity_text_running_with_current_operation() {
        let mut state = DashboardState::new();
        let mut thread = make_thread("t1", "Test Thread");
        thread.status = Some(ThreadStatus::Running);
        state.threads.insert("t1".to_string(), thread);

        // Simulate agent state with current operation (use "tool_use" which maps to Running)
        state
            .agent_states
            .insert("t1".to_string(), ("tool_use".to_string(), Some("Read: main.rs".to_string())));

        let views = state.compute_thread_views();

        assert_eq!(views.len(), 1);
        let view = &views[0];
        assert_eq!(view.activity_text, Some("Read: main.rs".to_string()));
    }

    #[test]
    fn test_activity_text_running_without_current_operation() {
        let mut state = DashboardState::new();
        let mut thread = make_thread("t1", "Test Thread");
        thread.status = Some(ThreadStatus::Running);
        state.threads.insert("t1".to_string(), thread);

        // Simulate agent state without current operation (use "thinking" which maps to Running)
        state
            .agent_states
            .insert("t1".to_string(), ("thinking".to_string(), None));

        let views = state.compute_thread_views();

        assert_eq!(views.len(), 1);
        let view = &views[0];
        assert_eq!(view.activity_text, Some("Thinking...".to_string()));
    }

    #[test]
    fn test_activity_text_idle() {
        let mut state = DashboardState::new();
        let mut thread = make_thread("t1", "Test Thread");
        thread.status = Some(ThreadStatus::Done);
        state.threads.insert("t1".to_string(), thread);

        let views = state.compute_thread_views();

        assert_eq!(views.len(), 1);
        let view = &views[0];
        assert_eq!(view.activity_text, Some("ready".to_string()));
    }

    #[test]
    fn test_activity_text_done() {
        let mut state = DashboardState::new();
        let mut thread = make_thread("t1", "Test Thread");
        thread.status = Some(ThreadStatus::Done);
        state.threads.insert("t1".to_string(), thread);

        let views = state.compute_thread_views();

        assert_eq!(views.len(), 1);
        let view = &views[0];
        assert_eq!(view.activity_text, Some("ready".to_string()));
    }

    #[test]
    fn test_activity_text_error() {
        let mut state = DashboardState::new();
        let mut thread = make_thread("t1", "Test Thread");
        thread.status = Some(ThreadStatus::Error);
        state.threads.insert("t1".to_string(), thread);

        let views = state.compute_thread_views();

        assert_eq!(views.len(), 1);
        let view = &views[0];
        assert_eq!(view.activity_text, Some("error".to_string()));
    }

    #[test]
    fn test_activity_text_waiting_is_none() {
        let mut state = DashboardState::new();
        let mut thread = make_thread("t1", "Test Thread");
        thread.status = Some(ThreadStatus::Waiting);
        state.threads.insert("t1".to_string(), thread);

        let views = state.compute_thread_views();

        assert_eq!(views.len(), 1);
        let view = &views[0];
        // Waiting threads should have None activity_text (uses old layout)
        assert_eq!(view.activity_text, None);
    }

    // -------------------- update_current_operation Tests --------------------

    #[test]
    fn test_update_current_operation_updates_existing_thread() {
        let mut state = DashboardState::new();
        let mut thread = make_thread("t1", "Test Thread");
        thread.status = Some(ThreadStatus::Running);
        state.threads.insert("t1".to_string(), thread);

        // Set initial agent state
        state.update_agent_state("t1", "tool_use", None);

        // Verify no current_operation yet
        let views = state.compute_thread_views();
        assert_eq!(views[0].activity_text, Some("Thinking...".to_string()));

        // Update current operation via update_current_operation
        state.update_current_operation("t1", Some("Read: main.rs"));

        // Verify activity_text reflects the display_name
        let views = state.compute_thread_views();
        assert_eq!(views[0].activity_text, Some("Read: main.rs".to_string()));

        // Verify original state was preserved
        let (state_str, _) = state.agent_states.get("t1").unwrap();
        assert_eq!(state_str, "tool_use");
    }

    #[test]
    fn test_update_current_operation_creates_new_entry_if_missing() {
        let mut state = DashboardState::new();
        let mut thread = make_thread("t1", "Test Thread");
        thread.status = Some(ThreadStatus::Running);
        state.threads.insert("t1".to_string(), thread);

        // No agent state exists yet - update_current_operation should create one
        assert!(!state.agent_states.contains_key("t1"));

        state.update_current_operation("t1", Some("Edit: handlers.rs"));

        // Should have created entry with default "running" state
        let (state_str, op) = state.agent_states.get("t1").unwrap();
        assert_eq!(state_str, "running");
        assert_eq!(op.as_deref(), Some("Edit: handlers.rs"));

        // Verify activity_text reflects the display_name
        let views = state.compute_thread_views();
        assert_eq!(views[0].activity_text, Some("Edit: handlers.rs".to_string()));
    }

    #[test]
    fn test_update_current_operation_clears_operation() {
        let mut state = DashboardState::new();
        let mut thread = make_thread("t1", "Test Thread");
        thread.status = Some(ThreadStatus::Running);
        state.threads.insert("t1".to_string(), thread);

        // Set initial state with operation
        state.update_agent_state("t1", "tool_use", Some("Read: main.rs"));

        // Verify operation is set
        let views = state.compute_thread_views();
        assert_eq!(views[0].activity_text, Some("Read: main.rs".to_string()));

        // Clear the operation
        state.update_current_operation("t1", None);

        // Verify activity_text falls back to "Thinking..."
        let views = state.compute_thread_views();
        assert_eq!(views[0].activity_text, Some("Thinking...".to_string()));
    }

    #[test]
    fn test_update_current_operation_marks_views_dirty() {
        let mut state = DashboardState::new();
        let thread = make_thread("t1", "Test Thread");
        state.threads.insert("t1".to_string(), thread);

        // Compute views to clear dirty flag
        let _ = state.compute_thread_views();
        assert!(!state.thread_views_dirty);

        // Update current operation
        state.update_current_operation("t1", Some("Glob: *.rs"));

        // Verify dirty flag is set
        assert!(state.thread_views_dirty);
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
        let repos = vec![];
        let ctx = state.build_render_context(&stats, &theme, &repos);

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
        let repos = vec![];
        let ctx = state.build_render_context(&stats, &theme, &repos);

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
        t1.status = Some(ThreadStatus::Done);
        state.threads.insert("t1".to_string(), t1);

        // Build initial context
        let stats = SystemStats::default();
        let theme = Theme::default();
        let repos = vec![];
        let ctx = state.build_render_context(&stats, &theme, &repos);
        assert_eq!(ctx.threads.len(), 1);

        // Update thread status (marks dirty)
        state.update_thread_status("t1", ThreadStatus::Waiting, Some(WaitingFor::UserInput));

        // Verify dirty
        assert!(state.thread_views_dirty);

        // build_render_context should reflect the update
        let ctx = state.build_render_context(&stats, &theme, &repos);
        let view = &ctx.threads[0];
        assert!(view.needs_action); // Waiting threads need action
    }

    // -------------------- find_first_user_input_thread Tests --------------------

    #[test]
    fn test_find_first_user_input_thread_returns_first_match() {
        let mut state = DashboardState::new();

        // Add three threads
        let mut t1 = make_thread("t1", "Thread 1");
        t1.status = Some(ThreadStatus::Done);
        t1.updated_at = Utc::now() - chrono::Duration::seconds(10);
        state.threads.insert("t1".to_string(), t1);

        let mut t2 = make_thread("t2", "Thread 2");
        t2.status = Some(ThreadStatus::Waiting);
        t2.updated_at = Utc::now() - chrono::Duration::seconds(5);
        state.threads.insert("t2".to_string(), t2);

        let mut t3 = make_thread("t3", "Thread 3");
        t3.status = Some(ThreadStatus::Waiting);
        t3.updated_at = Utc::now();
        state.threads.insert("t3".to_string(), t3);

        // Set waiting states - both t2 and t3 waiting for user input
        state.waiting_for.insert("t2".to_string(), WaitingFor::UserInput);
        state.waiting_for.insert("t3".to_string(), WaitingFor::UserInput);

        // Compute views to ensure sorting is correct
        state.thread_views_dirty = true;
        state.compute_thread_views();

        // Should return t3 (most recent waiting thread)
        let result = state.find_first_user_input_thread();
        assert_eq!(result, Some("t3".to_string()));
    }

    #[test]
    fn test_find_first_user_input_thread_returns_none_when_no_match() {
        let mut state = DashboardState::new();

        // Add thread but with different waiting state
        let mut t1 = make_thread("t1", "Thread 1");
        t1.status = Some(ThreadStatus::Waiting);
        state.threads.insert("t1".to_string(), t1);
        state.waiting_for.insert(
            "t1".to_string(),
            WaitingFor::Permission {
                request_id: "req-1".to_string(),
                tool_name: "Bash".to_string(),
            },
        );

        state.thread_views_dirty = true;
        state.compute_thread_views();

        // Should return None since no thread has UserInput waiting state
        let result = state.find_first_user_input_thread();
        assert_eq!(result, None);
    }

    #[test]
    fn test_find_first_user_input_thread_returns_none_when_empty() {
        let state = DashboardState::new();

        // Empty state should return None
        let result = state.find_first_user_input_thread();
        assert_eq!(result, None);
    }

    #[test]
    fn test_find_first_user_input_thread_skips_non_user_input() {
        let mut state = DashboardState::new();

        // Add threads with different waiting states
        let mut t1 = make_thread("t1", "Thread 1");
        t1.status = Some(ThreadStatus::Waiting);
        t1.updated_at = Utc::now() - chrono::Duration::seconds(10);
        state.threads.insert("t1".to_string(), t1);

        let mut t2 = make_thread("t2", "Thread 2");
        t2.status = Some(ThreadStatus::Waiting);
        t2.updated_at = Utc::now() - chrono::Duration::seconds(5);
        state.threads.insert("t2".to_string(), t2);

        let mut t3 = make_thread("t3", "Thread 3");
        t3.status = Some(ThreadStatus::Waiting);
        t3.updated_at = Utc::now();
        state.threads.insert("t3".to_string(), t3);

        // t1 = Plan approval, t2 = UserInput, t3 = Permission
        state.waiting_for.insert(
            "t1".to_string(),
            WaitingFor::PlanApproval {
                request_id: "plan-1".to_string(),
            },
        );
        state.waiting_for.insert("t2".to_string(), WaitingFor::UserInput);
        state.waiting_for.insert(
            "t3".to_string(),
            WaitingFor::Permission {
                request_id: "perm-1".to_string(),
                tool_name: "Edit".to_string(),
            },
        );

        state.thread_views_dirty = true;
        state.compute_thread_views();

        // Should return t2 (only UserInput thread)
        let result = state.find_first_user_input_thread();
        assert_eq!(result, Some("t2".to_string()));
    }

    // -------------------- Pending Question Tests --------------------

    #[test]
    fn test_set_pending_question() {
        use crate::state::session::{AskUserQuestionData, Question, QuestionOption};

        let mut state = DashboardState::new();

        let question_data = AskUserQuestionData {
            questions: vec![Question {
                question: "Which auth method?".to_string(),
                header: "Auth".to_string(),
                options: vec![
                    QuestionOption {
                        label: "JWT".to_string(),
                        description: "Stateless tokens".to_string(),
                    },
                    QuestionOption {
                        label: "Sessions".to_string(),
                        description: "Server-side".to_string(),
                    },
                ],
                multi_select: false,
            }],
            answers: std::collections::HashMap::new(),
        };

        state.set_pending_question("thread-123", "req-123".to_string(), question_data.clone());

        let result = state.get_pending_question("thread-123");
        assert!(result.is_some());
        let stored = result.unwrap();
        assert_eq!(stored.questions.len(), 1);
        assert_eq!(stored.questions[0].question, "Which auth method?");
        assert_eq!(stored.questions[0].options.len(), 2);

        // Verify request_id is stored
        assert_eq!(
            state.get_pending_question_request_id("thread-123"),
            Some("req-123")
        );
    }

    #[test]
    fn test_set_pending_question_marks_dirty() {
        use crate::state::session::{AskUserQuestionData, Question, QuestionOption};

        let mut state = DashboardState::new();
        state.thread_views_dirty = false;

        let question_data = AskUserQuestionData {
            questions: vec![Question {
                question: "Test?".to_string(),
                header: "Test".to_string(),
                options: vec![],
                multi_select: false,
            }],
            answers: std::collections::HashMap::new(),
        };

        state.set_pending_question("thread-123", "req-123".to_string(), question_data);
        assert!(state.thread_views_dirty);
    }

    #[test]
    fn test_get_pending_question_nonexistent() {
        let state = DashboardState::new();
        assert!(state.get_pending_question("nonexistent").is_none());
    }

    #[test]
    fn test_clear_pending_question() {
        use crate::state::session::{AskUserQuestionData, Question, QuestionOption};

        let mut state = DashboardState::new();

        let question_data = AskUserQuestionData {
            questions: vec![Question {
                question: "Test?".to_string(),
                header: "Test".to_string(),
                options: vec![],
                multi_select: false,
            }],
            answers: std::collections::HashMap::new(),
        };

        state.set_pending_question("thread-123", "req-123".to_string(), question_data);
        assert!(state.get_pending_question("thread-123").is_some());

        state.clear_pending_question("thread-123");
        assert!(state.get_pending_question("thread-123").is_none());
    }

    #[test]
    fn test_clear_pending_question_marks_dirty() {
        use crate::state::session::{AskUserQuestionData, Question, QuestionOption};

        let mut state = DashboardState::new();

        let question_data = AskUserQuestionData {
            questions: vec![Question {
                question: "Test?".to_string(),
                header: "Test".to_string(),
                options: vec![],
                multi_select: false,
            }],
            answers: std::collections::HashMap::new(),
        };

        state.set_pending_question("thread-123", "req-123".to_string(), question_data);
        state.thread_views_dirty = false;

        state.clear_pending_question("thread-123");
        assert!(state.thread_views_dirty);
    }

    #[test]
    fn test_pending_questions_multiple_threads() {
        use crate::state::session::{AskUserQuestionData, Question, QuestionOption};

        let mut state = DashboardState::new();

        let q1 = AskUserQuestionData {
            questions: vec![Question {
                question: "Question 1?".to_string(),
                header: "Q1".to_string(),
                options: vec![QuestionOption {
                    label: "A".to_string(),
                    description: "Option A".to_string(),
                }],
                multi_select: false,
            }],
            answers: std::collections::HashMap::new(),
        };

        let q2 = AskUserQuestionData {
            questions: vec![Question {
                question: "Question 2?".to_string(),
                header: "Q2".to_string(),
                options: vec![
                    QuestionOption {
                        label: "X".to_string(),
                        description: "Option X".to_string(),
                    },
                    QuestionOption {
                        label: "Y".to_string(),
                        description: "Option Y".to_string(),
                    },
                ],
                multi_select: true,
            }],
            answers: std::collections::HashMap::new(),
        };

        state.set_pending_question("thread-1", "req-1".to_string(), q1);
        state.set_pending_question("thread-2", "req-2".to_string(), q2);

        let result1 = state.get_pending_question("thread-1").unwrap();
        let result2 = state.get_pending_question("thread-2").unwrap();

        assert_eq!(result1.questions[0].header, "Q1");
        assert_eq!(result2.questions[0].header, "Q2");
        assert!(!result1.questions[0].multi_select);
        assert!(result2.questions[0].multi_select);
    }

    #[test]
    fn test_set_pending_question_replaces_existing() {
        use crate::state::session::{AskUserQuestionData, Question, QuestionOption};

        let mut state = DashboardState::new();

        let q1 = AskUserQuestionData {
            questions: vec![Question {
                question: "First question?".to_string(),
                header: "First".to_string(),
                options: vec![],
                multi_select: false,
            }],
            answers: std::collections::HashMap::new(),
        };

        let q2 = AskUserQuestionData {
            questions: vec![Question {
                question: "Second question?".to_string(),
                header: "Second".to_string(),
                options: vec![],
                multi_select: true,
            }],
            answers: std::collections::HashMap::new(),
        };

        state.set_pending_question("thread-123", "req-1".to_string(), q1);
        state.set_pending_question("thread-123", "req-2".to_string(), q2);

        let result = state.get_pending_question("thread-123").unwrap();
        assert_eq!(result.questions[0].header, "Second");
        assert!(result.questions[0].multi_select);
        // Request ID should also be replaced
        assert_eq!(
            state.get_pending_question_request_id("thread-123"),
            Some("req-2")
        );
    }

    #[test]
    fn test_clear_nonexistent_pending_question_does_not_panic() {
        let mut state = DashboardState::new();
        // Should not panic when clearing a nonexistent question
        state.clear_pending_question("nonexistent");
        assert!(state.get_pending_question("nonexistent").is_none());
    }

    // -------------------- Expand Thread with Question Data Tests --------------------

    #[test]
    fn test_expand_thread_populates_question_data_from_pending_questions() {
        use crate::state::session::{AskUserQuestionData, Question, QuestionOption};

        let mut state = DashboardState::new();
        let thread = make_thread("t1", "Test Thread");
        state.threads.insert("t1".to_string(), thread);

        // Set pending question data
        let question_data = AskUserQuestionData {
            questions: vec![Question {
                question: "Which auth method?".to_string(),
                header: "Auth".to_string(),
                options: vec![
                    QuestionOption {
                        label: "JWT".to_string(),
                        description: "Stateless tokens".to_string(),
                    },
                    QuestionOption {
                        label: "Sessions".to_string(),
                        description: "Server-side sessions".to_string(),
                    },
                ],
                multi_select: false,
            }],
            answers: std::collections::HashMap::new(),
        };
        state.set_pending_question("t1", "req-t1".to_string(), question_data);

        // Expand the thread
        state.expand_thread("t1", 10);

        // Verify the overlay was created with the question data
        if let Some(OverlayState::Question {
            thread_id,
            question_data,
            ..
        }) = state.overlay()
        {
            assert_eq!(thread_id, "t1");
            assert!(question_data.is_some());
            let qd = question_data.as_ref().unwrap();
            assert_eq!(qd.questions.len(), 1);
            assert_eq!(qd.questions[0].question, "Which auth method?");
            assert_eq!(qd.questions[0].options.len(), 2);
            assert_eq!(qd.questions[0].options[0].label, "JWT");
            assert_eq!(qd.questions[0].options[1].label, "Sessions");
        } else {
            panic!("Expected Question overlay with question_data");
        }
    }

    #[test]
    fn test_expand_thread_with_no_pending_question_has_none_question_data() {
        let mut state = DashboardState::new();
        let thread = make_thread("t1", "Test Thread");
        state.threads.insert("t1".to_string(), thread);

        // No pending question set

        // Expand the thread
        state.expand_thread("t1", 10);

        // Verify the overlay was created without question data
        if let Some(OverlayState::Question { question_data, .. }) = state.overlay() {
            assert!(question_data.is_none());
        } else {
            panic!("Expected Question overlay");
        }
    }

    #[test]
    fn test_show_free_form_preserves_question_data() {
        use crate::state::session::{AskUserQuestionData, Question, QuestionOption};

        let mut state = DashboardState::new();
        let thread = make_thread("t1", "Test Thread");
        state.threads.insert("t1".to_string(), thread);

        // Set pending question data
        let question_data = AskUserQuestionData {
            questions: vec![Question {
                question: "Select feature".to_string(),
                header: "Feature".to_string(),
                options: vec![QuestionOption {
                    label: "Feature A".to_string(),
                    description: "Enable feature A".to_string(),
                }],
                multi_select: false,
            }],
            answers: std::collections::HashMap::new(),
        };
        state.set_pending_question("t1", "req-t1".to_string(), question_data);

        // Expand and switch to free form
        state.expand_thread("t1", 10);
        state.show_free_form("t1");

        // Verify FreeForm overlay has the question data
        if let Some(OverlayState::FreeForm { question_data, .. }) = state.overlay() {
            assert!(question_data.is_some());
            let qd = question_data.as_ref().unwrap();
            assert_eq!(qd.questions[0].question, "Select feature");
        } else {
            panic!("Expected FreeForm overlay");
        }
    }

    #[test]
    fn test_back_to_options_preserves_question_data() {
        use crate::state::session::{AskUserQuestionData, Question, QuestionOption};

        let mut state = DashboardState::new();
        let thread = make_thread("t1", "Test Thread");
        state.threads.insert("t1".to_string(), thread);

        // Set pending question data
        let question_data = AskUserQuestionData {
            questions: vec![Question {
                question: "Choose option".to_string(),
                header: "Options".to_string(),
                options: vec![
                    QuestionOption {
                        label: "A".to_string(),
                        description: "Option A".to_string(),
                    },
                    QuestionOption {
                        label: "B".to_string(),
                        description: "Option B".to_string(),
                    },
                ],
                multi_select: true,
            }],
            answers: std::collections::HashMap::new(),
        };
        state.set_pending_question("t1", "req-t1".to_string(), question_data);

        // Expand, go to free form, then back to options
        state.expand_thread("t1", 10);
        state.show_free_form("t1");
        state.back_to_options("t1");

        // Verify Question overlay still has the question data
        if let Some(OverlayState::Question { question_data, .. }) = state.overlay() {
            assert!(question_data.is_some());
            let qd = question_data.as_ref().unwrap();
            assert_eq!(qd.questions[0].question, "Choose option");
            assert!(qd.questions[0].multi_select);
            assert_eq!(qd.questions[0].options.len(), 2);
        } else {
            panic!("Expected Question overlay");
        }
    }

    #[test]
    fn test_expand_thread_with_user_input_waiting_populates_question_data() {
        use crate::state::session::{AskUserQuestionData, Question, QuestionOption};

        let mut state = DashboardState::new();
        let thread = make_thread("t1", "Test Thread");
        state.threads.insert("t1".to_string(), thread);

        // Set WaitingFor::UserInput
        state
            .waiting_for
            .insert("t1".to_string(), WaitingFor::UserInput);

        // Set pending question data
        let question_data = AskUserQuestionData {
            questions: vec![Question {
                question: "Pick database".to_string(),
                header: "Database".to_string(),
                options: vec![
                    QuestionOption {
                        label: "PostgreSQL".to_string(),
                        description: "Relational database".to_string(),
                    },
                    QuestionOption {
                        label: "MongoDB".to_string(),
                        description: "Document database".to_string(),
                    },
                ],
                multi_select: false,
            }],
            answers: std::collections::HashMap::new(),
        };
        state.set_pending_question("t1", "req-t1".to_string(), question_data);

        // Expand the thread
        state.expand_thread("t1", 5);

        // Verify the overlay has question data
        if let Some(OverlayState::Question { question_data, .. }) = state.overlay() {
            assert!(question_data.is_some());
            let qd = question_data.as_ref().unwrap();
            assert_eq!(qd.questions[0].question, "Pick database");
        } else {
            panic!("Expected Question overlay");
        }
    }

    // -------------------- Question Submit Tests --------------------

    #[test]
    fn test_question_confirm_returns_request_id() {
        use crate::state::session::{AskUserQuestionData, Question, QuestionOption};

        let mut state = DashboardState::new();
        let thread = make_thread("t1", "Test Thread");
        state.threads.insert("t1".to_string(), thread);

        // Set pending question data with a request_id
        let question_data = AskUserQuestionData {
            questions: vec![Question {
                question: "Which option?".to_string(),
                header: "Options".to_string(),
                options: vec![QuestionOption {
                    label: "Option A".to_string(),
                    description: "First option".to_string(),
                }],
                multi_select: false,
            }],
            answers: std::collections::HashMap::new(),
        };
        state.set_pending_question("t1", "req-submit-test".to_string(), question_data);

        // Expand the thread to open overlay
        state.expand_thread("t1", 10);

        // Select the first option
        if let Some(qs) = &mut state.question_state {
            qs.set_current_selection(Some(0));
        }

        // Confirm should return (thread_id, request_id, answers)
        let result = state.question_confirm();
        assert!(result.is_some());
        let (thread_id, request_id, answers) = result.unwrap();
        assert_eq!(thread_id, "t1");
        assert_eq!(request_id, "req-submit-test");
        assert_eq!(answers.get("Which option?"), Some(&"Option A".to_string()));
    }

    #[test]
    fn test_get_pending_question_request_id_returns_none_for_nonexistent() {
        let state = DashboardState::new();
        assert!(state.get_pending_question_request_id("nonexistent").is_none());
    }

    // -------------------- DashboardQuestionState Navigation Tests --------------------

    #[test]
    fn test_dashboard_question_state_from_question_data() {
        use crate::state::session::{AskUserQuestionData, Question, QuestionOption};

        let question_data = AskUserQuestionData {
            questions: vec![
                Question {
                    question: "Q1?".to_string(),
                    header: "Q1".to_string(),
                    options: vec![
                        QuestionOption {
                            label: "A".to_string(),
                            description: "Opt A".to_string(),
                        },
                        QuestionOption {
                            label: "B".to_string(),
                            description: "Opt B".to_string(),
                        },
                    ],
                    multi_select: false,
                },
                Question {
                    question: "Q2?".to_string(),
                    header: "Q2".to_string(),
                    options: vec![
                        QuestionOption {
                            label: "X".to_string(),
                            description: "Opt X".to_string(),
                        },
                        QuestionOption {
                            label: "Y".to_string(),
                            description: "Opt Y".to_string(),
                        },
                        QuestionOption {
                            label: "Z".to_string(),
                            description: "Opt Z".to_string(),
                        },
                    ],
                    multi_select: true,
                },
            ],
            answers: std::collections::HashMap::new(),
        };

        let state = DashboardQuestionState::from_question_data(&question_data);

        assert_eq!(state.tab_index, 0);
        assert_eq!(state.selections.len(), 2);
        assert_eq!(state.selections[0], Some(0));
        assert_eq!(state.selections[1], Some(0));
        assert_eq!(state.multi_selections.len(), 2);
        assert_eq!(state.multi_selections[0].len(), 2);
        assert_eq!(state.multi_selections[1].len(), 3);
        assert_eq!(state.other_texts.len(), 2);
        assert!(!state.other_active);
        assert_eq!(state.answered.len(), 2);
    }

    #[test]
    fn test_dashboard_question_state_prev_option() {
        use crate::state::session::{AskUserQuestionData, Question, QuestionOption};

        let question_data = AskUserQuestionData {
            questions: vec![Question {
                question: "Q?".to_string(),
                header: "Q".to_string(),
                options: vec![
                    QuestionOption {
                        label: "A".to_string(),
                        description: "".to_string(),
                    },
                    QuestionOption {
                        label: "B".to_string(),
                        description: "".to_string(),
                    },
                    QuestionOption {
                        label: "C".to_string(),
                        description: "".to_string(),
                    },
                ],
                multi_select: false,
            }],
            answers: std::collections::HashMap::new(),
        };

        let mut state = DashboardQuestionState::from_question_data(&question_data);

        // Start at option 0
        assert_eq!(state.current_selection(), Some(0));

        // Prev wraps to "Other" (None)
        state.prev_option(3);
        assert_eq!(state.current_selection(), None);

        // Prev from "Other" wraps to last option
        state.prev_option(3);
        assert_eq!(state.current_selection(), Some(2));

        // Prev to middle option
        state.prev_option(3);
        assert_eq!(state.current_selection(), Some(1));
    }

    #[test]
    fn test_dashboard_question_state_next_option() {
        use crate::state::session::{AskUserQuestionData, Question, QuestionOption};

        let question_data = AskUserQuestionData {
            questions: vec![Question {
                question: "Q?".to_string(),
                header: "Q".to_string(),
                options: vec![
                    QuestionOption {
                        label: "A".to_string(),
                        description: "".to_string(),
                    },
                    QuestionOption {
                        label: "B".to_string(),
                        description: "".to_string(),
                    },
                ],
                multi_select: false,
            }],
            answers: std::collections::HashMap::new(),
        };

        let mut state = DashboardQuestionState::from_question_data(&question_data);

        // Start at option 0
        assert_eq!(state.current_selection(), Some(0));

        // Next to option 1
        state.next_option(2);
        assert_eq!(state.current_selection(), Some(1));

        // Next wraps to "Other" (None)
        state.next_option(2);
        assert_eq!(state.current_selection(), None);

        // Next from "Other" wraps to first option
        state.next_option(2);
        assert_eq!(state.current_selection(), Some(0));
    }

    #[test]
    fn test_dashboard_question_state_next_tab() {
        use crate::state::session::{AskUserQuestionData, Question};

        let question_data = AskUserQuestionData {
            questions: vec![
                Question {
                    question: "Q1?".to_string(),
                    header: "Q1".to_string(),
                    options: vec![],
                    multi_select: false,
                },
                Question {
                    question: "Q2?".to_string(),
                    header: "Q2".to_string(),
                    options: vec![],
                    multi_select: false,
                },
                Question {
                    question: "Q3?".to_string(),
                    header: "Q3".to_string(),
                    options: vec![],
                    multi_select: false,
                },
            ],
            answers: std::collections::HashMap::new(),
        };

        let mut state = DashboardQuestionState::from_question_data(&question_data);

        assert_eq!(state.tab_index, 0);

        state.next_tab(3);
        assert_eq!(state.tab_index, 1);

        state.next_tab(3);
        assert_eq!(state.tab_index, 2);

        // Wrap around
        state.next_tab(3);
        assert_eq!(state.tab_index, 0);
    }

    #[test]
    fn test_dashboard_question_state_toggle_multi_selection() {
        use crate::state::session::{AskUserQuestionData, Question, QuestionOption};

        let question_data = AskUserQuestionData {
            questions: vec![Question {
                question: "Q?".to_string(),
                header: "Q".to_string(),
                options: vec![
                    QuestionOption {
                        label: "A".to_string(),
                        description: "".to_string(),
                    },
                    QuestionOption {
                        label: "B".to_string(),
                        description: "".to_string(),
                    },
                ],
                multi_select: true,
            }],
            answers: std::collections::HashMap::new(),
        };

        let mut state = DashboardQuestionState::from_question_data(&question_data);

        // Initially nothing selected
        assert!(!state.is_multi_selected(0));
        assert!(!state.is_multi_selected(1));

        // Toggle first option
        state.toggle_multi_selection(0);
        assert!(state.is_multi_selected(0));
        assert!(!state.is_multi_selected(1));

        // Toggle second option
        state.toggle_multi_selection(1);
        assert!(state.is_multi_selected(0));
        assert!(state.is_multi_selected(1));

        // Toggle first again (off)
        state.toggle_multi_selection(0);
        assert!(!state.is_multi_selected(0));
        assert!(state.is_multi_selected(1));
    }

    #[test]
    fn test_dashboard_question_state_other_text_operations() {
        use crate::state::session::{AskUserQuestionData, Question, QuestionOption};

        let question_data = AskUserQuestionData {
            questions: vec![Question {
                question: "Q?".to_string(),
                header: "Q".to_string(),
                options: vec![QuestionOption {
                    label: "A".to_string(),
                    description: "".to_string(),
                }],
                multi_select: false,
            }],
            answers: std::collections::HashMap::new(),
        };

        let mut state = DashboardQuestionState::from_question_data(&question_data);

        // Initially empty
        assert_eq!(state.current_other_text(), "");

        // Type some characters
        state.push_other_char('H');
        state.push_other_char('e');
        state.push_other_char('l');
        state.push_other_char('l');
        state.push_other_char('o');
        assert_eq!(state.current_other_text(), "Hello");

        // Backspace
        state.pop_other_char();
        assert_eq!(state.current_other_text(), "Hell");

        state.pop_other_char();
        assert_eq!(state.current_other_text(), "Hel");
    }

    #[test]
    fn test_dashboard_question_state_mark_answered() {
        use crate::state::session::{AskUserQuestionData, Question};

        let question_data = AskUserQuestionData {
            questions: vec![
                Question {
                    question: "Q1?".to_string(),
                    header: "Q1".to_string(),
                    options: vec![],
                    multi_select: false,
                },
                Question {
                    question: "Q2?".to_string(),
                    header: "Q2".to_string(),
                    options: vec![],
                    multi_select: false,
                },
            ],
            answers: std::collections::HashMap::new(),
        };

        let mut state = DashboardQuestionState::from_question_data(&question_data);

        // Initially none answered
        assert!(!state.all_answered());

        // Mark first as answered
        state.mark_current_answered();
        assert!(state.answered[0]);
        assert!(!state.answered[1]);
        assert!(!state.all_answered());

        // Move to second and mark
        state.tab_index = 1;
        state.mark_current_answered();
        assert!(state.answered[0]);
        assert!(state.answered[1]);
        assert!(state.all_answered());
    }

    #[test]
    fn test_dashboard_question_state_advance_to_next_unanswered() {
        use crate::state::session::{AskUserQuestionData, Question};

        let question_data = AskUserQuestionData {
            questions: vec![
                Question {
                    question: "Q1?".to_string(),
                    header: "Q1".to_string(),
                    options: vec![],
                    multi_select: false,
                },
                Question {
                    question: "Q2?".to_string(),
                    header: "Q2".to_string(),
                    options: vec![],
                    multi_select: false,
                },
                Question {
                    question: "Q3?".to_string(),
                    header: "Q3".to_string(),
                    options: vec![],
                    multi_select: false,
                },
            ],
            answers: std::collections::HashMap::new(),
        };

        let mut state = DashboardQuestionState::from_question_data(&question_data);

        // Mark first as answered
        state.answered[0] = true;

        // Advance should move to tab 1
        let advanced = state.advance_to_next_unanswered(3);
        assert!(advanced);
        assert_eq!(state.tab_index, 1);

        // Mark second as answered
        state.answered[1] = true;

        // Advance should move to tab 2
        let advanced = state.advance_to_next_unanswered(3);
        assert!(advanced);
        assert_eq!(state.tab_index, 2);

        // Mark third as answered
        state.answered[2] = true;

        // No more unanswered - should return false
        let advanced = state.advance_to_next_unanswered(3);
        assert!(!advanced);
    }

    #[test]
    fn test_question_confirm_single_question_submits_immediately() {
        use crate::state::session::{AskUserQuestionData, Question, QuestionOption};

        let mut state = DashboardState::new();
        let thread = make_thread("t1", "Test Thread");
        state.threads.insert("t1".to_string(), thread);

        // Single question
        let question_data = AskUserQuestionData {
            questions: vec![Question {
                question: "Which framework?".to_string(),
                header: "Framework".to_string(),
                options: vec![
                    QuestionOption {
                        label: "React".to_string(),
                        description: "UI library".to_string(),
                    },
                    QuestionOption {
                        label: "Vue".to_string(),
                        description: "Progressive framework".to_string(),
                    },
                ],
                multi_select: false,
            }],
            answers: std::collections::HashMap::new(),
        };

        state.set_pending_question("t1", "req-single".to_string(), question_data);
        state.expand_thread("t1", 10);

        // Select second option
        if let Some(qs) = &mut state.question_state {
            qs.set_current_selection(Some(1));
        }

        // Confirm should return answers immediately (single question)
        let result = state.question_confirm();
        assert!(result.is_some());
        let (thread_id, request_id, answers) = result.unwrap();
        assert_eq!(thread_id, "t1");
        assert_eq!(request_id, "req-single");
        assert_eq!(answers.get("Which framework?"), Some(&"Vue".to_string()));
    }

    #[test]
    fn test_question_confirm_multi_question_requires_all_answered() {
        use crate::state::session::{AskUserQuestionData, Question, QuestionOption};

        let mut state = DashboardState::new();
        let thread = make_thread("t1", "Test Thread");
        state.threads.insert("t1".to_string(), thread);

        // Multiple questions
        let question_data = AskUserQuestionData {
            questions: vec![
                Question {
                    question: "Q1?".to_string(),
                    header: "Q1".to_string(),
                    options: vec![QuestionOption {
                        label: "A".to_string(),
                        description: "".to_string(),
                    }],
                    multi_select: false,
                },
                Question {
                    question: "Q2?".to_string(),
                    header: "Q2".to_string(),
                    options: vec![QuestionOption {
                        label: "B".to_string(),
                        description: "".to_string(),
                    }],
                    multi_select: false,
                },
            ],
            answers: std::collections::HashMap::new(),
        };

        state.set_pending_question("t1", "req-multi".to_string(), question_data);
        state.expand_thread("t1", 10);

        // First confirm should mark Q1 as answered and advance
        let result = state.question_confirm();
        assert!(result.is_none());
        assert_eq!(state.question_state.as_ref().unwrap().tab_index, 1);
        assert!(state.question_state.as_ref().unwrap().answered[0]);

        // Second confirm should submit
        let result = state.question_confirm();
        assert!(result.is_some());
        let (_, _, answers) = result.unwrap();
        assert_eq!(answers.len(), 2);
    }

    #[test]
    fn test_question_confirm_activates_other_when_selected() {
        use crate::state::session::{AskUserQuestionData, Question, QuestionOption};

        let mut state = DashboardState::new();
        let thread = make_thread("t1", "Test Thread");
        state.threads.insert("t1".to_string(), thread);

        let question_data = AskUserQuestionData {
            questions: vec![Question {
                question: "Q?".to_string(),
                header: "Q".to_string(),
                options: vec![QuestionOption {
                    label: "A".to_string(),
                    description: "".to_string(),
                }],
                multi_select: false,
            }],
            answers: std::collections::HashMap::new(),
        };

        state.set_pending_question("t1", "req-other".to_string(), question_data);
        state.expand_thread("t1", 10);

        // Select "Other" (None)
        if let Some(qs) = &mut state.question_state {
            qs.set_current_selection(None);
        }

        // Confirm should activate "Other" text input mode
        let result = state.question_confirm();
        assert!(result.is_none());
        assert!(state.question_state.as_ref().unwrap().other_active);
    }

    #[test]
    fn test_question_confirm_requires_other_text_not_empty() {
        use crate::state::session::{AskUserQuestionData, Question, QuestionOption};

        let mut state = DashboardState::new();
        let thread = make_thread("t1", "Test Thread");
        state.threads.insert("t1".to_string(), thread);

        let question_data = AskUserQuestionData {
            questions: vec![Question {
                question: "Q?".to_string(),
                header: "Q".to_string(),
                options: vec![QuestionOption {
                    label: "A".to_string(),
                    description: "".to_string(),
                }],
                multi_select: false,
            }],
            answers: std::collections::HashMap::new(),
        };

        state.set_pending_question("t1", "req-validate".to_string(), question_data);
        state.expand_thread("t1", 10);

        // Activate "Other" mode with empty text
        if let Some(qs) = &mut state.question_state {
            qs.set_current_selection(None);
            qs.other_active = true;
        }

        // Confirm should fail (empty text)
        let result = state.question_confirm();
        assert!(result.is_none());

        // Add text
        if let Some(qs) = &mut state.question_state {
            qs.push_other_char('H');
            qs.push_other_char('i');
        }

        // Now confirm should succeed
        let result = state.question_confirm();
        assert!(result.is_some());
        let (_, _, answers) = result.unwrap();
        assert_eq!(answers.get("Q?"), Some(&"Hi".to_string()));
    }

    #[test]
    fn test_question_state_reset_on_collapse() {
        use crate::state::session::{AskUserQuestionData, Question};

        let mut state = DashboardState::new();
        let thread = make_thread("t1", "Test Thread");
        state.threads.insert("t1".to_string(), thread);

        let question_data = AskUserQuestionData {
            questions: vec![Question {
                question: "Q?".to_string(),
                header: "Q".to_string(),
                options: vec![],
                multi_select: false,
            }],
            answers: std::collections::HashMap::new(),
        };

        state.set_pending_question("t1", "req-collapse".to_string(), question_data);
        state.expand_thread("t1", 10);

        // Question state should be initialized
        assert!(state.question_state.is_some());

        // Collapse overlay
        state.collapse_overlay();

        // Question state should be cleared
        assert!(state.question_state.is_none());
    }

    // -------------------- get_top_needs_action_thread Tests --------------------

    #[test]
    fn test_get_top_needs_action_thread_returns_user_input() {
        let mut state = DashboardState::new();
        let mut t1 = make_thread("t1", "Thread 1");
        t1.status = Some(ThreadStatus::Waiting);
        state.threads.insert("t1".to_string(), t1);
        state.waiting_for.insert("t1".to_string(), WaitingFor::UserInput);

        // Rebuild thread views to populate needs_action
        state.thread_views_dirty = true;
        let _ = state.compute_thread_views();

        let result = state.get_top_needs_action_thread();
        assert!(result.is_some());
        let (thread_id, waiting_for) = result.unwrap();
        assert_eq!(thread_id, "t1");
        assert!(matches!(waiting_for, WaitingFor::UserInput));
    }

    #[test]
    fn test_get_top_needs_action_thread_returns_permission() {
        let mut state = DashboardState::new();
        let mut t1 = make_thread("t1", "Thread 1");
        t1.status = Some(ThreadStatus::Waiting);
        state.threads.insert("t1".to_string(), t1);
        state.waiting_for.insert(
            "t1".to_string(),
            WaitingFor::Permission {
                request_id: "req-123".to_string(),
                tool_name: "Bash".to_string(),
            },
        );

        // Rebuild thread views to populate needs_action
        state.thread_views_dirty = true;
        let _ = state.compute_thread_views();

        let result = state.get_top_needs_action_thread();
        assert!(result.is_some());
        let (thread_id, waiting_for) = result.unwrap();
        assert_eq!(thread_id, "t1");
        if let WaitingFor::Permission { tool_name, .. } = waiting_for {
            assert_eq!(tool_name, "Bash");
        } else {
            panic!("Expected Permission variant");
        }
    }

    #[test]
    fn test_get_top_needs_action_thread_returns_none_when_no_threads_need_action() {
        let mut state = DashboardState::new();
        let mut t1 = make_thread("t1", "Thread 1");
        t1.status = Some(ThreadStatus::Done);
        state.threads.insert("t1".to_string(), t1);

        // Rebuild thread views
        state.thread_views_dirty = true;
        let _ = state.compute_thread_views();

        let result = state.get_top_needs_action_thread();
        assert!(result.is_none());
    }

    #[test]
    fn test_get_top_needs_action_thread_returns_top_thread_when_multiple_need_action() {
        let mut state = DashboardState::new();

        // Create two waiting threads with different timestamps
        let mut t1 = make_thread("t1", "Thread 1");
        t1.status = Some(ThreadStatus::Waiting);
        t1.updated_at = Utc::now() - chrono::Duration::seconds(10);

        let mut t2 = make_thread("t2", "Thread 2");
        t2.status = Some(ThreadStatus::Waiting);
        t2.updated_at = Utc::now();

        state.threads.insert("t1".to_string(), t1);
        state.threads.insert("t2".to_string(), t2);

        state.waiting_for.insert(
            "t1".to_string(),
            WaitingFor::Permission {
                request_id: "req-1".to_string(),
                tool_name: "Bash".to_string(),
            },
        );
        state.waiting_for.insert("t2".to_string(), WaitingFor::UserInput);

        // Rebuild thread views to populate needs_action
        state.thread_views_dirty = true;
        let views = state.compute_thread_views();
        let first_needs_action_id = views[0].id.clone();

        // The top thread should be returned (first in the sorted list)
        let result = state.get_top_needs_action_thread();
        assert!(result.is_some());
        let (thread_id, _) = result.unwrap();

        // Should match the first needs_action thread in the views
        assert_eq!(thread_id, first_needs_action_id);
    }

    #[test]
    fn test_get_top_needs_action_thread_prioritizes_user_input_over_permission() {
        let mut state = DashboardState::new();

        // Create two waiting threads
        let mut t1 = make_thread("t1", "Thread 1");
        t1.status = Some(ThreadStatus::Waiting);
        t1.updated_at = Utc::now();

        let mut t2 = make_thread("t2", "Thread 2");
        t2.status = Some(ThreadStatus::Waiting);
        t2.updated_at = Utc::now() - chrono::Duration::seconds(5);

        state.threads.insert("t1".to_string(), t1);
        state.threads.insert("t2".to_string(), t2);

        // t1 has UserInput (higher priority), t2 has Permission
        state.waiting_for.insert("t1".to_string(), WaitingFor::UserInput);
        state.waiting_for.insert(
            "t2".to_string(),
            WaitingFor::Permission {
                request_id: "req-2".to_string(),
                tool_name: "Edit".to_string(),
            },
        );

        // Rebuild thread views to populate needs_action
        state.thread_views_dirty = true;
        let _ = state.compute_thread_views();

        let result = state.get_top_needs_action_thread();
        assert!(result.is_some());
        let (thread_id, waiting_for) = result.unwrap();

        // UserInput should be prioritized
        assert_eq!(thread_id, "t1");
        assert!(matches!(waiting_for, WaitingFor::UserInput));
    }

    // -------------------- build_question_answers Tests --------------------
    // These tests verify the answers HashMap is built correctly for WebSocket response

    #[test]
    fn test_build_question_answers_single_select() {
        use crate::state::session::{AskUserQuestionData, Question, QuestionOption};

        let mut state = DashboardState::new();
        let thread = make_thread("t1", "Test Thread");
        state.threads.insert("t1".to_string(), thread);

        let question_data = AskUserQuestionData {
            questions: vec![Question {
                question: "Which framework should we use?".to_string(),
                header: "Framework".to_string(),
                options: vec![
                    QuestionOption {
                        label: "React".to_string(),
                        description: "UI library".to_string(),
                    },
                    QuestionOption {
                        label: "Vue".to_string(),
                        description: "Progressive framework".to_string(),
                    },
                    QuestionOption {
                        label: "Angular".to_string(),
                        description: "Full framework".to_string(),
                    },
                ],
                multi_select: false,
            }],
            answers: std::collections::HashMap::new(),
        };

        state.set_pending_question("t1", "req-single-select".to_string(), question_data);
        state.expand_thread("t1", 10);

        // Select "Vue" (index 1)
        if let Some(qs) = &mut state.question_state {
            qs.set_current_selection(Some(1));
        }

        let result = state.build_question_answers();
        assert!(result.is_some());
        let (thread_id, request_id, answers) = result.unwrap();

        assert_eq!(thread_id, "t1");
        assert_eq!(request_id, "req-single-select");
        assert_eq!(answers.len(), 1);
        assert_eq!(
            answers.get("Which framework should we use?"),
            Some(&"Vue".to_string())
        );
    }

    #[test]
    fn test_build_question_answers_multi_select() {
        use crate::state::session::{AskUserQuestionData, Question, QuestionOption};

        let mut state = DashboardState::new();
        let thread = make_thread("t1", "Test Thread");
        state.threads.insert("t1".to_string(), thread);

        let question_data = AskUserQuestionData {
            questions: vec![Question {
                question: "Select testing tools".to_string(),
                header: "Testing".to_string(),
                options: vec![
                    QuestionOption {
                        label: "Jest".to_string(),
                        description: "Unit testing".to_string(),
                    },
                    QuestionOption {
                        label: "Cypress".to_string(),
                        description: "E2E testing".to_string(),
                    },
                    QuestionOption {
                        label: "Playwright".to_string(),
                        description: "Browser testing".to_string(),
                    },
                ],
                multi_select: true,
            }],
            answers: std::collections::HashMap::new(),
        };

        state.set_pending_question("t1", "req-multi-select".to_string(), question_data);
        state.expand_thread("t1", 10);

        // Select Jest and Playwright (indices 0 and 2)
        if let Some(qs) = &mut state.question_state {
            qs.toggle_multi_selection(0);
            qs.toggle_multi_selection(2);
        }

        let result = state.build_question_answers();
        assert!(result.is_some());
        let (_, _, answers) = result.unwrap();

        assert_eq!(answers.len(), 1);
        let answer = answers.get("Select testing tools").unwrap();
        // Multi-select answers are comma-separated
        assert!(answer.contains("Jest"));
        assert!(answer.contains("Playwright"));
        assert!(!answer.contains("Cypress"));
    }

    #[test]
    fn test_build_question_answers_other_text() {
        use crate::state::session::{AskUserQuestionData, Question, QuestionOption};

        let mut state = DashboardState::new();
        let thread = make_thread("t1", "Test Thread");
        state.threads.insert("t1".to_string(), thread);

        let question_data = AskUserQuestionData {
            questions: vec![Question {
                question: "Which database?".to_string(),
                header: "Database".to_string(),
                options: vec![
                    QuestionOption {
                        label: "PostgreSQL".to_string(),
                        description: "Relational DB".to_string(),
                    },
                    QuestionOption {
                        label: "MongoDB".to_string(),
                        description: "Document DB".to_string(),
                    },
                ],
                multi_select: false,
            }],
            answers: std::collections::HashMap::new(),
        };

        state.set_pending_question("t1", "req-other".to_string(), question_data);
        state.expand_thread("t1", 10);

        // Select "Other" (None) and enter custom text
        if let Some(qs) = &mut state.question_state {
            qs.set_current_selection(None);
            qs.push_other_char('S');
            qs.push_other_char('Q');
            qs.push_other_char('L');
            qs.push_other_char('i');
            qs.push_other_char('t');
            qs.push_other_char('e');
        }

        let result = state.build_question_answers();
        assert!(result.is_some());
        let (_, _, answers) = result.unwrap();

        assert_eq!(answers.len(), 1);
        assert_eq!(
            answers.get("Which database?"),
            Some(&"SQLite".to_string())
        );
    }

    #[test]
    fn test_build_question_answers_multi_select_with_other() {
        use crate::state::session::{AskUserQuestionData, Question, QuestionOption};

        let mut state = DashboardState::new();
        let thread = make_thread("t1", "Test Thread");
        state.threads.insert("t1".to_string(), thread);

        let question_data = AskUserQuestionData {
            questions: vec![Question {
                question: "Select features".to_string(),
                header: "Features".to_string(),
                options: vec![
                    QuestionOption {
                        label: "Auth".to_string(),
                        description: "Authentication".to_string(),
                    },
                    QuestionOption {
                        label: "API".to_string(),
                        description: "REST API".to_string(),
                    },
                ],
                multi_select: true,
            }],
            answers: std::collections::HashMap::new(),
        };

        state.set_pending_question("t1", "req-multi-other".to_string(), question_data);
        state.expand_thread("t1", 10);

        // Select "Auth" and add "Other" text
        if let Some(qs) = &mut state.question_state {
            qs.toggle_multi_selection(0);
            qs.push_other_char('C');
            qs.push_other_char('a');
            qs.push_other_char('c');
            qs.push_other_char('h');
            qs.push_other_char('e');
        }

        let result = state.build_question_answers();
        assert!(result.is_some());
        let (_, _, answers) = result.unwrap();

        let answer = answers.get("Select features").unwrap();
        // Should include both the selected option and the other text
        assert!(answer.contains("Auth"));
        assert!(answer.contains("Cache"));
    }

    #[test]
    fn test_build_question_answers_multiple_questions() {
        use crate::state::session::{AskUserQuestionData, Question, QuestionOption};

        let mut state = DashboardState::new();
        let thread = make_thread("t1", "Test Thread");
        state.threads.insert("t1".to_string(), thread);

        let question_data = AskUserQuestionData {
            questions: vec![
                Question {
                    question: "Which language?".to_string(),
                    header: "Language".to_string(),
                    options: vec![
                        QuestionOption {
                            label: "Rust".to_string(),
                            description: "Systems language".to_string(),
                        },
                        QuestionOption {
                            label: "Go".to_string(),
                            description: "Simple language".to_string(),
                        },
                    ],
                    multi_select: false,
                },
                Question {
                    question: "Which framework?".to_string(),
                    header: "Framework".to_string(),
                    options: vec![
                        QuestionOption {
                            label: "Actix".to_string(),
                            description: "Fast web framework".to_string(),
                        },
                        QuestionOption {
                            label: "Axum".to_string(),
                            description: "Ergonomic framework".to_string(),
                        },
                    ],
                    multi_select: false,
                },
            ],
            answers: std::collections::HashMap::new(),
        };

        state.set_pending_question("t1", "req-multi-question".to_string(), question_data);
        state.expand_thread("t1", 10);

        // Answer first question: select "Rust"
        if let Some(qs) = &mut state.question_state {
            qs.set_current_selection(Some(0));
            // Move to second tab
            qs.tab_index = 1;
            // Select "Axum" for second question
            qs.selections[1] = Some(1);
        }

        let result = state.build_question_answers();
        assert!(result.is_some());
        let (_, _, answers) = result.unwrap();

        assert_eq!(answers.len(), 2);
        assert_eq!(answers.get("Which language?"), Some(&"Rust".to_string()));
        assert_eq!(answers.get("Which framework?"), Some(&"Axum".to_string()));
    }

    #[test]
    fn test_build_question_answers_uses_question_text_as_key() {
        use crate::state::session::{AskUserQuestionData, Question, QuestionOption};

        let mut state = DashboardState::new();
        let thread = make_thread("t1", "Test Thread");
        state.threads.insert("t1".to_string(), thread);

        // The question text (not header) should be used as the HashMap key
        let question_data = AskUserQuestionData {
            questions: vec![Question {
                question: "What is your preferred authentication method?".to_string(),
                header: "Auth".to_string(), // Short header for UI
                options: vec![QuestionOption {
                    label: "OAuth 2.0".to_string(),
                    description: "Industry standard".to_string(),
                }],
                multi_select: false,
            }],
            answers: std::collections::HashMap::new(),
        };

        state.set_pending_question("t1", "req-key".to_string(), question_data);
        state.expand_thread("t1", 10);

        if let Some(qs) = &mut state.question_state {
            qs.set_current_selection(Some(0));
        }

        let result = state.build_question_answers();
        assert!(result.is_some());
        let (_, _, answers) = result.unwrap();

        // Key should be the full question text, not the header
        assert!(answers.contains_key("What is your preferred authentication method?"));
        assert!(!answers.contains_key("Auth"));
    }

    #[test]
    fn test_build_question_answers_uses_option_label_as_value() {
        use crate::state::session::{AskUserQuestionData, Question, QuestionOption};

        let mut state = DashboardState::new();
        let thread = make_thread("t1", "Test Thread");
        state.threads.insert("t1".to_string(), thread);

        // The option label (not description) should be used as the value
        let question_data = AskUserQuestionData {
            questions: vec![Question {
                question: "Select tool".to_string(),
                header: "Tool".to_string(),
                options: vec![QuestionOption {
                    label: "ESLint".to_string(),
                    description: "JavaScript linter with extensive rule set".to_string(),
                }],
                multi_select: false,
            }],
            answers: std::collections::HashMap::new(),
        };

        state.set_pending_question("t1", "req-value".to_string(), question_data);
        state.expand_thread("t1", 10);

        if let Some(qs) = &mut state.question_state {
            qs.set_current_selection(Some(0));
        }

        let result = state.build_question_answers();
        assert!(result.is_some());
        let (_, _, answers) = result.unwrap();

        // Value should be the option label, not description
        assert_eq!(answers.get("Select tool"), Some(&"ESLint".to_string()));
    }

    // -------------------- pending_permissions Tests --------------------

    #[test]
    fn test_set_pending_permission() {
        use std::time::Instant;

        let mut state = DashboardState::new();

        let request = PermissionRequest {
            permission_id: "perm-001".to_string(),
            thread_id: Some("t1".to_string()),
            tool_name: "Bash".to_string(),
            description: "Run npm install".to_string(),
            context: Some("/Users/test/project".to_string()),
            tool_input: None,
            received_at: Instant::now(),
        };

        state.set_pending_permission("t1", request.clone());

        let result = state.get_pending_permission("t1");
        assert!(result.is_some());
        let perm = result.unwrap();
        assert_eq!(perm.permission_id, "perm-001");
        assert_eq!(perm.tool_name, "Bash");
    }

    #[test]
    fn test_get_pending_permission_returns_none_for_unknown_thread() {
        let state = DashboardState::new();
        assert!(state.get_pending_permission("unknown").is_none());
    }

    #[test]
    fn test_clear_pending_permission() {
        use std::time::Instant;

        let mut state = DashboardState::new();

        let request = PermissionRequest {
            permission_id: "perm-002".to_string(),
            thread_id: Some("t1".to_string()),
            tool_name: "Read".to_string(),
            description: "Read file".to_string(),
            context: None,
            tool_input: None,
            received_at: Instant::now(),
        };

        state.set_pending_permission("t1", request);
        assert!(state.get_pending_permission("t1").is_some());

        state.clear_pending_permission("t1");
        assert!(state.get_pending_permission("t1").is_none());
    }

    #[test]
    fn test_pending_permission_replaces_existing() {
        use std::time::Instant;

        let mut state = DashboardState::new();

        let request1 = PermissionRequest {
            permission_id: "perm-001".to_string(),
            thread_id: Some("t1".to_string()),
            tool_name: "Bash".to_string(),
            description: "First command".to_string(),
            context: None,
            tool_input: None,
            received_at: Instant::now(),
        };

        let request2 = PermissionRequest {
            permission_id: "perm-002".to_string(),
            thread_id: Some("t1".to_string()),
            tool_name: "Write".to_string(),
            description: "Second command".to_string(),
            context: None,
            tool_input: None,
            received_at: Instant::now(),
        };

        state.set_pending_permission("t1", request1);
        state.set_pending_permission("t1", request2);

        let result = state.get_pending_permission("t1").unwrap();
        assert_eq!(result.permission_id, "perm-002");
        assert_eq!(result.tool_name, "Write");
    }

    #[test]
    fn test_pending_permissions_multiple_threads() {
        use std::time::Instant;

        let mut state = DashboardState::new();

        let request1 = PermissionRequest {
            permission_id: "perm-t1".to_string(),
            thread_id: Some("t1".to_string()),
            tool_name: "Bash".to_string(),
            description: "Command for t1".to_string(),
            context: None,
            tool_input: None,
            received_at: Instant::now(),
        };

        let request2 = PermissionRequest {
            permission_id: "perm-t2".to_string(),
            thread_id: Some("t2".to_string()),
            tool_name: "Read".to_string(),
            description: "Read for t2".to_string(),
            context: None,
            tool_input: None,
            received_at: Instant::now(),
        };

        state.set_pending_permission("t1", request1);
        state.set_pending_permission("t2", request2);

        let perm1 = state.get_pending_permission("t1").unwrap();
        let perm2 = state.get_pending_permission("t2").unwrap();

        assert_eq!(perm1.permission_id, "perm-t1");
        assert_eq!(perm2.permission_id, "perm-t2");
    }

    #[test]
    fn test_set_pending_permission_marks_views_dirty() {
        use std::time::Instant;

        let mut state = DashboardState::new();
        let thread = make_thread("t1", "Test Thread");
        state.threads.insert("t1".to_string(), thread);

        // Compute views to clear dirty flag
        let _ = state.compute_thread_views();
        assert!(!state.thread_views_dirty);

        // Set pending permission
        let request = PermissionRequest {
            permission_id: "perm-001".to_string(),
            thread_id: Some("t1".to_string()),
            tool_name: "Bash".to_string(),
            description: "Run command".to_string(),
            context: None,
            tool_input: None,
            received_at: Instant::now(),
        };

        state.set_pending_permission("t1", request);

        // Verify dirty flag is set
        assert!(state.thread_views_dirty);
    }

    #[test]
    fn test_clear_pending_permission_marks_views_dirty() {
        use std::time::Instant;

        let mut state = DashboardState::new();
        let thread = make_thread("t1", "Test Thread");
        state.threads.insert("t1".to_string(), thread);

        // Set permission first
        let request = PermissionRequest {
            permission_id: "perm-001".to_string(),
            thread_id: Some("t1".to_string()),
            tool_name: "Bash".to_string(),
            description: "Run command".to_string(),
            context: None,
            tool_input: None,
            received_at: Instant::now(),
        };
        state.set_pending_permission("t1", request);

        // Compute views to clear dirty flag
        let _ = state.compute_thread_views();
        assert!(!state.thread_views_dirty);

        // Clear pending permission
        state.clear_pending_permission("t1");

        // Verify dirty flag is set
        assert!(state.thread_views_dirty);
    }

    #[test]
    fn test_clear_nonexistent_pending_permission_marks_views_dirty() {
        let mut state = DashboardState::new();
        let thread = make_thread("t1", "Test Thread");
        state.threads.insert("t1".to_string(), thread);

        // Compute views to clear dirty flag
        let _ = state.compute_thread_views();
        assert!(!state.thread_views_dirty);

        // Clear a non-existent permission (should still mark dirty for consistency)
        state.clear_pending_permission("t1");

        // Verify dirty flag is set
        assert!(state.thread_views_dirty);
    }

    // -------------------- update_thread_status clears pending_permissions Tests --------------------

    #[test]
    fn test_update_thread_status_clears_pending_permission_on_done() {
        use std::time::Instant;

        let mut state = DashboardState::new();
        let thread = make_thread("t1", "Test Thread");
        state.threads.insert("t1".to_string(), thread);

        // Set up pending permission
        let request = PermissionRequest {
            permission_id: "perm-001".to_string(),
            thread_id: Some("t1".to_string()),
            tool_name: "Bash".to_string(),
            description: "Run command".to_string(),
            context: None,
            tool_input: None,
            received_at: Instant::now(),
        };
        state.set_pending_permission("t1", request);

        // Set waiting_for to Permission
        state.waiting_for.insert(
            "t1".to_string(),
            WaitingFor::Permission {
                request_id: "perm-001".to_string(),
                tool_name: "Bash".to_string(),
            },
        );

        assert!(state.get_pending_permission("t1").is_some());

        // Update status to Done
        state.update_thread_status("t1", ThreadStatus::Done, None);

        // Pending permission should be cleared
        assert!(state.get_pending_permission("t1").is_none());
    }

    #[test]
    fn test_update_thread_status_clears_pending_permission_on_error() {
        use std::time::Instant;

        let mut state = DashboardState::new();
        let thread = make_thread("t1", "Test Thread");
        state.threads.insert("t1".to_string(), thread);

        // Set up pending permission
        let request = PermissionRequest {
            permission_id: "perm-002".to_string(),
            thread_id: Some("t1".to_string()),
            tool_name: "Write".to_string(),
            description: "Write file".to_string(),
            context: None,
            tool_input: None,
            received_at: Instant::now(),
        };
        state.set_pending_permission("t1", request);

        state.waiting_for.insert(
            "t1".to_string(),
            WaitingFor::Permission {
                request_id: "perm-002".to_string(),
                tool_name: "Write".to_string(),
            },
        );

        assert!(state.get_pending_permission("t1").is_some());

        // Update status to Error
        state.update_thread_status("t1", ThreadStatus::Error, None);

        // Pending permission should be cleared
        assert!(state.get_pending_permission("t1").is_none());
    }

    #[test]
    fn test_update_thread_status_clears_pending_permission_on_idle() {
        use std::time::Instant;

        let mut state = DashboardState::new();
        let thread = make_thread("t1", "Test Thread");
        state.threads.insert("t1".to_string(), thread);

        // Set up pending permission
        let request = PermissionRequest {
            permission_id: "perm-003".to_string(),
            thread_id: Some("t1".to_string()),
            tool_name: "Read".to_string(),
            description: "Read file".to_string(),
            context: None,
            tool_input: None,
            received_at: Instant::now(),
        };
        state.set_pending_permission("t1", request);

        state.waiting_for.insert(
            "t1".to_string(),
            WaitingFor::Permission {
                request_id: "perm-003".to_string(),
                tool_name: "Read".to_string(),
            },
        );

        assert!(state.get_pending_permission("t1").is_some());

        // Update status to Done (no longer waiting)
        state.update_thread_status("t1", ThreadStatus::Done, None);

        // Pending permission should be cleared
        assert!(state.get_pending_permission("t1").is_none());
    }

    #[test]
    fn test_update_thread_status_clears_pending_permission_when_no_longer_waiting_for_permission() {
        use std::time::Instant;

        let mut state = DashboardState::new();
        let thread = make_thread("t1", "Test Thread");
        state.threads.insert("t1".to_string(), thread);

        // Set up pending permission
        let request = PermissionRequest {
            permission_id: "perm-004".to_string(),
            thread_id: Some("t1".to_string()),
            tool_name: "Bash".to_string(),
            description: "Run command".to_string(),
            context: None,
            tool_input: None,
            received_at: Instant::now(),
        };
        state.set_pending_permission("t1", request);

        // Set waiting_for to Permission
        state.waiting_for.insert(
            "t1".to_string(),
            WaitingFor::Permission {
                request_id: "perm-004".to_string(),
                tool_name: "Bash".to_string(),
            },
        );

        assert!(state.get_pending_permission("t1").is_some());

        // Update status to Running with no waiting_for (permission was approved)
        state.update_thread_status("t1", ThreadStatus::Running, None);

        // Pending permission should be cleared
        assert!(state.get_pending_permission("t1").is_none());
    }

    #[test]
    fn test_update_thread_status_keeps_pending_permission_when_still_waiting() {
        use std::time::Instant;

        let mut state = DashboardState::new();
        let thread = make_thread("t1", "Test Thread");
        state.threads.insert("t1".to_string(), thread);

        // Set up pending permission
        let request = PermissionRequest {
            permission_id: "perm-005".to_string(),
            thread_id: Some("t1".to_string()),
            tool_name: "Bash".to_string(),
            description: "Run command".to_string(),
            context: None,
            tool_input: None,
            received_at: Instant::now(),
        };
        state.set_pending_permission("t1", request);

        // Update status to Waiting with Permission waiting_for (permission still pending)
        state.update_thread_status(
            "t1",
            ThreadStatus::Waiting,
            Some(WaitingFor::Permission {
                request_id: "perm-005".to_string(),
                tool_name: "Bash".to_string(),
            }),
        );

        // Pending permission should NOT be cleared
        assert!(state.get_pending_permission("t1").is_some());
    }

    // -------------------- needs_action with pending_permissions Tests --------------------

    #[test]
    fn test_build_thread_views_sets_needs_action_for_pending_permission() {
        use std::time::Instant;

        let mut state = DashboardState::new();
        let mut thread = make_thread("t1", "Test Thread");
        thread.status = Some(ThreadStatus::Running);
        state.threads.insert("t1".to_string(), thread);

        // Set up pending permission (without waiting_for set yet)
        let request = PermissionRequest {
            permission_id: "perm-006".to_string(),
            thread_id: Some("t1".to_string()),
            tool_name: "Bash".to_string(),
            description: "Run command".to_string(),
            context: None,
            tool_input: None,
            received_at: Instant::now(),
        };
        state.set_pending_permission("t1", request);

        let views = state.compute_thread_views();

        assert_eq!(views.len(), 1);
        // Thread should have needs_action = true due to pending permission
        assert!(views[0].needs_action);
    }

    #[test]
    fn test_build_thread_views_needs_action_false_without_pending_permission() {
        let mut state = DashboardState::new();
        let mut thread = make_thread("t1", "Test Thread");
        thread.status = Some(ThreadStatus::Running);
        state.threads.insert("t1".to_string(), thread);

        // No pending permission

        let views = state.compute_thread_views();

        assert_eq!(views.len(), 1);
        // Thread should have needs_action = false (Running status doesn't need attention)
        assert!(!views[0].needs_action);
    }

    #[test]
    fn test_build_thread_views_sorts_pending_permission_threads_first() {
        use std::time::Instant;

        let mut state = DashboardState::new();

        // Thread 1: Running, no pending permission
        let mut t1 = make_thread("t1", "Thread 1");
        t1.status = Some(ThreadStatus::Running);
        t1.updated_at = Utc::now();
        state.threads.insert("t1".to_string(), t1);

        // Thread 2: Running, has pending permission
        let mut t2 = make_thread("t2", "Thread 2");
        t2.status = Some(ThreadStatus::Running);
        t2.updated_at = Utc::now() - chrono::Duration::hours(1); // Older thread
        state.threads.insert("t2".to_string(), t2);

        // Set pending permission for t2
        let request = PermissionRequest {
            permission_id: "perm-007".to_string(),
            thread_id: Some("t2".to_string()),
            tool_name: "Bash".to_string(),
            description: "Run command".to_string(),
            context: None,
            tool_input: None,
            received_at: Instant::now(),
        };
        state.set_pending_permission("t2", request);

        let views = state.compute_thread_views();

        assert_eq!(views.len(), 2);
        // t2 should come first because it has needs_action = true
        assert_eq!(views[0].id, "t2");
        assert!(views[0].needs_action);
        // t1 should come second
        assert_eq!(views[1].id, "t1");
        assert!(!views[1].needs_action);
    }

    // ============================================================================
    // Top-Card Input Routing Tests (Phase 4 - Bug Fix)
    // ============================================================================
    //
    // These tests verify the fix for Y/N/A key capture when the top card is NOT
    // a permission prompt. The bug was that Y/N/A keys were being captured even
    // when the top card was an AskUserQuestion dialog.
    //
    // The fix routes input based on the TOP card's WaitingFor type:
    // - Permission -> Y/N/A captured
    // - UserInput -> Y/N/A should flow to text input
    // - PlanApproval -> Y/N/A captured

    #[test]
    fn test_get_top_needs_action_thread_user_input_is_top_when_most_recent() {
        // Setup: Thread 1 has Permission, Thread 2 has UserInput (more recent)
        // Expected: UserInput thread should be returned as top
        let mut state = DashboardState::new();

        // Thread 1: Permission, older
        let mut t1 = make_thread("t1", "Permission Thread");
        t1.status = Some(ThreadStatus::Waiting);
        t1.updated_at = Utc::now() - chrono::Duration::seconds(10);
        state.threads.insert("t1".to_string(), t1);

        // Thread 2: UserInput, newer
        let mut t2 = make_thread("t2", "UserInput Thread");
        t2.status = Some(ThreadStatus::Waiting);
        t2.updated_at = Utc::now();
        state.threads.insert("t2".to_string(), t2);

        state.waiting_for.insert(
            "t1".to_string(),
            WaitingFor::Permission {
                request_id: "perm-1".to_string(),
                tool_name: "Bash".to_string(),
            },
        );
        state.waiting_for.insert("t2".to_string(), WaitingFor::UserInput);

        state.thread_views_dirty = true;
        let _ = state.compute_thread_views();

        let result = state.get_top_needs_action_thread();
        assert!(result.is_some());
        let (thread_id, waiting_for) = result.unwrap();

        // UserInput thread (t2) is more recent, so it should be top
        assert_eq!(thread_id, "t2");
        assert!(matches!(waiting_for, WaitingFor::UserInput));
    }

    #[test]
    fn test_get_top_needs_action_thread_permission_is_top_when_most_recent() {
        // Setup: Thread 1 has UserInput (older), Thread 2 has Permission (more recent)
        // Expected: Permission thread should be returned as top
        let mut state = DashboardState::new();

        // Thread 1: UserInput, older
        let mut t1 = make_thread("t1", "UserInput Thread");
        t1.status = Some(ThreadStatus::Waiting);
        t1.updated_at = Utc::now() - chrono::Duration::seconds(10);
        state.threads.insert("t1".to_string(), t1);

        // Thread 2: Permission, newer
        let mut t2 = make_thread("t2", "Permission Thread");
        t2.status = Some(ThreadStatus::Waiting);
        t2.updated_at = Utc::now();
        state.threads.insert("t2".to_string(), t2);

        state.waiting_for.insert("t1".to_string(), WaitingFor::UserInput);
        state.waiting_for.insert(
            "t2".to_string(),
            WaitingFor::Permission {
                request_id: "perm-2".to_string(),
                tool_name: "Edit".to_string(),
            },
        );

        state.thread_views_dirty = true;
        let _ = state.compute_thread_views();

        let result = state.get_top_needs_action_thread();
        assert!(result.is_some());
        let (thread_id, waiting_for) = result.unwrap();

        // Permission thread (t2) is more recent, so it should be top
        assert_eq!(thread_id, "t2");
        assert!(matches!(waiting_for, WaitingFor::Permission { .. }));
    }

    #[test]
    fn test_get_top_pending_permission_returns_none_when_top_is_user_input() {
        // Setup: Top thread is UserInput, second thread has Permission
        // Expected: get_top_pending_permission should return None
        use std::time::Instant;

        let mut state = DashboardState::new();

        // Thread 1: Permission, older
        let mut t1 = make_thread("t1", "Permission Thread");
        t1.status = Some(ThreadStatus::Waiting);
        t1.updated_at = Utc::now() - chrono::Duration::seconds(10);
        state.threads.insert("t1".to_string(), t1);

        // Thread 2: UserInput, newer (TOP)
        let mut t2 = make_thread("t2", "UserInput Thread");
        t2.status = Some(ThreadStatus::Waiting);
        t2.updated_at = Utc::now();
        state.threads.insert("t2".to_string(), t2);

        // Set waiting_for states
        state.waiting_for.insert(
            "t1".to_string(),
            WaitingFor::Permission {
                request_id: "perm-1".to_string(),
                tool_name: "Bash".to_string(),
            },
        );
        state.waiting_for.insert("t2".to_string(), WaitingFor::UserInput);

        // Set pending permission for t1
        let request = PermissionRequest {
            permission_id: "perm-1".to_string(),
            thread_id: Some("t1".to_string()),
            tool_name: "Bash".to_string(),
            description: "Run command".to_string(),
            context: None,
            tool_input: None,
            received_at: Instant::now(),
        };
        state.set_pending_permission("t1", request);

        state.thread_views_dirty = true;
        let _ = state.compute_thread_views();

        // Verify t2 (UserInput) is top
        let top = state.get_top_needs_action_thread();
        assert_eq!(top.as_ref().map(|(id, _)| id.as_str()), Some("t2"));
        assert!(matches!(top.as_ref().map(|(_, wf)| wf), Some(WaitingFor::UserInput)));

        // get_top_pending_permission should return None because top is UserInput
        let top_permission = state.get_top_pending_permission();
        assert!(top_permission.is_none());
    }

    #[test]
    fn test_get_top_pending_permission_returns_permission_when_top_is_permission() {
        // Setup: Top thread has Permission
        // Expected: get_top_pending_permission should return the permission
        use std::time::Instant;

        let mut state = DashboardState::new();

        // Thread 1: UserInput, older
        let mut t1 = make_thread("t1", "UserInput Thread");
        t1.status = Some(ThreadStatus::Waiting);
        t1.updated_at = Utc::now() - chrono::Duration::seconds(10);
        state.threads.insert("t1".to_string(), t1);

        // Thread 2: Permission, newer (TOP)
        let mut t2 = make_thread("t2", "Permission Thread");
        t2.status = Some(ThreadStatus::Waiting);
        t2.updated_at = Utc::now();
        state.threads.insert("t2".to_string(), t2);

        // Set waiting_for states
        state.waiting_for.insert("t1".to_string(), WaitingFor::UserInput);
        state.waiting_for.insert(
            "t2".to_string(),
            WaitingFor::Permission {
                request_id: "perm-top".to_string(),
                tool_name: "Edit".to_string(),
            },
        );

        // Set pending permission for t2 (the top thread)
        let request = PermissionRequest {
            permission_id: "perm-top".to_string(),
            thread_id: Some("t2".to_string()),
            tool_name: "Edit".to_string(),
            description: "Edit file".to_string(),
            context: None,
            tool_input: None,
            received_at: Instant::now(),
        };
        state.set_pending_permission("t2", request);

        state.thread_views_dirty = true;
        let _ = state.compute_thread_views();

        // Verify t2 (Permission) is top
        let top = state.get_top_needs_action_thread();
        assert_eq!(top.as_ref().map(|(id, _)| id.as_str()), Some("t2"));
        assert!(matches!(top.as_ref().map(|(_, wf)| wf), Some(WaitingFor::Permission { .. })));

        // get_top_pending_permission should return the permission for t2
        let top_permission = state.get_top_pending_permission();
        assert!(top_permission.is_some());
        assert_eq!(top_permission.unwrap().permission_id, "perm-top");
        assert_eq!(top_permission.unwrap().tool_name, "Edit");
    }

    #[test]
    fn test_thread_removal_changes_top_needs_action() {
        // Test input isolation: when top card is removed, next card becomes active
        use std::time::Instant;

        let mut state = DashboardState::new();

        // Thread 1: Permission, older
        let mut t1 = make_thread("t1", "Permission Thread");
        t1.status = Some(ThreadStatus::Waiting);
        t1.updated_at = Utc::now() - chrono::Duration::seconds(10);
        state.threads.insert("t1".to_string(), t1);

        // Thread 2: UserInput, newer (TOP initially)
        let mut t2 = make_thread("t2", "UserInput Thread");
        t2.status = Some(ThreadStatus::Waiting);
        t2.updated_at = Utc::now();
        state.threads.insert("t2".to_string(), t2);

        state.waiting_for.insert(
            "t1".to_string(),
            WaitingFor::Permission {
                request_id: "perm-1".to_string(),
                tool_name: "Bash".to_string(),
            },
        );
        state.waiting_for.insert("t2".to_string(), WaitingFor::UserInput);

        // Set pending permission for t1
        let request = PermissionRequest {
            permission_id: "perm-1".to_string(),
            thread_id: Some("t1".to_string()),
            tool_name: "Bash".to_string(),
            description: "Run command".to_string(),
            context: None,
            tool_input: None,
            received_at: Instant::now(),
        };
        state.set_pending_permission("t1", request);

        state.thread_views_dirty = true;
        let _ = state.compute_thread_views();

        // Initially, t2 (UserInput) should be top
        let top = state.get_top_needs_action_thread();
        assert_eq!(top.as_ref().map(|(id, _)| id.as_str()), Some("t2"));

        // Simulate removing the top thread by clearing its waiting state
        state.clear_waiting_for("t2");
        state.thread_views_dirty = true;
        let _ = state.compute_thread_views();

        // Now t1 (Permission) should become top
        let new_top = state.get_top_needs_action_thread();
        assert!(new_top.is_some());
        let (new_thread_id, new_waiting_for) = new_top.unwrap();
        assert_eq!(new_thread_id, "t1");
        assert!(matches!(new_waiting_for, WaitingFor::Permission { .. }));

        // And get_top_pending_permission should now return the permission
        let top_permission = state.get_top_pending_permission();
        assert!(top_permission.is_some());
        assert_eq!(top_permission.unwrap().permission_id, "perm-1");
    }

    #[test]
    fn test_stacked_cards_isolation_three_threads() {
        // Test with 3 stacked cards to verify isolation between all types
        use std::time::Instant;

        let mut state = DashboardState::new();

        // Thread 1: Permission, oldest
        let mut t1 = make_thread("t1", "Permission Thread");
        t1.status = Some(ThreadStatus::Waiting);
        t1.updated_at = Utc::now() - chrono::Duration::seconds(20);
        state.threads.insert("t1".to_string(), t1);

        // Thread 2: PlanApproval, middle
        let mut t2 = make_thread("t2", "Plan Thread");
        t2.status = Some(ThreadStatus::Waiting);
        t2.updated_at = Utc::now() - chrono::Duration::seconds(10);
        state.threads.insert("t2".to_string(), t2);

        // Thread 3: UserInput, newest (TOP)
        let mut t3 = make_thread("t3", "UserInput Thread");
        t3.status = Some(ThreadStatus::Waiting);
        t3.updated_at = Utc::now();
        state.threads.insert("t3".to_string(), t3);

        state.waiting_for.insert(
            "t1".to_string(),
            WaitingFor::Permission {
                request_id: "perm-1".to_string(),
                tool_name: "Bash".to_string(),
            },
        );
        state.waiting_for.insert(
            "t2".to_string(),
            WaitingFor::PlanApproval {
                request_id: "plan-1".to_string(),
            },
        );
        state.waiting_for.insert("t3".to_string(), WaitingFor::UserInput);

        // Set pending permission for t1
        let request = PermissionRequest {
            permission_id: "perm-1".to_string(),
            thread_id: Some("t1".to_string()),
            tool_name: "Bash".to_string(),
            description: "Run command".to_string(),
            context: None,
            tool_input: None,
            received_at: Instant::now(),
        };
        state.set_pending_permission("t1", request);

        state.thread_views_dirty = true;
        let _ = state.compute_thread_views();

        // Step 1: Top should be t3 (UserInput)
        let top = state.get_top_needs_action_thread();
        assert_eq!(top.as_ref().map(|(id, _)| id.as_str()), Some("t3"));
        assert!(state.get_top_pending_permission().is_none());

        // Step 2: Remove t3 from needs_action
        state.clear_waiting_for("t3");
        state.thread_views_dirty = true;
        let _ = state.compute_thread_views();

        // Now top should be t2 (PlanApproval)
        let top = state.get_top_needs_action_thread();
        assert_eq!(top.as_ref().map(|(id, _)| id.as_str()), Some("t2"));
        assert!(matches!(top.as_ref().map(|(_, wf)| wf), Some(WaitingFor::PlanApproval { .. })));
        assert!(state.get_top_pending_permission().is_none()); // PlanApproval doesn't have permission

        // Step 3: Remove t2 from needs_action
        state.clear_waiting_for("t2");
        state.thread_views_dirty = true;
        let _ = state.compute_thread_views();

        // Now top should be t1 (Permission)
        let top = state.get_top_needs_action_thread();
        assert_eq!(top.as_ref().map(|(id, _)| id.as_str()), Some("t1"));
        assert!(matches!(top.as_ref().map(|(_, wf)| wf), Some(WaitingFor::Permission { .. })));

        // Now get_top_pending_permission should return the permission
        let perm = state.get_top_pending_permission();
        assert!(perm.is_some());
        assert_eq!(perm.unwrap().permission_id, "perm-1");
    }

    #[test]
    fn test_ask_user_question_permission_tool_is_user_input_type() {
        // Verify that AskUserQuestion tool (which is a Permission in WaitingFor)
        // has the correct routing behavior
        use std::time::Instant;

        let mut state = DashboardState::new();

        // Thread with AskUserQuestion permission
        let mut t1 = make_thread("t1", "AskUserQuestion Thread");
        t1.status = Some(ThreadStatus::Waiting);
        t1.updated_at = Utc::now();
        state.threads.insert("t1".to_string(), t1);

        // AskUserQuestion shows as Permission in WaitingFor but with special tool_name
        state.waiting_for.insert(
            "t1".to_string(),
            WaitingFor::Permission {
                request_id: "ask-1".to_string(),
                tool_name: "AskUserQuestion".to_string(),
            },
        );

        // Set pending permission for AskUserQuestion
        let request = PermissionRequest {
            permission_id: "ask-1".to_string(),
            thread_id: Some("t1".to_string()),
            tool_name: "AskUserQuestion".to_string(),
            description: "Answer a question".to_string(),
            context: None,
            tool_input: Some(serde_json::json!({
                "questions": [
                    {
                        "question": "What color?",
                        "header": "Color",
                        "options": [
                            {"label": "Red", "description": "Color red"},
                            {"label": "Blue", "description": "Color blue"}
                        ],
                        "multiSelect": false
                    }
                ],
                "answers": {}
            })),
            received_at: Instant::now(),
        };
        state.set_pending_permission("t1", request);

        state.thread_views_dirty = true;
        let _ = state.compute_thread_views();

        // get_top_needs_action_thread should return the thread with Permission type
        let top = state.get_top_needs_action_thread();
        assert!(top.is_some());
        let (_, waiting_for) = top.unwrap();

        // The WaitingFor type is Permission, but tool_name tells us it's AskUserQuestion
        if let WaitingFor::Permission { tool_name, .. } = waiting_for {
            assert_eq!(tool_name, "AskUserQuestion");
        } else {
            panic!("Expected Permission variant");
        }

        // get_top_pending_permission returns the permission since it IS a Permission type
        // The main.rs input handler checks tool_name to route AskUserQuestion specially
        let perm = state.get_top_pending_permission();
        assert!(perm.is_some());
        assert_eq!(perm.unwrap().tool_name, "AskUserQuestion");
    }

    #[test]
    fn test_needs_action_flag_set_for_waiting_threads() {
        // Verify needs_action flag is correctly set based on waiting state
        let mut state = DashboardState::new();

        // Thread 1: Waiting for Permission
        let mut t1 = make_thread("t1", "Thread 1");
        t1.status = Some(ThreadStatus::Waiting);
        t1.updated_at = Utc::now();
        state.threads.insert("t1".to_string(), t1);

        state.waiting_for.insert(
            "t1".to_string(),
            WaitingFor::Permission {
                request_id: "perm-1".to_string(),
                tool_name: "Bash".to_string(),
            },
        );

        state.thread_views_dirty = true;
        let views = state.compute_thread_views();

        assert_eq!(views.len(), 1);
        // Waiting thread should have needs_action set
        assert!(views[0].needs_action);
        assert_eq!(views[0].waiting_for, Some(WaitingFor::Permission {
            request_id: "perm-1".to_string(),
            tool_name: "Bash".to_string(),
        }));
    }

    #[test]
    fn test_needs_action_flag_set_for_pending_permission_threads() {
        // Verify needs_action is set when thread has pending permission
        // (even if not in Waiting status)
        use std::time::Instant;

        let mut state = DashboardState::new();

        // Thread 1: Running status but has pending permission
        let mut t1 = make_thread("t1", "Thread 1");
        t1.status = Some(ThreadStatus::Running);
        t1.updated_at = Utc::now();
        state.threads.insert("t1".to_string(), t1);

        // Set pending permission (but not waiting_for)
        let request = PermissionRequest {
            permission_id: "perm-1".to_string(),
            thread_id: Some("t1".to_string()),
            tool_name: "Read".to_string(),
            description: "Read file".to_string(),
            context: None,
            tool_input: None,
            received_at: Instant::now(),
        };
        state.set_pending_permission("t1", request);

        state.thread_views_dirty = true;
        let views = state.compute_thread_views();

        assert_eq!(views.len(), 1);
        // Thread with pending permission should have needs_action set
        assert!(views[0].needs_action);
    }

    #[test]
    fn test_get_top_pending_permission_empty_state() {
        // Edge case: no threads at all
        let state = DashboardState::new();
        assert!(state.get_top_pending_permission().is_none());
    }

    #[test]
    fn test_get_top_pending_permission_no_permissions() {
        // Edge case: threads exist but no permissions
        let mut state = DashboardState::new();

        let mut t1 = make_thread("t1", "Thread 1");
        t1.status = Some(ThreadStatus::Waiting);
        state.threads.insert("t1".to_string(), t1);
        state.waiting_for.insert("t1".to_string(), WaitingFor::UserInput);

        state.thread_views_dirty = true;
        let _ = state.compute_thread_views();

        // Top thread is UserInput, no permissions set
        assert!(state.get_top_pending_permission().is_none());
    }
}
