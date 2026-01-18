//! Thread management methods for ThreadCache

use chrono::{Duration, Utc};
use std::time::Instant;
use uuid::Uuid;

use crate::models::{Message, MessageRole, Thread, ThreadType};

use super::{ThreadCache, EVICTION_TIMEOUT_SECS};

impl ThreadCache {
    /// Get all threads in order (most recent first), excluding evicted threads
    pub fn threads(&self) -> Vec<&Thread> {
        let now = Instant::now();
        self.thread_order
            .iter()
            .filter_map(|id| {
                // Check if thread is evicted (not accessed in EVICTION_TIMEOUT_SECS)
                if let Some(last_time) = self.last_accessed.get(id) {
                    if now.duration_since(*last_time).as_secs() > EVICTION_TIMEOUT_SECS {
                        return None; // Evicted
                    }
                }
                self.threads.get(id)
            })
            .collect()
    }

    /// Touch a thread to update its last_accessed time (prevents eviction)
    pub fn touch_thread(&mut self, thread_id: &str) {
        if self.threads.contains_key(thread_id) {
            self.last_accessed
                .insert(thread_id.to_string(), Instant::now());

            // Also move to front of thread_order for MRU
            self.thread_order.retain(|id| id != thread_id);
            self.thread_order.insert(0, thread_id.to_string());
        }
    }

    /// Get a thread by ID
    pub fn get_thread(&self, id: &str) -> Option<&Thread> {
        self.threads.get(id)
    }

    /// Get the number of cached threads
    #[allow(dead_code)]
    pub fn thread_count(&self) -> usize {
        self.threads.len()
    }

    /// Add or update a thread in the cache
    pub fn upsert_thread(&mut self, thread: Thread) {
        let id = thread.id.clone();

        // Update thread order - move to front if exists, otherwise add to front
        self.thread_order.retain(|existing_id| existing_id != &id);
        self.thread_order.insert(0, id.clone());

        // Update last_accessed time
        self.last_accessed.insert(id.clone(), Instant::now());

        self.threads.insert(id, thread);
    }

    /// Create a stub thread locally (will be replaced by backend call in future)
    /// Returns the thread_id
    pub fn create_stub_thread(&mut self, first_message: String) -> String {
        let thread_id = Uuid::new_v4().to_string();
        let now = Utc::now();

        // Create title from first message (truncate if too long, respecting UTF-8 boundaries)
        let title = if first_message.len() > 40 {
            let mut end = 37;
            while end > 0 && !first_message.is_char_boundary(end) {
                end -= 1;
            }
            format!("{}...", &first_message[..end])
        } else {
            first_message.clone()
        };

        let thread = Thread {
            id: thread_id.clone(),
            description: None,
            title,
            preview: first_message,
            updated_at: now,
            thread_type: ThreadType::default(),
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: now,
            working_directory: None,
        };

        self.upsert_thread(thread);
        thread_id
    }

    /// Create a new thread with a streaming assistant response
    /// Returns the thread_id for tracking
    pub fn create_streaming_thread(&mut self, first_message: String) -> String {
        let thread_id = Uuid::new_v4().to_string();
        let now = Utc::now();

        // Create title from first message (truncate if too long, respecting UTF-8 boundaries)
        let title = if first_message.len() > 40 {
            let mut end = 37;
            while end > 0 && !first_message.is_char_boundary(end) {
                end -= 1;
            }
            format!("{}...", &first_message[..end])
        } else {
            first_message.clone()
        };

        let thread = Thread {
            id: thread_id.clone(),
            description: None,
            title,
            preview: first_message.clone(),
            updated_at: now,
            thread_type: ThreadType::default(),
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: now,
            working_directory: None,
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
            reasoning_content: String::new(),
            reasoning_collapsed: true,
            segments: Vec::new(),
            render_version: 0,
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
            reasoning_content: String::new(),
            reasoning_collapsed: false, // Show reasoning while streaming
            segments: Vec::new(),
            render_version: 0,
        };
        self.add_message(assistant_message);

        thread_id
    }

