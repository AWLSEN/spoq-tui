//! Dashboard UI module
//!
//! Provides the multi-thread dashboard view components for managing
//! multiple concurrent agent threads.

mod context;
pub mod header;

pub use context::{
    FilterState, OverlayState, Progress, RenderContext, SystemStats, Theme, ThreadMode, ThreadView,
};
