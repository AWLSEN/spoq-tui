use crate::models::{Message, StreamRequest, Thread};

pub const CONDUCTOR_BASE_URL: &str = "http://100.80.115.93:8000";

pub struct ConductorClient {
    pub base_url: String,
}

impl ConductorClient {
    pub fn new() -> Self {
        Self {
            base_url: CONDUCTOR_BASE_URL.to_string(),
        }
    }

    pub fn with_base_url(base_url: String) -> Self {
        Self { base_url }
    }

    // Stubbed - will implement later with actual HTTP calls
    pub fn stream(&self, _request: &StreamRequest) -> StubStream {
        StubStream::new()
    }

    pub fn get_thread(&self, _thread_id: &str) -> Option<Thread> {
        // Stub: return None for now
        None
    }

    pub fn get_recent_messages(&self) -> Vec<Message> {
        // Stub: return empty vec for now
        Vec::new()
    }

    pub fn cancel(&self, _session_id: &str) {
        // Stub: no-op for now
    }
}

// Placeholder for future SSE stream
pub struct StubStream {
    // Will hold actual stream state later
}

impl StubStream {
    pub fn new() -> Self {
        Self {}
    }
}
