//! Session-level state management
//!
//! SessionState contains information that persists at the session level,
//! not per-thread. This includes active skills, context usage tracking,
//! pending permissions, and OAuth requirements.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::Instant;

// ============================================================================
// AskUserQuestion Data Structures
// ============================================================================

/// A single option in an AskUserQuestion
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuestionOption {
    /// Display text for the option
    pub label: String,
    /// Explanation of what this option means
    pub description: String,
}

/// A single question from AskUserQuestion tool
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Question {
    /// The question text to display
    pub question: String,
    /// Short label/header for the question (max 12 chars)
    pub header: String,
    /// Available options for this question
    pub options: Vec<QuestionOption>,
    /// Whether multiple options can be selected
    #[serde(rename = "multiSelect", default)]
    pub multi_select: bool,
}

/// Data structure for the AskUserQuestion tool
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AskUserQuestionData {
    /// The questions to ask
    pub questions: Vec<Question>,
    /// Previously collected answers (usually empty on initial call)
    #[serde(default)]
    pub answers: HashMap<String, String>,
}

// ============================================================================
// AskUserQuestion UI State
// ============================================================================

/// State for the AskUserQuestion prompt UI
///
/// Tracks the current tab, selected options, and "Other" text input state
/// for the question prompt modal. Supports both single-select and multi-select modes.
#[derive(Debug, Clone, Default)]
pub struct AskUserQuestionState {
    /// Current question tab (0-indexed)
    pub tab_index: usize,
    /// Selected option index per question (None = "Other" selected)
    pub selections: Vec<Option<usize>>,
    /// "Other" text content per question
    pub other_texts: Vec<String>,
    /// Whether "Other" text input is active (cursor in text field)
    pub other_active: bool,
    /// For multiSelect: which options are toggled per question
    /// Each inner Vec corresponds to a question, with bools for each option
    pub multi_selections: Vec<Vec<bool>>,
    /// Tracks which questions have been answered (for multi-question flow)
    /// When a user confirms an answer on a question, that index is marked true.
    /// All must be true before final submission on multi-question prompts.
    pub answered: Vec<bool>,
}

impl AskUserQuestionState {
    /// Create a new AskUserQuestionState for the given number of questions
    ///
    /// # Arguments
    /// * `num_questions` - The number of questions in the prompt
    /// * `options_per_question` - A slice containing the number of options for each question
    ///
    /// # Returns
    /// A new AskUserQuestionState with:
    /// - `tab_index` set to 0
    /// - `selections` initialized to `Some(0)` (first option) for each question
    /// - `other_texts` initialized to empty strings for each question
    /// - `other_active` set to false
    /// - `multi_selections` initialized to all false for each option in each question
    pub fn new(num_questions: usize, options_per_question: &[usize]) -> Self {
        Self {
            tab_index: 0,
            selections: vec![Some(0); num_questions],
            other_texts: vec![String::new(); num_questions],
            other_active: false,
            multi_selections: options_per_question
                .iter()
                .map(|&count| vec![false; count])
                .collect(),
            answered: vec![false; num_questions],
        }
    }

    /// Create state from AskUserQuestionData
    ///
    /// Convenience constructor that extracts the number of questions and options
    /// from the data structure.
    pub fn from_data(data: &AskUserQuestionData) -> Self {
        let num_questions = data.questions.len();
        let options_per_question: Vec<usize> =
            data.questions.iter().map(|q| q.options.len()).collect();
        Self::new(num_questions, &options_per_question)
    }

    /// Reset all state to defaults
    ///
    /// Clears selections, other texts, and resets tab index to 0.
    /// This should be called when the question prompt is dismissed or answered.
    pub fn reset(&mut self) {
        self.tab_index = 0;
        self.selections.clear();
        self.other_texts.clear();
        self.other_active = false;
        self.multi_selections.clear();
        self.answered.clear();
    }

    /// Get the currently selected option index for the current tab
    ///
    /// Returns None if "Other" is selected or if tab_index is out of bounds.
    pub fn current_selection(&self) -> Option<usize> {
        self.selections.get(self.tab_index).copied().flatten()
    }

    /// Get the "Other" text for the current tab
    ///
    /// Returns an empty string if tab_index is out of bounds.
    pub fn current_other_text(&self) -> &str {
        self.other_texts
            .get(self.tab_index)
            .map(|s| s.as_str())
            .unwrap_or("")
    }

    /// Set the selection for the current tab
    ///
    /// # Arguments
    /// * `selection` - The option index to select, or None for "Other"
    pub fn set_current_selection(&mut self, selection: Option<usize>) {
        if self.tab_index < self.selections.len() {
            self.selections[self.tab_index] = selection;
        }
    }

    /// Toggle a multi-select option for the current tab
    ///
    /// # Arguments
    /// * `option_index` - The index of the option to toggle
    pub fn toggle_multi_selection(&mut self, option_index: usize) {
        if let Some(options) = self.multi_selections.get_mut(self.tab_index) {
            if option_index < options.len() {
                options[option_index] = !options[option_index];
            }
        }
    }

