//! Mock WebSocket connection for testing.
//!
//! Provides a mock WebSocket that allows message injection and
//! response verification for testing purposes.

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::{broadcast, watch, Mutex};

use crate::traits::{TraitWsError, WebSocketConnection};
use crate::websocket::messages::{WsIncomingMessage, WsOutgoingMessage};
use crate::websocket::WsConnectionState;

/// Mock WebSocket connection for testing.
///
/// This mock allows:
/// - Injecting incoming messages
/// - Capturing outgoing messages
/// - Controlling connection state
///
/// # Example
///
/// ```ignore
/// use spoq::adapters::mock::MockWebSocket;
/// use spoq::traits::WebSocketConnection;
/// use spoq::websocket::messages::{WsIncomingMessage, WsOutgoingMessage};
///
/// let mock = MockWebSocket::new();
///
/// // Inject an incoming message
/// mock.inject_message(WsIncomingMessage::Connected(WsConnected {
///     session_id: "test-session".to_string(),
///     timestamp: 1234567890,
/// }));
///
/// // Subscribe and receive
/// let mut rx = mock.subscribe();
/// let msg = rx.recv().await?;
///
/// // Send a message
/// mock.send(WsOutgoingMessage::...).await?;
///
/// // Verify sent messages
/// let sent = mock.get_sent_messages();
/// assert_eq!(sent.len(), 1);
/// ```
pub struct MockWebSocket {
    /// Broadcast sender for incoming messages
    incoming_tx: broadcast::Sender<WsIncomingMessage>,
    /// Watch sender for connection state
    state_tx: watch::Sender<WsConnectionState>,
    /// Watch receiver for connection state
    state_rx: watch::Receiver<WsConnectionState>,
    /// Captured outgoing messages
    sent_messages: Arc<Mutex<Vec<WsOutgoingMessage>>>,
    /// Whether send should fail
    send_should_fail: Arc<Mutex<bool>>,
}

impl MockWebSocket {
    /// Create a new mock WebSocket in connected state.
    pub fn new() -> Self {
        let (incoming_tx, _) = broadcast::channel(100);
        let (state_tx, state_rx) = watch::channel(WsConnectionState::Connected);

        Self {
            incoming_tx,
            state_tx,
            state_rx,
            sent_messages: Arc::new(Mutex::new(Vec::new())),
            send_should_fail: Arc::new(Mutex::new(false)),
        }
    }

    /// Create a new mock WebSocket in disconnected state.
    pub fn disconnected() -> Self {
        let (incoming_tx, _) = broadcast::channel(100);
        let (state_tx, state_rx) = watch::channel(WsConnectionState::Disconnected);

        Self {
            incoming_tx,
            state_tx,
            state_rx,
            sent_messages: Arc::new(Mutex::new(Vec::new())),
            send_should_fail: Arc::new(Mutex::new(false)),
        }
    }

    /// Inject an incoming message.
    ///
    /// The message will be delivered to all subscribers.
    pub fn inject_message(&self, msg: WsIncomingMessage) {
        // Ignore send errors (no subscribers)
        let _ = self.incoming_tx.send(msg);
    }

    /// Inject multiple incoming messages.
    pub fn inject_messages(&self, msgs: Vec<WsIncomingMessage>) {
        for msg in msgs {
            self.inject_message(msg);
        }
    }

    /// Set the connection state.
    pub fn set_state(&self, state: WsConnectionState) {
        let _ = self.state_tx.send(state);
    }

    /// Get all sent messages.
    pub async fn get_sent_messages(&self) -> Vec<WsOutgoingMessage> {
        self.sent_messages.lock().await.clone()
    }

    /// Clear all sent messages.
    pub async fn clear_sent_messages(&self) {
        self.sent_messages.lock().await.clear();
    }

    /// Configure whether send should fail.
    pub async fn set_send_should_fail(&self, should_fail: bool) {
        *self.send_should_fail.lock().await = should_fail;
    }

