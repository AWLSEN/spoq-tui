use crate::cache::ThreadCache;
use crate::conductor::ConductorClient;
use crate::debug::{
    DebugEvent, DebugEventKind, DebugEventSender, ErrorData, ErrorSource, ProcessedEventData,
    StateChangeData, StateType, StreamLifecycleData, StreamPhase,
};
use crate::events::SseEvent;
use crate::models::{StreamRequest, ThreadType};
use crate::state::{SessionState, SubagentTracker, Task, Thread, Todo, ToolTracker};
use crate::widgets::input_box::InputBox;
use chrono::Utc;
use color_eyre::Result;
use futures_util::StreamExt;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Truncate a string for debug output, adding "..." if truncated.
fn truncate_for_debug(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// Log thread metadata updates to a dedicated file for debugging
fn log_thread_update(message: &str) {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let log_path = format!("{}/spoq_thread.log", home);
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let _ = writeln!(file, "[{}] {}", timestamp, message);
        let _ = file.flush();
    }
}

/// Messages received from async operations (streaming, connection status)
#[derive(Debug, Clone)]
pub enum AppMessage {
    /// A token received during streaming
    StreamToken { thread_id: String, token: String },
    /// A reasoning/thinking token received during streaming
    ReasoningToken { thread_id: String, token: String },
    /// Streaming completed successfully
    StreamComplete { thread_id: String, message_id: i64 },
    /// An error occurred during streaming
    StreamError { thread_id: String, error: String },
    /// Connection status changed
    ConnectionStatus(bool),
    /// Thread created on backend - reconcile pending ID with real ID
    ThreadCreated {
        pending_id: String,
        real_id: String,
        title: Option<String>,
    },
    /// Messages loaded for a thread
    MessagesLoaded {
        thread_id: String,
        messages: Vec<crate::models::Message>,
    },
    /// Error loading messages for a thread
    MessagesLoadError {
        thread_id: String,
        error: String,
    },
    /// Todos updated from the assistant
    TodosUpdated {
        todos: Vec<Todo>,
    },
    /// Permission request from the assistant - needs user approval
    PermissionRequested {
        permission_id: String,
        tool_name: String,
        description: String,
        tool_input: Option<serde_json::Value>,
    },
    /// Tool call started
    ToolStarted {
        tool_call_id: String,
        tool_name: String,
    },
    /// Tool is executing with display info
    ToolExecuting {
        tool_call_id: String,
        display_name: String,
    },
    /// Tool completed with result
    ToolCompleted {
        tool_call_id: String,
        success: bool,
        summary: String,
    },
    /// Skills injected into the session
    SkillsInjected {
        skills: Vec<String>,
    },
    /// OAuth consent required
    OAuthConsentRequired {
        provider: String,
        url: Option<String>,
        skill_name: Option<String>,
    },
    /// Context compacted
    ContextCompacted {
        tokens_used: Option<u32>,
        token_limit: Option<u32>,
    },
    /// Thread metadata updated
    ThreadMetadataUpdated {
        thread_id: String,
        title: Option<String>,
        description: Option<String>,
    },
}

/// Represents which screen is currently active
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Screen {
    #[default]
    CommandDeck,
    Conversation,
}

/// Represents which UI component has focus
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Focus {
    Notifications,
    Tasks,
    #[default]
    Threads,
    Input,
}

/// Represents the current programming mode for Claude interactions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProgrammingMode {
    /// Plan mode - Claude creates plans before executing
    PlanMode,
    /// Bypass permissions - skip confirmation prompts
    BypassPermissions,
    /// No special mode active
    #[default]
    None,
}

/// Main application state
pub struct App {
    /// List of conversation threads (legacy - for storage compatibility)
    pub threads: Vec<Thread>,
    /// List of tasks
    pub tasks: Vec<Task>,
    /// Flag to track if the app should quit
    pub should_quit: bool,
    /// Current screen being displayed
    pub screen: Screen,
    /// ID of the active thread when in Conversation screen
    pub active_thread_id: Option<String>,
    /// Current focus panel
    pub focus: Focus,
    /// Selected index in notifications panel
    pub notifications_index: usize,
    /// Selected index in tasks panel
    pub tasks_index: usize,
    /// Selected index in threads panel
    pub threads_index: usize,
    /// Input box state
    pub input_box: InputBox,
    /// Migration/indexing progress (0-100), None when complete
    pub migration_progress: Option<u8>,
    /// Thread and message cache
    pub cache: ThreadCache,
    /// Receiver for async messages (streaming tokens, connection status)
    pub message_rx: Option<mpsc::UnboundedReceiver<AppMessage>>,
    /// Sender for async messages (clone this to pass to async tasks)
    pub message_tx: mpsc::UnboundedSender<AppMessage>,
    /// Current connection status to the backend
    pub connection_status: bool,
    /// Last stream error for display
    pub stream_error: Option<String>,
    /// Conductor API client (shared across async tasks)
    pub client: Arc<ConductorClient>,
    /// Tick counter for animations (blinking cursor, etc.)
    pub tick_count: u64,
    /// Scroll position for conversation view (0 = bottom/latest content)
    pub conversation_scroll: u16,
    /// Current programming mode for Claude interactions
    pub programming_mode: ProgrammingMode,
    /// Session-level state (skills, permissions, oauth, tokens)
    pub session_state: SessionState,
    /// Tool execution tracking per-thread (cleared on done event)
    pub tool_tracker: ToolTracker,
    /// Session-level todos from the assistant
    /// Subagent activity tracking (cleared on done event)
    pub subagent_tracker: SubagentTracker,
    pub todos: Vec<Todo>,
    /// Debug event sender for emitting internal events to debug server
    pub debug_tx: Option<DebugEventSender>,
}

impl App {
    /// Create a new App instance
    pub fn new() -> Result<Self> {
        Self::with_debug(None)
    }

    /// Create a new App instance with an optional debug event sender
    pub fn with_debug(debug_tx: Option<DebugEventSender>) -> Result<Self> {
        Self::with_client_and_debug(Arc::new(ConductorClient::new()), debug_tx)
    }

    /// Create a new App instance with a custom ConductorClient
    pub fn with_client(client: Arc<ConductorClient>) -> Result<Self> {
        Self::with_client_and_debug(client, None)
    }

    /// Create a new App instance with a custom ConductorClient and optional debug sender
    pub fn with_client_and_debug(
        client: Arc<ConductorClient>,
        debug_tx: Option<DebugEventSender>,
    ) -> Result<Self> {
        // Initialize empty cache - will be populated by initialize()
        let cache = ThreadCache::new();

        // Create the message channel for async communication
        let (message_tx, message_rx) = mpsc::unbounded_channel();

        Ok(Self {
            // Start with empty vectors - will be populated from server in initialize()
            threads: Vec::new(),
            tasks: Vec::new(),
            should_quit: false,
            screen: Screen::default(),
            active_thread_id: None,
            focus: Focus::default(),
            notifications_index: 0,
            tasks_index: 0,
            threads_index: 0,
            input_box: InputBox::new(),
            migration_progress: Some(0),
            cache,
            message_rx: Some(message_rx),
            message_tx,
            connection_status: false,
            stream_error: None,
            client,
            tick_count: 0,
            conversation_scroll: 0,
            programming_mode: ProgrammingMode::default(),
            session_state: SessionState::new(),
            tool_tracker: ToolTracker::new(),
            subagent_tracker: SubagentTracker::new(),
            todos: Vec::new(),
            debug_tx,
        })
    }

    /// Initialize the app by fetching data from the backend.
    ///
    /// Fetches threads and tasks from the server. If the server is unreachable
    /// or returns an error, the app starts with empty state and sets connection
    /// status to false.
    pub async fn initialize(&mut self) {
        // Fetch threads from server
        match self.client.fetch_threads().await {
            Ok(threads) => {
                // Populate cache with threads from server
                for thread in threads {
                    self.cache.upsert_thread(thread);
                }
                self.connection_status = true;
            }
            Err(_) => {
                // Server unreachable - start with empty state
                self.connection_status = false;
            }
        }

        // Fetch tasks from server (only if connected)
        if self.connection_status {
            match self.client.fetch_tasks().await {
                Ok(tasks) => {
                    self.tasks = tasks;
                }
                Err(_) => {
                    // Failed to fetch tasks - continue with empty tasks
                    self.tasks = Vec::new();
                }
            }
        }
    }

