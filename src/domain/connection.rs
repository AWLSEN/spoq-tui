//! Connection state management.
//!
//! This module provides [`ConnectionState`], a domain object that encapsulates
//! WebSocket connection status, HTTP connection status, and related retry logic.

use crate::websocket::{WsConnectionState, WsOutgoingMessage};
use tokio::sync::mpsc;

/// Connection state encapsulating WebSocket and HTTP connection status.
///
/// This domain object manages all connection-related concerns:
/// - WebSocket connection state (connected, reconnecting, disconnected)
/// - WebSocket message sender
/// - HTTP connection status
/// - Stream error tracking
#[derive(Debug)]
pub struct ConnectionState {
    /// WebSocket sender for sending messages to the server
    pub ws_sender: Option<mpsc::Sender<WsOutgoingMessage>>,
    /// WebSocket connection state for UI status indicator
    pub ws_connection_state: WsConnectionState,
    /// Current HTTP connection status to the backend
    pub connection_status: bool,
    /// Last stream error for display
    pub stream_error: Option<String>,
}

impl Default for ConnectionState {
    fn default() -> Self {
        Self::new()
    }
}

impl ConnectionState {
    /// Create a new ConnectionState with default values.
    pub fn new() -> Self {
        Self {
            ws_sender: None,
            ws_connection_state: WsConnectionState::Disconnected,
            connection_status: false,
            stream_error: None,
        }
    }

    /// Check if the WebSocket is connected.
    pub fn is_ws_connected(&self) -> bool {
        matches!(self.ws_connection_state, WsConnectionState::Connected)
    }

    /// Check if the WebSocket is in a reconnecting state.
    pub fn is_ws_reconnecting(&self) -> bool {
        matches!(
            self.ws_connection_state,
            WsConnectionState::Reconnecting { .. }
        )
    }

    /// Get the current reconnection attempt number, if reconnecting.
    pub fn ws_reconnection_attempt(&self) -> Option<u8> {
        match self.ws_connection_state {
            WsConnectionState::Reconnecting { attempt } => Some(attempt),
            _ => None,
        }
    }

    /// Check if the HTTP connection is active.
    pub fn is_http_connected(&self) -> bool {
        self.connection_status
    }

    /// Check if any connection is active (WebSocket or HTTP).
    pub fn is_any_connected(&self) -> bool {
        self.is_ws_connected() || self.is_http_connected()
    }

    /// Set the WebSocket connection state.
    pub fn set_ws_state(&mut self, state: WsConnectionState) {
        self.ws_connection_state = state;
    }

    /// Set the WebSocket sender.
    pub fn set_ws_sender(&mut self, sender: Option<mpsc::Sender<WsOutgoingMessage>>) {
        self.ws_sender = sender;
    }

    /// Set the HTTP connection status.
    pub fn set_http_connected(&mut self, connected: bool) {
        self.connection_status = connected;
    }

    /// Set a stream error message.
    pub fn set_stream_error(&mut self, error: Option<String>) {
        self.stream_error = error;
    }

    /// Clear the stream error.
    pub fn clear_stream_error(&mut self) {
        self.stream_error = None;
    }

    /// Check if there's a pending stream error.
    pub fn has_stream_error(&self) -> bool {
        self.stream_error.is_some()
    }

    /// Check if the WebSocket sender is available.
    pub fn can_send_ws(&self) -> bool {
        self.ws_sender.is_some() && self.is_ws_connected()
    }

    /// Send a message through the WebSocket.
    ///
    /// Returns true if the message was sent successfully, false otherwise.
    pub async fn send_ws_message(&self, message: WsOutgoingMessage) -> bool {
        if let Some(ref sender) = self.ws_sender {
            sender.send(message).await.is_ok()
        } else {
            false
        }
    }

    /// Disconnect the WebSocket.
    pub fn disconnect_ws(&mut self) {
        self.ws_sender = None;
        self.ws_connection_state = WsConnectionState::Disconnected;
    }

