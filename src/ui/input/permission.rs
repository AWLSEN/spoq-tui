//! Permission prompt rendering.
//!
//! Implements the permission prompt UI including the AskUserQuestion dialog.

use crate::state::session::{AskUserQuestionData, PermissionRequest};

// ============================================================================
// Permission Box Constants
// ============================================================================

/// Minimum width for the permission box (must fit keyboard options)
pub const MIN_PERMISSION_BOX_WIDTH: u16 = 30;
/// Default/maximum width for the permission box
pub const DEFAULT_PERMISSION_BOX_WIDTH: u16 = 60;
/// Default height for the permission box
pub const DEFAULT_PERMISSION_BOX_HEIGHT: u16 = 10;
/// Minimum height for a compact permission box (skips preview)
pub const MIN_PERMISSION_BOX_HEIGHT: u16 = 6;

// ============================================================================
// AskUserQuestion Parsing
// ============================================================================

/// Parse AskUserQuestion tool input into structured data.
///
/// Attempts to deserialize the tool_input JSON value into an `AskUserQuestionData`.
/// Returns `None` if the input doesn't match the expected structure.
pub fn parse_ask_user_question(tool_input: &serde_json::Value) -> Option<AskUserQuestionData> {
    serde_json::from_value(tool_input.clone()).ok()
}


/// Extract preview content from a PermissionRequest.
pub fn get_permission_preview(perm: &PermissionRequest) -> String {
    // First try context (human-readable description)
    if let Some(ref ctx) = perm.context {
        return ctx.clone();
    }

    // Fall back to tool_input if available
    if let Some(ref input) = perm.tool_input {
        // Try to extract common fields
        if let Some(path) = input.get("file_path").and_then(|v| v.as_str()) {
            return path.to_string();
        }
        if let Some(cmd) = input.get("command").and_then(|v| v.as_str()) {
            return cmd.to_string();
        }
        if let Some(content) = input.get("content").and_then(|v| v.as_str()) {
            // Truncate long content (respecting UTF-8 boundaries)
            if content.len() > 100 {
                return super::super::helpers::truncate_string(content, 100);
            }
            return content.to_string();
        }
        // Fallback: pretty print JSON
        if let Ok(pretty) = serde_json::to_string_pretty(input) {
            return pretty;
        }
    }

    String::new()
}


