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
        };

        self.add_message(message);
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
            },
            Message {
                id: 2,
                thread_id: "thread-001".to_string(),
                role: MessageRole::Assistant,
                content: "Here's how you can use tokio for async operations in Rust...".to_string(),
                created_at: now - Duration::minutes(5),
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
            },
            Message {
                id: 4,
                thread_id: "thread-002".to_string(),
                role: MessageRole::Assistant,
                content: "For TUI apps, consider using ratatui with a clean layout...".to_string(),
                created_at: now - Duration::hours(2),
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
            },
            Message {
                id: 6,
                thread_id: "thread-003".to_string(),
                role: MessageRole::Assistant,
                content: "You can use reqwest for HTTP requests. Here's an example...".to_string(),
                created_at: now - Duration::days(1),
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
}
