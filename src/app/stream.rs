//! Streaming input submission and SSE event processing for the App.

use std::sync::Arc;

use futures_util::StreamExt;
use tokio::sync::mpsc;

use crate::debug::{
    DebugEventKind, ErrorData, ErrorSource, ProcessedEventData, StreamLifecycleData, StreamPhase,
};
use crate::events::SseEvent;
use crate::models::{PermissionMode, StreamRequest, ThreadType};
use crate::state::Todo;

use super::{emit_debug, log_thread_update, truncate_for_debug, App, AppMessage, Screen};
use crate::debug::DebugEventSender;

impl App {
    /// Submit the current input, create a streaming thread, and spawn async API call.
    ///
    /// This handles two distinct cases:
    /// 1. NEW thread: When `active_thread_id` is None, creates a new thread with a client-generated UUID
    /// 2. CONTINUING thread: When `active_thread_id` exists, adds to the existing thread
    ///
    /// The client generates the thread_id (UUID) for new threads and sends it to the backend.
    /// The backend uses this client-provided UUID as the canonical thread_id.
    ///
    /// The unified stream endpoint routes based on thread_type parameter.
    /// The current permission_mode is sent with the request.
    ///
    /// Edge case: If the thread has a streaming response in progress, we block submission
    /// to prevent sending multiple messages before the current response completes.
    ///
    /// The `new_thread_type` parameter specifies what type of thread to create if this
    /// is a NEW conversation. It's ignored when continuing an existing thread.
    pub fn submit_input(&mut self, new_thread_type: ThreadType) {
        let content = self.textarea.content_expanded();
        if content.trim().is_empty() {
            return;
        }

        // CRITICAL: Check screen first to determine new vs continue.
        // CommandDeck = ALWAYS new thread (regardless of any stale active_thread_id)
        // Conversation = continue the thread that was opened via open_thread()
        let is_command_deck = self.screen == Screen::CommandDeck;

        // Extract working directory from selected folder (if any)
        let working_directory = self.selected_folder.as_ref().map(|f| f.path.clone());

        // Determine thread_id based on screen
        let (thread_id, is_new_thread) = if is_command_deck {
            // NEW thread - create pending, will reconcile when backend responds
            let pending_id = self.cache.create_pending_thread(
                content.clone(),
                new_thread_type,
                working_directory.clone(),
            );
            self.active_thread_id = Some(pending_id.clone());
            self.screen = Screen::Conversation;
            // Reset scroll for new conversation
            self.reset_scroll();
            // Clear selected folder after successful thread creation
            self.selected_folder = None;
            (pending_id, true)
        } else if let Some(existing_id) = &self.active_thread_id {
            // CONTINUING existing thread (we're on Conversation screen)
            // Check if there's already a streaming response in progress
            if self.cache.is_thread_streaming(existing_id) {
                // Block rapid second message - still waiting for response to complete
                self.stream_error = Some(
                    "Please wait for the current response to complete before sending another message."
                        .to_string(),
                );
                return;
            }

            if !self
                .cache
                .add_streaming_message(existing_id, content.clone())
            {
                // Thread doesn't exist in cache - might have been deleted
                self.stream_error = Some("Thread no longer exists.".to_string());
                return;
            }
            (existing_id.clone(), false)
        } else {
            // Edge case: on Conversation screen but no active_thread_id (shouldn't happen)
            // Fall back to creating new thread
            let pending_id = self.cache.create_pending_thread(
                content.clone(),
                new_thread_type,
                working_directory.clone(),
            );
            self.active_thread_id = Some(pending_id.clone());
            self.reset_scroll();
            // Clear selected folder after successful thread creation
            self.selected_folder = None;
            (pending_id, true)
        };

        // Add to input history before clearing
        self.input_history.add(content.clone());

        self.textarea.clear();
        self.textarea.clear_paste_tokens();

        // Reset history navigation after submit
        self.input_history.reset_navigation();

        self.mark_dirty();

        // Emit StreamLifecycle connecting event
        emit_debug(
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
        // Always send thread_id - for new threads, we generate a UUID upfront
        // The backend will use our client-generated UUID as the canonical thread_id
        let is_plan_mode = self.permission_mode == PermissionMode::Plan;
        let request = StreamRequest::with_thread(content, thread_id)
            .with_type(new_thread_type)
            .with_permission_mode(self.permission_mode)
            .with_working_directory(working_directory)
            .with_plan_mode(is_plan_mode);

        // Emit debug event with full StreamRequest JSON
        if let Ok(json_string) = serde_json::to_string_pretty(&request) {
            emit_debug(
                &self.debug_tx,
                DebugEventKind::ProcessedEvent(ProcessedEventData::new(
                    "StreamRequest",
                    json_string,
                )),
                Some(&thread_id_for_task),
            );
        }

        // Spawn async task for unified stream endpoint
        tokio::spawn(async move {
            match client.stream(&request).await {
                Ok(mut stream) => {
                    // Emit StreamLifecycle connected event
                    emit_debug(
                        &debug_tx,
                        DebugEventKind::StreamLifecycle(StreamLifecycleData::new(
                            StreamPhase::Connected,
                        )),
                        Some(&thread_id_for_task),
                    );
                    // Update connection status to connected since streaming works
                    let _ = message_tx.send(AppMessage::ConnectionStatus(true));
                    Self::process_stream(&mut stream, &message_tx, &thread_id_for_task, debug_tx)
                        .await;
                }
                Err(e) => {
                    // Emit error debug event
                    emit_debug(
                        &debug_tx,
                        DebugEventKind::Error(ErrorData::new(
                            ErrorSource::ConductorApi,
                            e.to_string(),
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

    /// Process a stream of SSE events and send messages to the app.
    ///
    /// This is a helper method extracted from submit_input to avoid code duplication
    /// between the programming and standard streaming paths.
    pub(super) async fn process_stream(
        stream: &mut std::pin::Pin<
            Box<
                dyn futures_util::Stream<Item = Result<SseEvent, crate::conductor::ConductorError>>
                    + Send,
            >,
        >,
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

                            // NOTE: StreamToken debug logging disabled to reduce noise
                            // Uncomment below to debug streaming content:
                            // emit_debug(
                            //     &debug_tx,
                            //     DebugEventKind::ProcessedEvent(ProcessedEventData::new(
                            //         "StreamToken",
                            //         format!("token: '{}'", truncate_for_debug(&content_event.text, 50)),
                            //     )),
                            //     Some(thread_id),
                            // );

                            let _ = message_tx.send(AppMessage::StreamToken {
                                thread_id: thread_id.to_string(),
                                token: content_event.text,
                            });
                        }
                        SseEvent::Done(done_event) => {
                            // Parse message_id from string to i64
                            let message_id = done_event.message_id.parse::<i64>().unwrap_or(0);

                            // Emit ProcessedEvent
                            emit_debug(
                                &debug_tx,
                                DebugEventKind::ProcessedEvent(ProcessedEventData::new(
                                    "StreamComplete",
                                    format!("message_id: {}", message_id),
                                )),
                                Some(thread_id),
                            );

                            // Emit StreamLifecycle completed
                            emit_debug(
                                &debug_tx,
                                DebugEventKind::StreamLifecycle(StreamLifecycleData::new(
                                    StreamPhase::Completed,
                                )),
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
                            emit_debug(
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
                            emit_debug(
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
                            let todos: Vec<Todo> =
                                todos_event.todos.iter().map(Todo::from_sse).collect();
                            emit_debug(
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
                            emit_debug(
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
                                thread_id: Some(thread_id.to_string()),
                                tool_name: perm_event.tool_name,
                                description: perm_event.description,
                                tool_input: perm_event.tool_input,
                            });
                        }
                        SseEvent::ToolCallStart(tool_event) => {
                            emit_debug(
                                &debug_tx,
                                DebugEventKind::ProcessedEvent(ProcessedEventData::new(
                                    "ToolStarted",
                                    format!("tool: {}", tool_event.tool_name),
                                )),
                                Some(thread_id),
                            );
                            let _ = message_tx.send(AppMessage::ToolStarted {
                                thread_id: thread_id.to_string(),
                                tool_call_id: tool_event.tool_call_id,
                                tool_name: tool_event.tool_name,
                            });
                        }
                        SseEvent::ToolCallArgument(arg_event) => {
                            // Send argument chunk to be accumulated in the ToolEvent
                            let _ = message_tx.send(AppMessage::ToolArgumentChunk {
                                thread_id: thread_id.to_string(),
                                tool_call_id: arg_event.tool_call_id,
                                chunk: arg_event.chunk,
                            });
                        }
                        SseEvent::ToolExecuting(tool_event) => {
                            let display_name = tool_event
                                .display_name
                                .clone()
                                .or(tool_event.url.clone())
                                .unwrap_or_else(|| "Executing...".to_string());
                            let tool_call_id = tool_event.tool_call_id.clone();
                            emit_debug(
                                &debug_tx,
                                DebugEventKind::ProcessedEvent(ProcessedEventData::new(
                                    "ToolExecuting",
                                    format!("display: {}", truncate_for_debug(&display_name, 40)),
                                )),
                                Some(thread_id),
                            );
                            let _ = message_tx.send(AppMessage::ToolExecuting {
                                thread_id: thread_id.to_string(),
                                tool_call_id,
                                display_name,
                            });
                        }
                        SseEvent::ToolResult(tool_event) => {
                            // Check if result looks like an error
                            let result = &tool_event.result;

                            // Check for JSON error field or string starting with "Error:"
                            let (success, summary) =
                                if result.starts_with("Error:") || result.starts_with("error:") {
                                    (false, result.clone())
                                } else if let Ok(json) =
                                    serde_json::from_str::<serde_json::Value>(result)
                                {
                                    // Check for {"error": "..."} or {"data": null, "error": "..."}
                                    if let Some(err) = json.get("error").and_then(|e| e.as_str()) {
                                        if !err.is_empty() {
                                            (false, err.to_string())
                                        } else if let Some(data) = json.get("data") {
                                            if data.is_null() {
                                                (false, "No data returned".to_string())
                                            } else {
                                                (true, "Complete".to_string())
                                            }
                                        } else {
                                            (true, "Complete".to_string())
                                        }
                                    } else {
                                        (true, "Complete".to_string())
                                    }
                                } else {
                                    // Summarize successful result (respecting UTF-8 boundaries)
                                    let summary = if result.len() > 50 {
                                        let mut end = 47;
                                        while end > 0 && !result.is_char_boundary(end) {
                                            end -= 1;
                                        }
                                        format!("{}...", &result[..end])
                                    } else if result.is_empty() {
                                        "Complete".to_string()
                                    } else {
                                        result.clone()
                                    };
                                    (true, summary)
                                };
                            emit_debug(
                                &debug_tx,
                                DebugEventKind::ProcessedEvent(ProcessedEventData::new(
                                    "ToolCompleted",
                                    format!(
                                        "success: {}, summary: {}",
                                        success,
                                        truncate_for_debug(&summary, 30)
                                    ),
                                )),
                                Some(thread_id),
                            );
                            let _ = message_tx.send(AppMessage::ToolCompleted {
                                thread_id: thread_id.to_string(),
                                tool_call_id: tool_event.tool_call_id,
                                success,
                                summary,
                                result: result.clone(),
                            });
                        }
                        SseEvent::SkillsInjected(skills_event) => {
                            emit_debug(
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
                            emit_debug(
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
                            emit_debug(
                                &debug_tx,
                                DebugEventKind::ProcessedEvent(ProcessedEventData::new(
                                    "ContextCompacted",
                                    format!(
                                        "tokens: {:?}/{:?}",
                                        context_event.tokens_used, context_event.token_limit
                                    ),
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
                                emit_debug(
                                    &debug_tx,
                                    DebugEventKind::ProcessedEvent(ProcessedEventData::new(
                                        "ReasoningToken",
                                        format!(
                                            "token: '{}'",
                                            truncate_for_debug(&reasoning_event.text, 50)
                                        ),
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
                            emit_debug(
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
                        SseEvent::SubagentStarted(subagent_event) => {
                            emit_debug(
                                &debug_tx,
                                DebugEventKind::ProcessedEvent(ProcessedEventData::new(
                                    "SubagentStarted",
                                    format!(
                                        "type: {}, task: {}",
                                        subagent_event.subagent_type,
                                        truncate_for_debug(&subagent_event.description, 30)
                                    ),
                                )),
                                Some(thread_id),
                            );
                            let _ = message_tx.send(AppMessage::SubagentStarted {
                                task_id: subagent_event.task_id,
                                description: subagent_event.description,
                                subagent_type: subagent_event.subagent_type,
                            });
                        }
                        SseEvent::SubagentProgress(subagent_event) => {
                            emit_debug(
                                &debug_tx,
                                DebugEventKind::ProcessedEvent(ProcessedEventData::new(
                                    "SubagentProgress",
                                    format!(
                                        "task: {}, msg: {}",
                                        subagent_event.task_id,
                                        truncate_for_debug(&subagent_event.message, 30)
                                    ),
                                )),
                                Some(thread_id),
                            );
                            let _ = message_tx.send(AppMessage::SubagentProgress {
                                task_id: subagent_event.task_id,
                                message: subagent_event.message,
                            });
                        }
                        SseEvent::SubagentCompleted(subagent_event) => {
                            emit_debug(
                                &debug_tx,
                                DebugEventKind::ProcessedEvent(ProcessedEventData::new(
                                    "SubagentCompleted",
                                    format!(
                                        "task: {}, tools: {:?}, summary: {}",
                                        subagent_event.task_id,
                                        subagent_event.tool_call_count,
                                        truncate_for_debug(&subagent_event.summary, 30)
                                    ),
                                )),
                                Some(thread_id),
                            );
                            let _ = message_tx.send(AppMessage::SubagentCompleted {
                                task_id: subagent_event.task_id,
                                summary: subagent_event.summary,
                                tool_call_count: subagent_event.tool_call_count,
                            });
                        }
                        SseEvent::Usage(usage_event) => {
                            emit_debug(
                                &debug_tx,
                                DebugEventKind::ProcessedEvent(ProcessedEventData::new(
                                    "UsageReceived",
                                    format!(
                                        "used: {}, limit: {}",
                                        usage_event.context_window_used,
                                        usage_event.context_window_limit
                                    ),
                                )),
                                Some(thread_id),
                            );
                            let _ = message_tx.send(AppMessage::UsageReceived {
                                context_used: usage_event.context_window_used,
                                context_limit: usage_event.context_window_limit,
                            });
                        }
                        SseEvent::SystemInit(system_init_event) => {
                            // SystemInit event sent when Claude CLI starts
                            emit_debug(
                                &debug_tx,
                                DebugEventKind::ProcessedEvent(ProcessedEventData::new(
                                    "SystemInit",
                                    format!(
                                        "cli_session: {}, model: {}, mode: {}, tools: {}",
                                        system_init_event.cli_session_id,
                                        system_init_event.model,
                                        system_init_event.permission_mode,
                                        system_init_event.tool_count
                                    ),
                                )),
                                Some(thread_id),
                            );
                            // No AppMessage needed - just for debugging/logging
                        }
                        SseEvent::Cancelled(cancelled_event) => {
                            // Stream was cancelled by user request (Ctrl+C)
                            emit_debug(
                                &debug_tx,
                                DebugEventKind::StreamLifecycle(StreamLifecycleData::with_details(
                                    StreamPhase::Completed,
                                    format!("cancelled: {}", cancelled_event.reason),
                                )),
                                Some(thread_id),
                            );
                            let _ = message_tx.send(AppMessage::StreamCancelled {
                                thread_id: thread_id.to_string(),
                                reason: cancelled_event.reason,
                            });
                            break; // Exit stream loop
                        }
                    }
                }
                Err(e) => {
                    // Emit Error debug event for stream errors
                    emit_debug(
                        &debug_tx,
                        DebugEventKind::Error(ErrorData::new(
                            ErrorSource::SseConnection,
                            e.to_string(),
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{
        CancelledEvent, SubagentCompletedEvent, SubagentProgressEvent, SubagentStartedEvent,
    };
    use tokio::sync::mpsc;

    // Helper function to create a trait object stream
    fn create_stream(
        events: Vec<Result<SseEvent, crate::conductor::ConductorError>>,
    ) -> std::pin::Pin<
        Box<
            dyn futures_util::Stream<Item = Result<SseEvent, crate::conductor::ConductorError>>
                + Send,
        >,
    > {
        Box::pin(futures_util::stream::iter(events))
    }

    #[tokio::test]
    async fn test_process_stream_subagent_started() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let thread_id = "test-thread-123";

        // Create a test event
        let event = SseEvent::SubagentStarted(SubagentStartedEvent {
            task_id: "task-001".to_string(),
            description: "Test subagent task".to_string(),
            subagent_type: "Explore".to_string(),
        });

        // Create a mock stream with a single event
        let events: Vec<Result<SseEvent, crate::conductor::ConductorError>> = vec![Ok(event)];
        let mut pinned_stream = create_stream(events);

        // Process the stream
        App::process_stream(&mut pinned_stream, &tx, thread_id, None).await;

        // Verify the message was sent
        let msg = rx.recv().await.expect("Should receive message");
        match msg {
            AppMessage::SubagentStarted {
                task_id,
                description,
                subagent_type,
            } => {
                assert_eq!(task_id, "task-001");
                assert_eq!(description, "Test subagent task");
                assert_eq!(subagent_type, "Explore");
            }
            _ => panic!("Expected SubagentStarted message, got {:?}", msg),
        }
    }

    #[tokio::test]
    async fn test_process_stream_subagent_progress() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let thread_id = "test-thread-456";

        // Create a test event
        let event = SseEvent::SubagentProgress(SubagentProgressEvent {
            task_id: "task-002".to_string(),
            message: "Processing files...".to_string(),
        });

        // Create a mock stream with a single event
        let events: Vec<Result<SseEvent, crate::conductor::ConductorError>> = vec![Ok(event)];
        let mut pinned_stream = create_stream(events);

        // Process the stream
        App::process_stream(&mut pinned_stream, &tx, thread_id, None).await;

        // Verify the message was sent
        let msg = rx.recv().await.expect("Should receive message");
        match msg {
            AppMessage::SubagentProgress { task_id, message } => {
                assert_eq!(task_id, "task-002");
                assert_eq!(message, "Processing files...");
            }
            _ => panic!("Expected SubagentProgress message, got {:?}", msg),
        }
    }

    #[tokio::test]
    async fn test_process_stream_subagent_completed() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let thread_id = "test-thread-789";

        // Create a test event
        let event = SseEvent::SubagentCompleted(SubagentCompletedEvent {
            task_id: "task-003".to_string(),
            summary: "Successfully analyzed codebase".to_string(),
            tool_call_count: Some(15),
        });

        // Create a mock stream with a single event
        let events: Vec<Result<SseEvent, crate::conductor::ConductorError>> = vec![Ok(event)];
        let mut pinned_stream = create_stream(events);

        // Process the stream
        App::process_stream(&mut pinned_stream, &tx, thread_id, None).await;

        // Verify the message was sent
        let msg = rx.recv().await.expect("Should receive message");
        match msg {
            AppMessage::SubagentCompleted {
                task_id,
                summary,
                tool_call_count,
            } => {
                assert_eq!(task_id, "task-003");
                assert_eq!(summary, "Successfully analyzed codebase");
                assert_eq!(tool_call_count, Some(15));
            }
            _ => panic!("Expected SubagentCompleted message, got {:?}", msg),
        }
    }

    #[tokio::test]
    async fn test_process_stream_subagent_completed_without_tool_count() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let thread_id = "test-thread-999";

        // Create a test event without tool_call_count
        let event = SseEvent::SubagentCompleted(SubagentCompletedEvent {
            task_id: "task-004".to_string(),
            summary: "Task completed".to_string(),
            tool_call_count: None,
        });

        // Create a mock stream with a single event
        let events: Vec<Result<SseEvent, crate::conductor::ConductorError>> = vec![Ok(event)];
        let mut pinned_stream = create_stream(events);

        // Process the stream
        App::process_stream(&mut pinned_stream, &tx, thread_id, None).await;

        // Verify the message was sent
        let msg = rx.recv().await.expect("Should receive message");
        match msg {
            AppMessage::SubagentCompleted {
                task_id,
                summary,
                tool_call_count,
            } => {
                assert_eq!(task_id, "task-004");
                assert_eq!(summary, "Task completed");
                assert_eq!(tool_call_count, None);
            }
            _ => panic!("Expected SubagentCompleted message, got {:?}", msg),
        }
    }

    #[tokio::test]
    async fn test_process_stream_multiple_subagent_events() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let thread_id = "test-thread-multi";

        // Create multiple test events
        let events: Vec<Result<SseEvent, crate::conductor::ConductorError>> = vec![
            Ok(SseEvent::SubagentStarted(SubagentStartedEvent {
                task_id: "task-multi".to_string(),
                description: "Multi-event test".to_string(),
                subagent_type: "Plan".to_string(),
            })),
            Ok(SseEvent::SubagentProgress(SubagentProgressEvent {
                task_id: "task-multi".to_string(),
                message: "Step 1 complete".to_string(),
            })),
            Ok(SseEvent::SubagentProgress(SubagentProgressEvent {
                task_id: "task-multi".to_string(),
                message: "Step 2 complete".to_string(),
            })),
            Ok(SseEvent::SubagentCompleted(SubagentCompletedEvent {
                task_id: "task-multi".to_string(),
                summary: "All steps completed".to_string(),
                tool_call_count: Some(20),
            })),
        ];
        let mut pinned_stream = create_stream(events);

        // Process the stream
        App::process_stream(&mut pinned_stream, &tx, thread_id, None).await;

        // Verify all messages were sent in order
        let msg1 = rx.recv().await.expect("Should receive first message");
        match msg1 {
            AppMessage::SubagentStarted { task_id, .. } => {
                assert_eq!(task_id, "task-multi");
            }
            _ => panic!("Expected SubagentStarted as first message"),
        }

        let msg2 = rx.recv().await.expect("Should receive second message");
        match msg2 {
            AppMessage::SubagentProgress { message, .. } => {
                assert_eq!(message, "Step 1 complete");
            }
            _ => panic!("Expected SubagentProgress as second message"),
        }

        let msg3 = rx.recv().await.expect("Should receive third message");
        match msg3 {
            AppMessage::SubagentProgress { message, .. } => {
                assert_eq!(message, "Step 2 complete");
            }
            _ => panic!("Expected SubagentProgress as third message"),
        }

        let msg4 = rx.recv().await.expect("Should receive fourth message");
        match msg4 {
            AppMessage::SubagentCompleted {
                summary,
                tool_call_count,
                ..
            } => {
                assert_eq!(summary, "All steps completed");
                assert_eq!(tool_call_count, Some(20));
            }
            _ => panic!("Expected SubagentCompleted as fourth message"),
        }
    }

    #[tokio::test]
    async fn test_process_stream_cancelled() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let thread_id = "test-thread-cancel";

        // Create a cancelled event
        let event = SseEvent::Cancelled(CancelledEvent {
            reason: "user_requested".to_string(),
        });

        // Create a mock stream with the event
        let events: Vec<Result<SseEvent, crate::conductor::ConductorError>> = vec![Ok(event)];
        let mut pinned_stream = create_stream(events);

        // Process the stream
        App::process_stream(&mut pinned_stream, &tx, thread_id, None).await;

        // Verify the message was sent
        let msg = rx.recv().await.expect("Should receive message");
        match msg {
            AppMessage::StreamCancelled { thread_id, reason } => {
                assert_eq!(thread_id, "test-thread-cancel");
                assert_eq!(reason, "user_requested");
            }
            _ => panic!("Expected StreamCancelled message, got {:?}", msg),
        }
    }

    #[tokio::test]
    async fn test_process_stream_cancelled_breaks_loop() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let thread_id = "test-thread-cancel-break";

        // Create events: cancelled followed by content (should not be processed)
        let events: Vec<Result<SseEvent, crate::conductor::ConductorError>> = vec![
            Ok(SseEvent::Cancelled(CancelledEvent {
                reason: "user_requested".to_string(),
            })),
            Ok(SseEvent::Content(crate::events::ContentEvent {
                text: "This should not be processed".to_string(),
                meta: crate::events::EventMeta::default(),
            })),
        ];
        let mut pinned_stream = create_stream(events);

        // Process the stream
        App::process_stream(&mut pinned_stream, &tx, thread_id, None).await;

        // Should only receive the cancelled message, not the content
        let msg = rx.recv().await.expect("Should receive message");
        match msg {
            AppMessage::StreamCancelled { .. } => {}
            _ => panic!("Expected StreamCancelled message, got {:?}", msg),
        }

        // Try to receive another message - should be None since we broke out of the loop
        // Use try_recv to avoid blocking
        assert!(rx.try_recv().is_err(), "Should not receive additional messages after cancelled");
    }
}
