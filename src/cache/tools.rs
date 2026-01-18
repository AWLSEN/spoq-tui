//! Tool and subagent event methods for ThreadCache

use super::ThreadCache;

impl ThreadCache {
    /// Start a tool event in the streaming message
    /// Adds a new running ToolEvent to the message's segments
    pub fn start_tool_in_message(
        &mut self,
        thread_id: &str,
        tool_call_id: String,
        function_name: String,
    ) {
        let resolved_id = self.resolve_thread_id(thread_id).to_string();

        if let Some(messages) = self.messages.get_mut(&resolved_id) {
            if let Some(streaming_msg) = messages.iter_mut().rev().find(|m| m.is_streaming) {
                streaming_msg.start_tool_event(tool_call_id, function_name);
            }
        }
    }

    /// Complete a tool event in a message
    /// Searches recent messages (not just streaming) since ToolCompleted can arrive after StreamDone
    pub fn complete_tool_in_message(&mut self, thread_id: &str, tool_call_id: &str) {
        let resolved_id = self.resolve_thread_id(thread_id).to_string();

        if let Some(messages) = self.messages.get_mut(&resolved_id) {
            // Search recent messages for the tool (ToolCompleted can arrive after message is finalized)
            for msg in messages.iter_mut().rev().take(5) {
                if msg.get_tool_event(tool_call_id).is_some() {
                    msg.complete_tool_event(tool_call_id);
                    return;
                }
            }
        }
    }

    /// Fail a tool event in a message
    /// Searches recent messages (not just streaming) since ToolCompleted can arrive after StreamDone
    pub fn fail_tool_in_message(&mut self, thread_id: &str, tool_call_id: &str) {
        let resolved_id = self.resolve_thread_id(thread_id).to_string();

        if let Some(messages) = self.messages.get_mut(&resolved_id) {
            // Search recent messages for the tool
            for msg in messages.iter_mut().rev().take(5) {
                if msg.get_tool_event(tool_call_id).is_some() {
                    msg.fail_tool_event(tool_call_id);
                    return;
                }
            }
        }
    }

    /// Set the result preview for a tool event in a message
    /// Searches recent messages (not just streaming) since ToolResult can arrive after StreamDone
    ///
    /// # Arguments
    /// * `thread_id` - The thread ID containing the message
    /// * `tool_call_id` - The tool call ID to update
    /// * `content` - The full result content (will be truncated by ToolEvent::set_result)
    /// * `is_error` - Whether the result represents an error
    pub fn set_tool_result(
        &mut self,
        thread_id: &str,
        tool_call_id: &str,
        content: &str,
        is_error: bool,
    ) {
        let resolved_id = self.resolve_thread_id(thread_id).to_string();

        if let Some(messages) = self.messages.get_mut(&resolved_id) {
            // Search recent messages for the tool
            for msg in messages.iter_mut().rev().take(5) {
                for segment in &mut msg.segments {
                    if let crate::models::MessageSegment::ToolEvent(event) = segment {
                        if event.tool_call_id == tool_call_id {
                            event.set_result(content, is_error);
                            return;
                        }
                    }
                }
            }
        }
    }

    /// Set the display_name for a tool event in a message
    /// Searches recent messages (not just streaming) since events can arrive after StreamDone
    pub fn set_tool_display_name(
        &mut self,
        thread_id: &str,
        tool_call_id: &str,
        display_name: String,
    ) {
        let resolved_id = self.resolve_thread_id(thread_id).to_string();

        if let Some(messages) = self.messages.get_mut(&resolved_id) {
            // Search recent messages for the tool
            for msg in messages.iter_mut().rev().take(5) {
                if msg.get_tool_event(tool_call_id).is_some() {
                    msg.set_tool_display_name(tool_call_id, display_name);
                    return;
                }
            }
        }
    }

    /// Append argument chunk to a tool event in a message
    /// Searches recent messages (not just streaming) since events can arrive after StreamDone
    pub fn append_tool_argument(&mut self, thread_id: &str, tool_call_id: &str, chunk: &str) {
        let resolved_id = self.resolve_thread_id(thread_id).to_string();

        if let Some(messages) = self.messages.get_mut(&resolved_id) {
            // Search recent messages for the tool
            for msg in messages.iter_mut().rev().take(5) {
                if msg.get_tool_event(tool_call_id).is_some() {
                    msg.append_tool_arg_chunk(tool_call_id, chunk);
                    return;
                }
            }
        }
    }

    // ============= Subagent Event Methods =============

