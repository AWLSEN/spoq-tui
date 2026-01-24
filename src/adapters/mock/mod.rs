//! Mock implementations for testing.
//!
//! This module provides mock implementations of all trait abstractions,
//! enabling unit testing without network dependencies or file system access.
//!
//! # Available Mocks
//!
//! - [`MockHttpClient`] - HTTP client with configurable responses
//! - [`MockWebSocket`] - WebSocket with message injection
//! - [`InMemoryCredentials`] - In-memory credential storage

pub mod credentials;
pub mod http;
pub mod websocket;

pub use credentials::InMemoryCredentials;
pub use http::MockHttpClient;
pub use websocket::MockWebSocket;
