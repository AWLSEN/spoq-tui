//! Tungstenite-based WebSocket adapter.
//!
//! This module provides a WebSocket connection implementation that wraps
//! the existing `WsClient` and implements the [`WebSocketConnection`] trait.

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::{broadcast, watch, Mutex};

use crate::traits::{TraitWsError, WebSocketConnection};
use crate::websocket::messages::{WsIncomingMessage, WsOutgoingMessage};
use crate::websocket::{WsClient, WsClientConfig, WsConnectionState};

/// WebSocket connection adapter using tokio-tungstenite.
///
/// This adapter wraps the existing [`WsClient`] implementation and provides
/// a trait-based interface for WebSocket operations.
///
/// # Example
///
/// ```ignore
/// use spoq::adapters::TungsteniteWsConnection;
/// use spoq::traits::WebSocketConnection;
/// use spoq::websocket::WsClientConfig;
///
/// let config = WsClientConfig::default();
/// let connection = TungsteniteWsConnection::connect(config).await?;
///
/// // Subscribe to incoming messages
/// let mut rx = connection.subscribe();
///
/// // Send a message
/// connection.send(WsOutgoingMessage::...).await?;
/// ```
pub struct TungsteniteWsConnection {
    /// The underlying WebSocket client (wrapped in Arc<Mutex> for shared access)
    client: Arc<Mutex<Option<WsClient>>>,
    /// Broadcast sender for incoming messages
    incoming_tx: broadcast::Sender<WsIncomingMessage>,
    /// Watch receiver for connection state
    state_rx: watch::Receiver<WsConnectionState>,
}

impl TungsteniteWsConnection {
    /// Connect to a WebSocket server using the provided configuration.
    ///
    /// # Arguments
    /// * `config` - WebSocket client configuration
    ///
    /// # Returns
    /// A connected WebSocket adapter or an error
    pub async fn connect(config: WsClientConfig) -> Result<Self, TraitWsError> {
        let client = WsClient::connect(config)
            .await
            .map_err(TraitWsError::from)?;

        // Get the state receiver before moving client
        let state_rx = client.state_receiver();

        // Create a broadcast channel for incoming messages
        let (incoming_tx, _) = broadcast::channel(100);

        // Wrap client in Arc<Mutex> for shared access
        let client_arc = Arc::new(Mutex::new(Some(client)));
        let client_for_task = client_arc.clone();
        let incoming_tx_clone = incoming_tx.clone();

        // Spawn a task to forward messages from client to broadcast channel
        tokio::spawn(async move {
            loop {
                let msg = {
                    let mut guard = client_for_task.lock().await;
                    if let Some(ref mut c) = *guard {
                        c.recv().await
                    } else {
                        break;
                    }
                };

                match msg {
                    Some(msg) => {
                        // Ignore send errors (no subscribers)
                        let _ = incoming_tx_clone.send(msg);
                    }
                    None => {
                        // Connection closed
                        break;
                    }
                }
            }
        });

        Ok(Self {
            client: client_arc,
            incoming_tx,
            state_rx,
        })
    }

    /// Connect with default configuration.
    pub async fn connect_default() -> Result<Self, TraitWsError> {
        Self::connect(WsClientConfig::default()).await
    }

    /// Connect with authentication.
    ///
    /// # Arguments
    /// * `token` - Authentication token
    pub async fn connect_with_auth(token: &str) -> Result<Self, TraitWsError> {
        let config = WsClientConfig::default().with_auth(token);
        Self::connect(config).await
    }
}

#[async_trait]
impl WebSocketConnection for TungsteniteWsConnection {
    async fn send(&self, msg: WsOutgoingMessage) -> Result<(), TraitWsError> {
        let guard = self.client.lock().await;
        if let Some(ref client) = *guard {
            client.send(msg).await.map_err(TraitWsError::from)
        } else {
            Err(TraitWsError::Disconnected)
        }
    }

    fn subscribe(&self) -> broadcast::Receiver<WsIncomingMessage> {
        self.incoming_tx.subscribe()
    }

    fn state(&self) -> watch::Receiver<WsConnectionState> {
        self.state_rx.clone()
    }

    fn shutdown(&self) {
        // Take the client out and drop it
        let client_arc = self.client.clone();
        tokio::spawn(async move {
            let mut guard = client_arc.lock().await;
            if let Some(client) = guard.take() {
                client.shutdown();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::websocket::WsError;

    #[test]
    fn test_ws_error_conversion() {
        let client_err = WsError::ConnectionFailed("test".to_string());
        let trait_err: TraitWsError = client_err.into();
        assert!(matches!(trait_err, TraitWsError::ConnectionFailed(_)));

        let client_err = WsError::Disconnected;
        let trait_err: TraitWsError = client_err.into();
        assert!(matches!(trait_err, TraitWsError::Disconnected));

        let client_err = WsError::SendFailed("test".to_string());
        let trait_err: TraitWsError = client_err.into();
        assert!(matches!(trait_err, TraitWsError::SendFailed(_)));

        let client_err = WsError::ParseError("test".to_string());
        let trait_err: TraitWsError = client_err.into();
        assert!(matches!(trait_err, TraitWsError::ParseError(_)));
    }

    #[tokio::test]
    async fn test_connect_failure() {
        // Try to connect to a non-existent server
        let config = WsClientConfig {
            host: "127.0.0.1:59999".to_string(),
            max_retries: 1,
            max_backoff_secs: 1,
            auth_token: None,
            use_tls: false,
        };

        let result = TungsteniteWsConnection::connect(config).await;
        assert!(result.is_err());

        if let Err(e) = result {
            assert!(matches!(e, TraitWsError::ConnectionFailed(_)));
        }
    }

    #[tokio::test]
    async fn test_connect_default_failure() {
        // Default config points to production server which won't be available in tests
        // This should fail with connection error
        let result = TungsteniteWsConnection::connect_default().await;
        // We expect this to fail in tests since there's no server
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_connect_with_auth_failure() {
        let result = TungsteniteWsConnection::connect_with_auth("test-token").await;
        // We expect this to fail in tests since there's no server
        assert!(result.is_err());
    }
}
