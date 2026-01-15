//! Streaming input submission and SSE event processing for the App.

use std::sync::Arc;

use futures_util::StreamExt;
use tokio::sync::mpsc;

use crate::debug::{
    DebugEventKind, ErrorData, ErrorSource, ProcessedEventData, StreamLifecycleData, StreamPhase,
};
use crate::events::SseEvent;
use crate::models::{StreamRequest, ThreadType};
use crate::state::Todo;

use super::{emit_debug, log_thread_update, truncate_for_debug, App, AppMessage, ProgrammingMode, Screen};
use crate::debug::DebugEventSender;

impl App {
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

        // CRITICAL: Check screen first to determine new vs continue.
        // CommandDeck = ALWAYS new thread (regardless of any stale active_thread_id)
        // Conversation = continue the thread that was opened via open_thread()
        let is_command_deck = self.screen == Screen::CommandDeck;

        // Determine thread_type based on screen and active thread
        let thread_type = if is_command_deck {
            // New thread from CommandDeck - use the specified type
            new_thread_type
        } else if self.is_active_thread_programming() {
            ThreadType::Programming
        } else {
            ThreadType::Normal
        };

        // Determine plan_mode for programming threads
        let plan_mode = if thread_type == ThreadType::Programming {
            matches!(self.programming_mode, ProgrammingMode::PlanMode)
        } else {
            false
        };

        // Determine thread_id based on screen
        let (thread_id, is_new_thread) = if is_command_deck {
            // NEW thread - create pending, will reconcile when backend responds
            let pending_id = self
                .cache
                .create_pending_thread(content.clone(), new_thread_type);
            self.active_thread_id = Some(pending_id.clone());
            self.screen = Screen::Conversation;
            // Reset scroll for new conversation
            self.conversation_scroll = 0;
            (pending_id, true)
        } else if let Some(existing_id) = &self.active_thread_id {
            // CONTINUING existing thread (we're on Conversation screen)
            // Check if thread is still pending (waiting for backend ThreadInfo)
            if existing_id.starts_with("pending-") {
                // Block rapid second message - still waiting for ThreadInfo
                self.stream_error = Some(
                    "Please wait for the current response to complete before sending another message."
                        .to_string(),
                );
                return;
            }

            if !self.cache.add_streaming_message(existing_id, content.clone()) {
                // Thread doesn't exist in cache - might have been deleted
                self.stream_error = Some("Thread no longer exists.".to_string());
                return;
            }
            (existing_id.clone(), false)
        } else {
            // Edge case: on Conversation screen but no active_thread_id (shouldn't happen)
            // Fall back to creating new thread
            let pending_id = self
                .cache
                .create_pending_thread(content.clone(), new_thread_type);
            self.active_thread_id = Some(pending_id.clone());
            self.conversation_scroll = 0;
            (pending_id, true)
        };

        self.input_box.clear();

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
                    emit_debug(
                        &debug_tx,
                        DebugEventKind::StreamLifecycle(StreamLifecycleData::new(
                            StreamPhase::Connected,
                        )),
                        Some(&thread_id_for_task),
                    );
                    Self::process_stream(&mut stream, &message_tx, &thread_id_for_task, debug_tx)
                        .await;
                }
                Err(e) => {
                    // Emit error debug event
                    emit_debug(
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

    /// Process a stream of SSE events and send messages to the app.
    ///
    /// This is a helper method extracted from submit_input to avoid code duplication
    /// between the programming and standard streaming paths.
    pub(super) async fn process_stream(
        stream: &mut std::pin::Pin<
            Box<
                dyn futures_util::Stream<
                        Item = Result<SseEvent, crate::conductor::ConductorError>,
                    > + Send,
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

                            // Emit ProcessedEvent
                            emit_debug(
                                &debug_tx,
                                DebugEventKind::ProcessedEvent(ProcessedEventData::new(
                                    "StreamToken",
                                    format!(
                                        "token: '{}'",
                                        truncate_for_debug(&content_event.text, 50)
                                    ),
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
                                tool_call_id: tool_event.tool_call_id,
                                tool_name: tool_event.tool_name,
                            });
                        }
                        SseEvent::ToolCallArgument(arg_event) => {
                            // Send argument chunk to be accumulated in the ToolEvent
                            let _ = message_tx.send(AppMessage::ToolArgumentChunk {
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
                        // Ignore other event types for now
                        _ => {}
                    }
                }
                Err(e) => {
                    // Emit Error debug event for stream errors
                    emit_debug(
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
}