    /// Start a subagent event in the streaming message.
    ///
    /// Creates a SubagentEvent segment and adds it to the current streaming message.
    ///
    /// # Arguments
    /// * `thread_id` - The thread ID containing the message
    /// * `task_id` - The task ID from the Task tool
    /// * `description` - Description of the subagent task
    /// * `subagent_type` - Type of subagent (e.g., "Explore", "general-purpose")
    pub fn start_subagent_in_message(
        &mut self,
        thread_id: &str,
        task_id: String,
        description: String,
        subagent_type: String,
    ) {
        let resolved_id = self.resolve_thread_id(thread_id).to_string();

        if let Some(messages) = self.messages.get_mut(&resolved_id) {
            if let Some(streaming_msg) = messages.iter_mut().rev().find(|m| m.is_streaming) {
                streaming_msg.start_subagent_event(task_id, description, subagent_type);
            }
        }
    }

    /// Update a subagent's progress message.
    ///
    /// Searches recent messages for the subagent event and updates its progress_message field.
    ///
    /// # Arguments
    /// * `thread_id` - The thread ID containing the message
    /// * `task_id` - The task ID to update
    /// * `message` - The progress message to set
    pub fn update_subagent_progress(&mut self, thread_id: &str, task_id: &str, message: String) {
        let resolved_id = self.resolve_thread_id(thread_id).to_string();

        if let Some(messages) = self.messages.get_mut(&resolved_id) {
            // Search recent messages (subagent events can span message finalization)
            for msg in messages.iter_mut().rev().take(5) {
                if msg.get_subagent_event(task_id).is_some() {
                    msg.update_subagent_progress(task_id, message);
                    return;
                }
            }
        }
    }

    /// Complete a subagent event in a message.
    ///
    /// Marks the subagent as complete with an optional summary and tool call count.
    ///
    /// # Arguments
    /// * `thread_id` - The thread ID containing the message
    /// * `task_id` - The task ID to complete
    /// * `summary` - Optional summary of the subagent results
    /// * `tool_call_count` - Number of tool calls made by the subagent
    pub fn complete_subagent_in_message(
        &mut self,
        thread_id: &str,
        task_id: &str,
        summary: Option<String>,
        tool_call_count: usize,
    ) {
        let resolved_id = self.resolve_thread_id(thread_id).to_string();

        if let Some(messages) = self.messages.get_mut(&resolved_id) {
            // Search recent messages (subagent completion can arrive after StreamDone)
            for msg in messages.iter_mut().rev().take(5) {
                if msg.get_subagent_event(task_id).is_some() {
                    msg.complete_subagent_event(task_id, summary, tool_call_count);
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{MessageRole, MessageSegment, SubagentEventStatus, ThreadType};

    #[test]
    fn test_start_subagent_in_message() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Hello".to_string());

        cache.start_subagent_in_message(
            &thread_id,
            "task-123".to_string(),
            "Explore codebase".to_string(),
            "Explore".to_string(),
        );

        let messages = cache.get_messages(&thread_id).unwrap();
        let assistant_msg = &messages[1];

        // Should have one segment (the subagent event)
        assert_eq!(assistant_msg.segments.len(), 1);

        if let MessageSegment::SubagentEvent(event) = &assistant_msg.segments[0] {
            assert_eq!(event.task_id, "task-123");
            assert_eq!(event.description, "Explore codebase");
            assert_eq!(event.subagent_type, "Explore");
            assert_eq!(event.status, SubagentEventStatus::Running);
            assert!(event.progress_message.is_none());
            assert!(event.summary.is_none());
            assert_eq!(event.tool_call_count, 0);
        } else {
            panic!("Expected SubagentEvent segment");
        }
    }

    #[test]
    fn test_start_subagent_in_message_no_streaming_message() {
        let mut cache = ThreadCache::new();
        cache.add_message_simple("thread-x", MessageRole::User, "Hello".to_string());

        // Should not panic when no streaming message exists
        cache.start_subagent_in_message(
            "thread-x",
            "task-123".to_string(),
            "Test task".to_string(),
            "Explore".to_string(),
        );

        // No subagent should be added
        let messages = cache.get_messages("thread-x").unwrap();
        assert!(messages[0].segments.is_empty());
    }

    #[test]
    fn test_start_subagent_in_message_nonexistent_thread() {
        let mut cache = ThreadCache::new();

        // Should not panic for nonexistent thread
        cache.start_subagent_in_message(
            "nonexistent",
            "task-123".to_string(),
            "Test task".to_string(),
            "Explore".to_string(),
        );

        assert!(cache.get_messages("nonexistent").is_none());
    }

    #[test]
    fn test_update_subagent_progress() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Hello".to_string());

        // Start a subagent
        cache.start_subagent_in_message(
            &thread_id,
            "task-456".to_string(),
            "Search for files".to_string(),
            "general-purpose".to_string(),
        );

        // Update its progress
        cache.update_subagent_progress(&thread_id, "task-456", "Reading src/main.rs".to_string());

        let messages = cache.get_messages(&thread_id).unwrap();
        let subagent = messages[1].get_subagent_event("task-456").unwrap();

        assert_eq!(
            subagent.progress_message,
            Some("Reading src/main.rs".to_string())
        );
    }

    #[test]
    fn test_update_subagent_progress_after_finalization() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Hello".to_string());

        // Start a subagent
        cache.start_subagent_in_message(
            &thread_id,
            "task-789".to_string(),
            "Long running task".to_string(),
            "Explore".to_string(),
        );

        // Finalize the message (subagent still running)
        cache.finalize_message(&thread_id, 100);

        // Update should still work after message is finalized
        cache.update_subagent_progress(&thread_id, "task-789", "Still working".to_string());

        let messages = cache.get_messages(&thread_id).unwrap();
        let assistant_msg = &messages[1];

        if let MessageSegment::SubagentEvent(event) = &assistant_msg.segments[0] {
            assert_eq!(event.progress_message, Some("Still working".to_string()));
        } else {
            panic!("Expected SubagentEvent segment");
        }
    }

    #[test]
    fn test_update_subagent_progress_nonexistent_task() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Hello".to_string());

        cache.start_subagent_in_message(
            &thread_id,
            "task-123".to_string(),
            "Task 1".to_string(),
            "Explore".to_string(),
        );

        // Update a different task ID (should do nothing)
        cache.update_subagent_progress(&thread_id, "wrong-task", "Progress".to_string());

        // Original task should be unchanged
        let messages = cache.get_messages(&thread_id).unwrap();
        let subagent = messages[1].get_subagent_event("task-123").unwrap();
        assert!(subagent.progress_message.is_none());
    }

