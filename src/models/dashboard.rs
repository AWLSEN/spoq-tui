//! Dashboard data models for thread status tracking and aggregation
//!
//! This module defines the core data models for the multi-thread dashboard view,
//! including thread status enums, waiting states, and aggregation types.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Thread Status
// ============================================================================

/// Status of a thread in the dashboard view
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ThreadStatus {
    /// Thread is not actively processing
    #[default]
    Idle,
    /// Agent is actively working
    Running,
    /// Waiting for user input or permission
    Waiting,
    /// Task completed successfully
    Done,
    /// Error occurred during processing
    Error,
}

impl ThreadStatus {
    /// Check if status indicates active work
    pub fn is_active(&self) -> bool {
        matches!(self, ThreadStatus::Running | ThreadStatus::Waiting)
    }

    /// Check if status indicates the thread needs attention
    pub fn needs_attention(&self) -> bool {
        matches!(self, ThreadStatus::Waiting | ThreadStatus::Error)
    }
}

// ============================================================================
// Waiting For
// ============================================================================

/// What the thread is waiting for (tagged enum for flexibility)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WaitingFor {
    /// Waiting for tool permission approval
    Permission {
        request_id: String,
        tool_name: String,
    },
    /// Waiting for plan approval
    PlanApproval {
        /// Summary title for display
        plan_summary: String,
    },
    /// Waiting for generic user input
    UserInput,
}

impl WaitingFor {
    /// Get a brief description of what's being waited for
    pub fn description(&self) -> String {
        match self {
            WaitingFor::Permission { tool_name, .. } => format!("Permission: {}", tool_name),
            WaitingFor::PlanApproval { plan_summary } => format!("Plan: {}", plan_summary),
            WaitingFor::UserInput => "User input".to_string(),
        }
    }
}

// ============================================================================
// Plan Summary
// ============================================================================

/// Summary of a plan awaiting approval
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanSummary {
    /// Title/name of the plan
    pub title: String,
    /// List of phase descriptions
    pub phases: Vec<String>,
    /// Number of files to be modified/created
    pub file_count: u32,
    /// Estimated token usage
    pub estimated_tokens: u32,
}

impl PlanSummary {
    /// Create a new plan summary
    pub fn new(title: String, phases: Vec<String>, file_count: u32, estimated_tokens: u32) -> Self {
        Self {
            title,
            phases,
            file_count,
            estimated_tokens,
        }
    }

    /// Get the number of phases
    pub fn phase_count(&self) -> usize {
        self.phases.len()
    }
}

// ============================================================================
// Aggregate
// ============================================================================

/// Aggregate statistics for the dashboard
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Aggregate {
    /// Count by status
    pub by_status: HashMap<ThreadStatus, u32>,
    /// Total number of repositories/threads tracked
    pub total_repos: u32,
}

impl Aggregate {
    /// Create a new empty aggregate
    pub fn new() -> Self {
        Self::default()
    }

    /// Count of working threads (Running + Waiting)
    pub fn working(&self) -> u32 {
        self.by_status.get(&ThreadStatus::Running).copied().unwrap_or(0)
            + self.by_status.get(&ThreadStatus::Waiting).copied().unwrap_or(0)
    }

    /// Count of threads ready for testing (Done)
    pub fn ready_to_test(&self) -> u32 {
        self.by_status.get(&ThreadStatus::Done).copied().unwrap_or(0)
    }

    /// Count of idle threads (Idle + Error)
    pub fn idle(&self) -> u32 {
        self.by_status.get(&ThreadStatus::Idle).copied().unwrap_or(0)
            + self.by_status.get(&ThreadStatus::Error).copied().unwrap_or(0)
    }

    /// Count of threads with a specific status
    pub fn count(&self, status: ThreadStatus) -> u32 {
        self.by_status.get(&status).copied().unwrap_or(0)
    }

