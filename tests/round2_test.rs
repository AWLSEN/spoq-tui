//! Integration tests for Round 2 features
//! Tests tool argument streaming and result storage

use spoq::cache::ThreadCache;
use spoq::models::{MessageSegment, ToolEvent};

// ============================================================================
// Tool Argument Streaming Tests
// ============================================================================

#[test]
fn test_append_tool_argument_basic() {
    let mut cache = ThreadCache::new();
    let thread_id = cache.create_streaming_thread("Use the Read tool".to_string());

    // Start a tool event
    let messages = cache.get_messages_mut(&thread_id).unwrap();
    let assistant_msg = &mut messages[1];
    assistant_msg.start_tool_event("tool-123".to_string(), "Read".to_string());

    // Append argument chunks
    cache.append_tool_argument(&thread_id, "tool-123", "{\"file");
    cache.append_tool_argument(&thread_id, "tool-123", "_path\": \"");
    cache.append_tool_argument(&thread_id, "tool-123", "/path/to/file.txt\"}");

    // Verify the arguments accumulated
    let messages = cache.get_messages(&thread_id).unwrap();
    let assistant_msg = &messages[1];

    if let Some(MessageSegment::ToolEvent(event)) = assistant_msg.segments.first() {
        assert_eq!(event.args_json, "{\"file_path\": \"/path/to/file.txt\"}");
    } else {
        panic!("Expected ToolEvent segment");
    }
}

#[test]
fn test_append_tool_argument_multiple_tools() {
    let mut cache = ThreadCache::new();
    let thread_id = cache.create_streaming_thread("Use multiple tools".to_string());

    // Start two tool events
    let messages = cache.get_messages_mut(&thread_id).unwrap();
    let assistant_msg = &mut messages[1];
    assistant_msg.start_tool_event("tool-1".to_string(), "Read".to_string());
    assistant_msg.start_tool_event("tool-2".to_string(), "Bash".to_string());

    // Append arguments to both tools
    cache.append_tool_argument(&thread_id, "tool-1", "{\"file_path\":");
    cache.append_tool_argument(&thread_id, "tool-2", "{\"command\":");
    cache.append_tool_argument(&thread_id, "tool-1", " \"file.txt\"}");
    cache.append_tool_argument(&thread_id, "tool-2", " \"ls\"}");

    // Verify both accumulated correctly
    let messages = cache.get_messages(&thread_id).unwrap();
    let assistant_msg = &messages[1];

    let tool_events: Vec<&ToolEvent> = assistant_msg
        .segments
        .iter()
        .filter_map(|seg| {
            if let MessageSegment::ToolEvent(event) = seg {
                Some(event)
            } else {
                None
            }
        })
        .collect();

    assert_eq!(tool_events.len(), 2);
    assert_eq!(tool_events[0].args_json, "{\"file_path\": \"file.txt\"}");
    assert_eq!(tool_events[1].args_json, "{\"command\": \"ls\"}");
}

#[test]
fn test_append_tool_argument_nonexistent_thread() {
    let mut cache = ThreadCache::new();

    // Should not panic on nonexistent thread
    cache.append_tool_argument("nonexistent-thread", "tool-123", "{\"test\": true}");

    // Thread should not be created
    assert!(cache.get_messages("nonexistent-thread").is_none());
}

#[test]
fn test_append_tool_argument_nonexistent_tool() {
    let mut cache = ThreadCache::new();
    let thread_id = cache.create_streaming_thread("Test".to_string());

    // Append to nonexistent tool should not panic
    cache.append_tool_argument(&thread_id, "nonexistent-tool", "{\"test\": true}");

    // Verify no tool event was created
    let messages = cache.get_messages(&thread_id).unwrap();
    let assistant_msg = &messages[1];
    assert_eq!(assistant_msg.segments.len(), 0);
}

// ============================================================================
// Tool Result Storage Tests
// ============================================================================