    #[test]
    fn test_complete_subagent_in_message() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Hello".to_string());

        // Start a subagent
        cache.start_subagent_in_message(
            &thread_id,
            "task-complete".to_string(),
            "Find files".to_string(),
            "Explore".to_string(),
        );

        // Complete it
        cache.complete_subagent_in_message(
            &thread_id,
            "task-complete",
            Some("Found 10 matching files".to_string()),
            5,
        );

        let messages = cache.get_messages(&thread_id).unwrap();
        let subagent = messages[1].get_subagent_event("task-complete").unwrap();

        assert_eq!(subagent.status, SubagentEventStatus::Complete);
        assert_eq!(
            subagent.summary,
            Some("Found 10 matching files".to_string())
        );
        assert_eq!(subagent.tool_call_count, 5);
        assert!(subagent.completed_at.is_some());
        assert!(subagent.duration_secs.is_some());
    }

    #[test]
    fn test_complete_subagent_in_message_without_summary() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Hello".to_string());

        cache.start_subagent_in_message(
            &thread_id,
            "task-no-summary".to_string(),
            "Task".to_string(),
            "Bash".to_string(),
        );

        cache.complete_subagent_in_message(&thread_id, "task-no-summary", None, 2);

        let messages = cache.get_messages(&thread_id).unwrap();
        let subagent = messages[1].get_subagent_event("task-no-summary").unwrap();

        assert_eq!(subagent.status, SubagentEventStatus::Complete);
        assert!(subagent.summary.is_none());
        assert_eq!(subagent.tool_call_count, 2);
    }

    #[test]
    fn test_complete_subagent_after_message_finalization() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Hello".to_string());

        cache.start_subagent_in_message(
            &thread_id,
            "task-late".to_string(),
            "Slow task".to_string(),
            "general-purpose".to_string(),
        );

        // Finalize message before subagent completes
        cache.finalize_message(&thread_id, 100);

        // Complete should still work
        cache.complete_subagent_in_message(&thread_id, "task-late", Some("Done".to_string()), 3);

        let messages = cache.get_messages(&thread_id).unwrap();
        let subagent = messages[1].get_subagent_event("task-late").unwrap();

        assert_eq!(subagent.status, SubagentEventStatus::Complete);
        assert_eq!(subagent.summary, Some("Done".to_string()));
    }

    #[test]
    fn test_subagent_with_reconciled_thread_id() {
        let mut cache = ThreadCache::new();
        let pending_id =
            cache.create_pending_thread("Hello".to_string(), ThreadType::Conversation, None);

        // Start subagent using pending ID
        cache.start_subagent_in_message(
            &pending_id,
            "task-pending".to_string(),
            "Task".to_string(),
            "Explore".to_string(),
        );

        // Reconcile thread ID
        cache.reconcile_thread_id(&pending_id, "real-thread-id", None);

        // Operations using old pending ID should still work (redirected)
        cache.update_subagent_progress(&pending_id, "task-pending", "Working".to_string());
        cache.complete_subagent_in_message(
            &pending_id,
            "task-pending",
            Some("Done".to_string()),
            1,
        );

        // Verify via real ID
        let messages = cache.get_messages("real-thread-id").unwrap();
        let subagent = messages[1].get_subagent_event("task-pending").unwrap();

        assert_eq!(subagent.status, SubagentEventStatus::Complete);
        assert_eq!(subagent.progress_message, Some("Working".to_string()));
        assert_eq!(subagent.summary, Some("Done".to_string()));
    }

    #[test]
    fn test_multiple_subagents_in_message() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Hello".to_string());

        // Start multiple subagents
        cache.start_subagent_in_message(
            &thread_id,
            "task-1".to_string(),
            "First task".to_string(),
            "Explore".to_string(),
        );
        cache.start_subagent_in_message(
            &thread_id,
            "task-2".to_string(),
            "Second task".to_string(),
            "Bash".to_string(),
        );
        cache.start_subagent_in_message(
            &thread_id,
            "task-3".to_string(),
            "Third task".to_string(),
            "general-purpose".to_string(),
        );

        let messages = cache.get_messages(&thread_id).unwrap();
        let assistant_msg = &messages[1];
        assert_eq!(assistant_msg.segments.len(), 3);

        // Complete tasks in different order
        cache.complete_subagent_in_message(&thread_id, "task-2", Some("Done 2".to_string()), 2);
        cache.complete_subagent_in_message(&thread_id, "task-1", Some("Done 1".to_string()), 1);

        // Update third task progress
        cache.update_subagent_progress(&thread_id, "task-3", "Still working".to_string());

        // Verify states
        let messages = cache.get_messages(&thread_id).unwrap();
        let task1 = messages[1].get_subagent_event("task-1").unwrap();
        let task2 = messages[1].get_subagent_event("task-2").unwrap();
        let task3 = messages[1].get_subagent_event("task-3").unwrap();

        assert_eq!(task1.status, SubagentEventStatus::Complete);
        assert_eq!(task2.status, SubagentEventStatus::Complete);
        assert_eq!(task3.status, SubagentEventStatus::Running);
        assert_eq!(task3.progress_message, Some("Still working".to_string()));
    }

    #[test]
    fn test_subagent_full_workflow() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Find all TODO comments".to_string());

        // Start subagent
        cache.start_subagent_in_message(
            &thread_id,
            "explore-task".to_string(),
            "Searching for TODOs".to_string(),
            "Explore".to_string(),
        );

        // Verify initial state
        let messages = cache.get_messages(&thread_id).unwrap();
        let subagent = messages[1].get_subagent_event("explore-task").unwrap();
        assert_eq!(subagent.status, SubagentEventStatus::Running);
        assert!(subagent.progress_message.is_none());

        // Update progress multiple times
        cache.update_subagent_progress(&thread_id, "explore-task", "Scanning src/".to_string());
        cache.update_subagent_progress(&thread_id, "explore-task", "Scanning tests/".to_string());
        cache.update_subagent_progress(
            &thread_id,
            "explore-task",
            "Processing results".to_string(),
        );

        // Progress should reflect last update
        let messages = cache.get_messages(&thread_id).unwrap();
        let subagent = messages[1].get_subagent_event("explore-task").unwrap();
        assert_eq!(
            subagent.progress_message,
            Some("Processing results".to_string())
        );

        // Complete
        cache.complete_subagent_in_message(
            &thread_id,
            "explore-task",
            Some("Found 15 TODO comments across 8 files".to_string()),
            12,
        );

        // Verify final state
        let messages = cache.get_messages(&thread_id).unwrap();
        let subagent = messages[1].get_subagent_event("explore-task").unwrap();
        assert_eq!(subagent.status, SubagentEventStatus::Complete);
        assert_eq!(
            subagent.summary,
            Some("Found 15 TODO comments across 8 files".to_string())
        );
        assert_eq!(subagent.tool_call_count, 12);
        assert!(subagent.duration_secs.is_some());
    }

    #[test]
    fn test_subagent_interleaved_with_text() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Analyze the code".to_string());

        // Append some text
        cache.append_to_message(&thread_id, "Let me search for ");

        // Start subagent
        cache.start_subagent_in_message(
            &thread_id,
            "search-task".to_string(),
            "Searching".to_string(),
            "Explore".to_string(),
        );

        // Append more text
        cache.append_to_message(&thread_id, " and then analyze.");

        let messages = cache.get_messages(&thread_id).unwrap();
        let segments = &messages[1].segments;

        // Should have: Text, SubagentEvent, Text
        assert_eq!(segments.len(), 3);
        assert!(matches!(segments[0], MessageSegment::Text(_)));
        assert!(matches!(segments[1], MessageSegment::SubagentEvent(_)));
        assert!(matches!(segments[2], MessageSegment::Text(_)));

        if let MessageSegment::Text(text) = &segments[0] {
            assert_eq!(text, "Let me search for ");
        }
        if let MessageSegment::Text(text) = &segments[2] {
            assert_eq!(text, " and then analyze.");
        }
    }
}