    /// Create a new thread with a client-generated UUID.
    ///
    /// The client generates the thread_id upfront and sends it to the backend.
    /// The backend will use this UUID as the canonical thread_id.
    ///
    /// # Arguments
    /// * `first_message` - The initial message content for the thread
    /// * `thread_type` - The type of thread (Normal or Programming)
    ///
    /// Returns the thread_id (a UUID) for tracking.
    pub fn create_pending_thread(
        &mut self,
        first_message: String,
        thread_type: ThreadType,
        working_directory: Option<String>,
    ) -> String {
        let thread_id = Uuid::new_v4().to_string();
        let now = Utc::now();

        // Create title from first message (truncate if too long, respecting UTF-8 boundaries)
        let title = if first_message.len() > 40 {
            let mut end = 37;
            while end > 0 && !first_message.is_char_boundary(end) {
                end -= 1;
            }
            format!("{}...", &first_message[..end])
        } else {
            first_message.clone()
        };

        let thread = Thread {
            id: thread_id.clone(),
            title,
            description: None,
            preview: first_message.clone(),
            updated_at: now,
            thread_type,
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: now,
            working_directory,
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
            reasoning_content: String::new(),
            reasoning_collapsed: true,
            segments: Vec::new(),
            render_version: 0,
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
            reasoning_content: String::new(),
            reasoning_collapsed: false, // Show reasoning while streaming
            segments: Vec::new(),
            render_version: 0,
        };
        self.add_message(assistant_message);

        thread_id
    }

    /// Update thread metadata (title and/or description).
    ///
    /// This method updates the title and/or description of a thread.
    /// It handles pending-to-real ID mapping automatically.
    ///
    /// # Arguments
    /// * `thread_id` - The thread ID (can be pending or real ID)
    /// * `title` - Optional new title for the thread
    /// * `description` - Optional new description for the thread
    ///
    /// # Returns
    /// `true` if the thread was found and updated, `false` if the thread doesn't exist.
    pub fn update_thread_metadata(
        &mut self,
        thread_id: &str,
        title: Option<String>,
        description: Option<String>,
    ) -> bool {
        // Resolve the thread_id in case it's a pending ID that was reconciled
        let resolved_id = self.resolve_thread_id(thread_id).to_string();

        // Try to get the thread
        if let Some(thread) = self.threads.get_mut(&resolved_id) {
            // Update title if provided
            if let Some(new_title) = title {
                thread.title = new_title;
            }

            // Update description if provided
            if let Some(new_description) = description {
                thread.description = Some(new_description);
            }

            true
        } else {
            false
        }
    }

    /// Populate with stub data for development/testing
    pub(crate) fn populate_stub_data(&mut self) {
        let now = Utc::now();

        // Stub thread 1 - Recent conversation
        let thread1 = Thread {
            id: "thread-001".to_string(),
            title: "Rust async patterns".to_string(),
            description: None,
            preview: "Here's how you can use tokio for async...".to_string(),
            updated_at: now - Duration::minutes(5),
            thread_type: ThreadType::default(),
            model: None,
            permission_mode: None,
            message_count: 2,
            created_at: now - Duration::minutes(10),
            working_directory: None,
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
                reasoning_content: String::new(),
                reasoning_collapsed: true,
                segments: Vec::new(),
                render_version: 0,
            },
            Message {
                id: 2,
                thread_id: "thread-001".to_string(),
                role: MessageRole::Assistant,
                content: "Here's how you can use tokio for async operations in Rust..."
                    .to_string(),
                created_at: now - Duration::minutes(5),
                is_streaming: false,
                partial_content: String::new(),
                reasoning_content: String::new(),
                reasoning_collapsed: true,
                segments: Vec::new(),
                render_version: 0,
            },
        ];

