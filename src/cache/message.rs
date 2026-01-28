//! Message management methods for ThreadCache

use chrono::Utc;

#[cfg(test)]
use chrono::Duration;

use crate::models::{Message, MessageRole};

use super::ThreadCache;

impl ThreadCache {
    /// Get messages for a thread
    pub fn get_messages(&self, thread_id: &str) -> Option<&Vec<Message>> {
        self.messages.get(thread_id)
    }

    /// Get mutable access to messages for a thread
    #[allow(dead_code)]
    pub fn get_messages_mut(&mut self, thread_id: &str) -> Option<&mut Vec<Message>> {
        self.messages.get_mut(thread_id)
    }

    /// Add a message to a thread
    pub fn add_message(&mut self, message: Message) {
        let thread_id = message.thread_id.clone();
        self.messages.entry(thread_id).or_default().push(message);
    }

    /// Add a message to a thread using role and content
    /// This is a convenience method that creates the full Message struct
    pub fn add_message_simple(&mut self, thread_id: &str, role: MessageRole, content: String) {
        let now = Utc::now();

        // Generate a simple message ID based on existing count
        let existing_count = self.messages.get(thread_id).map(|m| m.len()).unwrap_or(0);

        let message = Message {
            id: (existing_count + 1) as i64,
            thread_id: thread_id.to_string(),
            role,
            content,
            created_at: now,
            is_streaming: false,
            partial_content: String::new(),
            reasoning_content: String::new(),
            reasoning_collapsed: true,
            segments: Vec::new(),
            render_version: 0,
        };

        self.add_message(message);
    }

    /// Set messages for a thread.
    ///
    /// This method handles the race condition where the user sends a new message
    /// before the backend returns historical messages. It merges the incoming
    /// backend messages with any locally-added messages (streaming messages or
    /// messages with temporary IDs).
    ///
    /// Messages are considered "local" if they have:
    /// - `is_streaming = true` (streaming assistant placeholder)
    /// - `id = 0` (temporary ID before backend assigns real ID)
    /// - `id` higher than the max ID in the incoming messages (recently added)
    pub fn set_messages(&mut self, thread_id: String, messages: Vec<Message>) {
        // Check if there are existing local messages that should be preserved
        if let Some(existing) = self.messages.get(&thread_id) {
            // Find the maximum message ID from the backend response
            let max_backend_id = messages.iter().map(|m| m.id).max().unwrap_or(0);

            // Collect local messages that should be preserved:
            // - Streaming messages (assistant is actively generating)
            // - Messages with temporary ID (0) that are locally added
            // - Messages with ID higher than any backend message (user just sent)
            let local_messages: Vec<Message> = existing
                .iter()
                .filter(|m| m.is_streaming || m.id == 0 || m.id > max_backend_id)
                .cloned()
                .collect();

            if !local_messages.is_empty() {
                // Merge: backend messages first, then local messages
                let mut merged = messages;
                merged.extend(local_messages);
                self.messages.insert(thread_id, merged);
                return;
            }
        }

        // No local messages to preserve, just set the backend messages
        self.messages.insert(thread_id, messages);
    }

    /// Add a new message exchange to an existing thread.
    ///
    /// Creates a user message and a streaming assistant placeholder.
    /// Use this for follow-up messages in an existing conversation.
    ///
    /// # Arguments
    /// * `thread_id` - The ID of the existing thread
    /// * `user_content` - The user's message content
    ///
    /// # Returns
    /// `true` if the thread exists and messages were added, `false` otherwise.
    pub fn add_streaming_message(&mut self, thread_id: &str, user_content: String) -> bool {
        // Verify thread exists
        if !self.threads.contains_key(thread_id) {
            return false;
        }

        let now = Utc::now();

        // Get the next message ID based on existing messages
        let next_id = self
            .messages
            .get(thread_id)
            .map(|m| m.len() as i64 + 1)
            .unwrap_or(1);

        // Add user message
        let user_message = Message {
            id: next_id,
            thread_id: thread_id.to_string(),
            role: MessageRole::User,
            content: user_content.clone(),
            created_at: now,
            is_streaming: false,
            partial_content: String::new(),
            reasoning_content: String::new(),
            reasoning_collapsed: true,
            segments: Vec::new(),
            render_version: 0,
        };
        self.add_message(user_message);

        // Add streaming assistant placeholder
        let assistant_message = Message {
            id: 0, // Will be updated with real ID from backend
            thread_id: thread_id.to_string(),
            role: MessageRole::Assistant,
            content: String::new(),
            created_at: now,
            is_streaming: true,
            partial_content: String::new(),
            reasoning_content: String::new(),
            reasoning_collapsed: false, // Show reasoning while streaming
            segments: Vec::new(),
            render_version: 0,
        };
        self.add_message(assistant_message);

        // Update thread preview and updated_at
        if let Some(thread) = self.threads.get_mut(thread_id) {
            thread.preview = user_content;
            thread.updated_at = now;
        }

        // Move thread to front of order (most recent activity)
        self.thread_order.retain(|id| id != thread_id);
        self.thread_order.insert(0, thread_id.to_string());

        true
    }

