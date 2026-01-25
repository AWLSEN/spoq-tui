//! View state module for decoupling UI rendering from application state.
//!
//! This module provides view-only data structures that UI components can use
//! without importing the `App` struct, breaking the circular dependency between
//! `app` and `ui` modules.
//!
//! ## Architecture
//!
//! The key insight is that UI rendering is a pure function: data in -> pixels out.
//! By extracting the data that UI needs into a separate `AppViewState` struct,
//! we can make UI components truly functional and independent of `App`.
//!
//! ```text
//! ┌─────────────────┐
//! │      App        │
//! │  (owns state)   │
//! └────────┬────────┘
//!          │ view_state()
//!          ▼
//! ┌─────────────────┐
//! │  AppViewState   │
//! │  (borrows data) │
//! └────────┬────────┘
//!          │
//!          ▼
//! ┌─────────────────┐
//! │       UI        │
//! │ (pure rendering)│
//! └─────────────────┘
//! ```
//!
//! ## Components
//!
//! - [`AppViewState`]: Main view state struct containing all data for UI rendering
//! - [`SystemStats`]: System statistics (CPU, RAM, connection status)
//! - [`SessionViewState`]: Session-level view data (skills, context tokens)
//! - [`DashboardViewState`]: Dashboard-specific view data
//! - [`ScrollState`]: Scroll position and viewport info
//! - [`StreamingState`]: Current streaming status

mod app_view;
pub mod dashboard_view;
mod scroll_state;
mod session_view;
mod streaming_state;
mod system_stats;

// Re-export all public types
pub use app_view::AppViewState;
pub use dashboard_view::{
    DashboardViewState, OverlayState, Progress, RenderContext, Theme, ThreadView,
};
pub use scroll_state::ScrollState;
pub use session_view::SessionViewState;
pub use streaming_state::StreamingState;
pub use system_stats::SystemStats;
