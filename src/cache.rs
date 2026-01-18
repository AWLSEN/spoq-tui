use std::collections::HashMap;
use std::time::Instant;

use chrono::{Duration, Utc};
use uuid::Uuid;

use crate::models::{ErrorInfo, Message, MessageRole, Thread, ThreadType};

/// Eviction timeout in seconds (30 minutes)
const EVICTION_TIMEOUT_SECS: u64 = 30 * 60;

/// Local cache for threads and messages
/// Will fetch from backend in future phases
#[derive(Debug)]
#[derive(Default)]
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
    /// Inline errors per thread (displayed as banners)
    errors: HashMap<String, Vec<ErrorInfo>>,
    /// Index of currently focused error (for dismiss with 'd' key)
    focused_error_index: usize,
    /// Last accessed time for each thread (for LRU eviction)
    last_accessed: HashMap<String, Instant>,
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
            self.last_accessed.insert(thread_id.to_string(), Instant::now());

            // Also move to front of thread_order for MRU
            self.thread_order.retain(|id| id != thread_id);
            self.thread_order.insert(0, thread_id.to_string());
        }
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

        // Update last_accessed time
        self.last_accessed.insert(id.clone(), Instant::now());

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
                .filter(|m| {
                    m.is_streaming || m.id == 0 || m.id > max_backend_id
                })
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
        self.errors.clear();
        self.focused_error_index = 0;
        self.last_accessed.clear();
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
            reasoning_content: String::new(),
            reasoning_collapsed: true,
            segments: Vec::new(),
            render_version: 0,
        };

        self.add_message(message);
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

    /// Start a tool event in the streaming message
    /// Adds a new running ToolEvent to the message's segments
    pub fn start_tool_in_message(&mut self, thread_id: &str, tool_call_id: String, function_name: String) {
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
    pub fn set_tool_result(&mut self, thread_id: &str, tool_call_id: &str, content: &str, is_error: bool) {
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
    pub fn set_tool_display_name(&mut self, thread_id: &str, tool_call_id: &str, display_name: String) {
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

        // Update errors: move from pending_id key to real_id key
        if let Some(errors) = self.errors.remove(pending_id) {
            self.errors.insert(real_id.to_string(), errors);
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

    // ============= Error Management =============

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
    fn populate_stub_data(&mut self) {
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
                content: "Here's how you can use tokio for async operations in Rust...".to_string(),
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

        let new_messages = vec![
            Message {
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
    fn test_create_streaming_thread_uses_default_thread_type() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Hello".to_string());

        let thread = cache.get_thread(&thread_id).unwrap();
        // create_streaming_thread should use default thread type (Normal)
        assert_eq!(thread.thread_type, ThreadType::Conversation);
        assert_eq!(thread.thread_type, ThreadType::default());
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
    fn test_create_pending_thread_returns_uuid() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_pending_thread("Hello".to_string(), ThreadType::Conversation, None);

        // Should be a valid UUID (36 chars for standard UUID format)
        assert_eq!(thread_id.len(), 36);
        assert!(thread_id.contains('-'));
        // Verify it's a valid UUID by parsing
        assert!(uuid::Uuid::parse_str(&thread_id).is_ok());
    }

    #[test]
    fn test_create_pending_thread_creates_thread() {
        let mut cache = ThreadCache::new();
        let pending_id = cache.create_pending_thread("Test message".to_string(), ThreadType::Conversation, None);

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
        let pending_id = cache.create_pending_thread(long_message.clone(), ThreadType::Conversation, None);

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
        let pending_id = cache.create_pending_thread("User says hello".to_string(), ThreadType::Conversation, None);

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

        let pending_id = cache.create_pending_thread("New pending thread".to_string(), ThreadType::Conversation, None);

        assert_eq!(cache.thread_count(), initial_count + 1);
        assert_eq!(cache.threads()[0].id, pending_id);
    }

    #[test]
    fn test_create_pending_thread_full_workflow_with_reconciliation() {
        let mut cache = ThreadCache::new();

        // Create thread with client-generated UUID
        let thread_id = cache.create_pending_thread("What is Rust?".to_string(), ThreadType::Conversation, None);
        assert!(uuid::Uuid::parse_str(&thread_id).is_ok());

        // Stream some tokens
        cache.append_to_message(&thread_id, "Rust is ");
        cache.append_to_message(&thread_id, "a systems language.");

        // Verify streaming state
        let messages = cache.get_messages(&thread_id).unwrap();
        assert_eq!(messages[1].partial_content, "Rust is a systems language.");

        // Reconcile with backend ID (simulates backend returning a different ID)
        cache.reconcile_thread_id(&thread_id, "backend-thread-123", Some("Rust Programming".to_string()));

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
        let client_id = cache.create_pending_thread("Hello".to_string(), ThreadType::Conversation, None);
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
        let pending_id = cache.create_pending_thread("What is Rust?".to_string(), ThreadType::Conversation, None);

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
        let pending_id = cache.create_pending_thread("Hello".to_string(), ThreadType::Conversation, None);

        // Reconcile to real ID
        cache.reconcile_thread_id(&pending_id, "real-thread-123", None);

        // Should still be streaming under the real ID
        assert!(cache.is_thread_streaming("real-thread-123"));

        // Should also work with the old pending ID (redirected)
        assert!(cache.is_thread_streaming(&pending_id));
    }

    // ============= ThreadType Tests =============

    #[test]
    fn test_create_pending_thread_with_conversation_type() {
        let mut cache = ThreadCache::new();
        let pending_id = cache.create_pending_thread("Hello".to_string(), ThreadType::Conversation, None);

        let thread = cache.get_thread(&pending_id).unwrap();
        assert_eq!(thread.thread_type, ThreadType::Conversation);
    }

    #[test]
    fn test_create_pending_thread_with_programming_type() {
        let mut cache = ThreadCache::new();
        let pending_id = cache.create_pending_thread("Help me code".to_string(), ThreadType::Programming, None);

        let thread = cache.get_thread(&pending_id).unwrap();
        assert_eq!(thread.thread_type, ThreadType::Programming);
    }

    #[test]
    fn test_create_pending_thread_preserves_type_after_reconciliation() {
        let mut cache = ThreadCache::new();
        let pending_id = cache.create_pending_thread("Programming task".to_string(), ThreadType::Programming, None);

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

    // ============= Error Management Tests =============

    #[test]
    fn test_add_error_to_thread() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Hello".to_string());

        cache.add_error_simple(&thread_id, "tool_execution_failed".to_string(), "File not found".to_string());

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
        let pending_id = cache.create_pending_thread("Hello".to_string(), ThreadType::Conversation, None);

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

        let error = ErrorInfo::new("rate_limit_exceeded".to_string(), "Too many requests".to_string());
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
        let pending_id = cache.create_pending_thread("Hello".to_string(), ThreadType::Conversation, None);

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

    // ============= update_thread_metadata Tests =============

    #[test]
    fn test_update_thread_metadata_title_only() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Original title".to_string());

        // Update only the title
        let updated = cache.update_thread_metadata(
            &thread_id,
            Some("New title".to_string()),
            None,
        );

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
        let updated = cache.update_thread_metadata(
            &thread_id,
            None,
            Some("New description".to_string()),
        );

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
        let pending_id = cache.create_pending_thread("Original title".to_string(), ThreadType::Conversation, None);

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
        let pending_id = cache.create_pending_thread("Original title".to_string(), ThreadType::Conversation, None);

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
        let updated = cache.update_thread_metadata(
            "thread-123",
            None,
            Some("New description".to_string()),
        );

        assert!(updated);
        let thread = cache.get_thread("thread-123").unwrap();
        assert_eq!(thread.description, Some("New description".to_string()));
    }

    #[test]
    fn test_update_thread_metadata_multiple_updates() {
        let mut cache = ThreadCache::new();
        let thread_id = cache.create_streaming_thread("Original title".to_string());

        // First update: set title
        cache.update_thread_metadata(
            &thread_id,
            Some("Title 1".to_string()),
            None,
        );

        let thread = cache.get_thread(&thread_id).unwrap();
        assert_eq!(thread.title, "Title 1");
        assert!(thread.description.is_none());

        // Second update: set description
        cache.update_thread_metadata(
            &thread_id,
            None,
            Some("Description 1".to_string()),
        );

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
        assert_eq!(thread.description, Some("Thread about async Rust".to_string()));
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

    // ============= Subagent Event Tests =============

    #[test]
    fn test_start_subagent_in_message() {
        use crate::models::SubagentEventStatus;
        use crate::models::MessageSegment;

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

        assert_eq!(subagent.progress_message, Some("Reading src/main.rs".to_string()));
    }

    #[test]
    fn test_update_subagent_progress_after_finalization() {
        use crate::models::MessageSegment;

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
        use crate::models::SubagentEventStatus;

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
        assert_eq!(subagent.summary, Some("Found 10 matching files".to_string()));
        assert_eq!(subagent.tool_call_count, 5);
        assert!(subagent.completed_at.is_some());
        assert!(subagent.duration_secs.is_some());
    }

    #[test]
    fn test_complete_subagent_in_message_without_summary() {
        use crate::models::SubagentEventStatus;

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
        use crate::models::SubagentEventStatus;

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
        cache.complete_subagent_in_message(
            &thread_id,
            "task-late",
            Some("Done".to_string()),
            3,
        );

        let messages = cache.get_messages(&thread_id).unwrap();
        let subagent = messages[1].get_subagent_event("task-late").unwrap();

        assert_eq!(subagent.status, SubagentEventStatus::Complete);
        assert_eq!(subagent.summary, Some("Done".to_string()));
    }

    #[test]
    fn test_subagent_with_reconciled_thread_id() {
        use crate::models::SubagentEventStatus;

        let mut cache = ThreadCache::new();
        let pending_id = cache.create_pending_thread("Hello".to_string(), ThreadType::Conversation, None);

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
        cache.complete_subagent_in_message(&pending_id, "task-pending", Some("Done".to_string()), 1);

        // Verify via real ID
        let messages = cache.get_messages("real-thread-id").unwrap();
        let subagent = messages[1].get_subagent_event("task-pending").unwrap();

        assert_eq!(subagent.status, SubagentEventStatus::Complete);
        assert_eq!(subagent.progress_message, Some("Working".to_string()));
        assert_eq!(subagent.summary, Some("Done".to_string()));
    }

    #[test]
    fn test_multiple_subagents_in_message() {
        use crate::models::SubagentEventStatus;

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
        use crate::models::SubagentEventStatus;

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
        cache.update_subagent_progress(&thread_id, "explore-task", "Processing results".to_string());

        // Progress should reflect last update
        let messages = cache.get_messages(&thread_id).unwrap();
        let subagent = messages[1].get_subagent_event("explore-task").unwrap();
        assert_eq!(subagent.progress_message, Some("Processing results".to_string()));

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
        assert_eq!(subagent.summary, Some("Found 15 TODO comments across 8 files".to_string()));
        assert_eq!(subagent.tool_call_count, 12);
        assert!(subagent.duration_secs.is_some());
    }

    #[test]
    fn test_subagent_interleaved_with_text() {
        use crate::models::MessageSegment;

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
        let thread = Thread {
            id: thread_id.clone(),
            title: "Existing thread".to_string(),
            description: None,
            preview: "Preview".to_string(),
            updated_at: Utc::now(),
            thread_type: ThreadType::Conversation,
            model: None,
            permission_mode: None,
            message_count: 2,
            created_at: Utc::now(),
            working_directory: None,
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
        cache.set_messages(thread_id.clone(), vec![local_user_msg.clone(), streaming_assistant_msg.clone()]);

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
        assert_eq!(messages.len(), 4, "Should have merged 2 historical + 2 local messages");

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

        let thread = Thread {
            id: thread_id.clone(),
            title: "Test".to_string(),
            description: None,
            preview: "Preview".to_string(),
            updated_at: Utc::now(),
            thread_type: ThreadType::Conversation,
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: Utc::now(),
            working_directory: None,
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

        let new_messages = vec![
            Message {
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
            },
        ];

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

        let thread = Thread {
            id: thread_id.clone(),
            title: "Test".to_string(),
            description: None,
            preview: "Preview".to_string(),
            updated_at: Utc::now(),
            thread_type: ThreadType::Conversation,
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: Utc::now(),
            working_directory: None,
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