    /// Check if a multi-select option is selected for the current tab
    ///
    /// # Arguments
    /// * `option_index` - The index of the option to check
    ///
    /// Returns false if indices are out of bounds.
    pub fn is_multi_selected(&self, option_index: usize) -> bool {
        self.multi_selections
            .get(self.tab_index)
            .and_then(|options| options.get(option_index))
            .copied()
            .unwrap_or(false)
    }

    /// Append a character to the current tab's "Other" text
    pub fn push_other_char(&mut self, c: char) {
        if let Some(text) = self.other_texts.get_mut(self.tab_index) {
            text.push(c);
        }
    }

    /// Remove the last character from the current tab's "Other" text
    pub fn pop_other_char(&mut self) {
        if let Some(text) = self.other_texts.get_mut(self.tab_index) {
            text.pop();
        }
    }

    /// Move to the next tab (wraps around)
    ///
    /// # Arguments
    /// * `num_questions` - Total number of questions for wrap-around
    pub fn next_tab(&mut self, num_questions: usize) {
        if num_questions > 0 {
            self.tab_index = (self.tab_index + 1) % num_questions;
        }
    }

    /// Move to the previous tab (wraps around)
    ///
    /// # Arguments
    /// * `num_questions` - Total number of questions for wrap-around
    pub fn prev_tab(&mut self, num_questions: usize) {
        if num_questions > 0 {
            self.tab_index = if self.tab_index == 0 {
                num_questions - 1
            } else {
                self.tab_index - 1
            };
        }
    }

    /// Mark the current question as answered
    pub fn mark_current_answered(&mut self) {
        if self.tab_index < self.answered.len() {
            self.answered[self.tab_index] = true;
        }
    }

    /// Check if the current question has been answered
    pub fn is_current_answered(&self) -> bool {
        self.answered.get(self.tab_index).copied().unwrap_or(false)
    }

    /// Check if all questions have been answered
    pub fn all_answered(&self) -> bool {
        !self.answered.is_empty() && self.answered.iter().all(|&a| a)
    }

    /// Get the index of the first unanswered question, if any
    pub fn first_unanswered(&self) -> Option<usize> {
        self.answered.iter().position(|&a| !a)
    }

    /// Advance to the next unanswered question
    /// Returns true if moved to a new tab, false if no unanswered questions
    pub fn advance_to_next_unanswered(&mut self, num_questions: usize) -> bool {
        if num_questions == 0 {
            return false;
        }

        // Start from current position + 1 and wrap around
        for offset in 1..=num_questions {
            let idx = (self.tab_index + offset) % num_questions;
            if !self.answered.get(idx).copied().unwrap_or(true) {
                self.tab_index = idx;
                return true;
            }
        }
        false
    }
}

// ============================================================================
// Permission Request
// ============================================================================

/// A permission request waiting for user approval
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PermissionRequest {
    /// Unique ID for this permission request (for responding to backend)
    pub permission_id: String,
    /// Tool that requires permission
    pub tool_name: String,
    /// Description of what the tool wants to do
    pub description: String,
    /// Additional context about the request (e.g., file path, command)
    pub context: Option<String>,
    /// Raw tool input parameters for preview display
    #[serde(default)]
    pub tool_input: Option<serde_json::Value>,
    /// Timestamp when the request was received (for timeout tracking)
    /// Not serialized as it's runtime-only state
    #[serde(skip)]
    #[serde(default = "Instant::now")]
    pub received_at: Instant,
}

/// Session-level state that persists across threads
///
/// This contains information that is relevant to the entire session,
/// not specific to any single thread. It tracks things like:
/// - Active skills that have been loaded
/// - Context token usage from compaction events
/// - Pending permission prompts that need user input
/// - OAuth requirements for certain skills
/// - Tools that have been allowed for the session ("allow always")
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionState {
    /// Active skills loaded in this session
    pub skills: Vec<String>,

    /// Context tokens used (from context_compacted events)
    /// None if no compaction has occurred yet
    pub context_tokens_used: Option<u32>,

    /// Context token limit (max capacity)
    pub context_token_limit: Option<u32>,

    /// Current permission request awaiting user input
    pub pending_permission: Option<PermissionRequest>,

    /// OAuth requirement: (provider, skill_name)
    /// Set when a skill requires OAuth authentication
    pub oauth_required: Option<(String, String)>,

    /// OAuth consent URL for opening in browser
    pub oauth_url: Option<String>,

    /// Tools that have been allowed for the session ("allow always")
    /// When a user presses 'a' on a permission prompt, the tool is added here
    #[serde(default)]
    pub allowed_tools: HashSet<String>,
}

impl SessionState {
    /// Create a new empty SessionState
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a skill to the active skills list
    pub fn add_skill(&mut self, skill: String) {
        if !self.skills.contains(&skill) {
            self.skills.push(skill);
        }
    }

    /// Remove a skill from the active skills list
    pub fn remove_skill(&mut self, skill: &str) {
        self.skills.retain(|s| s != skill);
    }

    /// Check if a skill is active
    pub fn has_skill(&self, skill: &str) -> bool {
        self.skills.iter().any(|s| s == skill)
    }

