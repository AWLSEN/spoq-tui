//! Session-level view state
//!
//! This module provides a view-only struct for session-level information
//! that UI components need for rendering without accessing App.

/// Session-level view state for UI rendering
#[derive(Debug, Clone, Default)]
pub struct SessionViewState {
    /// Number of active skills
    pub skills_count: usize,
    /// Context tokens used (from context_compacted events)
    pub context_tokens_used: Option<u32>,
    /// Context token limit (max capacity)
    pub context_token_limit: Option<u32>,
    /// Whether there's a pending permission request
    pub has_pending_permission: bool,
    /// Whether OAuth is required
    pub needs_oauth: bool,
}

impl SessionViewState {
    /// Create a new session view state
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a session view state with given values
    pub fn with_values(
        skills_count: usize,
        context_tokens_used: Option<u32>,
        context_token_limit: Option<u32>,
        has_pending_permission: bool,
        needs_oauth: bool,
    ) -> Self {
        Self {
            skills_count,
            context_tokens_used,
            context_token_limit,
            has_pending_permission,
            needs_oauth,
        }
    }

    /// Get context usage as a percentage (0-100)
    pub fn context_percentage(&self) -> Option<u32> {
        match (self.context_tokens_used, self.context_token_limit) {
            (Some(used), Some(limit)) if limit > 0 => {
                Some((used as f64 / limit as f64 * 100.0).round() as u32)
            }
            _ => None,
        }
    }

    /// Check if context is nearly full (> 90%)
    pub fn context_nearly_full(&self) -> bool {
        self.context_percentage().map(|p| p > 90).unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_view_state_default() {
        let state = SessionViewState::default();
        assert_eq!(state.skills_count, 0);
        assert!(state.context_tokens_used.is_none());
        assert!(state.context_token_limit.is_none());
        assert!(!state.has_pending_permission);
        assert!(!state.needs_oauth);
    }

    #[test]
    fn test_session_view_state_with_values() {
        let state = SessionViewState::with_values(3, Some(45000), Some(100000), true, false);
        assert_eq!(state.skills_count, 3);
        assert_eq!(state.context_tokens_used, Some(45000));
        assert_eq!(state.context_token_limit, Some(100000));
        assert!(state.has_pending_permission);
        assert!(!state.needs_oauth);
    }

    #[test]
    fn test_context_percentage() {
        let state = SessionViewState::with_values(0, Some(50000), Some(100000), false, false);
        assert_eq!(state.context_percentage(), Some(50));

        let state = SessionViewState::with_values(0, Some(33333), Some(100000), false, false);
        assert_eq!(state.context_percentage(), Some(33));

        let state = SessionViewState::with_values(0, None, Some(100000), false, false);
        assert_eq!(state.context_percentage(), None);

        let state = SessionViewState::with_values(0, Some(50000), None, false, false);
        assert_eq!(state.context_percentage(), None);

        let state = SessionViewState::with_values(0, Some(50000), Some(0), false, false);
        assert_eq!(state.context_percentage(), None);
    }

    #[test]
    fn test_context_nearly_full() {
        let state = SessionViewState::with_values(0, Some(50000), Some(100000), false, false);
        assert!(!state.context_nearly_full());

        let state = SessionViewState::with_values(0, Some(95000), Some(100000), false, false);
        assert!(state.context_nearly_full());

        let state = SessionViewState::with_values(0, None, None, false, false);
        assert!(!state.context_nearly_full());
    }
}