    /// Reset all connection state.
    pub fn reset(&mut self) {
        self.ws_sender = None;
        self.ws_connection_state = WsConnectionState::Disconnected;
        self.connection_status = false;
        self.stream_error = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_state_new() {
        let state = ConnectionState::new();
        assert!(state.ws_sender.is_none());
        assert_eq!(state.ws_connection_state, WsConnectionState::Disconnected);
        assert!(!state.connection_status);
        assert!(state.stream_error.is_none());
    }

    #[test]
    fn test_connection_state_default() {
        let state = ConnectionState::default();
        assert!(!state.is_ws_connected());
        assert!(!state.is_http_connected());
    }

    #[test]
    fn test_is_ws_connected() {
        let mut state = ConnectionState::new();
        assert!(!state.is_ws_connected());

        state.ws_connection_state = WsConnectionState::Connected;
        assert!(state.is_ws_connected());

        state.ws_connection_state = WsConnectionState::Reconnecting { attempt: 1 };
        assert!(!state.is_ws_connected());
    }

    #[test]
    fn test_is_ws_reconnecting() {
        let mut state = ConnectionState::new();
        assert!(!state.is_ws_reconnecting());

        state.ws_connection_state = WsConnectionState::Reconnecting { attempt: 2 };
        assert!(state.is_ws_reconnecting());

        state.ws_connection_state = WsConnectionState::Connected;
        assert!(!state.is_ws_reconnecting());
    }

    #[test]
    fn test_ws_reconnection_attempt() {
        let mut state = ConnectionState::new();
        assert!(state.ws_reconnection_attempt().is_none());

        state.ws_connection_state = WsConnectionState::Reconnecting { attempt: 3 };
        assert_eq!(state.ws_reconnection_attempt(), Some(3));

        state.ws_connection_state = WsConnectionState::Connected;
        assert!(state.ws_reconnection_attempt().is_none());
    }

    #[test]
    fn test_is_http_connected() {
        let mut state = ConnectionState::new();
        assert!(!state.is_http_connected());

        state.connection_status = true;
        assert!(state.is_http_connected());
    }

    #[test]
    fn test_is_any_connected() {
        let mut state = ConnectionState::new();
        assert!(!state.is_any_connected());

        state.connection_status = true;
        assert!(state.is_any_connected());

        state.connection_status = false;
        state.ws_connection_state = WsConnectionState::Connected;
        assert!(state.is_any_connected());

        state.ws_connection_state = WsConnectionState::Disconnected;
        assert!(!state.is_any_connected());
    }

    #[test]
    fn test_set_ws_state() {
        let mut state = ConnectionState::new();
        state.set_ws_state(WsConnectionState::Connected);
        assert!(state.is_ws_connected());

        state.set_ws_state(WsConnectionState::Reconnecting { attempt: 1 });
        assert!(state.is_ws_reconnecting());
    }

    #[test]
    fn test_set_http_connected() {
        let mut state = ConnectionState::new();
        state.set_http_connected(true);
        assert!(state.is_http_connected());

        state.set_http_connected(false);
        assert!(!state.is_http_connected());
    }

    #[test]
    fn test_stream_error() {
        let mut state = ConnectionState::new();
        assert!(!state.has_stream_error());
        assert!(state.stream_error.is_none());

        state.set_stream_error(Some("Connection timeout".to_string()));
        assert!(state.has_stream_error());
        assert_eq!(state.stream_error, Some("Connection timeout".to_string()));

        state.clear_stream_error();
        assert!(!state.has_stream_error());
        assert!(state.stream_error.is_none());
    }

    #[test]
    fn test_can_send_ws() {
        let mut state = ConnectionState::new();
        assert!(!state.can_send_ws());

        // With sender but disconnected
        let (tx, _rx) = mpsc::channel(10);
        state.ws_sender = Some(tx);
        assert!(!state.can_send_ws());

        // With sender and connected
        state.ws_connection_state = WsConnectionState::Connected;
        assert!(state.can_send_ws());
    }

    #[test]
    fn test_disconnect_ws() {
        let mut state = ConnectionState::new();
        let (tx, _rx) = mpsc::channel(10);
        state.ws_sender = Some(tx);
        state.ws_connection_state = WsConnectionState::Connected;

        state.disconnect_ws();

        assert!(state.ws_sender.is_none());
        assert_eq!(state.ws_connection_state, WsConnectionState::Disconnected);
    }

    #[test]
    fn test_reset() {
        let mut state = ConnectionState::new();
        let (tx, _rx) = mpsc::channel(10);
        state.ws_sender = Some(tx);
        state.ws_connection_state = WsConnectionState::Connected;
        state.connection_status = true;
        state.stream_error = Some("error".to_string());

        state.reset();

        assert!(state.ws_sender.is_none());
        assert_eq!(state.ws_connection_state, WsConnectionState::Disconnected);
        assert!(!state.connection_status);
        assert!(state.stream_error.is_none());
    }

    #[tokio::test]
    async fn test_send_ws_message_no_sender() {
        use crate::websocket::WsCancelPermission;

        let state = ConnectionState::new();
        let msg =
            WsOutgoingMessage::CancelPermission(WsCancelPermission::new("test-123".to_string()));
        let result = state.send_ws_message(msg).await;
        assert!(!result);
    }

    #[tokio::test]
    async fn test_send_ws_message_with_sender() {
        use crate::websocket::WsCancelPermission;

        let mut state = ConnectionState::new();
        let (tx, mut rx) = mpsc::channel(10);
        state.ws_sender = Some(tx);

        let msg =
            WsOutgoingMessage::CancelPermission(WsCancelPermission::new("test-456".to_string()));
        let result = state.send_ws_message(msg).await;
        assert!(result);

        // Verify the message was received
        let received = rx.recv().await;
        assert!(received.is_some());
    }
}