#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_ask_user_question_valid() {
        let input = json!({
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
        });

        let result = parse_ask_user_question(&input);
        assert!(result.is_some());

        let data = result.unwrap();
        assert_eq!(data.questions.len(), 1);
        assert_eq!(data.questions[0].question, "Which library should we use?");
        assert_eq!(data.questions[0].header, "Auth method");
        assert_eq!(data.questions[0].options.len(), 2);
        assert!(!data.questions[0].multi_select);
    }

    #[test]
    fn test_parse_ask_user_question_multi_select() {
        let input = json!({
            "questions": [
                {
                    "question": "Select features",
                    "header": "Features",
                    "options": [
                        {"label": "A", "description": "Feature A"},
                        {"label": "B", "description": "Feature B"}
                    ],
                    "multiSelect": true
                }
            ],
            "answers": {}
        });

        let result = parse_ask_user_question(&input);
        assert!(result.is_some());
        assert!(result.unwrap().questions[0].multi_select);
    }

    #[test]
    fn test_parse_ask_user_question_missing_multi_select_defaults() {
        let input = json!({
            "questions": [
                {
                    "question": "Test?",
                    "header": "Test",
                    "options": []
                }
            ]
        });

        let result = parse_ask_user_question(&input);
        assert!(result.is_some());
        assert!(!result.unwrap().questions[0].multi_select);
    }

    #[test]
    fn test_parse_ask_user_question_with_answers() {
        let input = json!({
            "questions": [
                {
                    "question": "Test?",
                    "header": "Test",
                    "options": [{"label": "A", "description": "a"}],
                    "multiSelect": false
                }
            ],
            "answers": {"q1": "answer1"}
        });

        let result = parse_ask_user_question(&input);
        assert!(result.is_some());
        let data = result.unwrap();
        assert_eq!(data.answers.get("q1"), Some(&"answer1".to_string()));
    }

    #[test]
    fn test_parse_ask_user_question_multiple_questions() {
        let input = json!({
            "questions": [
                {
                    "question": "First?",
                    "header": "Q1",
                    "options": [{"label": "A", "description": "a"}],
                    "multiSelect": false
                },
                {
                    "question": "Second?",
                    "header": "Q2",
                    "options": [{"label": "B", "description": "b"}],
                    "multiSelect": true
                }
            ],
            "answers": {}
        });

        let result = parse_ask_user_question(&input);
        assert!(result.is_some());
        let data = result.unwrap();
        assert_eq!(data.questions.len(), 2);
        assert_eq!(data.questions[0].header, "Q1");
        assert_eq!(data.questions[1].header, "Q2");
    }

    #[test]
    fn test_parse_ask_user_question_invalid_missing_questions() {
        let input = json!({
            "answers": {}
        });

        let result = parse_ask_user_question(&input);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_ask_user_question_invalid_wrong_type() {
        let input = json!({
            "questions": "not an array",
            "answers": {}
        });

        let result = parse_ask_user_question(&input);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_ask_user_question_invalid_empty_object() {
        let input = json!({});

        let result = parse_ask_user_question(&input);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_ask_user_question_invalid_null() {
        let input = json!(null);

        let result = parse_ask_user_question(&input);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_ask_user_question_invalid_missing_required_question_fields() {
        let input = json!({
            "questions": [
                {
                    "header": "Test"
                    // missing "question" and "options"
                }
            ],
            "answers": {}
        });

        let result = parse_ask_user_question(&input);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_ask_user_question_completely_unrelated_json() {
        let input = json!({
            "command": "npm install",
            "file_path": "/some/path"
        });

        let result = parse_ask_user_question(&input);
        assert!(result.is_none());
    }

    // ========================================================================
    // Permission Box Responsive Tests
    // ========================================================================

    #[test]
    fn test_permission_box_width_normal() {
        let ctx = LayoutContext::new(120, 40);
        let area = Rect::new(0, 0, 120, 40);

        // For normal width, box should be 60 chars (DEFAULT_PERMISSION_BOX_WIDTH)
        let available = area.width.saturating_sub(4);
        let expected_width = DEFAULT_PERMISSION_BOX_WIDTH.min(available);

        assert_eq!(expected_width, 60);
        // Verify ctx is not narrow or extra small
        assert!(!ctx.is_narrow());
        assert!(!ctx.is_extra_small());
    }

    #[test]
    fn test_permission_box_width_narrow() {
        let ctx = LayoutContext::new(70, 24);

        // Narrow terminals should scale down
        let scaled = ctx.bounded_width(70, MIN_PERMISSION_BOX_WIDTH, DEFAULT_PERMISSION_BOX_WIDTH);

        // 70% of 70 = 49, clamped between 30 and 60
        assert!(scaled >= MIN_PERMISSION_BOX_WIDTH);
        assert!(scaled <= DEFAULT_PERMISSION_BOX_WIDTH);
    }

    #[test]
    fn test_permission_box_width_extra_small() {
        let ctx = LayoutContext::new(50, 24);

        // Extra small should use minimum width
        let available = 50u16.saturating_sub(4);
        let expected_width = MIN_PERMISSION_BOX_WIDTH.min(available);

        assert_eq!(expected_width, MIN_PERMISSION_BOX_WIDTH);
        assert!(ctx.is_extra_small());
    }

    #[test]
    fn test_permission_box_title_changes_on_narrow() {
        // On normal width, title is " Permission Required "
        // On narrow width, title is " Permission "
        let normal_ctx = LayoutContext::new(120, 40);
        let narrow_ctx = LayoutContext::new(70, 24);

        assert!(!normal_ctx.is_narrow());
        assert!(narrow_ctx.is_narrow());
    }

    #[test]
    fn test_permission_box_preview_hidden_on_short() {
        let ctx = LayoutContext::new(80, 20);

        // SM_HEIGHT is 24, so height < 24 means is_short() returns true
        assert!(ctx.is_short()); // 20 < 24

        // Preview should be hidden on short terminals
        let show_preview = !ctx.is_short();
        assert!(!show_preview, "Preview should be hidden on short terminals");
    }

    #[test]
    fn test_permission_box_keyboard_options_normal() {
        // Normal: [y] Yes  [a] Always  [n] No
        let ctx = LayoutContext::new(120, 40);
        assert!(!ctx.is_narrow());
        assert!(!ctx.is_extra_small());
    }

    #[test]
    fn test_permission_box_keyboard_options_narrow() {
        // Narrow: [y] Y  [a] A  [n] N
        let ctx = LayoutContext::new(70, 24);
        assert!(ctx.is_narrow());
        assert!(!ctx.is_extra_small());
    }

    #[test]
    fn test_permission_box_keyboard_options_extra_small() {
        // Extra small: [y]/[a]/[n]
        let ctx = LayoutContext::new(50, 24);
        assert!(ctx.is_extra_small());
    }
}