    /// Append a token to the streaming message in a thread
    /// Finds the last message with is_streaming=true and appends the token
    pub fn append_to_message(&mut self, thread_id: &str, token: &str) {
        // Resolve the thread_id in case it's a pending ID that was reconciled
        let resolved_id = self.resolve_thread_id(thread_id).to_string();

        if let Some(messages) = self.messages.get_mut(&resolved_id) {
            // Find the last streaming message
            if let Some(streaming_msg) = messages.iter_mut().rev().find(|m| m.is_streaming) {
                streaming_msg.append_token(token);
            }
        }
    }

    /// Append a reasoning token to the streaming message in a thread
    /// Finds the last message with is_streaming=true and appends to its reasoning content
    pub fn append_reasoning_to_message(&mut self, thread_id: &str, token: &str) {
        // Resolve the thread_id in case it's a pending ID that was reconciled
        let resolved_id = self.resolve_thread_id(thread_id).to_string();

        if let Some(messages) = self.messages.get_mut(&resolved_id) {
            // Find the last streaming message
            if let Some(streaming_msg) = messages.iter_mut().rev().find(|m| m.is_streaming) {
                streaming_msg.append_reasoning_token(token);
            }
        }
    }

    /// Check if a thread has any streaming messages.
    /// Returns true if any message in the thread has is_streaming=true.
    /// Returns false if the thread doesn't exist or has no streaming messages.
    pub fn is_thread_streaming(&self, thread_id: &str) -> bool {
        let resolved_id = self.resolve_thread_id(thread_id);
        self.messages
            .get(resolved_id)
            .map(|msgs| msgs.iter().any(|m| m.is_streaming))
            .unwrap_or(false)
    }

    /// Finalize the streaming message in a thread
    /// Updates the message ID to the real backend ID and marks streaming as complete
    pub fn finalize_message(&mut self, thread_id: &str, message_id: i64) {
        // Resolve the thread_id in case it's a pending ID that was reconciled
        let resolved_id = self.resolve_thread_id(thread_id).to_string();
        if let Some(messages) = self.messages.get_mut(&resolved_id) {
            // Find the streaming message
            if let Some(streaming_msg) = messages.iter_mut().rev().find(|m| m.is_streaming) {
                streaming_msg.id = message_id;
                streaming_msg.finalize();
            }
        }
    }

    /// Cancel a streaming message (user pressed Ctrl+C).
    /// Marks the message as no longer streaming and appends a cancellation indicator.
    pub fn cancel_streaming_message(&mut self, thread_id: &str) {
        // Resolve the thread_id in case it's a pending ID that was reconciled
        let resolved_id = self.resolve_thread_id(thread_id).to_string();
        if let Some(messages) = self.messages.get_mut(&resolved_id) {
            // Find the streaming message
            if let Some(streaming_msg) = messages.iter_mut().rev().find(|m| m.is_streaming) {
                streaming_msg.is_streaming = false;
                // Use a temporary ID for cancelled messages (negative to distinguish from real IDs)
                if streaming_msg.id == 0 {
                    streaming_msg.id = -1;
                }
                // Append cancellation indicator to content
                if !streaming_msg.content.is_empty() {
                    streaming_msg.content.push_str("\n\n[Cancelled]");
                } else {
                    streaming_msg.content = "[Cancelled]".to_string();
                }
                // Bump render version to invalidate caches
                streaming_msg.render_version += 1;
            }
        }
    }

    /// Toggle reasoning collapsed state for a specific message in a thread
    /// Used by 't' key handler to expand/collapse thinking blocks
    pub fn toggle_message_reasoning(&mut self, thread_id: &str, message_index: usize) -> bool {
        let resolved_id = self.resolve_thread_id(thread_id).to_string();

        if let Some(messages) = self.messages.get_mut(&resolved_id) {
            if let Some(message) = messages.get_mut(message_index) {
                if !message.reasoning_content.is_empty() {
                    message.toggle_reasoning_collapsed();
                    return true;
                }
            }
        }
        false
    }

