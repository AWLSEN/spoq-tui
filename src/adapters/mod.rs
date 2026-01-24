//! Concrete implementations of trait abstractions.
//!
//! This module provides production-ready adapters that wrap existing code
//! and implement the traits defined in `crate::traits`. These adapters enable
//! dependency injection and testability while maintaining the same functionality.
//!
//! # Adapters
//!
//! - [`ReqwestHttpClient`] - HTTP client using reqwest
//! - [`TungsteniteWsConnection`] - WebSocket using tokio-tungstenite
//! - [`FileCredentialsProvider`] - File-based credentials storage
//! - [`DefaultSseParser`] - SSE parser wrapping the existing implementation
//!
//! # Mock Implementations
//!
//! The [`mock`] submodule provides test doubles for all adapters:
//! - [`mock::MockHttpClient`] - Configurable HTTP responses
//! - [`mock::MockWebSocket`] - Message injection for testing
//! - [`mock::InMemoryCredentials`] - In-memory credential storage

pub mod default_sse;
pub mod file_credentials;
pub mod mock;
pub mod reqwest_http;
pub mod tungstenite_ws;

pub use default_sse::DefaultSseParser;
pub use file_credentials::FileCredentialsProvider;
pub use mock::{InMemoryCredentials, MockHttpClient, MockWebSocket};
pub use reqwest_http::ReqwestHttpClient;
pub use tungstenite_ws::TungsteniteWsConnection;
