//! Render context for dashboard components
//!
//! This module provides the data structures needed to render the multi-thread
//! dashboard view, including pre-computed thread views, overlay states, and
//! system statistics.

use crate::models::dashboard::{Aggregate, PlanSummary, ThreadStatus, WaitingFor};
use crate::ui::theme;

// Re-export ThreadMode from models for convenience
pub use crate::models::ThreadMode;

// ============================================================================
// Theme (lightweight reference for dashboard rendering)
// ============================================================================

/// Theme colors for dashboard rendering
///
/// Uses references to existing theme constants, keeping the struct lightweight
#[derive(Debug, Clone)]
pub struct Theme {
    /// Color for running/active threads
    pub active: ratatui::style::Color,
    /// Color for completed/successful threads
    pub success: ratatui::style::Color,
    /// Color for error states
    pub error: ratatui::style::Color,
    /// Color for waiting/pending states
    pub waiting: ratatui::style::Color,
    /// Color for idle/dimmed elements
    pub dim: ratatui::style::Color,
    /// Color for borders
    pub border: ratatui::style::Color,
    /// Color for accents/highlights
    pub accent: ratatui::style::Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            active: theme::COLOR_ACTIVE,
            success: theme::COLOR_TOOL_SUCCESS,
            error: theme::COLOR_TOOL_ERROR,
            waiting: ratatui::style::Color::Yellow,
            dim: theme::COLOR_DIM,
            border: theme::COLOR_BORDER,
            accent: theme::COLOR_ACCENT,
        }
    }
}

// ============================================================================
// Filter State
// ============================================================================

/// Filter options for thread display
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilterState {
    /// Show all threads (no filter)
    #[default]
    All,
    /// Show only working threads (Running + Waiting)
    Working,
    /// Show only threads ready for testing (Done)
    ReadyToTest,
    /// Show only idle threads (Idle + Error)
    Idle,
}

impl FilterState {
    /// Check if a thread status matches this filter
    pub fn matches(&self, status: ThreadStatus) -> bool {
        match self {
            FilterState::All => true,
            FilterState::Working => matches!(status, ThreadStatus::Running | ThreadStatus::Waiting),
            FilterState::ReadyToTest => matches!(status, ThreadStatus::Done),
            FilterState::Idle => matches!(status, ThreadStatus::Idle | ThreadStatus::Error),
        }
    }

    /// Get the display name for this filter
    pub fn display_name(&self) -> &'static str {
        match self {
            FilterState::All => "All",
            FilterState::Working => "Working",
            FilterState::ReadyToTest => "Ready to Test",
            FilterState::Idle => "Idle",
        }
    }

    /// Cycle to the next filter state
    pub fn next(&self) -> Self {
        match self {
            FilterState::All => FilterState::Working,
            FilterState::Working => FilterState::ReadyToTest,
            FilterState::ReadyToTest => FilterState::Idle,
            FilterState::Idle => FilterState::All,
        }
    }

    /// Cycle to the previous filter state
    pub fn prev(&self) -> Self {
        match self {
            FilterState::All => FilterState::Idle,
            FilterState::Working => FilterState::All,
            FilterState::ReadyToTest => FilterState::Working,
            FilterState::Idle => FilterState::ReadyToTest,
        }
    }
}

// ============================================================================
// Progress
// ============================================================================

/// Progress tracking for thread operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    /// Current step/phase
    pub current: u32,
    /// Total steps/phases
    pub total: u32,
}

impl Progress {
    /// Create new progress tracker
    pub fn new(current: u32, total: u32) -> Self {
        Self { current, total }
    }

    /// Get progress as percentage (0-100)
    pub fn percentage(&self) -> u8 {
        if self.total == 0 {
            0
        } else {
            ((self.current as f64 / self.total as f64) * 100.0) as u8
        }
    }

    /// Get progress as fraction string (e.g., "3/5")
    pub fn as_fraction(&self) -> String {
        format!("{}/{}", self.current, self.total)
    }

    /// Check if progress is complete
    pub fn is_complete(&self) -> bool {
        self.current >= self.total && self.total > 0
    }
}

