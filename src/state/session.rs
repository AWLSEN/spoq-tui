//! Session-level state management
//!
//! SessionState contains information that persists at the session level,
//! not per-thread. This includes active skills, context usage tracking,
//! pending permissions, and OAuth requirements.

use serde::{Deserialize, Serialize};

/// A permission request waiting for user approval
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PermissionRequest {
    /// Tool that requires permission
    pub tool_name: String,
    /// Description of what the tool wants to do
    pub description: String,
    /// Additional context about the request
    pub context: Option<String>,
}

/// Session-level state that persists across threads
///
/// This contains information that is relevant to the entire session,
/// not specific to any single thread. It tracks things like:
/// - Active skills that have been loaded
/// - Context token usage from compaction events
/// - Pending permission prompts that need user input
/// - OAuth requirements for certain skills
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionState {
    /// Active skills loaded in this session
    pub skills: Vec<String>,

    /// Context tokens used (from context_compacted events)
    /// None if no compaction has occurred yet
    pub context_tokens_used: Option<u32>,

    /// Current permission request awaiting user input
    pub pending_permission: Option<PermissionRequest>,

    /// OAuth requirement: (provider, skill_name)
    /// Set when a skill requires OAuth authentication
    pub oauth_required: Option<(String, String)>,
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

    /// Set a pending permission request
    pub fn set_pending_permission(&mut self, request: PermissionRequest) {
        self.pending_permission = Some(request);
    }

    /// Clear the pending permission request (after user responds)
    pub fn clear_pending_permission(&mut self) {
        self.pending_permission = None;
    }

    /// Check if there's a pending permission request
    pub fn has_pending_permission(&self) -> bool {
        self.pending_permission.is_some()
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

    /// Reset all session state (for new session)
    pub fn reset(&mut self) {
        self.skills.clear();
        self.context_tokens_used = None;
        self.pending_permission = None;
        self.oauth_required = None;
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
        assert!(state.pending_permission.is_none());
        assert!(state.oauth_required.is_none());
    }

    #[test]
    fn test_session_state_default() {
        let state = SessionState::default();
        assert!(state.skills.is_empty());
        assert!(state.context_tokens_used.is_none());
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
    fn test_pending_permission() {
        let mut state = SessionState::new();
        assert!(!state.has_pending_permission());

        let request = PermissionRequest {
            tool_name: "Bash".to_string(),
            description: "Run npm install".to_string(),
            context: None,
        };
        state.set_pending_permission(request.clone());

        assert!(state.has_pending_permission());
        assert_eq!(state.pending_permission, Some(request));
    }

    #[test]
    fn test_clear_pending_permission() {
        let mut state = SessionState::new();
        let request = PermissionRequest {
            tool_name: "Bash".to_string(),
            description: "Run npm install".to_string(),
            context: Some("Installing dependencies".to_string()),
        };
        state.set_pending_permission(request);
        assert!(state.has_pending_permission());

        state.clear_pending_permission();
        assert!(!state.has_pending_permission());
        assert!(state.pending_permission.is_none());
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
        state.set_pending_permission(PermissionRequest {
            tool_name: "Bash".to_string(),
            description: "Run command".to_string(),
            context: None,
        });
        state.set_oauth_required("github".to_string(), "skill".to_string());

        state.reset();

        assert!(state.skills.is_empty());
        assert!(state.context_tokens_used.is_none());
        assert!(state.pending_permission.is_none());
        assert!(state.oauth_required.is_none());
    }

    #[test]
    fn test_permission_request_equality() {
        let req1 = PermissionRequest {
            tool_name: "Bash".to_string(),
            description: "Run command".to_string(),
            context: Some("context".to_string()),
        };
        let req2 = PermissionRequest {
            tool_name: "Bash".to_string(),
            description: "Run command".to_string(),
            context: Some("context".to_string()),
        };
        let req3 = PermissionRequest {
            tool_name: "Read".to_string(),
            description: "Read file".to_string(),
            context: None,
        };

        assert_eq!(req1, req2);
        assert_ne!(req1, req3);
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
}