    /// Cycle focus to the next panel
    pub fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Notifications => Focus::Tasks,
            Focus::Tasks => Focus::Threads,
            Focus::Threads => Focus::Input,
            Focus::Input => Focus::Notifications,
        };
    }

    /// Cycle programming mode: PlanMode → BypassPermissions → None → PlanMode
    pub fn cycle_programming_mode(&mut self) {
        self.programming_mode = match self.programming_mode {
            ProgrammingMode::PlanMode => ProgrammingMode::BypassPermissions,
            ProgrammingMode::BypassPermissions => ProgrammingMode::None,
            ProgrammingMode::None => ProgrammingMode::PlanMode,
        };
    }

    /// Move selection up in the current focused panel
    pub fn move_up(&mut self) {
        match self.focus {
            Focus::Notifications => {
                if self.notifications_index > 0 {
                    self.notifications_index -= 1;
                }
            }
            Focus::Tasks => {
                if self.tasks_index > 0 {
                    self.tasks_index -= 1;
                }
            }
            Focus::Threads => {
                if self.threads_index > 0 {
                    self.threads_index -= 1;
                }
            }
            Focus::Input => {}
        }
    }

    /// Move selection down in the current focused panel
    pub fn move_down(&mut self, max_notifications: usize, max_tasks: usize, max_threads: usize) {
        match self.focus {
            Focus::Notifications => {
                if max_notifications > 0 && self.notifications_index < max_notifications - 1 {
                    self.notifications_index += 1;
                }
            }
            Focus::Tasks => {
                if max_tasks > 0 && self.tasks_index < max_tasks - 1 {
                    self.tasks_index += 1;
                }
            }
            Focus::Threads => {
                if max_threads > 0 && self.threads_index < max_threads - 1 {
                    self.threads_index += 1;
                }
            }
            Focus::Input => {}
        }
    }

    /// Create a new thread placeholder
    pub fn create_new_thread(&mut self) {
        use crate::state::Thread;
        use chrono::Utc;

        let new_thread = Thread {
            id: format!("thread-{}", self.threads.len() + 1),
            title: "New Thread".to_string(),
            preview: "No messages yet...".to_string(),
            created_at: Utc::now(),
        };
        self.threads.insert(0, new_thread);
        self.threads_index = 0;
        self.focus = Focus::Threads;
    }

    /// Submit the current input, create a streaming thread, and spawn async API call.
    ///
    /// This handles two distinct cases:
    /// 1. NEW thread: When `active_thread_id` is None, creates a new pending thread
    /// 2. CONTINUING thread: When `active_thread_id` exists, adds to the existing thread
    ///
    /// The unified stream endpoint routes based on thread_type parameter.
    /// For programming threads, plan_mode is set based on the current programming mode.
    ///
    /// Edge case: If active_thread_id starts with "pending-", we block submission
    /// because we're still waiting for the backend to confirm the thread ID.
    ///
    /// The `new_thread_type` parameter specifies what type of thread to create if this
    /// is a NEW conversation. It's ignored when continuing an existing thread.
    pub fn submit_input(&mut self, new_thread_type: ThreadType) {
        let content = self.input_box.content().to_string();
        if content.trim().is_empty() {
            return;
        }

        // Determine thread_type based on active thread or new_thread_type parameter
        let thread_type = if self.is_active_thread_programming() {
            ThreadType::Programming
        } else if self.active_thread_id.is_none() {
            // New thread - use the specified type
            new_thread_type
        } else {
            ThreadType::Normal
        };

        // Determine plan_mode for programming threads
        let plan_mode = if thread_type == ThreadType::Programming {
            matches!(self.programming_mode, ProgrammingMode::PlanMode)
        } else {
            false
        };

        // CRITICAL: Determine if this is a NEW thread or CONTINUING existing
        let (thread_id, is_new_thread) = if let Some(existing_id) = &self.active_thread_id {
            // Check if thread is still pending (waiting for backend ThreadInfo)
            if existing_id.starts_with("pending-") {
                // Block rapid second message - still waiting for ThreadInfo
                self.stream_error = Some(
                    "Please wait for the current response to complete before sending another message."
                        .to_string(),
                );
                return;
            }

            // CONTINUING existing thread
            if !self.cache.add_streaming_message(existing_id, content.clone()) {
                // Thread doesn't exist in cache - might have been deleted
                self.stream_error = Some("Thread no longer exists.".to_string());
                return;
            }
            (existing_id.clone(), false)
        } else {
            // NEW thread - create pending, will reconcile when backend responds
            // Use the specified thread type for new conversations
            let pending_id = self.cache.create_pending_thread(content.clone(), new_thread_type);
            self.active_thread_id = Some(pending_id.clone());
            self.screen = Screen::Conversation;
            // Reset scroll for new conversation
            self.conversation_scroll = 0;
            (pending_id, true)
        };

        self.input_box.clear();

        // Emit StreamLifecycle connecting event
        Self::emit_debug(
            &self.debug_tx,
            DebugEventKind::StreamLifecycle(StreamLifecycleData::with_details(
                StreamPhase::Connecting,
                format!("thread: {}, new: {}", thread_id, is_new_thread),
            )),
            Some(&thread_id),
        );

        // Clone what we need for the async task
        let client = Arc::clone(&self.client);
        let message_tx = self.message_tx.clone();
        let thread_id_for_task = thread_id.clone();
        let debug_tx = self.debug_tx.clone();

        // Build unified StreamRequest with thread_type
        let request = if is_new_thread {
            StreamRequest::new(content).with_type(thread_type)
        } else {
            StreamRequest::with_thread(content, thread_id).with_type(thread_type)
        };

        // Apply plan_mode if needed
        let request = if plan_mode {
            request.with_plan_mode(true)
        } else {
            request
        };

        // Spawn async task for unified stream endpoint
        tokio::spawn(async move {
            match client.stream(&request).await {
                Ok(mut stream) => {
                    // Emit StreamLifecycle connected event
                    Self::emit_debug(
                        &debug_tx,
                        DebugEventKind::StreamLifecycle(StreamLifecycleData::new(StreamPhase::Connected)),
                        Some(&thread_id_for_task),
                    );
                    Self::process_stream(&mut stream, &message_tx, &thread_id_for_task, debug_tx).await;
                }
                Err(e) => {
                    // Emit error debug event
                    Self::emit_debug(
                        &debug_tx,
                        DebugEventKind::Error(ErrorData::new(
                            ErrorSource::ConductorApi,
                            &e.to_string(),
                        )),
                        Some(&thread_id_for_task),
                    );
                    let _ = message_tx.send(AppMessage::StreamError {
                        thread_id: thread_id_for_task,
                        error: e.to_string(),
                    });
                }
            }
        });
    }

    /// Helper to emit a debug event if debug channel is available.
    fn emit_debug(debug_tx: &Option<DebugEventSender>, kind: DebugEventKind, thread_id: Option<&str>) {
        if let Some(ref tx) = debug_tx {
            let event = DebugEvent::with_context(kind, thread_id.map(String::from), None);
            let _ = tx.send(event);
        }
    }

    /// Process a stream of SSE events and send messages to the app.
    ///
    /// This is a helper method extracted from submit_input to avoid code duplication
    /// between the programming and standard streaming paths.
    async fn process_stream(
        stream: &mut std::pin::Pin<Box<dyn futures_util::Stream<Item = Result<SseEvent, crate::conductor::ConductorError>> + Send>>,
        message_tx: &mpsc::UnboundedSender<AppMessage>,
        thread_id: &str,
        debug_tx: Option<DebugEventSender>,
    ) {
        while let Some(result) = stream.next().await {
            match result {
                Ok(event) => {
                    match event {
                        SseEvent::Content(content_event) => {
                            // Skip empty tokens (from ping/skills_injected/etc)
                            if content_event.text.is_empty() {
                                continue;
                            }

                            // Emit ProcessedEvent
                            Self::emit_debug(
                                &debug_tx,
                                DebugEventKind::ProcessedEvent(ProcessedEventData::new(
                                    "StreamToken",
                                    format!("token: '{}'", truncate_for_debug(&content_event.text, 50)),
                                )),
                                Some(thread_id),
                            );

                            let _ = message_tx.send(AppMessage::StreamToken {
                                thread_id: thread_id.to_string(),
                                token: content_event.text,
                            });
                        }
                        SseEvent::Done(done_event) => {
                            // Parse message_id from string to i64
                            let message_id = done_event
                                .message_id
                                .parse::<i64>()
                                .unwrap_or(0);

                            // Emit ProcessedEvent
                            Self::emit_debug(
                                &debug_tx,
                                DebugEventKind::ProcessedEvent(ProcessedEventData::new(
                                    "StreamComplete",
                                    format!("message_id: {}", message_id),
                                )),
                                Some(thread_id),
                            );

                            // Emit StreamLifecycle completed
                            Self::emit_debug(
                                &debug_tx,
                                DebugEventKind::StreamLifecycle(StreamLifecycleData::new(StreamPhase::Completed)),
                                Some(thread_id),
                            );

                            let _ = message_tx.send(AppMessage::StreamComplete {
                                thread_id: thread_id.to_string(),
                                message_id,
                            });
                            // Don't break here - continue processing to receive thread_updated
                            // which arrives ~3 seconds after done. Stream will close naturally.
                        }
                        SseEvent::Error(error_event) => {
                            // Emit Error debug event
                            Self::emit_debug(
                                &debug_tx,
                                DebugEventKind::Error(ErrorData::new(
                                    ErrorSource::SseConnection,
                                    &error_event.message,
                                )),
                                Some(thread_id),
                            );

                            let _ = message_tx.send(AppMessage::StreamError {
                                thread_id: thread_id.to_string(),
                                error: error_event.message,
                            });
                            break;
                        }
                        SseEvent::UserMessageSaved(event) => {
                            // ThreadInfo event mapped to UserMessageSaved in conductor.rs
                            // This provides the real backend thread_id
                            Self::emit_debug(
                                &debug_tx,
                                DebugEventKind::ProcessedEvent(ProcessedEventData::new(
                                    "ThreadCreated",
                                    format!("real_id: {}", event.thread_id),
                                )),
                                Some(thread_id),
                            );
                            let _ = message_tx.send(AppMessage::ThreadCreated {
                                pending_id: thread_id.to_string(),
                                real_id: event.thread_id,
                                title: None, // Title not available in this event
                            });
                        }
                        SseEvent::TodosUpdated(todos_event) => {
                            // Convert SSE TodoItems to our Todo type
                            let todos: Vec<Todo> = todos_event.todos.iter()
                                .map(Todo::from_sse)
                                .collect();
                            Self::emit_debug(
                                &debug_tx,
                                DebugEventKind::ProcessedEvent(ProcessedEventData::new(
                                    "TodosUpdated",
                                    format!("{} todos", todos.len()),
                                )),
                                Some(thread_id),
                            );
                            let _ = message_tx.send(AppMessage::TodosUpdated { todos });
                        }
                        SseEvent::PermissionRequest(perm_event) => {
                            Self::emit_debug(
                                &debug_tx,
                                DebugEventKind::ProcessedEvent(ProcessedEventData::new(
                                    "PermissionRequested",
                                    format!("tool: {}", perm_event.tool_name),
                                )),
                                Some(thread_id),
                            );
                            // Send permission request to app for user approval
                            let _ = message_tx.send(AppMessage::PermissionRequested {
                                permission_id: perm_event.permission_id,
                                tool_name: perm_event.tool_name,
                                description: perm_event.description,
                                tool_input: perm_event.tool_input,
                            });
                        }
                        SseEvent::ToolCallStart(tool_event) => {
                            Self::emit_debug(
                                &debug_tx,
                                DebugEventKind::ProcessedEvent(ProcessedEventData::new(
                                    "ToolStarted",
                                    format!("tool: {}", tool_event.tool_name),
                                )),
                                Some(thread_id),
                            );
                            let _ = message_tx.send(AppMessage::ToolStarted {
                                tool_call_id: tool_event.tool_call_id,
                                tool_name: tool_event.tool_name,
                            });
                        }
                        SseEvent::ToolExecuting(tool_event) => {
                            let display_name = tool_event.display_name
                                .or(tool_event.url)
                                .unwrap_or_else(|| "Executing...".to_string());
                            Self::emit_debug(
                                &debug_tx,
                                DebugEventKind::ProcessedEvent(ProcessedEventData::new(
                                    "ToolExecuting",
                                    format!("display: {}", truncate_for_debug(&display_name, 40)),
                                )),
                                Some(thread_id),
                            );
                            let _ = message_tx.send(AppMessage::ToolExecuting {
                                tool_call_id: tool_event.tool_call_id,
                                display_name,
                            });
                        }
                        SseEvent::ToolResult(tool_event) => {
                            // Check if result looks like an error (starts with "Error:")
                            let result = &tool_event.result;
                            let (success, summary) = if result.starts_with("Error:") || result.starts_with("error:") {
                                (false, result.clone())
                            } else {
                                // Summarize successful result
                                let summary = if result.len() > 50 {
                                    format!("{}...", &result[..47])
                                } else if result.is_empty() {
                                    "Complete".to_string()
                                } else {
                                    result.clone()
                                };
                                (true, summary)
                            };
                            Self::emit_debug(
                                &debug_tx,
                                DebugEventKind::ProcessedEvent(ProcessedEventData::new(
                                    "ToolCompleted",
                                    format!("success: {}, summary: {}", success, truncate_for_debug(&summary, 30)),
                                )),
                                Some(thread_id),
                            );
                            let _ = message_tx.send(AppMessage::ToolCompleted {
                                tool_call_id: tool_event.tool_call_id,
                                success,
                                summary,
                            });
                        }
                        SseEvent::SkillsInjected(skills_event) => {
                            Self::emit_debug(
                                &debug_tx,
                                DebugEventKind::ProcessedEvent(ProcessedEventData::new(
                                    "SkillsInjected",
                                    format!("{} skills", skills_event.skills.len()),
                                )),
                                Some(thread_id),
                            );
                            let _ = message_tx.send(AppMessage::SkillsInjected {
                                skills: skills_event.skills,
                            });
                        }
                        SseEvent::OAuthConsentRequired(oauth_event) => {
                            Self::emit_debug(
                                &debug_tx,
                                DebugEventKind::ProcessedEvent(ProcessedEventData::new(
                                    "OAuthConsentRequired",
                                    format!("provider: {}", oauth_event.provider),
                                )),
                                Some(thread_id),
                            );
                            let _ = message_tx.send(AppMessage::OAuthConsentRequired {
                                provider: oauth_event.provider,
                                url: oauth_event.url,
                                skill_name: oauth_event.skill_name,
                            });
                        }
                        SseEvent::ContextCompacted(context_event) => {
                            Self::emit_debug(
                                &debug_tx,
                                DebugEventKind::ProcessedEvent(ProcessedEventData::new(
                                    "ContextCompacted",
                                    format!("tokens: {:?}/{:?}", context_event.tokens_used, context_event.token_limit),
                                )),
                                Some(thread_id),
                            );
                            let _ = message_tx.send(AppMessage::ContextCompacted {
                                tokens_used: context_event.tokens_used,
                                token_limit: context_event.token_limit,
                            });
                        }
                        SseEvent::Reasoning(reasoning_event) => {
                            // Send reasoning tokens to be displayed in collapsible block
                            if !reasoning_event.text.is_empty() {
                                Self::emit_debug(
                                    &debug_tx,
                                    DebugEventKind::ProcessedEvent(ProcessedEventData::new(
                                        "ReasoningToken",
                                        format!("token: '{}'", truncate_for_debug(&reasoning_event.text, 50)),
                                    )),
                                    Some(thread_id),
                                );
                                let _ = message_tx.send(AppMessage::ReasoningToken {
                                    thread_id: thread_id.to_string(),
                                    token: reasoning_event.text,
                                });
                            }
                        }
                        SseEvent::ThreadUpdated(thread_event) => {
                            log_thread_update(&format!(
                                "SSE received thread_updated: id={}, title={:?}, description={:?}",
                                thread_event.thread_id,
                                thread_event.title,
                                thread_event.description
                            ));
                            Self::emit_debug(
                                &debug_tx,
                                DebugEventKind::ProcessedEvent(ProcessedEventData::new(
                                    "ThreadMetadataUpdated",
                                    format!("title: {:?}", thread_event.title),
                                )),
                                Some(thread_id),
                            );
                            let _ = message_tx.send(AppMessage::ThreadMetadataUpdated {
                                thread_id: thread_event.thread_id,
                                title: thread_event.title,
                                description: thread_event.description,
                            });
                        }
                        // Ignore other event types for now
                        _ => {}
                    }
                }
                Err(e) => {
                    // Emit Error debug event for stream errors
                    Self::emit_debug(
                        &debug_tx,
                        DebugEventKind::Error(ErrorData::new(
                            ErrorSource::SseConnection,
                            &e.to_string(),
                        )),
                        Some(thread_id),
                    );
                    let _ = message_tx.send(AppMessage::StreamError {
                        thread_id: thread_id.to_string(),
                        error: e.to_string(),
                    });
                    break;
                }
            }
        }
    }

    /// Mark the app to quit
    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    /// Navigate back to the CommandDeck screen
    pub fn navigate_to_command_deck(&mut self) {
        self.screen = Screen::CommandDeck;
        self.active_thread_id = None;  // Clear so next submit creates new thread
        self.input_box.clear();        // Clear any partial input
    }

    /// Open a specific thread by ID for conversation
    pub fn open_thread(&mut self, thread_id: String) {
        // Set active thread and navigate (existing logic)
        self.active_thread_id = Some(thread_id.clone());
        self.screen = Screen::Conversation;
        self.input_box.clear();
        self.conversation_scroll = 0;

        // Check if messages need to be fetched
        if self.cache.get_messages(&thread_id).is_none() {
            // Spawn async fetch task
            let client = Arc::clone(&self.client);
            let message_tx = self.message_tx.clone();
            let tid = thread_id.clone();

            tokio::spawn(async move {
                match client.fetch_thread_with_messages(&tid).await {
                    Ok(response) => {
                        let messages: Vec<crate::models::Message> = response.messages
                            .into_iter()
                            .enumerate()
                            .map(|(i, m)| m.to_client_message(&tid, i as i64 + 1))
                            .collect();
                        let _ = message_tx.send(AppMessage::MessagesLoaded {
                            thread_id: tid,
                            messages,
                        });
                    }
                    Err(e) => {
                        let _ = message_tx.send(AppMessage::MessagesLoadError {
                            thread_id: tid,
                            error: e.to_string(),
                        });
                    }
                }
            });
        }
    }

    /// Open the currently selected thread from the threads panel
    pub fn open_selected_thread(&mut self) {
        let threads = self.cache.threads();

        // Check if selection is beyond thread list (e.g., "New Thread" button)
        if self.threads_index >= threads.len() {
            // No valid thread selected, just focus input
            self.focus = Focus::Input;
            return;
        }

        let thread_id = threads[self.threads_index].id.clone();
        self.open_thread(thread_id);
    }

    /// Handle an incoming async message
    pub fn handle_message(&mut self, msg: AppMessage) {
        match msg {
            AppMessage::StreamToken { thread_id, token } => {
                self.cache.append_to_message(&thread_id, &token);
                // Emit StateChange for message cache update
                Self::emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::MessageCache,
                        "Message content appended",
                        format!("token: '{}'", truncate_for_debug(&token, 30)),
                    )),
                    Some(&thread_id),
                );
                // Auto-scroll to bottom when new content arrives, but only for the active thread
                if self.active_thread_id.as_ref() == Some(&thread_id) {
                    self.conversation_scroll = 0;
                }
            }
            AppMessage::ReasoningToken { thread_id, token } => {
                self.cache.append_reasoning_to_message(&thread_id, &token);
                // Emit StateChange for reasoning update
                Self::emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::MessageCache,
                        "Reasoning content appended",
                        format!("token: '{}'", truncate_for_debug(&token, 30)),
                    )),
                    Some(&thread_id),
                );
                // Auto-scroll to bottom when new reasoning content arrives, but only for the active thread
                if self.active_thread_id.as_ref() == Some(&thread_id) {
                    self.conversation_scroll = 0;
                }
            }
            AppMessage::StreamComplete {
                thread_id,
                message_id,
            } => {
                self.cache.finalize_message(&thread_id, message_id);
                // Emit StateChange for message finalization
                Self::emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::MessageCache,
                        "Message finalized",
                        format!("message_id: {}", message_id),
                    )),
                    Some(&thread_id),
                );
                // Clear tool tracker when stream completes (ephemeral state)
                self.tool_tracker.clear();
                Self::emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::ToolTracker,
                        "Tool tracker cleared",
                        "cleared",
                    )),
                    Some(&thread_id),
                );
                // Auto-scroll to bottom when stream completes, but only for the active thread
                if self.active_thread_id.as_ref() == Some(&thread_id) {
                    self.conversation_scroll = 0;
                }
            }
            AppMessage::StreamError { thread_id: _, error } => {
                // Emit Error debug event
                Self::emit_debug(
                    &self.debug_tx,
                    DebugEventKind::Error(ErrorData::new(
                        ErrorSource::AppState,
                        &error,
                    )),
                    None,
                );
                self.stream_error = Some(error);
            }
            AppMessage::ConnectionStatus(connected) => {
                // Emit StateChange for connection status
                Self::emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::SessionState,
                        "Connection status changed",
                        format!("connected: {}", connected),
                    )),
                    None,
                );
                self.connection_status = connected;
                if connected {
                    // Clear any previous error when reconnected
                    self.stream_error = None;
                }
            }
            AppMessage::ThreadCreated {
                pending_id,
                real_id,
                title,
            } => {
                // Reconcile the pending local thread ID with the real backend ID
                self.cache
                    .reconcile_thread_id(&pending_id, &real_id, title.clone());
                // Emit StateChange for thread reconciliation
                Self::emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::with_previous(
                        StateType::ThreadCache,
                        "Thread ID reconciled",
                        pending_id.clone(),
                        format!("real_id: {}, title: {:?}", real_id, title),
                    )),
                    Some(&real_id),
                );
                // Update active_thread_id if it matches the pending ID
                if self.active_thread_id.as_ref() == Some(&pending_id) {
                    self.active_thread_id = Some(real_id);
                }
            }
            AppMessage::MessagesLoaded { thread_id, messages } => {
                let count = messages.len();
                self.cache.set_messages(thread_id.clone(), messages);
                // Emit StateChange for messages loaded
                Self::emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::MessageCache,
                        "Messages loaded",
                        format!("{} messages", count),
                    )),
                    Some(&thread_id),
                );
            }
            AppMessage::MessagesLoadError { thread_id: _, error } => {
                // Emit Error debug event
                Self::emit_debug(
                    &self.debug_tx,
                    DebugEventKind::Error(ErrorData::new(
                        ErrorSource::Cache,
                        &error,
                    )),
                    None,
                );
                self.stream_error = Some(error);
            }
            AppMessage::TodosUpdated { todos } => {
                let count = todos.len();
                self.todos = todos;
                // Emit StateChange for todos update
                Self::emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::Todos,
                        "Todos updated",
                        format!("{} items", count),
                    )),
                    None,
                );
            }
            AppMessage::PermissionRequested {
                permission_id,
                tool_name,
                description,
                tool_input,
            } => {
                // Check if this tool is already allowed (user previously chose "Always")
                if self.session_state.is_tool_allowed(&tool_name) {
                    // Auto-approve - send approval back to backend
                    self.approve_permission(&permission_id);
                } else {
                    // Store permission request for user approval
                    use crate::state::PermissionRequest;
                    self.session_state.set_pending_permission(PermissionRequest {
                        permission_id: permission_id.clone(),
                        tool_name: tool_name.clone(),
                        description,
                        context: None, // Context will be extracted from tool_input in UI
                        tool_input,
                    });
                    // Emit StateChange for pending permission
                    Self::emit_debug(
                        &self.debug_tx,
                        DebugEventKind::StateChange(StateChangeData::new(
                            StateType::SessionState,
                            "Permission pending",
                            format!("tool: {}, id: {}", tool_name, permission_id),
                        )),
                        None,
                    );
                }
            }
            AppMessage::ToolStarted { tool_call_id, tool_name } => {
                // Register tool in tracker with display status for UI
                self.tool_tracker.register_tool_started(
                    tool_call_id.clone(),
                    tool_name.clone(),
                    self.tick_count,
                );
                // Emit StateChange for tool tracker
                Self::emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::ToolTracker,
                        "Tool started",
                        format!("tool: {}", tool_name),
                    )),
                    self.active_thread_id.as_deref(),
                );
                // Also add tool event inline to the streaming message
                if let Some(thread_id) = &self.active_thread_id {
                    self.cache.start_tool_in_message(thread_id, tool_call_id, tool_name);
                }
            }
            AppMessage::ToolExecuting { tool_call_id, display_name } => {
                // Update tool to executing state with display info
                self.tool_tracker.set_tool_executing(&tool_call_id, display_name.clone());
                // Emit StateChange for tool executing
                Self::emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::ToolTracker,
                        "Tool executing",
                        format!("display: {}", truncate_for_debug(&display_name, 40)),
                    )),
                    self.active_thread_id.as_deref(),
                );
            }
            AppMessage::ToolCompleted { tool_call_id, success, summary } => {
                // Mark tool as completed with summary for fade display
                self.tool_tracker.complete_tool_with_summary(
                    &tool_call_id,
                    success,
                    summary.clone(),
                    self.tick_count,
                );
                // Emit StateChange for tool completion
                Self::emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::ToolTracker,
                        "Tool completed",
                        format!("success: {}, summary: {}", success, truncate_for_debug(&summary, 30)),
                    )),
                    self.active_thread_id.as_deref(),
                );
                // Also update the inline tool event in the streaming message
                if let Some(thread_id) = &self.active_thread_id {
                    if success {
                        self.cache.complete_tool_in_message(thread_id, &tool_call_id);
                    } else {
                        self.cache.fail_tool_in_message(thread_id, &tool_call_id);
                    }
                }
            }
            AppMessage::SkillsInjected { skills } => {
                let count = skills.len();
                // Update session state with injected skills
                for skill in skills {
                    self.session_state.add_skill(skill);
                }
                // Emit StateChange for skills injection
                Self::emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::SessionState,
                        "Skills injected",
                        format!("{} skills", count),
                    )),
                    None,
                );
            }
            AppMessage::OAuthConsentRequired { provider, url, skill_name } => {
                // Store OAuth requirement in session state
                if let Some(skill) = skill_name {
                    self.session_state.set_oauth_required(provider.clone(), skill);
                }
                if let Some(consent_url) = url {
                    self.session_state.set_oauth_url(consent_url);
                }
                // Emit StateChange for OAuth requirement
                Self::emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::SessionState,
                        "OAuth consent required",
                        format!("provider: {}", provider),
                    )),
                    None,
                );
            }
            AppMessage::ContextCompacted { tokens_used, token_limit } => {
                // Update context tracking in session state
                if let Some(used) = tokens_used {
                    self.session_state.set_context_tokens(used);
                }
                if let Some(limit) = token_limit {
                    self.session_state.set_context_token_limit(limit);
                }
                // Emit StateChange for context compaction
                Self::emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::SessionState,
                        "Context compacted",
                        format!("tokens: {:?}/{:?}", tokens_used, token_limit),
                    )),
                    None,
                );
            }
            AppMessage::ThreadMetadataUpdated { thread_id, title, description } => {
                log_thread_update(&format!(
                    "Updating cache: id={}, title={:?}, description={:?}",
                    thread_id, title, description
                ));
                let updated = self.cache.update_thread_metadata(&thread_id, title.clone(), description.clone());
                log_thread_update(&format!(
                    "Cache update result: id={}, success={}",
                    thread_id, updated
                ));
                // Emit StateChange for thread metadata update
                Self::emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::ThreadCache,
                        "Thread metadata updated",
                        format!("title: {:?}, updated: {}", title, updated),
                    )),
                    Some(&thread_id),
                );
            }
        }
    }

    /// Get a clone of the message sender for passing to async tasks
    pub fn message_sender(&self) -> mpsc::UnboundedSender<AppMessage> {
        self.message_tx.clone()
    }

    /// Spawn an async task to check connection status.
    ///
    /// This calls the ConductorClient health_check and sends the result
    /// via the message channel. The App will update connection_status
    /// when the message is received.
    pub fn check_connection(&self) {
        let tx = self.message_tx.clone();
        let client = Arc::clone(&self.client);
        tokio::spawn(async move {
            let connected = match client.health_check().await {
                Ok(healthy) => healthy,
                Err(_) => false,
            };
            let _ = tx.send(AppMessage::ConnectionStatus(connected));
        });
    }

    /// Clear the current stream error
    pub fn clear_error(&mut self) {
        self.stream_error = None;
    }

    /// Increment the tick counter for animations
    pub fn tick(&mut self) {
        self.tick_count = self.tick_count.wrapping_add(1);
    }

    /// Check if the currently active thread is a Programming thread
    pub fn is_active_thread_programming(&self) -> bool {
        if let Some(thread_id) = &self.active_thread_id {
            if let Some(thread) = self.cache.get_thread(thread_id) {
                return thread.thread_type == crate::models::ThreadType::Programming;
            }
        }
        false
    }

    /// Check if there is currently an active streaming message
    pub fn is_streaming(&self) -> bool {
        if let Some(thread_id) = &self.active_thread_id {
            self.cache.is_thread_streaming(thread_id)
        } else {
            false
        }
    }

    /// Toggle reasoning collapsed state for the last message with reasoning
    /// Returns true if a reasoning block was toggled
    pub fn toggle_reasoning(&mut self) -> bool {
        if let Some(thread_id) = &self.active_thread_id {
            if let Some(idx) = self.cache.find_last_reasoning_message_index(thread_id) {
                return self.cache.toggle_message_reasoning(thread_id, idx);
            }
        }
        false
    }

    /// Dismiss the currently focused error for the active thread
    /// Returns true if an error was dismissed
    pub fn dismiss_focused_error(&mut self) -> bool {
        if let Some(thread_id) = &self.active_thread_id {
            self.cache.dismiss_focused_error(thread_id)
        } else {
            false
        }
    }

    /// Check if the active thread has any errors
    pub fn has_errors(&self) -> bool {
        if let Some(thread_id) = &self.active_thread_id {
            self.cache.error_count(thread_id) > 0
        } else {
            false
        }
    }

    /// Add an error to the active thread
    pub fn add_error_to_active_thread(&mut self, error_code: String, message: String) {
        if let Some(thread_id) = &self.active_thread_id {
            self.cache.add_error_simple(thread_id, error_code, message);
        }
    }

    // ============= Permission Response Methods =============

    /// Approve the current pending permission (user pressed 'y')
    pub fn approve_permission(&mut self, permission_id: &str) {
        // Send approval to backend (spawns async task if runtime available)
        // This check allows unit tests to run without a Tokio runtime
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let client = Arc::clone(&self.client);
            let perm_id = permission_id.to_string();
            handle.spawn(async move {
                let _ = client.respond_to_permission(&perm_id, true).await;
            });
        }

        // Clear the pending permission
        self.session_state.clear_pending_permission();
    }

    /// Deny the current pending permission (user pressed 'n')
    pub fn deny_permission(&mut self, permission_id: &str) {
        // Send denial to backend (spawns async task if runtime available)
        // This check allows unit tests to run without a Tokio runtime
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let client = Arc::clone(&self.client);
            let perm_id = permission_id.to_string();
            handle.spawn(async move {
                let _ = client.respond_to_permission(&perm_id, false).await;
            });
        }

        // Clear the pending permission
        self.session_state.clear_pending_permission();
    }

    /// Allow the tool always for this session and approve (user pressed 'a')
    pub fn allow_tool_always(&mut self, tool_name: &str, permission_id: &str) {
        // Add tool to allowed list
        self.session_state.allow_tool(tool_name.to_string());

        // Approve the current permission
        self.approve_permission(permission_id);
    }

    /// Handle a permission response key press ('y', 'a', or 'n')
    /// Returns true if a permission was handled, false if no pending permission
    pub fn handle_permission_key(&mut self, key: char) -> bool {
        if let Some(ref perm) = self.session_state.pending_permission.clone() {
            match key {
                'y' | 'Y' => {
                    // Allow once
                    self.approve_permission(&perm.permission_id);
                    true
                }
                'a' | 'A' => {
                    // Allow always
                    self.allow_tool_always(&perm.tool_name, &perm.permission_id);
                    true
                }
                'n' | 'N' => {
                    // Deny
                    self.deny_permission(&perm.permission_id);
                    true
                }
                _ => false,
            }
        } else {
            false
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new().expect("Failed to create default App")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::MessageRole;

    #[test]
    fn test_screen_default_is_command_deck() {
        assert_eq!(Screen::default(), Screen::CommandDeck);
    }

    #[test]
    fn test_screen_equality() {
        assert_eq!(Screen::CommandDeck, Screen::CommandDeck);
        assert_eq!(Screen::Conversation, Screen::Conversation);
        assert_ne!(Screen::CommandDeck, Screen::Conversation);
    }

    #[test]
    fn test_screen_copy() {
        let screen = Screen::Conversation;
        let copied = screen;
        assert_eq!(screen, copied);
    }

    #[test]
    fn test_navigate_to_command_deck_from_conversation() {
        let mut app = App::default();
        app.screen = Screen::Conversation;
        app.active_thread_id = Some("thread-123".to_string());
        app.input_box.insert_char('T');
        app.input_box.insert_char('e');
        app.input_box.insert_char('s');
        app.input_box.insert_char('t');

        app.navigate_to_command_deck();

        assert_eq!(app.screen, Screen::CommandDeck);
        assert!(app.active_thread_id.is_none());
        assert!(app.input_box.is_empty());
    }

    #[test]
    fn test_navigate_to_command_deck_when_already_on_command_deck() {
        let mut app = App::default();
        assert_eq!(app.screen, Screen::CommandDeck);
        app.active_thread_id = Some("thread-456".to_string());
        app.input_box.insert_char('H');
        app.input_box.insert_char('i');

        app.navigate_to_command_deck();

        assert_eq!(app.screen, Screen::CommandDeck);
        assert!(app.active_thread_id.is_none());
        assert!(app.input_box.is_empty());
    }

    #[test]
    fn test_app_initializes_with_no_active_thread() {
        let app = App::default();
        assert!(app.active_thread_id.is_none());
    }

    #[test]
    fn test_app_initializes_on_command_deck() {
        let app = App::default();
        assert_eq!(app.screen, Screen::CommandDeck);
    }

    #[test]
    fn test_submit_input_with_empty_input_does_nothing() {
        let mut app = App::default();
        let initial_cache_count = app.cache.thread_count();

        app.submit_input(ThreadType::Normal);

        // Nothing should change with empty input
        assert_eq!(app.cache.thread_count(), initial_cache_count);
        assert_eq!(app.screen, Screen::CommandDeck);
        assert!(app.active_thread_id.is_none());
    }

    #[test]
    fn test_submit_input_with_whitespace_only_does_nothing() {
        let mut app = App::default();
        app.input_box.insert_char(' ');
        app.input_box.insert_char(' ');
        let initial_cache_count = app.cache.thread_count();

        app.submit_input(ThreadType::Normal);

        // Whitespace-only input should be ignored
        assert_eq!(app.cache.thread_count(), initial_cache_count);
        assert_eq!(app.screen, Screen::CommandDeck);
        assert!(app.active_thread_id.is_none());
    }

    #[tokio::test]
    async fn test_submit_input_creates_thread_and_navigates() {
        let mut app = App::default();
        app.input_box.insert_char('H');
        app.input_box.insert_char('i');
        let initial_cache_count = app.cache.thread_count();

        app.submit_input(ThreadType::Normal);

        // Should create a new thread
        assert_eq!(app.cache.thread_count(), initial_cache_count + 1);
        // Should navigate to conversation screen
        assert_eq!(app.screen, Screen::Conversation);
        // Should have an active thread ID that starts with "pending-"
        assert!(app.active_thread_id.is_some());
        assert!(app.active_thread_id.as_ref().unwrap().starts_with("pending-"));
        // Input should be cleared
        assert!(app.input_box.is_empty());
    }

    #[tokio::test]
    async fn test_submit_input_adds_messages_to_thread() {
        let mut app = App::default();
        app.input_box.insert_char('T');
        app.input_box.insert_char('e');
        app.input_box.insert_char('s');
        app.input_box.insert_char('t');

        app.submit_input(ThreadType::Normal);

        let thread_id = app.active_thread_id.as_ref().unwrap();
        let messages = app.cache.get_messages(thread_id);
        assert!(messages.is_some());

        let messages = messages.unwrap();
        // Should have user message and streaming assistant placeholder
        assert_eq!(messages.len(), 2);

        // First message should be the user's input
        assert_eq!(messages[0].role, MessageRole::User);
        assert_eq!(messages[0].content, "Test");

        // Second message should be the streaming assistant placeholder
        assert_eq!(messages[1].role, MessageRole::Assistant);
        assert!(messages[1].is_streaming);
        assert!(messages[1].content.is_empty());
    }

    #[tokio::test]
    async fn test_submit_input_creates_pending_thread_at_front() {
        let mut app = App::default();
        app.input_box.insert_char('N');
        app.input_box.insert_char('e');
        app.input_box.insert_char('w');

        app.submit_input(ThreadType::Normal);

        let thread_id = app.active_thread_id.as_ref().unwrap();
        // The new thread should be at the front of the list and have pending- prefix
        assert_eq!(app.cache.threads()[0].id, *thread_id);
        assert!(thread_id.starts_with("pending-"));
    }

    // ============= New Thread vs Continuing Thread Tests =============

    #[tokio::test]
    async fn test_submit_input_new_thread_when_no_active_thread() {
        let mut app = App::default();
        assert!(app.active_thread_id.is_none());
        app.input_box.insert_char('H');
        app.input_box.insert_char('i');

        app.submit_input(ThreadType::Normal);

        // Should create a pending thread
        let thread_id = app.active_thread_id.as_ref().unwrap();
        assert!(thread_id.starts_with("pending-"));
        // Should navigate to conversation
        assert_eq!(app.screen, Screen::Conversation);
    }

    #[tokio::test]
    async fn test_submit_input_continues_existing_thread() {
        let mut app = App::default();

        // Create an existing thread with a real (non-pending) ID
        let existing_id = "real-thread-123".to_string();
        app.cache.upsert_thread(crate::models::Thread {
            id: existing_id.clone(),
            title: "Existing Thread".to_string(),
            description: None,
            preview: "Previous message".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::default(),
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: chrono::Utc::now(),
        });
        app.cache.add_message_simple(&existing_id, MessageRole::User, "Previous question".to_string());
        app.cache.add_message_simple(&existing_id, MessageRole::Assistant, "Previous answer".to_string());

        // Set as active thread
        app.active_thread_id = Some(existing_id.clone());
        app.screen = Screen::Conversation;

        let initial_msg_count = app.cache.get_messages(&existing_id).unwrap().len();

        // Submit follow-up
        app.input_box.insert_char('F');
        app.input_box.insert_char('o');
        app.input_box.insert_char('l');
        app.input_box.insert_char('l');
        app.input_box.insert_char('o');
        app.input_box.insert_char('w');
        app.submit_input(ThreadType::Normal);

        // Should NOT create a new thread
        assert_eq!(app.active_thread_id.as_ref().unwrap(), &existing_id);
        // Should add messages to existing thread
        let messages = app.cache.get_messages(&existing_id).unwrap();
        assert_eq!(messages.len(), initial_msg_count + 2); // +1 user, +1 streaming assistant

        // Last user message should be our follow-up
        let user_msgs: Vec<_> = messages.iter().filter(|m| m.role == MessageRole::User).collect();
        assert_eq!(user_msgs.last().unwrap().content, "Follow");
    }

    #[tokio::test]
    async fn test_submit_input_blocks_rapid_submit_on_pending_thread() {
        let mut app = App::default();

        // First submit creates pending thread
        app.input_box.insert_char('F');
        app.input_box.insert_char('i');
        app.input_box.insert_char('r');
        app.input_box.insert_char('s');
        app.input_box.insert_char('t');
        app.submit_input(ThreadType::Normal);

        let pending_id = app.active_thread_id.clone().unwrap();
        assert!(pending_id.starts_with("pending-"));

        // Try to submit again while still pending
        app.input_box.insert_char('S');
        app.input_box.insert_char('e');
        app.input_box.insert_char('c');
        app.input_box.insert_char('o');
        app.input_box.insert_char('n');
        app.input_box.insert_char('d');
        app.submit_input(ThreadType::Normal);

        // Should NOT create a new thread or add messages
        // Should set an error
        assert!(app.stream_error.is_some());
        assert!(app.stream_error.as_ref().unwrap().contains("wait"));

        // Input should NOT be cleared (submission was rejected)
        assert!(!app.input_box.is_empty());
        assert_eq!(app.input_box.content(), "Second");

        // Should still be on the pending thread
        assert_eq!(app.active_thread_id, Some(pending_id));
    }

    #[tokio::test]
    async fn test_submit_input_allows_submit_after_thread_reconciled() {
        let mut app = App::default();

        // First submit creates pending thread
        app.input_box.insert_char('F');
        app.input_box.insert_char('i');
        app.input_box.insert_char('r');
        app.input_box.insert_char('s');
        app.input_box.insert_char('t');
        app.submit_input(ThreadType::Normal);

        let pending_id = app.active_thread_id.clone().unwrap();

        // Simulate backend responding with ThreadCreated
        app.handle_message(AppMessage::ThreadCreated {
            pending_id: pending_id.clone(),
            real_id: "real-backend-id".to_string(),
            title: None,
        });

        // Verify reconciliation happened
        assert_eq!(app.active_thread_id, Some("real-backend-id".to_string()));

        // Finalize the first response
        app.cache.append_to_message("real-backend-id", "First response");
        app.cache.finalize_message("real-backend-id", 1);

        // Now second submit should work
        app.input_box.insert_char('S');
        app.input_box.insert_char('e');
        app.input_box.insert_char('c');
        app.input_box.insert_char('o');
        app.input_box.insert_char('n');
        app.input_box.insert_char('d');
        let before_count = app.cache.get_messages("real-backend-id").unwrap().len();
        app.submit_input(ThreadType::Normal);

        // Should add to existing thread
        let messages = app.cache.get_messages("real-backend-id").unwrap();
        assert_eq!(messages.len(), before_count + 2);
        assert!(app.input_box.is_empty());
    }

    #[tokio::test]
    async fn test_submit_input_handles_deleted_thread() {
        let mut app = App::default();

        // Set active thread to non-existent (simulates deleted thread)
        app.active_thread_id = Some("deleted-thread".to_string());
        app.screen = Screen::Conversation;

        app.input_box.insert_char('T');
        app.input_box.insert_char('e');
        app.input_box.insert_char('s');
        app.input_box.insert_char('t');
        app.submit_input(ThreadType::Normal);

        // Should show error about thread not existing
        assert!(app.stream_error.is_some());
        assert!(app.stream_error.as_ref().unwrap().contains("no longer exists"));

        // Input should NOT be cleared
        assert!(!app.input_box.is_empty());
    }

    #[tokio::test]
    async fn test_submit_input_full_conversation_workflow() {
        let mut app = App::default();

        // === Turn 1: New thread ===
        app.input_box.insert_char('H');
        app.input_box.insert_char('i');
        app.submit_input(ThreadType::Normal);

        let pending_id = app.active_thread_id.clone().unwrap();
        assert!(pending_id.starts_with("pending-"));
        assert_eq!(app.screen, Screen::Conversation);

        // Simulate backend response
        app.handle_message(AppMessage::ThreadCreated {
            pending_id: pending_id.clone(),
            real_id: "thread-abc".to_string(),
            title: Some("Greeting".to_string()),
        });
        app.cache.append_to_message("thread-abc", "Hello! How can I help?");
        app.cache.finalize_message("thread-abc", 100);

        assert_eq!(app.active_thread_id, Some("thread-abc".to_string()));

        // === Turn 2: Continue thread ===
        app.input_box.insert_char('T');
        app.input_box.insert_char('e');
        app.input_box.insert_char('l');
        app.input_box.insert_char('l');
        app.input_box.insert_char(' ');
        app.input_box.insert_char('m');
        app.input_box.insert_char('e');
        app.submit_input(ThreadType::Normal);

        // Should still be on same thread
        assert_eq!(app.active_thread_id, Some("thread-abc".to_string()));

        // Should have 4 messages: user1, assistant1, user2, assistant2(streaming)
        let messages = app.cache.get_messages("thread-abc").unwrap();
        assert_eq!(messages.len(), 4);
        assert_eq!(messages[2].content, "Tell me");
        assert!(messages[3].is_streaming);

        // === Navigate away and back ===
        app.navigate_to_command_deck();
        assert!(app.active_thread_id.is_none());
        assert_eq!(app.screen, Screen::CommandDeck);

        // === Turn 3: New thread after navigating away ===
        app.input_box.insert_char('N');
        app.input_box.insert_char('e');
        app.input_box.insert_char('w');
        app.submit_input(ThreadType::Normal);

        // Should be a NEW pending thread
        let new_pending = app.active_thread_id.clone().unwrap();
        assert!(new_pending.starts_with("pending-"));
        assert_ne!(new_pending, "thread-abc");

        // Cache should have both threads
        assert!(app.cache.get_thread("thread-abc").is_some());
        assert!(app.cache.get_thread(&new_pending).is_some());
    }

    #[test]
    fn test_handle_message_stream_token() {
        let mut app = App::default();
        // Create a streaming thread first
        let thread_id = app.cache.create_streaming_thread("Test".to_string());

        // Send a stream token
        app.handle_message(AppMessage::StreamToken {
            thread_id: thread_id.clone(),
            token: "Hello".to_string(),
        });

        // Verify the token was appended
        let messages = app.cache.get_messages(&thread_id).unwrap();
        let assistant_msg = messages.iter().find(|m| m.role == MessageRole::Assistant).unwrap();
        assert!(assistant_msg.content.contains("Hello") || assistant_msg.partial_content.contains("Hello"));
    }

    #[test]
    fn test_handle_message_stream_complete() {
        let mut app = App::default();
        // Create a streaming thread first
        let thread_id = app.cache.create_streaming_thread("Test".to_string());

        // Append some tokens
        app.handle_message(AppMessage::StreamToken {
            thread_id: thread_id.clone(),
            token: "Response".to_string(),
        });

        // Complete the stream
        app.handle_message(AppMessage::StreamComplete {
            thread_id: thread_id.clone(),
            message_id: 42,
        });

        // Verify the message was finalized with correct ID
        let messages = app.cache.get_messages(&thread_id).unwrap();
        let assistant_msg = messages.iter().find(|m| m.role == MessageRole::Assistant).unwrap();
        assert_eq!(assistant_msg.id, 42);
        assert!(!assistant_msg.is_streaming);
    }

    #[test]
    fn test_handle_message_stream_error() {
        let mut app = App::default();

        // Send a stream error
        app.handle_message(AppMessage::StreamError {
            thread_id: "thread-001".to_string(),
            error: "Connection failed".to_string(),
        });

        // Verify the error was stored
        assert!(app.stream_error.is_some());
        assert_eq!(app.stream_error.as_ref().unwrap(), "Connection failed");
    }

    #[test]
    fn test_handle_message_connection_status_connected() {
        let mut app = App::default();
        app.stream_error = Some("Previous error".to_string());
        assert!(!app.connection_status);

        // Send connection status update
        app.handle_message(AppMessage::ConnectionStatus(true));

        // Verify status updated and error cleared
        assert!(app.connection_status);
        assert!(app.stream_error.is_none());
    }

    #[test]
    fn test_handle_message_connection_status_disconnected() {
        let mut app = App::default();
        app.connection_status = true;

        // Send disconnection status
        app.handle_message(AppMessage::ConnectionStatus(false));

        // Verify status updated (error not cleared on disconnect)
        assert!(!app.connection_status);
    }

    #[test]
    fn test_message_sender_returns_clone() {
        let app = App::default();
        let _sender = app.message_sender();
        // Just verify it compiles and returns without panic
        // The sender should be usable for sending messages
    }

    #[test]
    fn test_clear_error() {
        let mut app = App::default();
        app.stream_error = Some("Test error".to_string());

        app.clear_error();

        assert!(app.stream_error.is_none());
    }

    #[test]
    fn test_clear_error_when_no_error() {
        let mut app = App::default();
        assert!(app.stream_error.is_none());

        app.clear_error();

        assert!(app.stream_error.is_none());
    }

    // ============= ThreadCreated Message Tests =============

    #[test]
    fn test_handle_message_thread_created_reconciles_cache() {
        let mut app = App::default();
        // Create a streaming thread first
        let pending_id = app.cache.create_streaming_thread("Test".to_string());

        // Handle ThreadCreated message
        app.handle_message(AppMessage::ThreadCreated {
            pending_id: pending_id.clone(),
            real_id: "real-backend-id".to_string(),
            title: None,
        });

        // Verify the thread was reconciled
        assert!(app.cache.get_thread(&pending_id).is_none());
        assert!(app.cache.get_thread("real-backend-id").is_some());
    }

    #[test]
    fn test_handle_message_thread_created_updates_active_thread() {
        let mut app = App::default();
        // Create a streaming thread and set it as active
        let pending_id = app.cache.create_streaming_thread("Test".to_string());
        app.active_thread_id = Some(pending_id.clone());

        // Handle ThreadCreated message
        app.handle_message(AppMessage::ThreadCreated {
            pending_id,
            real_id: "real-backend-id".to_string(),
            title: None,
        });

        // Verify active_thread_id was updated
        assert_eq!(app.active_thread_id, Some("real-backend-id".to_string()));
    }

    #[test]
    fn test_handle_message_thread_created_does_not_update_different_active_thread() {
        let mut app = App::default();
        // Create a streaming thread
        let pending_id = app.cache.create_streaming_thread("Test".to_string());
        // Set a different thread as active
        app.active_thread_id = Some("different-thread".to_string());

        // Handle ThreadCreated message
        app.handle_message(AppMessage::ThreadCreated {
            pending_id,
            real_id: "real-backend-id".to_string(),
            title: None,
        });

        // Verify active_thread_id was NOT updated (it's different)
        assert_eq!(app.active_thread_id, Some("different-thread".to_string()));
    }

    #[test]
    fn test_handle_message_thread_created_with_title() {
        let mut app = App::default();
        let pending_id = app.cache.create_streaming_thread("Original".to_string());

        app.handle_message(AppMessage::ThreadCreated {
            pending_id,
            real_id: "real-backend-id".to_string(),
            title: Some("New Title from Backend".to_string()),
        });

        let thread = app.cache.get_thread("real-backend-id").unwrap();
        assert_eq!(thread.title, "New Title from Backend");
    }

    #[test]
    fn test_handle_message_thread_created_messages_accessible_by_new_id() {
        let mut app = App::default();
        let pending_id = app.cache.create_streaming_thread("Test".to_string());

        // Append some tokens
        app.cache.append_to_message(&pending_id, "Response content");

        // Handle ThreadCreated message
        app.handle_message(AppMessage::ThreadCreated {
            pending_id: pending_id.clone(),
            real_id: "real-backend-id".to_string(),
            title: None,
        });

        // Messages should be accessible by the new ID
        let messages = app.cache.get_messages("real-backend-id");
        assert!(messages.is_some());
        let messages = messages.unwrap();
        assert_eq!(messages.len(), 2);

        // All messages should have the new thread_id
        for msg in messages {
            assert_eq!(msg.thread_id, "real-backend-id");
        }
    }

    // ============= Initialize Tests =============

    #[tokio::test]
    async fn test_initialize_sets_connection_status() {
        // When server is unreachable, initialize sets connection_status to false
        let client = Arc::new(ConductorClient::with_base_url("http://127.0.0.1:1".to_string()));
        let mut app = App::with_client(client).unwrap();

        // Connection status should start as false
        assert!(!app.connection_status);

        app.initialize().await;

        // After initialization with unreachable server, connection_status should remain false
        assert!(!app.connection_status);
    }

    #[tokio::test]
    async fn test_initialize_starts_with_empty_cache() {
        let client = Arc::new(ConductorClient::with_base_url("http://127.0.0.1:1".to_string()));
        let app = App::with_client(client).unwrap();

        // Cache should start empty (no stub data)
        assert_eq!(app.cache.thread_count(), 0);
    }

    // ============= Scroll Behavior Tests =============

    #[test]
    fn test_stream_token_does_not_reset_scroll_for_non_active_thread() {
        let mut app = App::default();

        // Create two threads
        let thread1_id = app.cache.create_streaming_thread("Thread 1".to_string());
        let thread2_id = app.cache.create_streaming_thread("Thread 2".to_string());

        // Set thread 1 as active
        app.active_thread_id = Some(thread1_id.clone());

        // Set conversation scroll to a non-zero value (user has scrolled up)
        app.conversation_scroll = 5;

        // Receive token for thread 2 (non-active thread)
        app.handle_message(AppMessage::StreamToken {
            thread_id: thread2_id.clone(),
            token: "Hello from thread 2".to_string(),
        });

        // Scroll should NOT be reset (should still be 5)
        assert_eq!(app.conversation_scroll, 5);
    }

    #[test]
    fn test_stream_token_resets_scroll_for_active_thread() {
        let mut app = App::default();

        // Create a thread and set it as active
        let thread_id = app.cache.create_streaming_thread("Active thread".to_string());
        app.active_thread_id = Some(thread_id.clone());

        // Set conversation scroll to a non-zero value
        app.conversation_scroll = 10;

        // Receive token for the active thread
        app.handle_message(AppMessage::StreamToken {
            thread_id: thread_id.clone(),
            token: "Hello".to_string(),
        });

        // Scroll should be reset to 0 (auto-scroll to bottom)
        assert_eq!(app.conversation_scroll, 0);
    }

    #[test]
    fn test_stream_complete_does_not_reset_scroll_for_non_active_thread() {
        let mut app = App::default();

        // Create two threads
        let thread1_id = app.cache.create_streaming_thread("Thread 1".to_string());
        let thread2_id = app.cache.create_streaming_thread("Thread 2".to_string());

        // Set thread 1 as active
        app.active_thread_id = Some(thread1_id.clone());

        // Set conversation scroll to a non-zero value
        app.conversation_scroll = 7;

        // Complete stream for thread 2 (non-active thread)
        app.handle_message(AppMessage::StreamComplete {
            thread_id: thread2_id.clone(),
            message_id: 42,
        });

        // Scroll should NOT be reset
        assert_eq!(app.conversation_scroll, 7);
    }

    #[test]
    fn test_stream_complete_resets_scroll_for_active_thread() {
        let mut app = App::default();

        // Create a thread and set it as active
        let thread_id = app.cache.create_streaming_thread("Active thread".to_string());
        app.active_thread_id = Some(thread_id.clone());

        // Set conversation scroll to a non-zero value
        app.conversation_scroll = 15;

        // Complete stream for the active thread
        app.handle_message(AppMessage::StreamComplete {
            thread_id: thread_id.clone(),
            message_id: 99,
        });

        // Scroll should be reset to 0
        assert_eq!(app.conversation_scroll, 0);
    }

    // ============= ProgrammingMode Tests =============

    #[test]
    fn test_programming_mode_default_is_none() {
        assert_eq!(ProgrammingMode::default(), ProgrammingMode::None);
    }

    #[test]
    fn test_programming_mode_equality() {
        assert_eq!(ProgrammingMode::PlanMode, ProgrammingMode::PlanMode);
        assert_eq!(ProgrammingMode::BypassPermissions, ProgrammingMode::BypassPermissions);
        assert_eq!(ProgrammingMode::None, ProgrammingMode::None);
        assert_ne!(ProgrammingMode::PlanMode, ProgrammingMode::None);
        assert_ne!(ProgrammingMode::BypassPermissions, ProgrammingMode::PlanMode);
    }

    #[test]
    fn test_programming_mode_copy() {
        let mode = ProgrammingMode::PlanMode;
        let copied = mode;
        assert_eq!(mode, copied);
    }

    #[test]
    fn test_app_initializes_with_no_programming_mode() {
        let app = App::default();
        assert_eq!(app.programming_mode, ProgrammingMode::None);
    }

    #[test]
    fn test_cycle_programming_mode_from_none_to_plan() {
        let mut app = App::default();
        assert_eq!(app.programming_mode, ProgrammingMode::None);

        app.cycle_programming_mode();

        assert_eq!(app.programming_mode, ProgrammingMode::PlanMode);
    }

    #[test]
    fn test_cycle_programming_mode_from_plan_to_bypass() {
        let mut app = App::default();
        app.programming_mode = ProgrammingMode::PlanMode;

        app.cycle_programming_mode();

        assert_eq!(app.programming_mode, ProgrammingMode::BypassPermissions);
    }

    #[test]
    fn test_cycle_programming_mode_from_bypass_to_none() {
        let mut app = App::default();
        app.programming_mode = ProgrammingMode::BypassPermissions;

        app.cycle_programming_mode();

        assert_eq!(app.programming_mode, ProgrammingMode::None);
    }

    #[test]
    fn test_cycle_programming_mode_full_cycle() {
        let mut app = App::default();

        // Start at None (default)
        assert_eq!(app.programming_mode, ProgrammingMode::None);

        // Cycle: None → PlanMode
        app.cycle_programming_mode();
        assert_eq!(app.programming_mode, ProgrammingMode::PlanMode);

        // Cycle: PlanMode → BypassPermissions
        app.cycle_programming_mode();
        assert_eq!(app.programming_mode, ProgrammingMode::BypassPermissions);

        // Cycle: BypassPermissions → None
        app.cycle_programming_mode();
        assert_eq!(app.programming_mode, ProgrammingMode::None);

        // Cycle: None → PlanMode (wraps around)
        app.cycle_programming_mode();
        assert_eq!(app.programming_mode, ProgrammingMode::PlanMode);
    }

    #[test]
    fn test_cycle_programming_mode_multiple_cycles() {
        let mut app = App::default();

        // Cycle through 3 complete cycles (9 transitions)
        for _ in 0..3 {
            app.cycle_programming_mode(); // None → PlanMode
            assert_eq!(app.programming_mode, ProgrammingMode::PlanMode);

            app.cycle_programming_mode(); // PlanMode → BypassPermissions
            assert_eq!(app.programming_mode, ProgrammingMode::BypassPermissions);

            app.cycle_programming_mode(); // BypassPermissions → None
            assert_eq!(app.programming_mode, ProgrammingMode::None);
        }
    }

    // ============= is_active_thread_programming Tests =============

    #[test]
    fn test_is_active_thread_programming_returns_false_when_no_active_thread() {
        let app = App::default();
        assert!(app.active_thread_id.is_none());
        assert!(!app.is_active_thread_programming());
    }

    #[test]
    fn test_is_active_thread_programming_returns_false_for_normal_thread() {
        let mut app = App::default();

        // Create a normal thread
        let thread = crate::models::Thread {
            id: "thread-conv".to_string(),
            title: "Normal Thread".to_string(),
            description: None,
            preview: "Just talking".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Normal,
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: chrono::Utc::now(),
        };
        app.cache.upsert_thread(thread);
        app.active_thread_id = Some("thread-conv".to_string());

        assert!(!app.is_active_thread_programming());
    }

    #[test]
    fn test_is_active_thread_programming_returns_true_for_programming_thread() {
        let mut app = App::default();

        // Create a programming thread
        let thread = crate::models::Thread {
            id: "thread-prog".to_string(),
            title: "Programming Thread".to_string(),
            description: None,
            preview: "Code review".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Programming,
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: chrono::Utc::now(),
        };
        app.cache.upsert_thread(thread);
        app.active_thread_id = Some("thread-prog".to_string());

        assert!(app.is_active_thread_programming());
    }

    #[test]
    fn test_is_active_thread_programming_returns_false_for_nonexistent_thread() {
        let mut app = App::default();
        app.active_thread_id = Some("nonexistent-thread".to_string());

        assert!(!app.is_active_thread_programming());
    }

    #[test]
    fn test_is_active_thread_programming_after_thread_type_change() {
        let mut app = App::default();

        // Create a normal thread
        let thread = crate::models::Thread {
            id: "thread-1".to_string(),
            title: "Thread".to_string(),
            description: None,
            preview: "Content".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Normal,
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: chrono::Utc::now(),
        };
        app.cache.upsert_thread(thread);
        app.active_thread_id = Some("thread-1".to_string());

        assert!(!app.is_active_thread_programming());

        // Update to programming thread
        let thread = crate::models::Thread {
            id: "thread-1".to_string(),
            title: "Thread".to_string(),
            description: None,
            preview: "Content".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Programming,
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: chrono::Utc::now(),
        };
        app.cache.upsert_thread(thread);

        assert!(app.is_active_thread_programming());
    }

    // ============= Submit Input with Programming Thread Tests =============

    #[tokio::test]
    async fn test_submit_input_on_programming_thread_uses_programming_mode() {
        let mut app = App::default();

        // Create a programming thread
        let thread = crate::models::Thread {
            id: "prog-thread-123".to_string(),
            title: "Programming Thread".to_string(),
            description: None,
            preview: "Code discussion".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: ThreadType::Programming,
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: chrono::Utc::now(),
        };
        app.cache.upsert_thread(thread);
        app.cache.add_message_simple("prog-thread-123", MessageRole::User, "Previous".to_string());
        app.cache.add_message_simple("prog-thread-123", MessageRole::Assistant, "Response".to_string());
        app.active_thread_id = Some("prog-thread-123".to_string());
        app.screen = Screen::Conversation;

        // Set plan mode
        app.programming_mode = ProgrammingMode::PlanMode;

        // Submit input
        app.input_box.insert_char('H');
        app.input_box.insert_char('i');
        app.submit_input(ThreadType::Normal);

        // Should add streaming message to the thread
        let messages = app.cache.get_messages("prog-thread-123").unwrap();
        assert_eq!(messages.len(), 4); // 2 original + user + assistant streaming
        assert!(messages[3].is_streaming);
    }

    #[tokio::test]
    async fn test_submit_input_programming_mode_none_sets_correct_flags() {
        let mut app = App::default();

        // Create a programming thread
        let thread = crate::models::Thread {
            id: "prog-thread-456".to_string(),
            title: "Programming Thread".to_string(),
            description: None,
            preview: "Code discussion".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: ThreadType::Programming,
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: chrono::Utc::now(),
        };
        app.cache.upsert_thread(thread);
        app.cache.add_message_simple("prog-thread-456", MessageRole::User, "Prev".to_string());
        app.cache.add_message_simple("prog-thread-456", MessageRole::Assistant, "Resp".to_string());
        app.active_thread_id = Some("prog-thread-456".to_string());
        app.screen = Screen::Conversation;

        // Mode is None by default
        assert_eq!(app.programming_mode, ProgrammingMode::None);

        // Submit input
        app.input_box.insert_char('T');
        app.input_box.insert_char('e');
        app.input_box.insert_char('s');
        app.input_box.insert_char('t');
        app.submit_input(ThreadType::Normal);

        // Input should be cleared (submission was accepted)
        assert!(app.input_box.is_empty());
    }

    #[tokio::test]
    async fn test_submit_input_new_thread_is_not_programming() {
        let mut app = App::default();
        assert!(app.active_thread_id.is_none());

        // Submit creates a new non-programming thread
        app.input_box.insert_char('N');
        app.input_box.insert_char('e');
        app.input_box.insert_char('w');
        app.submit_input(ThreadType::Normal);

        // New thread should be at front
        let thread_id = app.active_thread_id.as_ref().unwrap();
        assert!(thread_id.starts_with("pending-"));

        // The new thread should NOT be a programming thread
        assert!(!app.is_active_thread_programming());
    }

    #[test]
    fn test_create_pending_thread_uses_thread_type_parameter() {
        let mut app = App::default();

        // Create a programming pending thread via cache directly
        let pending_id = app.cache.create_pending_thread("Code task".to_string(), ThreadType::Programming);

        // Thread should have programming type
        let thread = app.cache.get_thread(&pending_id).unwrap();
        assert_eq!(thread.thread_type, ThreadType::Programming);
    }

    #[tokio::test]
    async fn test_submit_input_creates_programming_thread_when_specified() {
        let mut app = App::default();
        assert!(app.active_thread_id.is_none());

        // Submit with Programming thread type (like Shift+Enter on CommandDeck)
        app.input_box.insert_char('C');
        app.input_box.insert_char('o');
        app.input_box.insert_char('d');
        app.input_box.insert_char('e');
        app.submit_input(ThreadType::Programming);

        // New thread should be created
        let thread_id = app.active_thread_id.as_ref().unwrap();
        assert!(thread_id.starts_with("pending-"));

        // The new thread SHOULD be a programming thread
        assert!(app.is_active_thread_programming());

        // Verify thread type in cache
        let thread = app.cache.get_thread(thread_id).unwrap();
        assert_eq!(thread.thread_type, ThreadType::Programming);
    }

    // ============= Mode Indicator Visibility Tests =============
    // Mode indicator should only be visible for programming threads

    #[test]
    fn test_mode_indicator_visibility_logic_programming_thread() {
        let mut app = App::default();

        // Create a programming thread
        let thread = crate::models::Thread {
            id: "prog-thread".to_string(),
            title: "Programming".to_string(),
            description: None,
            preview: "Code".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Programming,
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: chrono::Utc::now(),
        };
        app.cache.upsert_thread(thread);
        app.active_thread_id = Some("prog-thread".to_string());
        app.screen = Screen::Conversation;

        // is_active_thread_programming determines mode indicator visibility
        // For programming threads, it should be true (indicator visible)
        assert!(app.is_active_thread_programming());
    }

    #[test]
    fn test_mode_indicator_visibility_logic_normal_thread() {
        let mut app = App::default();

        // Create a normal thread
        let thread = crate::models::Thread {
            id: "conv-thread".to_string(),
            title: "Normal".to_string(),
            description: None,
            preview: "Chat".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Normal,
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: chrono::Utc::now(),
        };
        app.cache.upsert_thread(thread);
        app.active_thread_id = Some("conv-thread".to_string());
        app.screen = Screen::Conversation;

        // For normal threads, indicator should NOT be visible
        assert!(!app.is_active_thread_programming());
    }

    #[test]
    fn test_mode_indicator_visibility_logic_no_thread() {
        let app = App::default();

        // No active thread means no indicator
        assert!(app.active_thread_id.is_none());
        assert!(!app.is_active_thread_programming());
    }

    // ============= Shift+Tab Behavior Tests =============
    // Tests for the conditions that determine Shift+Tab behavior

    #[test]
    fn test_shift_tab_should_cycle_mode_for_programming_thread_in_conversation() {
        let mut app = App::default();

        // Create a programming thread
        let thread = crate::models::Thread {
            id: "prog-thread".to_string(),
            title: "Programming".to_string(),
            description: None,
            preview: "Code".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Programming,
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: chrono::Utc::now(),
        };
        app.cache.upsert_thread(thread);
        app.active_thread_id = Some("prog-thread".to_string());
        app.screen = Screen::Conversation;

        // Condition for Shift+Tab to cycle mode:
        // 1. Screen is Conversation
        // 2. Active thread is Programming type
        assert_eq!(app.screen, Screen::Conversation);
        assert!(app.is_active_thread_programming());

        // Mode cycling should work
        assert_eq!(app.programming_mode, ProgrammingMode::None);
        app.cycle_programming_mode();
        assert_eq!(app.programming_mode, ProgrammingMode::PlanMode);
    }

    #[test]
    fn test_shift_tab_should_not_cycle_mode_for_normal_thread() {
        let mut app = App::default();

        // Create a normal thread
        let thread = crate::models::Thread {
            id: "conv-thread".to_string(),
            title: "Normal".to_string(),
            description: None,
            preview: "Chat".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Normal,
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: chrono::Utc::now(),
        };
        app.cache.upsert_thread(thread);
        app.active_thread_id = Some("conv-thread".to_string());
        app.screen = Screen::Conversation;

        // Shift+Tab should NOT cycle mode for conversation threads
        // (it should cycle focus instead, but that logic is in main.rs)
        assert_eq!(app.screen, Screen::Conversation);
        assert!(!app.is_active_thread_programming());
    }

    #[test]
    fn test_shift_tab_should_not_cycle_mode_on_command_deck() {
        let mut app = App::default();

        // Create a programming thread but stay on CommandDeck
        let thread = crate::models::Thread {
            id: "prog-thread".to_string(),
            title: "Programming".to_string(),
            description: None,
            preview: "Code".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Programming,
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: chrono::Utc::now(),
        };
        app.cache.upsert_thread(thread);
        app.active_thread_id = Some("prog-thread".to_string());
        app.screen = Screen::CommandDeck; // Not in Conversation

        // Even with programming thread, Shift+Tab should not cycle mode
        // because we're not on Conversation screen
        assert_eq!(app.screen, Screen::CommandDeck);
        // The condition for mode cycling is both screen AND thread type
    }

    // ============= Programming Mode Persistence Tests =============

    #[test]
    fn test_programming_mode_persists_across_thread_switches() {
        let mut app = App::default();

        // Set programming mode
        app.programming_mode = ProgrammingMode::PlanMode;

        // Create and switch to a programming thread
        let thread1 = crate::models::Thread {
            id: "prog-1".to_string(),
            title: "Programming 1".to_string(),
            description: None,
            preview: "Code 1".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: ThreadType::Programming,
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: chrono::Utc::now(),
        };
        app.cache.upsert_thread(thread1);
        // Pre-populate messages to avoid lazy fetch triggering tokio::spawn
        app.cache.set_messages("prog-1".to_string(), vec![]);
        app.open_thread("prog-1".to_string());

        // Mode should persist
        assert_eq!(app.programming_mode, ProgrammingMode::PlanMode);

        // Switch to another thread
        let thread2 = crate::models::Thread {
            id: "prog-2".to_string(),
            title: "Programming 2".to_string(),
            description: None,
            preview: "Code 2".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: ThreadType::Programming,
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: chrono::Utc::now(),
        };
        app.cache.upsert_thread(thread2);
        // Pre-populate messages to avoid lazy fetch triggering tokio::spawn
        app.cache.set_messages("prog-2".to_string(), vec![]);
        app.open_thread("prog-2".to_string());

        // Mode should still persist
        assert_eq!(app.programming_mode, ProgrammingMode::PlanMode);
    }

    #[test]
    fn test_programming_mode_persists_after_navigate_to_command_deck() {
        let mut app = App::default();

        // Set programming mode
        app.programming_mode = ProgrammingMode::BypassPermissions;

        // Navigate to command deck
        app.navigate_to_command_deck();

        // Mode should persist (it's app-level, not thread-level)
        assert_eq!(app.programming_mode, ProgrammingMode::BypassPermissions);
    }

    // ============= Thread Type with Add Streaming Message Tests =============

    #[test]
    fn test_add_streaming_message_preserves_thread_type() {
        let mut app = App::default();

        // Create a programming thread
        let pending_id = app.cache.create_pending_thread("Code question".to_string(), ThreadType::Programming);

        // Verify initial type
        let thread = app.cache.get_thread(&pending_id).unwrap();
        assert_eq!(thread.thread_type, ThreadType::Programming);

        // Finalize first response
        app.cache.append_to_message(&pending_id, "Answer");
        app.cache.finalize_message(&pending_id, 1);

        // Reconcile with backend
        app.cache.reconcile_thread_id(&pending_id, "real-thread-123", None);

        // Add follow-up message
        app.cache.add_streaming_message("real-thread-123", "Follow-up".to_string());

        // Thread type should be preserved
        let thread = app.cache.get_thread("real-thread-123").unwrap();
        assert_eq!(thread.thread_type, ThreadType::Programming);
    }

    // ============= State Module Integration Tests =============

    #[test]
    fn test_app_initializes_session_state() {
        let app = App::default();

        // SessionState should be initialized
        assert!(app.session_state.skills.is_empty());
        assert!(app.session_state.context_tokens_used.is_none());
        assert!(app.session_state.pending_permission.is_none());
        assert!(app.session_state.oauth_required.is_none());
    }

    #[test]
    fn test_app_initializes_tool_tracker() {
        let app = App::default();

        // ToolTracker should be initialized and empty
        assert_eq!(app.tool_tracker.total_count(), 0);
        assert!(!app.tool_tracker.has_active_tools());
    }

    #[test]
    fn test_session_state_persists_across_operations() {
        let mut app = App::default();

        // Modify session state
        app.session_state.add_skill("git".to_string());
        app.session_state.set_context_tokens(5000);

        // Navigate around
        app.navigate_to_command_deck();

        // Session state should persist
        assert!(app.session_state.has_skill("git"));
        assert_eq!(app.session_state.context_tokens_used, Some(5000));
    }

    #[test]
    fn test_tool_tracker_can_track_tools() {
        use crate::state::tools::{ToolCallState, ToolCallStatus};

        let mut app = App::default();

        // Register a tool call
        let state = ToolCallState::new("Bash".to_string());
        app.tool_tracker.register_tool("tool-1".to_string(), state);

        // Verify tracking
        assert_eq!(app.tool_tracker.total_count(), 1);
        assert!(app.tool_tracker.contains("tool-1"));

        // Start the tool
        app.tool_tracker.start_tool("tool-1");
        let tool_state = app.tool_tracker.get_tool("tool-1").unwrap();
        assert_eq!(tool_state.status, ToolCallStatus::Running);

        // Complete the tool
        app.tool_tracker.complete_tool("tool-1", Some("output".to_string()));
        let tool_state = app.tool_tracker.get_tool("tool-1").unwrap();
        assert_eq!(tool_state.status, ToolCallStatus::Completed);
        assert!(!app.tool_tracker.has_active_tools());
    }

    #[test]
    fn test_tool_tracker_independent_per_app() {
        let mut app1 = App::default();
        let app2 = App::default();

        // Add tool to app1
        app1.tool_tracker.register_tool(
            "tool-1".to_string(),
            crate::state::tools::ToolCallState::new("Bash".to_string())
        );

        // app2 should not see it
        assert_eq!(app1.tool_tracker.total_count(), 1);
        assert_eq!(app2.tool_tracker.total_count(), 0);
    }

    // ============= is_streaming() Tests =============

    #[test]
    fn test_is_streaming_returns_false_when_no_active_thread() {
        let app = App::default();
        assert!(app.active_thread_id.is_none());
        assert!(!app.is_streaming());
    }

    #[test]
    fn test_is_streaming_returns_true_when_thread_is_streaming() {
        let mut app = App::default();

        // Create a streaming thread
        let thread_id = app.cache.create_streaming_thread("Test message".to_string());
        app.active_thread_id = Some(thread_id.clone());

        // Should detect streaming
        assert!(app.is_streaming());
    }

    #[test]
    fn test_is_streaming_returns_false_when_thread_not_streaming() {
        let mut app = App::default();

        // Create a streaming thread and finalize it
        let thread_id = app.cache.create_streaming_thread("Test message".to_string());
        app.cache.append_to_message(&thread_id, "Response");
        app.cache.finalize_message(&thread_id, 1);
        app.active_thread_id = Some(thread_id.clone());

        // Should NOT detect streaming (message is finalized)
        assert!(!app.is_streaming());
    }

    #[test]
    fn test_is_streaming_returns_false_for_nonexistent_thread() {
        let mut app = App::default();
        app.active_thread_id = Some("nonexistent-thread".to_string());

        assert!(!app.is_streaming());
    }

    #[test]
    fn test_is_streaming_updates_when_stream_completes() {
        let mut app = App::default();

        // Create a streaming thread
        let thread_id = app.cache.create_streaming_thread("Question".to_string());
        app.active_thread_id = Some(thread_id.clone());

        // Initially streaming
        assert!(app.is_streaming());

        // Finalize the stream
        app.cache.append_to_message(&thread_id, "Answer");
        app.cache.finalize_message(&thread_id, 42);

        // Should no longer be streaming
        assert!(!app.is_streaming());
    }

    // ============= TodosUpdated Tests =============

    #[test]
    fn test_app_initializes_with_empty_todos() {
        let app = App::default();
        assert!(app.todos.is_empty());
    }

    #[test]
    fn test_handle_message_todos_updated() {
        use crate::state::{Todo, TodoStatus};

        let mut app = App::default();

        let todos = vec![
            Todo {
                content: "Fix the bug".to_string(),
                active_form: "Fixing the bug".to_string(),
                status: TodoStatus::Pending,
            },
            Todo {
                content: "Run tests".to_string(),
                active_form: "Running tests".to_string(),
                status: TodoStatus::InProgress,
            },
        ];

        app.handle_message(AppMessage::TodosUpdated {
            todos: todos.clone(),
        });

        assert_eq!(app.todos.len(), 2);
        assert_eq!(app.todos[0].content, "Fix the bug");
        assert_eq!(app.todos[0].status, TodoStatus::Pending);
        assert_eq!(app.todos[1].content, "Run tests");
        assert_eq!(app.todos[1].status, TodoStatus::InProgress);
    }

    #[test]
    fn test_todos_updated_replaces_previous_todos() {
        use crate::state::{Todo, TodoStatus};

        let mut app = App::default();

        // Set initial todos
        let initial_todos = vec![
            Todo {
                content: "Old task 1".to_string(),
                active_form: "Old task 1".to_string(),
                status: TodoStatus::Pending,
            },
            Todo {
                content: "Old task 2".to_string(),
                active_form: "Old task 2".to_string(),
                status: TodoStatus::Pending,
            },
        ];
        app.handle_message(AppMessage::TodosUpdated {
            todos: initial_todos,
        });
        assert_eq!(app.todos.len(), 2);

        // Update with new todos
        let new_todos = vec![Todo {
            content: "New task".to_string(),
            active_form: "New task".to_string(),
            status: TodoStatus::InProgress,
        }];
        app.handle_message(AppMessage::TodosUpdated { todos: new_todos });

        // Should replace the old todos
        assert_eq!(app.todos.len(), 1);
        assert_eq!(app.todos[0].content, "New task");
        assert_eq!(app.todos[0].status, TodoStatus::InProgress);
    }

    #[test]
    fn test_todos_updated_with_empty_list() {
        use crate::state::{Todo, TodoStatus};

        let mut app = App::default();

        // Set initial todos
        let initial_todos = vec![Todo {
            content: "Task".to_string(),
            active_form: "Task".to_string(),
            status: TodoStatus::Pending,
        }];
        app.handle_message(AppMessage::TodosUpdated {
            todos: initial_todos,
        });
        assert_eq!(app.todos.len(), 1);

        // Clear todos
        app.handle_message(AppMessage::TodosUpdated { todos: Vec::new() });

        assert!(app.todos.is_empty());
    }

    #[test]
    fn test_todos_updated_preserves_active_form() {
        use crate::state::{Todo, TodoStatus};

        let mut app = App::default();

        let todos = vec![Todo {
            content: "Build the project".to_string(),
            active_form: "Building the project".to_string(),
            status: TodoStatus::InProgress,
        }];

        app.handle_message(AppMessage::TodosUpdated {
            todos: todos.clone(),
        });

        assert_eq!(app.todos[0].content, "Build the project");
        assert_eq!(app.todos[0].active_form, "Building the project");
    }

    // ============= Inline Error Management Tests =============

    #[test]
    fn test_has_errors_returns_false_when_no_active_thread() {
        let app = App::default();
        assert!(app.active_thread_id.is_none());
        assert!(!app.has_errors());
    }

    #[test]
    fn test_has_errors_returns_false_when_no_errors() {
        let mut app = App::default();
        let thread_id = app.cache.create_streaming_thread("Hello".to_string());
        app.active_thread_id = Some(thread_id);

        assert!(!app.has_errors());
    }

    #[test]
    fn test_has_errors_returns_true_when_errors_exist() {
        let mut app = App::default();
        let thread_id = app.cache.create_streaming_thread("Hello".to_string());
        app.active_thread_id = Some(thread_id.clone());

        app.cache.add_error_simple(&thread_id, "error".to_string(), "message".to_string());

        assert!(app.has_errors());
    }

    #[test]
    fn test_add_error_to_active_thread() {
        let mut app = App::default();
        let thread_id = app.cache.create_streaming_thread("Hello".to_string());
        app.active_thread_id = Some(thread_id.clone());

        app.add_error_to_active_thread("test_error".to_string(), "Test message".to_string());

        assert!(app.has_errors());
        let errors = app.cache.get_errors(&thread_id).unwrap();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].error_code, "test_error");
        assert_eq!(errors[0].message, "Test message");
    }

    #[test]
    fn test_add_error_to_active_thread_when_no_active_thread() {
        let mut app = App::default();
        assert!(app.active_thread_id.is_none());

        // Should not panic
        app.add_error_to_active_thread("error".to_string(), "message".to_string());

        // No errors should be added anywhere
        assert!(!app.has_errors());
    }

    #[test]
    fn test_dismiss_focused_error_when_no_active_thread() {
        let mut app = App::default();
        assert!(app.active_thread_id.is_none());

        let dismissed = app.dismiss_focused_error();
        assert!(!dismissed);
    }

    #[test]
    fn test_dismiss_focused_error_when_no_errors() {
        let mut app = App::default();
        let thread_id = app.cache.create_streaming_thread("Hello".to_string());
        app.active_thread_id = Some(thread_id);

        let dismissed = app.dismiss_focused_error();
        assert!(!dismissed);
    }

    #[test]
    fn test_dismiss_focused_error_removes_error() {
        let mut app = App::default();
        let thread_id = app.cache.create_streaming_thread("Hello".to_string());
        app.active_thread_id = Some(thread_id.clone());

        app.cache.add_error_simple(&thread_id, "error1".to_string(), "First".to_string());
        app.cache.add_error_simple(&thread_id, "error2".to_string(), "Second".to_string());
        assert!(app.has_errors());
        assert_eq!(app.cache.error_count(&thread_id), 2);

        let dismissed = app.dismiss_focused_error();
        assert!(dismissed);
        assert_eq!(app.cache.error_count(&thread_id), 1);

        // Should still have one error
        assert!(app.has_errors());

        // Dismiss the remaining error
        let dismissed = app.dismiss_focused_error();
        assert!(dismissed);
        assert_eq!(app.cache.error_count(&thread_id), 0);

        // No more errors
        assert!(!app.has_errors());
    }

    #[test]
    fn test_error_persists_across_navigate_to_command_deck() {
        let mut app = App::default();
        let thread_id = app.cache.create_streaming_thread("Hello".to_string());
        app.active_thread_id = Some(thread_id.clone());
        app.screen = Screen::Conversation;

        app.add_error_to_active_thread("error".to_string(), "message".to_string());
        assert!(app.has_errors());

        // Navigate away
        app.navigate_to_command_deck();
        assert!(app.active_thread_id.is_none());

        // Error should still exist in cache
        assert_eq!(app.cache.error_count(&thread_id), 1);
    }

    #[test]
    fn test_multiple_errors_on_active_thread() {
        let mut app = App::default();
        let thread_id = app.cache.create_streaming_thread("Hello".to_string());
        app.active_thread_id = Some(thread_id.clone());

        app.add_error_to_active_thread("error1".to_string(), "First error".to_string());
        app.add_error_to_active_thread("error2".to_string(), "Second error".to_string());
        app.add_error_to_active_thread("error3".to_string(), "Third error".to_string());

        assert_eq!(app.cache.error_count(&thread_id), 3);
        assert!(app.has_errors());
    }

    // ============= Permission Handling Tests =============

    #[test]
    fn test_handle_permission_key_returns_false_when_no_pending() {
        let mut app = App::default();
        assert!(!app.session_state.has_pending_permission());

        // Should return false when no permission is pending
        assert!(!app.handle_permission_key('y'));
        assert!(!app.handle_permission_key('a'));
        assert!(!app.handle_permission_key('n'));
    }

    #[test]
    fn test_handle_permission_key_y_approves_and_clears() {
        let mut app = App::default();

        // Set up a pending permission
        use crate::state::PermissionRequest;
        app.session_state.set_pending_permission(PermissionRequest {
            permission_id: "perm-123".to_string(),
            tool_name: "Bash".to_string(),
            description: "Run npm install".to_string(),
            context: None,
            tool_input: None,
        });
        assert!(app.session_state.has_pending_permission());

        // Press 'y' to approve
        let handled = app.handle_permission_key('y');
        assert!(handled);

        // Permission should be cleared
        assert!(!app.session_state.has_pending_permission());
        // Tool should NOT be added to allowed list (that's only for 'a')
        assert!(!app.session_state.is_tool_allowed("Bash"));
    }

    #[test]
    fn test_handle_permission_key_a_allows_always_and_clears() {
        let mut app = App::default();

        // Set up a pending permission
        use crate::state::PermissionRequest;
        app.session_state.set_pending_permission(PermissionRequest {
            permission_id: "perm-456".to_string(),
            tool_name: "Write".to_string(),
            description: "Write file".to_string(),
            context: Some("/home/user/test.rs".to_string()),
            tool_input: None,
        });
        assert!(app.session_state.has_pending_permission());

        // Press 'a' to allow always
        let handled = app.handle_permission_key('a');
        assert!(handled);

        // Permission should be cleared
        assert!(!app.session_state.has_pending_permission());
        // Tool SHOULD be added to allowed list
        assert!(app.session_state.is_tool_allowed("Write"));
    }

    #[test]
    fn test_handle_permission_key_n_denies_and_clears() {
        let mut app = App::default();

        // Set up a pending permission
        use crate::state::PermissionRequest;
        app.session_state.set_pending_permission(PermissionRequest {
            permission_id: "perm-789".to_string(),
            tool_name: "Edit".to_string(),
            description: "Edit file".to_string(),
            context: None,
            tool_input: None,
        });
        assert!(app.session_state.has_pending_permission());

        // Press 'n' to deny
        let handled = app.handle_permission_key('n');
        assert!(handled);

        // Permission should be cleared
        assert!(!app.session_state.has_pending_permission());
        // Tool should NOT be added to allowed list
        assert!(!app.session_state.is_tool_allowed("Edit"));
    }

    #[test]
    fn test_handle_permission_key_uppercase_works() {
        let mut app = App::default();

        use crate::state::PermissionRequest;
        app.session_state.set_pending_permission(PermissionRequest {
            permission_id: "perm-abc".to_string(),
            tool_name: "Bash".to_string(),
            description: "Run command".to_string(),
            context: None,
            tool_input: None,
        });

        // Uppercase 'Y' should also work
        let handled = app.handle_permission_key('Y');
        assert!(handled);
        assert!(!app.session_state.has_pending_permission());
    }

    #[test]
    fn test_handle_permission_key_invalid_returns_false() {
        let mut app = App::default();

        use crate::state::PermissionRequest;
        app.session_state.set_pending_permission(PermissionRequest {
            permission_id: "perm-def".to_string(),
            tool_name: "Bash".to_string(),
            description: "Run command".to_string(),
            context: None,
            tool_input: None,
        });

        // Invalid keys should return false and NOT clear permission
        assert!(!app.handle_permission_key('x'));
        assert!(!app.handle_permission_key('q'));
        assert!(!app.handle_permission_key(' '));

        // Permission should still be pending
        assert!(app.session_state.has_pending_permission());
    }

    #[test]
    fn test_permission_auto_approve_when_tool_allowed() {
        let mut app = App::default();

        // Pre-allow the Bash tool
        app.session_state.allow_tool("Bash".to_string());
        assert!(app.session_state.is_tool_allowed("Bash"));

        // Receive a permission request for Bash
        app.handle_message(AppMessage::PermissionRequested {
            permission_id: "perm-auto".to_string(),
            tool_name: "Bash".to_string(),
            description: "Run npm install".to_string(),
            tool_input: None,
        });

        // Permission should NOT be set as pending (auto-approved)
        assert!(!app.session_state.has_pending_permission());
    }

    #[test]
    fn test_permission_request_stored_when_tool_not_allowed() {
        let mut app = App::default();

        // Bash is NOT pre-allowed
        assert!(!app.session_state.is_tool_allowed("Bash"));

        // Receive a permission request for Bash
        app.handle_message(AppMessage::PermissionRequested {
            permission_id: "perm-store".to_string(),
            tool_name: "Bash".to_string(),
            description: "Run npm install".to_string(),
            tool_input: Some(serde_json::json!({"command": "npm install"})),
        });

        // Permission SHOULD be set as pending
        assert!(app.session_state.has_pending_permission());

        // Verify the stored permission data
        let perm = app.session_state.pending_permission.as_ref().unwrap();
        assert_eq!(perm.permission_id, "perm-store");
        assert_eq!(perm.tool_name, "Bash");
        assert_eq!(perm.description, "Run npm install");
        assert!(perm.tool_input.is_some());
    }

    #[test]
    fn test_allow_always_persists_for_subsequent_requests() {
        let mut app = App::default();

        // First request: user presses 'a' to allow always
        use crate::state::PermissionRequest;
        app.session_state.set_pending_permission(PermissionRequest {
            permission_id: "perm-first".to_string(),
            tool_name: "Read".to_string(),
            description: "Read file".to_string(),
            context: None,
            tool_input: None,
        });
        app.handle_permission_key('a');

        // Verify Read is now allowed
        assert!(app.session_state.is_tool_allowed("Read"));

        // Second request for Read tool (simulated)
        app.handle_message(AppMessage::PermissionRequested {
            permission_id: "perm-second".to_string(),
            tool_name: "Read".to_string(),
            description: "Read another file".to_string(),
            tool_input: None,
        });

        // Should be auto-approved (no pending permission)
        assert!(!app.session_state.has_pending_permission());
    }

    #[test]
    fn test_skills_injected_message() {
        let mut app = App::default();
        assert!(app.session_state.skills.is_empty());

        app.handle_message(AppMessage::SkillsInjected {
            skills: vec!["commit".to_string(), "review".to_string()],
        });

        assert_eq!(app.session_state.skills.len(), 2);
        assert!(app.session_state.has_skill("commit"));
        assert!(app.session_state.has_skill("review"));
    }

    #[test]
    fn test_skills_injected_deduplication() {
        let mut app = App::default();

        app.handle_message(AppMessage::SkillsInjected {
            skills: vec!["commit".to_string()],
        });
        app.handle_message(AppMessage::SkillsInjected {
            skills: vec!["commit".to_string(), "review".to_string()],
        });

        assert_eq!(app.session_state.skills.len(), 2);
    }

    #[test]
    fn test_oauth_consent_required_message() {
        let mut app = App::default();
        assert!(!app.session_state.needs_oauth());
        assert!(app.session_state.oauth_url.is_none());

        app.handle_message(AppMessage::OAuthConsentRequired {
            provider: "github".to_string(),
            url: Some("https://github.com/oauth".to_string()),
            skill_name: Some("git-commit".to_string()),
        });

        assert!(app.session_state.needs_oauth());
        assert_eq!(
            app.session_state.oauth_required,
            Some(("github".to_string(), "git-commit".to_string()))
        );
        assert_eq!(
            app.session_state.oauth_url,
            Some("https://github.com/oauth".to_string())
        );
    }

    #[test]
    fn test_oauth_consent_without_url() {
        let mut app = App::default();

        app.handle_message(AppMessage::OAuthConsentRequired {
            provider: "google".to_string(),
            url: None,
            skill_name: Some("calendar".to_string()),
        });

        assert!(app.session_state.needs_oauth());
        assert!(app.session_state.oauth_url.is_none());
    }

    #[test]
    fn test_context_compacted_message() {
        let mut app = App::default();
        assert!(app.session_state.context_tokens_used.is_none());
        assert!(app.session_state.context_token_limit.is_none());

        app.handle_message(AppMessage::ContextCompacted {
            tokens_used: Some(45_000),
            token_limit: Some(100_000),
        });

        assert_eq!(app.session_state.context_tokens_used, Some(45_000));
        assert_eq!(app.session_state.context_token_limit, Some(100_000));
    }

    #[test]
    fn test_context_compacted_updates_existing() {
        let mut app = App::default();
        app.session_state.set_context_tokens(30_000);
        app.session_state.set_context_token_limit(100_000);

        app.handle_message(AppMessage::ContextCompacted {
            tokens_used: Some(50_000),
            token_limit: None, // Don't update limit
        });

        assert_eq!(app.session_state.context_tokens_used, Some(50_000));
        assert_eq!(app.session_state.context_token_limit, Some(100_000));
    }

    #[test]
    fn test_thread_metadata_updated_updates_thread() {
        let mut app = App::default();

        // Create a thread
        let thread_id = app.cache.create_streaming_thread("Original Title".to_string());

        // Update metadata via message
        app.handle_message(AppMessage::ThreadMetadataUpdated {
            thread_id: thread_id.clone(),
            title: Some("Updated Title".to_string()),
            description: Some("New Description".to_string()),
        });

        // Verify the thread was updated
        let thread = app.cache.get_thread(&thread_id).unwrap();
        assert_eq!(thread.title, "Updated Title");
        assert_eq!(thread.description, Some("New Description".to_string()));
    }

    #[test]
    fn test_thread_metadata_updated_partial_update() {
        let mut app = App::default();

        // Create a thread
        let thread_id = app.cache.create_streaming_thread("Original Title".to_string());

        // Update only description
        app.handle_message(AppMessage::ThreadMetadataUpdated {
            thread_id: thread_id.clone(),
            title: None,
            description: Some("Just a description".to_string()),
        });

        // Verify title unchanged, description updated
        let thread = app.cache.get_thread(&thread_id).unwrap();
        assert_eq!(thread.title, "Original Title");
        assert_eq!(thread.description, Some("Just a description".to_string()));
    }

    #[test]
    fn test_thread_metadata_updated_nonexistent_thread() {
        let mut app = App::default();

        // Try to update a thread that doesn't exist
        app.handle_message(AppMessage::ThreadMetadataUpdated {
            thread_id: "nonexistent-thread".to_string(),
            title: Some("Title".to_string()),
            description: Some("Description".to_string()),
        });

        // Should not panic, just do nothing
        assert!(app.cache.get_thread("nonexistent-thread").is_none());
    }
}