// ============================================================================
// Thread View
// ============================================================================

/// Pre-computed view data for a single thread
///
/// This struct contains all the data needed to render a thread card,
/// computed once and reused to avoid repeated calculations during rendering.
#[derive(Debug, Clone)]
pub struct ThreadView {
    /// Thread ID
    pub id: String,
    /// Thread title/name
    pub title: String,
    /// Repository display name (shortened path)
    pub repository: String,
    /// Thread operation mode
    pub mode: ThreadMode,
    /// Current status
    pub status: ThreadStatus,
    /// What the thread is waiting for (if any)
    pub waiting_for: Option<WaitingFor>,
    /// Progress through current operation
    pub progress: Option<Progress>,
    /// Duration since last activity
    pub duration: String,
    /// Whether this thread needs user action
    pub needs_action: bool,
}

impl ThreadView {
    /// Create a new thread view with minimal required fields
    pub fn new(id: String, title: String, repository: String) -> Self {
        Self {
            id,
            title,
            repository,
            mode: ThreadMode::Normal,
            status: ThreadStatus::Idle,
            waiting_for: None,
            progress: None,
            duration: String::new(),
            needs_action: false,
        }
    }

    /// Builder-style setter for mode
    pub fn with_mode(mut self, mode: ThreadMode) -> Self {
        self.mode = mode;
        self
    }

    /// Builder-style setter for status
    pub fn with_status(mut self, status: ThreadStatus) -> Self {
        self.status = status;
        self.needs_action = status.needs_attention();
        self
    }

    /// Builder-style setter for waiting_for
    pub fn with_waiting_for(mut self, waiting_for: Option<WaitingFor>) -> Self {
        self.waiting_for = waiting_for;
        if self.waiting_for.is_some() {
            self.needs_action = true;
        }
        self
    }

    /// Builder-style setter for progress
    pub fn with_progress(mut self, progress: Option<Progress>) -> Self {
        self.progress = progress;
        self
    }

    /// Builder-style setter for duration
    pub fn with_duration(mut self, duration: String) -> Self {
        self.duration = duration;
        self
    }

    /// Get the status line for display
    pub fn status_line(&self) -> String {
        if let Some(ref waiting) = self.waiting_for {
            waiting.description()
        } else {
            match self.status {
                ThreadStatus::Idle => "Idle".to_string(),
                ThreadStatus::Running => "Running".to_string(),
                ThreadStatus::Waiting => "Waiting".to_string(),
                ThreadStatus::Done => "Done".to_string(),
                ThreadStatus::Error => "Error".to_string(),
            }
        }
    }
}

// ============================================================================
// Overlay State
// ============================================================================

/// State for overlay dialogs (permission requests, questions, plan approvals)
#[derive(Debug, Clone)]
pub enum OverlayState {
    /// Multiple choice question
    Question {
        thread_id: String,
        thread_title: String,
        repository: String,
        question: String,
        options: Vec<String>,
        /// Y position to anchor the overlay
        anchor_y: u16,
    },
    /// Free-form text input
    FreeForm {
        thread_id: String,
        thread_title: String,
        repository: String,
        question: String,
        input: String,
        cursor_pos: usize,
        anchor_y: u16,
    },
    /// Plan approval dialog
    Plan {
        thread_id: String,
        thread_title: String,
        repository: String,
        request_id: String,
        summary: PlanSummary,
        scroll_offset: usize,
        anchor_y: u16,
    },
}

impl OverlayState {
    /// Get the thread ID associated with this overlay
    pub fn thread_id(&self) -> &str {
        match self {
            OverlayState::Question { thread_id, .. } => thread_id,
            OverlayState::FreeForm { thread_id, .. } => thread_id,
            OverlayState::Plan { thread_id, .. } => thread_id,
        }
    }

    /// Get the thread title associated with this overlay
    pub fn thread_title(&self) -> &str {
        match self {
            OverlayState::Question { thread_title, .. } => thread_title,
            OverlayState::FreeForm { thread_title, .. } => thread_title,
            OverlayState::Plan { thread_title, .. } => thread_title,
        }
    }

