use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, watch};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

use super::messages::{WsCommandResponse, WsIncomingMessage};

/// WebSocket connection errors
#[derive(Debug, Clone)]
pub enum WsError {
    ConnectionFailed(String),
    Disconnected,
    SendFailed(String),
    ParseError(String),
}

impl std::fmt::Display for WsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WsError::ConnectionFailed(msg) => write!(f, "Connection failed: {}", msg),
            WsError::Disconnected => write!(f, "Disconnected from server"),
            WsError::SendFailed(msg) => write!(f, "Send failed: {}", msg),
            WsError::ParseError(msg) => write!(f, "Parse error: {}", msg),
        }
    }
}

impl std::error::Error for WsError {}

/// WebSocket connection state
#[derive(Debug, Clone, PartialEq)]
pub enum WsConnectionState {
    Connected,
    Reconnecting { attempt: u8 },
    Disconnected,
}

/// Configuration for WebSocket client
#[derive(Debug, Clone)]
pub struct WsClientConfig {
    pub host: String,
    pub max_retries: u8,
    pub max_backoff_secs: u64,
}

impl Default for WsClientConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1:3000".to_string(),
            max_retries: 5,
            max_backoff_secs: 30,
        }
    }
}

/// WebSocket client for communicating with the Claude Code server
pub struct WsClient {
    /// Channel to send responses back to the server
    response_tx: mpsc::Sender<WsCommandResponse>,
    /// Receiver for incoming messages from the server
    incoming_rx: mpsc::Receiver<WsIncomingMessage>,
    /// Watch receiver for connection state changes
    state_rx: watch::Receiver<WsConnectionState>,
    /// Flag to signal shutdown
    shutdown: Arc<AtomicBool>,
}

impl WsClient {
    /// Connect to the WebSocket server
    ///
    /// Returns a WsClient on success, or WsError if initial connection fails
    pub async fn connect(config: WsClientConfig) -> Result<Self, WsError> {
        let url = format!("ws://{}/ws", config.host);

        // Try initial connection
        let ws_stream = connect_async(&url)
            .await
            .map_err(|e| WsError::ConnectionFailed(e.to_string()))?;

        info!("Connected to WebSocket server at {}", url);

        let (ws_sink, ws_stream) = ws_stream.0.split();

        // Create channels
        let (incoming_tx, incoming_rx) = mpsc::channel::<WsIncomingMessage>(100);
        let (response_tx, response_rx) = mpsc::channel::<WsCommandResponse>(100);
        let (state_tx, state_rx) = watch::channel(WsConnectionState::Connected);

        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_clone = shutdown.clone();

        // Spawn the background connection handler
        tokio::spawn(async move {
            run_connection_loop(
                url,
                config,
                ws_sink,
                ws_stream,
                incoming_tx,
                response_rx,
                state_tx,
                shutdown_clone,
            )
            .await;
        });

        Ok(Self {
            response_tx,
            incoming_rx,
            state_rx,
            shutdown,
        })
    }

    /// Check if currently connected
    pub fn is_connected(&self) -> bool {
        matches!(*self.state_rx.borrow(), WsConnectionState::Connected)
    }

    /// Get the current connection state
    pub fn connection_state(&self) -> WsConnectionState {
        self.state_rx.borrow().clone()
    }

    /// Subscribe to connection state changes
    pub fn state_receiver(&self) -> watch::Receiver<WsConnectionState> {
        self.state_rx.clone()
    }

    /// Send a command response to the server
    pub async fn send_response(&self, response: WsCommandResponse) -> Result<(), WsError> {
        self.response_tx
            .send(response)
            .await
            .map_err(|e| WsError::SendFailed(e.to_string()))
    }

    /// Receive the next incoming message
    pub async fn recv(&mut self) -> Option<WsIncomingMessage> {
        self.incoming_rx.recv().await
    }

    /// Get a reference to the incoming message receiver for use with select!
    pub fn incoming_receiver(&mut self) -> &mut mpsc::Receiver<WsIncomingMessage> {
        &mut self.incoming_rx
    }

    /// Gracefully shutdown the WebSocket connection
    pub fn shutdown(&self) {
        info!("Shutting down WebSocket client");
        self.shutdown.store(true, Ordering::SeqCst);
    }
}

