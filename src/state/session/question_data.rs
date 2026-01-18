//! Question data structures for AskUserQuestion tool
//!
//! Contains the data structures that represent questions from the
//! AskUserQuestion tool. These are deserialized from the backend
//! and used to populate the question prompt UI.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

#[cfg(test)]
mod tests {
    use super::*;

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
                options: vec![QuestionOption {
                    label: "A".to_string(),
                    description: "Desc A".to_string(),
                }],
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

        let data: AskUserQuestionData = serde_json::from_str(json).expect("Failed to deserialize");

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

        let data: AskUserQuestionData = serde_json::from_str(json).expect("Failed to deserialize");

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

        let data: AskUserQuestionData = serde_json::from_str(json).expect("Failed to deserialize");

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

        let data: AskUserQuestionData = serde_json::from_str(json).expect("Failed to deserialize");

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
}
