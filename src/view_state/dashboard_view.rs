//! Dashboard-specific view state
//!
//! This module provides view-only types for the dashboard that can be
//! rendered without accessing App.

use crate::models::dashboard::{Aggregate, PlanSummary, ThreadStatus, WaitingFor};
use crate::models::ThreadMode;
use crate::state::dashboard::DashboardQuestionState;
use crate::state::session::AskUserQuestionData;

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
// ThreadView
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
    /// Current operation description (e.g., "Reading file", "Running tests")
    pub current_operation: Option<String>,
    /// Pre-computed activity text for display in the Activity column
    ///
    /// This is computed based on thread state:
    /// - Running + tool active: tool's display_name (e.g., "Read: main.rs")
    /// - Running + no tool: "Thinking..."
    /// - Idle: "idle"
    /// - Done: "done"
    /// - Error: "error"
    /// - Waiting: None (uses old layout with status column + actions)
    pub activity_text: Option<String>,
}

impl ThreadView {
    /// Create a new thread view with minimal required fields
    pub fn new(id: String, title: String, repository: String) -> Self {
        Self {
            id,
            title,
            repository,
            mode: ThreadMode::Normal,
            status: ThreadStatus::Done,
            waiting_for: None,
            progress: None,
            duration: String::new(),
            needs_action: false,
            current_operation: None,
            activity_text: None,
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

    /// Builder-style setter for current_operation
    pub fn with_current_operation(mut self, current_operation: Option<String>) -> Self {
        self.current_operation = current_operation;
        self
    }

    /// Builder-style setter for activity_text
    pub fn with_activity_text(mut self, activity_text: Option<String>) -> Self {
        self.activity_text = activity_text;
        self
    }

    /// Get the status line for display
    ///
    /// Priority:
    /// 1. If waiting_for is set, show the waiting description
    /// 2. If running and current_operation is set, show current_operation
    /// 3. Otherwise show the status name
    pub fn status_line(&self) -> String {
        if let Some(ref waiting) = self.waiting_for {
            waiting.description()
        } else if self.status == ThreadStatus::Running {
            if let Some(ref op) = self.current_operation {
                return op.clone();
            }
            "Running".to_string()
        } else {
            match self.status {
                ThreadStatus::Running => "Running".to_string(), // unreachable but complete
                ThreadStatus::Waiting => "Waiting".to_string(),
                ThreadStatus::Done => "Ready".to_string(),
                ThreadStatus::Error => "Error".to_string(),
            }
        }
    }
}

// ============================================================================
// OverlayState
// ============================================================================

/// State for overlay dialogs (permission requests, questions, plan approvals)
#[derive(Debug, Clone)]
pub enum OverlayState {
    /// Multiple choice question from AskUserQuestion tool
    Question {
        thread_id: String,
        thread_title: String,
        repository: String,
        /// Full question data from the AskUserQuestion tool
        question_data: Option<AskUserQuestionData>,
        /// Y position to anchor the overlay
        anchor_y: u16,
    },
    /// Free-form text input
    FreeForm {
        thread_id: String,
        thread_title: String,
        repository: String,
        /// Full question data from the AskUserQuestion tool (preserved when switching from Question)
        question_data: Option<AskUserQuestionData>,
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
        selected_action: usize,     // 0=Approve, 1=Reject, 2=Feedback
        feedback_text: String,
        feedback_active: bool,
        anchor_y: u16,
    },
    /// Claude CLI login required dialog
    ClaudeLogin {
        request_id: String,
        auth_url: String,
        state: ClaudeLoginState,
        anchor_y: u16,
    },
    /// Claude accounts management overlay
    ClaudeAccounts {
        accounts: Vec<ClaudeAccountInfo>,
        selected_index: usize,
        anchor_y: u16,
        /// True when an Add Account flow is in progress (blocks duplicate presses)
        adding: bool,
        /// Request ID for the in-progress add flow
        add_request_id: Option<String>,
        /// Status message shown at bottom (e.g., "Authenticating...", "Added!", error)
        status_message: Option<String>,
        /// True when paste-token text input is active
        paste_mode: bool,
        /// Buffer for pasted token text
        paste_buffer: String,
        /// OAuth URL from setup-token output (for display in overlay)
        auth_url: Option<String>,
    },
    /// VPS configuration overlay (/vps command)
    VpsConfig {
        state: VpsConfigState,
        anchor_y: u16,
    },
}

/// Info about a Claude account for display in the overlay
#[derive(Debug, Clone)]
pub struct ClaudeAccountInfo {
    pub id: String,
    pub label: String,
    pub email: Option<String>,
    pub priority: i64,
    pub status: String,
    pub cooldown_until: Option<i64>,
    pub last_error: Option<String>,
}

/// State of the Claude login dialog
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaudeLoginState {
    /// Initial state - URL displayed, browser may have auto-opened
    ShowingUrl { browser_opened: bool },
    /// User pressed Done - waiting for backend verification
    Verifying,
    /// Backend confirmed successful authentication
    VerificationSuccess { email: String, success_time: std::time::Instant },
    /// Backend reported authentication failed
    VerificationFailed { error: String },
    /// Browser auto-open failed - show error with manual option
    BrowserOpenFailed { auth_url: String, error: String },
}

/// Mode selector for VPS configuration dialog
#[derive(Debug, Clone, PartialEq)]
pub enum VpsConfigMode {
    Remote,
    Local,
}

impl Default for VpsConfigMode {
    fn default() -> Self {
        Self::Remote
    }
}

// ============================================================================
// Enhanced VPS Config Types
// ============================================================================

/// Per-field validation errors for VPS config dialog
#[derive(Debug, Clone, Default)]
pub struct FieldErrors {
    /// Error message for IP field
    pub ip: Option<String>,
    /// Error message for password field
    pub password: Option<String>,
}

impl FieldErrors {
    /// Create new empty field errors
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if there are any errors
    pub fn has_errors(&self) -> bool {
        self.ip.is_some() || self.password.is_some()
    }

    /// Clear all errors
    pub fn clear(&mut self) {
        self.ip = None;
        self.password = None;
    }
}

/// Provisioning phases for progress tracking
#[derive(Debug, Clone, PartialEq)]
pub enum ProvisioningPhase {
    /// Initial connection phase
    Connecting,
    /// Replacing VPS (SSH + script execution)
    ReplacingVps,
    /// Waiting for health check
    WaitingForHealth {
        /// Progress percentage (0-100)
        progress: u8,
        /// Current status message
        message: String,
    },
    /// Finalizing setup
    Finalizing,
}

impl ProvisioningPhase {
    /// Get display message for this phase
    pub fn display_message(&self) -> String {
        match self {
            Self::Connecting => "Connecting...".to_string(),
            Self::ReplacingVps => "Replacing VPS...".to_string(),
            Self::WaitingForHealth { progress, message } => {
                format!("{}% - {}", progress, message)
            }
            Self::Finalizing => "Finalizing...".to_string(),
        }
    }

    /// Get progress percentage (0-100)
    pub fn progress_percent(&self) -> u8 {
        match self {
            Self::Connecting => 10,
            Self::ReplacingVps => 30,
            Self::WaitingForHealth { progress, .. } => *progress,
            Self::Finalizing => 95,
        }
    }
}

/// VPS configuration errors with actionable categorization
#[derive(Debug, Clone)]
pub enum VpsError {
    /// Network connectivity issues
    Network(String),
    /// Authentication expired (401)
    AuthExpired,
    /// No active subscription (403)
    NoSubscription,
    /// VPS already exists for user (409)
    Conflict,
    /// Rate limited (429)
    RateLimited {
        /// Seconds to wait before retry
        retry_after: u32,
    },
    /// SSH connection failed
    SshFailed(String),
    /// Operation timed out
    Timeout,
    /// Server error (5xx)
    Server {
        /// HTTP status code
        status: u16,
        /// Error message
        message: String,
    },
    /// Unknown/generic error
    Unknown(String),
}

impl VpsError {
    /// Get a user-friendly error message
    pub fn user_message(&self) -> String {
        match self {
            Self::Network(msg) => format!("Network error: {}", msg),
            Self::AuthExpired => "Session expired.".to_string(),
            Self::NoSubscription => "Subscription required.".to_string(),
            Self::Conflict => "VPS already exists.".to_string(),
            Self::RateLimited { retry_after } => {
                format!("Too many requests. Wait {}s.", retry_after)
            }
            Self::SshFailed(msg) => format!("SSH failed: {}", msg),
            Self::Timeout => "Operation timed out.".to_string(),
            Self::Server { status, message } => format!("Server error ({}): {}", status, message),
            Self::Unknown(msg) => msg.clone(),
        }
    }

    /// Get the error header for display
    pub fn header(&self) -> &'static str {
        match self {
            Self::AuthExpired => "Session Expired",
            Self::NoSubscription => "Subscription Required",
            Self::RateLimited { .. } => "Rate Limited",
            _ => "Failed to replace VPS",
        }
    }

    /// Check if this is an auth error (show Login instead of Retry)
    pub fn is_auth_error(&self) -> bool {
        matches!(self, Self::AuthExpired)
    }

    /// Check if this error is retriable
    pub fn is_retriable(&self) -> bool {
        !matches!(self, Self::NoSubscription)
    }
}

/// State of the VPS configuration dialog (/vps command)
#[derive(Debug, Clone)]
pub enum VpsConfigState {
    /// Input form for VPS credentials
    InputFields {
        /// Current mode (Remote or Local)
        mode: VpsConfigMode,
        /// VPS IP address
        ip: String,
        /// SSH password
        password: String,
        /// Which field is focused: 0=mode, 1=IP, 2=password
        field_focus: u8,
        /// Per-field validation errors
        errors: FieldErrors,
    },
    /// Provisioning in progress
    Provisioning {
        /// Current provisioning phase with progress
        phase: ProvisioningPhase,
        /// Spinner animation frame (0-3)
        spinner_frame: usize,
    },
    /// VPS replacement succeeded
    Success {
        /// New VPS hostname (e.g., "user.spoq.dev")
        hostname: String,
    },
    /// VPS replacement failed
    Error {
        /// Categorized error with actions
        error: VpsError,
        /// Saved input for retry (ip, password)
        saved_input: Option<(String, String)>,
    },
    /// Re-authenticating via device flow
    Authenticating {
        /// Verification URL to show the user
        verification_url: String,
        /// User code to display
        user_code: String,
        /// Spinner animation frame (0-3)
        spinner_frame: usize,
    },
}

impl OverlayState {
    /// Get the thread ID associated with this overlay (returns empty string for ClaudeLogin/VpsConfig)
    pub fn thread_id(&self) -> &str {
        match self {
            OverlayState::Question { thread_id, .. } => thread_id,
            OverlayState::FreeForm { thread_id, .. } => thread_id,
            OverlayState::Plan { thread_id, .. } => thread_id,
            OverlayState::ClaudeLogin { .. } => "",
            OverlayState::ClaudeAccounts { .. } => "",
            OverlayState::VpsConfig { .. } => "",
        }
    }

