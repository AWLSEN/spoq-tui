//! Message handling for the App.

use crate::debug::{
    DebugEventKind, ErrorData, ErrorSource, StateChangeData, StateType,
};

use super::{emit_debug, log_thread_update, truncate_for_debug, App, AppMessage};

impl App {
    /// Handle an incoming async message
    pub fn handle_message(&mut self, msg: AppMessage) {
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
            }
            AppMessage::StreamError { thread_id: _, error } => {
                // Reset stream statistics on error
                self.stream_start_time = None;
                self.last_event_time = None;
                self.cumulative_token_count = 0;

                // Emit Error debug event
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::Error(ErrorData::new(ErrorSource::AppState, &error)),
                    None,
                );
                self.stream_error = Some(error);
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
            AppMessage::MessagesLoaded { thread_id, messages } => {
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
                    self.session_state
                        .set_pending_permission(PermissionRequest {
                            permission_id: permission_id.clone(),
                            tool_name: tool_name.clone(),
                            description,
                            context: None, // Context will be extracted from tool_input in UI
                            tool_input,
                        });
                    // Emit StateChange for pending permission
                    emit_debug(
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
            AppMessage::ToolStarted {
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
                    self.active_thread_id.as_deref(),
                );
                // Also add tool event inline to the streaming message
                if let Some(thread_id) = &self.active_thread_id {
                    self.cache
                        .start_tool_in_message(thread_id, tool_call_id, tool_name);
                }
            }
            AppMessage::ToolExecuting {
                tool_call_id,
                display_name,
            } => {
                // Update tool to executing state with display info
                self.tool_tracker
                    .set_tool_executing(&tool_call_id, display_name.clone());
                // Update the display_name in the message segments
                if let Some(thread_id) = &self.active_thread_id {
                    self.cache
                        .set_tool_display_name(thread_id, &tool_call_id, display_name.clone());
                }
                // Emit StateChange for tool executing
                emit_debug(
                    &self.debug_tx,
                    DebugEventKind::StateChange(StateChangeData::new(
                        StateType::ToolTracker,
                        "Tool executing",
                        format!("display: {}", truncate_for_debug(&display_name, 40)),
                    )),
                    self.active_thread_id.as_deref(),
                );
            }
            AppMessage::ToolCompleted {
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
                    self.active_thread_id.as_deref(),
                );
                // Also update the inline tool event in the streaming message
                if let Some(thread_id) = &self.active_thread_id {
                    // Store the result content in the tool event
                    self.cache
                        .set_tool_result(thread_id, &tool_call_id, &result, !success);
                    if success {
                        self.cache.complete_tool_in_message(thread_id, &tool_call_id);
                    } else {
                        self.cache.fail_tool_in_message(thread_id, &tool_call_id);
                    }
                }
            }
            AppMessage::ToolArgumentChunk { tool_call_id, chunk } => {
                // Append argument chunk to the tool event for live display
                if let Some(thread_id) = &self.active_thread_id {
                    self.cache
                        .append_tool_argument(thread_id, &tool_call_id, &chunk);
                }
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
                let updated = self
                    .cache
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
        }
    }
}
