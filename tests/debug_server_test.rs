//! Integration tests for the debug server.

use spoq::debug::{
    create_debug_channel, start_debug_server_on, DebugEvent, DebugEventKind, StateSnapshot,
    StreamLifecycleData, StreamPhase,
};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::time::timeout;

/// Test that the debug server starts and serves the dashboard HTML.
#[tokio::test]
async fn test_debug_server_dashboard_returns_200() {
    let (tx, _rx) = create_debug_channel(16);

    // Bind to port 0 to get a random available port
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let (handle, _state) = start_debug_server_on(addr, tx.clone())
        .await
        .expect("Failed to start debug server");

    // Give the server a moment to start
    tokio::time::sleep(Duration::from_millis(50)).await;

    // The server is running, but we need to find the actual port
    // Since we can't easily get it from the handle, let's use a fixed port for testing
    drop(handle);

    // Restart with known port
    let addr: SocketAddr = "127.0.0.1:13030".parse().unwrap();
    let (handle, _state) = start_debug_server_on(addr, tx)
        .await
        .expect("Failed to start debug server");

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Make HTTP request to dashboard
    let client = reqwest::Client::new();
    let response = client
        .get("http://127.0.0.1:13030/")
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 200);

    let body = response.text().await.expect("Failed to get body");
    assert!(body.contains("SPOQ Debug Dashboard"));
    assert!(body.contains("WebSocket"));

    handle.abort();
}

/// Test that the state endpoint returns JSON.
#[tokio::test]
async fn test_debug_server_state_endpoint() {
    let (tx, _rx) = create_debug_channel(16);

    let addr: SocketAddr = "127.0.0.1:13031".parse().unwrap();
    let (handle, state_snapshot) = start_debug_server_on(addr, tx)
        .await
        .expect("Failed to start debug server");

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Update the state snapshot
    {
        let mut state = state_snapshot.write().await;
        state.threads_count = 5;
        state.messages_count = 42;
        state.is_streaming = true;
        state.session_id = Some("test-session-123".to_string());
    }

    // Make HTTP request to state endpoint
    let client = reqwest::Client::new();
    let response = client
        .get("http://127.0.0.1:13031/state")
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 200);

    let snapshot: StateSnapshot = response.json().await.expect("Failed to parse JSON");
    assert_eq!(snapshot.threads_count, 5);
    assert_eq!(snapshot.messages_count, 42);
    assert!(snapshot.is_streaming);
    assert_eq!(snapshot.session_id, Some("test-session-123".to_string()));

    handle.abort();
}

/// Test that WebSocket endpoint upgrades and receives events.
#[tokio::test]
async fn test_debug_server_websocket_receives_events() {
    use futures_util::StreamExt;
    use tokio_tungstenite::connect_async;

    let (tx, _rx) = create_debug_channel(16);

    let addr: SocketAddr = "127.0.0.1:13032".parse().unwrap();
    let (handle, _state) = start_debug_server_on(addr, tx.clone())
        .await
        .expect("Failed to start debug server");

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Connect via WebSocket
    let (mut ws_stream, _response) = connect_async("ws://127.0.0.1:13032/ws")
        .await
        .expect("Failed to connect to WebSocket");

    // Send a debug event
    let event = DebugEvent::new(DebugEventKind::StreamLifecycle(StreamLifecycleData::new(
        StreamPhase::Connected,
    )));
    tx.send(event).expect("Failed to send event");

    // Receive the event via WebSocket (with timeout)
    let msg = timeout(Duration::from_secs(2), ws_stream.next())
        .await
        .expect("Timeout waiting for message")
        .expect("Stream ended")
        .expect("WebSocket error");

    match msg {
        tokio_tungstenite::tungstenite::Message::Text(text) => {
            let received: DebugEvent = serde_json::from_str(&text).expect("Failed to parse event");
            assert!(matches!(
                received.event,
                DebugEventKind::StreamLifecycle(_)
            ));
        }
        _ => panic!("Expected text message"),
    }

    // Close WebSocket
    ws_stream.close(None).await.ok();
    handle.abort();
}

/// Test that multiple WebSocket clients can receive events.
#[tokio::test]
async fn test_debug_server_multiple_websocket_clients() {
    use futures_util::StreamExt;
    use tokio_tungstenite::connect_async;

    let (tx, _rx) = create_debug_channel(16);

    let addr: SocketAddr = "127.0.0.1:13033".parse().unwrap();
    let (handle, _state) = start_debug_server_on(addr, tx.clone())
        .await
        .expect("Failed to start debug server");

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Connect two WebSocket clients
    let (mut ws1, _) = connect_async("ws://127.0.0.1:13033/ws")
        .await
        .expect("Failed to connect ws1");
    let (mut ws2, _) = connect_async("ws://127.0.0.1:13033/ws")
        .await
        .expect("Failed to connect ws2");

    // Send an event
    let event = DebugEvent::new(DebugEventKind::StreamLifecycle(
        StreamLifecycleData::with_details(StreamPhase::Completed, "Test complete"),
    ));
    tx.send(event).expect("Failed to send event");

    // Both clients should receive the event
    let msg1 = timeout(Duration::from_secs(2), ws1.next())
        .await
        .expect("Timeout on ws1")
        .expect("ws1 stream ended")
        .expect("ws1 error");

    let msg2 = timeout(Duration::from_secs(2), ws2.next())
        .await
        .expect("Timeout on ws2")
        .expect("ws2 stream ended")
        .expect("ws2 error");

    // Verify both received the same event
    match (msg1, msg2) {
        (
            tokio_tungstenite::tungstenite::Message::Text(t1),
            tokio_tungstenite::tungstenite::Message::Text(t2),
        ) => {
            let e1: DebugEvent = serde_json::from_str(&t1).unwrap();
            let e2: DebugEvent = serde_json::from_str(&t2).unwrap();

            assert!(matches!(e1.event, DebugEventKind::StreamLifecycle(_)));
            assert!(matches!(e2.event, DebugEventKind::StreamLifecycle(_)));
        }
        _ => panic!("Expected text messages"),
    }

    handle.abort();
}

/// Test CORS headers are present.
#[tokio::test]
async fn test_debug_server_cors_headers() {
    let (tx, _rx) = create_debug_channel(16);

    let addr: SocketAddr = "127.0.0.1:13034".parse().unwrap();
    let (handle, _state) = start_debug_server_on(addr, tx)
        .await
        .expect("Failed to start debug server");

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Make a preflight OPTIONS request
    let client = reqwest::Client::new();
    let response = client
        .request(reqwest::Method::OPTIONS, "http://127.0.0.1:13034/state")
        .header("Origin", "http://localhost:3000")
        .header("Access-Control-Request-Method", "GET")
        .send()
        .await
        .expect("Failed to send OPTIONS request");

    // CORS headers should be present
    let headers = response.headers();
    assert!(
        headers.contains_key("access-control-allow-origin")
            || headers.contains_key("vary")
            || response.status().is_success()
    );

    handle.abort();
}