    /// Update context tokens used
    pub fn set_context_tokens(&mut self, tokens: u32) {
        self.context_tokens_used = Some(tokens);
    }

    /// Set context token limit
    pub fn set_context_token_limit(&mut self, limit: u32) {
        self.context_token_limit = Some(limit);
    }

    /// Set OAuth consent URL
    pub fn set_oauth_url(&mut self, url: String) {
        self.oauth_url = Some(url);
    }

    /// Clear OAuth URL (after opening or when no longer needed)
    pub fn clear_oauth_url(&mut self) {
        self.oauth_url = None;
    }

    /// Set a pending permission request
    pub fn set_pending_permission(&mut self, request: PermissionRequest) {
        self.pending_permission = Some(request);
    }

    /// Clear the pending permission request (after user responds)
    pub fn clear_pending_permission(&mut self) {
        self.pending_permission = None;
    }

    /// Check if there's a pending permission request
    pub fn has_pending_permission(&self) -> bool {
        self.pending_permission.is_some()
    }

    /// Set OAuth requirement
    pub fn set_oauth_required(&mut self, provider: String, skill_name: String) {
        self.oauth_required = Some((provider, skill_name));
    }

    /// Clear OAuth requirement (after authentication)
    pub fn clear_oauth_required(&mut self) {
        self.oauth_required = None;
    }

    /// Check if OAuth is required
    pub fn needs_oauth(&self) -> bool {
        self.oauth_required.is_some()
    }

    /// Add a tool to the allowed tools set (for "allow always" behavior)
    pub fn allow_tool(&mut self, tool_name: String) {
        self.allowed_tools.insert(tool_name);
    }

    /// Check if a tool is allowed (user has previously selected "allow always")
    pub fn is_tool_allowed(&self, tool_name: &str) -> bool {
        self.allowed_tools.contains(tool_name)
    }

    /// Remove a tool from the allowed set
    pub fn disallow_tool(&mut self, tool_name: &str) {
        self.allowed_tools.remove(tool_name);
    }

