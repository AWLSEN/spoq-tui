//! SSE (Server-Sent Events) stream parser
//!
//! Parses SSE format from the Conductor backend streaming API.
//! SSE format consists of:
//! - `event: <type>` - event type line
//! - `data: <json>` - data payload line
//! - Empty line - signals end of event
//! - Lines starting with `:` - comments (ignored)
//!
//! # Module structure
//! - `events` - Event type definitions (SseEvent enum, SseLine, SseParseError)
//! - `payloads` - Internal payload deserialization structs
//! - `parser` - Parsing logic (SseParser, parse_sse_line, parse_sse_event)

mod events;
mod parser;
mod payloads;

// Re-export public types
pub use events::{SseEvent, SseEventMeta, SseLine, SseParseError};
pub use parser::{parse_sse_event, parse_sse_line, SseParser};
