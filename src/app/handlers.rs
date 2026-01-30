//! Message handling for the App.

use crate::credential_watcher::{
    handle_credential_change, handle_debounce_expired, handle_sync_complete, handle_sync_failed,
};
use crate::debug::{DebugEventKind, ErrorData, ErrorSource, StateChangeData, StateType};
use crate::models::ThreadMode;
use crate::state::dashboard::PhaseProgressData;
use tracing::info;

use super::{emit_debug, log_thread_update, truncate_for_debug, App, AppMessage};

impl App {
    /// Handle an incoming async message
    /// All message handlers mark the app as dirty since they update visible state.
    pub fn handle_message(&mut self, msg: AppMessage) {
        // All messages result in state changes that require a redraw
        self.mark_dirty();
        match msg {
            AppMessage::StreamToken { thread_id, token } => {
                // Initialize stream start time if this is the first token
                let now = std::time::Instant::now();
                if self.stream_start_time.is_none() {
                    self.stream_start_time = Some(now);
                }

                // Calculate latency since last event
                let latency_ms = self
                    .last_event_time
                    .map(|last| now.duration_since(last).as_millis() as u64);
                self.last_event_time = Some(now);

                // Estimate token count (rough approximation: 4 chars per token)
                let estimated_tokens = (token.len() as f64 / 4.0).ceil() as u64;
                self.cumulative_token_count += estimated_tokens;

                // Calculate tokens per second
                let tokens_per_second = if let Some(start) = self.stream_start_time {
                    let elapsed_secs = now.duration_since(start).as_secs_f64();
                    if elapsed_secs > 0.0 {
                        Some(self.cumulative_token_count as f64 / elapsed_secs)
                    } else {
                        None
                    }
                } else {
                    None
                };

                self.cache.append_to_message(&thread_id, &token);

                // Emit ProcessedEvent with statistics
                use crate::debug::ProcessedEventData;
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::ProcessedEvent(ProcessedEventData::with_stats(
                        "StreamToken",
                        format!("token: '{}'", truncate_for_debug(&token, 50)),
                        Some(self.cumulative_token_count),
                        tokens_per_second,
                        latency_ms,
                    )),
                    Some(&thread_id),
                );

                // Emit StateChange for message cache update
                emit_debug(
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
                    self.reset_scroll();
                }
            }
            AppMessage::ReasoningToken { thread_id, token } => {
                self.cache.append_reasoning_to_message(&thread_id, &token);
                // Emit StateChange for reasoning update
                emit_debug(
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
                    self.reset_scroll();
                }
            }
            AppMessage::StreamComplete {
                thread_id,
                message_id,
            } => {
                self.cache.finalize_message(&thread_id, message_id);

                // Reset stream statistics
                self.stream_start_time = None;
                self.last_event_time = None;
                self.cumulative_token_count = 0;

                // Reset cancel state
                self.reset_cancel_state();

                // Emit StateChange for message finalization
                emit_debug(
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
                emit_debug(
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
                    self.reset_scroll();
                }

                // Fire native OS notification when the TUI is not focused
                if !self.is_focused {
                    let title = self.cache.get_thread(&thread_id).map(|t| t.title.as_str());
                    crate::notifications::notify_task_complete(title);
                }
            }
            AppMessage::StreamError {
                thread_id,
                error,
            } => {
                // Clear queued steering on stream error
                if let Some(ref qs) = self.queued_steering {
                    if qs.thread_id == thread_id {
                        self.queued_steering = None;
                    }
                }

                // Reset stream statistics on error
                self.stream_start_time = None;
                self.last_event_time = None;
                self.cumulative_token_count = 0;

                // Reset cancel state
                self.reset_cancel_state();

                // Emit Error debug event
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::Error(ErrorData::new(ErrorSource::AppState, &error)),
                    None,
                );
                self.stream_error = Some(error);
            }
            AppMessage::StreamCancelled { thread_id, reason } => {
                // Clear queued steering on cancel
                if let Some(ref qs) = self.queued_steering {
                    if qs.thread_id == thread_id {
                        self.queued_steering = None;
                    }
                }

                // Mark message as no longer streaming
                self.cache.cancel_streaming_message(&thread_id);

                // Reset stream statistics
                self.stream_start_time = None;
                self.last_event_time = None;
                self.cumulative_token_count = 0;

                // Reset cancel state
                self.reset_cancel_state();

                // Clear tool tracker when stream is cancelled
                self.tool_tracker.clear();

                // Emit StateChange for stream cancellation
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::MessageCache,
                        "Stream cancelled",
                        format!("thread: {}, reason: {}", thread_id, reason),
                    )),
                    Some(&thread_id),
                );

                // Auto-scroll to bottom, but only for the active thread
                if self.active_thread_id.as_ref() == Some(&thread_id) {
                    self.reset_scroll();
                }
            }
            AppMessage::ConnectionStatus(connected) => {
                // Emit StateChange for connection status
                emit_debug(
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
                emit_debug(
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
            AppMessage::MessagesLoaded {
                thread_id,
                messages,
            } => {
                let count = messages.len();
                log_thread_update(&format!(
                    "HANDLER: MessagesLoaded received for {}, {} messages",
                    thread_id, count
                ));
                self.cache.set_messages(thread_id.clone(), messages);
                log_thread_update(&format!(
                    "HANDLER: Messages stored in cache for {}",
                    thread_id
                ));
                // Emit StateChange for messages loaded
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::MessageCache,
                        "Messages loaded",
                        format!("{} messages", count),
                    )),
                    Some(&thread_id),
                );
            }
            AppMessage::MessagesLoadError { thread_id, error } => {
                log_thread_update(&format!(
                    "HANDLER: MessagesLoadError for {}: {}",
                    thread_id, error
                ));
                // Emit Error debug event
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::Error(ErrorData::new(ErrorSource::Cache, &error)),
                    None,
                );
                self.stream_error = Some(error);
            }
            AppMessage::TodosUpdated { todos } => {
                let count = todos.len();
                self.todos = todos;
                // Emit StateChange for todos update
                emit_debug(
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
                thread_id,
                tool_name,
                description,
                tool_input,
            } => {
                info!("PermissionRequested: tool={} id={} thread={:?}", tool_name, permission_id, thread_id);

                // Special handling for ExitPlanMode - convert to plan approval flow
                // This allows showing the full plan markdown content instead of a generic Y/N prompt
                if tool_name == "ExitPlanMode" {
                    use crate::models::dashboard::{
                        PlanRequest, PlanSummary, ThreadStatus, WaitingFor,
                    };

                    // Extract plan content from tool_input.plan
                    let plan_content = tool_input
                        .as_ref()
                        .and_then(|input| input.get("plan"))
                        .and_then(|v| v.as_str())
                        .map(String::from);

                    let effective_thread_id = thread_id
                        .clone()
                        .unwrap_or_else(|| "unknown".to_string());

                    info!(
                        "Converting ExitPlanMode permission to plan approval: thread={}, has_content={}",
                        effective_thread_id,
                        plan_content.is_some()
                    );

                    // Create PlanSummary with the plan content
                    let plan_summary = PlanSummary::with_content(
                        "Plan Approval".to_string(), // Default title
                        vec![],                       // No phases in this format
                        0,                            // No file count
                        None,                         // No token estimate
                        None,                         // Plan file path (could extract if available)
                        plan_content,                 // The actual plan markdown
                    );

                    // Update dashboard as plan approval request
                    self.dashboard.update_thread_status(
                        &effective_thread_id,
                        ThreadStatus::Waiting,
                        Some(WaitingFor::PlanApproval {
                            request_id: permission_id.clone(),
                        }),
                    );
                    self.dashboard.set_plan_request(
                        &effective_thread_id,
                        PlanRequest::from_permission(permission_id.clone(), plan_summary.clone()),
                    );

                    // Compute thread views to ensure needs_action is set
                    self.dashboard.compute_thread_views();

                    // Emit StateChange for plan approval
                    emit_debug(
                        &self.debug_tx,
                        DebugEventKind::StateChange(StateChangeData::new(
                            StateType::DashboardState,
                            "ExitPlanMode converted to plan approval",
                            format!(
                                "id: {}, thread: {}, title: {}",
                                permission_id, effective_thread_id, plan_summary.title
                            ),
                        )),
                        Some(&effective_thread_id),
                    );

                    // Skip normal permission handling
                    return;
                }

                // Check if this tool is already allowed (user previously chose "Always")
                if self.session_state.is_tool_allowed(&tool_name) {
                    // Auto-approve - send approval back to backend
                    self.approve_permission(&permission_id);
                } else {
                    use crate::state::PermissionRequest;

                    // Determine the thread_id to use - fallback to "unknown" if None
                    let effective_thread_id = thread_id
                        .clone()
                        .unwrap_or_else(|| "unknown".to_string());

                    // Store permission request for user approval in dashboard (per-thread)
                    self.dashboard.set_pending_permission(
                        &effective_thread_id,
                        PermissionRequest {
                            permission_id: permission_id.clone(),
                            thread_id: thread_id.clone(),
                            tool_name: tool_name.clone(),
                            description,
                            context: None, // Context will be extracted from tool_input in UI
                            tool_input,
                            received_at: std::time::Instant::now(),
                        },
                    );

                    // Set thread waiting state to ensure it shows as needs_action
                    self.dashboard.update_thread_status(
                        &effective_thread_id,
                        crate::models::dashboard::ThreadStatus::Waiting,
                        Some(crate::models::dashboard::WaitingFor::Permission {
                            request_id: permission_id.clone(),
                            tool_name: tool_name.clone(),
                        }),
                    );

                    // Compute thread views to ensure needs_action is set
                    self.dashboard.compute_thread_views();

                    // AskUserQuestion requires auto-initialization of question state
                    if tool_name == "AskUserQuestion" {
                        self.init_question_state();
                    }

                    // Emit StateChange for pending permission
                    emit_debug(
                        &self.debug_tx,
                        DebugEventKind::StateChange(StateChangeData::new(
                            StateType::DashboardState,
                            "Permission pending",
                            format!(
                                "tool: {}, id: {}, thread: {}",
                                tool_name, permission_id, effective_thread_id
                            ),
                        )),
                        Some(&effective_thread_id),
                    );
                }
            }
            AppMessage::ToolStarted {
                thread_id,
                tool_call_id,
                tool_name,
            } => {
                // Register tool in tracker with display status for UI
                self.tool_tracker.register_tool_started(
                    tool_call_id.clone(),
                    tool_name.clone(),
                    self.tick_count,
                );
                // Emit StateChange for tool tracker
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::ToolTracker,
                        "Tool started",
                        format!("tool: {}", tool_name),
                    )),
                    Some(&thread_id),
                );
                // Also add tool event inline to the streaming message
                self.cache
                    .start_tool_in_message(&thread_id, tool_call_id, tool_name);
            }
            AppMessage::ToolExecuting {
                thread_id,
                tool_call_id,
                display_name,
            } => {
                // Update tool to executing state with display info
                self.tool_tracker
                    .set_tool_executing(&tool_call_id, display_name.clone());
                // Update the display_name in the message segments
                self.cache
                    .set_tool_display_name(&thread_id, &tool_call_id, display_name.clone());
                // Update dashboard state with display_name as current_operation for activity display
                self.dashboard
                    .update_current_operation(&thread_id, Some(&display_name));
                // Emit StateChange for tool executing
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::ToolTracker,
                        "Tool executing",
                        format!("display: {}", truncate_for_debug(&display_name, 40)),
                    )),
                    Some(&thread_id),
                );
            }
            AppMessage::ToolCompleted {
                thread_id,
                tool_call_id,
                success,
                summary,
                result,
            } => {
                // Mark tool as completed with summary for fade display
                self.tool_tracker.complete_tool_with_summary(
                    &tool_call_id,
                    success,
                    summary.clone(),
                    self.tick_count,
                );
                // Emit StateChange for tool completion
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::ToolTracker,
                        "Tool completed",
                        format!(
                            "success: {}, summary: {}",
                            success,
                            truncate_for_debug(&summary, 30)
                        ),
                    )),
                    Some(&thread_id),
                );
                // Also update the inline tool event in the streaming message
                // Store the result content in the tool event
                self.cache
                    .set_tool_result(&thread_id, &tool_call_id, &result, !success);
                if success {
                    self.cache
                        .complete_tool_in_message(&thread_id, &tool_call_id);
                } else {
                    self.cache.fail_tool_in_message(&thread_id, &tool_call_id);
                }
            }
            AppMessage::ToolArgumentChunk {
                thread_id,
                tool_call_id,
                chunk,
            } => {
                // Append argument chunk to the tool event for live display
                self.cache
                    .append_tool_argument(&thread_id, &tool_call_id, &chunk);
            }
            AppMessage::SkillsInjected { skills } => {
                let count = skills.len();
                // Update session state with injected skills
                for skill in skills {
                    self.session_state.add_skill(skill);
                }
                // Emit StateChange for skills injection
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::SessionState,
                        "Skills injected",
                        format!("{} skills", count),
                    )),
                    None,
                );
            }
            AppMessage::OAuthConsentRequired {
                provider,
                url,
                skill_name,
            } => {
                // Store OAuth requirement in session state
                if let Some(skill) = skill_name {
                    self.session_state
                        .set_oauth_required(provider.clone(), skill);
                }
                if let Some(consent_url) = url {
                    self.session_state.set_oauth_url(consent_url);
                }
                // Emit StateChange for OAuth requirement
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::SessionState,
                        "OAuth consent required",
                        format!("provider: {}", provider),
                    )),
                    None,
                );
            }
            AppMessage::ContextCompacted {
                tokens_used,
                token_limit,
            } => {
                // Update context tracking in session state
                if let Some(used) = tokens_used {
                    self.session_state.set_context_tokens(used);
                }
                if let Some(limit) = token_limit {
                    self.session_state.set_context_token_limit(limit);
                }
                // Emit StateChange for context compaction
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::SessionState,
                        "Context compacted",
                        format!("tokens: {:?}/{:?}", tokens_used, token_limit),
                    )),
                    None,
                );
            }
            AppMessage::ThreadMetadataUpdated {
                thread_id,
                title,
                description,
            } => {
                log_thread_update(&format!(
                    "Updating cache: id={}, title={:?}, description={:?}",
                    thread_id, title, description
                ));
                let updated = self.cache.update_thread_metadata(
                    &thread_id,
                    title.clone(),
                    description.clone(),
                );
                // Also update dashboard state so thread views reflect the new title
                self.dashboard
                    .update_thread_metadata(&thread_id, title.clone(), description.clone());
                log_thread_update(&format!(
                    "Cache update result: id={}, success={}",
                    thread_id, updated
                ));
                // Emit StateChange for thread metadata update
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::ThreadCache,
                        "Thread metadata updated",
                        format!("title: {:?}, updated: {}", title, updated),
                    )),
                    Some(&thread_id),
                );
            }
            AppMessage::SubagentStarted {
                task_id,
                description,
                subagent_type,
            } => {
                // Add subagent event to the streaming message
                if let Some(thread_id) = &self.active_thread_id {
                    self.cache.start_subagent_in_message(
                        thread_id,
                        task_id.clone(),
                        description.clone(),
                        subagent_type.clone(),
                    );
                }

                // Register subagent in tracker
                self.subagent_tracker.register_subagent(
                    task_id.clone(),
                    subagent_type.clone(),
                    description.clone(),
                    self.tick_count,
                );

                // Emit StateChange for subagent tracker update
                let active_count = self.subagent_tracker.active_count();
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::SubagentTracker,
                        "Subagent registered",
                        format!("active: {}, task: {}", active_count, task_id),
                    )),
                    self.active_thread_id.as_deref(),
                );

                // Emit StateChange for subagent started
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::MessageCache,
                        "Subagent started",
                        format!(
                            "task: {}, type: {}, desc: {}",
                            task_id,
                            subagent_type,
                            truncate_for_debug(&description, 30)
                        ),
                    )),
                    self.active_thread_id.as_deref(),
                );
            }
            AppMessage::SubagentProgress { task_id, message } => {
                // Update subagent progress in the message
                if let Some(thread_id) = &self.active_thread_id {
                    self.cache
                        .update_subagent_progress(thread_id, &task_id, message.clone());
                }

                // Update subagent progress in tracker
                if let Some(subagent) = self.subagent_tracker.get_subagent_mut(&task_id) {
                    subagent.set_progress(message.clone());
                }

                // Emit StateChange for subagent progress
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::MessageCache,
                        "Subagent progress",
                        format!(
                            "task: {}, msg: {}",
                            task_id,
                            truncate_for_debug(&message, 40)
                        ),
                    )),
                    self.active_thread_id.as_deref(),
                );
            }
            AppMessage::SubagentCompleted {
                task_id,
                summary,
                tool_call_count,
            } => {
                // Convert empty string to None for optional summary
                let summary_opt = if summary.is_empty() {
                    None
                } else {
                    Some(summary.clone())
                };

                // Mark subagent as completed in the message
                if let Some(thread_id) = &self.active_thread_id {
                    self.cache.complete_subagent_in_message(
                        thread_id,
                        &task_id,
                        summary_opt.clone(),
                        tool_call_count.unwrap_or(0) as usize,
                    );
                }

                // Mark subagent as complete in tracker
                if let Some(subagent) = self.subagent_tracker.get_subagent_mut(&task_id) {
                    subagent.complete(true, summary.clone(), self.tick_count);
                }

                // Remove completed subagent from tracker
                self.subagent_tracker.remove_subagent(&task_id);

                // Emit StateChange for subagent tracker update
                let active_count = self.subagent_tracker.active_count();
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::SubagentTracker,
                        "Subagent completed and removed",
                        format!("active: {}, task: {}", active_count, task_id),
                    )),
                    self.active_thread_id.as_deref(),
                );

                // Emit StateChange for subagent completion
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::MessageCache,
                        "Subagent completed",
                        format!(
                            "task: {}, summary: {}, tools: {:?}",
                            task_id,
                            summary_opt
                                .as_ref()
                                .map(|s| truncate_for_debug(s, 30))
                                .unwrap_or_else(|| "none".to_string()),
                            tool_call_count
                        ),
                    )),
                    self.active_thread_id.as_deref(),
                );
            }
            AppMessage::UsageReceived {
                context_used,
                context_limit,
            } => {
                // Update context tracking in session state
                self.session_state.set_context_tokens(context_used);
                self.session_state.set_context_token_limit(context_limit);
                // Emit StateChange for usage update
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::SessionState,
                        "Usage received",
                        format!("tokens: {}/{}", context_used, context_limit),
                    )),
                    None,
                );
            }
            AppMessage::WsConnected => {
                use crate::websocket::WsConnectionState;
                tracing::info!("WS_CONNECTED: WebSocket connection established");
                self.ws_connection_state = WsConnectionState::Connected;
                // Emit StateChange for WebSocket connection
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::WebSocket,
                        "WS_CONNECTED",
                        "WebSocket connection established - awaiting backend events",
                    )),
                    None,
                );
            }
            AppMessage::WsDisconnected => {
                use crate::websocket::WsConnectionState;
                tracing::info!("WebSocket disconnected");
                self.ws_connection_state = WsConnectionState::Disconnected;
                // Emit StateChange for WebSocket disconnection
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::SessionState,
                        "WebSocket disconnected",
                        "disconnected",
                    )),
                    None,
                );
            }
            AppMessage::WsReconnecting { attempt } => {
                use crate::websocket::WsConnectionState;
                tracing::info!("WebSocket reconnecting (attempt {})", attempt);
                self.ws_connection_state = WsConnectionState::Reconnecting { attempt };
                // Emit StateChange for WebSocket reconnection attempt
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::SessionState,
                        "WebSocket reconnecting",
                        format!("attempt: {}", attempt),
                    )),
                    None,
                );
            }
            AppMessage::WsRawMessage { message } => {
                // Log WebSocket raw message for debugging
                tracing::info!("WS_RAW: {}", &message[..message.len().min(100)]);
                // Emit debug event for raw WebSocket message (for debugging)
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::WebSocket,
                        "WS_RECV",
                        message,
                    )),
                    None,
                );
            }
            AppMessage::WsParseError { error, raw } => {
                // Emit debug event for WebSocket parse error (for debugging)
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::Error(ErrorData::new(
                        ErrorSource::WebSocket,
                        format!("WS parse error: {} | raw: {}", error, raw),
                    )),
                    None,
                );
            }
            AppMessage::FoldersLoaded(folders) => {
                let count = folders.len();
                self.folders = folders;
                self.folders_loading = false;
                self.folders_error = None;
                // Emit StateChange for folders loaded
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::SessionState,
                        "Folders loaded",
                        format!("{} folders", count),
                    )),
                    None,
                );
            }
            AppMessage::FoldersLoadFailed(error) => {
                self.folders_loading = false;
                self.folders_error = Some(error.clone());
                // Emit Error debug event
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::Error(ErrorData::new(ErrorSource::AppState, &error)),
                    None,
                );
            }
            AppMessage::ReposLoaded(repos) => {
                let count = repos.len();
                self.repos = repos;
                self.repos_loading = false;
                self.repos_error = None;
                // Emit StateChange for repos loaded
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::SessionState,
                        "Repos loaded",
                        format!("count={}", count),
                    )),
                    None,
                );
            }
            AppMessage::ReposLoadFailed(error) => {
                self.repos_loading = false;
                self.repos_error = Some(error.clone());
                // Emit Error debug event
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::Error(ErrorData::new(ErrorSource::AppState, &error)),
                    None,
                );
            }
            AppMessage::FilesLoaded(files) => {
                let count = files.len();
                self.file_picker.set_items(files);
                // Emit StateChange for files loaded
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::SessionState,
                        "Files loaded",
                        format!("count={}", count),
                    )),
                    None,
                );
                self.mark_dirty();
            }
            AppMessage::FilesLoadFailed(error) => {
                self.file_picker.set_error(error.clone());
                // Emit Error debug event
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::Error(ErrorData::new(ErrorSource::AppState, &error)),
                    None,
                );
                self.mark_dirty();
            }
            AppMessage::FolderPickerOpen => {
                self.folder_picker_visible = true;
                self.folder_picker_filter.clear();
                self.folder_picker_cursor = 0;
                // Emit StateChange for folder picker open
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::SessionState,
                        "Folder picker opened",
                        "visible",
                    )),
                    None,
                );
            }
            AppMessage::FolderPickerClose => {
                self.folder_picker_visible = false;
                self.folder_picker_filter.clear();
                self.folder_picker_cursor = 0;
                // Emit StateChange for folder picker close
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::SessionState,
                        "Folder picker closed",
                        "hidden",
                    )),
                    None,
                );
            }
            AppMessage::FolderPickerFilterChanged(filter) => {
                self.folder_picker_filter = filter.clone();
                // Reset cursor to 0 when filter changes
                self.folder_picker_cursor = 0;
                // Emit StateChange for filter change
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::SessionState,
                        "Folder picker filter changed",
                        truncate_for_debug(&filter, 30),
                    )),
                    None,
                );
            }
            AppMessage::FolderPickerCursorUp => {
                if self.folder_picker_cursor > 0 {
                    self.folder_picker_cursor -= 1;
                }
            }
            AppMessage::FolderPickerCursorDown => {
                // Note: Actual bounds checking against filtered list happens at render time
                // Here we just increment; the UI will clamp to valid range
                self.folder_picker_cursor += 1;
            }
            AppMessage::FolderSelected(folder) => {
                let folder_name = folder.name.clone();
                self.selected_folder = Some(folder);
                self.folder_picker_visible = false;
                self.folder_picker_filter.clear();
                self.folder_picker_cursor = 0;
                // Emit StateChange for folder selection
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::SessionState,
                        "Folder selected",
                        truncate_for_debug(&folder_name, 30),
                    )),
                    None,
                );
            }
            AppMessage::FolderCleared => {
                self.selected_folder = None;
                // Emit StateChange for folder cleared
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::SessionState,
                        "Folder cleared",
                        "none",
                    )),
                    None,
                );
            }
            AppMessage::SystemStatsUpdate(stats) => {
                // Update system stats for dashboard header display
                self.system_stats = stats;
                // Emit StateChange for system stats update
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::SessionState,
                        "System stats updated",
                        format!(
                            "cpu: {:.1}%, ram: {:.1}/{:.1}GB",
                            self.system_stats.cpu_percent,
                            self.system_stats.ram_used_gb,
                            self.system_stats.ram_total_gb
                        ),
                    )),
                    None,
                );
            }
            AppMessage::ThreadStatusUpdate {
                thread_id,
                status,
                waiting_for,
            } => {
                // Update dashboard state with thread status
                self.dashboard
                    .update_thread_status(&thread_id, status, waiting_for.clone());
                // Emit StateChange for thread status update
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::WebSocket,
                        "WS_THREAD_STATUS",
                        format!("thread: {}, status: {:?}", thread_id, status),
                    )),
                    Some(&thread_id),
                );
            }
            AppMessage::AgentStatusUpdate {
                thread_id,
                state,
                current_operation,
            } => {
                // Log for terminal debugging
                tracing::info!("WS_AGENT_STATUS: thread={}, state={}", thread_id, state);
                // Update dashboard state with agent status
                self.dashboard
                    .update_agent_state(&thread_id, &state, current_operation.as_deref());
                // Emit StateChange for agent status update
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::WebSocket,
                        "WS_AGENT_STATUS",
                        format!("thread: {}, state: {}", thread_id, state),
                    )),
                    Some(&thread_id),
                );
            }
            AppMessage::PlanApprovalRequest {
                thread_id,
                request_id,
                plan_summary,
            } => {
                info!("PlanApprovalRequest: thread={} req={} phases={}", thread_id, request_id, plan_summary.phases.len());
                // Update dashboard state with plan request and waiting state
                use crate::models::dashboard::{ThreadStatus, WaitingFor};
                self.dashboard.update_thread_status(
                    &thread_id,
                    ThreadStatus::Waiting,
                    Some(WaitingFor::PlanApproval {
                        request_id: request_id.clone(),
                    }),
                );
                self.dashboard.set_plan_request(
                    &thread_id,
                    crate::models::dashboard::PlanRequest::new(
                        request_id.clone(),
                        plan_summary.clone(),
                    ),
                );
                // Emit StateChange for plan approval request
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::DashboardState,
                        "Plan approval requested",
                        format!(
                            "thread: {}, request_id: {}, plan: {}",
                            thread_id, request_id, plan_summary.title
                        ),
                    )),
                    Some(&thread_id),
                );
            }
            AppMessage::WsThreadCreated { thread } => {
                // Log for terminal debugging with detailed thread information
                tracing::info!(
                    "WS_THREAD_CREATED: thread_id={}, title={:?}, mode={:?}, status={:?}, verified={:?}",
                    thread.id,
                    thread.title,
                    thread.mode,
                    thread.status,
                    thread.verified
                );
                // Add newly created thread to dashboard state
                let thread_id = thread.id.clone();
                self.dashboard.add_thread(thread);
                // Emit StateChange for new thread
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::WebSocket,
                        "WS_THREAD_CREATED",
                        format!("thread_id: {} - successfully added to dashboard", thread_id),
                    )),
                    Some(&thread_id),
                );
            }
            AppMessage::ThreadModeUpdate { thread_id, mode } => {
                // Log for terminal debugging
                tracing::info!(
                    "THREAD_MODE_UPDATE: thread_id={}, mode={:?}",
                    thread_id,
                    mode
                );
                // Update thread mode in dashboard state
                self.dashboard.update_thread_mode(&thread_id, mode);

                // Track planning state for UI indicator
                self.dashboard
                    .set_thread_planning(&thread_id, mode == ThreadMode::Plan);

                // Emit StateChange for thread mode update
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::DashboardState,
                        "Thread mode updated",
                        format!("thread_id: {}, mode: {:?}", thread_id, mode),
                    )),
                    Some(&thread_id),
                );
            }
            AppMessage::PhaseProgressUpdate {
                thread_id,
                plan_id,
                phase_index,
                total_phases,
                phase_name,
                status,
                tool_count,
                last_tool,
                last_file,
            } => {
                // Log for terminal debugging
                tracing::info!(
                    "PHASE_PROGRESS_UPDATE: plan_id={}, phase={}/{}, status={:?}, thread_id={:?}",
                    plan_id,
                    phase_index + 1,
                    total_phases,
                    status,
                    thread_id
                );
                // Create phase progress data and update dashboard
                let progress = PhaseProgressData::new(
                    phase_index,
                    total_phases,
                    phase_name.clone(),
                    status,
                    tool_count,
                    last_tool.clone(),
                    last_file.clone(),
                );
                // Update phase progress - use thread_id if available, otherwise use plan_id as fallback
                let progress_key = thread_id.as_deref().unwrap_or(&plan_id);
                self.dashboard.update_phase_progress(progress_key, progress);
                // Emit StateChange for phase progress
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::DashboardState,
                        "Phase progress updated",
                        format!(
                            "plan_id: {}, phase: {}/{} ({}), status: {:?}, tools: {}, last_tool: {}{}",
                            plan_id,
                            phase_index + 1,
                            total_phases,
                            phase_name,
                            status,
                            tool_count,
                            last_tool,
                            last_file.as_ref().map(|f| format!(", file: {}", f)).unwrap_or_default()
                        ),
                    )),
                    thread_id.as_deref(),
                );
            }
            AppMessage::ThreadVerified {
                thread_id,
                verified_at,
            } => {
                // Log for terminal debugging
                tracing::info!(
                    "THREAD_VERIFIED: thread_id={}, verified_at={}",
                    thread_id,
                    verified_at
                );
                // Parse verified_at string and update dashboard state
                if let Ok(verified_dt) = chrono::DateTime::parse_from_rfc3339(&verified_at) {
                    self.dashboard.update_thread_verified(
                        &thread_id,
                        verified_dt.with_timezone(&chrono::Utc),
                    );
                } else {
                    // Fallback to current time if parsing fails
                    self.dashboard
                        .update_thread_verified(&thread_id, chrono::Utc::now());
                    tracing::warn!(
                        "Failed to parse verified_at timestamp '{}', using current time",
                        verified_at
                    );
                }
                // Emit StateChange for thread verification
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::DashboardState,
                        "Thread verified",
                        format!("thread_id: {}, verified_at: {}", thread_id, verified_at),
                    )),
                    Some(&thread_id),
                );
            }

            AppMessage::PendingQuestion {
                thread_id,
                request_id,
                question_data,
            } => {
                // Update thread status to Waiting with UserInput (so 'A' key can find it)
                use crate::models::dashboard::{ThreadStatus, WaitingFor};
                self.dashboard.update_thread_status(
                    &thread_id,
                    ThreadStatus::Waiting,
                    Some(WaitingFor::UserInput),
                );

                // Store the question data in dashboard state (with request_id for WebSocket response)
                self.dashboard.set_pending_question(&thread_id, request_id.clone(), question_data.clone());

                // Emit StateChange for pending question
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::DashboardState,
                        "Pending question stored",
                        format!(
                            "thread_id: {}, request_id: {}, questions: {}",
                            thread_id,
                            request_id,
                            question_data.questions.len()
                        ),
                    )),
                    Some(&thread_id),
                );
            }

            // =========================================================================
            // Unified Picker Messages
            // =========================================================================
            AppMessage::UnifiedPickerFoldersLoaded(items) => {
                // Cache for session
                self.picker_cache.set_folders(items.clone());
                // Update picker if visible
                if self.unified_picker.visible {
                    self.unified_picker.folders.set_items(items);
                    self.unified_picker.validate_selection();
                }
                self.mark_dirty();
            }
            AppMessage::UnifiedPickerFoldersFailed(error) => {
                if self.unified_picker.visible {
                    self.unified_picker.folders.set_error(error);
                }
                self.mark_dirty();
            }
            AppMessage::UnifiedPickerReposLoaded(items) => {
                // Cache for entire session (repos rarely change)
                self.picker_cache.set_repos(items.clone());
                // Update picker if visible
                if self.unified_picker.visible {
                    self.unified_picker.repos.set_items(items);
                    self.unified_picker.validate_selection();
                }
                self.mark_dirty();
            }
            AppMessage::UnifiedPickerReposFailed(error) => {
                if self.unified_picker.visible {
                    self.unified_picker.repos.set_error(error);
                }
                self.mark_dirty();
            }
            AppMessage::UnifiedPickerThreadsLoaded(items) => {
                // Cache with TTL (threads change more often)
                self.picker_cache.set_threads(items.clone());
                // Update picker if visible
                if self.unified_picker.visible {
                    self.unified_picker.threads.set_items(items);
                    self.unified_picker.validate_selection();
                }
                self.mark_dirty();
            }
            AppMessage::UnifiedPickerThreadsFailed(error) => {
                if self.unified_picker.visible {
                    self.unified_picker.threads.set_error(error);
                }
                self.mark_dirty();
            }
            AppMessage::UnifiedPickerCloneComplete { local_path, name, message } => {
                // Set the cloned repo as working directory
                let folder = crate::models::Folder {
                    name,
                    path: local_path,
                };
                self.selected_folder = Some(folder);

                // Close the picker
                self.unified_picker.finish_clone();
                self.unified_picker.close();

                // Clear textarea and set the message
                self.textarea.clear();
                self.textarea.set_content(&message);

                // Submit to create new thread with the message
                self.submit_input(crate::models::ThreadType::Programming);

                self.mark_dirty();
            }
            AppMessage::UnifiedPickerCloneFailed { error } => {
                self.unified_picker_clone_failed(error);
            }

            // =========================================================================
            // Sync Messages
            // =========================================================================
            AppMessage::TriggerSync => {
                // Set status to starting and spawn async sync task
                use crate::app::SyncStatus;
                use crate::conductor::ConductorClient;

                tracing::info!("TriggerSync received - setting status to Starting");
                self.sync_status = SyncStatus::Starting;
                self.mark_dirty(); // CRITICAL: Force UI redraw

                // Extract client config for creating a new client in the async task
                let base_url = self.client.base_url.clone();
                let auth_token = self.credentials.access_token.clone();
                let refresh_token = self.credentials.refresh_token.clone();
                let tx = self.message_tx.clone();

                tracing::info!("Spawning sync task with base_url: {}", base_url);
                tokio::spawn(async move {
                    tracing::info!("Sync task started");

                    // Send started message
                    let _ = tx.send(AppMessage::SyncStarted);

                    // Send progress message
                    let _ = tx.send(AppMessage::SyncProgress {
                        message: "Syncing tokens to VPS...".to_string(),
                    });

                    // Create a new client for the sync operation (needs &mut self)
                    let mut client = ConductorClient::with_url(&base_url);
                    if let Some(token) = auth_token {
                        client = client.with_auth(&token);
                    }
                    if let Some(refresh) = refresh_token {
                        client = client.with_refresh_token(&refresh);
                    }

                    tracing::info!("Calling sync_tokens...");
                    // Perform the actual sync
                    match client.sync_tokens("all").await {
                        Ok(result) => {
                            tracing::info!("Sync succeeded: {:?}", result.success);

                            // Extract verification results from sync response
                            let github_cli = if let Some(v) = result.verification {
                                tracing::info!("Using embedded verification from sync response");
                                v.github_cli_works.unwrap_or(false)
                            } else {
                                // Fallback: explicitly call verify_tokens()
                                tracing::info!("No embedded verification, calling verify_tokens()...");
                                let _ = tx.send(AppMessage::SyncProgress {
                                    message: "Verifying tokens on VPS...".to_string(),
                                });

                                match client.verify_tokens().await {
                                    Ok(verify_result) => {
                                        tracing::info!("Verify succeeded: github={}",
                                            verify_result.github_cli.authenticated);
                                        verify_result.github_cli.authenticated
                                    }
                                    Err(e) => {
                                        tracing::warn!("Verify failed, using sync success as fallback: {}", e);
                                        result.success
                                    }
                                }
                            };

                            let _ = tx.send(AppMessage::SyncComplete {
                                github_cli,
                            });
                        }
                        Err(e) => {
                            tracing::error!("Sync failed: {}", e);
                            let _ = tx.send(AppMessage::SyncFailed {
                                error: e.to_string(),
                            });
                        }
                    }
                });

                // Emit StateChange for sync triggered
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::SessionState,
                        "Sync triggered",
                        "starting",
                    )),
                    None,
                );
            }
            AppMessage::SyncStarted => {
                use crate::app::SyncStatus;
                tracing::info!("SyncStarted received");
                self.sync_status = SyncStatus::InProgress {
                    message: "Starting sync...".to_string(),
                };
                self.mark_dirty(); // Force UI redraw
                // Emit StateChange for sync started
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::SessionState,
                        "Sync started",
                        "in_progress",
                    )),
                    None,
                );
            }
            AppMessage::SyncProgress { message } => {
                use crate::app::SyncStatus;
                tracing::info!("SyncProgress received: {}", message);
                self.sync_status = SyncStatus::InProgress {
                    message: message.clone(),
                };
                self.mark_dirty(); // Force UI redraw
                // Emit StateChange for sync progress
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::SessionState,
                        "Sync progress",
                        &message,
                    )),
                    None,
                );
            }
            AppMessage::SyncComplete {
                github_cli,
            } => {
                use crate::app::SyncStatus;
                tracing::info!("SyncComplete received: github_cli={}", github_cli);
                self.sync_status = SyncStatus::Complete {
                    github_cli,
                };
                // Reset credential watcher backoff on success
                handle_sync_complete(&mut self.credential_watch_state);
                self.mark_dirty(); // Force UI redraw
                // Emit StateChange for sync complete
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::SessionState,
                        "Sync complete",
                        format!("github_cli: {}", github_cli),
                    )),
                    None,
                );
            }
            AppMessage::SyncFailed { error } => {
                use crate::app::SyncStatus;
                tracing::error!("SyncFailed received: {}", error);
                self.sync_status = SyncStatus::Failed {
                    error: error.clone(),
                };
                // Record failure in credential watcher backoff
                handle_sync_failed(&mut self.credential_watch_state, &error);
                self.mark_dirty(); // Force UI redraw
                // Emit StateChange for sync failed
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::SessionState,
                        "Sync failed",
                        &error,
                    )),
                    None,
                );
            }
            // =========================================================================
            // Browse List Messages
            // =========================================================================
            AppMessage::BrowseListThreadsLoaded {
                threads,
                offset,
                has_more,
            } => {
                // Only update if we're still on the BrowseList screen in Threads mode
                if self.screen == crate::app::Screen::BrowseList
                    && self.browse_list.mode == crate::app::BrowseListMode::Threads
                {
                    if offset == 0 {
                        // Initial load - replace items
                        self.browse_list.threads = threads;
                    } else {
                        // Pagination - append items
                        self.browse_list.threads.extend(threads);
                    }
                    self.browse_list.total_count = self.browse_list.threads.len();
                    self.browse_list.has_more = has_more;
                    self.browse_list.loading = false;
                    self.browse_list.searching = false;
                    self.mark_dirty();
                }
            }
            AppMessage::BrowseListReposLoaded {
                repos,
                offset,
                has_more,
            } => {
                // Only update if we're still on the BrowseList screen in Repos mode
                if self.screen == crate::app::Screen::BrowseList
                    && self.browse_list.mode == crate::app::BrowseListMode::Repos
                {
                    if offset == 0 {
                        // Initial load - store in both all_repos (master) and repos (filtered view)
                        self.browse_list.all_repos = repos.clone();
                        self.browse_list.repos = repos;
                    } else {
                        // Pagination - append items
                        self.browse_list.all_repos.extend(repos.clone());
                        self.browse_list.repos.extend(repos);
                    }
                    self.browse_list.total_count = self.browse_list.repos.len();
                    self.browse_list.has_more = has_more;
                    self.browse_list.loading = false;
                    self.browse_list.searching = false;
                    self.mark_dirty();
                }
            }
            AppMessage::BrowseListError(error) => {
                if self.screen == crate::app::Screen::BrowseList {
                    self.browse_list.error = Some(error);
                    self.browse_list.loading = false;
                    self.browse_list.searching = false;
                    self.mark_dirty();
                }
            }
            AppMessage::BrowseListSearchDebounced { query } => {
                // Execute debounced search if query still matches pending
                if self.screen == crate::app::Screen::BrowseList {
                    self.browse_list_execute_search(query);
                }
            }
            AppMessage::BrowseListCloneComplete { local_path, name } => {
                // Clone succeeded - set folder and close browse list
                if self.screen == crate::app::Screen::BrowseList {
                    self.browse_list_clone_complete(local_path, name);
                }
            }
            AppMessage::BrowseListCloneFailed { error } => {
                // Clone failed - show error
                if self.screen == crate::app::Screen::BrowseList {
                    self.browse_list_clone_failed(error);
                }
            }

            // =========================================================================
            // Credential Auto-Sync Messages
            // =========================================================================
            AppMessage::CredentialFileChanged { path } => {
                handle_credential_change(
                    &mut self.credential_watch_state,
                    &mut self.credential_debouncer,
                    &self.message_tx,
                    &format!("file: {}", path),
                );
            }
            AppMessage::CredentialDebounceExpired => {
                handle_debounce_expired(
                    &mut self.credential_watch_state,
                    &mut self.credential_debouncer,
                    &self.message_tx,
                );
            }

            // =========================================================================
            // Claude CLI Login Messages
            // =========================================================================
            AppMessage::ClaudeLoginRequired {
                request_id,
                auth_url,
                auto_open,
            } => {
                tracing::info!(
                    "Claude CLI login required: request_id={}, auto_open={}",
                    request_id, auto_open
                );

                // Auto-open browser if requested
                let browser_opened = if auto_open {
                    match open::that(&auth_url) {
                        Ok(()) => {
                            tracing::info!("Browser opened for auth URL");
                            true
                        }
                        Err(e) => {
                            tracing::warn!("Failed to open browser: {}", e);
                            // Show specific error state instead of generic message
                            use crate::view_state::ClaudeLoginState;
                            self.dashboard.show_claude_login(request_id.clone(), auth_url.clone(), false);
                            self.dashboard.update_claude_login_state(
                                ClaudeLoginState::BrowserOpenFailed {
                                    auth_url: auth_url.clone(),
                                    error: format!("Could not open browser: {}", e),
                                }
                            );
                            return;
                        }
                    }
                } else {
                    false
                };

                // Show the Claude login overlay
                self.dashboard
                    .show_claude_login(request_id, auth_url, browser_opened);
            }
            AppMessage::ClaudeLoginVerificationResult {
                request_id,
                success,
                account_email,
                error,
            } => {
                use crate::view_state::ClaudeLoginState;

                tracing::info!(
                    "Claude CLI login verification: request_id={}, success={}, email={:?}, error={:?}",
                    request_id, success, account_email, error
                );

                // Only update if this is for the current login dialog
                if self.dashboard.claude_login_request_id() == Some(&request_id) {
                    if success {
                        let now = std::time::Instant::now();
                        self.dashboard.update_claude_login_state(
                            ClaudeLoginState::VerificationSuccess {
                                email: account_email.unwrap_or_else(|| "Unknown".to_string()),
                                success_time: now,
                            },
                        );
                        // Schedule auto-close after 1.5 seconds
                        self.claude_login_auto_close = Some(now + std::time::Duration::from_millis(1500));
                    } else {
                        self.dashboard.update_claude_login_state(
                            ClaudeLoginState::VerificationFailed {
                                error: error.unwrap_or_else(|| "Unknown error".to_string()),
                            },
                        );
                    }
                }
            }

            // =========================================================================
            // Claude CLI Token Messages (VPS setup-token flow)
            // =========================================================================
            AppMessage::ClaudeAuthTokenRequest {
                request_id,
                message,
            } => {
                tracing::info!(
                    "Claude CLI auth token request: request_id={}, message={}",
                    request_id, message
                );

                let tx = self.message_tx.clone();
                let req_id = request_id.clone();

                // Clone for unwrap_or_else closure
                let tx_panic = tx.clone();
                let req_id_panic = req_id.clone();

                // Use tokio::spawn_blocking for CPU-bound blocking operation
                tokio::spawn(async move {
                    tokio::task::spawn_blocking(move || {
                        tracing::info!("Running claude setup-token for request_id={}", req_id);

                        match crate::setup::run_claude_setup_token() {
                            Ok(result) if result.success => {
                                if let Some(token) = result.token {
                                    if token.is_empty() {
                                        tracing::error!("Claude setup-token returned empty token");
                                        let _ = tx.send(AppMessage::ClaudeAuthTokenFailed {
                                            request_id: req_id,
                                            error: "Authentication succeeded but token is empty".to_string(),
                                        });
                                        return;
                                    }

                                    tracing::info!("Claude setup-token succeeded: token_length={}", token.len());
                                    let _ = tx.send(AppMessage::ClaudeAuthTokenCaptured {
                                        request_id: req_id,
                                        token,
                                    });
                                } else {
                                    tracing::error!("Claude setup-token succeeded but no token captured");
                                    let _ = tx.send(AppMessage::ClaudeAuthTokenFailed {
                                        request_id: req_id,
                                        error: "Token capture succeeded but token is empty".to_string(),
                                    });
                                }
                            }
                            Ok(result) => {
                                let error = result.error.unwrap_or_else(|| "Authentication failed without details".to_string());
                                tracing::error!("Claude setup-token failed: {}", error);
                                let _ = tx.send(AppMessage::ClaudeAuthTokenFailed {
                                    request_id: req_id,
                                    error,
                                });
                            }
                            Err(e) => {
                                tracing::error!("Claude setup-token error: {}", e);
                                let _ = tx.send(AppMessage::ClaudeAuthTokenFailed {
                                    request_id: req_id,
                                    error: e.to_string(),
                                });
                            }
                        }
                    })
                    .await
                    .unwrap_or_else(|e| {
                        tracing::error!("Setup-token task panicked: {}", e);
                        let _ = tx_panic.send(AppMessage::ClaudeAuthTokenFailed {
                            request_id: req_id_panic,
                            error: "Setup-token task failed unexpectedly".to_string(),
                        });
                    });
                });
            }
            AppMessage::ClaudeAuthTokenCaptured {
                request_id,
                token,
            } => {
                tracing::info!(
                    "Claude CLI auth token captured: request_id={}, token_length={}",
                    request_id, token.len()
                );

                // Send token to backend via WebSocket
                use crate::websocket::{WsClaudeAuthTokenResponse, WsOutgoingMessage};
                let response = WsClaudeAuthTokenResponse::new(request_id.clone(), token);
                if let Some(ref sender) = self.ws_sender {
                    if let Err(e) = sender.try_send(WsOutgoingMessage::ClaudeAuthTokenResponse(response)) {
                        tracing::error!("Failed to send Claude auth token: {}", e);

                        // Show error to user
                        let _ = self.message_tx.send(AppMessage::ClaudeAuthTokenFailed {
                            request_id,
                            error: "Failed to send token to server - connection may be lost".to_string(),
                        });
                    }
                } else {
                    tracing::error!("No WebSocket connection available to send Claude auth token");

                    // Show error to user
                    let _ = self.message_tx.send(AppMessage::ClaudeAuthTokenFailed {
                        request_id,
                        error: "No connection to server - please check your connection".to_string(),
                    });
                }
            }
            AppMessage::ClaudeAuthTokenFailed {
                request_id,
                error,
            } => {
                tracing::error!(
                    "Claude CLI auth token capture failed: request_id={}, error={}",
                    request_id, error
                );
                // TODO: Could show an error notification to the user
            }
            AppMessage::ClaudeAuthTokenStored {
                request_id,
                success,
                error,
            } => {
                if success {
                    tracing::info!(
                        "Claude CLI auth token stored successfully: request_id={}",
                        request_id
                    );
                } else {
                    tracing::error!(
                        "Claude CLI auth token storage failed: request_id={}, error={:?}",
                        request_id, error
                    );
                }
            }
            // =========================================================================
            // Steering Messages (soft-interrupt flow)
            // =========================================================================
            AppMessage::SteeringQueued { thread_id } => {
                if let Some(ref mut qs) = self.queued_steering {
                    if qs.thread_id == thread_id {
                        qs.transition_to(crate::models::SteeringMessageState::Sent);
                        self.mark_dirty();
                    }
                }
            }
            AppMessage::SteeringInterrupting { thread_id } => {
                if let Some(ref mut qs) = self.queued_steering {
                    if qs.thread_id == thread_id {
                        qs.transition_to(crate::models::SteeringMessageState::Interrupting);
                        self.mark_dirty();
                    }
                }
            }
            AppMessage::SteeringResuming { thread_id } => {
                if let Some(ref mut qs) = self.queued_steering {
                    if qs.thread_id == thread_id {
                        qs.transition_to(crate::models::SteeringMessageState::Resuming);
                        self.mark_dirty();
                    }
                }
            }
            AppMessage::SteeringCompleted {
                thread_id,
                duration_ms: _,
            } => {
                if let Some(qs) = self.queued_steering.take() {
                    if qs.thread_id == thread_id {
                        // Promote the queued steering to a visible message
                        self.promote_steering_to_message(&qs);
                    } else {
                        // Put it back if thread_id doesn't match
                        self.queued_steering = Some(qs);
                    }
                }
            }
            AppMessage::SteeringFailed { thread_id, error } => {
                if let Some(ref mut qs) = self.queued_steering {
                    if qs.thread_id == thread_id {
                        qs.transition_to(crate::models::SteeringMessageState::Failed(error));
                        self.mark_dirty();
                    }
                }
            }
        }
    }
}