        // Stub thread 2 - Older conversation
        let thread2 = Thread {
            id: "thread-002".to_string(),
            title: "TUI design best practices".to_string(),
            description: None,
            preview: "For TUI apps, consider using ratatui...".to_string(),
            updated_at: now - Duration::hours(2),
            thread_type: ThreadType::default(),
            model: None,
            permission_mode: None,
            message_count: 2,
            created_at: now - Duration::hours(3),
            working_directory: None,
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
                reasoning_content: String::new(),
                reasoning_collapsed: true,
                segments: Vec::new(),
                render_version: 0,
            },
            Message {
                id: 4,
                thread_id: "thread-002".to_string(),
                role: MessageRole::Assistant,
                content: "For TUI apps, consider using ratatui with a clean layout...".to_string(),
                created_at: now - Duration::hours(2),
                is_streaming: false,
                partial_content: String::new(),
                reasoning_content: String::new(),
                reasoning_collapsed: true,
                segments: Vec::new(),
                render_version: 0,
            },
        ];

        // Stub thread 3 - Day old conversation
        let thread3 = Thread {
            id: "thread-003".to_string(),
            title: "API integration help".to_string(),
            description: None,
            preview: "You can use reqwest for HTTP requests...".to_string(),
            updated_at: now - Duration::days(1),
            thread_type: ThreadType::default(),
            model: None,
            permission_mode: None,
            message_count: 2,
            created_at: now - Duration::days(1) - Duration::hours(1),
            working_directory: None,
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
                reasoning_content: String::new(),
                reasoning_collapsed: true,
                segments: Vec::new(),
                render_version: 0,
            },
            Message {
                id: 6,
                thread_id: "thread-003".to_string(),
                role: MessageRole::Assistant,
                content: "You can use reqwest for HTTP requests. Here's an example...".to_string(),
                created_at: now - Duration::days(1),
                is_streaming: false,
                partial_content: String::new(),
                reasoning_content: String::new(),
                reasoning_collapsed: true,
                segments: Vec::new(),
                render_version: 0,
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
    fn test_get_thread_by_id() {
        let cache = ThreadCache::with_stub_data();

        let thread = cache.get_thread("thread-001");
        assert!(thread.is_some());
        assert_eq!(thread.unwrap().title, "Rust async patterns");

        let nonexistent = cache.get_thread("nonexistent");
        assert!(nonexistent.is_none());
    }

    #[test]
    fn test_upsert_thread_new() {
        let mut cache = ThreadCache::new();

        let thread = Thread {
            id: "new-thread".to_string(),
            title: "New Thread".to_string(),
            description: None,
            preview: "Preview text".to_string(),
            updated_at: Utc::now(),
            thread_type: ThreadType::default(),
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: Utc::now(),
            working_directory: None,
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
            description: None,
            preview: "Updated preview".to_string(),
            updated_at: Utc::now(),
            thread_type: ThreadType::default(),
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: Utc::now(),
            working_directory: None,
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
    fn test_thread_order_maintained_after_upsert() {
        let mut cache = ThreadCache::new();

        // Add three threads
        for i in 1..=3 {
            cache.upsert_thread(Thread {
                id: format!("thread-{}", i),
                title: format!("Thread {}", i),
                description: None,
                preview: "Preview".to_string(),
                updated_at: Utc::now(),
                thread_type: ThreadType::default(),
                model: None,
                permission_mode: None,
                message_count: 0,
                created_at: Utc::now(),
                working_directory: None,
            });
        }

        // Thread 3 should be at front (most recently added)
        assert_eq!(cache.threads()[0].id, "thread-3");

        // Update thread 1
        cache.upsert_thread(Thread {
            id: "thread-1".to_string(),
            title: "Updated Thread 1".to_string(),
            description: None,
            preview: "New preview".to_string(),
            updated_at: Utc::now(),
            thread_type: ThreadType::default(),
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: Utc::now(),
            working_directory: None,
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
        let long_message =
            "This is a very long message that should be truncated in the title field".to_string();
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
    fn test_create_stub_thread_uses_default_thread_type() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_stub_thread("Hello".to_string());

        let thread = cache.get_thread(&thread_id).unwrap();
        // create_stub_thread should use default thread type (Normal)
        assert_eq!(thread.thread_type, ThreadType::Conversation);
        assert_eq!(thread.thread_type, ThreadType::default());
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
    fn test_create_streaming_thread_uses_default_thread_type() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Hello".to_string());

        let thread = cache.get_thread(&thread_id).unwrap();
        // create_streaming_thread should use default thread type (Normal)
        assert_eq!(thread.thread_type, ThreadType::Conversation);
        assert_eq!(thread.thread_type, ThreadType::default());
    }

    // ============= Pending Thread Tests =============

    #[test]
    fn test_create_pending_thread_returns_uuid() {
        let mut cache = ThreadCache::new();
        let thread_id =
            cache.create_pending_thread("Hello".to_string(), ThreadType::Conversation, None);

        // Should be a valid UUID (36 chars for standard UUID format)
        assert_eq!(thread_id.len(), 36);
        assert!(thread_id.contains('-'));
        // Verify it's a valid UUID by parsing
        assert!(uuid::Uuid::parse_str(&thread_id).is_ok());
    }

    #[test]
    fn test_create_pending_thread_creates_thread() {
        let mut cache = ThreadCache::new();
        let pending_id =
            cache.create_pending_thread("Test message".to_string(), ThreadType::Conversation, None);

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
        let pending_id =
            cache.create_pending_thread(long_message.clone(), ThreadType::Conversation, None);

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
        let pending_id = cache.create_pending_thread(
            "User says hello".to_string(),
            ThreadType::Conversation,
            None,
        );

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

        let pending_id = cache.create_pending_thread(
            "New pending thread".to_string(),
            ThreadType::Conversation,
            None,
        );

        assert_eq!(cache.thread_count(), initial_count + 1);
        assert_eq!(cache.threads()[0].id, pending_id);
    }

    // ============= ThreadType Tests =============

    #[test]
    fn test_create_pending_thread_with_conversation_type() {
        let mut cache = ThreadCache::new();
        let pending_id =
            cache.create_pending_thread("Hello".to_string(), ThreadType::Conversation, None);

        let thread = cache.get_thread(&pending_id).unwrap();
        assert_eq!(thread.thread_type, ThreadType::Conversation);
    }

    #[test]
    fn test_create_pending_thread_with_programming_type() {
        let mut cache = ThreadCache::new();
        let pending_id =
            cache.create_pending_thread("Help me code".to_string(), ThreadType::Programming, None);

        let thread = cache.get_thread(&pending_id).unwrap();
        assert_eq!(thread.thread_type, ThreadType::Programming);
    }

    #[test]
    fn test_create_pending_thread_preserves_type_after_reconciliation() {
        let mut cache = ThreadCache::new();
        let pending_id = cache.create_pending_thread(
            "Programming task".to_string(),
            ThreadType::Programming,
            None,
        );

        // Reconcile with backend ID
        cache.reconcile_thread_id(&pending_id, "real-backend-123", None);

        // Thread type should be preserved
        let thread = cache.get_thread("real-backend-123").unwrap();
        assert_eq!(thread.thread_type, ThreadType::Programming);
    }

    #[test]
    fn test_create_pending_thread_with_working_directory() {
        let mut cache = ThreadCache::new();
        let working_dir = Some("/Users/test/project".to_string());
        let pending_id = cache.create_pending_thread(
            "Code task".to_string(),
            ThreadType::Programming,
            working_dir.clone(),
        );

        let thread = cache.get_thread(&pending_id).unwrap();
        assert_eq!(thread.working_directory, working_dir);
    }

    #[test]
    fn test_create_pending_thread_preserves_working_directory_after_reconciliation() {
        let mut cache = ThreadCache::new();
        let working_dir = Some("/Users/test/my-project".to_string());
        let pending_id = cache.create_pending_thread(
            "Programming task".to_string(),
            ThreadType::Programming,
            working_dir.clone(),
        );

        // Reconcile with backend ID
        cache.reconcile_thread_id(&pending_id, "real-backend-456", None);

        // Working directory should be preserved
        let thread = cache.get_thread("real-backend-456").unwrap();
        assert_eq!(thread.working_directory, working_dir);
    }

    // ============= update_thread_metadata Tests =============

    #[test]
    fn test_update_thread_metadata_title_only() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Original title".to_string());

        // Update only the title
        let updated =
            cache.update_thread_metadata(&thread_id, Some("New title".to_string()), None);

        assert!(updated);
        let thread = cache.get_thread(&thread_id).unwrap();
        assert_eq!(thread.title, "New title");
        assert!(thread.description.is_none());
    }

    #[test]
    fn test_update_thread_metadata_description_only() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Original title".to_string());

        // Update only the description
        let updated =
            cache.update_thread_metadata(&thread_id, None, Some("New description".to_string()));

        assert!(updated);
        let thread = cache.get_thread(&thread_id).unwrap();
        assert_eq!(thread.title, "Original title");
        assert_eq!(thread.description, Some("New description".to_string()));
    }

    #[test]
    fn test_update_thread_metadata_both_fields() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Original title".to_string());

        // Update both title and description
        let updated = cache.update_thread_metadata(
            &thread_id,
            Some("New title".to_string()),
            Some("New description".to_string()),
        );

        assert!(updated);
        let thread = cache.get_thread(&thread_id).unwrap();
        assert_eq!(thread.title, "New title");
        assert_eq!(thread.description, Some("New description".to_string()));
    }

    #[test]
    fn test_update_thread_metadata_neither_field() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Original title".to_string());

        // Call with both None (no-op)
        let updated = cache.update_thread_metadata(&thread_id, None, None);

        assert!(updated);
        let thread = cache.get_thread(&thread_id).unwrap();
        assert_eq!(thread.title, "Original title");
        assert!(thread.description.is_none());
    }

    #[test]
    fn test_update_thread_metadata_nonexistent_thread() {
        let mut cache = ThreadCache::new();

        // Try to update a thread that doesn't exist
        let updated = cache.update_thread_metadata(
            "nonexistent-thread",
            Some("Title".to_string()),
            Some("Description".to_string()),
        );

        assert!(!updated);
    }

    #[test]
    fn test_update_thread_metadata_with_pending_id() {
        let mut cache = ThreadCache::new();
        let pending_id = cache.create_pending_thread(
            "Original title".to_string(),
            ThreadType::Conversation,
            None,
        );

        // Reconcile with backend ID
        cache.reconcile_thread_id(&pending_id, "real-backend-123", None);

        // Update using the old pending ID (should redirect to real ID)
        let updated = cache.update_thread_metadata(
            &pending_id,
            Some("Updated title".to_string()),
            Some("Updated description".to_string()),
        );

        assert!(updated);

        // Check that the real thread was updated
        let thread = cache.get_thread("real-backend-123").unwrap();
        assert_eq!(thread.title, "Updated title");
        assert_eq!(thread.description, Some("Updated description".to_string()));

        // Old pending ID should not exist as a thread
        assert!(cache.get_thread(&pending_id).is_none());
    }

    #[test]
    fn test_update_thread_metadata_with_real_id() {
        let mut cache = ThreadCache::new();
        let pending_id = cache.create_pending_thread(
            "Original title".to_string(),
            ThreadType::Conversation,
            None,
        );

        // Reconcile with backend ID
        cache.reconcile_thread_id(&pending_id, "real-backend-123", None);

        // Update using the real backend ID
        let updated = cache.update_thread_metadata(
            "real-backend-123",
            Some("Updated title".to_string()),
            Some("Updated description".to_string()),
        );

        assert!(updated);

        let thread = cache.get_thread("real-backend-123").unwrap();
        assert_eq!(thread.title, "Updated title");
        assert_eq!(thread.description, Some("Updated description".to_string()));
    }

    #[test]
    fn test_update_thread_metadata_overwrites_existing_description() {
        let mut cache = ThreadCache::new();
        let now = Utc::now();

        // Create a thread with an existing description
        let thread = Thread {
            id: "thread-123".to_string(),
            title: "Original title".to_string(),
            description: Some("Original description".to_string()),
            preview: "Preview".to_string(),
            updated_at: now,
            thread_type: ThreadType::Conversation,
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: now,
            working_directory: None,
        };
        cache.upsert_thread(thread);

        // Update the description
        let updated =
            cache.update_thread_metadata("thread-123", None, Some("New description".to_string()));

        assert!(updated);
        let thread = cache.get_thread("thread-123").unwrap();
        assert_eq!(thread.description, Some("New description".to_string()));
    }

    #[test]
    fn test_update_thread_metadata_multiple_updates() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Original title".to_string());

        // First update: set title
        cache.update_thread_metadata(&thread_id, Some("Title 1".to_string()), None);

        let thread = cache.get_thread(&thread_id).unwrap();
        assert_eq!(thread.title, "Title 1");
        assert!(thread.description.is_none());

        // Second update: set description
        cache.update_thread_metadata(&thread_id, None, Some("Description 1".to_string()));

        let thread = cache.get_thread(&thread_id).unwrap();
        assert_eq!(thread.title, "Title 1");
        assert_eq!(thread.description, Some("Description 1".to_string()));

        // Third update: change both
        cache.update_thread_metadata(
            &thread_id,
            Some("Title 2".to_string()),
            Some("Description 2".to_string()),
        );

        let thread = cache.get_thread(&thread_id).unwrap();
        assert_eq!(thread.title, "Title 2");
        assert_eq!(thread.description, Some("Description 2".to_string()));
    }

    #[test]
    fn test_update_thread_metadata_preserves_other_fields() {
        let mut cache = ThreadCache::new();
        let now = Utc::now();

        // Create a thread with specific fields
        let thread = Thread {
            id: "thread-123".to_string(),
            title: "Original title".to_string(),
            description: None,
            preview: "Preview text".to_string(),
            updated_at: now,
            thread_type: ThreadType::Programming,
            model: Some("gpt-4".to_string()),
            permission_mode: Some("auto".to_string()),
            message_count: 42,
            created_at: now,
            working_directory: None,
        };
        cache.upsert_thread(thread);

        // Update metadata
        cache.update_thread_metadata(
            "thread-123",
            Some("New title".to_string()),
            Some("New description".to_string()),
        );

        // Verify other fields are preserved
        let thread = cache.get_thread("thread-123").unwrap();
        assert_eq!(thread.preview, "Preview text");
        assert_eq!(thread.thread_type, ThreadType::Programming);
        assert_eq!(thread.model, Some("gpt-4".to_string()));
        assert_eq!(thread.permission_mode, Some("auto".to_string()));
        assert_eq!(thread.message_count, 42);
    }

    #[test]
    fn test_update_thread_metadata_with_stub_data() {
        let mut cache = ThreadCache::with_stub_data();

        // Update one of the stub threads
        let updated = cache.update_thread_metadata(
            "thread-001",
            Some("Updated Rust patterns".to_string()),
            Some("Thread about async Rust".to_string()),
        );

        assert!(updated);

        let thread = cache.get_thread("thread-001").unwrap();
        assert_eq!(thread.title, "Updated Rust patterns");
        assert_eq!(
            thread.description,
            Some("Thread about async Rust".to_string())
        );
    }

    #[test]
    fn test_update_thread_metadata_during_streaming() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Original title".to_string());

        // While streaming is in progress
        cache.append_to_message(&thread_id, "Some content");

        // Update metadata during streaming
        let updated = cache.update_thread_metadata(
            &thread_id,
            Some("Updated title".to_string()),
            Some("Updated description".to_string()),
        );

        assert!(updated);
        assert!(cache.is_thread_streaming(&thread_id));

        let thread = cache.get_thread(&thread_id).unwrap();
        assert_eq!(thread.title, "Updated title");
        assert_eq!(thread.description, Some("Updated description".to_string()));
    }
}
