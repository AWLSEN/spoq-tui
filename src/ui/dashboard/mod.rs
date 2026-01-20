//! Dashboard UI module
//!
//! Provides the multi-thread dashboard view components for managing
//! multiple concurrent agent threads.

mod context;
pub mod header;
pub mod overlay;
pub mod plan_card;
pub mod question_card;
pub mod states;
pub mod status_bar;
pub mod thread_list;
pub mod thread_row;

pub use context::{
    FilterState, OverlayState, Progress, RenderContext, SystemStats, Theme, ThreadMode, ThreadView,
};
