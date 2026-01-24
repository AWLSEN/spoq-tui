//! Spoq TUI - A terminal user interface for AI conversations
//!
//! This library exposes modules for use in integration tests.

pub mod adapters;
pub mod app;
pub mod auth;
pub mod cli;
pub mod cache;
pub mod cli_output;
pub mod conductor;
pub mod debug;
pub mod domain;
pub mod error;
pub mod events;
pub mod health_check;
pub mod input_history;
pub mod markdown;
pub mod models;
pub mod rendered_lines_cache;
pub mod setup;
pub mod sse;
pub mod startup;
pub mod state;
pub mod system;
pub mod terminal;
pub mod traits;
pub mod ui;
pub mod view_state;
pub mod update;
pub mod websocket;
pub mod widgets;
