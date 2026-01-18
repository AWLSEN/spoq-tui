//! Thread ID reconciliation and sync methods for ThreadCache

use crate::models::{Message, Thread};

use super::ThreadCache;

impl ThreadCache {
    /// Resolve a thread ID, following pending->real mappings if needed.
    /// This allows streaming tokens sent with the old pending ID to be
    /// redirected to the correct thread after reconciliation.
    pub(crate) fn resolve_thread_id<'a>(&'a self, thread_id: &'a str) -> &'a str {
        self.pending_to_real
            .get(thread_id)
            .map(|s| s.as_str())
            .unwrap_or(thread_id)
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

        // Update errors: move from pending_id key to real_id key
        if let Some(errors) = self.errors.remove(pending_id) {
            self.errors.insert(real_id.to_string(), errors);
        }

        // Track the mapping so streaming tokens using the old pending ID
        // can be redirected to the correct thread
        self.pending_to_real
            .insert(pending_id.to_string(), real_id.to_string());
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ThreadType;

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

    #[test]
    fn test_create_pending_thread_full_workflow_with_reconciliation() {
        let mut cache = ThreadCache::new();

        // Create thread with client-generated UUID
        let thread_id =
            cache.create_pending_thread("What is Rust?".to_string(), ThreadType::Conversation, None);
        assert!(uuid::Uuid::parse_str(&thread_id).is_ok());

        // Stream some tokens
        cache.append_to_message(&thread_id, "Rust is ");
        cache.append_to_message(&thread_id, "a systems language.");

        // Verify streaming state
        let messages = cache.get_messages(&thread_id).unwrap();
        assert_eq!(messages[1].partial_content, "Rust is a systems language.");

        // Reconcile with backend ID (simulates backend returning a different ID)
        cache.reconcile_thread_id(
            &thread_id,
            "backend-thread-123",
            Some("Rust Programming".to_string()),
        );

        // Verify old ID is gone
        assert!(cache.get_thread(&thread_id).is_none());
        assert!(cache.get_messages(&thread_id).is_none());

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
        // and reconciles the thread ID, subsequent tokens using the OLD client-generated ID
        // must be redirected to the new real ID (if backend returns a different ID).
        let mut cache = ThreadCache::new();

        // Create thread with client-generated UUID
        let client_id =
            cache.create_pending_thread("Hello".to_string(), ThreadType::Conversation, None);
        assert!(uuid::Uuid::parse_str(&client_id).is_ok());

        // Simulate receiving user_message_saved which triggers reconciliation
        // BEFORE all content tokens arrive (if backend returns a different ID)
        cache.reconcile_thread_id(&client_id, "real-thread-42", None);

        // Now tokens arrive using the OLD client-generated ID
        // (this is what the async task does since it captured client_id at spawn time)
        cache.append_to_message(&client_id, "Hi ");
        cache.append_to_message(&client_id, "there!");

        // Tokens should have been redirected to the real thread
        let messages = cache.get_messages("real-thread-42").unwrap();
        assert_eq!(messages.len(), 2); // User message + streaming assistant message
        assert_eq!(messages[1].partial_content, "Hi there!");

        // Finalize also uses the old ID
        cache.finalize_message(&client_id, 999);
        let messages = cache.get_messages("real-thread-42").unwrap();
        assert!(!messages[1].is_streaming);
        assert_eq!(messages[1].content, "Hi there!");
        assert_eq!(messages[1].id, 999);
    }
}