    /// Simulate a disconnection.
    pub fn simulate_disconnect(&self) {
        let _ = self.state_tx.send(WsConnectionState::Disconnected);
    }

    /// Simulate a reconnection attempt.
    pub fn simulate_reconnecting(&self, attempt: u8) {
        let _ = self
            .state_tx
            .send(WsConnectionState::Reconnecting { attempt });
    }

    /// Simulate a successful reconnection.
    pub fn simulate_reconnected(&self) {
        let _ = self.state_tx.send(WsConnectionState::Connected);
    }

    /// Get the number of subscribers to incoming messages.
    pub fn subscriber_count(&self) -> usize {
        self.incoming_tx.receiver_count()
    }
}

impl Default for MockWebSocket {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for MockWebSocket {
    fn clone(&self) -> Self {
        Self {
            incoming_tx: self.incoming_tx.clone(),
            state_tx: self.state_tx.clone(),
            state_rx: self.state_rx.clone(),
            sent_messages: self.sent_messages.clone(),
            send_should_fail: self.send_should_fail.clone(),
        }
    }
}

#[async_trait]
impl WebSocketConnection for MockWebSocket {
    async fn send(&self, msg: WsOutgoingMessage) -> Result<(), TraitWsError> {
        if *self.send_should_fail.lock().await {
            return Err(TraitWsError::SendFailed("Mock send failure".to_string()));
        }

        self.sent_messages.lock().await.push(msg);
        Ok(())
    }

    fn subscribe(&self) -> broadcast::Receiver<WsIncomingMessage> {
        self.incoming_tx.subscribe()
    }

    fn state(&self) -> watch::Receiver<WsConnectionState> {
        self.state_rx.clone()
    }

