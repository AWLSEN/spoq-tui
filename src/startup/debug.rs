//! Debug system initialization module.
//!
//! This module handles starting the debug server during startup.

use crate::debug::{create_debug_channel, DebugEventSender};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

/// Result of debug system initialization.
pub struct DebugSystemResult {
    /// Debug event sender (for sending debug events)
    pub tx: Option<DebugEventSender>,
    /// Server handle (for cleanup on shutdown)
    pub server_handle: Option<JoinHandle<()>>,
    /// State snapshot (for debug server state)
    pub state_snapshot: Option<Arc<RwLock<crate::debug::StateSnapshot>>>,
}

impl Default for DebugSystemResult {
    fn default() -> Self {
        Self {
            tx: None,
            server_handle: None,
            state_snapshot: None,
        }
    }
}

/// Start the debug system (channel + server).
///
/// Returns the debug event sender and server handle if successful.
/// If the debug server fails to start, returns None for all - the app continues without debug.
///
/// # Arguments
/// * `port` - Port to bind the debug server (default: 3030)
///
/// # Returns
/// DebugSystemResult containing sender, handle, and state snapshot (if successful)
pub async fn start_debug_system(port: u16) -> DebugSystemResult {
    // Create debug channel with capacity for 1000 events
    let (debug_tx, _) = create_debug_channel(1000);

    // Try to start the debug server
    let addr: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();

    match crate::debug::start_debug_server_on(addr, debug_tx.clone()).await {
        Ok((handle, state_snapshot)) => {
            // Server started successfully
            DebugSystemResult {
                tx: Some(debug_tx),
                server_handle: Some(handle),
                state_snapshot: Some(state_snapshot),
            }
        }
        Err(_e) => {
            // Server failed to start - continue without debug
            // (e.g., port already in use)
            DebugSystemResult::default()
        }
    }
}

/// Start the debug system with default port (3030).
pub async fn start_debug_system_default() -> DebugSystemResult {
    start_debug_system(3030).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_system_result_default() {
        let result = DebugSystemResult::default();
        assert!(result.tx.is_none());
        assert!(result.server_handle.is_none());
        assert!(result.state_snapshot.is_none());
    }

    #[tokio::test]
    async fn test_start_debug_system() {
        // Use a random high port to avoid conflicts
        let port = 30000 + (std::process::id() % 1000) as u16;
        let result = start_debug_system(port).await;

        // Debug system should start successfully
        assert!(result.tx.is_some());
        assert!(result.server_handle.is_some());
        assert!(result.state_snapshot.is_some());

        // Clean up
        if let Some(handle) = result.server_handle {
            handle.abort();
        }
    }

    #[tokio::test]
    async fn test_start_debug_system_port_in_use() {
        // First, bind to a port
        let port = 30100 + (std::process::id() % 1000) as u16;
        let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port))
            .await
            .expect("Failed to bind test port");

        // Try to start debug system on same port - should fail gracefully
        let result = start_debug_system(port).await;

        // Should return empty result, not panic
        assert!(result.tx.is_none());
        assert!(result.server_handle.is_none());

        // Clean up
        drop(listener);
    }
}
