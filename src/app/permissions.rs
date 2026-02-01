//! Permission handling methods for the App.

use std::sync::Arc;
use std::time::Duration;

use crate::models::dashboard::{ThreadStatus, WaitingFor};
use crate::models::PermissionMode;
use crate::state::session::AskUserQuestionState;
use crate::ui::input::parse_ask_user_question;
use crate::websocket::{
    WsCancelPermission, WsCommandResponse, WsCommandResult, WsConnectionState, WsOutgoingMessage,
    WsPermissionData,
};
use tracing::{debug, error, info, warn};

use super::App;

/// Maximum elapsed time before considering a permission expired (server times out at 300s)
const PERMISSION_TIMEOUT_SECS: u64 = 295;

/// Retry delay for WebSocket send failures
const WS_RETRY_DELAY_MS: u64 = 500;

/// Result of sending a permission response
#[derive(Debug)]
pub enum PermissionResponseResult {
    /// Successfully sent via WebSocket
    SentViaWebSocket,
    /// Sent via HTTP fallback
    SentViaHttpFallback,
    /// Permission expired before sending
    Expired,
    /// Failed to send (connection lost and no HTTP fallback)
    Failed(String),
}

impl App {
    /// Check if a pending permission has expired
    ///
    /// Returns true if the permission was received more than PERMISSION_TIMEOUT_SECS ago.
    /// Searches across all threads in the dashboard to find the permission by ID.
    fn is_permission_expired(&self, permission_id: &str) -> bool {
        if let Some((_, perm)) = self.dashboard.find_permission_by_id(permission_id) {
            return perm.received_at.elapsed().as_secs() >= PERMISSION_TIMEOUT_SECS;
        }
        false
    }

    /// Send a permission response via WebSocket
    ///
    /// Constructs a `WsCommandResponse` and sends it through the WebSocket channel.
    /// Returns a result indicating success or failure.
    fn send_ws_permission_response(&self, request_id: &str, allowed: bool) -> Result<(), String> {
        let sender = match &self.ws_sender {
            Some(s) => s,
            None => return Err("WebSocket sender not available".to_string()),
        };

        // Check if WebSocket is connected
        if self.ws_connection_state != WsConnectionState::Connected {
            return Err("WebSocket not connected".to_string());
        }

        let response = WsCommandResponse {
            type_: "command_response".to_string(),
            request_id: request_id.to_string(),
            result: WsCommandResult {
                status: "success".to_string(),
                data: WsPermissionData {
                    allowed,
                    message: None,
                },
            },
        };

        // Try to send - this is a non-blocking channel send
        match sender.try_send(WsOutgoingMessage::CommandResponse(response)) {
            Ok(()) => {
                debug!(
                    "Sent permission response via WebSocket: {} -> {}",
                    request_id, allowed
                );
                Ok(())
            }
            Err(e) => Err(format!("Failed to send via WebSocket: {}", e)),
        }
    }

