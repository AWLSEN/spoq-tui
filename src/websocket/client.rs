use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, watch};
use tokio_tungstenite::{
    connect_async_tls_with_config, tungstenite::client::IntoClientRequest, tungstenite::Message,
};
use tracing::{debug, error, info, warn};

use super::messages::{WsIncomingMessage, WsOutgoingMessage};

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
    /// Optional authentication token for Bearer auth
    pub auth_token: Option<String>,
    /// Whether to use TLS (wss://) for the connection
    pub use_tls: bool,
}

impl Default for WsClientConfig {
    fn default() -> Self {
        // Allow WebSocket host to be configured via environment variable
        let host =
            std::env::var("SPOQ_WS_HOST").unwrap_or_else(|_| "100.85.185.33:8000".to_string());
        // Check for dev token in environment
        let auth_token = std::env::var("SPOQ_DEV_TOKEN").ok();
        Self {
            host,
            max_retries: 5,
            max_backoff_secs: 30,
            auth_token,
            use_tls: false, // Default to non-TLS for local/IP connections
        }
    }
}

impl WsClientConfig {
    /// Set the WebSocket host.
    ///
    /// The host should be in format "host:port" (without protocol).
    /// Returns self for method chaining.
    pub fn with_host(mut self, host: impl Into<String>) -> Self {
        self.host = host.into();
        self
    }

    /// Set the authentication token for Bearer auth.
    ///
    /// Returns self for method chaining.
    pub fn with_auth(mut self, token: &str) -> Self {
        self.auth_token = Some(token.to_string());
        self
    }

    /// Set whether to use TLS (wss://) for the connection.
    ///
    /// Returns self for method chaining.
    pub fn with_tls(mut self, use_tls: bool) -> Self {
        self.use_tls = use_tls;
        self
    }
}

/// WebSocket client for communicating with the Claude Code server
pub struct WsClient {
    /// Channel to send outgoing messages to the server
    outgoing_tx: mpsc::Sender<WsOutgoingMessage>,
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
        // Build URL with correct protocol based on TLS setting
        let protocol = if config.use_tls { "wss" } else { "ws" };
        let url = if let Some(ref token) = config.auth_token {
            format!("{}://{}/ws?token={}", protocol, config.host, token)
        } else {
            format!("{}://{}/ws", protocol, config.host)
        };

        // Log connection attempt
        info!(
            "WS_CONNECT: Attempting connection to {} (has_token={})",
            config.host,
            config.auth_token.is_some()
        );

        // Build the WebSocket request
        let request = url
            .clone()
            .into_client_request()
            .map_err(|e| WsError::ConnectionFailed(e.to_string()))?;

        // Try initial connection with 15 second timeout (Cloudflare Tunnel needs more time)
        let ws_stream = tokio::time::timeout(
            Duration::from_secs(15),
            connect_async_tls_with_config(request, None, false, None),
        )
        .await
        .map_err(|_| WsError::ConnectionFailed(format!("Connection timeout to {}", url)))?
        .map_err(|e| WsError::ConnectionFailed(e.to_string()))?;

        info!("Connected to WebSocket server at {}", url);

        let (ws_sink, ws_stream) = ws_stream.0.split();

        // Create channels
        let (incoming_tx, incoming_rx) = mpsc::channel::<WsIncomingMessage>(100);
        let (outgoing_tx, outgoing_rx) = mpsc::channel::<WsOutgoingMessage>(100);
        let (state_tx, state_rx) = watch::channel(WsConnectionState::Connected);

        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_clone = shutdown.clone();

        // Spawn the background connection handler
        tokio::spawn(async move {
            run_connection_loop(ConnectionLoopParams {
                url,
                config,
                ws_sink,
                ws_stream,
                incoming_tx,
                outgoing_rx,
                state_tx,
                shutdown: shutdown_clone,
            })
            .await;
        });

