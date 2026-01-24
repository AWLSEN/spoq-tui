//! Trait abstractions for dependency injection and testability.
//!
//! This module provides trait-based abstractions for core functionality,
//! enabling dependency injection, mocking, and better testability.
//!
//! # Traits
//!
//! - [`HttpClient`] - HTTP client operations (GET, POST, streaming)
//! - [`WebSocketConnection`] - WebSocket connection management
//! - [`CredentialsProvider`] - Credentials storage and retrieval
//! - [`SseParser`] - Server-Sent Events parsing
//! - [`TerminalBackend`] - Terminal rendering backend

pub mod credentials;
pub mod http;
pub mod sse;
pub mod terminal;
pub mod websocket;

pub use credentials::{CredentialsError, CredentialsProvider};
pub use http::{Headers, HttpClient, HttpError, Response};
pub use sse::{SseParseError as TraitSseParseError, SseParserTrait};
pub use terminal::{TerminalBackend, TerminalError};
pub use websocket::{WebSocketConnection, WsError as TraitWsError};
