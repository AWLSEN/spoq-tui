//! Session-level state management
//!
//! SessionState contains information that persists at the session level,
//! not per-thread. This includes active skills, context usage tracking,
//! and OAuth requirements.
//!
//! Note: Pending permissions are stored per-thread in DashboardState.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::Instant;

/// A permission request waiting for user approval
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PermissionRequest {
    /// Unique ID for this permission request (for responding to backend)
    pub permission_id: String,
    /// Thread ID this request belongs to (optional for backwards compatibility)
    #[serde(default)]
    pub thread_id: Option<String>,
    /// Tool that requires permission
    pub tool_name: String,
    /// Description of what the tool wants to do
    pub description: String,
    /// Additional context about the request (e.g., file path, command)
    pub context: Option<String>,
    /// Raw tool input parameters for preview display
    #[serde(default)]
    pub tool_input: Option<serde_json::Value>,
    /// Timestamp when the request was received (for timeout tracking)
    /// Not serialized as it's runtime-only state
    #[serde(skip)]
    #[serde(default = "Instant::now")]
    pub received_at: Instant,
}

/// Session-level state that persists across threads
///
/// This contains information that is relevant to the entire session,
/// not specific to any single thread. It tracks things like:
/// - Active skills that have been loaded
/// - Context token usage from compaction events
/// - OAuth requirements for certain skills
/// - Tools that have been allowed for the session ("allow always")
///
/// Note: Pending permissions are stored per-thread in DashboardState.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionState {
    /// Active skills loaded in this session
    pub skills: Vec<String>,

    /// Context tokens used (from context_compacted events)
    /// None if no compaction has occurred yet
    pub context_tokens_used: Option<u32>,

    /// Context token limit (max capacity)
    pub context_token_limit: Option<u32>,

    /// OAuth requirement: (provider, skill_name)
    /// Set when a skill requires OAuth authentication
    pub oauth_required: Option<(String, String)>,

    /// OAuth consent URL for opening in browser
    pub oauth_url: Option<String>,

    /// Tools that have been allowed for the session ("allow always")
    /// When a user presses 'a' on a permission prompt, the tool is added here
    #[serde(default)]
    pub allowed_tools: HashSet<String>,
}

impl SessionState {
    /// Create a new empty SessionState
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a skill to the active skills list
    pub fn add_skill(&mut self, skill: String) {
        if !self.skills.contains(&skill) {
            self.skills.push(skill);
        }
    }

    /// Remove a skill from the active skills list
    pub fn remove_skill(&mut self, skill: &str) {
        self.skills.retain(|s| s != skill);
    }

    /// Check if a skill is active
    pub fn has_skill(&self, skill: &str) -> bool {
        self.skills.iter().any(|s| s == skill)
    }

    /// Update context tokens used
    pub fn set_context_tokens(&mut self, tokens: u32) {
        self.context_tokens_used = Some(tokens);
    }

    /// Set context token limit
    pub fn set_context_token_limit(&mut self, limit: u32) {
        self.context_token_limit = Some(limit);
    }

    /// Set OAuth consent URL
    pub fn set_oauth_url(&mut self, url: String) {
        self.oauth_url = Some(url);
    }

    /// Clear OAuth URL (after opening or when no longer needed)
    pub fn clear_oauth_url(&mut self) {
        self.oauth_url = None;
    }

    /// Set OAuth requirement
    pub fn set_oauth_required(&mut self, provider: String, skill_name: String) {
        self.oauth_required = Some((provider, skill_name));
    }

    /// Clear OAuth requirement (after authentication)
    pub fn clear_oauth_required(&mut self) {
        self.oauth_required = None;
    }

    /// Check if OAuth is required
    pub fn needs_oauth(&self) -> bool {
        self.oauth_required.is_some()
    }

    /// Add a tool to the allowed tools set (for "allow always" behavior)
    pub fn allow_tool(&mut self, tool_name: String) {
        self.allowed_tools.insert(tool_name);
    }

    /// Check if a tool is allowed (user has previously selected "allow always")
    pub fn is_tool_allowed(&self, tool_name: &str) -> bool {
        self.allowed_tools.contains(tool_name)
    }

    /// Remove a tool from the allowed set
    pub fn disallow_tool(&mut self, tool_name: &str) {
        self.allowed_tools.remove(tool_name);
    }

