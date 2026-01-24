//! SSE (Server-Sent Events) parser trait abstraction.
//!
//! Provides a trait-based abstraction for SSE parsing, enabling
//! dependency injection and mocking in tests.

use crate::sse::SseEvent;

/// SSE parsing errors.
#[derive(Debug, Clone, PartialEq)]
pub enum SseParseError {
    /// Unknown event type received
    UnknownEventType(String),
    /// Invalid JSON in data payload
    InvalidJson { event_type: String, source: String },
    /// Missing data for event
    MissingData { event_type: String },
    /// Incomplete event (waiting for more data)
    Incomplete,
    /// Other parsing error
    Other(String),
}

impl std::fmt::Display for SseParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SseParseError::UnknownEventType(t) => write!(f, "Unknown SSE event type: {}", t),
            SseParseError::InvalidJson { event_type, source } => {
                write!(f, "Invalid JSON for event '{}': {}", event_type, source)
            }
            SseParseError::MissingData { event_type } => {
                write!(f, "Missing data for event type: {}", event_type)
            }
            SseParseError::Incomplete => write!(f, "Incomplete SSE event"),
            SseParseError::Other(msg) => write!(f, "SSE parse error: {}", msg),
        }
    }
}

impl std::error::Error for SseParseError {}

impl From<crate::sse::SseParseError> for SseParseError {
    fn from(err: crate::sse::SseParseError) -> Self {
        match err {
            crate::sse::SseParseError::UnknownEventType(t) => SseParseError::UnknownEventType(t),
            crate::sse::SseParseError::InvalidJson { event_type, source } => {
                SseParseError::InvalidJson { event_type, source }
            }
            crate::sse::SseParseError::MissingData { event_type } => {
                SseParseError::MissingData { event_type }
            }
        }
    }
}

/// Trait for SSE (Server-Sent Events) parsing.
///
/// This trait abstracts SSE parsing to enable dependency injection
/// and mocking in tests. The parser is stateful - it accumulates
/// lines until a complete event can be emitted.
///
/// # SSE Format
///
/// SSE events consist of:
/// - `event: <type>` - event type line
/// - `data: <payload>` - data payload line(s)
/// - Empty line - signals end of event
/// - `: comment` - comments (ignored)
///
/// # Example
///
/// ```ignore
/// use spoq::traits::SseParserTrait;
///
/// fn process_stream<P: SseParserTrait>(parser: &mut P, line: &str) {
///     match parser.feed_line(line) {
///         Ok(Some(event)) => {
///             // Complete event received
///             handle_event(event);
///         }
///         Ok(None) => {
///             // Line consumed, waiting for more data
///         }
///         Err(e) => {
///             // Parse error
///             eprintln!("Parse error: {}", e);
///         }
///     }
/// }
/// ```
pub trait SseParserTrait: Send {
    /// Feed a line to the parser, potentially returning a complete event.
    ///
    /// # Arguments
    /// * `line` - A single line from the SSE stream (without trailing newline)
    ///
    /// # Returns
    /// - `Ok(Some(event))` if a complete event was parsed
    /// - `Ok(None)` if the line was consumed but no complete event yet
    /// - `Err(error)` if parsing failed
    fn feed_line(&mut self, line: &str) -> Result<Option<SseEvent>, SseParseError>;

    /// Reset the parser state.
    ///
    /// This clears any accumulated state, useful when starting a new stream
    /// or recovering from errors.
    fn reset(&mut self);
}

/// Wrapper to implement the trait for the existing SseParser.
impl SseParserTrait for crate::sse::SseParser {
    fn feed_line(&mut self, line: &str) -> Result<Option<SseEvent>, SseParseError> {
        crate::sse::SseParser::feed_line(self, line).map_err(SseParseError::from)
    }

    fn reset(&mut self) {
        crate::sse::SseParser::reset(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sse_parse_error_display() {
        assert_eq!(
            SseParseError::UnknownEventType("foo".to_string()).to_string(),
            "Unknown SSE event type: foo"
        );
        assert_eq!(
            SseParseError::InvalidJson {
                event_type: "content".to_string(),
                source: "expected value".to_string()
            }
            .to_string(),
            "Invalid JSON for event 'content': expected value"
        );
        assert_eq!(
            SseParseError::MissingData {
                event_type: "content".to_string()
            }
            .to_string(),
            "Missing data for event type: content"
        );
        assert_eq!(
            SseParseError::Incomplete.to_string(),
            "Incomplete SSE event"
        );
        assert_eq!(
            SseParseError::Other("unknown error".to_string()).to_string(),
            "SSE parse error: unknown error"
        );
    }

    #[test]
    fn test_sse_parse_error_clone() {
        let err = SseParseError::UnknownEventType("test".to_string());
        let cloned = err.clone();
        assert_eq!(err, cloned);
    }

    #[test]
    fn test_sse_parse_error_from_sse_module() {
        let sse_err = crate::sse::SseParseError::UnknownEventType("test".to_string());
        let trait_err: SseParseError = sse_err.into();
        assert_eq!(trait_err, SseParseError::UnknownEventType("test".to_string()));

        let sse_err = crate::sse::SseParseError::InvalidJson {
            event_type: "content".to_string(),
            source: "error".to_string(),
        };
        let trait_err: SseParseError = sse_err.into();
        assert_eq!(
            trait_err,
            SseParseError::InvalidJson {
                event_type: "content".to_string(),
                source: "error".to_string(),
            }
        );

        let sse_err = crate::sse::SseParseError::MissingData {
            event_type: "content".to_string(),
        };
        let trait_err: SseParseError = sse_err.into();
        assert_eq!(
            trait_err,
            SseParseError::MissingData {
                event_type: "content".to_string(),
            }
        );
    }

    #[test]
    fn test_sse_parse_error_implements_error_trait() {
        let err = SseParseError::Incomplete;
        let _: &dyn std::error::Error = &err;
    }

    #[test]
    fn test_sse_parser_trait_implementation() {
        use crate::sse::{SseEvent, SseEventMeta, SseParser};

        let mut parser = SseParser::new();

        // Test feed_line through trait
        let result = SseParserTrait::feed_line(&mut parser, "event: content");
        assert!(result.unwrap().is_none());

        let result = SseParserTrait::feed_line(&mut parser, r#"data: {"text": "Hello"}"#);
        assert!(result.unwrap().is_none());

        let result = SseParserTrait::feed_line(&mut parser, "");
        let event = result.unwrap().unwrap();
        assert_eq!(
            event,
            SseEvent::Content {
                text: "Hello".to_string(),
                meta: SseEventMeta::default(),
            }
        );

        // Test reset through trait
        SseParserTrait::feed_line(&mut parser, "event: content").unwrap();
        SseParserTrait::reset(&mut parser);
        let result = SseParserTrait::feed_line(&mut parser, "");
        assert!(result.unwrap().is_none());
    }
}
