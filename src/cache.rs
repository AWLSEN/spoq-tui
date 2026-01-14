use std::collections::HashMap;

use chrono::{Duration, Utc};
use uuid::Uuid;

use crate::models::{Message, MessageRole, Thread};

/// Local cache for threads and messages
/// Will fetch from backend in future phases
#[derive(Debug, Default)]
pub struct ThreadCache {
    /// Cached threads indexed by thread ID
    threads: HashMap<String, Thread>,
    /// Cached messages indexed by thread ID
    messages: HashMap<String, Vec<Message>>,
    /// Order of thread IDs (most recent first)
    thread_order: Vec<String>,
    /// Mapping from pending IDs to real IDs for redirecting tokens
    /// When a thread is reconciled, we keep track so streaming tokens using
    /// the old pending ID can be redirected to the correct thread.
    pending_to_real: HashMap<String, String>,
}

impl ThreadCache {
    /// Create a new empty ThreadCache
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a ThreadCache populated with stub data for development
    pub fn with_stub_data() -> Self {
        let mut cache = Self::new();
        cache.populate_stub_data();
        cache
    }

    /// Get all threads in order (most recent first)
    pub fn threads(&self) -> Vec<&Thread> {
        self.thread_order
            .iter()
            .filter_map(|id| self.threads.get(id))
            .collect()
    }

    /// Get a thread by ID
    pub fn get_thread(&self, id: &str) -> Option<&Thread> {
        self.threads.get(id)
    }

    /// Get messages for a thread
    pub fn get_messages(&self, thread_id: &str) -> Option<&Vec<Message>> {
        self.messages.get(thread_id)
    }

    /// Add or update a thread in the cache
    pub fn upsert_thread(&mut self, thread: Thread) {
        let id = thread.id.clone();

        // Update thread order - move to front if exists, otherwise add to front
        self.thread_order.retain(|existing_id| existing_id != &id);
        self.thread_order.insert(0, id.clone());

        self.threads.insert(id, thread);
    }

    /// Add a message to a thread
    pub fn add_message(&mut self, message: Message) {
        let thread_id = message.thread_id.clone();
        self.messages
            .entry(thread_id)
            .or_default()
            .push(message);
    }

    /// Set messages for a thread (replaces existing)
    pub fn set_messages(&mut self, thread_id: String, messages: Vec<Message>) {
        self.messages.insert(thread_id, messages);
    }

    /// Get the number of cached threads
    #[allow(dead_code)]
    pub fn thread_count(&self) -> usize {
        self.threads.len()
    }