    /// Reset all session state (for new session)
    pub fn reset(&mut self) {
        self.skills.clear();
        self.context_tokens_used = None;
        self.context_token_limit = None;
        self.pending_permission = None;
        self.oauth_required = None;
        self.oauth_url = None;
        self.allowed_tools.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_state_new() {
        let state = SessionState::new();
        assert!(state.skills.is_empty());
        assert!(state.context_tokens_used.is_none());
        assert!(state.pending_permission.is_none());
        assert!(state.oauth_required.is_none());
        assert!(state.allowed_tools.is_empty());
    }

    #[test]
    fn test_session_state_default() {
        let state = SessionState::default();
        assert!(state.skills.is_empty());
        assert!(state.context_tokens_used.is_none());
        assert!(state.allowed_tools.is_empty());
    }

    #[test]
    fn test_add_skill() {
        let mut state = SessionState::new();
        state.add_skill("commit".to_string());
        assert!(state.has_skill("commit"));
        assert_eq!(state.skills.len(), 1);
    }

    #[test]
    fn test_add_skill_deduplication() {
        let mut state = SessionState::new();
        state.add_skill("commit".to_string());
        state.add_skill("commit".to_string());
        assert_eq!(state.skills.len(), 1);
    }

    #[test]
    fn test_add_multiple_skills() {
        let mut state = SessionState::new();
        state.add_skill("commit".to_string());
        state.add_skill("review".to_string());
        state.add_skill("lint".to_string());
        assert_eq!(state.skills.len(), 3);
        assert!(state.has_skill("commit"));
        assert!(state.has_skill("review"));
        assert!(state.has_skill("lint"));
    }

    #[test]
    fn test_remove_skill() {
        let mut state = SessionState::new();
        state.add_skill("commit".to_string());
        state.add_skill("review".to_string());
        state.remove_skill("commit");
        assert!(!state.has_skill("commit"));
        assert!(state.has_skill("review"));
        assert_eq!(state.skills.len(), 1);
    }

    #[test]
    fn test_remove_nonexistent_skill() {
        let mut state = SessionState::new();
        state.add_skill("commit".to_string());
        state.remove_skill("nonexistent");
        assert_eq!(state.skills.len(), 1);
        assert!(state.has_skill("commit"));
    }

    #[test]
    fn test_has_skill_false() {
        let state = SessionState::new();
        assert!(!state.has_skill("commit"));
    }

    #[test]
    fn test_set_context_tokens() {
        let mut state = SessionState::new();
        assert!(state.context_tokens_used.is_none());
        state.set_context_tokens(1000);
        assert_eq!(state.context_tokens_used, Some(1000));
    }

    #[test]
    fn test_update_context_tokens() {
        let mut state = SessionState::new();
        state.set_context_tokens(1000);
        state.set_context_tokens(2000);
        assert_eq!(state.context_tokens_used, Some(2000));
    }

    #[test]
    fn test_pending_permission() {
        let mut state = SessionState::new();
        assert!(!state.has_pending_permission());

        let request = PermissionRequest {
            permission_id: "perm-001".to_string(),
            tool_name: "Bash".to_string(),
            description: "Run npm install".to_string(),
            context: None,
            tool_input: None,
            received_at: Instant::now(),
        };
        state.set_pending_permission(request.clone());

        assert!(state.has_pending_permission());
        assert_eq!(state.pending_permission, Some(request));
    }

    #[test]
    fn test_clear_pending_permission() {
        let mut state = SessionState::new();
        let request = PermissionRequest {
            permission_id: "perm-002".to_string(),
            tool_name: "Bash".to_string(),
            description: "Run npm install".to_string(),
            context: Some("Installing dependencies".to_string()),
            tool_input: None,
            received_at: Instant::now(),
        };
        state.set_pending_permission(request);
        assert!(state.has_pending_permission());

        state.clear_pending_permission();
        assert!(!state.has_pending_permission());
        assert!(state.pending_permission.is_none());
    }

    #[test]
    fn test_oauth_required() {
        let mut state = SessionState::new();
        assert!(!state.needs_oauth());

        state.set_oauth_required("github".to_string(), "git-commit".to_string());
        assert!(state.needs_oauth());
        assert_eq!(
            state.oauth_required,
            Some(("github".to_string(), "git-commit".to_string()))
        );
    }

    #[test]
    fn test_clear_oauth_required() {
        let mut state = SessionState::new();
        state.set_oauth_required("github".to_string(), "git-commit".to_string());
        assert!(state.needs_oauth());

        state.clear_oauth_required();
        assert!(!state.needs_oauth());
        assert!(state.oauth_required.is_none());
    }

    #[test]
    fn test_reset() {
        let mut state = SessionState::new();
        state.add_skill("commit".to_string());
        state.add_skill("review".to_string());
        state.set_context_tokens(5000);
        state.set_pending_permission(PermissionRequest {
            permission_id: "perm-003".to_string(),
            tool_name: "Bash".to_string(),
            description: "Run command".to_string(),
            context: None,
            tool_input: None,
            received_at: Instant::now(),
        });
        state.set_oauth_required("github".to_string(), "skill".to_string());
        state.allow_tool("Bash".to_string());

        state.reset();

        assert!(state.skills.is_empty());
        assert!(state.context_tokens_used.is_none());
        assert!(state.pending_permission.is_none());
        assert!(state.oauth_required.is_none());
        assert!(state.allowed_tools.is_empty());
    }

    #[test]
    fn test_permission_request_equality() {
        let now = Instant::now();
        let req1 = PermissionRequest {
            permission_id: "perm-a".to_string(),
            tool_name: "Bash".to_string(),
            description: "Run command".to_string(),
            context: Some("context".to_string()),
            tool_input: None,
            received_at: now,
        };
        let req2 = PermissionRequest {
            permission_id: "perm-a".to_string(),
            tool_name: "Bash".to_string(),
            description: "Run command".to_string(),
            context: Some("context".to_string()),
            tool_input: None,
            received_at: now,
        };
        let req3 = PermissionRequest {
            permission_id: "perm-b".to_string(),
            tool_name: "Read".to_string(),
            description: "Read file".to_string(),
            context: None,
            tool_input: None,
            received_at: now,
        };

        assert_eq!(req1, req2);
        assert_ne!(req1, req3);
    }

    // ============= Allowed Tools Tests =============

    #[test]
    fn test_allow_tool() {
        let mut state = SessionState::new();
        assert!(!state.is_tool_allowed("Bash"));

        state.allow_tool("Bash".to_string());
        assert!(state.is_tool_allowed("Bash"));
    }

    #[test]
    fn test_allow_tool_deduplication() {
        let mut state = SessionState::new();
        state.allow_tool("Bash".to_string());
        state.allow_tool("Bash".to_string());
        assert_eq!(state.allowed_tools.len(), 1);
    }

    #[test]
    fn test_allow_multiple_tools() {
        let mut state = SessionState::new();
        state.allow_tool("Bash".to_string());
        state.allow_tool("Read".to_string());
        state.allow_tool("Write".to_string());

        assert!(state.is_tool_allowed("Bash"));
        assert!(state.is_tool_allowed("Read"));
        assert!(state.is_tool_allowed("Write"));
        assert!(!state.is_tool_allowed("Edit"));
    }

    #[test]
    fn test_disallow_tool() {
        let mut state = SessionState::new();
        state.allow_tool("Bash".to_string());
        assert!(state.is_tool_allowed("Bash"));

        state.disallow_tool("Bash");
        assert!(!state.is_tool_allowed("Bash"));
    }

    #[test]
    fn test_disallow_nonexistent_tool() {
        let mut state = SessionState::new();
        // Should not panic when disallowing a tool that wasn't allowed
        state.disallow_tool("Nonexistent");
        assert!(!state.is_tool_allowed("Nonexistent"));
    }

    #[test]
    fn test_allowed_tools_persists() {
        let mut state = SessionState::new();
        state.allow_tool("Bash".to_string());

        // Clone state (simulates persistence)
        let state2 = state.clone();
        assert!(state2.is_tool_allowed("Bash"));
    }

    #[test]
    fn test_session_state_serialization() {
        let mut state = SessionState::new();
        state.add_skill("commit".to_string());
        state.set_context_tokens(1234);

        let json = serde_json::to_string(&state).expect("Failed to serialize");
        let deserialized: SessionState =
            serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(state.skills, deserialized.skills);
        assert_eq!(state.context_tokens_used, deserialized.context_tokens_used);
    }

    #[test]
    fn test_set_context_token_limit() {
        let mut state = SessionState::new();
        assert!(state.context_token_limit.is_none());

        state.set_context_token_limit(100_000);
        assert_eq!(state.context_token_limit, Some(100_000));

        state.set_context_token_limit(200_000);
        assert_eq!(state.context_token_limit, Some(200_000));
    }

    #[test]
    fn test_oauth_url_handling() {
        let mut state = SessionState::new();
        assert!(state.oauth_url.is_none());

        state.set_oauth_url("https://oauth.example.com/consent".to_string());
        assert_eq!(
            state.oauth_url,
            Some("https://oauth.example.com/consent".to_string())
        );

        state.clear_oauth_url();
        assert!(state.oauth_url.is_none());
    }

    #[test]
    fn test_reset_clears_all_new_fields() {
        let mut state = SessionState::new();
        state.add_skill("test-skill".to_string());
        state.set_context_tokens(5000);
        state.set_context_token_limit(100_000);
        state.set_oauth_url("https://example.com".to_string());
        state.set_oauth_required("github".to_string(), "skill".to_string());
        state.allow_tool("Bash".to_string());

        state.reset();

        assert!(state.skills.is_empty());
        assert!(state.context_tokens_used.is_none());
        assert!(state.context_token_limit.is_none());
        assert!(state.oauth_url.is_none());
        assert!(state.oauth_required.is_none());
        assert!(state.allowed_tools.is_empty());
    }

    #[test]
    fn test_context_tokens_and_limit_together() {
        let mut state = SessionState::new();
        state.set_context_tokens(45_000);
        state.set_context_token_limit(100_000);

        assert_eq!(state.context_tokens_used, Some(45_000));
        assert_eq!(state.context_token_limit, Some(100_000));
    }

    #[test]
    fn test_oauth_with_url() {
        let mut state = SessionState::new();
        state.set_oauth_required("google".to_string(), "calendar".to_string());
        state.set_oauth_url("https://accounts.google.com/consent".to_string());

        assert!(state.needs_oauth());
        assert_eq!(
            state.oauth_required,
            Some(("google".to_string(), "calendar".to_string()))
        );
        assert_eq!(
            state.oauth_url,
            Some("https://accounts.google.com/consent".to_string())
        );
    }

    // ============= AskUserQuestion Tests =============

    #[test]
    fn test_question_option_creation() {
        let option = QuestionOption {
            label: "Option A".to_string(),
            description: "Description of A".to_string(),
        };
        assert_eq!(option.label, "Option A");
        assert_eq!(option.description, "Description of A");
    }

    #[test]
    fn test_question_creation() {
        let question = Question {
            question: "Which library should we use?".to_string(),
            header: "Auth method".to_string(),
            options: vec![
                QuestionOption {
                    label: "Option A".to_string(),
                    description: "Description of A".to_string(),
                },
                QuestionOption {
                    label: "Option B".to_string(),
                    description: "Description of B".to_string(),
                },
            ],
            multi_select: false,
        };
        assert_eq!(question.question, "Which library should we use?");
        assert_eq!(question.header, "Auth method");
        assert_eq!(question.options.len(), 2);
        assert!(!question.multi_select);
    }

    #[test]
    fn test_ask_user_question_data_creation() {
        let data = AskUserQuestionData {
            questions: vec![Question {
                question: "Which library?".to_string(),
                header: "Library".to_string(),
                options: vec![
                    QuestionOption {
                        label: "A".to_string(),
                        description: "Desc A".to_string(),
                    },
                ],
                multi_select: false,
            }],
            answers: HashMap::new(),
        };
        assert_eq!(data.questions.len(), 1);
        assert!(data.answers.is_empty());
    }

    #[test]
    fn test_ask_user_question_data_with_answers() {
        let mut answers = HashMap::new();
        answers.insert("q1".to_string(), "answer1".to_string());

        let data = AskUserQuestionData {
            questions: vec![],
            answers,
        };
        assert_eq!(data.answers.get("q1"), Some(&"answer1".to_string()));
    }

    #[test]
    fn test_question_option_serialization() {
        let option = QuestionOption {
            label: "Test Label".to_string(),
            description: "Test Description".to_string(),
        };

        let json = serde_json::to_string(&option).expect("Failed to serialize");
        let deserialized: QuestionOption =
            serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(option, deserialized);
    }

    #[test]
    fn test_question_serialization_multi_select_rename() {
        let question = Question {
            question: "Test?".to_string(),
            header: "Test".to_string(),
            options: vec![],
            multi_select: true,
        };

        let json = serde_json::to_string(&question).expect("Failed to serialize");
        // Verify camelCase is used in JSON
        assert!(json.contains("multiSelect"));
        assert!(!json.contains("multi_select"));
    }

    #[test]
    fn test_ask_user_question_data_deserialization() {
        let json = r#"{
            "questions": [
                {
                    "question": "Which library should we use?",
                    "header": "Auth method",
                    "options": [
                        {"label": "Option A", "description": "Description of A"},
                        {"label": "Option B", "description": "Description of B"}
                    ],
                    "multiSelect": false
                }
            ],
            "answers": {}
        }"#;

        let data: AskUserQuestionData =
            serde_json::from_str(json).expect("Failed to deserialize");

        assert_eq!(data.questions.len(), 1);
        let q = &data.questions[0];
        assert_eq!(q.question, "Which library should we use?");
        assert_eq!(q.header, "Auth method");
        assert_eq!(q.options.len(), 2);
        assert_eq!(q.options[0].label, "Option A");
        assert_eq!(q.options[0].description, "Description of A");
        assert_eq!(q.options[1].label, "Option B");
        assert!(!q.multi_select);
        assert!(data.answers.is_empty());
    }

