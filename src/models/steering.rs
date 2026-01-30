//! Steering message state tracking for soft-interrupt flow

use chrono::{DateTime, Utc};

/// State of a queued steering message through its lifecycle
#[derive(Debug, Clone, PartialEq)]
pub enum SteeringMessageState {
    /// Message sent to backend, waiting for acknowledgment
    Queued,
    /// Backend acknowledged receipt (steering_queued received)
    Sent,
    /// Backend is interrupting current stream (steering_interrupting received)
    Interrupting,
    /// Backend is resuming with new process (steering_resuming received)
    Resuming,
    /// Resume completed successfully - ready to promote to message
    Completed,
    /// Resume failed with error
    Failed(String),
}

impl SteeringMessageState {
    /// Returns true if this state indicates active processing
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            Self::Queued | Self::Sent | Self::Interrupting | Self::Resuming
        )
    }

    /// Returns true if this is a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed(_))
    }

    /// Get display icon for this state
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Queued => "...",
            Self::Sent => "->",
            Self::Interrupting => "||",
            Self::Resuming => ">>",
            Self::Completed => "OK",
            Self::Failed(_) => "X",
        }
    }

    /// Get display text for this state
    pub fn display_text(&self) -> &'static str {
        match self {
            Self::Queued => "Queued",
            Self::Sent => "Waiting for boundary...",
            Self::Interrupting => "Interrupting...",
            Self::Resuming => "Resuming...",
            Self::Completed => "Applied",
            Self::Failed(_) => "Failed",
        }
    }
}

/// A steering message that has been queued but not yet promoted to a visible message
#[derive(Debug, Clone)]
pub struct QueuedSteeringMessage {
    /// Thread this steering applies to
    pub thread_id: String,
    /// The user's steering instruction
    pub instruction: String,
    /// When the steering was queued
    pub queued_at: DateTime<Utc>,
    /// Current state in the lifecycle
    pub state: SteeringMessageState,
}

impl QueuedSteeringMessage {
    /// Create a new queued steering message
    pub fn new(thread_id: String, instruction: String) -> Self {
        Self {
            thread_id,
            instruction,
            queued_at: Utc::now(),
            state: SteeringMessageState::Queued,
        }
    }

    /// Update state to the next lifecycle stage
    pub fn transition_to(&mut self, new_state: SteeringMessageState) {
        self.state = new_state;
    }

    /// Get a preview of the instruction (truncated if too long)
    pub fn preview(&self, max_len: usize) -> String {
        if self.instruction.len() > max_len {
            format!("{}...", &self.instruction[..max_len.saturating_sub(3)])
        } else {
            self.instruction.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_transitions() {
        let mut qs = QueuedSteeringMessage::new(
            "thread-1".to_string(),
            "test instruction".to_string(),
        );
        assert!(matches!(qs.state, SteeringMessageState::Queued));
        assert!(qs.state.is_active());

        qs.transition_to(SteeringMessageState::Sent);
        assert!(matches!(qs.state, SteeringMessageState::Sent));

        qs.transition_to(SteeringMessageState::Completed);
        assert!(qs.state.is_terminal());
    }

    #[test]
    fn test_preview_truncation() {
        let qs = QueuedSteeringMessage::new(
            "t".to_string(),
            "a".repeat(100),
        );
        let preview = qs.preview(20);
        assert!(preview.len() <= 20);
        assert!(preview.ends_with("..."));
    }
}
