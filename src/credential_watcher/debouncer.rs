//! Debouncing logic for credential change events.
//!
//! Will be fully implemented in Phase 3.

/// Manages debouncing of credential change events
#[derive(Debug, Default)]
pub struct Debouncer {
    pending_since: Option<std::time::Instant>,
    timer_active: bool,
}

impl Debouncer {
    pub fn new() -> Self {
        Self::default()
    }
}