    /// Send permission response with retry and fallback logic
    ///
    /// This method:
    /// 1. Checks if permission has expired (>50s elapsed)
    /// 2. Tries to send via WebSocket
    /// 3. If WS fails, retries once after 500ms
    /// 4. If still fails, falls back to HTTP if available
    fn send_permission_response(
        &mut self,
        permission_id: &str,
        allowed: bool,
    ) -> PermissionResponseResult {
        // Check if permission has expired
        if self.is_permission_expired(permission_id) {
            warn!(
                "Permission {} expired before response could be sent",
                permission_id
            );
            return PermissionResponseResult::Expired;
        }

        // Try WebSocket first
        match self.send_ws_permission_response(permission_id, allowed) {
            Ok(()) => return PermissionResponseResult::SentViaWebSocket,
            Err(e) => {
                debug!("First WebSocket send attempt failed: {}", e);
            }
        }

        // WebSocket send failed - we need to handle retry/fallback asynchronously
        // Since we can't do async operations directly here, we spawn a task
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let perm_id = permission_id.to_string();
            let ws_sender = self.ws_sender.clone();
            let ws_state = self.ws_connection_state.clone();
            let client = Arc::clone(&self.client);

            handle.spawn(async move {
                // Retry WebSocket after delay
                tokio::time::sleep(Duration::from_millis(WS_RETRY_DELAY_MS)).await;

                if let Some(sender) = ws_sender {
                    if ws_state == WsConnectionState::Connected {
                        let response = WsCommandResponse {
                            type_: "command_response".to_string(),
                            request_id: perm_id.clone(),
                            result: WsCommandResult {
                                status: "success".to_string(),
                                data: WsPermissionData {
                                    allowed,
                                    message: None,
                                },
                            },
                        };

                        if sender
                            .send(WsOutgoingMessage::CommandResponse(response))
                            .await
                            .is_ok()
                        {
                            debug!("Permission response sent via WebSocket on retry");
                            return;
                        }
                    }
                }

                // Fall back to HTTP
                debug!("Falling back to HTTP for permission response");
                if let Err(e) = client.respond_to_permission(&perm_id, allowed).await {
                    error!(
                        "Failed to send permission response via HTTP fallback: {:?}",
                        e
                    );
                }
            });

            // We've spawned a retry/fallback task
            PermissionResponseResult::SentViaHttpFallback
        } else {
            // No runtime available (e.g., in tests without async context)
            PermissionResponseResult::Failed("No runtime available for retry/fallback".to_string())
        }
    }

    /// Approve a pending permission (user pressed 'y')
    pub fn approve_permission(&mut self, permission_id: &str) {
        let result = self.send_permission_response(permission_id, true);

        match result {
            PermissionResponseResult::SentViaWebSocket => {
                debug!("Permission {} approved via WebSocket", permission_id);
            }
            PermissionResponseResult::SentViaHttpFallback => {
                debug!(
                    "Permission {} approval sent via HTTP fallback",
                    permission_id
                );
            }
            PermissionResponseResult::Expired => {
                warn!("Permission {} expired - could not approve", permission_id);
                // Could set an error notification here if needed
            }
            PermissionResponseResult::Failed(e) => {
                error!("Failed to approve permission {}: {}", permission_id, e);
            }
        }

        self.cleanup_question_response(permission_id, None);
    }

    /// Deny a pending permission (user pressed 'n')
    pub fn deny_permission(&mut self, permission_id: &str) {
        let result = self.send_permission_response(permission_id, false);

        match result {
            PermissionResponseResult::SentViaWebSocket => {
                debug!("Permission {} denied via WebSocket", permission_id);
            }
            PermissionResponseResult::SentViaHttpFallback => {
                debug!("Permission {} denial sent via HTTP fallback", permission_id);
            }
            PermissionResponseResult::Expired => {
                warn!("Permission {} expired - could not deny", permission_id);
            }
            PermissionResponseResult::Failed(e) => {
                error!("Failed to deny permission {}: {}", permission_id, e);
            }
        }

        self.cleanup_question_response(permission_id, None);
    }

    /// Cancel a pending permission (user pressed Shift+Escape)
    pub fn cancel_permission(&mut self, permission_id: &str) {
        // Send cancel message via WebSocket
        if let Some(ref sender) = self.ws_sender {
            let cancel_msg = WsCancelPermission::new(permission_id.to_string());
            debug!("Cancelling permission {} via WebSocket", permission_id);
            let _ = sender.try_send(WsOutgoingMessage::CancelPermission(cancel_msg));
        } else {
            warn!("No WebSocket sender available for cancel");
        }

        self.cleanup_question_response(permission_id, None);
    }

    /// Complete cleanup after an AskUserQuestion response is sent.
    ///
    /// Clears all related state: both storage maps, waiting_for, both question
    /// states (session-level and dashboard-level), and the dashboard overlay.
    /// Safe to call even when some state doesn't exist (e.g., no overlay open).
    ///
    /// `thread_id_hint` is used when the caller already knows the thread_id
    /// (e.g., dashboard overlay path). Falls back to looking up via permission_id.
    fn cleanup_question_response(&mut self, permission_id: &str, thread_id_hint: Option<&str>) {
        let thread_id_from_perm = self.dashboard.clear_permission_by_id(permission_id);
        let thread_id = thread_id_hint.map(|s| s.to_string()).or(thread_id_from_perm);

        if let Some(ref tid) = thread_id {
            self.dashboard.clear_pending_question(tid);
            self.dashboard.update_thread_status(tid, ThreadStatus::Running, None);
        }

        self.question_state.reset();
        self.dashboard.collapse_overlay();
        self.mark_dirty();
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
        info!("handle_permission_key called with key: '{}'", key);

        // Check top thread type - if UserInput, Y/N/A should do nothing
        // (A key will be used to open dialog instead)
        if matches!(key, 'y' | 'Y' | 'n' | 'N' | 'a' | 'A') {
            if let Some((_, wf)) = self.dashboard.get_top_needs_action_thread() {
                if matches!(wf, WaitingFor::UserInput) {
                    info!("Ignoring Y/N/A key because top thread is UserInput");
                    return false; // Ignore Y/N/A when top thread is UserInput
                }
            }
        }

        // Find a pending permission from the top thread needing action,
        // or fall back to searching all threads
        let perm_info = if let Some((thread_id, _)) = self.dashboard.get_top_needs_action_thread() {
            // Try to get permission for the top thread
            self.dashboard
                .get_pending_permission(&thread_id)
                .map(|p| (p.permission_id.clone(), p.tool_name.clone()))
        } else {
            // No top thread, search all pending permissions
            self.dashboard
                .find_permission_by_id("")
                .map(|(_, p)| (p.permission_id.clone(), p.tool_name.clone()))
        };

        // Also check all pending permissions if no specific thread permission found
        let perm_info = perm_info.or_else(|| {
            // Search all pending permissions
            for (_, perm) in self.dashboard.pending_permissions_iter() {
                return Some((perm.permission_id.clone(), perm.tool_name.clone()));
            }
            None
        });

        if let Some((permission_id, tool_name)) = perm_info {
            info!(
                "Pending permission found: {} for tool {}",
                permission_id, tool_name
            );
            match key {
                'y' | 'Y' => {
                    info!("User pressed 'y' - approving permission");
                    self.approve_permission(&permission_id);
                    true
                }
                'a' | 'A' => {
                    info!("User pressed 'a' - allowing tool always");
                    self.allow_tool_always(&tool_name, &permission_id);
                    true
                }
                'n' | 'N' => {
                    info!("User pressed 'n' - denying permission");
                    self.deny_permission(&permission_id);
                    true
                }
                _ => {
                    // Consume all non-permission keys when permission modal is active.
                    // This prevents any fallback handling that might insert chars into
                    // the textarea, which would break subsequent Y/N/A key handling.
                    info!("Key '{}' consumed (not a permission key)", key);
                    true
                }
            }
        } else {
            // No permission found - check for plan approval on active thread
            info!("No pending permission found, checking for plan approval");
            self.handle_plan_approval_key(key)
        }
    }

    /// Handle a plan approval key press ('y' or 'n')
    ///
    /// Returns true if a plan approval was handled, false if no pending plan approval.
    /// This is called when no permission is pending but y/n was pressed.
    ///
    /// Note: If the plan originated from an ExitPlanMode permission request,
    /// we send a permission_response instead of plan_approval_response.
    fn handle_plan_approval_key(&mut self, key: char) -> bool {
        // Get active thread ID (conversation view)
        let thread_id = match &self.active_thread_id {
            Some(id) => id.clone(),
            None => {
                info!("No active thread for plan approval");
                return false;
            }
        };

        // Check if there's a plan approval pending for this thread
        let request_id = match self.dashboard.get_plan_request_id(&thread_id) {
            Some(id) => id.to_string(),
            None => {
                info!("No plan approval pending for thread {}", thread_id);
                return false;
            }
        };

        // Check if this plan came from a permission request (ExitPlanMode)
        let from_permission = self.dashboard.is_plan_from_permission(&thread_id);

        match key {
            'y' | 'Y' => {
                info!(
                    "User pressed 'y' - approving plan {} (from_permission={})",
                    request_id, from_permission
                );
                // Send appropriate response type based on origin
                // If from_permission, send permission_response; otherwise plan_approval_response
                let sent = if from_permission {
                    self.send_permission_response_for_thread(&request_id, true)
                } else {
                    self.send_plan_approval_response(&request_id, true)
                };
                if sent {
                    self.dashboard.remove_plan_request(&thread_id);
                    self.dashboard.set_thread_planning(&thread_id, false);
                    // Switch to Execution mode after plan approval
                    self.permission_mode = PermissionMode::Execution;
                    self.thread_mode_sync.request_mode_change(
                        thread_id.clone(),
                        PermissionMode::Execution,
                    );
                }
                sent
            }
            'n' | 'N' => {
                info!(
                    "User pressed 'n' - rejecting plan {} (from_permission={})",
                    request_id, from_permission
                );
                // Send appropriate response type based on origin
                // If from_permission, send permission_response; otherwise plan_approval_response
                let sent = if from_permission {
                    self.send_permission_response_for_thread(&request_id, false)
                } else {
                    self.send_plan_approval_response(&request_id, false)
                };
                if sent {
                    self.dashboard.remove_plan_request(&thread_id);
                    self.dashboard.set_thread_planning(&thread_id, false);
                }
                sent
            }
            _ => {
                info!("Key '{}' not recognized for plan approval", key);
                false
            }
        }
    }

    /// Submit plan feedback text as a rejection with a message.
    ///
    /// Extracts feedback text from the plan approval state, sends a rejection
    /// response that includes the feedback, and cleans up the state.
    pub fn submit_plan_feedback(&mut self) {
        let thread_id = match &self.active_thread_id {
            Some(id) => id.clone(),
            None => return,
        };

        let feedback = self.dashboard.get_plan_approval_state(&thread_id)
            .map(|s| s.feedback_text.clone())
            .unwrap_or_default();

        if feedback.is_empty() {
            // Nothing to submit
            return;
        }

        let request_id = match self.dashboard.get_plan_request_id(&thread_id) {
            Some(id) => id.to_string(),
            None => return,
        };
        let from_permission = self.dashboard.is_plan_from_permission(&thread_id);

        let sent = if from_permission {
            self.send_permission_response_with_message(&request_id, false, Some(feedback))
        } else {
            self.send_plan_approval_response_with_message(&request_id, false, Some(feedback))
        };

        if sent {
            self.dashboard.remove_plan_request(&thread_id);
            // Keep planning mode — Claude will revise and call ExitPlanMode again
        }

        if let Some(state) = self.dashboard.get_plan_approval_state_mut(&thread_id) {
            state.feedback_active = false;
            state.feedback_text.clear();
        }
        self.mark_dirty();
    }

    // ========================================================================
    // AskUserQuestion Navigation Methods
    // ========================================================================

    /// Find an AskUserQuestion permission from the top needs-action thread
    ///
    /// Returns the permission if found, None otherwise.
    fn find_ask_user_question_permission(&self) -> Option<&crate::state::PermissionRequest> {
        // Only check the TOP needs-action thread
        if let Some((thread_id, _)) = self.dashboard.get_top_needs_action_thread() {
            if let Some(perm) = self.dashboard.get_pending_permission(&thread_id) {
                if perm.tool_name == "AskUserQuestion" {
                    return Some(perm);
                }
            }
        }
        None
    }

    /// Check if there is a pending AskUserQuestion permission on the top needs-action thread
    pub fn is_ask_user_question_pending(&self) -> bool {
        // Only check the TOP needs-action thread
        if let Some((thread_id, _)) = self.dashboard.get_top_needs_action_thread() {
            if let Some(perm) = self.dashboard.get_pending_permission(&thread_id) {
                if perm.tool_name == "AskUserQuestion" {
                    if let Some(ref tool_input) = perm.tool_input {
                        return parse_ask_user_question(tool_input).is_some();
                    }
                }
            }
        }
        false
    }

    /// Initialize question state from the pending AskUserQuestion permission
    pub fn init_question_state(&mut self) {
        // We need to find the AskUserQuestion permission and clone the data we need
        // because we can't hold a reference while mutating self.question_state
        let question_data = self.find_ask_user_question_permission().and_then(|perm| {
            perm.tool_input
                .as_ref()
                .and_then(|ti| parse_ask_user_question(ti))
        });

        if let Some(data) = question_data {
            self.question_state = AskUserQuestionState::from_data(&data);
            self.mark_dirty();
        }
    }

    /// Get the number of questions in the pending AskUserQuestion
    fn get_question_count(&self) -> usize {
        if let Some(perm) = self.find_ask_user_question_permission() {
            if let Some(ref tool_input) = perm.tool_input {
                if let Some(data) = parse_ask_user_question(tool_input) {
                    return data.questions.len();
                }
            }
        }
        0
    }

    /// Get the number of options for the current question
    fn get_current_option_count(&self) -> usize {
        if let Some(perm) = self.find_ask_user_question_permission() {
            if let Some(ref tool_input) = perm.tool_input {
                if let Some(data) = parse_ask_user_question(tool_input) {
                    if let Some(question) = data.questions.get(self.question_state.tab_index) {
                        return question.options.len();
                    }
                }
            }
        }
        0
    }

    /// Check if the current question is multi-select
    fn is_current_question_multi_select(&self) -> bool {
        if let Some(perm) = self.find_ask_user_question_permission() {
            if let Some(ref tool_input) = perm.tool_input {
                if let Some(data) = parse_ask_user_question(tool_input) {
                    if let Some(question) = data.questions.get(self.question_state.tab_index) {
                        return question.multi_select;
                    }
                }
            }
        }
        false
    }

    /// Move to the next question tab
    pub fn question_next_tab(&mut self) {
        let count = self.get_question_count();
        if count > 1 {
            self.question_state.next_tab(count);
            self.mark_dirty();
            debug!("Moved to question tab {}", self.question_state.tab_index);
        }
    }

    /// Move to the previous option in the current question
    pub fn question_prev_option(&mut self) {
        let option_count = self.get_current_option_count();

        if let Some(current) = self.question_state.current_selection() {
            if current > 0 {
                self.question_state.set_current_selection(Some(current - 1));
            } else {
                // Wrap to "Other" (None)
                self.question_state.set_current_selection(None);
            }
        } else {
            // Currently on "Other", move to last option
            if option_count > 0 {
                self.question_state
                    .set_current_selection(Some(option_count - 1));
            }
        }
        self.mark_dirty();
        debug!(
            "Selection now: {:?}",
            self.question_state.current_selection()
        );
    }

    /// Move to the next option in the current question
    pub fn question_next_option(&mut self) {
        let option_count = self.get_current_option_count();

        if let Some(current) = self.question_state.current_selection() {
            if current < option_count - 1 {
                self.question_state.set_current_selection(Some(current + 1));
            } else {
                // Wrap to "Other" (None)
                self.question_state.set_current_selection(None);
            }
        } else {
            // Currently on "Other", wrap to first option
            self.question_state.set_current_selection(Some(0));
        }
        self.mark_dirty();
        debug!(
            "Selection now: {:?}",
            self.question_state.current_selection()
        );
    }

    /// Toggle the current option in multi-select mode
    pub fn question_toggle_option(&mut self) {
        if self.is_current_question_multi_select() {
            if let Some(idx) = self.question_state.current_selection() {
                self.question_state.toggle_multi_selection(idx);
                self.mark_dirty();
                debug!("Toggled option {}", idx);
            }
        }
    }

    /// Handle Enter key in question UI
    ///
    /// For single questions: submits immediately
    /// For multiple questions: marks current as answered and advances to next tab
    ///                         Only submits when all questions are answered
    ///
    /// Returns true if a response was sent
    pub fn question_confirm(&mut self) -> bool {
        let num_questions = self.get_question_count();

        // Check if "Other" is selected and not in text input mode
        if self.question_state.current_selection().is_none() && !self.question_state.other_active {
            // Activate "Other" text input mode
            self.question_state.other_active = true;
            self.mark_dirty();
            debug!("Activated 'Other' text input mode");
            return false;
        }

        // If in "Other" text input mode, validate and mark answered
        if self.question_state.other_active {
            let other_text = self.question_state.current_other_text().to_string();
            if other_text.is_empty() {
                // Don't submit/advance with empty "Other" text
                return false;
            }
            // Deactivate "Other" mode as we're confirming this answer
            self.question_state.other_active = false;
        }

        // For single question, submit immediately
        if num_questions == 1 {
            return self.submit_question_answer();
        }

        // Multiple questions: mark current as answered and advance
        self.question_state.mark_current_answered();
        debug!(
            "Marked question {} as answered",
            self.question_state.tab_index
        );

        // Check if all questions are now answered
        if self.question_state.all_answered() {
            debug!("All questions answered, submitting");
            return self.submit_question_answer();
        }

        // Advance to next unanswered question
        if self
            .question_state
            .advance_to_next_unanswered(num_questions)
        {
            debug!("Advanced to question {}", self.question_state.tab_index);
        }

        false
    }

    /// Cancel "Other" text input mode
    pub fn question_cancel_other(&mut self) {
        if self.question_state.other_active {
            self.question_state.other_active = false;
            // Clear the other text
            if let Some(text) = self
                .question_state
                .other_texts
                .get_mut(self.question_state.tab_index)
            {
                text.clear();
            }
            self.mark_dirty();
            debug!("Cancelled 'Other' text input mode");
        }
    }

    /// Handle character input for "Other" text
    pub fn question_type_char(&mut self, c: char) {
        if self.question_state.other_active {
            self.question_state.push_other_char(c);
            self.mark_dirty();
        }
    }

    /// Handle backspace for "Other" text
    pub fn question_backspace(&mut self) {
        if self.question_state.other_active {
            self.question_state.pop_other_char();
            self.mark_dirty();
        }
    }

    /// Submit the question answer via WebSocket
    fn submit_question_answer(&mut self) -> bool {
        // Find the AskUserQuestion permission and extract data we need
        // We need to clone the data because we can't hold a reference while mutating self
        let perm_data = self.find_ask_user_question_permission().map(|perm| {
            (
                perm.permission_id.clone(),
                perm.tool_input.clone(),
            )
        });

        let (permission_id, tool_input) = match perm_data {
            Some((pid, Some(ti))) => (pid, ti),
            _ => return false,
        };

        let data = match parse_ask_user_question(&tool_input) {
            Some(d) => d,
            None => return false,
        };

        // Build the answers map
        let mut answers: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();

        for (i, question) in data.questions.iter().enumerate() {
            let answer = if question.multi_select {
                // Collect all selected options for multi-select
                let selected: Vec<String> = question
                    .options
                    .iter()
                    .enumerate()
                    .filter(|(opt_idx, _)| {
                        self.question_state
                            .multi_selections
                            .get(i)
                            .map(|s: &Vec<bool>| s.get(*opt_idx).copied().unwrap_or(false))
                            .unwrap_or(false)
                    })
                    .map(|(_, opt)| opt.label.clone())
                    .collect();

                // Also check if "Other" has text for this question
                let other_text = self
                    .question_state
                    .other_texts
                    .get(i)
                    .cloned()
                    .unwrap_or_default();
                if !other_text.is_empty() {
                    let mut with_other = selected;
                    with_other.push(other_text);
                    with_other.join(", ")
                } else if selected.is_empty() {
                    // No selections - skip this question
                    continue;
                } else {
                    selected.join(", ")
                }
            } else {
                // Single select
                if let Some(selection) = self.question_state.selections.get(i).copied().flatten() {
                    if let Some(opt) = question.options.get(selection) {
                        opt.label.clone()
                    } else {
                        continue;
                    }
                } else {
                    // "Other" selected - use the text
                    let other_text = self
                        .question_state
                        .other_texts
                        .get(i)
                        .cloned()
                        .unwrap_or_default();
                    if other_text.is_empty() {
                        continue;
                    }
                    other_text
                }
            };

            answers.insert(question.question.clone(), answer);
        }

        // Send the response and clean up all related state
        self.send_question_response(&permission_id, answers);
        self.cleanup_question_response(&permission_id, None);

        true
    }

    // ========================================================================
    // Dashboard Question Submission
    // ========================================================================

    /// Submit dashboard question response via WebSocket
    ///
    /// Called when user confirms selection in dashboard question overlay.
    /// Collects answers from the dashboard question state and sends via WebSocket.
    ///
    /// # Arguments
    /// * `thread_id` - The thread this question belongs to
    /// * `request_id` - The WebSocket request ID for the response
    /// * `answers` - The collected answers (question -> answer)
    ///
    /// Response format: `{type: "command_response", request_id, result: {status: "success", data: {allowed: true, message: answers_json}}}`
    pub fn submit_dashboard_question(
        &mut self,
        thread_id: &str,
        request_id: &str,
        answers: std::collections::HashMap<String, String>,
    ) {
        info!(
            "Submitting dashboard question response for thread {} (request_id: {})",
            thread_id, request_id
        );

        // Send the response and clean up all related state
        self.send_question_response(request_id, answers);
        self.cleanup_question_response(request_id, Some(thread_id));

        debug!(
            "Dashboard question submitted and overlay closed for thread {}",
            thread_id
        );
    }

    /// Send question answers via WebSocket
    fn send_question_response(
        &self,
        request_id: &str,
        answers: std::collections::HashMap<String, String>,
    ) {
        let sender = match &self.ws_sender {
            Some(s) => s,
            None => {
                warn!("No WebSocket sender for question response");
                return;
            }
        };

        if self.ws_connection_state != WsConnectionState::Connected {
            warn!("WebSocket not connected for question response");
            return;
        }

        // Convert answers to JSON Value
        let answers_value = serde_json::to_value(&answers).unwrap_or_default();

        let response = WsCommandResponse {
            type_: "command_response".to_string(),
            request_id: request_id.to_string(),
            result: WsCommandResult {
                status: "success".to_string(),
                data: WsPermissionData {
                    allowed: true,
                    message: Some(answers_value.to_string()),
                },
            },
        };

        if let Err(e) = sender.try_send(WsOutgoingMessage::CommandResponse(response)) {
            error!("Failed to send question response: {}", e);
        } else {
            debug!("Sent question response for {}", request_id);
        }
    }

    // ========================================================================
    // Dialog Opening Helpers
    // ========================================================================

    /// Open the AskUserQuestion dialog for the first user input thread
    ///
    /// This method finds the first thread waiting for user input and expands it.
    /// Only opens if no overlay is currently active.
    ///
    /// Returns true if a dialog was opened, false otherwise.
    pub fn open_ask_user_question_dialog(&mut self) -> bool {
        // Check if an overlay is NOT already open
        if self.dashboard.overlay().is_none() {
            // Find first thread waiting for user input
            if let Some(thread_id) = self.dashboard.find_first_user_input_thread() {
                // Use a reasonable anchor_y for keyboard-triggered overlay
                // (middle of screen is typical for non-click interactions)
                let computed_anchor_y = self.terminal_height / 2;
                self.dashboard.expand_thread(&thread_id, computed_anchor_y);
                debug!("Opened question dialog for thread {}", thread_id);
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::PermissionRequest;
    use std::time::Instant;
    use tokio::sync::mpsc;

    /// Helper to create a test App with WebSocket sender
    fn create_test_app_with_ws() -> (App, mpsc::Receiver<WsOutgoingMessage>) {
        let mut app = App::default();
        let (tx, rx) = mpsc::channel(10);
        app.ws_sender = Some(tx);
        app.ws_connection_state = WsConnectionState::Connected;
        (app, rx)
    }

    /// Helper to create a test permission request
    fn create_test_permission(permission_id: &str) -> PermissionRequest {
        PermissionRequest {
            permission_id: permission_id.to_string(),
            thread_id: Some("test-thread".to_string()),
            tool_name: "Bash".to_string(),
            description: "Run a command".to_string(),
            context: Some("ls -la".to_string()),
            tool_input: Some(serde_json::json!({"command": "ls -la"})),
            received_at: Instant::now(),
        }
    }

    /// Default test thread ID for consistency
    const TEST_THREAD_ID: &str = "test-thread";

    /// Helper to extract WsCommandResponse from WsOutgoingMessage
    fn extract_command_response(msg: WsOutgoingMessage) -> WsCommandResponse {
        match msg {
            WsOutgoingMessage::CommandResponse(resp) => resp,
            WsOutgoingMessage::CancelPermission(_) => panic!("Expected CommandResponse"),
            WsOutgoingMessage::PlanApprovalResponse(_) => panic!("Expected CommandResponse"),
            WsOutgoingMessage::ClaudeLoginResponse(_) => panic!("Expected CommandResponse"),
            WsOutgoingMessage::ClaudeAuthTokenResponse(_) => panic!("Expected CommandResponse"),
            WsOutgoingMessage::Steering(_) => panic!("Expected CommandResponse"),
            WsOutgoingMessage::ClaudeAccountsListRequest(_) => panic!("Expected CommandResponse"),
            WsOutgoingMessage::ClaudeAccountAddRequest(_) => panic!("Expected CommandResponse"),
            WsOutgoingMessage::ClaudeAccountRemoveRequest(_) => panic!("Expected CommandResponse"),
            WsOutgoingMessage::ClaudeAccountSelectRequest(_) => panic!("Expected CommandResponse"),
        }
    }

    /// Helper to setup a thread with permission state for testing
    /// This ensures the thread is properly registered and appears as "needs action"
    fn setup_thread_with_permission(app: &mut App, thread_id: &str, permission: PermissionRequest) {
        use crate::models::{Thread, ThreadMode, ThreadStatus, ThreadType};
        use crate::models::dashboard::WaitingFor;
        use chrono::Utc;

        // Create and add thread
        let thread = Thread {
            id: thread_id.to_string(),
            title: "Test Thread".to_string(),
            description: None,
            preview: String::new(),
            updated_at: Utc::now(),
            thread_type: ThreadType::Conversation,
            mode: ThreadMode::Normal,
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: Utc::now(),
            working_directory: None,
            status: Some(ThreadStatus::Waiting),
            verified: None,
            verified_at: None,
        };
        app.dashboard.add_thread(thread);

        // Set pending permission
        app.dashboard.set_pending_permission(thread_id, permission);

        // Set waiting state using public API
        app.dashboard.update_thread_status(
            thread_id,
            ThreadStatus::Waiting,
            Some(WaitingFor::Permission {
                request_id: "test-request".to_string(),
                tool_name: "Bash".to_string(),
            }),
        );

        // Trigger thread views computation to ensure needs_action is set
        let _ = app.dashboard.compute_thread_views();
    }

    #[test]
    fn test_permission_response_result_debug() {
        // Test that all variants can be debug-printed
        let results = vec![
            PermissionResponseResult::SentViaWebSocket,
            PermissionResponseResult::SentViaHttpFallback,
            PermissionResponseResult::Expired,
            PermissionResponseResult::Failed("test error".to_string()),
        ];

        for result in results {
            // Just ensure debug formatting works
            let _ = format!("{:?}", result);
        }
    }

    #[test]
    fn test_is_permission_expired_no_pending_permission() {
        let app = App::default();
        assert!(!app.is_permission_expired("test-id"));
    }

    #[test]
    fn test_is_permission_expired_wrong_id() {
        let mut app = App::default();
        app.dashboard
            .set_pending_permission(TEST_THREAD_ID, create_test_permission("perm-123"));
        // Different ID should return false
        assert!(!app.is_permission_expired("perm-456"));
    }

    #[test]
    fn test_is_permission_expired_not_expired() {
        let mut app = App::default();
        app.dashboard
            .set_pending_permission(TEST_THREAD_ID, create_test_permission("perm-123"));
        // Just created, should not be expired
        assert!(!app.is_permission_expired("perm-123"));
    }

    #[test]
    fn test_send_ws_permission_response_no_sender() {
        let app = App::default();
        let result = app.send_ws_permission_response("test-id", true);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not available"));
    }

    #[test]
    fn test_send_ws_permission_response_disconnected() {
        let mut app = App::default();
        let (tx, _rx) = mpsc::channel(10);
        app.ws_sender = Some(tx);
        app.ws_connection_state = WsConnectionState::Disconnected;

        let result = app.send_ws_permission_response("test-id", true);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not connected"));
    }

    #[test]
    fn test_send_ws_permission_response_reconnecting() {
        let mut app = App::default();
        let (tx, _rx) = mpsc::channel(10);
        app.ws_sender = Some(tx);
        app.ws_connection_state = WsConnectionState::Reconnecting { attempt: 2 };

        let result = app.send_ws_permission_response("test-id", true);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not connected"));
    }

    #[tokio::test]
    async fn test_send_ws_permission_response_success() {
        let (app, mut rx) = create_test_app_with_ws();

        let result = app.send_ws_permission_response("perm-123", true);
        assert!(result.is_ok());

        // Verify the message was sent
        let msg = extract_command_response(rx.recv().await.unwrap());
        assert_eq!(msg.type_, "command_response");
        assert_eq!(msg.request_id, "perm-123");
        assert_eq!(msg.result.status, "success");
        assert!(msg.result.data.allowed);
        assert!(msg.result.data.message.is_none());
    }

    #[tokio::test]
    async fn test_send_ws_permission_response_denial() {
        let (app, mut rx) = create_test_app_with_ws();

        let result = app.send_ws_permission_response("perm-456", false);
        assert!(result.is_ok());

        // Verify the message was sent
        let msg = extract_command_response(rx.recv().await.unwrap());
        assert_eq!(msg.request_id, "perm-456");
        assert!(!msg.result.data.allowed);
    }

    #[tokio::test]
    async fn test_approve_permission_clears_pending() {
        let (mut app, _rx) = create_test_app_with_ws();
        app.dashboard
            .set_pending_permission(TEST_THREAD_ID, create_test_permission("perm-123"));

        app.approve_permission("perm-123");

        assert!(app.dashboard.get_pending_permission(TEST_THREAD_ID).is_none());
    }

    #[tokio::test]
    async fn test_deny_permission_clears_pending() {
        let (mut app, _rx) = create_test_app_with_ws();
        app.dashboard
            .set_pending_permission(TEST_THREAD_ID, create_test_permission("perm-123"));

        app.deny_permission("perm-123");

        assert!(app.dashboard.get_pending_permission(TEST_THREAD_ID).is_none());
    }

    #[tokio::test]
    async fn test_approve_permission_sends_ws_message() {
        let (mut app, mut rx) = create_test_app_with_ws();
        app.dashboard
            .set_pending_permission(TEST_THREAD_ID, create_test_permission("perm-789"));

        app.approve_permission("perm-789");

        let msg = extract_command_response(rx.recv().await.unwrap());
        assert_eq!(msg.request_id, "perm-789");
        assert!(msg.result.data.allowed);
    }

    #[tokio::test]
    async fn test_deny_permission_sends_ws_message() {
        let (mut app, mut rx) = create_test_app_with_ws();
        app.dashboard
            .set_pending_permission(TEST_THREAD_ID, create_test_permission("perm-abc"));

        app.deny_permission("perm-abc");

        let msg = extract_command_response(rx.recv().await.unwrap());
        assert_eq!(msg.request_id, "perm-abc");
        assert!(!msg.result.data.allowed);
    }

    #[tokio::test]
    async fn test_allow_tool_always_adds_to_allowed_and_approves() {
        let (mut app, mut rx) = create_test_app_with_ws();
        app.dashboard
            .set_pending_permission(TEST_THREAD_ID, create_test_permission("perm-xyz"));

        app.allow_tool_always("Bash", "perm-xyz");

        // Tool should be in allowed list
        assert!(app.session_state.allowed_tools.contains("Bash"));

        // Permission should be approved and cleared
        assert!(app.dashboard.get_pending_permission(TEST_THREAD_ID).is_none());

        // WebSocket message should be sent
        let msg = extract_command_response(rx.recv().await.unwrap());
        assert_eq!(msg.request_id, "perm-xyz");
        assert!(msg.result.data.allowed);
    }

    #[tokio::test]
    async fn test_handle_permission_key_y() {
        let (mut app, mut rx) = create_test_app_with_ws();
        app.dashboard
            .set_pending_permission(TEST_THREAD_ID, create_test_permission("perm-y"));

        let handled = app.handle_permission_key('y');
        assert!(handled);

        let msg = extract_command_response(rx.recv().await.unwrap());
        assert!(msg.result.data.allowed);
    }

    #[tokio::test]
    async fn test_handle_permission_key_n() {
        let (mut app, mut rx) = create_test_app_with_ws();
        app.dashboard
            .set_pending_permission(TEST_THREAD_ID, create_test_permission("perm-n"));

        let handled = app.handle_permission_key('n');
        assert!(handled);

        let msg = extract_command_response(rx.recv().await.unwrap());
        assert!(!msg.result.data.allowed);
    }

    #[tokio::test]
    async fn test_handle_permission_key_a() {
        let (mut app, mut rx) = create_test_app_with_ws();
        let mut perm = create_test_permission("perm-a");
        perm.tool_name = "Read".to_string();
        app.dashboard.set_pending_permission(TEST_THREAD_ID, perm);

        let handled = app.handle_permission_key('a');
        assert!(handled);
        assert!(app.session_state.allowed_tools.contains("Read"));

        let msg = extract_command_response(rx.recv().await.unwrap());
        assert!(msg.result.data.allowed);
    }

    #[test]
    fn test_handle_permission_key_no_pending() {
        let mut app = App::default();
        let handled = app.handle_permission_key('y');
        assert!(!handled);
    }

    #[test]
    fn test_handle_permission_key_non_yna_key_consumed() {
        // Non-Y/N/A keys should be consumed (return true) when permission is pending
        // This prevents fallback handling from inserting chars into textarea
        let mut app = App::default();
        app.dashboard
            .set_pending_permission(TEST_THREAD_ID, create_test_permission("perm-x"));

        let handled = app.handle_permission_key('x');
        assert!(handled); // Key should be consumed (not fall through)

        // Permission should still be pending (not approved or denied)
        assert!(app.dashboard.get_pending_permission(TEST_THREAD_ID).is_some());
    }

    #[test]
    fn test_handle_permission_key_various_non_yna_keys_consumed() {
        // Various non-permission keys should all be consumed
        let mut app = App::default();
        app.dashboard
            .set_pending_permission(TEST_THREAD_ID, create_test_permission("perm-multi"));

        // Test several different keys
        for key in ['g', 'z', '1', ' ', '.', 'q'] {
            // Reset permission for each test
            app.dashboard
                .set_pending_permission(TEST_THREAD_ID, create_test_permission("perm-multi"));

            let handled = app.handle_permission_key(key);
            assert!(handled, "Key '{}' should be consumed", key);

            // Permission should still be pending
            assert!(
                app.dashboard.get_pending_permission(TEST_THREAD_ID).is_some(),
                "Permission should still be pending after key '{}'",
                key
            );
        }
    }

    #[test]
    fn test_handle_permission_key_uppercase_y() {
        let (mut app, _rx) = create_test_app_with_ws();
        app.dashboard
            .set_pending_permission(TEST_THREAD_ID, create_test_permission("perm-Y"));

        let handled = app.handle_permission_key('Y');
        assert!(handled);
    }

    #[test]
    fn test_handle_permission_key_uppercase_n() {
        let (mut app, _rx) = create_test_app_with_ws();
        app.dashboard
            .set_pending_permission(TEST_THREAD_ID, create_test_permission("perm-N"));

        let handled = app.handle_permission_key('N');
        assert!(handled);
    }

    #[test]
    fn test_handle_permission_key_uppercase_a() {
        let (mut app, _rx) = create_test_app_with_ws();
        app.dashboard
            .set_pending_permission(TEST_THREAD_ID, create_test_permission("perm-A"));

        let handled = app.handle_permission_key('A');
        assert!(handled);
    }

    #[test]
    fn test_send_permission_response_no_ws_no_runtime() {
        // Without tokio runtime, should return Failed
        let mut app = App::default();
        app.dashboard
            .set_pending_permission(TEST_THREAD_ID, create_test_permission("perm-test"));

        let result = app.send_permission_response("perm-test", true);
        match result {
            PermissionResponseResult::Failed(msg) => {
                assert!(msg.contains("No runtime available"));
            }
            _ => panic!("Expected Failed result"),
        }
    }

    #[tokio::test]
    async fn test_ws_response_message_format() {
        let (app, mut rx) = create_test_app_with_ws();

        app.send_ws_permission_response("req-format-test", true)
            .unwrap();

        let msg = rx.recv().await.unwrap();

        // Verify JSON serialization matches expected format
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["type"], "command_response");
        assert_eq!(json["request_id"], "req-format-test");
        assert_eq!(json["result"]["status"], "success");
        assert_eq!(json["result"]["data"]["allowed"], true);
        // message should not be present when None due to skip_serializing_if
        assert!(json["result"]["data"]["message"].is_null());
    }

    #[tokio::test]
    async fn test_cancel_permission_sends_cancel_message() {
        let (mut app, mut rx) = create_test_app_with_ws();
        app.dashboard
            .set_pending_permission(TEST_THREAD_ID, create_test_permission("perm-cancel"));

        app.cancel_permission("perm-cancel");

        // Permission should be cleared
        assert!(app.dashboard.get_pending_permission(TEST_THREAD_ID).is_none());

        // Verify cancel message was sent
        let msg = rx.recv().await.unwrap();
        match msg {
            WsOutgoingMessage::CancelPermission(cancel) => {
                assert_eq!(cancel.request_id, "perm-cancel");
                assert_eq!(cancel.type_, "cancel_permission");
            }
            _ => panic!("Expected CancelPermission message"),
        }
    }

    // ============================================================================
    // AskUserQuestion Navigation Tests
    // ============================================================================

    /// Helper to create an AskUserQuestion permission
    fn create_ask_user_question_permission(permission_id: &str) -> PermissionRequest {
        PermissionRequest {
            permission_id: permission_id.to_string(),
            thread_id: Some(TEST_THREAD_ID.to_string()),
            tool_name: "AskUserQuestion".to_string(),
            description: "Answer questions".to_string(),
            context: None,
            tool_input: Some(serde_json::json!({
                "questions": [
                    {
                        "question": "Select an option",
                        "header": "Choice",
                        "options": [
                            {"label": "Option A", "description": "First option"},
                            {"label": "Option B", "description": "Second option"},
                            {"label": "Option C", "description": "Third option"}
                        ],
                        "multiSelect": false
                    }
                ],
                "answers": {}
            })),
            received_at: Instant::now(),
        }
    }

    /// Helper to create multi-question permission
    fn create_multi_question_permission(permission_id: &str) -> PermissionRequest {
        PermissionRequest {
            permission_id: permission_id.to_string(),
            thread_id: Some(TEST_THREAD_ID.to_string()),
            tool_name: "AskUserQuestion".to_string(),
            description: "Answer questions".to_string(),
            context: None,
            tool_input: Some(serde_json::json!({
                "questions": [
                    {
                        "question": "First question",
                        "header": "Q1",
                        "options": [
                            {"label": "A1", "description": ""},
                            {"label": "A2", "description": ""}
                        ],
                        "multiSelect": false
                    },
                    {
                        "question": "Second question",
                        "header": "Q2",
                        "options": [
                            {"label": "B1", "description": ""},
                            {"label": "B2", "description": ""},
                            {"label": "B3", "description": ""}
                        ],
                        "multiSelect": true
                    }
                ],
                "answers": {}
            })),
            received_at: Instant::now(),
        }
    }

    #[test]
    fn test_is_ask_user_question_pending_true() {
        let mut app = App::default();
        setup_thread_with_permission(&mut app, TEST_THREAD_ID, create_ask_user_question_permission("perm-q"));

        assert!(app.is_ask_user_question_pending());
    }

    #[test]
    fn test_is_ask_user_question_pending_false_wrong_tool() {
        let mut app = App::default();
        app.dashboard
            .set_pending_permission(TEST_THREAD_ID, create_test_permission("perm-x"));

        assert!(!app.is_ask_user_question_pending());
    }

    #[test]
    fn test_is_ask_user_question_pending_false_no_permission() {
        let app = App::default();
        assert!(!app.is_ask_user_question_pending());
    }

    #[test]
    fn test_init_question_state() {
        let mut app = App::default();
        setup_thread_with_permission(
            &mut app,
            TEST_THREAD_ID,
            create_ask_user_question_permission("perm-init"),
        );

        app.init_question_state();

        // Should have initialized selections for 1 question
        assert_eq!(app.question_state.selections.len(), 1);
        assert_eq!(app.question_state.tab_index, 0);
        // First option should be selected by default
        assert_eq!(app.question_state.selections[0], Some(0));
    }

    #[test]
    fn test_question_next_option() {
        let mut app = App::default();
        setup_thread_with_permission(
            &mut app,
            TEST_THREAD_ID,
            create_ask_user_question_permission("perm-next"),
        );
        app.init_question_state();

        // Start at option 0
        assert_eq!(app.question_state.current_selection(), Some(0));

        // Move to option 1
        app.question_next_option();
        assert_eq!(app.question_state.current_selection(), Some(1));

        // Move to option 2
        app.question_next_option();
        assert_eq!(app.question_state.current_selection(), Some(2));

        // Wrap to "Other" (None)
        app.question_next_option();
        assert_eq!(app.question_state.current_selection(), None);

        // Wrap back to option 0
        app.question_next_option();
        assert_eq!(app.question_state.current_selection(), Some(0));
    }

    #[test]
    fn test_question_prev_option() {
        let mut app = App::default();
        setup_thread_with_permission(
            &mut app,
            TEST_THREAD_ID,
            create_ask_user_question_permission("perm-prev"),
        );
        app.init_question_state();

        // Start at option 0
        assert_eq!(app.question_state.current_selection(), Some(0));

        // Wrap to "Other" (None)
        app.question_prev_option();
        assert_eq!(app.question_state.current_selection(), None);

        // Move to last option (2)
        app.question_prev_option();
        assert_eq!(app.question_state.current_selection(), Some(2));

        // Move to option 1
        app.question_prev_option();
        assert_eq!(app.question_state.current_selection(), Some(1));

        // Move to option 0
        app.question_prev_option();
        assert_eq!(app.question_state.current_selection(), Some(0));
    }

    #[test]
    fn test_question_next_tab() {
        let mut app = App::default();
        setup_thread_with_permission(&mut app, TEST_THREAD_ID, create_multi_question_permission("perm-tab"));
        app.init_question_state();

        // Start at tab 0
        assert_eq!(app.question_state.tab_index, 0);

        // Move to tab 1
        app.question_next_tab();
        assert_eq!(app.question_state.tab_index, 1);

        // Wrap back to tab 0
        app.question_next_tab();
        assert_eq!(app.question_state.tab_index, 0);
    }

    #[test]
    fn test_question_next_tab_single_question_no_change() {
        let mut app = App::default();
        setup_thread_with_permission(
            &mut app,
            TEST_THREAD_ID,
            create_ask_user_question_permission("perm-single"),
        );
        app.init_question_state();

        // Start at tab 0
        assert_eq!(app.question_state.tab_index, 0);

        // Should not change (only 1 question)
        app.question_next_tab();
        assert_eq!(app.question_state.tab_index, 0);
    }

    #[test]
    fn test_question_toggle_option_multi_select() {
        let mut app = App::default();
        let perm = create_multi_question_permission("perm-toggle");
        setup_thread_with_permission(&mut app, TEST_THREAD_ID, perm);
        app.init_question_state();

        // Switch to second question (multi-select)
        app.question_next_tab();

        // At option 0
        assert_eq!(app.question_state.current_selection(), Some(0));
        assert!(!app.question_state.is_multi_selected(0));

        // Toggle option 0 on
        app.question_toggle_option();
        assert!(app.question_state.is_multi_selected(0));

        // Toggle option 0 off
        app.question_toggle_option();
        assert!(!app.question_state.is_multi_selected(0));
    }

    #[test]
    fn test_question_toggle_option_single_select_no_effect() {
        let mut app = App::default();
        setup_thread_with_permission(
            &mut app,
            TEST_THREAD_ID,
            create_ask_user_question_permission("perm-no-toggle"),
        );
        app.init_question_state();

        // First question is single-select
        assert_eq!(app.question_state.current_selection(), Some(0));

        // Toggle should have no effect in single-select mode
        app.question_toggle_option();
        assert!(!app.question_state.is_multi_selected(0));
    }

    #[test]
    fn test_question_type_char_and_backspace() {
        let mut app = App::default();
        setup_thread_with_permission(
            &mut app,
            TEST_THREAD_ID,
            create_ask_user_question_permission("perm-type"),
        );
        app.init_question_state();

        // Move to "Other"
        app.question_next_option();
        app.question_next_option();
        app.question_next_option();
        assert_eq!(app.question_state.current_selection(), None);

        // Activate "Other" text input
        app.question_state.other_active = true;

        // Type some characters
        app.question_type_char('t');
        app.question_type_char('e');
        app.question_type_char('s');
        app.question_type_char('t');

        assert_eq!(app.question_state.current_other_text(), "test");

        // Backspace
        app.question_backspace();
        assert_eq!(app.question_state.current_other_text(), "tes");

        app.question_backspace();
        app.question_backspace();
        assert_eq!(app.question_state.current_other_text(), "t");

        app.question_backspace();
        assert_eq!(app.question_state.current_other_text(), "");
    }

    #[test]
    fn test_question_cancel_other() {
        let mut app = App::default();
        setup_thread_with_permission(
            &mut app,
            TEST_THREAD_ID,
            create_ask_user_question_permission("perm-cancel"),
        );
        app.init_question_state();

        // Activate "Other" mode and type
        app.question_state.other_active = true;
        app.question_type_char('x');
        assert_eq!(app.question_state.current_other_text(), "x");

        // Cancel should clear text and deactivate
        app.question_cancel_other();
        assert!(!app.question_state.other_active);
        assert_eq!(app.question_state.current_other_text(), "");
    }

    #[test]
    fn test_question_confirm_activates_other() {
        let mut app = App::default();
        setup_thread_with_permission(
            &mut app,
            TEST_THREAD_ID,
            create_ask_user_question_permission("perm-confirm"),
        );
        app.init_question_state();

        // Move to "Other"
        app.question_next_option();
        app.question_next_option();
        app.question_next_option();
        assert_eq!(app.question_state.current_selection(), None);
        assert!(!app.question_state.other_active);

        // Confirm should activate "Other" text input (not submit)
        let result = app.question_confirm();
        assert!(!result); // Should not submit
        assert!(app.question_state.other_active);
    }

    #[test]
    fn test_question_confirm_on_option() {
        let (mut app, _rx) = create_test_app_with_ws();
        setup_thread_with_permission(
            &mut app,
            TEST_THREAD_ID,
            create_ask_user_question_permission("perm-submit"),
        );
        app.init_question_state();

        // Select option 1
        app.question_next_option();
        assert_eq!(app.question_state.current_selection(), Some(1));

        // Confirm should submit
        let result = app.question_confirm();
        assert!(result);

        // Permission should be cleared
        assert!(app.dashboard.get_pending_permission(TEST_THREAD_ID).is_none());
    }

    #[test]
    fn test_question_backspace_not_in_other_mode() {
        let mut app = App::default();
        app.dashboard.set_pending_permission(
            TEST_THREAD_ID,
            create_ask_user_question_permission("perm-bs"),
        );
        app.init_question_state();

        // Not in "Other" mode
        assert!(!app.question_state.other_active);

        // Backspace should have no effect
        app.question_backspace();
        assert_eq!(app.question_state.current_other_text(), "");
    }

    #[test]
    fn test_question_type_char_not_in_other_mode() {
        let mut app = App::default();
        app.dashboard.set_pending_permission(
            TEST_THREAD_ID,
            create_ask_user_question_permission("perm-type-no"),
        );
        app.init_question_state();

        // Not in "Other" mode
        assert!(!app.question_state.other_active);

        // Typing should have no effect
        app.question_type_char('x');
        assert_eq!(app.question_state.current_other_text(), "");
    }

    // ============= Multi-Question Tab Progression Tests =============

    #[test]
    fn test_question_confirm_multi_question_advances_tab() {
        let (mut app, _rx) = create_test_app_with_ws();
        setup_thread_with_permission(
            &mut app,
            TEST_THREAD_ID,
            create_multi_question_permission("perm-multi-advance"),
        );
        app.init_question_state();

        // Start at tab 0, select option 0
        assert_eq!(app.question_state.tab_index, 0);
        assert_eq!(app.question_state.current_selection(), Some(0));

        // Confirm should mark as answered and advance to tab 1, NOT submit
        let result = app.question_confirm();
        assert!(!result); // Should NOT submit yet
        assert!(app.question_state.answered[0]); // Tab 0 marked answered
        assert_eq!(app.question_state.tab_index, 1); // Moved to tab 1
        assert!(app.dashboard.get_pending_permission(TEST_THREAD_ID).is_some()); // Permission still pending
    }

    #[test]
    fn test_question_confirm_multi_question_submits_on_last() {
        let (mut app, _rx) = create_test_app_with_ws();
        setup_thread_with_permission(
            &mut app,
            TEST_THREAD_ID,
            create_multi_question_permission("perm-multi-submit"),
        );
        app.init_question_state();

        // Answer first question
        let result1 = app.question_confirm();
        assert!(!result1); // Not submitted yet
        assert_eq!(app.question_state.tab_index, 1);

        // For multi-select question, toggle an option first
        app.question_toggle_option();

        // Answer second (last) question - should submit
        let result2 = app.question_confirm();
        assert!(result2); // Should submit now
        assert!(app.dashboard.get_pending_permission(TEST_THREAD_ID).is_none()); // Permission cleared
    }

    #[test]
    fn test_question_confirm_multi_question_answered_tracking() {
        let (mut app, _rx) = create_test_app_with_ws();
        setup_thread_with_permission(
            &mut app,
            TEST_THREAD_ID,
            create_multi_question_permission("perm-multi-track"),
        );
        app.init_question_state();

        // Both questions start unanswered
        assert!(!app.question_state.answered[0]);
        assert!(!app.question_state.answered[1]);

        // Confirm first question
        app.question_confirm();
        assert!(app.question_state.answered[0]);
        assert!(!app.question_state.answered[1]);

        // Can use tab to go back to first question
        app.question_next_tab();
        assert_eq!(app.question_state.tab_index, 0);
        assert!(app.question_state.is_current_answered()); // Still marked as answered
    }

    #[test]
    fn test_question_confirm_single_question_submits_immediately() {
        let (mut app, _rx) = create_test_app_with_ws();
        setup_thread_with_permission(
            &mut app,
            TEST_THREAD_ID,
            create_ask_user_question_permission("perm-single-submit"),
        );
        app.init_question_state();

        // Single question - confirm should submit immediately
        let result = app.question_confirm();
        assert!(result); // Should submit
        assert!(app.dashboard.get_pending_permission(TEST_THREAD_ID).is_none()); // Permission cleared
    }

    #[test]
    fn test_question_confirm_multi_question_update_answered() {
        let (mut app, _rx) = create_test_app_with_ws();
        setup_thread_with_permission(
            &mut app,
            TEST_THREAD_ID,
            create_multi_question_permission("perm-multi-update"),
        );
        app.init_question_state();

        // Answer first question and advance
        app.question_confirm();
        assert_eq!(app.question_state.tab_index, 1);

        // Go back to first question
        app.question_next_tab();
        assert_eq!(app.question_state.tab_index, 0);

        // Change selection and confirm again (update)
        app.question_next_option();
        let result = app.question_confirm();

        // Since second question is not answered, should advance to it
        assert!(!result); // Not submitted yet
        assert_eq!(app.question_state.tab_index, 1); // Back to unanswered question
    }

    // ============= Dashboard Question Submission Tests =============

    #[tokio::test]
    async fn test_submit_dashboard_question_sends_ws_message() {
        use crate::state::session::{AskUserQuestionData, Question, QuestionOption};

        let (mut app, mut rx) = create_test_app_with_ws();

        // Set up pending question in dashboard
        let question_data = AskUserQuestionData {
            questions: vec![Question {
                question: "Which option?".to_string(),
                header: "Options".to_string(),
                options: vec![QuestionOption {
                    label: "Option A".to_string(),
                    description: "First option".to_string(),
                }],
                multi_select: false,
            }],
            answers: std::collections::HashMap::new(),
        };
        app.dashboard
            .set_pending_question("test-thread", "req-dashboard".to_string(), question_data);

        // Prepare answers
        let mut answers = std::collections::HashMap::new();
        answers.insert("Which option?".to_string(), "Option A".to_string());

        // Submit the question
        app.submit_dashboard_question("test-thread", "req-dashboard", answers);

        // Verify WebSocket message was sent
        let msg = rx.recv().await.unwrap();
        let resp = extract_command_response(msg);
        assert_eq!(resp.request_id, "req-dashboard");
        assert_eq!(resp.result.status, "success");
        assert!(resp.result.data.allowed); // Always true for questions
        assert!(resp.result.data.message.is_some());

        // Verify message contains the answer as JSON
        let message = resp.result.data.message.unwrap();
        assert!(message.contains("Option A"));

        // Verify pending question is cleared
        assert!(app.dashboard.get_pending_question("test-thread").is_none());
    }

    #[tokio::test]
    async fn test_submit_dashboard_question_closes_overlay() {
        use crate::state::session::{AskUserQuestionData, Question, QuestionOption};
        use crate::models::{Thread, ThreadMode, ThreadType};
        use chrono::Utc;

        let (mut app, _rx) = create_test_app_with_ws();

        // Add a thread and set pending question
        let thread = Thread {
            id: "t1".to_string(),
            title: "Test Thread".to_string(),
            description: None,
            preview: String::new(),
            updated_at: Utc::now(),
            thread_type: ThreadType::Conversation,
            mode: ThreadMode::Normal,
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: Utc::now(),
            working_directory: None,
            status: None,
            verified: None,
            verified_at: None,
        };
        app.dashboard.add_thread(thread);

        let question_data = AskUserQuestionData {
            questions: vec![Question {
                question: "Test?".to_string(),
                header: "Test".to_string(),
                options: vec![QuestionOption {
                    label: "A".to_string(),
                    description: "Option A".to_string(),
                }],
                multi_select: false,
            }],
            answers: std::collections::HashMap::new(),
        };
        app.dashboard
            .set_pending_question("t1", "req-overlay".to_string(), question_data);

        // Expand thread to open overlay
        app.dashboard.expand_thread("t1", 10);
        assert!(app.dashboard.overlay().is_some());

        // Submit question
        let mut answers = std::collections::HashMap::new();
        answers.insert("Test?".to_string(), "A".to_string());
        app.submit_dashboard_question("t1", "req-overlay", answers);

        // Verify overlay is closed
        assert!(app.dashboard.overlay().is_none());
    }

    #[tokio::test]
    async fn test_submit_dashboard_question_multi_select_comma_separated() {
        let (mut app, mut rx) = create_test_app_with_ws();

        // Prepare multi-select answers (comma-separated)
        let mut answers = std::collections::HashMap::new();
        answers.insert("Select features".to_string(), "Feature A, Feature B".to_string());

        // Submit
        app.submit_dashboard_question("t1", "req-multi", answers);

        // Verify message contains comma-separated values
        let msg = rx.recv().await.unwrap();
        let resp = extract_command_response(msg);
        let message = resp.result.data.message.unwrap();
        assert!(message.contains("Feature A, Feature B"));
    }
}
