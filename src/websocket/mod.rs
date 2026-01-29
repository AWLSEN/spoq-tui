//! WebSocket client for communicating with the Claude Code server.
//!
//! This module provides a WebSocket client with automatic reconnection support
//! and integration with the permission system. It handles incoming permission
//! requests from the server and sends responses back via WebSocket.

pub mod client;
pub mod messages;

pub use client::{WsClient, WsClientConfig, WsConnectionState, WsError};
pub use messages::{
    ClaudeLoginStatus, WsCancelPermission, WsClaudeLoginRequest, WsClaudeLoginResponse,
    WsClaudeLoginVerificationResult, WsCommandResponse, WsCommandResult, WsIncomingMessage,
    WsOutgoingMessage, WsPermissionData, WsPermissionRequest, WsPlanApprovalResponse,
};