impl Drop for WsClient {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Run the main connection loop with reconnection logic
async fn run_connection_loop(
    url: String,
    config: WsClientConfig,
    mut ws_sink: futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Message,
    >,
    mut ws_stream: futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
    incoming_tx: mpsc::Sender<WsIncomingMessage>,
    mut response_rx: mpsc::Receiver<WsCommandResponse>,
    state_tx: watch::Sender<WsConnectionState>,
    shutdown: Arc<AtomicBool>,
) {
    loop {
        if shutdown.load(Ordering::SeqCst) {
            debug!("Shutdown signal received, closing connection");
            let _ = ws_sink.close().await;
            break;
        }

        tokio::select! {
            // Handle incoming messages
            msg = ws_stream.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<WsIncomingMessage>(&text) {
                            Ok(parsed) => {
                                debug!("Received message: {:?}", parsed);
                                if incoming_tx.send(parsed).await.is_err() {
                                    warn!("Incoming channel closed, shutting down");
                                    break;
                                }
                            }
                            Err(e) => {
                                warn!("Failed to parse message: {} - {}", e, text);
                                // Continue without crashing - skip malformed messages
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) => {
                        info!("Received close frame from server");
                        let _ = state_tx.send(WsConnectionState::Disconnected);
                        // Attempt reconnection
                        if let Some((new_sink, new_stream)) = attempt_reconnect(
                            &url,
                            &config,
                            &state_tx,
                            &shutdown,
                        ).await {
                            ws_sink = new_sink;
                            ws_stream = new_stream;
                            let _ = state_tx.send(WsConnectionState::Connected);
                        } else {
                            break;
                        }
                    }
                    Some(Ok(Message::Ping(data))) => {
                        debug!("Received ping, sending pong");
                        let _ = ws_sink.send(Message::Pong(data)).await;
                    }
                    Some(Ok(_)) => {
                        // Ignore other message types (Pong, Binary, Frame)
                    }
                    Some(Err(e)) => {
                        error!("WebSocket error: {}", e);
                        let _ = state_tx.send(WsConnectionState::Disconnected);
                        // Attempt reconnection
                        if let Some((new_sink, new_stream)) = attempt_reconnect(
                            &url,
                            &config,
                            &state_tx,
                            &shutdown,
                        ).await {
                            ws_sink = new_sink;
                            ws_stream = new_stream;
                            let _ = state_tx.send(WsConnectionState::Connected);
                        } else {
                            break;
                        }
                    }
                    None => {
                        info!("WebSocket stream ended");
                        let _ = state_tx.send(WsConnectionState::Disconnected);
                        // Attempt reconnection
                        if let Some((new_sink, new_stream)) = attempt_reconnect(
                            &url,
                            &config,
                            &state_tx,
                            &shutdown,
                        ).await {
                            ws_sink = new_sink;
                            ws_stream = new_stream;
                            let _ = state_tx.send(WsConnectionState::Connected);
                        } else {
                            break;
                        }
                    }
                }
            }
            // Handle outgoing responses
            response = response_rx.recv() => {
                match response {
                    Some(resp) => {
                        match serde_json::to_string(&resp) {
                            Ok(json) => {
                                debug!("Sending response: {}", json);
                                if let Err(e) = ws_sink.send(Message::Text(json)).await {
                                    error!("Failed to send response: {}", e);
                                    // Don't break - the connection might recover
                                }
                            }
                            Err(e) => {
                                error!("Failed to serialize response: {}", e);
                            }
                        }
                    }
                    None => {
                        debug!("Response channel closed, shutting down");
                        break;
                    }
                }
            }
        }
    }

    info!("Connection loop ended");
    let _ = state_tx.send(WsConnectionState::Disconnected);
}

