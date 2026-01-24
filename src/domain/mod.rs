//! Domain objects for the TUI application.
//!
//! This module contains well-defined domain objects extracted from the App struct
//! to improve testability, reduce coupling, and provide clear boundaries.
//!
//! ## Domain Objects
//!
//! - [`AuthenticationState`] - Authentication tokens and refresh logic
//! - [`ConnectionState`] - WebSocket connection status and retry state
//! - [`InputState`] - Text input, history, and folder picker state
//! - [`ScrollState`] - Scroll position and momentum scrolling state

pub mod auth;
pub mod connection;
pub mod input;
pub mod scroll;

pub use auth::AuthenticationState;
pub use connection::ConnectionState;
pub use input::InputState;
pub use scroll::ScrollState;
