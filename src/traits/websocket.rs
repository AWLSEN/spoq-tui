//! WebSocket connection trait abstraction.
//!
//! Provides a trait-based abstraction for WebSocket operations, enabling
//! dependency injection and mocking in tests.

use async_trait::async_trait;
use tokio::sync::{broadcast, watch};

use crate::websocket::messages::{WsIncomingMessage, WsOutgoingMessage};
use crate::websocket::WsConnectionState;

/// WebSocket connection errors.
#[derive(Debug, Clone)]
pub enum WsError {
    /// Connection failed
    ConnectionFailed(String),
    /// Disconnected from server
    Disconnected,
    /// Failed to send message
    SendFailed(String),
    /// Failed to parse message
    ParseError(String),
    /// Connection timeout
    Timeout(String),
    /// Other error
    Other(String),
}

impl std::fmt::Display for WsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WsError::ConnectionFailed(msg) => write!(f, "Connection failed: {}", msg),
            WsError::Disconnected => write!(f, "Disconnected from server"),
            WsError::SendFailed(msg) => write!(f, "Send failed: {}", msg),
            WsError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            WsError::Timeout(msg) => write!(f, "Connection timeout: {}", msg),
            WsError::Other(msg) => write!(f, "WebSocket error: {}", msg),
        }
    }
}

impl std::error::Error for WsError {}

impl From<crate::websocket::WsError> for WsError {
    fn from(err: crate::websocket::WsError) -> Self {
        match err {
            crate::websocket::WsError::ConnectionFailed(msg) => WsError::ConnectionFailed(msg),
            crate::websocket::WsError::Disconnected => WsError::Disconnected,
            crate::websocket::WsError::SendFailed(msg) => WsError::SendFailed(msg),
            crate::websocket::WsError::ParseError(msg) => WsError::ParseError(msg),
        }
    }
}

/// Trait for WebSocket connection operations.
///
/// This trait abstracts WebSocket operations to enable dependency injection
/// and mocking in tests. The trait uses tokio broadcast and watch channels
/// for message distribution and state monitoring.
///
/// # Example
///
/// ```ignore
/// use spoq::traits::WebSocketConnection;
/// use spoq::websocket::{WsOutgoingMessage, WsConnectionState};
///
/// async fn handle_connection<C: WebSocketConnection>(conn: &C) {
///     // Subscribe to incoming messages
///     let mut rx = conn.subscribe();
///
///     // Monitor connection state
///     let state = conn.state();
///
///     // Send a message
///     conn.send(WsOutgoingMessage::CancelPermission(...)).await?;
/// }
/// ```
#[async_trait]
pub trait WebSocketConnection: Send + Sync {
    /// Send a message to the server.
    ///
    /// # Arguments
    /// * `msg` - The outgoing message to send
    ///
    /// # Returns
    /// Ok(()) on success, or an error if the send failed
    async fn send(&self, msg: WsOutgoingMessage) -> Result<(), WsError>;

    /// Subscribe to incoming messages.
    ///
    /// Returns a broadcast receiver that will receive copies of all
    /// incoming messages. Multiple subscribers can exist simultaneously.
    ///
    /// # Returns
    /// A broadcast receiver for incoming messages
    fn subscribe(&self) -> broadcast::Receiver<WsIncomingMessage>;

    /// Get a receiver for connection state changes.
    ///
    /// The watch receiver provides the current state and notifies
    /// on state changes.
    ///
    /// # Returns
    /// A watch receiver for connection state
    fn state(&self) -> watch::Receiver<WsConnectionState>;

    /// Gracefully shutdown the connection.
    ///
    /// This should close the WebSocket connection and clean up resources.
    fn shutdown(&self);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ws_error_display() {
        assert_eq!(
            WsError::ConnectionFailed("timeout".to_string()).to_string(),
            "Connection failed: timeout"
        );
        assert_eq!(WsError::Disconnected.to_string(), "Disconnected from server");
        assert_eq!(
            WsError::SendFailed("channel closed".to_string()).to_string(),
            "Send failed: channel closed"
        );
        assert_eq!(
            WsError::ParseError("invalid json".to_string()).to_string(),
            "Parse error: invalid json"
        );
        assert_eq!(
            WsError::Timeout("30s".to_string()).to_string(),
            "Connection timeout: 30s"
        );
        assert_eq!(
            WsError::Other("unknown".to_string()).to_string(),
            "WebSocket error: unknown"
        );
    }

    #[test]
    fn test_ws_error_clone() {
        let err = WsError::ConnectionFailed("test".to_string());
        let cloned = err.clone();
        assert_eq!(err.to_string(), cloned.to_string());
    }

    #[test]
    fn test_ws_error_from_client_error() {
        let client_err = crate::websocket::WsError::ConnectionFailed("test".to_string());
        let trait_err: WsError = client_err.into();
        assert!(matches!(trait_err, WsError::ConnectionFailed(_)));

        let client_err = crate::websocket::WsError::Disconnected;
        let trait_err: WsError = client_err.into();
        assert!(matches!(trait_err, WsError::Disconnected));

        let client_err = crate::websocket::WsError::SendFailed("test".to_string());
        let trait_err: WsError = client_err.into();
        assert!(matches!(trait_err, WsError::SendFailed(_)));

        let client_err = crate::websocket::WsError::ParseError("test".to_string());
        let trait_err: WsError = client_err.into();
        assert!(matches!(trait_err, WsError::ParseError(_)));
    }

    #[test]
    fn test_ws_error_implements_error_trait() {
        let err = WsError::Disconnected;
        let _: &dyn std::error::Error = &err;
    }
}