        Ok(Self {
            outgoing_tx,
            incoming_rx,
            state_rx,
            shutdown,
        })
    }

    /// Subscribe to connection state changes
    pub fn state_receiver(&self) -> watch::Receiver<WsConnectionState> {
        self.state_rx.clone()
    }

    /// Send a message to the server
    pub async fn send(&self, message: WsOutgoingMessage) -> Result<(), WsError> {
        self.outgoing_tx
            .send(message)
            .await
            .map_err(|e| WsError::SendFailed(e.to_string()))
    }

    /// Receive the next incoming message
    pub async fn recv(&mut self) -> Option<WsIncomingMessage> {
        self.incoming_rx.recv().await
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

/// Parameters for the connection loop
struct ConnectionLoopParams {
    url: String,
    config: WsClientConfig,
    ws_sink: futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Message,
    >,
    ws_stream: futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
    incoming_tx: mpsc::Sender<WsIncomingMessage>,
    outgoing_rx: mpsc::Receiver<WsOutgoingMessage>,
    state_tx: watch::Sender<WsConnectionState>,
    shutdown: Arc<AtomicBool>,
}

/// Run the main connection loop with reconnection logic
async fn run_connection_loop(params: ConnectionLoopParams) {
    let ConnectionLoopParams {
        url,
        config,
        mut ws_sink,
        mut ws_stream,
        incoming_tx,
        mut outgoing_rx,
        state_tx,
        shutdown,
    } = params;
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
                        // Send raw message for debugging (truncated to 200 chars)
                        let raw_preview = if text.len() > 200 {
                            format!("{}...", &text[..200])
                        } else {
                            text.clone()
                        };
                        let _ = incoming_tx.send(WsIncomingMessage::RawMessage(raw_preview)).await;

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
                                // Send parse error for debugging
                                let _ = incoming_tx.send(WsIncomingMessage::ParseError {
                                    error: e.to_string(),
                                    raw: if text.len() > 500 { format!("{}...", &text[..500]) } else { text },
                                }).await;
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
            // Handle outgoing messages
            outgoing = outgoing_rx.recv() => {
                match outgoing {
                    Some(msg) => {
                        match serde_json::to_string(&msg) {
                            Ok(json) => {
                                debug!("Sending outgoing message: {}", json);
                                if let Err(e) = ws_sink.send(Message::Text(json)).await {
                                    error!("Failed to send outgoing message: {}", e);
                                    // Don't break - the connection might recover
                                }
                            }
                            Err(e) => {
                                error!("Failed to serialize outgoing message: {}", e);
                            }
                        }
                    }
                    None => {
                        debug!("Outgoing channel closed, shutting down");
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
        let backoff_secs = std::cmp::min(1u64 << (attempt - 1), config.max_backoff_secs);
        info!(
            "Reconnection attempt {} of {}, waiting {}s",
            attempt, config.max_retries, backoff_secs
        );

        tokio::time::sleep(Duration::from_secs(backoff_secs)).await;

        if shutdown.load(Ordering::SeqCst) {
            debug!("Shutdown requested during backoff");
            return None;
        }

        // Build the WebSocket request (token is already in URL query param)
        let request = match url.into_client_request() {
            Ok(req) => req,
            Err(e) => {
                warn!("Failed to build reconnection request: {}", e);
                continue;
            }
        };

        match connect_async_tls_with_config(request, None, false, None).await {
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
        assert_ne!(
            WsConnectionState::Connected,
            WsConnectionState::Disconnected
        );
    }

    #[test]
    fn test_ws_client_config_default() {
        // Note: This test may fail if SPOQ_WS_HOST env var is set
        let config = WsClientConfig::default();
        // Default host when env var is not set
        if std::env::var("SPOQ_WS_HOST").is_err() {
            assert_eq!(config.host, "100.85.185.33:8000");
        }
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
            auth_token: None,
            use_tls: false,
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

    #[test]
    fn test_ws_connection_state_debug() {
        let state = WsConnectionState::Connected;
        let debug_str = format!("{:?}", state);
        assert!(debug_str.contains("Connected"));

        let state = WsConnectionState::Reconnecting { attempt: 2 };
        let debug_str = format!("{:?}", state);
        assert!(debug_str.contains("Reconnecting"));
        assert!(debug_str.contains("2"));
    }

    #[test]
    fn test_ws_client_config_custom() {
        let config = WsClientConfig {
            host: "example.com:8080".to_string(),
            max_retries: 10,
            max_backoff_secs: 60,
            auth_token: None,
            use_tls: false,
        };

        assert_eq!(config.host, "example.com:8080");
        assert_eq!(config.max_retries, 10);
        assert_eq!(config.max_backoff_secs, 60);
        assert!(config.auth_token.is_none());
    }

    #[test]
    fn test_ws_client_config_clone() {
        let config = WsClientConfig {
            host: "localhost:3000".to_string(),
            max_retries: 3,
            max_backoff_secs: 15,
            auth_token: Some("test-token".to_string()),
            use_tls: false,
        };

        let cloned = config.clone();
        assert_eq!(config.host, cloned.host);
        assert_eq!(config.max_retries, cloned.max_retries);
        assert_eq!(config.max_backoff_secs, cloned.max_backoff_secs);
        assert_eq!(config.auth_token, cloned.auth_token);
    }

    #[test]
    fn test_ws_error_implements_error_trait() {
        let err = WsError::ParseError("test".to_string());
        let _: &dyn std::error::Error = &err;
    }

    #[test]
    fn test_all_ws_errors_display() {
        let errors = vec![
            WsError::ConnectionFailed("timeout".to_string()),
            WsError::Disconnected,
            WsError::SendFailed("closed".to_string()),
            WsError::ParseError("invalid json".to_string()),
        ];

        for err in errors {
            let display = err.to_string();
            assert!(!display.is_empty());
        }
    }

    #[test]
    fn test_backoff_calculation_cap() {
        let max_backoff = 30u64;

        // Test that backoff is capped correctly
        for attempt in 1..=10 {
            let backoff = std::cmp::min(1u64 << (attempt - 1), max_backoff);
            assert!(backoff <= max_backoff, "Backoff should never exceed max");
        }
    }

    #[test]
    fn test_backoff_calculation_progression() {
        let max_backoff = 100u64;

        // Test exponential progression
        let backoff1 = std::cmp::min(1u64 << 0, max_backoff);
        let backoff2 = std::cmp::min(1u64 << 1, max_backoff);
        let backoff3 = std::cmp::min(1u64 << 2, max_backoff);

        assert!(backoff2 > backoff1);
        assert!(backoff3 > backoff2);
        assert_eq!(backoff2, backoff1 * 2);
        assert_eq!(backoff3, backoff2 * 2);
    }

    #[test]
    fn test_ws_connection_state_all_variants() {
        let states = vec![
            WsConnectionState::Connected,
            WsConnectionState::Disconnected,
            WsConnectionState::Reconnecting { attempt: 1 },
            WsConnectionState::Reconnecting { attempt: 5 },
        ];

        for state in states {
            // Test that all states can be cloned and compared
            let cloned = state.clone();
            assert_eq!(state, cloned);
        }
    }

    #[test]
    fn test_ws_error_debug() {
        let err = WsError::ConnectionFailed("test error".to_string());
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("ConnectionFailed"));
        assert!(debug_str.contains("test error"));
    }

    #[test]
    fn test_ws_client_config_debug() {
        let config = WsClientConfig {
            host: "test.example.com:8000".to_string(),
            max_retries: 5,
            max_backoff_secs: 30,
            auth_token: None,
            use_tls: false,
        };
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("test.example.com:8000"));
        assert!(debug_str.contains("5"));
        assert!(debug_str.contains("30"));
    }

    #[test]
    fn test_ws_client_config_with_auth() {
        let config = WsClientConfig::default().with_auth("my-auth-token");
        assert_eq!(config.auth_token, Some("my-auth-token".to_string()));
    }

    #[test]
    fn test_ws_client_config_default_no_auth() {
        let config = WsClientConfig::default();
        assert!(config.auth_token.is_none());
    }

    #[test]
    fn test_ws_client_config_with_auth_custom() {
        let config = WsClientConfig {
            host: "custom.example.com:9000".to_string(),
            max_retries: 3,
            max_backoff_secs: 10,
            auth_token: Some("secret-token".to_string()),
            use_tls: false,
        };
        assert_eq!(config.host, "custom.example.com:9000");
        assert_eq!(config.auth_token, Some("secret-token".to_string()));
    }
}