    /// Clear all cached data
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.threads.clear();
        self.messages.clear();
        self.thread_order.clear();
    }

    /// Create a stub thread locally (will be replaced by backend call in future)
    /// Returns the thread_id
    pub fn create_stub_thread(&mut self, first_message: String) -> String {
        let thread_id = Uuid::new_v4().to_string();
        let now = Utc::now();

        // Create title from first message (truncate if too long)
        let title = if first_message.len() > 40 {
            format!("{}...", &first_message[..37])
        } else {
            first_message.clone()
        };

        let thread = Thread {
            id: thread_id.clone(),
            title,
            preview: first_message,
            updated_at: now,
        };

        self.upsert_thread(thread);
        thread_id
    }

    /// Add a message to a thread using role and content
    /// This is a convenience method that creates the full Message struct
    pub fn add_message_simple(
        &mut self,
        thread_id: &str,
        role: MessageRole,
        content: String,
    ) {
        let now = Utc::now();

        // Generate a simple message ID based on existing count
        let existing_count = self
            .messages
            .get(thread_id)
            .map(|m| m.len())
            .unwrap_or(0);

        let message = Message {
            id: (existing_count + 1) as i64,
            thread_id: thread_id.to_string(),
            role,
            content,
            created_at: now,
            is_streaming: false,
            partial_content: String::new(),
        };

        self.add_message(message);
    }

    /// Create a new thread with a streaming assistant response
    /// Returns the thread_id for tracking
    pub fn create_streaming_thread(&mut self, first_message: String) -> String {
        let thread_id = Uuid::new_v4().to_string();
        let now = Utc::now();

        // Create title from first message (truncate if too long)
        let title = if first_message.len() > 40 {
            format!("{}...", &first_message[..37])
        } else {
            first_message.clone()
        };

        let thread = Thread {
            id: thread_id.clone(),
            title,
            preview: first_message.clone(),
            updated_at: now,
        };

        self.upsert_thread(thread);

        // Add the user message
        let user_message = Message {
            id: 1,
            thread_id: thread_id.clone(),
            role: MessageRole::User,
            content: first_message,
            created_at: now,
            is_streaming: false,
            partial_content: String::new(),
        };
        self.add_message(user_message);

        // Add placeholder assistant message with is_streaming=true
        let assistant_message = Message {
            id: 0, // Will be updated with real ID from backend
            thread_id: thread_id.clone(),
            role: MessageRole::Assistant,
            content: String::new(),
            created_at: now,
            is_streaming: true,
            partial_content: String::new(),
        };
        self.add_message(assistant_message);

        thread_id
    }

    /// Resolve a thread ID, following pending→real mappings if needed.
    /// This allows streaming tokens sent with the old pending ID to be
    /// redirected to the correct thread after reconciliation.
    fn resolve_thread_id<'a>(&'a self, thread_id: &'a str) -> &'a str {
        self.pending_to_real
            .get(thread_id)
            .map(|s| s.as_str())
            .unwrap_or(thread_id)
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

    /// Reconcile a pending (local) thread ID with the real backend thread ID.
    ///
    /// This is called when we receive the ThreadInfo event from the backend,
    /// which provides the actual thread_id that the backend assigned.
    ///
    /// # Arguments
    /// * `pending_id` - The local UUID we generated before the backend responded
    /// * `real_id` - The actual thread ID from the backend
    /// * `title` - Optional title to update the thread with
    pub fn reconcile_thread_id(
        &mut self,
        pending_id: &str,
        real_id: &str,
        title: Option<String>,
    ) {
        // If pending_id equals real_id, nothing to do (this can happen in some flows)
        if pending_id == real_id {
            // Just update title if provided
            if let Some(new_title) = title {
                if let Some(thread) = self.threads.get_mut(pending_id) {
                    thread.title = new_title;
                }
            }
            return;
        }

        // Remove the thread with pending_id and re-insert with real_id
        if let Some(mut thread) = self.threads.remove(pending_id) {
            thread.id = real_id.to_string();
            if let Some(new_title) = title {
                thread.title = new_title;
            }
            self.threads.insert(real_id.to_string(), thread);
        }

        // Update thread_order to replace pending_id with real_id
        if let Some(pos) = self.thread_order.iter().position(|id| id == pending_id) {
            self.thread_order[pos] = real_id.to_string();
        }

        // Update messages: move from pending_id key to real_id key
        // and update each message's thread_id field
        if let Some(mut messages) = self.messages.remove(pending_id) {
            for msg in &mut messages {
                msg.thread_id = real_id.to_string();
            }
            self.messages.insert(real_id.to_string(), messages);
        }

        // Track the mapping so streaming tokens using the old pending ID
        // can be redirected to the correct thread
        self.pending_to_real
            .insert(pending_id.to_string(), real_id.to_string());
    }

    /// Get mutable access to messages for a thread
    #[allow(dead_code)]
    pub fn get_messages_mut(&mut self, thread_id: &str) -> Option<&mut Vec<Message>> {
        self.messages.get_mut(thread_id)
    }

    /// Create a pending thread (uses temporary ID until backend confirms).
    ///
    /// This creates a thread with a "pending-" prefix that will be reconciled
    /// with the real backend ID once we receive the ThreadInfo event.
    ///
    /// Returns the pending thread_id for tracking.
    pub fn create_pending_thread(&mut self, first_message: String) -> String {
        let pending_id = format!("pending-{}", Uuid::new_v4());
        let now = Utc::now();

        // Create title from first message (truncate if too long)
        let title = if first_message.len() > 40 {
            format!("{}...", &first_message[..37])
        } else {
            first_message.clone()
        };

        let thread = Thread {
            id: pending_id.clone(),
            title,
            preview: first_message.clone(),
            updated_at: now,
        };

        self.upsert_thread(thread);

        // Add the user message
        let user_message = Message {
            id: 1,
            thread_id: pending_id.clone(),
            role: MessageRole::User,
            content: first_message,
            created_at: now,
            is_streaming: false,
            partial_content: String::new(),
        };
        self.add_message(user_message);

        // Add placeholder assistant message with is_streaming=true
        let assistant_message = Message {
            id: 0, // Will be updated with real ID from backend
            thread_id: pending_id.clone(),
            role: MessageRole::Assistant,
            content: String::new(),
            created_at: now,
            is_streaming: true,
            partial_content: String::new(),
        };
        self.add_message(assistant_message);

        pending_id
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

    /// Sync a thread to the server (future implementation)
    ///
    /// TODO: Implement when backend PUT /threads/:id endpoint exists
    /// Expected to update thread title, preview, and updated_at on server
    #[allow(dead_code)]
    pub async fn sync_thread_to_server(&self, _thread: &Thread) -> Result<(), String> {
        // Stub implementation - will be replaced when backend endpoint exists
        // Expected endpoint: PUT /api/threads/:id
        // Expected payload: { title, preview, updated_at }
        Ok(())
    }

    /// Sync a message to the server (future implementation)
    ///
    /// TODO: Implement when backend POST /threads/:id/messages endpoint exists
    /// Expected to create or update a message on the server
    #[allow(dead_code)]
    pub async fn sync_message_to_server(&self, _message: &Message) -> Result<(), String> {
        // Stub implementation - will be replaced when backend endpoint exists
        // Expected endpoint: POST /api/threads/:thread_id/messages
        // Expected payload: { role, content, created_at }
        Ok(())
    }

    /// Populate with stub data for development/testing
    fn populate_stub_data(&mut self) {
        let now = Utc::now();

        // Stub thread 1 - Recent conversation
        let thread1 = Thread {
            id: "thread-001".to_string(),
            title: "Rust async patterns".to_string(),
            preview: "Here's how you can use tokio for async...".to_string(),
            updated_at: now - Duration::minutes(5),
        };

        let messages1 = vec![
            Message {
                id: 1,
                thread_id: "thread-001".to_string(),
                role: MessageRole::User,
                content: "Can you explain Rust async patterns?".to_string(),
                created_at: now - Duration::minutes(10),
                is_streaming: false,
                partial_content: String::new(),
            },
            Message {
                id: 2,
                thread_id: "thread-001".to_string(),
                role: MessageRole::Assistant,
                content: "Here's how you can use tokio for async operations in Rust...".to_string(),
                created_at: now - Duration::minutes(5),
                is_streaming: false,
                partial_content: String::new(),
            },
        ];

        // Stub thread 2 - Older conversation
        let thread2 = Thread {
            id: "thread-002".to_string(),
            title: "TUI design best practices".to_string(),
            preview: "For TUI apps, consider using ratatui...".to_string(),
            updated_at: now - Duration::hours(2),
        };

        let messages2 = vec![
            Message {
                id: 3,
                thread_id: "thread-002".to_string(),
                role: MessageRole::User,
                content: "What are best practices for TUI design?".to_string(),
                created_at: now - Duration::hours(3),
                is_streaming: false,
                partial_content: String::new(),
            },
            Message {
                id: 4,
                thread_id: "thread-002".to_string(),
                role: MessageRole::Assistant,
                content: "For TUI apps, consider using ratatui with a clean layout...".to_string(),
                created_at: now - Duration::hours(2),
                is_streaming: false,
                partial_content: String::new(),
            },
        ];

        // Stub thread 3 - Day old conversation
        let thread3 = Thread {
            id: "thread-003".to_string(),
            title: "API integration help".to_string(),
            preview: "You can use reqwest for HTTP requests...".to_string(),
            updated_at: now - Duration::days(1),
        };

        let messages3 = vec![
            Message {
                id: 5,
                thread_id: "thread-003".to_string(),
                role: MessageRole::User,
                content: "How do I integrate with a REST API in Rust?".to_string(),
                created_at: now - Duration::days(1) - Duration::hours(1),
                is_streaming: false,
                partial_content: String::new(),
            },
            Message {
                id: 6,
                thread_id: "thread-003".to_string(),
                role: MessageRole::Assistant,
                content: "You can use reqwest for HTTP requests. Here's an example...".to_string(),
                created_at: now - Duration::days(1),
                is_streaming: false,
                partial_content: String::new(),
            },
        ];

        // Add threads in reverse chronological order (oldest first)
        // so that the most recent ends up at front after all inserts
        self.upsert_thread(thread3);
        self.upsert_thread(thread2);
        self.upsert_thread(thread1);

        // Add messages
        self.set_messages("thread-001".to_string(), messages1);
        self.set_messages("thread-002".to_string(), messages2);
        self.set_messages("thread-003".to_string(), messages3);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_cache_is_empty() {
        let cache = ThreadCache::new();
        assert_eq!(cache.thread_count(), 0);
        assert!(cache.threads().is_empty());
    }

    #[test]
    fn test_with_stub_data_has_threads() {
        let cache = ThreadCache::with_stub_data();
        assert_eq!(cache.thread_count(), 3);
        assert_eq!(cache.threads().len(), 3);
    }

    #[test]
    fn test_stub_data_thread_order() {
        let cache = ThreadCache::with_stub_data();
        let threads = cache.threads();

        // Most recent thread should be first
        assert_eq!(threads[0].id, "thread-001");
        assert_eq!(threads[1].id, "thread-002");
        assert_eq!(threads[2].id, "thread-003");
    }

    #[test]
    fn test_get_thread_by_id() {
        let cache = ThreadCache::with_stub_data();

        let thread = cache.get_thread("thread-001");
        assert!(thread.is_some());
        assert_eq!(thread.unwrap().title, "Rust async patterns");

        let nonexistent = cache.get_thread("nonexistent");
        assert!(nonexistent.is_none());
    }

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
    fn test_upsert_thread_new() {
        let mut cache = ThreadCache::new();

        let thread = Thread {
            id: "new-thread".to_string(),
            title: "New Thread".to_string(),
            preview: "Preview text".to_string(),
            updated_at: Utc::now(),
        };

        cache.upsert_thread(thread);

        assert_eq!(cache.thread_count(), 1);
        assert!(cache.get_thread("new-thread").is_some());
    }

    #[test]
    fn test_upsert_thread_updates_existing() {
        let mut cache = ThreadCache::with_stub_data();

        let updated_thread = Thread {
            id: "thread-001".to_string(),
            title: "Updated Title".to_string(),
            preview: "Updated preview".to_string(),
            updated_at: Utc::now(),
        };

        cache.upsert_thread(updated_thread);

        // Count should remain the same
        assert_eq!(cache.thread_count(), 3);

        // Thread should be updated
        let thread = cache.get_thread("thread-001").unwrap();
        assert_eq!(thread.title, "Updated Title");

        // Should be moved to front
        assert_eq!(cache.threads()[0].id, "thread-001");
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
        };

        cache.add_message(message);

        let messages = cache.get_messages("thread-x");
        assert!(messages.is_some());
        assert_eq!(messages.unwrap().len(), 1);
    }

    #[test]
    fn test_set_messages_replaces() {
        let mut cache = ThreadCache::with_stub_data();

        let new_messages = vec![
            Message {
                id: 999,
                thread_id: "thread-001".to_string(),
                role: MessageRole::System,
                content: "System message".to_string(),
                created_at: Utc::now(),
                is_streaming: false,
                partial_content: String::new(),
            },
        ];

        cache.set_messages("thread-001".to_string(), new_messages);

        let messages = cache.get_messages("thread-001").unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].id, 999);
    }

    #[test]
    fn test_clear() {
        let mut cache = ThreadCache::with_stub_data();
        assert!(cache.thread_count() > 0);

        cache.clear();

        assert_eq!(cache.thread_count(), 0);
        assert!(cache.threads().is_empty());
        assert!(cache.get_messages("thread-001").is_none());
    }

    #[test]
    fn test_thread_order_maintained_after_upsert() {
        let mut cache = ThreadCache::new();

        // Add three threads
        for i in 1..=3 {
            cache.upsert_thread(Thread {
                id: format!("thread-{}", i),
                title: format!("Thread {}", i),
                preview: "Preview".to_string(),
                updated_at: Utc::now(),
            });
        }

        // Thread 3 should be at front (most recently added)
        assert_eq!(cache.threads()[0].id, "thread-3");

        // Update thread 1
        cache.upsert_thread(Thread {
            id: "thread-1".to_string(),
            title: "Updated Thread 1".to_string(),
            preview: "New preview".to_string(),
            updated_at: Utc::now(),
        });

        // Thread 1 should now be at front
        assert_eq!(cache.threads()[0].id, "thread-1");
        assert_eq!(cache.threads()[1].id, "thread-3");
        assert_eq!(cache.threads()[2].id, "thread-2");
    }

    #[test]
    fn test_create_stub_thread_returns_uuid() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_stub_thread("Hello world".to_string());

        // Should be a valid UUID format
        assert!(thread_id.len() == 36); // UUID format: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
        assert!(thread_id.contains('-'));
    }

    #[test]
    fn test_create_stub_thread_adds_to_cache() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_stub_thread("Test message".to_string());

        let thread = cache.get_thread(&thread_id);
        assert!(thread.is_some());

        let thread = thread.unwrap();
        assert_eq!(thread.id, thread_id);
        assert_eq!(thread.title, "Test message");
        assert_eq!(thread.preview, "Test message");
    }

    #[test]
    fn test_create_stub_thread_truncates_long_title() {
        let mut cache = ThreadCache::new();
        let long_message = "This is a very long message that should be truncated in the title field".to_string();
        let thread_id = cache.create_stub_thread(long_message.clone());

        let thread = cache.get_thread(&thread_id).unwrap();
        // Title should be truncated to 37 chars + "..."
        assert_eq!(thread.title.len(), 40);
        assert!(thread.title.ends_with("..."));
        // Preview should be the full message
        assert_eq!(thread.preview, long_message);
    }

    #[test]
    fn test_create_stub_thread_at_front_of_order() {
        let mut cache = ThreadCache::with_stub_data();
        let initial_count = cache.thread_count();

        let thread_id = cache.create_stub_thread("New thread".to_string());

        assert_eq!(cache.thread_count(), initial_count + 1);
        assert_eq!(cache.threads()[0].id, thread_id);
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

    // ============= Streaming Tests =============

    #[test]
    fn test_create_streaming_thread_returns_uuid() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Hello world".to_string());

        // Should be a valid UUID format
        assert_eq!(thread_id.len(), 36);
        assert!(thread_id.contains('-'));
    }

    #[test]
    fn test_create_streaming_thread_creates_thread() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Test message".to_string());

        let thread = cache.get_thread(&thread_id);
        assert!(thread.is_some());

        let thread = thread.unwrap();
        assert_eq!(thread.id, thread_id);
        assert_eq!(thread.title, "Test message");
        assert_eq!(thread.preview, "Test message");
    }

    #[test]
    fn test_create_streaming_thread_creates_user_message() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("User says hello".to_string());

        let messages = cache.get_messages(&thread_id).unwrap();
        assert_eq!(messages.len(), 2);

        // First message should be user message
        assert_eq!(messages[0].role, MessageRole::User);
        assert_eq!(messages[0].content, "User says hello");
        assert!(!messages[0].is_streaming);
    }

    #[test]
    fn test_create_streaming_thread_creates_streaming_assistant_message() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Hello".to_string());

        let messages = cache.get_messages(&thread_id).unwrap();
        assert_eq!(messages.len(), 2);

        // Second message should be streaming assistant message
        assert_eq!(messages[1].role, MessageRole::Assistant);
        assert_eq!(messages[1].id, 0); // Placeholder ID
        assert!(messages[1].is_streaming);
        assert!(messages[1].content.is_empty());
        assert!(messages[1].partial_content.is_empty());
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
        assert_eq!(messages[1].partial_content, "Rust is a systems programming language.");
        assert!(messages[1].content.is_empty());

        // Finalize
        cache.finalize_message(&thread_id, 999);

        // Verify final state
        let messages = cache.get_messages(&thread_id).unwrap();
        assert!(!messages[1].is_streaming);
        assert_eq!(messages[1].id, 999);
        assert_eq!(messages[1].content, "Rust is a systems programming language.");
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

    // ============= Thread ID Reconciliation Tests =============

    #[test]
    fn test_reconcile_thread_id_updates_thread() {
        let mut cache = ThreadCache::new();
        let pending_id = cache.create_streaming_thread("Hello".to_string());

        // Reconcile with a new real_id
        cache.reconcile_thread_id(&pending_id, "real-backend-id", None);

        // Old ID should not exist
        assert!(cache.get_thread(&pending_id).is_none());
        // New ID should exist
        let thread = cache.get_thread("real-backend-id");
        assert!(thread.is_some());
        assert_eq!(thread.unwrap().id, "real-backend-id");
    }

    #[test]
    fn test_reconcile_thread_id_updates_messages() {
        let mut cache = ThreadCache::new();
        let pending_id = cache.create_streaming_thread("Hello".to_string());

        // Add some tokens
        cache.append_to_message(&pending_id, "Response");

        // Reconcile
        cache.reconcile_thread_id(&pending_id, "real-backend-id", None);

        // Old messages should not exist under old ID
        assert!(cache.get_messages(&pending_id).is_none());

        // Messages should exist under new ID with updated thread_id
        let messages = cache.get_messages("real-backend-id");
        assert!(messages.is_some());
        let messages = messages.unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].thread_id, "real-backend-id");
        assert_eq!(messages[1].thread_id, "real-backend-id");
    }

    #[test]
    fn test_reconcile_thread_id_updates_thread_order() {
        let mut cache = ThreadCache::new();

        // Create multiple threads
        let pending_id = cache.create_streaming_thread("First".to_string());
        cache.create_streaming_thread("Second".to_string());

        // The first thread should still be first in order after reconciliation
        cache.reconcile_thread_id(&pending_id, "real-backend-id", None);

        // Get the thread order (second is at front because it was created last)
        let threads = cache.threads();
        // After reconciliation, "real-backend-id" should be in the list
        let has_real_id = threads.iter().any(|t| t.id == "real-backend-id");
        assert!(has_real_id);
        // Pending ID should not be in the list
        let has_pending_id = threads.iter().any(|t| t.id == pending_id);
        assert!(!has_pending_id);
    }

    #[test]
    fn test_reconcile_thread_id_with_title() {
        let mut cache = ThreadCache::new();
        let pending_id = cache.create_streaming_thread("Original title".to_string());

        // Reconcile with a new title
        cache.reconcile_thread_id(&pending_id, "real-backend-id", Some("New Title".to_string()));

        let thread = cache.get_thread("real-backend-id").unwrap();
        assert_eq!(thread.title, "New Title");
    }

    #[test]
    fn test_reconcile_thread_id_same_id() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Hello".to_string());

        // Reconcile with the same ID (edge case)
        cache.reconcile_thread_id(&thread_id, &thread_id, Some("Updated Title".to_string()));

        // Thread should still exist with updated title
        let thread = cache.get_thread(&thread_id).unwrap();
        assert_eq!(thread.title, "Updated Title");
    }

    #[test]
    fn test_reconcile_thread_id_nonexistent() {
        let mut cache = ThreadCache::new();

        // Should not panic when reconciling nonexistent thread
        cache.reconcile_thread_id("nonexistent", "real-id", None);

        // Neither should exist
        assert!(cache.get_thread("nonexistent").is_none());
        assert!(cache.get_thread("real-id").is_none());
    }

    #[test]
    fn test_reconcile_preserves_thread_data() {
        let mut cache = ThreadCache::new();
        let pending_id = cache.create_streaming_thread("Test message".to_string());

        // Get original preview before reconciliation
        let original_preview = cache.get_thread(&pending_id).unwrap().preview.clone();

        cache.reconcile_thread_id(&pending_id, "real-id", None);

        // Verify original data is preserved
        let thread = cache.get_thread("real-id").unwrap();
        assert_eq!(thread.preview, original_preview);
    }

    // ============= Pending Thread Tests =============

    #[test]
    fn test_create_pending_thread_returns_pending_prefixed_id() {
        let mut cache = ThreadCache::new();
        let pending_id = cache.create_pending_thread("Hello".to_string());

        // Should start with "pending-" prefix
        assert!(pending_id.starts_with("pending-"));
        // Rest should be a UUID (36 chars for standard UUID)
        let uuid_part = &pending_id[8..]; // Skip "pending-"
        assert_eq!(uuid_part.len(), 36);
        assert!(uuid_part.contains('-'));
    }

    #[test]
    fn test_create_pending_thread_creates_thread() {
        let mut cache = ThreadCache::new();
        let pending_id = cache.create_pending_thread("Test message".to_string());

        let thread = cache.get_thread(&pending_id);
        assert!(thread.is_some());

        let thread = thread.unwrap();
        assert_eq!(thread.id, pending_id);
        assert_eq!(thread.title, "Test message");
        assert_eq!(thread.preview, "Test message");
    }

    #[test]
    fn test_create_pending_thread_truncates_long_title() {
        let mut cache = ThreadCache::new();
        let long_message =
            "This is a very long message that should be truncated in the title field".to_string();
        let pending_id = cache.create_pending_thread(long_message.clone());

        let thread = cache.get_thread(&pending_id).unwrap();
        // Title should be truncated to 37 chars + "..."
        assert_eq!(thread.title.len(), 40);
        assert!(thread.title.ends_with("..."));
        // Preview should be the full message
        assert_eq!(thread.preview, long_message);
    }

    #[test]
    fn test_create_pending_thread_creates_messages() {
        let mut cache = ThreadCache::new();
        let pending_id = cache.create_pending_thread("User says hello".to_string());

        let messages = cache.get_messages(&pending_id).unwrap();
        assert_eq!(messages.len(), 2);

        // First message should be user message
        assert_eq!(messages[0].role, MessageRole::User);
        assert_eq!(messages[0].content, "User says hello");
        assert!(!messages[0].is_streaming);
        assert_eq!(messages[0].thread_id, pending_id);

        // Second message should be streaming assistant placeholder
        assert_eq!(messages[1].role, MessageRole::Assistant);
        assert_eq!(messages[1].id, 0);
        assert!(messages[1].is_streaming);
        assert!(messages[1].content.is_empty());
        assert_eq!(messages[1].thread_id, pending_id);
    }

    #[test]
    fn test_create_pending_thread_at_front_of_order() {
        let mut cache = ThreadCache::with_stub_data();
        let initial_count = cache.thread_count();

        let pending_id = cache.create_pending_thread("New pending thread".to_string());

        assert_eq!(cache.thread_count(), initial_count + 1);
        assert_eq!(cache.threads()[0].id, pending_id);
    }

    #[test]
    fn test_create_pending_thread_full_workflow_with_reconciliation() {
        let mut cache = ThreadCache::new();

        // Create pending thread
        let pending_id = cache.create_pending_thread("What is Rust?".to_string());
        assert!(pending_id.starts_with("pending-"));

        // Stream some tokens
        cache.append_to_message(&pending_id, "Rust is ");
        cache.append_to_message(&pending_id, "a systems language.");

        // Verify streaming state
        let messages = cache.get_messages(&pending_id).unwrap();
        assert_eq!(messages[1].partial_content, "Rust is a systems language.");

        // Reconcile with backend ID
        cache.reconcile_thread_id(&pending_id, "backend-thread-123", Some("Rust Programming".to_string()));

        // Verify old ID is gone
        assert!(cache.get_thread(&pending_id).is_none());
        assert!(cache.get_messages(&pending_id).is_none());

        // Verify new ID exists with correct data
        let thread = cache.get_thread("backend-thread-123").unwrap();
        assert_eq!(thread.title, "Rust Programming");

        let messages = cache.get_messages("backend-thread-123").unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].thread_id, "backend-thread-123");
        assert_eq!(messages[1].thread_id, "backend-thread-123");
        assert_eq!(messages[1].partial_content, "Rust is a systems language.");

        // Finalize the message
        cache.finalize_message("backend-thread-123", 42);
        let messages = cache.get_messages("backend-thread-123").unwrap();
        assert!(!messages[1].is_streaming);
        assert_eq!(messages[1].content, "Rust is a systems language.");
    }

    #[test]
    fn test_tokens_redirected_after_reconciliation() {
        // This tests the critical bug fix: when user_message_saved arrives
        // and reconciles the thread ID, subsequent tokens using the OLD pending ID
        // must be redirected to the new real ID.
        let mut cache = ThreadCache::new();

        // Create pending thread
        let pending_id = cache.create_pending_thread("Hello".to_string());
        assert!(pending_id.starts_with("pending-"));

        // Simulate receiving user_message_saved which triggers reconciliation
        // BEFORE all content tokens arrive
        cache.reconcile_thread_id(&pending_id, "real-thread-42", None);

        // Now tokens arrive using the OLD pending ID
        // (this is what the async task does since it captured pending_id at spawn time)
        cache.append_to_message(&pending_id, "Hi ");
        cache.append_to_message(&pending_id, "there!");

        // Tokens should have been redirected to the real thread
        let messages = cache.get_messages("real-thread-42").unwrap();
        assert_eq!(messages.len(), 2); // User message + streaming assistant message
        assert_eq!(messages[1].partial_content, "Hi there!");

        // Finalize also uses the old ID
        cache.finalize_message(&pending_id, 999);
        let messages = cache.get_messages("real-thread-42").unwrap();
        assert!(!messages[1].is_streaming);
        assert_eq!(messages[1].content, "Hi there!");
        assert_eq!(messages[1].id, 999);
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
        assert_eq!(cache.get_thread(&thread_id).unwrap().preview, "Original message");

        // Add follow-up
        cache.add_streaming_message(&thread_id, "New follow-up message".to_string());

        // Preview should be updated
        assert_eq!(cache.get_thread(&thread_id).unwrap().preview, "New follow-up message");
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
        let pending_id = cache.create_pending_thread("What is Rust?".to_string());

        // Stream first response
        cache.append_to_message(&pending_id, "Rust is a systems programming language.");
        cache.finalize_message(&pending_id, 1);

        // Reconcile with backend
        cache.reconcile_thread_id(&pending_id, "thread-abc", Some("Rust Info".to_string()));

        // Add follow-up question
        let result = cache.add_streaming_message("thread-abc", "Tell me more about ownership.".to_string());
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
        assert_eq!(messages[1].content, "Rust is a systems programming language.");

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
        let pending_id = cache.create_pending_thread("Hello".to_string());

        // Reconcile to real ID
        cache.reconcile_thread_id(&pending_id, "real-thread-123", None);

        // Should still be streaming under the real ID
        assert!(cache.is_thread_streaming("real-thread-123"));

        // Should also work with the old pending ID (redirected)
        assert!(cache.is_thread_streaming(&pending_id));
    }
}