    /// Get the thread title associated with this overlay
    pub fn thread_title(&self) -> &str {
        match self {
            OverlayState::Question { thread_title, .. } => thread_title,
            OverlayState::FreeForm { thread_title, .. } => thread_title,
            OverlayState::Plan { thread_title, .. } => thread_title,
            OverlayState::ClaudeLogin { .. } => "Claude Login",
            OverlayState::ClaudeAccounts { .. } => "Claude Accounts",
            OverlayState::VpsConfig { .. } => "Change VPS",
        }
    }

    /// Get the repository associated with this overlay
    pub fn repository(&self) -> &str {
        match self {
            OverlayState::Question { repository, .. } => repository,
            OverlayState::FreeForm { repository, .. } => repository,
            OverlayState::Plan { repository, .. } => repository,
            OverlayState::ClaudeLogin { .. } => "",
            OverlayState::ClaudeAccounts { .. } => "",
            OverlayState::VpsConfig { .. } => "",
        }
    }

    /// Check if this is a plan approval overlay
    pub fn is_plan(&self) -> bool {
        matches!(self, OverlayState::Plan { .. })
    }

    /// Check if this is a Claude login overlay
    pub fn is_claude_login(&self) -> bool {
        matches!(self, OverlayState::ClaudeLogin { .. })
    }
}

// ============================================================================
// Theme
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
        use ratatui::style::Color;
        // Using same colors as crate::ui::theme
        Self {
            active: Color::LightGreen,        // COLOR_ACTIVE
            success: Color::Rgb(4, 181, 117), // COLOR_TOOL_SUCCESS
            error: Color::Red,                // COLOR_TOOL_ERROR
            waiting: Color::Yellow,
            dim: Color::DarkGray,    // COLOR_DIM
            border: Color::DarkGray, // COLOR_BORDER
            accent: Color::White,    // COLOR_ACCENT
        }
    }
}

