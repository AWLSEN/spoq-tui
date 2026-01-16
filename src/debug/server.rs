//! Debug server implementation.
//!
//! This module provides the HTTP and WebSocket server for the debug dashboard.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::{Html, IntoResponse, Json},
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{broadcast, RwLock};
use tokio::task::JoinHandle;
use tower_http::cors::{Any, CorsLayer};

use super::events::StateSnapshot;
use super::html::DASHBOARD_HTML;
use super::DebugEventSender;

/// Shared state for the debug server.
#[derive(Clone)]
pub struct DebugServerState {
    /// Broadcast sender for subscribing to events
    pub event_tx: DebugEventSender,
    /// Cached state snapshot
    pub state_snapshot: Arc<RwLock<StateSnapshot>>,
}

/// Debug server that provides HTTP and WebSocket access to debug events.
pub struct DebugServer {
    /// The event sender for subscribing to events
    event_tx: DebugEventSender,
    /// Cached state snapshot
    state_snapshot: Arc<RwLock<StateSnapshot>>,
}

impl DebugServer {
    /// Create a new debug server.
    ///
    /// # Arguments
    /// * `event_tx` - The broadcast sender to subscribe to for events
    pub fn new(event_tx: DebugEventSender) -> Self {
        Self {
            event_tx,
            state_snapshot: Arc::new(RwLock::new(StateSnapshot::default())),
        }
    }

    /// Get a handle to update the state snapshot.
    pub fn state_snapshot(&self) -> Arc<RwLock<StateSnapshot>> {
        Arc::clone(&self.state_snapshot)
    }

    /// Get the server state for sharing with handlers.
    fn into_state(self) -> DebugServerState {
        DebugServerState {
            event_tx: self.event_tx,
            state_snapshot: self.state_snapshot,
        }
    }
}

/// Start the debug server on the specified address.
///
/// Returns a JoinHandle for the server task and the state snapshot handle.
pub async fn start_debug_server(
    event_tx: DebugEventSender,
) -> color_eyre::Result<(JoinHandle<()>, Arc<RwLock<StateSnapshot>>)> {
    start_debug_server_on("127.0.0.1:3030".parse().unwrap(), event_tx).await
}

/// Start the debug server on a specific address.
///
/// This is useful for tests that need to bind to a random port.
pub async fn start_debug_server_on(
    addr: SocketAddr,
    event_tx: DebugEventSender,
) -> color_eyre::Result<(JoinHandle<()>, Arc<RwLock<StateSnapshot>>)> {
    let server = DebugServer::new(event_tx);
    let state_snapshot = server.state_snapshot();
    let server_state = server.into_state();

    // Configure CORS for local development
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/", get(dashboard_handler))
        .route("/ws", get(websocket_handler))
        .route("/state", get(state_handler))
        .layer(cors)
        .with_state(server_state);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    let actual_addr = listener.local_addr()?;

    tracing::info!("Debug server listening on http://{}", actual_addr);

    let handle = tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!("Debug server error: {}", e);
        }
    });

    Ok((handle, state_snapshot))
}

/// Handler for the dashboard HTML page.
async fn dashboard_handler() -> impl IntoResponse {
    Html(DASHBOARD_HTML)
}

/// Handler for WebSocket connections.
async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<DebugServerState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_websocket(socket, state))
}

/// Handle an individual WebSocket connection.
async fn handle_websocket(socket: WebSocket, state: DebugServerState) {
    let (mut sender, mut receiver) = socket.split();

    // Subscribe to the broadcast channel
    let mut event_rx = state.event_tx.subscribe();

    // Spawn a task to forward events to the WebSocket
    let send_task = tokio::spawn(async move {
        loop {
            match event_rx.recv().await {
                Ok(event) => {
                    // Serialize the event to JSON
                    match serde_json::to_string(&event) {
                        Ok(json) => {
                            if sender.send(Message::Text(json)).await.is_err() {
                                // Client disconnected
                                break;
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Failed to serialize debug event: {}", e);
                        }
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("WebSocket client lagged, missed {} events", n);
                    // Continue receiving
                }
                Err(broadcast::error::RecvError::Closed) => {
                    // Channel closed, server shutting down
                    break;
                }
            }
        }
    });

    // Handle incoming messages (mostly for ping/pong and close)
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Close(_)) => break,
            Ok(Message::Ping(data)) => {
                // Ping handling is automatic in axum
                let _ = data;
            }
            Err(_) => break,
            _ => {}
        }
    }

    // Abort the send task when the connection closes
    send_task.abort();
}

/// Handler for the state endpoint.
async fn state_handler(State(state): State<DebugServerState>) -> impl IntoResponse {
    let snapshot = state.state_snapshot.read().await;
    Json(snapshot.clone())
}
