//! Error management methods for ThreadCache

use crate::models::ErrorInfo;

use super::ThreadCache;

impl ThreadCache {
    /// Add an error to a thread's error list
    pub fn add_error(&mut self, thread_id: &str, error: ErrorInfo) {
        let resolved_id = self.resolve_thread_id(thread_id).to_string();
        self.errors.entry(resolved_id).or_default().push(error);
    }

    /// Add an error by code and message (convenience method)
    pub fn add_error_simple(&mut self, thread_id: &str, error_code: String, message: String) {
        let error = ErrorInfo::new(error_code, message);
        self.add_error(thread_id, error);
    }

    /// Get errors for a thread
    pub fn get_errors(&self, thread_id: &str) -> Option<&Vec<ErrorInfo>> {
        let resolved_id = self.resolve_thread_id(thread_id);
        self.errors.get(resolved_id)
    }

    /// Get errors for a thread (mutable)
    #[allow(dead_code)]
    pub fn get_errors_mut(&mut self, thread_id: &str) -> Option<&mut Vec<ErrorInfo>> {
        let resolved_id = self.resolve_thread_id(thread_id).to_string();
        self.errors.get_mut(&resolved_id)
    }

    /// Get the number of errors for a thread
    pub fn error_count(&self, thread_id: &str) -> usize {
        self.get_errors(thread_id).map(|e| e.len()).unwrap_or(0)
    }

    /// Dismiss (remove) an error by its ID
    pub fn dismiss_error(&mut self, thread_id: &str, error_id: &str) -> bool {
        let resolved_id = self.resolve_thread_id(thread_id).to_string();
        if let Some(errors) = self.errors.get_mut(&resolved_id) {
            let before_len = errors.len();
            errors.retain(|e| e.id != error_id);
            let removed = errors.len() < before_len;

            // Adjust focused index if needed
            if removed && self.focused_error_index >= errors.len() && !errors.is_empty() {
                self.focused_error_index = errors.len() - 1;
            }
            return removed;
        }
        false
    }

    /// Dismiss the currently focused error for a thread
    /// Returns true if an error was dismissed
    pub fn dismiss_focused_error(&mut self, thread_id: &str) -> bool {
        let resolved_id = self.resolve_thread_id(thread_id).to_string();
        if let Some(errors) = self.errors.get_mut(&resolved_id) {
            if self.focused_error_index < errors.len() {
                errors.remove(self.focused_error_index);
                // Adjust focused index
                if self.focused_error_index >= errors.len() && !errors.is_empty() {
                    self.focused_error_index = errors.len() - 1;
                }
                return true;
            }
        }
        false
    }

    /// Clear all errors for a thread
    pub fn clear_errors(&mut self, thread_id: &str) {
        let resolved_id = self.resolve_thread_id(thread_id).to_string();
        self.errors.remove(&resolved_id);
        self.focused_error_index = 0;
    }

    /// Get the focused error index
    pub fn focused_error_index(&self) -> usize {
        self.focused_error_index
    }

    /// Set the focused error index
    pub fn set_focused_error_index(&mut self, index: usize) {
        self.focused_error_index = index;
    }

    /// Move focus to next error (wraps around)
    pub fn focus_next_error(&mut self, thread_id: &str) {
        let count = self.error_count(thread_id);
        if count > 0 {
            self.focused_error_index = (self.focused_error_index + 1) % count;
        }
    }