    /// Get the repository associated with this overlay
    pub fn repository(&self) -> &str {
        match self {
            OverlayState::Question { repository, .. } => repository,
            OverlayState::FreeForm { repository, .. } => repository,
            OverlayState::Plan { repository, .. } => repository,
        }
    }

    /// Check if this is a plan approval overlay
    pub fn is_plan(&self) -> bool {
        matches!(self, OverlayState::Plan { .. })
    }
}

// ============================================================================
// System Stats
// ============================================================================

/// System statistics for dashboard header
#[derive(Debug, Clone, Default)]
pub struct SystemStats {
    /// WebSocket connection status
    pub connected: bool,
    /// CPU usage percentage (0-100)
    pub cpu_percent: f32,
    /// RAM used in GB
    pub ram_used_gb: f32,
    /// Total RAM in GB
    pub ram_total_gb: f32,
}

impl SystemStats {
    /// Create new system stats
    pub fn new(connected: bool, cpu_percent: f32, ram_used_gb: f32, ram_total_gb: f32) -> Self {
        Self {
            connected,
            cpu_percent,
            ram_used_gb,
            ram_total_gb,
        }
    }

    /// Check if system is under heavy load
    ///
    /// Returns true if CPU > 90% OR RAM usage > 90%
    pub fn is_heavy_load(&self) -> bool {
        self.cpu_percent > 90.0 || self.ram_percent() > 90.0
    }

    /// Get RAM usage as percentage
    pub fn ram_percent(&self) -> f32 {
        if self.ram_total_gb > 0.0 {
            (self.ram_used_gb / self.ram_total_gb) * 100.0
        } else {
            0.0
        }
    }

    /// Get formatted RAM string (e.g., "8.2/16.0 GB")
    pub fn ram_display(&self) -> String {
        format!("{:.1}/{:.1} GB", self.ram_used_gb, self.ram_total_gb)
    }

    /// Get formatted CPU string (e.g., "45%")
    pub fn cpu_display(&self) -> String {
        format!("{:.0}%", self.cpu_percent)
    }

    /// Get connection status display
    pub fn connection_display(&self) -> &'static str {
        if self.connected {
            "Connected"
        } else {
            "Disconnected"
        }
    }
}

// ============================================================================
// Render Context
// ============================================================================

/// Complete render context for the dashboard view
///
/// This struct aggregates all the data needed to render the dashboard,
/// using references to avoid unnecessary cloning.
#[derive(Debug)]
pub struct RenderContext<'a> {
    /// Pre-computed thread views
    pub threads: &'a [ThreadView],
    /// Aggregate statistics
    pub aggregate: &'a Aggregate,
    /// Current filter state
    pub filter: Option<FilterState>,
    /// Active overlay (if any)
    pub overlay: Option<&'a OverlayState>,
    /// System statistics
    pub system_stats: &'a SystemStats,
    /// Theme colors
    pub theme: &'a Theme,
}

impl<'a> RenderContext<'a> {
    /// Create a new render context
    pub fn new(
        threads: &'a [ThreadView],
        aggregate: &'a Aggregate,
        system_stats: &'a SystemStats,
        theme: &'a Theme,
    ) -> Self {
        Self {
            threads,
            aggregate,
            filter: None,
            overlay: None,
            system_stats,
            theme,
        }
    }

    /// Set filter state
    pub fn with_filter(mut self, filter: Option<FilterState>) -> Self {
        self.filter = filter;
        self
    }

    /// Set overlay state
    pub fn with_overlay(mut self, overlay: Option<&'a OverlayState>) -> Self {
        self.overlay = overlay;
        self
    }

    /// Get filtered threads based on current filter
    pub fn filtered_threads(&self) -> Vec<&ThreadView> {
        match self.filter {
            Some(filter) => self
                .threads
                .iter()
                .filter(|t| filter.matches(t.status))
                .collect(),
            None => self.threads.iter().collect(),
        }
    }

