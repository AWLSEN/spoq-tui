//! Spoq TUI - A terminal user interface for AI conversations
//!
//! This library provides a terminal-based user interface for AI conversations.
//! It exposes a carefully curated public API while keeping implementation details
//! hidden from documentation.
//!
//! # Public API
//!
//! The following modules are part of the public API and documented:
//!
//! - [`models`] - Core data types (Thread, Message, etc.)
//! - [`app`] - Application state and core types (App, Screen, Focus)
//! - [`cache`] - Thread and message caching
//! - [`state`] - Application state management
//! - [`ui`] - User interface rendering
//! - [`adapters`] - Trait implementations for dependency injection
//! - [`sse`] - Server-sent events parsing
//! - [`widgets`] - Reusable UI widgets
//! - [`prelude`] - Convenient re-exports
//!
//! # Internal Modules
//!
//! The following modules are internal implementation details. They are accessible
//! for the binary and tests but are not part of the stable public API:
//!
//! - cli, startup, terminal, update - Binary entry point support
//! - auth, websocket, conductor - Backend communication
//! - debug, events - Development and event handling
//! - markdown, input_history - Text processing utilities

// ============================================================================
// Public API - Available to external consumers and integration tests
// ============================================================================

/// Core data types: Thread, Message, ThreadType, etc.
pub mod models;

/// Application state and logic (App, Screen, Focus, AppMessage)
pub mod app;

/// Thread and message caching
pub mod cache;

/// Application state management (SessionState, DashboardState, etc.)
pub mod state;

/// User interface rendering
pub mod ui;

/// Trait implementations for dependency injection
pub mod adapters;

/// Server-sent events parsing
pub mod sse;

/// Reusable UI widgets (TextAreaInput)
pub mod widgets;

/// Convenient re-exports of commonly used types
pub mod prelude;

// ============================================================================
// Internal modules - Required by main.rs but not part of stable public API
// ============================================================================
// These are marked #[doc(hidden)] to exclude from documentation while
// remaining accessible to the binary crate.

/// Clipboard image reading and file-based image ingestion
#[doc(hidden)]
pub mod clipboard;

/// Authentication and credential management
#[doc(hidden)]
pub mod auth;

/// CLI argument parsing and command handling
#[doc(hidden)]
pub mod cli;

/// CLI output formatting (non-TUI output)
#[doc(hidden)]
pub mod cli_output;

/// Conductor client for backend communication
#[doc(hidden)]
pub mod conductor;

/// Debug system for development
#[doc(hidden)]
pub mod debug;

/// Domain types and business logic
#[doc(hidden)]
pub mod domain;

/// Error types (internal)
#[doc(hidden)]
pub mod error;

/// Event types for SSE/WebSocket messages
#[doc(hidden)]
pub mod events;

/// Health check functionality
#[doc(hidden)]
pub mod health_check;

/// Input handling utilities
#[doc(hidden)]
pub mod input;

/// Input history management
#[doc(hidden)]
pub mod input_history;

/// Markdown rendering utilities
#[doc(hidden)]
pub mod markdown;

/// Rendered lines cache for virtualization
#[doc(hidden)]
pub mod rendered_lines_cache;

/// Setup flow for first-time configuration
#[doc(hidden)]
pub mod setup;

/// Startup sequence and preflight checks
#[doc(hidden)]
pub mod startup;

/// System information gathering
#[doc(hidden)]
pub mod system;

/// Terminal management (setup, cleanup, panic hooks)
#[doc(hidden)]
pub mod terminal;

/// Trait abstractions for dependency injection
#[doc(hidden)]
pub mod traits;

/// Application update checking and installation
#[doc(hidden)]
pub mod update;

/// View state management (internal)
#[doc(hidden)]
pub mod view_state;

/// WebSocket client for real-time communication
#[doc(hidden)]
pub mod websocket;

/// Credential change detection and auto-sync system
#[doc(hidden)]
pub mod credential_watcher;

/// Native OS notifications for task completion
#[doc(hidden)]
pub mod notifications;