    #[test]
    fn test_ask_user_question_data_deserialization_multi_select_default() {
        // Test that multiSelect defaults to false when not present
        let json = r#"{
            "questions": [
                {
                    "question": "Test?",
                    "header": "Test",
                    "options": []
                }
            ]
        }"#;

        let data: AskUserQuestionData =
            serde_json::from_str(json).expect("Failed to deserialize");

        assert!(!data.questions[0].multi_select);
        assert!(data.answers.is_empty()); // answers should default to empty
    }

    #[test]
    fn test_ask_user_question_data_deserialization_multi_select_true() {
        let json = r#"{
            "questions": [
                {
                    "question": "Select features",
                    "header": "Features",
                    "options": [
                        {"label": "Feature A", "description": "Enables A"},
                        {"label": "Feature B", "description": "Enables B"}
                    ],
                    "multiSelect": true
                }
            ],
            "answers": {}
        }"#;

        let data: AskUserQuestionData =
            serde_json::from_str(json).expect("Failed to deserialize");

        assert!(data.questions[0].multi_select);
    }

    #[test]
    fn test_ask_user_question_data_multiple_questions() {
        let json = r#"{
            "questions": [
                {
                    "question": "First question?",
                    "header": "Q1",
                    "options": [{"label": "A", "description": "a"}],
                    "multiSelect": false
                },
                {
                    "question": "Second question?",
                    "header": "Q2",
                    "options": [{"label": "B", "description": "b"}],
                    "multiSelect": true
                }
            ],
            "answers": {"prev": "value"}
        }"#;

        let data: AskUserQuestionData =
            serde_json::from_str(json).expect("Failed to deserialize");

        assert_eq!(data.questions.len(), 2);
        assert_eq!(data.questions[0].header, "Q1");
        assert_eq!(data.questions[1].header, "Q2");
        assert!(!data.questions[0].multi_select);
        assert!(data.questions[1].multi_select);
        assert_eq!(data.answers.get("prev"), Some(&"value".to_string()));
    }

    #[test]
    fn test_question_equality() {
        let q1 = Question {
            question: "Test?".to_string(),
            header: "Test".to_string(),
            options: vec![QuestionOption {
                label: "A".to_string(),
                description: "B".to_string(),
            }],
            multi_select: false,
        };
        let q2 = q1.clone();

        assert_eq!(q1, q2);
    }

    // ============= AskUserQuestionState Tests =============

    #[test]
    fn test_ask_user_question_state_new() {
        let state = AskUserQuestionState::new(3, &[2, 3, 4]);

        assert_eq!(state.tab_index, 0);
        assert_eq!(state.selections.len(), 3);
        assert_eq!(state.selections[0], Some(0)); // First option selected by default
        assert_eq!(state.selections[1], Some(0));
        assert_eq!(state.selections[2], Some(0));
        assert_eq!(state.other_texts.len(), 3);
        assert!(state.other_texts.iter().all(|t| t.is_empty()));
        assert!(!state.other_active);
        assert_eq!(state.multi_selections.len(), 3);
        assert_eq!(state.multi_selections[0].len(), 2);
        assert_eq!(state.multi_selections[1].len(), 3);
        assert_eq!(state.multi_selections[2].len(), 4);
    }

    #[test]
    fn test_ask_user_question_state_default() {
        let state = AskUserQuestionState::default();

        assert_eq!(state.tab_index, 0);
        assert!(state.selections.is_empty());
        assert!(state.other_texts.is_empty());
        assert!(!state.other_active);
        assert!(state.multi_selections.is_empty());
    }

    #[test]
    fn test_ask_user_question_state_from_data() {
        let data = AskUserQuestionData {
            questions: vec![
                Question {
                    question: "Q1?".to_string(),
                    header: "Q1".to_string(),
                    options: vec![
                        QuestionOption {
                            label: "A".to_string(),
                            description: "".to_string(),
                        },
                        QuestionOption {
                            label: "B".to_string(),
                            description: "".to_string(),
                        },
                    ],
                    multi_select: false,
                },
                Question {
                    question: "Q2?".to_string(),
                    header: "Q2".to_string(),
                    options: vec![QuestionOption {
                        label: "C".to_string(),
                        description: "".to_string(),
                    }],
                    multi_select: true,
                },
            ],
            answers: HashMap::new(),
        };

        let state = AskUserQuestionState::from_data(&data);

        assert_eq!(state.tab_index, 0);
        assert_eq!(state.selections.len(), 2);
        assert_eq!(state.other_texts.len(), 2);
        assert_eq!(state.multi_selections.len(), 2);
        assert_eq!(state.multi_selections[0].len(), 2);
        assert_eq!(state.multi_selections[1].len(), 1);
    }

    #[test]
    fn test_ask_user_question_state_reset() {
        let mut state = AskUserQuestionState::new(2, &[3, 2]);
        state.tab_index = 1;
        state.selections[0] = None;
        state.other_texts[0] = "custom text".to_string();
        state.other_active = true;
        state.multi_selections[0][0] = true;

        state.reset();

        assert_eq!(state.tab_index, 0);
        assert!(state.selections.is_empty());
        assert!(state.other_texts.is_empty());
        assert!(!state.other_active);
        assert!(state.multi_selections.is_empty());
    }

    #[test]
    fn test_ask_user_question_state_current_selection() {
        let mut state = AskUserQuestionState::new(3, &[2, 3, 4]);

        // First tab, default selection
        assert_eq!(state.current_selection(), Some(0));

        // Change selection
        state.selections[0] = Some(1);
        assert_eq!(state.current_selection(), Some(1));

        // Select "Other"
        state.selections[0] = None;
        assert_eq!(state.current_selection(), None);

        // Move to second tab
        state.tab_index = 1;
        assert_eq!(state.current_selection(), Some(0));
    }

    #[test]
    fn test_ask_user_question_state_current_other_text() {
        let mut state = AskUserQuestionState::new(2, &[2, 2]);

        assert_eq!(state.current_other_text(), "");

        state.other_texts[0] = "custom answer".to_string();
        assert_eq!(state.current_other_text(), "custom answer");

        state.tab_index = 1;
        assert_eq!(state.current_other_text(), "");
    }

    #[test]
    fn test_ask_user_question_state_set_current_selection() {
        let mut state = AskUserQuestionState::new(2, &[3, 3]);

        state.set_current_selection(Some(2));
        assert_eq!(state.selections[0], Some(2));

        state.set_current_selection(None);
        assert_eq!(state.selections[0], None);

        state.tab_index = 1;
        state.set_current_selection(Some(1));
        assert_eq!(state.selections[1], Some(1));
    }

    #[test]
    fn test_ask_user_question_state_toggle_multi_selection() {
        let mut state = AskUserQuestionState::new(2, &[3, 2]);

        // Initially all false
        assert!(!state.is_multi_selected(0));
        assert!(!state.is_multi_selected(1));
        assert!(!state.is_multi_selected(2));

        // Toggle first option
        state.toggle_multi_selection(0);
        assert!(state.is_multi_selected(0));

        // Toggle second option
        state.toggle_multi_selection(1);
        assert!(state.is_multi_selected(1));

        // Toggle first option again (off)
        state.toggle_multi_selection(0);
        assert!(!state.is_multi_selected(0));

        // Switch tabs and verify independence
        state.tab_index = 1;
        assert!(!state.is_multi_selected(0));
        state.toggle_multi_selection(0);
        assert!(state.is_multi_selected(0));
    }

    #[test]
    fn test_ask_user_question_state_is_multi_selected_out_of_bounds() {
        let state = AskUserQuestionState::new(1, &[2]);

        // Out of bounds option
        assert!(!state.is_multi_selected(5));

        // Out of bounds tab
        let empty_state = AskUserQuestionState::default();
        assert!(!empty_state.is_multi_selected(0));
    }

    #[test]
    fn test_ask_user_question_state_push_pop_other_char() {
        let mut state = AskUserQuestionState::new(2, &[2, 2]);

        state.push_other_char('H');
        state.push_other_char('i');
        assert_eq!(state.current_other_text(), "Hi");

        state.pop_other_char();
        assert_eq!(state.current_other_text(), "H");

        state.pop_other_char();
        assert_eq!(state.current_other_text(), "");

        // Pop on empty string should not panic
        state.pop_other_char();
        assert_eq!(state.current_other_text(), "");
    }

    #[test]
    fn test_ask_user_question_state_next_prev_tab() {
        let mut state = AskUserQuestionState::new(3, &[2, 2, 2]);

        assert_eq!(state.tab_index, 0);

        state.next_tab(3);
        assert_eq!(state.tab_index, 1);

        state.next_tab(3);
        assert_eq!(state.tab_index, 2);

        // Wrap around
        state.next_tab(3);
        assert_eq!(state.tab_index, 0);

        // Previous tab
        state.prev_tab(3);
        assert_eq!(state.tab_index, 2);

        state.prev_tab(3);
        assert_eq!(state.tab_index, 1);

        state.prev_tab(3);
        assert_eq!(state.tab_index, 0);
    }

    #[test]
    fn test_ask_user_question_state_tab_navigation_zero_questions() {
        let mut state = AskUserQuestionState::default();

        // Should not panic with zero questions
        state.next_tab(0);
        assert_eq!(state.tab_index, 0);

        state.prev_tab(0);
        assert_eq!(state.tab_index, 0);
    }

    #[test]
    fn test_ask_user_question_state_clone() {
        let mut state = AskUserQuestionState::new(2, &[3, 2]);
        state.tab_index = 1;
        state.selections[0] = Some(2);
        state.other_texts[0] = "custom".to_string();
        state.other_active = true;
        state.multi_selections[0][1] = true;

        let cloned = state.clone();

        assert_eq!(cloned.tab_index, 1);
        assert_eq!(cloned.selections[0], Some(2));
        assert_eq!(cloned.other_texts[0], "custom");
        assert!(cloned.other_active);
        assert!(cloned.multi_selections[0][1]);
    }

    // ============= Answered Tracking Tests =============

    #[test]
    fn test_ask_user_question_state_answered_initialized() {
        let state = AskUserQuestionState::new(3, &[2, 2, 2]);
        assert_eq!(state.answered.len(), 3);
        assert!(state.answered.iter().all(|&a| !a)); // All false initially
    }

    #[test]
    fn test_ask_user_question_state_mark_current_answered() {
        let mut state = AskUserQuestionState::new(3, &[2, 2, 2]);

        // Mark first question as answered
        state.mark_current_answered();
        assert!(state.answered[0]);
        assert!(!state.answered[1]);
        assert!(!state.answered[2]);

        // Move to second question and mark
        state.tab_index = 1;
        state.mark_current_answered();
        assert!(state.answered[0]);
        assert!(state.answered[1]);
        assert!(!state.answered[2]);
    }

    #[test]
    fn test_ask_user_question_state_is_current_answered() {
        let mut state = AskUserQuestionState::new(2, &[2, 2]);

        assert!(!state.is_current_answered());

        state.answered[0] = true;
        assert!(state.is_current_answered());

        state.tab_index = 1;
        assert!(!state.is_current_answered());
    }

    #[test]
    fn test_ask_user_question_state_all_answered() {
        let mut state = AskUserQuestionState::new(3, &[2, 2, 2]);

        assert!(!state.all_answered()); // None answered

        state.answered[0] = true;
        assert!(!state.all_answered()); // Only one answered

        state.answered[1] = true;
        assert!(!state.all_answered()); // Two answered

        state.answered[2] = true;
        assert!(state.all_answered()); // All answered
    }

    #[test]
    fn test_ask_user_question_state_all_answered_empty() {
        let state = AskUserQuestionState::default();
        assert!(!state.all_answered()); // Empty state should return false
    }

    #[test]
    fn test_ask_user_question_state_first_unanswered() {
        let mut state = AskUserQuestionState::new(3, &[2, 2, 2]);

        assert_eq!(state.first_unanswered(), Some(0));

        state.answered[0] = true;
        assert_eq!(state.first_unanswered(), Some(1));

        state.answered[1] = true;
        assert_eq!(state.first_unanswered(), Some(2));

        state.answered[2] = true;
        assert_eq!(state.first_unanswered(), None); // All answered
    }

    #[test]
    fn test_ask_user_question_state_advance_to_next_unanswered() {
        let mut state = AskUserQuestionState::new(3, &[2, 2, 2]);

        // From tab 0, advance to tab 1
        state.answered[0] = true;
        let advanced = state.advance_to_next_unanswered(3);
        assert!(advanced);
        assert_eq!(state.tab_index, 1);

        // From tab 1, advance to tab 2
        state.answered[1] = true;
        let advanced = state.advance_to_next_unanswered(3);
        assert!(advanced);
        assert_eq!(state.tab_index, 2);

        // All answered - no more to advance
        state.answered[2] = true;
        let advanced = state.advance_to_next_unanswered(3);
        assert!(!advanced);
        assert_eq!(state.tab_index, 2); // Should stay at current
    }

    #[test]
    fn test_ask_user_question_state_advance_wraps_around() {
        let mut state = AskUserQuestionState::new(3, &[2, 2, 2]);

        // Answer tabs 1 and 2, current at 2
        state.tab_index = 2;
        state.answered[1] = true;
        state.answered[2] = true;

        // Should wrap to tab 0
        let advanced = state.advance_to_next_unanswered(3);
        assert!(advanced);
        assert_eq!(state.tab_index, 0);
    }

    #[test]
    fn test_ask_user_question_state_reset_clears_answered() {
        let mut state = AskUserQuestionState::new(2, &[2, 2]);
        state.answered[0] = true;
        state.answered[1] = true;

        state.reset();

        assert!(state.answered.is_empty());
    }

    #[test]
    fn test_ask_user_question_state_clone_includes_answered() {
        let mut state = AskUserQuestionState::new(2, &[2, 2]);
        state.answered[0] = true;

        let cloned = state.clone();
        assert!(cloned.answered[0]);
        assert!(!cloned.answered[1]);
    }
}