#[test]
fn test_set_tool_result_success() {
    let mut cache = ThreadCache::new();
    let thread_id = cache.create_streaming_thread("Use the Read tool".to_string());

    // Start and complete a tool event
    let messages = cache.get_messages_mut(&thread_id).unwrap();
    let assistant_msg = &mut messages[1];
    assistant_msg.start_tool_event("tool-123".to_string(), "Read".to_string());

    // Set success result
    cache.set_tool_result(&thread_id, "tool-123", "File contents here", false);

    // Verify result was stored
    let messages = cache.get_messages(&thread_id).unwrap();
    let assistant_msg = &messages[1];

    if let Some(MessageSegment::ToolEvent(event)) = assistant_msg.segments.first() {
        assert_eq!(event.result_preview.as_ref().unwrap(), "File contents here");
        assert!(!event.result_is_error);
    } else {
        panic!("Expected ToolEvent segment");
    }
}

#[test]
fn test_set_tool_result_error() {
    let mut cache = ThreadCache::new();
    let thread_id = cache.create_streaming_thread("Use the Bash tool".to_string());

    // Start a tool event
    let messages = cache.get_messages_mut(&thread_id).unwrap();
    let assistant_msg = &mut messages[1];
    assistant_msg.start_tool_event("tool-456".to_string(), "Bash".to_string());

    // Set error result
    cache.set_tool_result(&thread_id, "tool-456", "Command failed: exit code 1", true);

    // Verify error was stored
    let messages = cache.get_messages(&thread_id).unwrap();
    let assistant_msg = &messages[1];

    if let Some(MessageSegment::ToolEvent(event)) = assistant_msg.segments.first() {
        assert_eq!(
            event.result_preview.as_ref().unwrap(),
            "Command failed: exit code 1"
        );
        assert!(event.result_is_error);
    } else {
        panic!("Expected ToolEvent segment");
    }
}

#[test]
fn test_set_tool_result_truncates_long_content() {
    let mut cache = ThreadCache::new();
    let thread_id = cache.create_streaming_thread("Test".to_string());

    // Start a tool event
    let messages = cache.get_messages_mut(&thread_id).unwrap();
    let assistant_msg = &mut messages[1];
    assistant_msg.start_tool_event("tool-789".to_string(), "Read".to_string());

    // Create content longer than 500 chars
    let long_content = "a".repeat(600);
    cache.set_tool_result(&thread_id, "tool-789", &long_content, false);

    // Verify result was truncated
    let messages = cache.get_messages(&thread_id).unwrap();
    let assistant_msg = &messages[1];

    if let Some(MessageSegment::ToolEvent(event)) = assistant_msg.segments.first() {
        let result = event.result_preview.as_ref().unwrap();
        assert!(result.len() <= 503); // 500 + "..."
        assert!(result.ends_with("..."));
    } else {
        panic!("Expected ToolEvent segment");
    }
}

#[test]
fn test_set_tool_result_nonexistent_thread() {
    let mut cache = ThreadCache::new();

    // Should not panic on nonexistent thread
    cache.set_tool_result("nonexistent-thread", "tool-123", "Result", false);

    // Thread should not be created
    assert!(cache.get_messages("nonexistent-thread").is_none());
}

#[test]
fn test_set_tool_result_nonexistent_tool() {
    let mut cache = ThreadCache::new();
    let thread_id = cache.create_streaming_thread("Test".to_string());

    // Set result for nonexistent tool should not panic
    cache.set_tool_result(&thread_id, "nonexistent-tool", "Result", false);

    // Verify no tool event was affected
    let messages = cache.get_messages(&thread_id).unwrap();
    let assistant_msg = &messages[1];
    assert_eq!(assistant_msg.segments.len(), 0);
}

// ============================================================================
// Full Workflow Tests (Argument Streaming + Result Storage)
// ============================================================================

#[test]
fn test_complete_tool_workflow() {
    let mut cache = ThreadCache::new();
    let thread_id = cache.create_streaming_thread("Read a file".to_string());

    // Start tool event
    let messages = cache.get_messages_mut(&thread_id).unwrap();
    let assistant_msg = &mut messages[1];
    assistant_msg.start_tool_event("tool-123".to_string(), "Read".to_string());

    // Stream arguments
    cache.append_tool_argument(&thread_id, "tool-123", "{\"file_path\": ");
    cache.append_tool_argument(&thread_id, "tool-123", "\"/path/to/file.txt\"}");

    // Set result
    cache.set_tool_result(&thread_id, "tool-123", "File contents here", false);

    // Verify complete workflow
    let messages = cache.get_messages(&thread_id).unwrap();
    let assistant_msg = &messages[1];

    if let Some(MessageSegment::ToolEvent(event)) = assistant_msg.segments.first() {
        assert_eq!(event.tool_call_id, "tool-123");
        assert_eq!(event.function_name, "Read");
        assert_eq!(event.args_json, "{\"file_path\": \"/path/to/file.txt\"}");
        assert_eq!(event.result_preview.as_ref().unwrap(), "File contents here");
        assert!(!event.result_is_error);
    } else {
        panic!("Expected ToolEvent segment");
    }
}