// ============================================================================
// RenderContext
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
    /// Active overlay (if any)
    pub overlay: Option<&'a OverlayState>,
    /// System statistics
    pub system_stats: &'a super::SystemStats,
    /// Theme colors
    pub theme: &'a Theme,
    /// Question navigation state for multi-question flow
    pub question_state: Option<&'a DashboardQuestionState>,
    /// Remaining seconds for the current question timer (None = no timer)
    pub question_timer_secs: Option<u32>,
    /// GitHub repos for empty state
    pub repos: &'a [crate::models::GitHubRepo],
}

impl<'a> RenderContext<'a> {
    /// Create a new render context
    pub fn new(
        threads: &'a [ThreadView],
        aggregate: &'a Aggregate,
        system_stats: &'a super::SystemStats,
        theme: &'a Theme,
        repos: &'a [crate::models::GitHubRepo],
    ) -> Self {
        Self {
            threads,
            aggregate,
            overlay: None,
            system_stats,
            theme,
            question_state: None,
            question_timer_secs: None,
            repos,
        }
    }

    /// Set overlay state
    pub fn with_overlay(mut self, overlay: Option<&'a OverlayState>) -> Self {
        self.overlay = overlay;
        self
    }

    /// Set question navigation state
    pub fn with_question_state(mut self, state: Option<&'a DashboardQuestionState>) -> Self {
        self.question_state = state;
        self
    }

