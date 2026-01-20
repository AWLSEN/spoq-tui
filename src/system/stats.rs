//! System statistics polling for CPU and RAM monitoring.
//!
//! This module provides functionality to periodically poll system stats (CPU, RAM)
//! and send updates to the application for display in the dashboard header.

use sysinfo::System;
use std::time::Duration;
use tokio::time::interval;
use crate::ui::dashboard::SystemStats;

/// Polls system CPU and RAM stats at 1-second intervals.
///
/// This struct wraps the sysinfo System instance and provides methods to
/// refresh and retrieve current system statistics.
pub struct SystemStatsPoller {
    system: System,
}

impl SystemStatsPoller {
    /// Create a new system stats poller.
    ///
    /// Initializes with all system information loaded.
    pub fn new() -> Self {
        Self {
            system: System::new_all(),
        }
    }

    /// Refresh and return current stats.
    ///
    /// This method refreshes CPU and memory information, then constructs
    /// a SystemStats struct with the current values.
    ///
    /// Note: The `connected` field is set to true by default; it should be
    /// updated by the websocket handler based on actual connection status.
    pub fn poll(&mut self) -> SystemStats {
        self.system.refresh_cpu();
        self.system.refresh_memory();

        let cpu_percent = self.system.global_cpu_info().cpu_usage();
        let ram_used_bytes = self.system.used_memory();
        let ram_total_bytes = self.system.total_memory();

        SystemStats {
            connected: true, // Will be set by websocket handler
            cpu_percent,
            ram_used_gb: ram_used_bytes as f32 / 1_073_741_824.0, // bytes to GB
            ram_total_gb: ram_total_bytes as f32 / 1_073_741_824.0,
        }
    }
}

impl Default for SystemStatsPoller {
    fn default() -> Self {
        Self::new()
    }
}

/// Spawns async task that polls stats every 1 second.
///
/// Returns a channel receiver that will receive SystemStats updates every second.
/// The spawned task will continue running until the receiver is dropped.
///
/// # Example
///
/// ```ignore
/// let stats_rx = spawn_stats_poller();
/// // In your main loop:
/// if let Ok(stats) = stats_rx.try_recv() {
///     // Update UI with new stats
/// }
/// ```
pub fn spawn_stats_poller() -> tokio::sync::mpsc::Receiver<SystemStats> {
    let (tx, rx) = tokio::sync::mpsc::channel(1);

    tokio::spawn(async move {
        let mut poller = SystemStatsPoller::new();
        let mut interval = interval(Duration::from_secs(1));

        loop {
            interval.tick().await;
            let stats = poller.poll();
            if tx.send(stats).await.is_err() {
                break; // Receiver dropped
            }
        }
    });

    rx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_stats_poller_new() {
        let poller = SystemStatsPoller::new();
        // Just verify it constructs without panic
        assert!(std::mem::size_of_val(&poller.system) > 0);
    }

    #[test]
    fn test_system_stats_poller_poll() {
        let mut poller = SystemStatsPoller::new();
        let stats = poller.poll();

        // CPU should be in valid range (0-100 per core, so max ~800% on 8-core systems)
        // We allow up to 1600% to cover high-end systems
        assert!(stats.cpu_percent >= 0.0);
        assert!(stats.cpu_percent <= 1600.0, "CPU percent is unreasonably high: {}", stats.cpu_percent);

        // RAM values should be positive
        assert!(stats.ram_used_gb >= 0.0);
        assert!(stats.ram_total_gb > 0.0);

        // Used should not exceed total
        assert!(stats.ram_used_gb <= stats.ram_total_gb);

        // Connected should be true (default)
        assert!(stats.connected);
    }

    #[test]
    fn test_system_stats_poller_multiple_polls() {
        let mut poller = SystemStatsPoller::new();

        // Poll multiple times to ensure it doesn't panic
        let stats1 = poller.poll();
        let stats2 = poller.poll();
        let stats3 = poller.poll();

        // All polls should produce valid data
        assert!(stats1.cpu_percent >= 0.0);
        assert!(stats2.cpu_percent >= 0.0);
        assert!(stats3.cpu_percent >= 0.0);

        assert!(stats1.ram_total_gb > 0.0);
        assert!(stats2.ram_total_gb > 0.0);
        assert!(stats3.ram_total_gb > 0.0);
    }

    #[test]
    fn test_system_stats_poller_default() {
        let poller = SystemStatsPoller::default();
        assert!(std::mem::size_of_val(&poller.system) > 0);
    }

    #[tokio::test]
    async fn test_spawn_stats_poller() {
        let mut stats_rx = spawn_stats_poller();

        // Should receive a stats update within 2 seconds
        let result = tokio::time::timeout(
            Duration::from_secs(2),
            stats_rx.recv()
        ).await;

        assert!(result.is_ok());
        let stats = result.unwrap();
        assert!(stats.is_some());

        let stats = stats.unwrap();
        assert!(stats.cpu_percent >= 0.0);
        assert!(stats.ram_total_gb > 0.0);
    }

    #[tokio::test]
    async fn test_spawn_stats_poller_multiple_updates() {
        let mut stats_rx = spawn_stats_poller();

        // Receive first update
        let stats1 = tokio::time::timeout(
            Duration::from_secs(2),
            stats_rx.recv()
        ).await;
        assert!(stats1.is_ok());

        // Receive second update
        let stats2 = tokio::time::timeout(
            Duration::from_secs(2),
            stats_rx.recv()
        ).await;
        assert!(stats2.is_ok());

        // Both should be valid
        assert!(stats1.unwrap().unwrap().cpu_percent >= 0.0);
        assert!(stats2.unwrap().unwrap().cpu_percent >= 0.0);
    }
}