#[test]
fn test_multiple_tools_complete_workflow() {
    let mut cache = ThreadCache::new();
    let thread_id = cache.create_streaming_thread("Use multiple tools".to_string());

    // Start first tool
    let messages = cache.get_messages_mut(&thread_id).unwrap();
    let assistant_msg = &mut messages[1];
    assistant_msg.start_tool_event("tool-1".to_string(), "Read".to_string());

    // Stream first tool arguments
    cache.append_tool_argument(&thread_id, "tool-1", "{\"file_path\": \"file1.txt\"}");

    // Start second tool
    let messages = cache.get_messages_mut(&thread_id).unwrap();
    let assistant_msg = &mut messages[1];
    assistant_msg.start_tool_event("tool-2".to_string(), "Bash".to_string());

    // Stream second tool arguments
    cache.append_tool_argument(&thread_id, "tool-2", "{\"command\": \"ls\"}");

    // Set results for both
    cache.set_tool_result(&thread_id, "tool-1", "File 1 contents", false);
    cache.set_tool_result(&thread_id, "tool-2", "file1.txt\nfile2.txt", false);

    // Verify both tools completed
    let messages = cache.get_messages(&thread_id).unwrap();
    let assistant_msg = &messages[1];

    let tool_events: Vec<&ToolEvent> = assistant_msg
        .segments
        .iter()
        .filter_map(|seg| {
            if let MessageSegment::ToolEvent(event) = seg {
                Some(event)
            } else {
                None
            }
        })
        .collect();

    assert_eq!(tool_events.len(), 2);

    // First tool
    assert_eq!(tool_events[0].tool_call_id, "tool-1");
    assert_eq!(tool_events[0].args_json, "{\"file_path\": \"file1.txt\"}");
    assert_eq!(
        tool_events[0].result_preview.as_ref().unwrap(),
        "File 1 contents"
    );

    // Second tool
    assert_eq!(tool_events[1].tool_call_id, "tool-2");
    assert_eq!(tool_events[1].args_json, "{\"command\": \"ls\"}");
    assert_eq!(
        tool_events[1].result_preview.as_ref().unwrap(),
        "file1.txt\nfile2.txt"
    );
}

#[test]
fn test_tool_workflow_with_mixed_text_and_tools() {
    let mut cache = ThreadCache::new();
    let thread_id = cache.create_streaming_thread("Let me help you".to_string());

    // Add text before tool
    cache.append_to_message(&thread_id, "Let me read that file for you.");

    // Start tool
    let messages = cache.get_messages_mut(&thread_id).unwrap();
    let assistant_msg = &mut messages[1];
    assistant_msg.start_tool_event("tool-123".to_string(), "Read".to_string());

    // Stream arguments
    cache.append_tool_argument(&thread_id, "tool-123", "{\"file_path\": \"file.txt\"}");

    // Add text after tool starts
    cache.append_to_message(&thread_id, "Reading now...");

    // Set result
    cache.set_tool_result(&thread_id, "tool-123", "File contents", false);

    // Verify segments are in correct order
    let messages = cache.get_messages(&thread_id).unwrap();
    let assistant_msg = &messages[1];

    assert_eq!(assistant_msg.segments.len(), 3);

    // Text segment
    if let MessageSegment::Text(text) = &assistant_msg.segments[0] {
        assert_eq!(text, "Let me read that file for you.");
    } else {
        panic!("Expected Text segment");
    }

    // Tool segment
    if let MessageSegment::ToolEvent(event) = &assistant_msg.segments[1] {
        assert_eq!(event.tool_call_id, "tool-123");
        assert_eq!(event.result_preview.as_ref().unwrap(), "File contents");
    } else {
        panic!("Expected ToolEvent segment");
    }

    // Text segment
    if let MessageSegment::Text(text) = &assistant_msg.segments[2] {
        assert_eq!(text, "Reading now...");
    } else {
        panic!("Expected Text segment");
    }
}