    /// Move focus to previous error (wraps around)
    pub fn focus_prev_error(&mut self, thread_id: &str) {
        let count = self.error_count(thread_id);
        if count > 0 {
            if self.focused_error_index == 0 {
                self.focused_error_index = count - 1;
            } else {
                self.focused_error_index -= 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ThreadType;

    #[test]
    fn test_add_error_to_thread() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Hello".to_string());

        cache.add_error_simple(
            &thread_id,
            "tool_execution_failed".to_string(),
            "File not found".to_string(),
        );

        let errors = cache.get_errors(&thread_id);
        assert!(errors.is_some());
        assert_eq!(errors.unwrap().len(), 1);
        assert_eq!(errors.unwrap()[0].error_code, "tool_execution_failed");
        assert_eq!(errors.unwrap()[0].message, "File not found");
    }

    #[test]
    fn test_add_multiple_errors() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Hello".to_string());

        cache.add_error_simple(&thread_id, "error1".to_string(), "First error".to_string());
        cache.add_error_simple(&thread_id, "error2".to_string(), "Second error".to_string());
        cache.add_error_simple(&thread_id, "error3".to_string(), "Third error".to_string());

        assert_eq!(cache.error_count(&thread_id), 3);
    }

    #[test]
    fn test_error_count_for_nonexistent_thread() {
        let cache = ThreadCache::new();
        assert_eq!(cache.error_count("nonexistent"), 0);
    }

    #[test]
    fn test_get_errors_for_nonexistent_thread() {
        let cache = ThreadCache::new();
        assert!(cache.get_errors("nonexistent").is_none());
    }

    #[test]
    fn test_dismiss_error_by_id() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Hello".to_string());

        cache.add_error_simple(&thread_id, "error1".to_string(), "First".to_string());
        cache.add_error_simple(&thread_id, "error2".to_string(), "Second".to_string());

        let error_id = cache.get_errors(&thread_id).unwrap()[0].id.clone();

        let dismissed = cache.dismiss_error(&thread_id, &error_id);
        assert!(dismissed);
        assert_eq!(cache.error_count(&thread_id), 1);

        // Remaining error should be "error2"
        let remaining = cache.get_errors(&thread_id).unwrap();
        assert_eq!(remaining[0].error_code, "error2");
    }

    #[test]
    fn test_dismiss_nonexistent_error() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Hello".to_string());

        cache.add_error_simple(&thread_id, "error1".to_string(), "First".to_string());