    /// Increment count for a status
    pub fn increment(&mut self, status: ThreadStatus) {
        *self.by_status.entry(status).or_insert(0) += 1;
        self.total_repos += 1;
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Infer thread status from agent state string
///
/// Maps backend agent state strings to ThreadStatus enum.
///
/// Actual backend values: `thinking`, `streaming`, `tool_use`, `idle`
/// Additional legacy values are supported for backward compatibility.
pub fn infer_status_from_agent_state(state: &str) -> ThreadStatus {
    match state.to_lowercase().as_str() {
        // Primary backend values
        "thinking" | "streaming" | "tool_use" => ThreadStatus::Running,
        "idle" => ThreadStatus::Idle,
        // Legacy/extended values for backward compatibility
        "running" | "executing" => ThreadStatus::Running,
        "waiting" | "awaiting_permission" | "awaiting_input" | "paused" => ThreadStatus::Waiting,
        "done" | "complete" | "completed" | "finished" | "success" => ThreadStatus::Done,
        "error" | "failed" | "failure" => ThreadStatus::Error,
        "ready" | "" => ThreadStatus::Idle,
        _ => ThreadStatus::Idle, // Default to Idle for unknown states
    }
}

/// Derive display repository name from full path
///
/// Converts absolute paths to user-friendly format:
/// - `/Users/sam/api` -> `~/api`
/// - `/home/user/project` -> `~/project`
/// - Short paths remain unchanged
pub fn derive_repository(path: &str) -> String {
    if path.is_empty() {
        return String::new();
    }

    // Try to find home directory patterns and replace with ~
    let path = if let Some(rest) = path.strip_prefix("/Users/") {
        // macOS: /Users/username/... -> ~/...
        if let Some(slash_idx) = rest.find('/') {
            format!("~{}", &rest[slash_idx..])
        } else {
            "~".to_string()
        }
    } else if let Some(rest) = path.strip_prefix("/home/") {
        // Linux: /home/username/... -> ~/...
        if let Some(slash_idx) = rest.find('/') {
            format!("~{}", &rest[slash_idx..])
        } else {
            "~".to_string()
        }
    } else {
        path.to_string()
    };

    // If path is still long, just take the last component(s)
    if path.len() > 30 {
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        if parts.len() >= 2 {
            format!(".../{}/{}", parts[parts.len() - 2], parts[parts.len() - 1])
        } else if !parts.is_empty() {
            format!(".../{}", parts[parts.len() - 1])
        } else {
            path
        }
    } else {
        path
    }
}

/// Compute human-readable duration from a timestamp
///
/// Returns formats like: "12m", "3h", "2d", "<1m"
pub fn compute_duration(from: DateTime<Utc>) -> String {
    let now = Utc::now();
    let diff = now.signed_duration_since(from);

    if diff < Duration::zero() {
        return "0s".to_string();
    }

    let total_seconds = diff.num_seconds();
    let minutes = diff.num_minutes();
    let hours = diff.num_hours();
    let days = diff.num_days();

    if days > 0 {
        format!("{}d", days)
    } else if hours > 0 {
        format!("{}h", hours)
    } else if minutes > 0 {
        format!("{}m", minutes)
    } else if total_seconds > 0 {
        format!("{}s", total_seconds)
    } else {
        "<1m".to_string()
    }
}

/// Compute aggregate statistics from threads
///
/// Uses agent_events to determine current status for each thread
pub fn compute_local_aggregate(
    threads: &[crate::models::Thread],
    agent_events: &HashMap<String, String>,
) -> Aggregate {
    let mut aggregate = Aggregate::new();

    for thread in threads {
        let status = if let Some(state) = agent_events.get(&thread.id) {
            infer_status_from_agent_state(state)
        } else {
            ThreadStatus::Idle
        };
        aggregate.increment(status);
    }

    aggregate
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------- ThreadStatus Tests --------------------

    #[test]
    fn test_thread_status_default() {
        assert_eq!(ThreadStatus::default(), ThreadStatus::Idle);
    }

    #[test]
    fn test_thread_status_is_active() {
        assert!(!ThreadStatus::Idle.is_active());
        assert!(ThreadStatus::Running.is_active());
        assert!(ThreadStatus::Waiting.is_active());
        assert!(!ThreadStatus::Done.is_active());
        assert!(!ThreadStatus::Error.is_active());
    }

    #[test]
    fn test_thread_status_needs_attention() {
        assert!(!ThreadStatus::Idle.needs_attention());
        assert!(!ThreadStatus::Running.needs_attention());
        assert!(ThreadStatus::Waiting.needs_attention());
        assert!(!ThreadStatus::Done.needs_attention());
        assert!(ThreadStatus::Error.needs_attention());
    }

    #[test]
    fn test_thread_status_serialization() {
        // Test snake_case serialization
        let running = ThreadStatus::Running;
        let json = serde_json::to_string(&running).expect("Failed to serialize");
        assert_eq!(json, "\"running\"");

        let waiting = ThreadStatus::Waiting;
        let json = serde_json::to_string(&waiting).expect("Failed to serialize");
        assert_eq!(json, "\"waiting\"");
    }

    #[test]
    fn test_thread_status_deserialization() {
        let json = "\"running\"";
        let status: ThreadStatus = serde_json::from_str(json).expect("Failed to deserialize");
        assert_eq!(status, ThreadStatus::Running);

        let json = "\"done\"";
        let status: ThreadStatus = serde_json::from_str(json).expect("Failed to deserialize");
        assert_eq!(status, ThreadStatus::Done);

        let json = "\"error\"";
        let status: ThreadStatus = serde_json::from_str(json).expect("Failed to deserialize");
        assert_eq!(status, ThreadStatus::Error);
    }

    #[test]
    fn test_thread_status_hash() {
        // Test that ThreadStatus can be used as HashMap key
        let mut map: HashMap<ThreadStatus, u32> = HashMap::new();
        map.insert(ThreadStatus::Running, 5);
        map.insert(ThreadStatus::Waiting, 3);

        assert_eq!(map.get(&ThreadStatus::Running), Some(&5));
        assert_eq!(map.get(&ThreadStatus::Waiting), Some(&3));
        assert_eq!(map.get(&ThreadStatus::Idle), None);
    }

    // -------------------- WaitingFor Tests --------------------

    #[test]
    fn test_waiting_for_permission() {
        let waiting = WaitingFor::Permission {
            request_id: "req-123".to_string(),
            tool_name: "Bash".to_string(),
        };

        assert_eq!(waiting.description(), "Permission: Bash");
    }

    #[test]
    fn test_waiting_for_plan_approval() {
        let waiting = WaitingFor::PlanApproval {
            plan_summary: "Add dark mode".to_string(),
        };

        assert_eq!(waiting.description(), "Plan: Add dark mode");
    }

    #[test]
    fn test_waiting_for_user_input() {
        let waiting = WaitingFor::UserInput;
        assert_eq!(waiting.description(), "User input");
    }

    #[test]
    fn test_waiting_for_serialization() {
        let waiting = WaitingFor::Permission {
            request_id: "req-456".to_string(),
            tool_name: "Read".to_string(),
        };

        let json = serde_json::to_string(&waiting).expect("Failed to serialize");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("Failed to parse");

        assert_eq!(parsed["type"], "permission");
        assert_eq!(parsed["request_id"], "req-456");
        assert_eq!(parsed["tool_name"], "Read");
    }

    #[test]
    fn test_waiting_for_deserialization() {
        let json = r#"{"type": "plan_approval", "plan_summary": "Test plan"}"#;
        let waiting: WaitingFor = serde_json::from_str(json).expect("Failed to deserialize");

        match waiting {
            WaitingFor::PlanApproval { plan_summary } => {
                assert_eq!(plan_summary, "Test plan");
            }
            _ => panic!("Expected PlanApproval"),
        }
    }

    // -------------------- PlanSummary Tests --------------------

    #[test]
    fn test_plan_summary_new() {
        let plan = PlanSummary::new(
            "Test Plan".to_string(),
            vec!["Phase 1".to_string(), "Phase 2".to_string()],
            5,
            10000,
        );

        assert_eq!(plan.title, "Test Plan");
        assert_eq!(plan.phases.len(), 2);
        assert_eq!(plan.file_count, 5);
        assert_eq!(plan.estimated_tokens, 10000);
    }

    #[test]
    fn test_plan_summary_phase_count() {
        let plan = PlanSummary::new(
            "Multi-phase".to_string(),
            vec![
                "Setup".to_string(),
                "Implementation".to_string(),
                "Testing".to_string(),
            ],
            10,
            50000,
        );

        assert_eq!(plan.phase_count(), 3);
    }

    #[test]
    fn test_plan_summary_serialization() {
        let plan = PlanSummary::new(
            "Serialize Test".to_string(),
            vec!["Phase A".to_string()],
            2,
            5000,
        );

        let json = serde_json::to_string(&plan).expect("Failed to serialize");
        let deserialized: PlanSummary = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(plan, deserialized);
    }

    // -------------------- Aggregate Tests --------------------

    #[test]
    fn test_aggregate_new() {
        let agg = Aggregate::new();
        assert_eq!(agg.total_repos, 0);
        assert!(agg.by_status.is_empty());
    }

    #[test]
    fn test_aggregate_increment() {
        let mut agg = Aggregate::new();
        agg.increment(ThreadStatus::Running);
        agg.increment(ThreadStatus::Running);
        agg.increment(ThreadStatus::Waiting);

        assert_eq!(agg.count(ThreadStatus::Running), 2);
        assert_eq!(agg.count(ThreadStatus::Waiting), 1);
        assert_eq!(agg.total_repos, 3);
    }

    #[test]
    fn test_aggregate_working() {
        let mut agg = Aggregate::new();
        agg.increment(ThreadStatus::Running);
        agg.increment(ThreadStatus::Running);
        agg.increment(ThreadStatus::Waiting);
        agg.increment(ThreadStatus::Idle);

        assert_eq!(agg.working(), 3); // 2 Running + 1 Waiting
    }

    #[test]
    fn test_aggregate_ready_to_test() {
        let mut agg = Aggregate::new();
        agg.increment(ThreadStatus::Done);
        agg.increment(ThreadStatus::Done);
        agg.increment(ThreadStatus::Running);

        assert_eq!(agg.ready_to_test(), 2);
    }

    #[test]
    fn test_aggregate_idle() {
        let mut agg = Aggregate::new();
        agg.increment(ThreadStatus::Idle);
        agg.increment(ThreadStatus::Error);
        agg.increment(ThreadStatus::Error);
        agg.increment(ThreadStatus::Running);

        assert_eq!(agg.idle(), 3); // 1 Idle + 2 Error
    }

    #[test]
    fn test_aggregate_count_missing() {
        let agg = Aggregate::new();
        assert_eq!(agg.count(ThreadStatus::Running), 0);
    }

    // -------------------- infer_status_from_agent_state Tests --------------------

    #[test]
    fn test_infer_status_running() {
        // Primary backend values
        assert_eq!(
            infer_status_from_agent_state("thinking"),
            ThreadStatus::Running
        );
        assert_eq!(
            infer_status_from_agent_state("streaming"),
            ThreadStatus::Running
        );
        assert_eq!(
            infer_status_from_agent_state("tool_use"),
            ThreadStatus::Running
        );
        // Legacy values
        assert_eq!(
            infer_status_from_agent_state("running"),
            ThreadStatus::Running
        );
        assert_eq!(
            infer_status_from_agent_state("executing"),
            ThreadStatus::Running
        );
        assert_eq!(
            infer_status_from_agent_state("THINKING"),
            ThreadStatus::Running
        ); // case insensitive
        assert_eq!(
            infer_status_from_agent_state("STREAMING"),
            ThreadStatus::Running
        ); // case insensitive
    }

    #[test]
    fn test_infer_status_waiting() {
        assert_eq!(
            infer_status_from_agent_state("waiting"),
            ThreadStatus::Waiting
        );
        assert_eq!(
            infer_status_from_agent_state("awaiting_permission"),
            ThreadStatus::Waiting
        );
        assert_eq!(
            infer_status_from_agent_state("awaiting_input"),
            ThreadStatus::Waiting
        );
        assert_eq!(
            infer_status_from_agent_state("paused"),
            ThreadStatus::Waiting
        );
    }

    #[test]
    fn test_infer_status_done() {
        assert_eq!(infer_status_from_agent_state("done"), ThreadStatus::Done);
        assert_eq!(
            infer_status_from_agent_state("complete"),
            ThreadStatus::Done
        );
        assert_eq!(
            infer_status_from_agent_state("completed"),
            ThreadStatus::Done
        );
        assert_eq!(
            infer_status_from_agent_state("finished"),
            ThreadStatus::Done
        );
        assert_eq!(
            infer_status_from_agent_state("success"),
            ThreadStatus::Done
        );
    }

    #[test]
    fn test_infer_status_error() {
        assert_eq!(infer_status_from_agent_state("error"), ThreadStatus::Error);
        assert_eq!(
            infer_status_from_agent_state("failed"),
            ThreadStatus::Error
        );
        assert_eq!(
            infer_status_from_agent_state("failure"),
            ThreadStatus::Error
        );
    }

    #[test]
    fn test_infer_status_idle() {
        assert_eq!(infer_status_from_agent_state("idle"), ThreadStatus::Idle);
        assert_eq!(infer_status_from_agent_state("ready"), ThreadStatus::Idle);
        assert_eq!(infer_status_from_agent_state(""), ThreadStatus::Idle);
    }

    #[test]
    fn test_infer_status_unknown() {
        assert_eq!(
            infer_status_from_agent_state("unknown_state"),
            ThreadStatus::Idle
        );
        assert_eq!(
            infer_status_from_agent_state("xyz"),
            ThreadStatus::Idle
        );
    }

    // -------------------- derive_repository Tests --------------------

    #[test]
    fn test_derive_repository_empty() {
        assert_eq!(derive_repository(""), "");
    }

    #[test]
    fn test_derive_repository_macos_path() {
        assert_eq!(derive_repository("/Users/sam/api"), "~/api");
        assert_eq!(derive_repository("/Users/sam/projects/myapp"), "~/projects/myapp");
        assert_eq!(derive_repository("/Users/john"), "~");
    }

    #[test]
    fn test_derive_repository_linux_path() {
        assert_eq!(derive_repository("/home/sam/api"), "~/api");
        assert_eq!(derive_repository("/home/user/projects/myapp"), "~/projects/myapp");
        assert_eq!(derive_repository("/home/john"), "~");
    }

    #[test]
    fn test_derive_repository_other_path() {
        assert_eq!(derive_repository("/var/www/app"), "/var/www/app");
        assert_eq!(derive_repository("/opt/project"), "/opt/project");
    }

    #[test]
    fn test_derive_repository_long_path() {
        let long_path = "/Users/sam/very/deep/nested/directory/structure/here";
        let result = derive_repository(long_path);
        assert!(result.len() <= 35); // Should be truncated
        assert!(result.starts_with("..."));
    }

    // -------------------- compute_duration Tests --------------------

    #[test]
    fn test_compute_duration_seconds() {
        let from = Utc::now() - Duration::seconds(30);
        let result = compute_duration(from);
        assert_eq!(result, "30s");
    }

    #[test]
    fn test_compute_duration_minutes() {
        let from = Utc::now() - Duration::minutes(12);
        let result = compute_duration(from);
        assert_eq!(result, "12m");
    }

    #[test]
    fn test_compute_duration_hours() {
        let from = Utc::now() - Duration::hours(3);
        let result = compute_duration(from);
        assert_eq!(result, "3h");
    }

    #[test]
    fn test_compute_duration_days() {
        let from = Utc::now() - Duration::days(2);
        let result = compute_duration(from);
        assert_eq!(result, "2d");
    }

    #[test]
    fn test_compute_duration_future() {
        let from = Utc::now() + Duration::hours(1);
        let result = compute_duration(from);
        assert_eq!(result, "0s");
    }

    #[test]
    fn test_compute_duration_just_now() {
        let from = Utc::now();
        let result = compute_duration(from);
        // Should be "0s" or "<1m" depending on timing
        assert!(result == "0s" || result == "<1m");
    }
}
