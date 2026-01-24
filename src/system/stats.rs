//! System statistics module.
//!
//! This module previously contained local polling functionality for CPU and RAM monitoring.
//! System stats are now received from the backend via WebSocket connections.
//! See `src/app/websocket.rs` for the WebSocket handler that processes system metrics.
