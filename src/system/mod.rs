//! System monitoring and statistics.
//!
//! This module provides system resource monitoring capabilities including
//! CPU and RAM usage tracking.

mod stats;
pub use stats::{spawn_stats_poller, SystemStatsPoller};