    /// Check if there's an active overlay
    pub fn has_overlay(&self) -> bool {
        self.overlay.is_some()
    }

    /// Get count of threads needing action
    pub fn action_count(&self) -> usize {
        self.threads.iter().filter(|t| t.needs_action).count()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------- FilterState Tests --------------------

    #[test]
    fn test_filter_state_default() {
        assert_eq!(FilterState::default(), FilterState::All);
    }

    #[test]
    fn test_filter_state_matches() {
        assert!(FilterState::All.matches(ThreadStatus::Running));
        assert!(FilterState::All.matches(ThreadStatus::Idle));

        assert!(FilterState::Working.matches(ThreadStatus::Running));
        assert!(FilterState::Working.matches(ThreadStatus::Waiting));
        assert!(!FilterState::Working.matches(ThreadStatus::Idle));
        assert!(!FilterState::Working.matches(ThreadStatus::Done));

        assert!(FilterState::ReadyToTest.matches(ThreadStatus::Done));
        assert!(!FilterState::ReadyToTest.matches(ThreadStatus::Running));

        assert!(FilterState::Idle.matches(ThreadStatus::Idle));
        assert!(FilterState::Idle.matches(ThreadStatus::Error));
        assert!(!FilterState::Idle.matches(ThreadStatus::Running));
    }

    #[test]
    fn test_filter_state_cycle() {
        let state = FilterState::All;
        assert_eq!(state.next(), FilterState::Working);
        assert_eq!(state.next().next(), FilterState::ReadyToTest);
        assert_eq!(state.next().next().next(), FilterState::Idle);
        assert_eq!(state.next().next().next().next(), FilterState::All);

        assert_eq!(state.prev(), FilterState::Idle);
    }

    #[test]
    fn test_filter_state_display_name() {
        assert_eq!(FilterState::All.display_name(), "All");
        assert_eq!(FilterState::Working.display_name(), "Working");
        assert_eq!(FilterState::ReadyToTest.display_name(), "Ready to Test");
        assert_eq!(FilterState::Idle.display_name(), "Idle");
    }

    // -------------------- ThreadMode Tests --------------------

    #[test]
    fn test_thread_mode_default() {
        assert_eq!(ThreadMode::default(), ThreadMode::Normal);
    }

    #[test]
    fn test_thread_mode_symbol() {
        assert_eq!(ThreadMode::Normal.symbol(), "");
        assert_eq!(ThreadMode::Plan.symbol(), "[PLAN]");
        assert_eq!(ThreadMode::Exec.symbol(), "[EXEC]");
    }

    // -------------------- Progress Tests --------------------

    #[test]
    fn test_progress_new() {
        let p = Progress::new(3, 5);
        assert_eq!(p.current, 3);
        assert_eq!(p.total, 5);
    }

    #[test]
    fn test_progress_percentage() {
        assert_eq!(Progress::new(0, 100).percentage(), 0);
        assert_eq!(Progress::new(50, 100).percentage(), 50);
        assert_eq!(Progress::new(100, 100).percentage(), 100);
        assert_eq!(Progress::new(1, 3).percentage(), 33);
        assert_eq!(Progress::new(0, 0).percentage(), 0); // Edge case
    }

    #[test]
    fn test_progress_as_fraction() {
        assert_eq!(Progress::new(3, 5).as_fraction(), "3/5");
        assert_eq!(Progress::new(0, 10).as_fraction(), "0/10");
    }

    #[test]
    fn test_progress_is_complete() {
        assert!(!Progress::new(3, 5).is_complete());
        assert!(Progress::new(5, 5).is_complete());
        assert!(Progress::new(6, 5).is_complete()); // Over 100%
        assert!(!Progress::new(0, 0).is_complete()); // Edge case
    }

    // -------------------- ThreadView Tests --------------------

    #[test]
    fn test_thread_view_new() {
        let view = ThreadView::new("id-1".to_string(), "Test".to_string(), "~/project".to_string());
        assert_eq!(view.id, "id-1");
        assert_eq!(view.title, "Test");
        assert_eq!(view.repository, "~/project");
        assert_eq!(view.mode, ThreadMode::Normal);
        assert_eq!(view.status, ThreadStatus::Idle);
        assert!(!view.needs_action);
    }

    #[test]
    fn test_thread_view_builder() {
        let view = ThreadView::new("id-2".to_string(), "Builder Test".to_string(), "~/app".to_string())
            .with_mode(ThreadMode::Exec)
            .with_status(ThreadStatus::Running)
            .with_duration("5m".to_string());

        assert_eq!(view.mode, ThreadMode::Exec);
        assert_eq!(view.status, ThreadStatus::Running);
        assert_eq!(view.duration, "5m");
    }

    #[test]
    fn test_thread_view_needs_action() {
        let view = ThreadView::new("id-3".to_string(), "Action Test".to_string(), "~/repo".to_string())
            .with_status(ThreadStatus::Waiting);
        assert!(view.needs_action);

        let view = ThreadView::new("id-4".to_string(), "No Action".to_string(), "~/repo".to_string())
            .with_status(ThreadStatus::Running);
        assert!(!view.needs_action);
    }

    #[test]
    fn test_thread_view_with_waiting_for() {
        let view = ThreadView::new("id-5".to_string(), "Waiting Test".to_string(), "~/repo".to_string())
            .with_waiting_for(Some(WaitingFor::UserInput));
        assert!(view.needs_action);
        assert_eq!(view.status_line(), "User input");
    }

    #[test]
    fn test_thread_view_status_line() {
        let view = ThreadView::new("id-6".to_string(), "Status".to_string(), "~/repo".to_string())
            .with_status(ThreadStatus::Running);
        assert_eq!(view.status_line(), "Running");

        let view = view.with_waiting_for(Some(WaitingFor::Permission {
            request_id: "req-1".to_string(),
            tool_name: "Bash".to_string(),
        }));
        assert_eq!(view.status_line(), "Permission: Bash");
    }

    // -------------------- OverlayState Tests --------------------

    #[test]
    fn test_overlay_state_thread_id() {
        let overlay = OverlayState::Question {
            thread_id: "thread-1".to_string(),
            thread_title: "Test Thread".to_string(),
            repository: "~/repo".to_string(),
            question: "Continue?".to_string(),
            options: vec!["Yes".to_string(), "No".to_string()],
            anchor_y: 10,
        };
        assert_eq!(overlay.thread_id(), "thread-1");
        assert_eq!(overlay.thread_title(), "Test Thread");
        assert_eq!(overlay.repository(), "~/repo");
        assert!(!overlay.is_plan());
    }

    #[test]
    fn test_overlay_state_plan() {
        let overlay = OverlayState::Plan {
            thread_id: "thread-2".to_string(),
            thread_title: "Plan Thread".to_string(),
            repository: "~/project".to_string(),
            request_id: "req-plan-1".to_string(),
            summary: PlanSummary::new("Test Plan".to_string(), vec!["Phase 1".to_string()], 3, 1000),
            scroll_offset: 0,
            anchor_y: 5,
        };
        assert!(overlay.is_plan());
        assert_eq!(overlay.thread_id(), "thread-2");
    }

    // -------------------- SystemStats Tests --------------------

    #[test]
    fn test_system_stats_default() {
        let stats = SystemStats::default();
        assert!(!stats.connected);
        assert_eq!(stats.cpu_percent, 0.0);
        assert_eq!(stats.ram_used_gb, 0.0);
        assert_eq!(stats.ram_total_gb, 0.0);
    }

    #[test]
    fn test_system_stats_new() {
        let stats = SystemStats::new(true, 45.5, 8.2, 16.0);
        assert!(stats.connected);
        assert_eq!(stats.cpu_percent, 45.5);
        assert_eq!(stats.ram_used_gb, 8.2);
        assert_eq!(stats.ram_total_gb, 16.0);
    }

    #[test]
    fn test_system_stats_is_heavy_load() {
        // Normal load
        assert!(!SystemStats::new(true, 50.0, 8.0, 16.0).is_heavy_load());
        // High CPU
        assert!(SystemStats::new(true, 95.0, 8.0, 16.0).is_heavy_load());
        // High RAM
        assert!(SystemStats::new(true, 50.0, 15.0, 16.0).is_heavy_load());
        // Both high
        assert!(SystemStats::new(true, 95.0, 15.0, 16.0).is_heavy_load());
    }

    #[test]
    fn test_system_stats_ram_percent() {
        let stats = SystemStats::new(true, 50.0, 8.0, 16.0);
        assert_eq!(stats.ram_percent(), 50.0);

        let stats = SystemStats::new(true, 50.0, 0.0, 0.0);
        assert_eq!(stats.ram_percent(), 0.0); // Avoid division by zero
    }

    #[test]
    fn test_system_stats_displays() {
        let stats = SystemStats::new(true, 45.0, 8.2, 16.0);
        assert_eq!(stats.cpu_display(), "45%");
        assert_eq!(stats.ram_display(), "8.2/16.0 GB");
        assert_eq!(stats.connection_display(), "Connected");

        let stats = SystemStats::new(false, 0.0, 0.0, 0.0);
        assert_eq!(stats.connection_display(), "Disconnected");
    }

    // -------------------- RenderContext Tests --------------------

    #[test]
    fn test_render_context_new() {
        let threads = vec![];
        let aggregate = Aggregate::new();
        let stats = SystemStats::default();
        let theme = Theme::default();

        let ctx = RenderContext::new(&threads, &aggregate, &stats, &theme);
        assert!(!ctx.has_overlay());
        assert_eq!(ctx.filter, None);
    }

    #[test]
    fn test_render_context_filtered_threads() {
        let threads = vec![
            ThreadView::new("1".to_string(), "T1".to_string(), "~/r1".to_string())
                .with_status(ThreadStatus::Running),
            ThreadView::new("2".to_string(), "T2".to_string(), "~/r2".to_string())
                .with_status(ThreadStatus::Idle),
            ThreadView::new("3".to_string(), "T3".to_string(), "~/r3".to_string())
                .with_status(ThreadStatus::Done),
        ];
        let aggregate = Aggregate::new();
        let stats = SystemStats::default();
        let theme = Theme::default();

        let ctx = RenderContext::new(&threads, &aggregate, &stats, &theme);

        // No filter - all threads
        assert_eq!(ctx.filtered_threads().len(), 3);

        // With Working filter
        let ctx = ctx.with_filter(Some(FilterState::Working));
        assert_eq!(ctx.filtered_threads().len(), 1);
        assert_eq!(ctx.filtered_threads()[0].id, "1");

        // With Idle filter
        let ctx = RenderContext::new(&threads, &aggregate, &stats, &theme)
            .with_filter(Some(FilterState::Idle));
        assert_eq!(ctx.filtered_threads().len(), 1);
        assert_eq!(ctx.filtered_threads()[0].id, "2");
    }

    #[test]
    fn test_render_context_action_count() {
        let threads = vec![
            ThreadView::new("1".to_string(), "T1".to_string(), "~/r1".to_string())
                .with_status(ThreadStatus::Waiting),
            ThreadView::new("2".to_string(), "T2".to_string(), "~/r2".to_string())
                .with_status(ThreadStatus::Running),
            ThreadView::new("3".to_string(), "T3".to_string(), "~/r3".to_string())
                .with_status(ThreadStatus::Error),
        ];
        let aggregate = Aggregate::new();
        let stats = SystemStats::default();
        let theme = Theme::default();

        let ctx = RenderContext::new(&threads, &aggregate, &stats, &theme);
        assert_eq!(ctx.action_count(), 2); // Waiting and Error need action
    }

    // -------------------- Theme Tests --------------------

    #[test]
    fn test_theme_default() {
        let theme = Theme::default();
        assert_eq!(theme.active, theme::COLOR_ACTIVE);
        assert_eq!(theme.success, theme::COLOR_TOOL_SUCCESS);
        assert_eq!(theme.error, theme::COLOR_TOOL_ERROR);
    }
}