        let dismissed = cache.dismiss_error(&thread_id, "nonexistent-id");
        assert!(!dismissed);
        assert_eq!(cache.error_count(&thread_id), 1);
    }

    #[test]
    fn test_dismiss_focused_error() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Hello".to_string());

        cache.add_error_simple(&thread_id, "error1".to_string(), "First".to_string());
        cache.add_error_simple(&thread_id, "error2".to_string(), "Second".to_string());

        // Focus is at 0 by default
        assert_eq!(cache.focused_error_index(), 0);

        let dismissed = cache.dismiss_focused_error(&thread_id);
        assert!(dismissed);
        assert_eq!(cache.error_count(&thread_id), 1);

        // Remaining error should be "error2"
        let remaining = cache.get_errors(&thread_id).unwrap();
        assert_eq!(remaining[0].error_code, "error2");
    }

    #[test]
    fn test_dismiss_focused_error_adjusts_index() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Hello".to_string());

        cache.add_error_simple(&thread_id, "error1".to_string(), "First".to_string());
        cache.add_error_simple(&thread_id, "error2".to_string(), "Second".to_string());

        // Focus on second error
        cache.set_focused_error_index(1);
        assert_eq!(cache.focused_error_index(), 1);

        cache.dismiss_focused_error(&thread_id);

        // After dismissing, index should adjust to stay in bounds
        assert_eq!(cache.error_count(&thread_id), 1);
        assert_eq!(cache.focused_error_index(), 0);
    }

    #[test]
    fn test_dismiss_focused_error_when_no_errors() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Hello".to_string());

        let dismissed = cache.dismiss_focused_error(&thread_id);
        assert!(!dismissed);
    }

    #[test]
    fn test_clear_errors() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Hello".to_string());

        cache.add_error_simple(&thread_id, "error1".to_string(), "First".to_string());
        cache.add_error_simple(&thread_id, "error2".to_string(), "Second".to_string());
        assert_eq!(cache.error_count(&thread_id), 2);

        cache.clear_errors(&thread_id);

        assert_eq!(cache.error_count(&thread_id), 0);
        assert!(cache.get_errors(&thread_id).is_none());
    }

    #[test]
    fn test_focus_next_error() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Hello".to_string());

        cache.add_error_simple(&thread_id, "error1".to_string(), "First".to_string());
        cache.add_error_simple(&thread_id, "error2".to_string(), "Second".to_string());
        cache.add_error_simple(&thread_id, "error3".to_string(), "Third".to_string());

        assert_eq!(cache.focused_error_index(), 0);

        cache.focus_next_error(&thread_id);
        assert_eq!(cache.focused_error_index(), 1);

        cache.focus_next_error(&thread_id);
        assert_eq!(cache.focused_error_index(), 2);

        // Wraps around
        cache.focus_next_error(&thread_id);
        assert_eq!(cache.focused_error_index(), 0);
    }

    #[test]
    fn test_focus_prev_error() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Hello".to_string());

        cache.add_error_simple(&thread_id, "error1".to_string(), "First".to_string());
        cache.add_error_simple(&thread_id, "error2".to_string(), "Second".to_string());
        cache.add_error_simple(&thread_id, "error3".to_string(), "Third".to_string());

        assert_eq!(cache.focused_error_index(), 0);

        // Wraps around from 0 to last
        cache.focus_prev_error(&thread_id);
        assert_eq!(cache.focused_error_index(), 2);

        cache.focus_prev_error(&thread_id);
        assert_eq!(cache.focused_error_index(), 1);

        cache.focus_prev_error(&thread_id);
        assert_eq!(cache.focused_error_index(), 0);
    }

    #[test]
    fn test_errors_reconciled_with_thread_id() {
        let mut cache = ThreadCache::new();
        let pending_id =
            cache.create_pending_thread("Hello".to_string(), ThreadType::Conversation, None);

        // Add errors using pending ID
        cache.add_error_simple(&pending_id, "error1".to_string(), "First".to_string());
        cache.add_error_simple(&pending_id, "error2".to_string(), "Second".to_string());
        assert_eq!(cache.error_count(&pending_id), 2);

        // Reconcile with backend ID
        cache.reconcile_thread_id(&pending_id, "real-backend-123", None);

        // Errors should be accessible by new ID
        assert_eq!(cache.error_count("real-backend-123"), 2);

        // The old pending ID should now redirect to the real ID
        // (this is intentional for token redirection during streaming)
        // So errors are still accessible via the pending ID (redirected)
        assert_eq!(cache.error_count(&pending_id), 2);

        // Verify errors have correct content
        let errors = cache.get_errors("real-backend-123").unwrap();
        assert_eq!(errors[0].error_code, "error1");
        assert_eq!(errors[1].error_code, "error2");
    }

    #[test]
    fn test_errors_cleared_on_cache_clear() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Hello".to_string());

        cache.add_error_simple(&thread_id, "error1".to_string(), "First".to_string());
        assert_eq!(cache.error_count(&thread_id), 1);

        cache.clear();

        // Errors should be cleared along with other cache data
        assert_eq!(cache.error_count(&thread_id), 0);
    }

    #[test]
    fn test_add_error_with_errorinfo_struct() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Hello".to_string());

        let error = ErrorInfo::new(
            "rate_limit_exceeded".to_string(),
            "Too many requests".to_string(),
        );
        let error_id = error.id.clone();
        cache.add_error(&thread_id, error);

        let errors = cache.get_errors(&thread_id).unwrap();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].id, error_id);
        assert_eq!(errors[0].error_code, "rate_limit_exceeded");
    }

    #[test]
    fn test_dismiss_all_errors_one_by_one() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Hello".to_string());

        cache.add_error_simple(&thread_id, "error1".to_string(), "First".to_string());
        cache.add_error_simple(&thread_id, "error2".to_string(), "Second".to_string());
        cache.add_error_simple(&thread_id, "error3".to_string(), "Third".to_string());
        assert_eq!(cache.error_count(&thread_id), 3);

        // Dismiss all errors one by one using focused dismiss
        cache.dismiss_focused_error(&thread_id);
        assert_eq!(cache.error_count(&thread_id), 2);

        cache.dismiss_focused_error(&thread_id);
        assert_eq!(cache.error_count(&thread_id), 1);

        cache.dismiss_focused_error(&thread_id);
        assert_eq!(cache.error_count(&thread_id), 0);

        // Trying to dismiss when no errors should return false
        let dismissed = cache.dismiss_focused_error(&thread_id);
        assert!(!dismissed);
    }
}
