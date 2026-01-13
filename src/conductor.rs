use crate::models::{Message, StreamRequest, Thread};

#[allow(dead_code)]
pub const CONDUCTOR_BASE_URL: &str = "http://100.80.115.93:8000";

#[allow(dead_code)]
pub struct ConductorClient {
    pub base_url: String,
}

impl ConductorClient {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            base_url: CONDUCTOR_BASE_URL.to_string(),
        }
    }

    #[allow(dead_code)]
    pub fn with_base_url(base_url: String) -> Self {
        Self { base_url }
    }

    // Stubbed - will implement later with actual HTTP calls
    #[allow(dead_code)]
    pub fn stream(&self, _request: &StreamRequest) -> StubStream {
        StubStream::new()
    }

    #[allow(dead_code)]
    pub fn get_thread(&self, _thread_id: &str) -> Option<Thread> {
        // Stub: return None for now
        None
    }

    #[allow(dead_code)]
    pub fn get_recent_messages(&self) -> Vec<Message> {
        // Stub: return empty vec for now
        Vec::new()
    }

    #[allow(dead_code)]
    pub fn cancel(&self, _session_id: &str) {
        // Stub: no-op for now
    }
}

// Placeholder for future SSE stream
#[allow(dead_code)]
pub struct StubStream {
    // Will hold actual stream state later
}

impl StubStream {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::StreamRequest;

    #[test]
    fn test_conductor_client_new() {
        let client = ConductorClient::new();
        assert_eq!(client.base_url, CONDUCTOR_BASE_URL);
    }

    #[test]
    fn test_conductor_client_with_base_url() {
        let custom_url = "http://localhost:8080".to_string();
        let client = ConductorClient::with_base_url(custom_url.clone());
        assert_eq!(client.base_url, custom_url);
    }

    #[test]
    fn test_stream_returns_stub() {
        let client = ConductorClient::new();
        let request = StreamRequest::new("test".to_string());
        let _stream = client.stream(&request);
        // Just verify it doesn't panic - stub returns StubStream
    }

    #[test]
    fn test_get_thread_returns_none() {
        let client = ConductorClient::new();
        let result = client.get_thread("test-id");
        assert!(result.is_none());
    }

    #[test]
    fn test_get_recent_messages_returns_empty() {
        let client = ConductorClient::new();
        let messages = client.get_recent_messages();
        assert!(messages.is_empty());
    }

    #[test]
    fn test_cancel_doesnt_panic() {
        let client = ConductorClient::new();
        client.cancel("session-id");
        // Just verify it doesn't panic
    }

    #[test]
    fn test_stub_stream_new() {
        let _stream = StubStream::new();
        // Just verify construction doesn't panic
    }
}
