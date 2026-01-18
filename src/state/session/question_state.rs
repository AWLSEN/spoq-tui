//! AskUserQuestion UI state machine
//!
//! Contains the state machine for the AskUserQuestion prompt UI.
//! This tracks current tab, selections, "Other" text input, and
//! multi-select toggle states.

use super::AskUserQuestionData;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::session::{Question, QuestionOption};
    use std::collections::HashMap;

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
