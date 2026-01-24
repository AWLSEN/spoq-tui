//! System statistics for dashboard header
//!
//! This module provides a view-only struct for system statistics
//! that can be rendered by UI components without accessing App.

/// System statistics for dashboard header
#[derive(Debug, Clone, Default)]
pub struct SystemStats {
    /// WebSocket connection status
    pub connected: bool,
    /// CPU usage percentage (0-100)
    pub cpu_percent: f32,
    /// RAM used in GB
    pub ram_used_gb: f32,
    /// Total RAM in GB
    pub ram_total_gb: f32,
}

impl SystemStats {
    /// Create new system stats
    pub fn new(connected: bool, cpu_percent: f32, ram_used_gb: f32, ram_total_gb: f32) -> Self {
        Self {
            connected,
            cpu_percent,
            ram_used_gb,
            ram_total_gb,
        }
    }

    /// Check if system is under heavy load
    ///
    /// Returns true if CPU > 90% OR RAM usage > 90%
    pub fn is_heavy_load(&self) -> bool {
        self.cpu_percent > 90.0 || self.ram_percent() > 90.0
    }

    /// Get RAM usage as percentage
    pub fn ram_percent(&self) -> f32 {
        if self.ram_total_gb > 0.0 {
            (self.ram_used_gb / self.ram_total_gb) * 100.0
        } else {
            0.0
        }
    }

    /// Get formatted RAM string (e.g., "8.2/16.0 GB")
    pub fn ram_display(&self) -> String {
        format!("{:.1}/{:.1} GB", self.ram_used_gb, self.ram_total_gb)
    }

    /// Get formatted CPU string (e.g., "45%")
    pub fn cpu_display(&self) -> String {
        format!("{:.0}%", self.cpu_percent)
    }

    /// Get connection status display
    pub fn connection_display(&self) -> &'static str {
        if self.connected {
            "Connected"
        } else {
            "Disconnected"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_stats_default() {
        let stats = SystemStats::default();
        assert!(!stats.connected);
        assert_eq!(stats.cpu_percent, 0.0);
        assert_eq!(stats.ram_used_gb, 0.0);
        assert_eq!(stats.ram_total_gb, 0.0);
    }

    #[test]
    fn test_system_stats_new() {
        let stats = SystemStats::new(true, 45.5, 8.2, 16.0);
        assert!(stats.connected);
        assert_eq!(stats.cpu_percent, 45.5);
        assert_eq!(stats.ram_used_gb, 8.2);
        assert_eq!(stats.ram_total_gb, 16.0);
    }

    #[test]
    fn test_system_stats_is_heavy_load() {
        // Normal load
        assert!(!SystemStats::new(true, 50.0, 8.0, 16.0).is_heavy_load());
        // High CPU
        assert!(SystemStats::new(true, 95.0, 8.0, 16.0).is_heavy_load());
        // High RAM
        assert!(SystemStats::new(true, 50.0, 15.0, 16.0).is_heavy_load());
        // Both high
        assert!(SystemStats::new(true, 95.0, 15.0, 16.0).is_heavy_load());
    }

    #[test]
    fn test_system_stats_ram_percent() {
        let stats = SystemStats::new(true, 50.0, 8.0, 16.0);
        assert_eq!(stats.ram_percent(), 50.0);

        let stats = SystemStats::new(true, 50.0, 0.0, 0.0);
        assert_eq!(stats.ram_percent(), 0.0); // Avoid division by zero
    }

    #[test]
    fn test_system_stats_displays() {
        let stats = SystemStats::new(true, 45.0, 8.2, 16.0);
        assert_eq!(stats.cpu_display(), "45%");
        assert_eq!(stats.ram_display(), "8.2/16.0 GB");
        assert_eq!(stats.connection_display(), "Connected");

        let stats = SystemStats::new(false, 0.0, 0.0, 0.0);
        assert_eq!(stats.connection_display(), "Disconnected");
    }
}