    /// Find the index of the last assistant message with reasoning content
    pub fn find_last_reasoning_message_index(&self, thread_id: &str) -> Option<usize> {
        let resolved_id = self.resolve_thread_id(thread_id);

        if let Some(messages) = self.messages.get(resolved_id) {
            // Find last assistant message with reasoning content
            messages
                .iter()
                .enumerate()
                .rev()
                .find(|(_, m)| m.role == MessageRole::Assistant && !m.reasoning_content.is_empty())
                .map(|(idx, _)| idx)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ThreadType;

    #[test]
    fn test_get_messages_for_thread() {
        let cache = ThreadCache::with_stub_data();

        let messages = cache.get_messages("thread-001");
        assert!(messages.is_some());
        let messages = messages.unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, MessageRole::User);
        assert_eq!(messages[1].role, MessageRole::Assistant);
    }

    #[test]
    fn test_add_message() {
        let mut cache = ThreadCache::new();

        let message = Message {
            id: 100,
            thread_id: "thread-x".to_string(),
            role: MessageRole::User,
            content: "Test message".to_string(),
            created_at: Utc::now(),
            is_streaming: false,
            partial_content: String::new(),
            reasoning_content: String::new(),
            reasoning_collapsed: true,
            segments: Vec::new(),
            render_version: 0,
        };

        cache.add_message(message);

        let messages = cache.get_messages("thread-x");
        assert!(messages.is_some());
        assert_eq!(messages.unwrap().len(), 1);
    }

    #[test]
    fn test_set_messages_replaces() {
        let mut cache = ThreadCache::with_stub_data();

        let new_messages = vec![Message {
            id: 999,
            thread_id: "thread-001".to_string(),
            role: MessageRole::System,
            content: "System message".to_string(),
            created_at: Utc::now(),
            is_streaming: false,
            partial_content: String::new(),
            reasoning_content: String::new(),
            reasoning_collapsed: true,
            segments: Vec::new(),
            render_version: 0,
        }];

        cache.set_messages("thread-001".to_string(), new_messages);

        let messages = cache.get_messages("thread-001").unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].id, 999);
    }

    #[test]
    fn test_add_message_simple_creates_message() {
        let mut cache = ThreadCache::new();
        cache.add_message_simple("thread-x", MessageRole::User, "Hello".to_string());

        let messages = cache.get_messages("thread-x");
        assert!(messages.is_some());

        let messages = messages.unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].id, 1);
        assert_eq!(messages[0].thread_id, "thread-x");
        assert_eq!(messages[0].role, MessageRole::User);
        assert_eq!(messages[0].content, "Hello");
    }

    #[test]
    fn test_add_message_simple_increments_id() {
        let mut cache = ThreadCache::new();
        cache.add_message_simple("thread-x", MessageRole::User, "First".to_string());
        cache.add_message_simple("thread-x", MessageRole::Assistant, "Second".to_string());
        cache.add_message_simple("thread-x", MessageRole::User, "Third".to_string());

        let messages = cache.get_messages("thread-x").unwrap();
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].id, 1);
        assert_eq!(messages[1].id, 2);
        assert_eq!(messages[2].id, 3);
    }

    #[test]
    fn test_append_to_message_accumulates_tokens() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Hello".to_string());

        cache.append_to_message(&thread_id, "Hello");
        cache.append_to_message(&thread_id, " ");
        cache.append_to_message(&thread_id, "world");

        let messages = cache.get_messages(&thread_id).unwrap();
        let streaming_msg = &messages[1];

        assert!(streaming_msg.is_streaming);
        assert_eq!(streaming_msg.partial_content, "Hello world");
        assert!(streaming_msg.content.is_empty()); // Content remains empty until finalized
    }

    #[test]
    fn test_append_to_message_does_nothing_for_nonexistent_thread() {
        let mut cache = ThreadCache::new();

        // Should not panic
        cache.append_to_message("nonexistent", "token");

        // No thread should exist
        assert!(cache.get_messages("nonexistent").is_none());
    }

    #[test]
    fn test_append_to_message_does_nothing_without_streaming_message() {
        let mut cache = ThreadCache::new();
        cache.add_message_simple("thread-x", MessageRole::User, "Hello".to_string());

        // Should not panic
        cache.append_to_message("thread-x", "token");

        // Message should be unchanged
        let messages = cache.get_messages("thread-x").unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "Hello");
    }

    #[test]
    fn test_finalize_message_moves_content() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Hello".to_string());

        cache.append_to_message(&thread_id, "Response ");
        cache.append_to_message(&thread_id, "content");
        cache.finalize_message(&thread_id, 42);

        let messages = cache.get_messages(&thread_id).unwrap();
        let finalized_msg = &messages[1];

        assert!(!finalized_msg.is_streaming);
        assert_eq!(finalized_msg.id, 42);
        assert_eq!(finalized_msg.content, "Response content");
        assert!(finalized_msg.partial_content.is_empty());
    }

    #[test]
    fn test_finalize_message_updates_message_id() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Hello".to_string());

        // Initially message ID is 0
        let messages = cache.get_messages(&thread_id).unwrap();
        assert_eq!(messages[1].id, 0);

        cache.finalize_message(&thread_id, 12345);

        // After finalization, message ID should be updated
        let messages = cache.get_messages(&thread_id).unwrap();
        assert_eq!(messages[1].id, 12345);
    }

    #[test]
    fn test_finalize_message_does_nothing_for_nonexistent_thread() {
        let mut cache = ThreadCache::new();

        // Should not panic
        cache.finalize_message("nonexistent", 42);
    }

    #[test]
    fn test_finalize_message_does_nothing_without_streaming_message() {
        let mut cache = ThreadCache::new();
        cache.add_message_simple("thread-x", MessageRole::User, "Hello".to_string());

        // Should not panic
        cache.finalize_message("thread-x", 42);

        // Message should be unchanged
        let messages = cache.get_messages("thread-x").unwrap();
        assert_eq!(messages[0].id, 1);
        assert_eq!(messages[0].content, "Hello");
    }

    #[test]
    fn test_streaming_full_workflow() {
        let mut cache = ThreadCache::new();

        // Create streaming thread
        let thread_id = cache.create_streaming_thread("What is Rust?".to_string());

        // Verify initial state
        let messages = cache.get_messages(&thread_id).unwrap();
        assert_eq!(messages.len(), 2);
        assert!(messages[1].is_streaming);

        // Stream tokens
        cache.append_to_message(&thread_id, "Rust is ");
        cache.append_to_message(&thread_id, "a systems ");
        cache.append_to_message(&thread_id, "programming language.");

        // Verify streaming state
        let messages = cache.get_messages(&thread_id).unwrap();
        assert!(messages[1].is_streaming);
        assert_eq!(
            messages[1].partial_content,
            "Rust is a systems programming language."
        );
        assert!(messages[1].content.is_empty());

        // Finalize
        cache.finalize_message(&thread_id, 999);

        // Verify final state
        let messages = cache.get_messages(&thread_id).unwrap();
        assert!(!messages[1].is_streaming);
        assert_eq!(messages[1].id, 999);
        assert_eq!(
            messages[1].content,
            "Rust is a systems programming language."
        );
        assert!(messages[1].partial_content.is_empty());
    }

    #[test]
    fn test_get_messages_mut() {
        let mut cache = ThreadCache::new();
        cache.add_message_simple("thread-x", MessageRole::User, "Hello".to_string());

        let messages = cache.get_messages_mut("thread-x");
        assert!(messages.is_some());

        let messages = messages.unwrap();
        messages[0].content = "Modified".to_string();

        // Verify modification persisted
        let messages = cache.get_messages("thread-x").unwrap();
        assert_eq!(messages[0].content, "Modified");
    }

    #[test]
    fn test_get_messages_mut_nonexistent() {
        let mut cache = ThreadCache::new();
        assert!(cache.get_messages_mut("nonexistent").is_none());
    }

    // ============= Add Streaming Message Tests =============

    #[test]
    fn test_add_streaming_message_to_existing_thread() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("First question".to_string());

        // Finalize the first response
        cache.append_to_message(&thread_id, "First answer");
        cache.finalize_message(&thread_id, 1);

        // Add a follow-up message
        let result = cache.add_streaming_message(&thread_id, "Follow-up question".to_string());

        assert!(result);

        let messages = cache.get_messages(&thread_id).unwrap();
        assert_eq!(messages.len(), 4); // Original 2 + new 2

        // Check the new user message
        assert_eq!(messages[2].role, MessageRole::User);
        assert_eq!(messages[2].content, "Follow-up question");
        assert!(!messages[2].is_streaming);
        assert_eq!(messages[2].id, 3); // Next sequential ID

        // Check the new streaming assistant message
        assert_eq!(messages[3].role, MessageRole::Assistant);
        assert!(messages[3].is_streaming);
        assert_eq!(messages[3].id, 0);
    }

    #[test]
    fn test_add_streaming_message_returns_false_for_nonexistent_thread() {
        let mut cache = ThreadCache::new();

        let result = cache.add_streaming_message("nonexistent", "Message".to_string());

        assert!(!result);
        assert!(cache.get_messages("nonexistent").is_none());
    }

    #[test]
    fn test_add_streaming_message_updates_thread_preview() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Original message".to_string());

        // Verify original preview
        assert_eq!(
            cache.get_thread(&thread_id).unwrap().preview,
            "Original message"
        );

        // Add follow-up
        cache.add_streaming_message(&thread_id, "New follow-up message".to_string());

        // Preview should be updated
        assert_eq!(
            cache.get_thread(&thread_id).unwrap().preview,
            "New follow-up message"
        );
    }

    #[test]
    fn test_add_streaming_message_updates_thread_updated_at() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Original".to_string());

        let original_updated_at = cache.get_thread(&thread_id).unwrap().updated_at;

        // Sleep briefly to ensure time difference (or we can just check it's >= original)
        cache.add_streaming_message(&thread_id, "Follow-up".to_string());

        let new_updated_at = cache.get_thread(&thread_id).unwrap().updated_at;
        assert!(new_updated_at >= original_updated_at);
    }

    #[test]
    fn test_add_streaming_message_moves_thread_to_front() {
        let mut cache = ThreadCache::new();

        // Create multiple threads
        let thread1 = cache.create_streaming_thread("Thread 1".to_string());
        let thread2 = cache.create_streaming_thread("Thread 2".to_string());
        let thread3 = cache.create_streaming_thread("Thread 3".to_string());

        // Thread 3 should be at front
        assert_eq!(cache.threads()[0].id, thread3);

        // Add message to thread 1
        cache.add_streaming_message(&thread1, "Follow-up".to_string());

        // Now thread 1 should be at front
        assert_eq!(cache.threads()[0].id, thread1);
        assert_eq!(cache.threads()[1].id, thread3);
        assert_eq!(cache.threads()[2].id, thread2);
    }

    #[test]
    fn test_add_streaming_message_increments_message_ids() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("First".to_string());

        // First thread creates messages with IDs 1 (user) and 0 (streaming assistant)
        cache.finalize_message(&thread_id, 2);

        // Add second exchange
        cache.add_streaming_message(&thread_id, "Second".to_string());

        let messages = cache.get_messages(&thread_id).unwrap();
        // Messages: [user(1), assistant(2), user(3), assistant(0)]
        assert_eq!(messages[2].id, 3);
        assert_eq!(messages[3].id, 0); // Placeholder until finalized
    }

    #[test]
    fn test_add_streaming_message_can_stream_tokens() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("First question".to_string());
        cache.finalize_message(&thread_id, 1);

        // Add follow-up
        cache.add_streaming_message(&thread_id, "Follow-up".to_string());

        // Stream tokens to the new assistant message
        cache.append_to_message(&thread_id, "Response ");
        cache.append_to_message(&thread_id, "content");

        let messages = cache.get_messages(&thread_id).unwrap();
        assert_eq!(messages[3].partial_content, "Response content");
        assert!(messages[3].is_streaming);

        // Finalize
        cache.finalize_message(&thread_id, 99);

        let messages = cache.get_messages(&thread_id).unwrap();
        assert_eq!(messages[3].content, "Response content");
        assert!(!messages[3].is_streaming);
        assert_eq!(messages[3].id, 99);
    }

    #[test]
    fn test_add_streaming_message_full_conversation_workflow() {
        let mut cache = ThreadCache::new();

        // Start conversation with pending thread
        let pending_id = cache.create_pending_thread(
            "What is Rust?".to_string(),
            ThreadType::Conversation,
            None,
        );

        // Stream first response
        cache.append_to_message(&pending_id, "Rust is a systems programming language.");
        cache.finalize_message(&pending_id, 1);

        // Reconcile with backend
        cache.reconcile_thread_id(&pending_id, "thread-abc", Some("Rust Info".to_string()));

        // Add follow-up question
        let result =
            cache.add_streaming_message("thread-abc", "Tell me more about ownership.".to_string());
        assert!(result);

        // Stream second response
        cache.append_to_message("thread-abc", "Ownership is Rust's key feature.");
        cache.finalize_message("thread-abc", 3);

        // Verify final state
        let messages = cache.get_messages("thread-abc").unwrap();
        assert_eq!(messages.len(), 4);

        assert_eq!(messages[0].role, MessageRole::User);
        assert_eq!(messages[0].content, "What is Rust?");

        assert_eq!(messages[1].role, MessageRole::Assistant);
        assert_eq!(
            messages[1].content,
            "Rust is a systems programming language."
        );

        assert_eq!(messages[2].role, MessageRole::User);
        assert_eq!(messages[2].content, "Tell me more about ownership.");

        assert_eq!(messages[3].role, MessageRole::Assistant);
        assert_eq!(messages[3].content, "Ownership is Rust's key feature.");

        // All messages should have correct thread_id
        for msg in messages {
            assert_eq!(msg.thread_id, "thread-abc");
        }

        // Thread should have updated preview
        let thread = cache.get_thread("thread-abc").unwrap();
        assert_eq!(thread.preview, "Tell me more about ownership.");
    }

    #[test]
    fn test_add_streaming_message_to_stub_data_thread() {
        let mut cache = ThreadCache::with_stub_data();

        // Add to an existing stub thread
        let result = cache.add_streaming_message("thread-001", "New question".to_string());
        assert!(result);

        let messages = cache.get_messages("thread-001").unwrap();
        // Original 2 messages + new 2
        assert_eq!(messages.len(), 4);

        // Thread should be moved to front
        assert_eq!(cache.threads()[0].id, "thread-001");
    }

    // ============= is_thread_streaming Tests =============

    #[test]
    fn test_is_thread_streaming_returns_true_when_streaming() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Hello".to_string());

        // Thread has a streaming message
        assert!(cache.is_thread_streaming(&thread_id));
    }

    #[test]
    fn test_is_thread_streaming_returns_false_when_not_streaming() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Hello".to_string());

        // Finalize the streaming message
        cache.finalize_message(&thread_id, 1);

        // No longer streaming
        assert!(!cache.is_thread_streaming(&thread_id));
    }

    #[test]
    fn test_is_thread_streaming_returns_false_for_unknown_thread() {
        let cache = ThreadCache::new();

        // Unknown thread should return false
        assert!(!cache.is_thread_streaming("nonexistent-thread"));
    }

    #[test]
    fn test_is_thread_streaming_with_reconciled_thread() {
        let mut cache = ThreadCache::new();
        let pending_id =
            cache.create_pending_thread("Hello".to_string(), ThreadType::Conversation, None);

        // Reconcile to real ID
        cache.reconcile_thread_id(&pending_id, "real-thread-123", None);

        // Should still be streaming under the real ID
        assert!(cache.is_thread_streaming("real-thread-123"));

        // Should also work with the old pending ID (redirected)
        assert!(cache.is_thread_streaming(&pending_id));
    }

    // ============= Reasoning Tests =============

    #[test]
    fn test_append_reasoning_to_message() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Hello".to_string());

        // Append reasoning tokens to the streaming message
        cache.append_reasoning_to_message(&thread_id, "Let me think");
        cache.append_reasoning_to_message(&thread_id, " about this.");

        let messages = cache.get_messages(&thread_id).unwrap();
        let assistant_msg = &messages[1]; // Second message is assistant

        assert_eq!(assistant_msg.reasoning_content, "Let me think about this.");
    }

    #[test]
    fn test_append_reasoning_to_message_with_pending_id() {
        let mut cache = ThreadCache::new();
        let pending_id =
            cache.create_pending_thread("Hello".to_string(), ThreadType::Conversation, None);

        // Append reasoning tokens
        cache.append_reasoning_to_message(&pending_id, "Reasoning token");

        // Reconcile with backend ID
        cache.reconcile_thread_id(&pending_id, "real-id-123", None);

        // Check reasoning content is accessible via real ID
        let messages = cache.get_messages("real-id-123").unwrap();
        let assistant_msg = &messages[1];
        assert_eq!(assistant_msg.reasoning_content, "Reasoning token");
    }

    #[test]
    fn test_toggle_message_reasoning() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Hello".to_string());

        // Add reasoning content
        cache.append_reasoning_to_message(&thread_id, "Some reasoning");

        // Finalize the message
        cache.finalize_message(&thread_id, 100);

        // After finalize, reasoning should be collapsed
        let messages = cache.get_messages(&thread_id).unwrap();
        assert!(messages[1].reasoning_collapsed);

        // Toggle should return true and uncollapse
        let toggled = cache.toggle_message_reasoning(&thread_id, 1);
        assert!(toggled);

        let messages = cache.get_messages(&thread_id).unwrap();
        assert!(!messages[1].reasoning_collapsed);

        // Toggle again should collapse
        cache.toggle_message_reasoning(&thread_id, 1);
        let messages = cache.get_messages(&thread_id).unwrap();
        assert!(messages[1].reasoning_collapsed);
    }

    #[test]
    fn test_toggle_message_reasoning_no_content() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Hello".to_string());

        // Finalize without adding reasoning content
        cache.finalize_message(&thread_id, 100);

        // Toggle should return false (no reasoning to toggle)
        let toggled = cache.toggle_message_reasoning(&thread_id, 1);
        assert!(!toggled);
    }

    #[test]
    fn test_find_last_reasoning_message_index() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Hello".to_string());

        // Add reasoning and finalize first message
        cache.append_reasoning_to_message(&thread_id, "Reasoning 1");
        cache.finalize_message(&thread_id, 100);

        // Add another exchange
        cache.add_streaming_message(&thread_id, "Second question".to_string());
        cache.append_reasoning_to_message(&thread_id, "Reasoning 2");
        cache.finalize_message(&thread_id, 101);

        // Should find the last assistant message with reasoning (index 3)
        let idx = cache.find_last_reasoning_message_index(&thread_id);
        assert!(idx.is_some());
        assert_eq!(idx.unwrap(), 3); // Index of second assistant message
    }

    #[test]
    fn test_find_last_reasoning_message_index_none() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Hello".to_string());

        // Finalize without adding reasoning
        cache.finalize_message(&thread_id, 100);

        // Should not find any message with reasoning
        let idx = cache.find_last_reasoning_message_index(&thread_id);
        assert!(idx.is_none());
    }

    // ============= set_messages Merge Tests (Race Condition Fix) =============

    #[test]
    fn test_set_messages_merges_streaming_messages() {
        // This tests the critical race condition fix:
        // When a user sends a message before backend returns historical messages,
        // set_messages should merge the incoming messages with local streaming ones.
        let mut cache = ThreadCache::new();
        let thread_id = "thread-123".to_string();

        // Simulate user opening an existing thread and immediately sending a message
        // This creates local messages with streaming assistant placeholder
        let thread = crate::models::Thread {
            id: thread_id.clone(),
            title: "Existing thread".to_string(),
            description: None,
            preview: "Preview".to_string(),
            updated_at: Utc::now(),
            thread_type: ThreadType::Conversation,
            mode: crate::models::ThreadMode::default(),
            model: None,
            permission_mode: None,
            message_count: 2,
            created_at: Utc::now(),
            working_directory: None,
            status: None,
            verified: None,
            verified_at: None,
        };
        cache.upsert_thread(thread);

        // User sends a message - creates local user message (id=3) and streaming assistant (id=0)
        let now = Utc::now();
        let local_user_msg = Message {
            id: 3,
            thread_id: thread_id.clone(),
            role: MessageRole::User,
            content: "New question from user".to_string(),
            created_at: now,
            is_streaming: false,
            partial_content: String::new(),
            reasoning_content: String::new(),
            reasoning_collapsed: true,
            segments: Vec::new(),
            render_version: 0,
        };
        let streaming_assistant_msg = Message {
            id: 0, // Placeholder ID
            thread_id: thread_id.clone(),
            role: MessageRole::Assistant,
            content: String::new(),
            created_at: now,
            is_streaming: true,
            partial_content: "Partial response...".to_string(),
            reasoning_content: String::new(),
            reasoning_collapsed: false,
            segments: Vec::new(),
            render_version: 0,
        };
        cache.set_messages(
            thread_id.clone(),
            vec![local_user_msg.clone(), streaming_assistant_msg.clone()],
        );

        // Backend returns historical messages (older conversation)
        let historical_msg1 = Message {
            id: 1,
            thread_id: thread_id.clone(),
            role: MessageRole::User,
            content: "Old question".to_string(),
            created_at: now - Duration::hours(1),
            is_streaming: false,
            partial_content: String::new(),
            reasoning_content: String::new(),
            reasoning_collapsed: true,
            segments: Vec::new(),
            render_version: 0,
        };
        let historical_msg2 = Message {
            id: 2,
            thread_id: thread_id.clone(),
            role: MessageRole::Assistant,
            content: "Old answer".to_string(),
            created_at: now - Duration::hours(1),
            is_streaming: false,
            partial_content: String::new(),
            reasoning_content: String::new(),
            reasoning_collapsed: true,
            segments: Vec::new(),
            render_version: 0,
        };

        // This is the critical call that would previously REPLACE all messages
        // After fix, it should MERGE with local streaming messages
        cache.set_messages(thread_id.clone(), vec![historical_msg1, historical_msg2]);

        // Verify: should have 4 messages (2 historical + 2 local)
        let messages = cache.get_messages(&thread_id).unwrap();
        assert_eq!(
            messages.len(),
            4,
            "Should have merged 2 historical + 2 local messages"
        );

        // First two should be historical
        assert_eq!(messages[0].id, 1);
        assert_eq!(messages[0].content, "Old question");
        assert_eq!(messages[1].id, 2);
        assert_eq!(messages[1].content, "Old answer");

        // Last two should be local (the new user message and streaming assistant)
        assert_eq!(messages[2].id, 3);
        assert_eq!(messages[2].content, "New question from user");
        assert!(messages[3].is_streaming);
        assert_eq!(messages[3].partial_content, "Partial response...");
    }

    #[test]
    fn test_set_messages_preserves_streaming_message_with_id_zero() {
        let mut cache = ThreadCache::new();
        let thread_id = "thread-456".to_string();

        let thread = crate::models::Thread {
            id: thread_id.clone(),
            title: "Test".to_string(),
            description: None,
            preview: "Preview".to_string(),
            updated_at: Utc::now(),
            thread_type: ThreadType::Conversation,
            mode: crate::models::ThreadMode::default(),
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: Utc::now(),
            working_directory: None,
            status: None,
            verified: None,
            verified_at: None,
        };
        cache.upsert_thread(thread);

        // Create a streaming message with id=0
        let now = Utc::now();
        let streaming_msg = Message {
            id: 0,
            thread_id: thread_id.clone(),
            role: MessageRole::Assistant,
            content: String::new(),
            created_at: now,
            is_streaming: true,
            partial_content: "In progress...".to_string(),
            reasoning_content: String::new(),
            reasoning_collapsed: false,
            segments: Vec::new(),
            render_version: 0,
        };
        cache.set_messages(thread_id.clone(), vec![streaming_msg]);

        // Backend returns empty (no historical messages)
        cache.set_messages(thread_id.clone(), vec![]);

        // Streaming message with id=0 should be preserved
        let messages = cache.get_messages(&thread_id).unwrap();
        assert_eq!(messages.len(), 1);
        assert!(messages[0].is_streaming);
        assert_eq!(messages[0].id, 0);
    }

    #[test]
    fn test_set_messages_replaces_when_no_local_messages() {
        // When there are no local streaming messages, set_messages should replace as before
        let mut cache = ThreadCache::with_stub_data();

        let new_messages = vec![Message {
            id: 999,
            thread_id: "thread-001".to_string(),
            role: MessageRole::System,
            content: "System message".to_string(),
            created_at: Utc::now(),
            is_streaming: false,
            partial_content: String::new(),
            reasoning_content: String::new(),
            reasoning_collapsed: true,
            segments: Vec::new(),
            render_version: 0,
        }];

        cache.set_messages("thread-001".to_string(), new_messages);

        // Should replace (no local messages to preserve)
        let messages = cache.get_messages("thread-001").unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].id, 999);
    }

    #[test]
    fn test_set_messages_preserves_messages_with_higher_ids() {
        // Messages with IDs higher than the max backend ID should be preserved
        let mut cache = ThreadCache::new();
        let thread_id = "thread-789".to_string();

        let thread = crate::models::Thread {
            id: thread_id.clone(),
            title: "Test".to_string(),
            description: None,
            preview: "Preview".to_string(),
            updated_at: Utc::now(),
            thread_type: ThreadType::Conversation,
            mode: crate::models::ThreadMode::default(),
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: Utc::now(),
            working_directory: None,
            status: None,
            verified: None,
            verified_at: None,
        };
        cache.upsert_thread(thread);

        let now = Utc::now();

        // Local message with high ID (e.g., user just sent a message)
        let local_msg = Message {
            id: 100, // Higher than any backend message
            thread_id: thread_id.clone(),
            role: MessageRole::User,
            content: "New message".to_string(),
            created_at: now,
            is_streaming: false,
            partial_content: String::new(),
            reasoning_content: String::new(),
            reasoning_collapsed: true,
            segments: Vec::new(),
            render_version: 0,
        };
        cache.set_messages(thread_id.clone(), vec![local_msg]);

        // Backend returns older messages with lower IDs
        let backend_msg = Message {
            id: 5,
            thread_id: thread_id.clone(),
            role: MessageRole::User,
            content: "Old message".to_string(),
            created_at: now - Duration::hours(1),
            is_streaming: false,
            partial_content: String::new(),
            reasoning_content: String::new(),
            reasoning_collapsed: true,
            segments: Vec::new(),
            render_version: 0,
        };
        cache.set_messages(thread_id.clone(), vec![backend_msg]);

        // Should have both messages
        let messages = cache.get_messages(&thread_id).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].id, 5); // Backend message
        assert_eq!(messages[1].id, 100); // Local message with higher ID
    }
}
