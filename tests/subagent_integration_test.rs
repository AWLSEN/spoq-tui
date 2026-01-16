//! Integration tests for subagent event flow from SSE parsing through UI rendering.
//!
//! These tests verify the full subagent event flow:
//! 1. Parse SubagentStarted SSE event correctly
//! 2. Parse SubagentProgress SSE event correctly
//! 3. Parse SubagentCompleted SSE event correctly
//! 4. Subagent event creates MessageSegment in cache
//! 5. Multiple parallel subagents render with correct tree connectors
//! 6. Completed subagent shows summary and tool count

use spoq::app::{App, AppMessage};
use spoq::cache::ThreadCache;
use spoq::models::{MessageSegment, SubagentEvent, SubagentEventStatus};
use spoq::sse::{SseEvent, SseParser};

// ============================================================================
// Test Case 1: Parse SubagentStarted SSE event correctly
// ============================================================================

#[test]
fn test_parse_subagent_started_sse_event() {
    let mut parser = SseParser::new();

    parser.feed_line("event: subagent_started").unwrap();
    parser
        .feed_line(r#"data: {"task_id": "task-001", "description": "Explore codebase", "subagent_type": "Explore"}"#)
        .unwrap();

    let event = parser.feed_line("").unwrap();

    assert_eq!(
        event,
        Some(SseEvent::SubagentStarted {
            task_id: "task-001".to_string(),
            description: "Explore codebase".to_string(),
            subagent_type: "Explore".to_string(),
        })
    );
}

#[test]
fn test_parse_subagent_started_with_special_characters() {
    let mut parser = SseParser::new();

    parser.feed_line("event: subagent_started").unwrap();
    parser
        .feed_line(r#"data: {"task_id": "task-uuid-123-abc", "description": "Search for 'config.json' files", "subagent_type": "general-purpose"}"#)
        .unwrap();

    let event = parser.feed_line("").unwrap();

    assert_eq!(
        event,
        Some(SseEvent::SubagentStarted {
            task_id: "task-uuid-123-abc".to_string(),
            description: "Search for 'config.json' files".to_string(),
            subagent_type: "general-purpose".to_string(),
        })
    );
}

#[test]
fn test_parse_subagent_started_with_empty_description() {
    let mut parser = SseParser::new();

    parser.feed_line("event: subagent_started").unwrap();
    parser
        .feed_line(r#"data: {"task_id": "task-empty", "description": "", "subagent_type": "Bash"}"#)
        .unwrap();

    let event = parser.feed_line("").unwrap();

    assert_eq!(
        event,
        Some(SseEvent::SubagentStarted {
            task_id: "task-empty".to_string(),
            description: "".to_string(),
            subagent_type: "Bash".to_string(),
        })
    );
}

// ============================================================================
// Test Case 2: Parse SubagentProgress SSE event correctly
// ============================================================================

#[test]
fn test_parse_subagent_progress_sse_event() {
    let mut parser = SseParser::new();

    parser.feed_line("event: subagent_progress").unwrap();
    parser
        .feed_line(r#"data: {"task_id": "task-002", "message": "Searching files..."}"#)
        .unwrap();

    let event = parser.feed_line("").unwrap();

    assert_eq!(
        event,
        Some(SseEvent::SubagentProgress {
            task_id: "task-002".to_string(),
            message: "Searching files...".to_string(),
        })
    );
}

#[test]
fn test_parse_subagent_progress_with_long_message() {
    let mut parser = SseParser::new();

    let long_message = "Reading file: /Users/test/project/src/components/very/deep/nested/path/to/component/SomeVeryLongComponentName.tsx";

    parser.feed_line("event: subagent_progress").unwrap();
    parser
        .feed_line(&format!(r#"data: {{"task_id": "task-long", "message": "{}"}}"#, long_message))
        .unwrap();

    let event = parser.feed_line("").unwrap();

    assert_eq!(
        event,
        Some(SseEvent::SubagentProgress {
            task_id: "task-long".to_string(),
            message: long_message.to_string(),
        })
    );
}

#[test]
fn test_parse_subagent_progress_with_empty_message() {
    let mut parser = SseParser::new();

    parser.feed_line("event: subagent_progress").unwrap();
    parser
        .feed_line(r#"data: {"task_id": "task-empty-progress", "message": ""}"#)
        .unwrap();

    let event = parser.feed_line("").unwrap();

    assert_eq!(
        event,
        Some(SseEvent::SubagentProgress {
            task_id: "task-empty-progress".to_string(),
            message: "".to_string(),
        })
    );
}

// ============================================================================
// Test Case 3: Parse SubagentCompleted SSE event correctly
// ============================================================================

#[test]
fn test_parse_subagent_completed_sse_event_with_tool_count() {
    let mut parser = SseParser::new();

    parser.feed_line("event: subagent_completed").unwrap();
    parser
        .feed_line(r#"data: {"task_id": "task-003", "summary": "Found 15 files", "tool_call_count": 42}"#)
        .unwrap();

    let event = parser.feed_line("").unwrap();

    assert_eq!(
        event,
        Some(SseEvent::SubagentCompleted {
            task_id: "task-003".to_string(),
            summary: "Found 15 files".to_string(),
            tool_call_count: Some(42),
        })
    );
}

#[test]
fn test_parse_subagent_completed_sse_event_without_tool_count() {
    let mut parser = SseParser::new();

    parser.feed_line("event: subagent_completed").unwrap();
    parser
        .feed_line(r#"data: {"task_id": "task-no-count", "summary": "Analysis complete"}"#)
        .unwrap();

    let event = parser.feed_line("").unwrap();

    assert_eq!(
        event,
        Some(SseEvent::SubagentCompleted {
            task_id: "task-no-count".to_string(),
            summary: "Analysis complete".to_string(),
            tool_call_count: None,
        })
    );
}

#[test]
fn test_parse_subagent_completed_with_zero_tool_count() {
    let mut parser = SseParser::new();

    parser.feed_line("event: subagent_completed").unwrap();
    parser
        .feed_line(r#"data: {"task_id": "task-zero", "summary": "Quick lookup", "tool_call_count": 0}"#)
        .unwrap();

    let event = parser.feed_line("").unwrap();

    assert_eq!(
        event,
        Some(SseEvent::SubagentCompleted {
            task_id: "task-zero".to_string(),
            summary: "Quick lookup".to_string(),
            tool_call_count: Some(0),
        })
    );
}

#[test]
fn test_parse_subagent_completed_with_empty_summary() {
    let mut parser = SseParser::new();

    parser.feed_line("event: subagent_completed").unwrap();
    parser
        .feed_line(r#"data: {"task_id": "task-empty-summary", "summary": "", "tool_call_count": 5}"#)
        .unwrap();

    let event = parser.feed_line("").unwrap();

    assert_eq!(
        event,
        Some(SseEvent::SubagentCompleted {
            task_id: "task-empty-summary".to_string(),
            summary: "".to_string(),
            tool_call_count: Some(5),
        })
    );
}

// ============================================================================
// Test Case 4: Subagent event creates MessageSegment in cache
// ============================================================================

#[test]
fn test_subagent_event_creates_message_segment_in_cache() {
    let mut cache = ThreadCache::new();
    let thread_id = cache.create_streaming_thread("Test subagent".to_string());

    // Start a subagent
    cache.start_subagent_in_message(
        &thread_id,
        "task-segment".to_string(),
        "Explore codebase".to_string(),
        "Explore".to_string(),
    );

    // Verify the segment was created
    let messages = cache.get_messages(&thread_id).unwrap();
    let assistant_msg = &messages[1];

    assert_eq!(assistant_msg.segments.len(), 1);

    if let MessageSegment::SubagentEvent(event) = &assistant_msg.segments[0] {
        assert_eq!(event.task_id, "task-segment");
        assert_eq!(event.description, "Explore codebase");
        assert_eq!(event.subagent_type, "Explore");
        assert_eq!(event.status, SubagentEventStatus::Running);
    } else {
        panic!("Expected SubagentEvent segment");
    }
}

#[test]
fn test_subagent_event_creates_segment_with_correct_status() {
    let mut cache = ThreadCache::new();
    let thread_id = cache.create_streaming_thread("Test".to_string());

    // Start and verify running status
    cache.start_subagent_in_message(
        &thread_id,
        "task-status".to_string(),
        "Task".to_string(),
        "Bash".to_string(),
    );

    let messages = cache.get_messages(&thread_id).unwrap();
    let event = messages[1].get_subagent_event("task-status").unwrap();
    assert_eq!(event.status, SubagentEventStatus::Running);

    // Update progress
    cache.update_subagent_progress(&thread_id, "task-status", "Working...".to_string());

    let messages = cache.get_messages(&thread_id).unwrap();
    let event = messages[1].get_subagent_event("task-status").unwrap();
    assert_eq!(event.status, SubagentEventStatus::Running);
    assert_eq!(event.progress_message, Some("Working...".to_string()));

    // Complete and verify complete status
    cache.complete_subagent_in_message(
        &thread_id,
        "task-status",
        Some("Done".to_string()),
        3,
    );

    let messages = cache.get_messages(&thread_id).unwrap();
    let event = messages[1].get_subagent_event("task-status").unwrap();
    assert_eq!(event.status, SubagentEventStatus::Complete);
}

#[tokio::test]
async fn test_subagent_event_via_app_message() {
    let mut app = App::new().expect("Failed to create app");

    // Create a thread
    for c in "Test message".chars() {
        app.input_box.insert_char(c);
    }
    app.submit_input(spoq::models::ThreadType::Conversation);

    let thread_id = app.active_thread_id.clone().unwrap();

    // Send SubagentStarted message
    app.handle_message(AppMessage::SubagentStarted {
        task_id: "task-app-msg".to_string(),
        description: "Explore via app message".to_string(),
        subagent_type: "Explore".to_string(),
    });

    // Verify segment was created
    let messages = app.cache.get_messages(&thread_id).unwrap();
    let assistant_msg = &messages[1];

    let event = assistant_msg.get_subagent_event("task-app-msg");
    assert!(event.is_some());
    assert_eq!(event.unwrap().description, "Explore via app message");
}

// ============================================================================
// Test Case 5: Multiple parallel subagents render with correct tree connectors
// ============================================================================

#[test]
fn test_multiple_parallel_subagents_in_cache() {
    let mut cache = ThreadCache::new();
    let thread_id = cache.create_streaming_thread("Multi-task test".to_string());

    // Start multiple parallel subagents
    cache.start_subagent_in_message(
        &thread_id,
        "task-parallel-1".to_string(),
        "First parallel task".to_string(),
        "Explore".to_string(),
    );
    cache.start_subagent_in_message(
        &thread_id,
        "task-parallel-2".to_string(),
        "Second parallel task".to_string(),
        "Bash".to_string(),
    );
    cache.start_subagent_in_message(
        &thread_id,
        "task-parallel-3".to_string(),
        "Third parallel task".to_string(),
        "general-purpose".to_string(),
    );

    // Verify all three segments exist
    let messages = cache.get_messages(&thread_id).unwrap();
    let assistant_msg = &messages[1];

    assert_eq!(assistant_msg.segments.len(), 3);

    // Verify each segment
    if let MessageSegment::SubagentEvent(e1) = &assistant_msg.segments[0] {
        assert_eq!(e1.task_id, "task-parallel-1");
        assert_eq!(e1.subagent_type, "Explore");
    } else {
        panic!("Expected SubagentEvent for first segment");
    }

    if let MessageSegment::SubagentEvent(e2) = &assistant_msg.segments[1] {
        assert_eq!(e2.task_id, "task-parallel-2");
        assert_eq!(e2.subagent_type, "Bash");
    } else {
        panic!("Expected SubagentEvent for second segment");
    }

    if let MessageSegment::SubagentEvent(e3) = &assistant_msg.segments[2] {
        assert_eq!(e3.task_id, "task-parallel-3");
        assert_eq!(e3.subagent_type, "general-purpose");
    } else {
        panic!("Expected SubagentEvent for third segment");
    }
}

#[test]
fn test_multiple_parallel_subagents_segment_structure() {
    // Test that multiple parallel subagents create proper consecutive segments
    // which is what the tree connector rendering logic depends on
    let mut cache = ThreadCache::new();
    let thread_id = cache.create_streaming_thread("Multi-parallel test".to_string());

    // Create 3 parallel subagent events
    let events = vec![
        ("task-1", "First task", "Explore"),
        ("task-2", "Second task", "Bash"),
        ("task-3", "Third task", "general-purpose"),
    ];

    for (task_id, desc, subagent_type) in &events {
        cache.start_subagent_in_message(
            &thread_id,
            task_id.to_string(),
            desc.to_string(),
            subagent_type.to_string(),
        );
    }

    let messages = cache.get_messages(&thread_id).unwrap();
    let segments = &messages[1].segments;

    // All 3 should be consecutive SubagentEvent segments
    assert_eq!(segments.len(), 3);

    // Verify they are all SubagentEvents (consecutive = tree connectors will be applied)
    let all_subagent_events = segments.iter().all(|s| matches!(s, MessageSegment::SubagentEvent(_)));
    assert!(all_subagent_events, "All segments should be SubagentEvents for tree connector rendering");
}

#[test]
fn test_single_subagent_creates_single_segment() {
    // Single subagent should use bullet point (no tree connector)
    let mut cache = ThreadCache::new();
    let thread_id = cache.create_streaming_thread("Single test".to_string());

    cache.start_subagent_in_message(
        &thread_id,
        "task-single".to_string(),
        "Single task".to_string(),
        "Explore".to_string(),
    );

    let messages = cache.get_messages(&thread_id).unwrap();
    let segments = &messages[1].segments;

    // Single segment means no tree connectors needed
    assert_eq!(segments.len(), 1);
    assert!(matches!(segments[0], MessageSegment::SubagentEvent(_)));
}

#[test]
fn test_subagent_events_interleaved_with_text() {
    // When subagents are not consecutive (text in between), they get separate tree groups
    let mut cache = ThreadCache::new();
    let thread_id = cache.create_streaming_thread("Interleaved test".to_string());

    // Add text
    cache.append_to_message(&thread_id, "Starting first search...");

    // Add first subagent
    cache.start_subagent_in_message(
        &thread_id,
        "task-interleaved-1".to_string(),
        "First task".to_string(),
        "Explore".to_string(),
    );

    // Add more text - this breaks the consecutive sequence
    cache.append_to_message(&thread_id, "Now for the second search...");

    // Add second subagent
    cache.start_subagent_in_message(
        &thread_id,
        "task-interleaved-2".to_string(),
        "Second task".to_string(),
        "Bash".to_string(),
    );

    let messages = cache.get_messages(&thread_id).unwrap();
    let segments = &messages[1].segments;

    // Should have 4 segments: Text, SubagentEvent, Text, SubagentEvent
    assert_eq!(segments.len(), 4);
    assert!(matches!(segments[0], MessageSegment::Text(_)));
    assert!(matches!(segments[1], MessageSegment::SubagentEvent(_)));
    assert!(matches!(segments[2], MessageSegment::Text(_)));
    assert!(matches!(segments[3], MessageSegment::SubagentEvent(_)));
}

#[tokio::test]
async fn test_multiple_parallel_subagents_via_app_messages() {
    let mut app = App::new().expect("Failed to create app");

    // Create a thread
    for c in "Test parallel".chars() {
        app.input_box.insert_char(c);
    }
    app.submit_input(spoq::models::ThreadType::Conversation);

    let thread_id = app.active_thread_id.clone().unwrap();

    // Start three parallel subagents
    app.handle_message(AppMessage::SubagentStarted {
        task_id: "parallel-1".to_string(),
        description: "First parallel".to_string(),
        subagent_type: "Explore".to_string(),
    });
    app.handle_message(AppMessage::SubagentStarted {
        task_id: "parallel-2".to_string(),
        description: "Second parallel".to_string(),
        subagent_type: "Bash".to_string(),
    });
    app.handle_message(AppMessage::SubagentStarted {
        task_id: "parallel-3".to_string(),
        description: "Third parallel".to_string(),
        subagent_type: "Plan".to_string(),
    });

    // Verify all three are in the message
    let messages = app.cache.get_messages(&thread_id).unwrap();
    let assistant_msg = &messages[1];

    assert!(assistant_msg.get_subagent_event("parallel-1").is_some());
    assert!(assistant_msg.get_subagent_event("parallel-2").is_some());
    assert!(assistant_msg.get_subagent_event("parallel-3").is_some());

    // Verify has_running_subagents
    assert!(assistant_msg.has_running_subagents());
}

// ============================================================================
// Test Case 6: Completed subagent shows summary and tool count
// ============================================================================

#[test]
fn test_completed_subagent_shows_summary_and_tool_count() {
    let mut cache = ThreadCache::new();
    let thread_id = cache.create_streaming_thread("Test completion".to_string());

    // Start a subagent
    cache.start_subagent_in_message(
        &thread_id,
        "task-complete".to_string(),
        "Search for files".to_string(),
        "Explore".to_string(),
    );

    // Complete it with summary and tool count
    cache.complete_subagent_in_message(
        &thread_id,
        "task-complete",
        Some("Found 10 matching files in src/ directory".to_string()),
        15,
    );

    // Verify the completed state
    let messages = cache.get_messages(&thread_id).unwrap();
    let event = messages[1].get_subagent_event("task-complete").unwrap();

    assert_eq!(event.status, SubagentEventStatus::Complete);
    assert_eq!(
        event.summary,
        Some("Found 10 matching files in src/ directory".to_string())
    );
    assert_eq!(event.tool_call_count, 15);
    assert!(event.completed_at.is_some());
    assert!(event.duration_secs.is_some());
}

#[test]
fn test_completed_subagent_without_summary() {
    let mut cache = ThreadCache::new();
    let thread_id = cache.create_streaming_thread("Test no summary".to_string());

    cache.start_subagent_in_message(
        &thread_id,
        "task-no-summary".to_string(),
        "Quick task".to_string(),
        "Bash".to_string(),
    );

    cache.complete_subagent_in_message(&thread_id, "task-no-summary", None, 2);

    let messages = cache.get_messages(&thread_id).unwrap();
    let event = messages[1].get_subagent_event("task-no-summary").unwrap();

    assert_eq!(event.status, SubagentEventStatus::Complete);
    assert!(event.summary.is_none());
    assert_eq!(event.tool_call_count, 2);
}

#[test]
fn test_completed_subagent_has_summary_and_tool_count_in_event() {
    // Test that a completed subagent event stores the summary and tool count
    // which will be rendered by the UI
    let mut event = SubagentEvent::new(
        "task-render-complete".to_string(),
        "Test rendering".to_string(),
        "Explore".to_string(),
    );
    event.tool_call_count = 8;
    event.complete(Some("Successfully analyzed 25 files".to_string()));

    // Verify the event data that will be used for rendering
    assert_eq!(event.status, SubagentEventStatus::Complete);
    assert_eq!(event.tool_call_count, 8);
    assert_eq!(event.summary, Some("Successfully analyzed 25 files".to_string()));
    assert!(event.completed_at.is_some());
    assert!(event.duration_secs.is_some());
}

#[test]
fn test_completed_subagent_with_1_tool_use() {
    // Test singular tool use case - UI should render "1 tool use" not "1 tool uses"
    let mut event = SubagentEvent::new(
        "task-one-tool".to_string(),
        "Single tool task".to_string(),
        "Bash".to_string(),
    );
    event.tool_call_count = 1;
    event.complete(Some("Done".to_string()));

    // Verify the event has tool_call_count = 1 for singular rendering
    assert_eq!(event.status, SubagentEventStatus::Complete);
    assert_eq!(event.tool_call_count, 1);
    assert_eq!(event.summary, Some("Done".to_string()));
}

#[tokio::test]
async fn test_completed_subagent_via_app_message() {
    let mut app = App::new().expect("Failed to create app");

    // Create a thread
    for c in "Test completion flow".chars() {
        app.input_box.insert_char(c);
    }
    app.submit_input(spoq::models::ThreadType::Conversation);

    let thread_id = app.active_thread_id.clone().unwrap();

    // Start a subagent
    app.handle_message(AppMessage::SubagentStarted {
        task_id: "complete-flow".to_string(),
        description: "Full flow test".to_string(),
        subagent_type: "Explore".to_string(),
    });

    // Verify it's running
    let messages = app.cache.get_messages(&thread_id).unwrap();
    assert!(messages[1].has_running_subagents());

    // Update progress
    app.handle_message(AppMessage::SubagentProgress {
        task_id: "complete-flow".to_string(),
        message: "Processing...".to_string(),
    });

    // Complete it
    app.handle_message(AppMessage::SubagentCompleted {
        task_id: "complete-flow".to_string(),
        summary: "Found 20 files matching criteria".to_string(),
        tool_call_count: Some(12),
    });

    // Verify completion
    let messages = app.cache.get_messages(&thread_id).unwrap();
    let event = messages[1].get_subagent_event("complete-flow").unwrap();

    assert_eq!(event.status, SubagentEventStatus::Complete);
    assert_eq!(
        event.summary,
        Some("Found 20 files matching criteria".to_string())
    );
    assert_eq!(event.tool_call_count, 12);
    assert!(!messages[1].has_running_subagents());
}

// ============================================================================
// Full End-to-End Integration Tests
// ============================================================================

#[tokio::test]
async fn test_full_subagent_flow_sse_to_cache_to_render() {
    // This test simulates the complete flow from SSE parsing through cache update to rendering

    // Step 1: Parse SSE events
    let mut parser = SseParser::new();

    // Parse started event
    parser.feed_line("event: subagent_started").unwrap();
    parser
        .feed_line(r#"data: {"task_id": "e2e-task", "description": "End-to-end test", "subagent_type": "Explore"}"#)
        .unwrap();
    let started_event = parser.feed_line("").unwrap().unwrap();

    // Parse progress event
    parser.feed_line("event: subagent_progress").unwrap();
    parser
        .feed_line(r#"data: {"task_id": "e2e-task", "message": "Scanning directory..."}"#)
        .unwrap();
    let progress_event = parser.feed_line("").unwrap().unwrap();

    // Parse completed event
    parser.feed_line("event: subagent_completed").unwrap();
    parser
        .feed_line(r#"data: {"task_id": "e2e-task", "summary": "Found 5 relevant files", "tool_call_count": 7}"#)
        .unwrap();
    let completed_event = parser.feed_line("").unwrap().unwrap();

    // Step 2: Create app and process events
    let mut app = App::new().expect("Failed to create app");

    for c in "E2E test".chars() {
        app.input_box.insert_char(c);
    }
    app.submit_input(spoq::models::ThreadType::Conversation);

    let thread_id = app.active_thread_id.clone().unwrap();

    // Convert SSE events to AppMessages and handle them
    if let SseEvent::SubagentStarted {
        task_id,
        description,
        subagent_type,
    } = started_event
    {
        app.handle_message(AppMessage::SubagentStarted {
            task_id,
            description,
            subagent_type,
        });
    }

    if let SseEvent::SubagentProgress { task_id, message } = progress_event {
        app.handle_message(AppMessage::SubagentProgress { task_id, message });
    }

    if let SseEvent::SubagentCompleted {
        task_id,
        summary,
        tool_call_count,
    } = completed_event
    {
        app.handle_message(AppMessage::SubagentCompleted {
            task_id,
            summary,
            tool_call_count,
        });
    }

    // Step 3: Verify cache state
    let messages = app.cache.get_messages(&thread_id).unwrap();
    let event = messages[1].get_subagent_event("e2e-task").unwrap();

    assert_eq!(event.status, SubagentEventStatus::Complete);
    assert_eq!(event.summary, Some("Found 5 relevant files".to_string()));
    assert_eq!(event.tool_call_count, 7);

    // Step 4: Verify rendering data is correct (summary, tool count, status)
    // The UI rendering is internal, but we verify the data that will be used for rendering
    assert!(event.completed_at.is_some(), "Completed event should have completion time");
    assert!(event.duration_secs.is_some(), "Completed event should have duration");
}

#[tokio::test]
async fn test_subagent_lifecycle_with_thread_reconciliation() {
    let mut app = App::new().expect("Failed to create app");

    // Create a thread (pending state)
    for c in "Reconciliation test".chars() {
        app.input_box.insert_char(c);
    }
    app.submit_input(spoq::models::ThreadType::Conversation);

    let pending_id = app.active_thread_id.clone().unwrap();
    assert!(uuid::Uuid::parse_str(&pending_id).is_ok());

    // Start subagent on pending thread
    app.handle_message(AppMessage::SubagentStarted {
        task_id: "reconcile-task".to_string(),
        description: "Task on pending thread".to_string(),
        subagent_type: "Explore".to_string(),
    });

    // Reconcile to real ID
    app.handle_message(AppMessage::ThreadCreated {
        pending_id: pending_id.clone(),
        real_id: "real-thread-id".to_string(),
        title: Some("Reconciliation test".to_string()),
    });

    // Update and complete subagent using the (old) pending ID reference
    // The cache should still find the subagent via ID resolution
    app.handle_message(AppMessage::SubagentProgress {
        task_id: "reconcile-task".to_string(),
        message: "Working after reconciliation".to_string(),
    });

    app.handle_message(AppMessage::SubagentCompleted {
        task_id: "reconcile-task".to_string(),
        summary: "Completed after reconciliation".to_string(),
        tool_call_count: Some(3),
    });

    // Verify subagent is accessible via real thread ID
    let messages = app.cache.get_messages("real-thread-id").unwrap();
    let event = messages[1].get_subagent_event("reconcile-task").unwrap();

    assert_eq!(event.status, SubagentEventStatus::Complete);
    assert_eq!(
        event.summary,
        Some("Completed after reconciliation".to_string())
    );
}

#[test]
fn test_mixed_segments_text_and_subagents() {
    let mut cache = ThreadCache::new();
    let thread_id = cache.create_streaming_thread("Mixed content test".to_string());

    // Add some text
    cache.append_to_message(&thread_id, "Let me search for that.");

    // Start first subagent
    cache.start_subagent_in_message(
        &thread_id,
        "task-mixed-1".to_string(),
        "Search files".to_string(),
        "Explore".to_string(),
    );

    // Start second subagent (parallel)
    cache.start_subagent_in_message(
        &thread_id,
        "task-mixed-2".to_string(),
        "Analyze patterns".to_string(),
        "general-purpose".to_string(),
    );

    // Verify segment order
    let messages = cache.get_messages(&thread_id).unwrap();
    let segments = &messages[1].segments;

    assert_eq!(segments.len(), 3);
    assert!(matches!(segments[0], MessageSegment::Text(_)));
    assert!(matches!(segments[1], MessageSegment::SubagentEvent(_)));
    assert!(matches!(segments[2], MessageSegment::SubagentEvent(_)));

    // Complete one subagent
    cache.complete_subagent_in_message(
        &thread_id,
        "task-mixed-1",
        Some("Found 5 files".to_string()),
        4,
    );

    // Verify states
    let messages = cache.get_messages(&thread_id).unwrap();
    let event1 = messages[1].get_subagent_event("task-mixed-1").unwrap();
    let event2 = messages[1].get_subagent_event("task-mixed-2").unwrap();

    assert_eq!(event1.status, SubagentEventStatus::Complete);
    assert_eq!(event2.status, SubagentEventStatus::Running);

    // Still has running subagents
    assert!(messages[1].has_running_subagents());

    // Complete the second one
    cache.complete_subagent_in_message(
        &thread_id,
        "task-mixed-2",
        Some("Analysis complete".to_string()),
        6,
    );

    let messages = cache.get_messages(&thread_id).unwrap();
    assert!(!messages[1].has_running_subagents());
}

#[test]
fn test_subagent_events_after_message_finalization() {
    let mut cache = ThreadCache::new();
    let thread_id = cache.create_streaming_thread("Finalization test".to_string());

    // Start subagent
    cache.start_subagent_in_message(
        &thread_id,
        "task-finalize".to_string(),
        "Long running task".to_string(),
        "Explore".to_string(),
    );

    // Finalize the message while subagent is still running
    cache.finalize_message(&thread_id, 100);

    // Update subagent progress after finalization - should still work
    cache.update_subagent_progress(
        &thread_id,
        "task-finalize",
        "Still working after finalization".to_string(),
    );

    let messages = cache.get_messages(&thread_id).unwrap();
    let event = messages[1].get_subagent_event("task-finalize").unwrap();
    assert_eq!(
        event.progress_message,
        Some("Still working after finalization".to_string())
    );

    // Complete subagent after finalization
    cache.complete_subagent_in_message(
        &thread_id,
        "task-finalize",
        Some("Completed after finalization".to_string()),
        10,
    );

    let messages = cache.get_messages(&thread_id).unwrap();
    let event = messages[1].get_subagent_event("task-finalize").unwrap();
    assert_eq!(event.status, SubagentEventStatus::Complete);
    assert_eq!(event.tool_call_count, 10);
}

/// Realistic SSE stream simulation with multiple subagent events
#[test]
fn test_realistic_sse_stream_with_subagents() {
    let mut parser = SseParser::new();
    let mut events = Vec::new();

    // Simulate a realistic SSE stream with subagent lifecycle
    let stream_lines = [
        ": connected",
        "",
        "event: subagent_started",
        r#"data: {"task_id": "explore-1", "description": "Explore src/", "subagent_type": "Explore"}"#,
        "",
        "event: subagent_started",
        r#"data: {"task_id": "explore-2", "description": "Explore tests/", "subagent_type": "Explore"}"#,
        "",
        "event: subagent_progress",
        r#"data: {"task_id": "explore-1", "message": "Found 15 files"}"#,
        "",
        "event: subagent_completed",
        r#"data: {"task_id": "explore-1", "summary": "Analyzed src/ directory", "tool_call_count": 8}"#,
        "",
        "event: subagent_progress",
        r#"data: {"task_id": "explore-2", "message": "Found 10 test files"}"#,
        "",
        "event: subagent_completed",
        r#"data: {"task_id": "explore-2", "summary": "Analyzed tests/ directory", "tool_call_count": 6}"#,
        "",
    ];

    for line in stream_lines {
        if let Ok(Some(event)) = parser.feed_line(line) {
            events.push(event);
        }
    }

    // Should have 6 events: 2 started, 2 progress, 2 completed
    assert_eq!(events.len(), 6);

    // Verify event types
    assert!(matches!(events[0], SseEvent::SubagentStarted { .. }));
    assert!(matches!(events[1], SseEvent::SubagentStarted { .. }));
    assert!(matches!(events[2], SseEvent::SubagentProgress { .. }));
    assert!(matches!(events[3], SseEvent::SubagentCompleted { .. }));
    assert!(matches!(events[4], SseEvent::SubagentProgress { .. }));
    assert!(matches!(events[5], SseEvent::SubagentCompleted { .. }));

    // Verify first completed event details
    if let SseEvent::SubagentCompleted {
        task_id,
        summary,
        tool_call_count,
    } = &events[3]
    {
        assert_eq!(task_id, "explore-1");
        assert_eq!(summary, "Analyzed src/ directory");
        assert_eq!(*tool_call_count, Some(8));
    }
}