    /// Set question timer remaining seconds
    pub fn with_question_timer(mut self, secs: Option<u32>) -> Self {
        self.question_timer_secs = secs;
        self
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
// DashboardViewState
// ============================================================================

/// Dashboard-level view state for UI rendering
#[derive(Debug, Clone, Default)]
pub struct DashboardViewState {
    /// Whether an overlay is open
    pub has_overlay: bool,
    /// Total thread count
    pub thread_count: usize,
    /// Count of threads needing action
    pub action_count: usize,
}

impl DashboardViewState {
    /// Create a new dashboard view state
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a dashboard view state with given values
    pub fn with_values(has_overlay: bool, thread_count: usize, action_count: usize) -> Self {
        Self {
            has_overlay,
            thread_count,
            action_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let view = ThreadView::new(
            "id-1".to_string(),
            "Test".to_string(),
            "~/project".to_string(),
        );
        assert_eq!(view.id, "id-1");
        assert_eq!(view.title, "Test");
        assert_eq!(view.repository, "~/project");
        assert_eq!(view.mode, ThreadMode::Normal);
        assert_eq!(view.status, ThreadStatus::Done);
        assert!(!view.needs_action);
    }

    #[test]
    fn test_thread_view_builder() {
        let view = ThreadView::new(
            "id-2".to_string(),
            "Builder Test".to_string(),
            "~/app".to_string(),
        )
        .with_mode(ThreadMode::Exec)
        .with_status(ThreadStatus::Running)
        .with_duration("5m".to_string());

        assert_eq!(view.mode, ThreadMode::Exec);
        assert_eq!(view.status, ThreadStatus::Running);
        assert_eq!(view.duration, "5m");
    }

    #[test]
    fn test_thread_view_needs_action() {
        let view = ThreadView::new(
            "id-3".to_string(),
            "Action Test".to_string(),
            "~/repo".to_string(),
        )
        .with_status(ThreadStatus::Waiting);
        assert!(view.needs_action);

        let view = ThreadView::new(
            "id-4".to_string(),
            "No Action".to_string(),
            "~/repo".to_string(),
        )
        .with_status(ThreadStatus::Running);
        assert!(!view.needs_action);
    }

    #[test]
    fn test_thread_view_with_waiting_for() {
        let view = ThreadView::new(
            "id-5".to_string(),
            "Waiting Test".to_string(),
            "~/repo".to_string(),
        )
        .with_waiting_for(Some(WaitingFor::UserInput));
        assert!(view.needs_action);
        assert_eq!(view.status_line(), "User input");
    }

    #[test]
    fn test_thread_view_status_line() {
        let view = ThreadView::new(
            "id-6".to_string(),
            "Status".to_string(),
            "~/repo".to_string(),
        )
        .with_status(ThreadStatus::Running);
        assert_eq!(view.status_line(), "Running");

        let view = view.with_waiting_for(Some(WaitingFor::Permission {
            request_id: "req-1".to_string(),
            tool_name: "Bash".to_string(),
        }));
        assert_eq!(view.status_line(), "Permission: Bash");
    }

    #[test]
    fn test_thread_view_current_operation() {
        // When running without current_operation, show "Running"
        let view = ThreadView::new(
            "id-7".to_string(),
            "Op Test".to_string(),
            "~/repo".to_string(),
        )
        .with_status(ThreadStatus::Running);
        assert_eq!(view.status_line(), "Running");

        // When running with current_operation, show the operation
        let view = view.with_current_operation(Some("Reading file".to_string()));
        assert_eq!(view.status_line(), "Reading file");

        // When waiting_for is set, it takes priority over current_operation
        let view = view.with_waiting_for(Some(WaitingFor::UserInput));
        assert_eq!(view.status_line(), "User input");
    }

    // -------------------- Activity Text Tests --------------------

    #[test]
    fn test_thread_view_with_activity_text() {
        // Test the with_activity_text builder method
        let view = ThreadView::new(
            "id-activity".to_string(),
            "Activity Test".to_string(),
            "~/repo".to_string(),
        )
        .with_status(ThreadStatus::Running)
        .with_activity_text(Some("Read: main.rs".to_string()));

        assert_eq!(view.activity_text, Some("Read: main.rs".to_string()));
    }

    #[test]
    fn test_thread_view_activity_text_default_is_none() {
        // Default ThreadView should have None activity_text
        let view = ThreadView::new(
            "id-default".to_string(),
            "Default Test".to_string(),
            "~/repo".to_string(),
        );

        assert_eq!(view.activity_text, None);
    }

    #[test]
    fn test_thread_view_activity_text_edit_format() {
        // Test Edit tool display format
        let view = ThreadView::new(
            "id-edit".to_string(),
            "Edit Test".to_string(),
            "~/repo".to_string(),
        )
        .with_status(ThreadStatus::Running)
        .with_activity_text(Some("Edit: handlers.rs".to_string()));

        assert_eq!(view.activity_text, Some("Edit: handlers.rs".to_string()));
    }

    #[test]
    fn test_thread_view_activity_text_glob_format() {
        // Test Glob tool display format
        let view = ThreadView::new(
            "id-glob".to_string(),
            "Glob Test".to_string(),
            "~/repo".to_string(),
        )
        .with_status(ThreadStatus::Running)
        .with_activity_text(Some("Glob: **/*.rs".to_string()));

        assert_eq!(view.activity_text, Some("Glob: **/*.rs".to_string()));
    }

    #[test]
    fn test_thread_view_activity_text_thinking() {
        // Test Thinking fallback (when no tool active)
        let view = ThreadView::new(
            "id-thinking".to_string(),
            "Thinking Test".to_string(),
            "~/repo".to_string(),
        )
        .with_status(ThreadStatus::Running)
        .with_activity_text(Some("Thinking...".to_string()));

        assert_eq!(view.activity_text, Some("Thinking...".to_string()));
    }

    #[test]
    fn test_thread_view_activity_text_done() {
        // Test done status activity_text
        let view = ThreadView::new(
            "id-done".to_string(),
            "Done Test".to_string(),
            "~/repo".to_string(),
        )
        .with_status(ThreadStatus::Done)
        .with_activity_text(Some("done".to_string()));

        assert_eq!(view.activity_text, Some("done".to_string()));
    }

    #[test]
    fn test_thread_view_activity_text_idle() {
        // Test idle status activity_text
        let view = ThreadView::new(
            "id-idle".to_string(),
            "Idle Test".to_string(),
            "~/repo".to_string(),
        )
        .with_status(ThreadStatus::Done)
        .with_activity_text(Some("idle".to_string()));

        assert_eq!(view.activity_text, Some("idle".to_string()));
    }

    #[test]
    fn test_thread_view_activity_text_waiting_is_none() {
        // Waiting threads should have None activity_text (uses old layout)
        let view = ThreadView::new(
            "id-waiting".to_string(),
            "Waiting Test".to_string(),
            "~/repo".to_string(),
        )
        .with_status(ThreadStatus::Waiting)
        .with_waiting_for(Some(WaitingFor::Permission {
            request_id: "req-1".to_string(),
            tool_name: "Bash".to_string(),
        }));

        // activity_text should be None for waiting threads (they use action layout)
        assert_eq!(view.activity_text, None);
    }

    // -------------------- OverlayState Tests --------------------

    #[test]
    fn test_overlay_state_thread_id() {
        let overlay = OverlayState::Question {
            thread_id: "thread-1".to_string(),
            thread_title: "Test Thread".to_string(),
            repository: "~/repo".to_string(),
            question_data: None,
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
            summary: PlanSummary::new(
                "Test Plan".to_string(),
                vec!["Phase 1".to_string()],
                3,
                Some(1000),
            ),
            scroll_offset: 0,
            selected_action: 0,
            feedback_text: String::new(),
            feedback_active: false,
            anchor_y: 5,
        };
        assert!(overlay.is_plan());
        assert_eq!(overlay.thread_id(), "thread-2");
    }

    // -------------------- DashboardViewState Tests --------------------

    #[test]
    fn test_dashboard_view_state_default() {
        let state = DashboardViewState::default();
        assert!(!state.has_overlay);
        assert_eq!(state.thread_count, 0);
        assert_eq!(state.action_count, 0);
    }

    #[test]
    fn test_dashboard_view_state_with_values() {
        let state = DashboardViewState::with_values(true, 10, 3);
        assert!(state.has_overlay);
        assert_eq!(state.thread_count, 10);
        assert_eq!(state.action_count, 3);
    }
}