/// Attempt to reconnect with exponential backoff
async fn attempt_reconnect(
    url: &str,
    config: &WsClientConfig,
    state_tx: &watch::Sender<WsConnectionState>,
    shutdown: &Arc<AtomicBool>,
) -> Option<(
    futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Message,
    >,
    futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
)> {
    for attempt in 1..=config.max_retries {
        if shutdown.load(Ordering::SeqCst) {
            debug!("Shutdown requested during reconnection");
            return None;
        }

        let _ = state_tx.send(WsConnectionState::Reconnecting { attempt });

        // Calculate backoff: 1s, 2s, 4s, 8s, ... capped at max_backoff_secs
        let backoff_secs = std::cmp::min(
            1u64 << (attempt - 1),
            config.max_backoff_secs,
        );
        info!(
            "Reconnection attempt {} of {}, waiting {}s",
            attempt, config.max_retries, backoff_secs
        );

        tokio::time::sleep(Duration::from_secs(backoff_secs)).await;

        if shutdown.load(Ordering::SeqCst) {
            debug!("Shutdown requested during backoff");
            return None;
        }

        match connect_async(url).await {
            Ok((ws_stream, _)) => {
                info!("Reconnected successfully on attempt {}", attempt);
                let (ws_sink, ws_stream) = ws_stream.split();
                return Some((ws_sink, ws_stream));
            }
            Err(e) => {
                warn!("Reconnection attempt {} failed: {}", attempt, e);
            }
        }
    }

    error!(
        "Failed to reconnect after {} attempts, giving up",
        config.max_retries
    );
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ws_error_display() {
        let err = WsError::ConnectionFailed("timeout".to_string());
        assert_eq!(err.to_string(), "Connection failed: timeout");

        let err = WsError::Disconnected;
        assert_eq!(err.to_string(), "Disconnected from server");

        let err = WsError::SendFailed("channel closed".to_string());
        assert_eq!(err.to_string(), "Send failed: channel closed");

        let err = WsError::ParseError("invalid json".to_string());
        assert_eq!(err.to_string(), "Parse error: invalid json");
    }

    #[test]
    fn test_ws_connection_state_equality() {
        assert_eq!(WsConnectionState::Connected, WsConnectionState::Connected);
        assert_eq!(
            WsConnectionState::Reconnecting { attempt: 1 },
            WsConnectionState::Reconnecting { attempt: 1 }
        );
        assert_ne!(
            WsConnectionState::Reconnecting { attempt: 1 },
            WsConnectionState::Reconnecting { attempt: 2 }
        );
        assert_eq!(
            WsConnectionState::Disconnected,
            WsConnectionState::Disconnected
        );
        assert_ne!(WsConnectionState::Connected, WsConnectionState::Disconnected);
    }

    #[test]
    fn test_ws_client_config_default() {
        let config = WsClientConfig::default();
        assert_eq!(config.host, "127.0.0.1:3000");
        assert_eq!(config.max_retries, 5);
        assert_eq!(config.max_backoff_secs, 30);
    }

    #[test]
    fn test_backoff_calculation() {
        // Test the backoff calculation logic
        let max_backoff = 30u64;

        // Attempt 1: 2^0 = 1s
        let backoff1 = std::cmp::min(1u64 << 0, max_backoff);
        assert_eq!(backoff1, 1);

        // Attempt 2: 2^1 = 2s
        let backoff2 = std::cmp::min(1u64 << 1, max_backoff);
        assert_eq!(backoff2, 2);

        // Attempt 3: 2^2 = 4s
        let backoff3 = std::cmp::min(1u64 << 2, max_backoff);
        assert_eq!(backoff3, 4);

        // Attempt 4: 2^3 = 8s
        let backoff4 = std::cmp::min(1u64 << 3, max_backoff);
        assert_eq!(backoff4, 8);

        // Attempt 5: 2^4 = 16s
        let backoff5 = std::cmp::min(1u64 << 4, max_backoff);
        assert_eq!(backoff5, 16);

        // Attempt 6: 2^5 = 32s, but capped at 30s
        let backoff6 = std::cmp::min(1u64 << 5, max_backoff);
        assert_eq!(backoff6, 30);
    }

    #[tokio::test]
    async fn test_ws_client_connect_failure() {
        // Try to connect to a non-existent server
        let config = WsClientConfig {
            host: "127.0.0.1:59999".to_string(),
            max_retries: 1,
            max_backoff_secs: 1,
        };

        let result = WsClient::connect(config).await;
        assert!(result.is_err());

        if let Err(WsError::ConnectionFailed(msg)) = result {
            assert!(!msg.is_empty());
        } else {
            panic!("Expected ConnectionFailed error");
        }
    }

    #[test]
    fn test_ws_error_clone() {
        let err = WsError::ConnectionFailed("test".to_string());
        let cloned = err.clone();
        match cloned {
            WsError::ConnectionFailed(msg) => assert_eq!(msg, "test"),
            _ => panic!("Expected ConnectionFailed"),
        }
    }

    #[test]
    fn test_ws_connection_state_clone() {
        let state = WsConnectionState::Reconnecting { attempt: 3 };
        let cloned = state.clone();
        assert_eq!(cloned, WsConnectionState::Reconnecting { attempt: 3 });
    }
}