    /// Reset all session state (for new session)
    pub fn reset(&mut self) {
        self.skills.clear();
        self.context_tokens_used = None;
        self.context_token_limit = None;
        self.oauth_required = None;
        self.oauth_url = None;
        self.allowed_tools.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_state_new() {
        let state = SessionState::new();
        assert!(state.skills.is_empty());
        assert!(state.context_tokens_used.is_none());
        assert!(state.oauth_required.is_none());
        assert!(state.allowed_tools.is_empty());
    }

    #[test]
    fn test_session_state_default() {
        let state = SessionState::default();
        assert!(state.skills.is_empty());
        assert!(state.context_tokens_used.is_none());
        assert!(state.allowed_tools.is_empty());
    }

    #[test]
    fn test_add_skill() {
        let mut state = SessionState::new();
        state.add_skill("commit".to_string());
        assert!(state.has_skill("commit"));
        assert_eq!(state.skills.len(), 1);
    }

    #[test]
    fn test_add_skill_deduplication() {
        let mut state = SessionState::new();
        state.add_skill("commit".to_string());
        state.add_skill("commit".to_string());
        assert_eq!(state.skills.len(), 1);
    }

    #[test]
    fn test_add_multiple_skills() {
        let mut state = SessionState::new();
        state.add_skill("commit".to_string());
        state.add_skill("review".to_string());
        state.add_skill("lint".to_string());
        assert_eq!(state.skills.len(), 3);
        assert!(state.has_skill("commit"));
        assert!(state.has_skill("review"));
        assert!(state.has_skill("lint"));
    }

    #[test]
    fn test_remove_skill() {
        let mut state = SessionState::new();
        state.add_skill("commit".to_string());
        state.add_skill("review".to_string());
        state.remove_skill("commit");
        assert!(!state.has_skill("commit"));
        assert!(state.has_skill("review"));
        assert_eq!(state.skills.len(), 1);
    }

    #[test]
    fn test_remove_nonexistent_skill() {
        let mut state = SessionState::new();
        state.add_skill("commit".to_string());
        state.remove_skill("nonexistent");
        assert_eq!(state.skills.len(), 1);
        assert!(state.has_skill("commit"));
    }

    #[test]
    fn test_has_skill_false() {
        let state = SessionState::new();
        assert!(!state.has_skill("commit"));
    }

    #[test]
    fn test_set_context_tokens() {
        let mut state = SessionState::new();
        assert!(state.context_tokens_used.is_none());
        state.set_context_tokens(1000);
        assert_eq!(state.context_tokens_used, Some(1000));
    }

    #[test]
    fn test_update_context_tokens() {
        let mut state = SessionState::new();
        state.set_context_tokens(1000);
        state.set_context_tokens(2000);
        assert_eq!(state.context_tokens_used, Some(2000));
    }

    #[test]
    fn test_oauth_required() {
        let mut state = SessionState::new();
        assert!(!state.needs_oauth());

        state.set_oauth_required("github".to_string(), "git-commit".to_string());
        assert!(state.needs_oauth());
        assert_eq!(
            state.oauth_required,
            Some(("github".to_string(), "git-commit".to_string()))
        );
    }

    #[test]
    fn test_clear_oauth_required() {
        let mut state = SessionState::new();
        state.set_oauth_required("github".to_string(), "git-commit".to_string());
        assert!(state.needs_oauth());

        state.clear_oauth_required();
        assert!(!state.needs_oauth());
        assert!(state.oauth_required.is_none());
    }

    #[test]
    fn test_reset() {
        let mut state = SessionState::new();
        state.add_skill("commit".to_string());
        state.add_skill("review".to_string());
        state.set_context_tokens(5000);
        state.set_oauth_required("github".to_string(), "skill".to_string());
        state.allow_tool("Bash".to_string());

        state.reset();

        assert!(state.skills.is_empty());
        assert!(state.context_tokens_used.is_none());
        assert!(state.oauth_required.is_none());
        assert!(state.allowed_tools.is_empty());
    }

    #[test]
    fn test_permission_request_equality() {
        let now = Instant::now();
        let req1 = PermissionRequest {
            permission_id: "perm-a".to_string(),
            tool_name: "Bash".to_string(),
            description: "Run command".to_string(),
            context: Some("context".to_string()),
            tool_input: None,
            received_at: now,
        };
        let req2 = PermissionRequest {
            permission_id: "perm-a".to_string(),
            tool_name: "Bash".to_string(),
            description: "Run command".to_string(),
            context: Some("context".to_string()),
            tool_input: None,
            received_at: now,
        };
        let req3 = PermissionRequest {
            permission_id: "perm-b".to_string(),
            tool_name: "Read".to_string(),
            description: "Read file".to_string(),
            context: None,
            tool_input: None,
            received_at: now,
        };

        assert_eq!(req1, req2);
        assert_ne!(req1, req3);
    }

    // ============= Allowed Tools Tests =============

    #[test]
    fn test_allow_tool() {
        let mut state = SessionState::new();
        assert!(!state.is_tool_allowed("Bash"));

        state.allow_tool("Bash".to_string());
        assert!(state.is_tool_allowed("Bash"));
    }

    #[test]
    fn test_allow_tool_deduplication() {
        let mut state = SessionState::new();
        state.allow_tool("Bash".to_string());
        state.allow_tool("Bash".to_string());
        assert_eq!(state.allowed_tools.len(), 1);
    }

    #[test]
    fn test_allow_multiple_tools() {
        let mut state = SessionState::new();
        state.allow_tool("Bash".to_string());
        state.allow_tool("Read".to_string());
        state.allow_tool("Write".to_string());

        assert!(state.is_tool_allowed("Bash"));
        assert!(state.is_tool_allowed("Read"));
        assert!(state.is_tool_allowed("Write"));
        assert!(!state.is_tool_allowed("Edit"));
    }

    #[test]
    fn test_disallow_tool() {
        let mut state = SessionState::new();
        state.allow_tool("Bash".to_string());
        assert!(state.is_tool_allowed("Bash"));

        state.disallow_tool("Bash");
        assert!(!state.is_tool_allowed("Bash"));
    }

    #[test]
    fn test_disallow_nonexistent_tool() {
        let mut state = SessionState::new();
        // Should not panic when disallowing a tool that wasn't allowed
        state.disallow_tool("Nonexistent");
        assert!(!state.is_tool_allowed("Nonexistent"));
    }

    #[test]
    fn test_allowed_tools_persists() {
        let mut state = SessionState::new();
        state.allow_tool("Bash".to_string());

        // Clone state (simulates persistence)
        let state2 = state.clone();
        assert!(state2.is_tool_allowed("Bash"));
    }

    #[test]
    fn test_session_state_serialization() {
        let mut state = SessionState::new();
        state.add_skill("commit".to_string());
        state.set_context_tokens(1234);

        let json = serde_json::to_string(&state).expect("Failed to serialize");
        let deserialized: SessionState =
            serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(state.skills, deserialized.skills);
        assert_eq!(state.context_tokens_used, deserialized.context_tokens_used);
    }

    #[test]
    fn test_set_context_token_limit() {
        let mut state = SessionState::new();
        assert!(state.context_token_limit.is_none());

        state.set_context_token_limit(100_000);
        assert_eq!(state.context_token_limit, Some(100_000));

        state.set_context_token_limit(200_000);
        assert_eq!(state.context_token_limit, Some(200_000));
    }

    #[test]
    fn test_oauth_url_handling() {
        let mut state = SessionState::new();
        assert!(state.oauth_url.is_none());

        state.set_oauth_url("https://oauth.example.com/consent".to_string());
        assert_eq!(
            state.oauth_url,
            Some("https://oauth.example.com/consent".to_string())
        );

        state.clear_oauth_url();
        assert!(state.oauth_url.is_none());
    }

    #[test]
    fn test_reset_clears_all_new_fields() {
        let mut state = SessionState::new();
        state.add_skill("test-skill".to_string());
        state.set_context_tokens(5000);
        state.set_context_token_limit(100_000);
        state.set_oauth_url("https://example.com".to_string());
        state.set_oauth_required("github".to_string(), "skill".to_string());
        state.allow_tool("Bash".to_string());

        state.reset();

        assert!(state.skills.is_empty());
        assert!(state.context_tokens_used.is_none());
        assert!(state.context_token_limit.is_none());
        assert!(state.oauth_url.is_none());
        assert!(state.oauth_required.is_none());
        assert!(state.allowed_tools.is_empty());
    }

    #[test]
    fn test_context_tokens_and_limit_together() {
        let mut state = SessionState::new();
        state.set_context_tokens(45_000);
        state.set_context_token_limit(100_000);

        assert_eq!(state.context_tokens_used, Some(45_000));
        assert_eq!(state.context_token_limit, Some(100_000));
    }

    #[test]
    fn test_oauth_with_url() {
        let mut state = SessionState::new();
        state.set_oauth_required("google".to_string(), "calendar".to_string());
        state.set_oauth_url("https://accounts.google.com/consent".to_string());

        assert!(state.needs_oauth());
        assert_eq!(
            state.oauth_required,
            Some(("google".to_string(), "calendar".to_string()))
        );
        assert_eq!(
            state.oauth_url,
            Some("https://accounts.google.com/consent".to_string())
        );
    }
}
