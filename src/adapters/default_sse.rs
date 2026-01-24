//! Default SSE parser adapter.
//!
//! This module provides an SSE parser implementation that wraps the existing
//! `SseParser` and implements the [`SseParserTrait`].

use crate::sse::{SseEvent, SseParser};
use crate::traits::{SseParserTrait, TraitSseParseError as SseParseError};

/// Default SSE parser adapter.
///
/// This adapter wraps the existing [`SseParser`] and provides a trait-based
/// interface. The underlying parser is already stateful and handles
/// line accumulation and event emission.
///
/// # Example
///
/// ```ignore
/// use spoq::adapters::DefaultSseParser;
/// use spoq::traits::SseParserTrait;
///
/// let mut parser = DefaultSseParser::new();
///
/// // Feed lines from an SSE stream
/// parser.feed_line("event: content")?;
/// parser.feed_line(r#"data: {"text": "Hello"}"#)?;
///
/// // Empty line triggers event emission
/// if let Some(event) = parser.feed_line("")? {
///     println!("Received event: {:?}", event);
/// }
/// ```
#[derive(Debug, Default)]
pub struct DefaultSseParser {
    inner: SseParser,
}

impl DefaultSseParser {
    /// Create a new SSE parser.
    pub fn new() -> Self {
        Self {
            inner: SseParser::new(),
        }
    }

    /// Get a reference to the inner parser.
    pub fn inner(&self) -> &SseParser {
        &self.inner
    }

    /// Get a mutable reference to the inner parser.
    pub fn inner_mut(&mut self) -> &mut SseParser {
        &mut self.inner
    }
}

impl SseParserTrait for DefaultSseParser {
    fn feed_line(&mut self, line: &str) -> Result<Option<SseEvent>, SseParseError> {
        self.inner.feed_line(line).map_err(SseParseError::from)
    }

    fn reset(&mut self) {
        self.inner.reset()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sse::SseEventMeta;

    #[test]
    fn test_default_sse_parser_new() {
        let parser = DefaultSseParser::new();
        let _ = parser.inner();
    }

    #[test]
    fn test_default_sse_parser_default() {
        let parser = DefaultSseParser::default();
        let _ = parser.inner();
    }

    #[test]
    fn test_feed_line_content_event() {
        let mut parser = DefaultSseParser::new();

        // Feed event type
        let result = parser.feed_line("event: content");
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());

        // Feed data
        let result = parser.feed_line(r#"data: {"text": "Hello"}"#);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());

        // Empty line triggers event emission
        let result = parser.feed_line("");
        assert!(result.is_ok());

        let event = result.unwrap();
        assert!(event.is_some());

        match event.unwrap() {
            SseEvent::Content { text, meta } => {
                assert_eq!(text, "Hello");
                assert_eq!(meta, SseEventMeta::default());
            }
            _ => panic!("Expected Content event"),
        }
    }

    #[test]
    fn test_feed_line_done_event() {
        let mut parser = DefaultSseParser::new();

        parser.feed_line("event: done").unwrap();
        let event = parser.feed_line("").unwrap();

        assert!(matches!(event, Some(SseEvent::Done)));
    }

    #[test]
    fn test_feed_line_ping_event() {
        let mut parser = DefaultSseParser::new();

        parser.feed_line("event: ping").unwrap();
        let event = parser.feed_line("").unwrap();

        assert!(matches!(event, Some(SseEvent::Ping)));
    }

    #[test]
    fn test_feed_line_comment_ignored() {
        let mut parser = DefaultSseParser::new();

        // Comments should be ignored
        let result = parser.feed_line(": this is a comment");
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());

        // Empty line after comment should not emit anything
        let result = parser.feed_line("");
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_reset() {
        let mut parser = DefaultSseParser::new();

        // Start accumulating an event
        parser.feed_line("event: content").unwrap();
        parser.feed_line(r#"data: {"text": "Hello"}"#).unwrap();

        // Reset
        parser.reset();

        // Empty line should not emit anything
        let event = parser.feed_line("").unwrap();
        assert!(event.is_none());
    }

    #[test]
    fn test_multiple_events() {
        let mut parser = DefaultSseParser::new();
        let mut events = Vec::new();

        // First event
        parser.feed_line("event: content").unwrap();
        parser.feed_line(r#"data: {"text": "First"}"#).unwrap();
        if let Some(event) = parser.feed_line("").unwrap() {
            events.push(event);
        }

        // Second event
        parser.feed_line("event: content").unwrap();
        parser.feed_line(r#"data: {"text": "Second"}"#).unwrap();
        if let Some(event) = parser.feed_line("").unwrap() {
            events.push(event);
        }

        // Done event
        parser.feed_line("event: done").unwrap();
        if let Some(event) = parser.feed_line("").unwrap() {
            events.push(event);
        }

        assert_eq!(events.len(), 3);

        match &events[0] {
            SseEvent::Content { text, .. } => assert_eq!(text, "First"),
            _ => panic!("Expected Content event"),
        }

        match &events[1] {
            SseEvent::Content { text, .. } => assert_eq!(text, "Second"),
            _ => panic!("Expected Content event"),
        }

        assert!(matches!(events[2], SseEvent::Done));
    }

    #[test]
    fn test_error_on_invalid_json() {
        let mut parser = DefaultSseParser::new();

        parser.feed_line("event: content").unwrap();
        parser.feed_line("data: not valid json").unwrap();

        let result = parser.feed_line("");
        assert!(result.is_err());
        assert!(matches!(result, Err(SseParseError::InvalidJson { .. })));
    }

    #[test]
    fn test_inner_mut() {
        let mut parser = DefaultSseParser::new();
        let inner = parser.inner_mut();
        inner.reset();
    }

    #[test]
    fn test_with_metadata() {
        let mut parser = DefaultSseParser::new();

        // Backend format with type in JSON
        parser
            .feed_line(r#"data: {"type":"content","seq":1,"timestamp":1736956800000,"session_id":"sess-abc","thread_id":"thread-123","data":"Hello"}"#)
            .unwrap();

        let event = parser.feed_line("").unwrap();
        assert!(event.is_some());

        match event.unwrap() {
            SseEvent::Content { text, meta } => {
                assert_eq!(text, "Hello");
                assert_eq!(meta.seq, Some(1));
                assert_eq!(meta.timestamp, Some(1736956800000));
                assert_eq!(meta.session_id, Some("sess-abc".to_string()));
                assert_eq!(meta.thread_id, Some("thread-123".to_string()));
            }
            _ => panic!("Expected Content event"),
        }
    }
}