    fn shutdown(&self) {
        let _ = self.state_tx.send(WsConnectionState::Disconnected);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::websocket::messages::WsConnected;

    #[test]
    fn test_mock_websocket_new() {
        let mock = MockWebSocket::new();
        assert_eq!(*mock.state_rx.borrow(), WsConnectionState::Connected);
    }

    #[test]
    fn test_mock_websocket_disconnected() {
        let mock = MockWebSocket::disconnected();
        assert_eq!(*mock.state_rx.borrow(), WsConnectionState::Disconnected);
    }

    #[test]
    fn test_mock_websocket_default() {
        let mock = MockWebSocket::default();
        assert_eq!(*mock.state_rx.borrow(), WsConnectionState::Connected);
    }

    #[tokio::test]
    async fn test_inject_message() {
        let mock = MockWebSocket::new();
        let mut rx = mock.subscribe();

        mock.inject_message(WsIncomingMessage::Connected(WsConnected {
            session_id: "test-session".to_string(),
            timestamp: 1234567890,
        }));

        let msg = rx.recv().await.unwrap();
        match msg {
            WsIncomingMessage::Connected(connected) => {
                assert_eq!(connected.session_id, "test-session");
                assert_eq!(connected.timestamp, 1234567890);
            }
            _ => panic!("Expected Connected message"),
        }
    }

    #[tokio::test]
    async fn test_send_message() {
        use crate::websocket::messages::{WsCancelPermission, WsOutgoingMessage};

        let mock = MockWebSocket::new();

        let msg =
            WsOutgoingMessage::CancelPermission(WsCancelPermission::new("req-123".to_string()));
        mock.send(msg).await.unwrap();

        let sent = mock.get_sent_messages().await;
        assert_eq!(sent.len(), 1);
    }

    #[tokio::test]
    async fn test_send_failure() {
        use crate::websocket::messages::{WsCancelPermission, WsOutgoingMessage};

        let mock = MockWebSocket::new();
        mock.set_send_should_fail(true).await;

        let msg =
            WsOutgoingMessage::CancelPermission(WsCancelPermission::new("req-123".to_string()));
        let result = mock.send(msg).await;

        assert!(result.is_err());
        assert!(matches!(result, Err(TraitWsError::SendFailed(_))));
    }

    #[test]
    fn test_set_state() {
        let mock = MockWebSocket::new();

        mock.set_state(WsConnectionState::Disconnected);
        assert_eq!(*mock.state_rx.borrow(), WsConnectionState::Disconnected);

        mock.set_state(WsConnectionState::Reconnecting { attempt: 3 });
        assert_eq!(
            *mock.state_rx.borrow(),
            WsConnectionState::Reconnecting { attempt: 3 }
        );
    }

    #[test]
    fn test_simulate_disconnect_reconnect() {
        let mock = MockWebSocket::new();

        mock.simulate_disconnect();
        assert_eq!(*mock.state_rx.borrow(), WsConnectionState::Disconnected);

        mock.simulate_reconnecting(1);
        assert_eq!(
            *mock.state_rx.borrow(),
            WsConnectionState::Reconnecting { attempt: 1 }
        );

        mock.simulate_reconnected();
        assert_eq!(*mock.state_rx.borrow(), WsConnectionState::Connected);
    }

    #[test]
    fn test_shutdown() {
        let mock = MockWebSocket::new();
        mock.shutdown();
        assert_eq!(*mock.state_rx.borrow(), WsConnectionState::Disconnected);
    }

    #[tokio::test]
    async fn test_clear_sent_messages() {
        use crate::websocket::messages::{WsCancelPermission, WsOutgoingMessage};

        let mock = MockWebSocket::new();

        let msg =
            WsOutgoingMessage::CancelPermission(WsCancelPermission::new("req-123".to_string()));
        mock.send(msg).await.unwrap();

        assert_eq!(mock.get_sent_messages().await.len(), 1);

        mock.clear_sent_messages().await;
        assert!(mock.get_sent_messages().await.is_empty());
    }

    #[test]
    fn test_subscriber_count() {
        let mock = MockWebSocket::new();
        assert_eq!(mock.subscriber_count(), 0);

        let _rx1 = mock.subscribe();
        assert_eq!(mock.subscriber_count(), 1);

        let _rx2 = mock.subscribe();
        assert_eq!(mock.subscriber_count(), 2);
    }

    #[test]
    fn test_clone() {
        let mock = MockWebSocket::new();
        let cloned = mock.clone();

        // Both should share the same state
        mock.set_state(WsConnectionState::Disconnected);
        assert_eq!(*cloned.state_rx.borrow(), WsConnectionState::Disconnected);
    }

    #[tokio::test]
    async fn test_inject_multiple_messages() {
        let mock = MockWebSocket::new();
        let mut rx = mock.subscribe();

        mock.inject_messages(vec![
            WsIncomingMessage::Connected(WsConnected {
                session_id: "session-1".to_string(),
                timestamp: 1000,
            }),
            WsIncomingMessage::Connected(WsConnected {
                session_id: "session-2".to_string(),
                timestamp: 2000,
            }),
        ]);

        let msg1 = rx.recv().await.unwrap();
        let msg2 = rx.recv().await.unwrap();

        match msg1 {
            WsIncomingMessage::Connected(c) => assert_eq!(c.session_id, "session-1"),
            _ => panic!("Expected Connected message"),
        }

        match msg2 {
            WsIncomingMessage::Connected(c) => assert_eq!(c.session_id, "session-2"),
            _ => panic!("Expected Connected message"),
        }
    }

    #[test]
    fn test_state_receiver() {
        let mock = MockWebSocket::new();
        let state_rx = mock.state();

        assert_eq!(*state_rx.borrow(), WsConnectionState::Connected);

        mock.set_state(WsConnectionState::Disconnected);

        // The receiver should see the update
        let state_rx2 = mock.state();
        assert_eq!(*state_rx2.borrow(), WsConnectionState::Disconnected);
    }
}
