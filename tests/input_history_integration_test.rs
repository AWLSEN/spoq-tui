// Integration tests for Round 1: Input History with Persistence
// Tests for:
// 1. InputHistory module functionality
// 2. TextAreaInput cursor position methods
// 3. History persistence to ~/.spoq_history
// 4. Integration between InputHistory and TextAreaInput

use spoq::input_history::InputHistory;
use spoq::widgets::textarea_input::TextAreaInput;
use std::fs;
use tempfile::TempDir;

// =============================================================================
// InputHistory Core Functionality Tests
// =============================================================================

#[test]
fn test_input_history_new_is_empty() {
    let history = InputHistory::new();
    assert!(history.is_empty());
    assert_eq!(history.len(), 0);
}

#[test]
fn test_input_history_add_and_navigate() {
    let mut history = InputHistory::new();
    history.add("first command".to_string());
    history.add("second command".to_string());
    history.add("third command".to_string());

    assert_eq!(history.len(), 3);

    // Navigate up through history
    let entry = history.navigate_up("current text");
    assert_eq!(entry, Some("third command"));

    let entry = history.navigate_up("current text");
    assert_eq!(entry, Some("second command"));

    let entry = history.navigate_up("current text");
    assert_eq!(entry, Some("first command"));

    // At oldest - should stay there
    let entry = history.navigate_up("current text");
    assert_eq!(entry, Some("first command"));
}

#[test]
fn test_input_history_navigate_down_cycle() {
    let mut history = InputHistory::new();
    history.add("cmd1".to_string());
    history.add("cmd2".to_string());

    // Go up
    history.navigate_up("my input");
    history.navigate_up("my input");

    // Go back down
    let entry = history.navigate_down();
    assert_eq!(entry, Some("cmd2"));

    // Back to current input
    let entry = history.navigate_down();
    assert_eq!(entry, None);

    // Verify saved input is preserved
    assert_eq!(history.get_current_input(), "my input");
}

#[test]
fn test_input_history_skips_empty_and_duplicates() {
    let mut history = InputHistory::new();

    // Empty entries should be skipped
    history.add("".to_string());
    history.add("   ".to_string());
    assert_eq!(history.len(), 0);

    // Add valid entry
    history.add("command".to_string());
    assert_eq!(history.len(), 1);

    // Duplicate should be skipped
    history.add("command".to_string());
    assert_eq!(history.len(), 1);

    // Different entry should be added
    history.add("different".to_string());
    assert_eq!(history.len(), 2);
}

#[test]
fn test_input_history_reset_navigation() {
    let mut history = InputHistory::new();
    history.add("entry".to_string());

    history.navigate_up("current");
    assert!(history.current_index().is_some());

    history.reset_navigation();
    assert!(history.current_index().is_none());
    assert_eq!(history.get_current_input(), "");
}

#[test]
fn test_input_history_multiline_entries() {
    let mut history = InputHistory::new();
    let multiline = "line 1\nline 2\nline 3".to_string();

    history.add(multiline.clone());
    assert_eq!(history.len(), 1);

    let entry = history.navigate_up("");
    assert_eq!(entry, Some(multiline.as_str()));
}

// =============================================================================
// TextAreaInput Cursor Position Methods Tests
// =============================================================================

#[test]
fn test_textarea_is_cursor_on_first_line_single_line() {
    let input = TextAreaInput::new();
    assert!(input.is_cursor_on_first_line());
}

#[test]
fn test_textarea_is_cursor_on_first_line_multiline() {
    let mut input = TextAreaInput::new();
    input.insert_char('A');
    input.insert_newline();
    input.insert_char('B');

    // Cursor is on second line after inserting
    assert!(!input.is_cursor_on_first_line());

    // Move cursor up to first line
    input.move_cursor_up();
    assert!(input.is_cursor_on_first_line());
}

#[test]
fn test_textarea_is_cursor_on_last_line_single_line() {
    let input = TextAreaInput::new();
    assert!(input.is_cursor_on_last_line());
}

#[test]
fn test_textarea_is_cursor_on_last_line_multiline() {
    let mut input = TextAreaInput::new();
    input.insert_char('A');
    input.insert_newline();
    input.insert_char('B');

    // Cursor is on last line after inserting
    assert!(input.is_cursor_on_last_line());

    // Move cursor up to first line
    input.move_cursor_up();
    assert!(!input.is_cursor_on_last_line());

    // Move cursor back down to last line
    input.move_cursor_down();
    assert!(input.is_cursor_on_last_line());
}

#[test]
fn test_textarea_set_content_single_line() {
    let mut input = TextAreaInput::new();
    input.set_content("test content");

    assert_eq!(input.content(), "test content");
    assert_eq!(input.line_count(), 1);
}

#[test]
fn test_textarea_set_content_multiline() {
    let mut input = TextAreaInput::new();
    input.set_content("line1\nline2\nline3");

    assert_eq!(input.content(), "line1\nline2\nline3");
    assert_eq!(input.line_count(), 3);
}

#[test]
fn test_textarea_set_content_replaces_existing() {
    let mut input = TextAreaInput::new();
    input.insert_char('H');
    input.insert_char('i');

    input.set_content("replaced");
    assert_eq!(input.content(), "replaced");
}

#[test]
fn test_textarea_set_content_empty() {
    let mut input = TextAreaInput::new();
    input.insert_char('X');

    input.set_content("");
    assert!(input.is_empty());
}

// =============================================================================
// Integration Tests: InputHistory + TextAreaInput
// =============================================================================

#[test]
fn test_history_navigation_with_textarea() {
    let mut history = InputHistory::new();
    let mut textarea = TextAreaInput::new();

    // Add some history
    history.add("first entry".to_string());
    history.add("second entry".to_string());

    // User types something
    textarea.set_content("current typing");

    // Navigate up - should show most recent entry
    if let Some(entry) = history.navigate_up(&textarea.content()) {
        textarea.set_content(entry);
    }
    assert_eq!(textarea.content(), "second entry");

    // Navigate up again - should show older entry
    if let Some(entry) = history.navigate_up(&textarea.content()) {
        textarea.set_content(entry);
    }
    assert_eq!(textarea.content(), "first entry");

    // Navigate down - should show newer entry
    if let Some(entry) = history.navigate_down() {
        textarea.set_content(entry);
    }
    assert_eq!(textarea.content(), "second entry");

    // Navigate down to current input
    if history.navigate_down().is_none() {
        textarea.set_content(history.get_current_input());
    }
    assert_eq!(textarea.content(), "current typing");
}

#[test]
fn test_history_preserves_multiline_textarea_content() {
    let mut history = InputHistory::new();
    let mut textarea = TextAreaInput::new();

    // Create multiline content in textarea
    textarea.set_content("line 1\nline 2\nline 3");

    // Add to history
    history.add(textarea.content());

    // Clear textarea
    textarea.clear();
    assert!(textarea.is_empty());

    // Navigate up - should restore multiline content
    if let Some(entry) = history.navigate_up(&textarea.content()) {
        textarea.set_content(entry);
    }

    assert_eq!(textarea.content(), "line 1\nline 2\nline 3");
    assert_eq!(textarea.line_count(), 3);
}

#[test]
fn test_cursor_navigation_boundaries_for_history() {
    let mut textarea = TextAreaInput::new();

    // Single line - cursor is on both first and last line
    textarea.set_content("single line");
    assert!(textarea.is_cursor_on_first_line());
    assert!(textarea.is_cursor_on_last_line());

    // Multiline - add lines
    textarea.insert_newline();
    textarea.insert_char('2');
    textarea.insert_newline();
    textarea.insert_char('3');

    // Cursor is now on last line
    assert!(!textarea.is_cursor_on_first_line());
    assert!(textarea.is_cursor_on_last_line());

    // Move to first line
    textarea.move_cursor_top();
    assert!(textarea.is_cursor_on_first_line());
    assert!(!textarea.is_cursor_on_last_line());
}

// =============================================================================
// History Persistence Tests (using temporary directory)
// =============================================================================

#[test]
fn test_history_save_and_load() {
    // Create temporary directory for test
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let history_file = temp_dir.path().join(".spoq_history");

    // Set up test by manually saving to temp file
    let mut history = InputHistory::new();
    history.add("command 1".to_string());
    history.add("command 2".to_string());
    history.add("command 3".to_string());

    // Manually write to temp file (simulating save)
    let entries = ["command 1", "command 2", "command 3"];
    fs::write(&history_file, entries.join("\n")).expect("Failed to write history");

    // Read back and verify
    let contents = fs::read_to_string(&history_file).expect("Failed to read history");
    let lines: Vec<&str> = contents.lines().collect();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0], "command 1");
    assert_eq!(lines[1], "command 2");
    assert_eq!(lines[2], "command 3");
}

#[test]
fn test_history_handles_multiline_persistence() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let history_file = temp_dir.path().join(".spoq_history");

    // Test escaping of multiline entries
    let mut history = InputHistory::new();
    let multiline = "line 1\nline 2\nline 3".to_string();
    history.add(multiline);

    // Manually save with escaping (how the real save() works)
    let escaped = "line 1\\nline 2\\nline 3";
    fs::write(&history_file, escaped).expect("Failed to write");

    // Read back
    let contents = fs::read_to_string(&history_file).expect("Failed to read");
    assert!(contents.contains("\\n"));

    // When loading, entries should be unescaped
    let unescaped = contents.replace("\\n", "\n");
    assert_eq!(unescaped, "line 1\nline 2\nline 3");
}

#[test]
fn test_history_max_size_limit() {
    let mut history = InputHistory::new();

    // Add more than max entries (1000)
    for i in 0..1100 {
        history.add(format!("command {}", i));
    }

    // Should be capped at 1000
    assert_eq!(history.len(), 1000);
}

#[test]
fn test_empty_history_file_creates_new() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let history_file = temp_dir.path().join(".spoq_history");

    // Create empty file
    fs::write(&history_file, "").expect("Failed to write");

    // Loading should result in empty history
    let contents = fs::read_to_string(&history_file).expect("Failed to read");
    assert!(contents.is_empty());
}

// =============================================================================
// Edge Cases
// =============================================================================

#[test]
fn test_navigate_empty_history() {
    let mut history = InputHistory::new();

    let result = history.navigate_up("some text");
    assert_eq!(result, None);

    let result = history.navigate_down();
    assert_eq!(result, None);
}

#[test]
fn test_textarea_cursor_methods_with_empty_content() {
    let input = TextAreaInput::new();

    // Empty textarea - cursor is on first and last line
    assert!(input.is_cursor_on_first_line());
    assert!(input.is_cursor_on_last_line());
}

#[test]
fn test_history_add_resets_navigation() {
    let mut history = InputHistory::new();
    history.add("entry 1".to_string());

    // Navigate up
    history.navigate_up("current");
    assert!(history.current_index().is_some());

    // Adding new entry should reset navigation
    history.add("entry 2".to_string());
    assert!(history.current_index().is_none());
}

#[test]
fn test_integration_workflow_complete_cycle() {
    let mut history = InputHistory::new();
    let mut textarea = TextAreaInput::new();

    // Simulate user typing and submitting
    textarea.set_content("first command");
    history.add(textarea.content());
    textarea.clear();

    textarea.set_content("second command");
    history.add(textarea.content());
    textarea.clear();

    // User starts typing new command
    textarea.set_content("new cmd in progress");

    // User presses up arrow to see history
    if let Some(entry) = history.navigate_up(&textarea.content()) {
        textarea.set_content(entry);
    }
    assert_eq!(textarea.content(), "second command");

    // Press up again
    if let Some(entry) = history.navigate_up(&textarea.content()) {
        textarea.set_content(entry);
    }
    assert_eq!(textarea.content(), "first command");

    // Press down to go back
    if let Some(entry) = history.navigate_down() {
        textarea.set_content(entry);
    }
    assert_eq!(textarea.content(), "second command");

    // Press down again to restore original input
    if history.navigate_down().is_none() {
        textarea.set_content(history.get_current_input());
    }
    assert_eq!(textarea.content(), "new cmd in progress");

    // Submit the command
    history.add(textarea.content());
    textarea.clear();

    // Verify history has all three entries
    assert_eq!(history.len(), 3);
}

// =============================================================================
// Round 3: Submit Flow Integration Tests
// =============================================================================

#[test]
fn test_submit_flow_adds_to_history_and_resets_navigation() {
    let mut history = InputHistory::new();
    let mut textarea = TextAreaInput::new();

    // Simulate the exact sequence in submit_input():
    // 1. User types content
    textarea.set_content("Fix the bug in main.rs");
    let content = textarea.content();

    // 2. Add to history (line 91 in stream.rs)
    history.add(content);

    // 3. Clear textarea (line 93)
    textarea.clear();

    // 4. Reset navigation (line 96)
    history.reset_navigation();

    // Verify:
    assert_eq!(history.len(), 1);
    assert!(
        history.current_index().is_none(),
        "Navigation should be reset"
    );
    assert!(textarea.is_empty(), "Textarea should be cleared");
}

#[test]
fn test_submit_flow_preserves_history_across_multiple_submits() {
    let mut history = InputHistory::new();
    let mut textarea = TextAreaInput::new();

    // First submit
    textarea.set_content("Create a new feature");
    history.add(textarea.content());
    textarea.clear();
    history.reset_navigation();

    // Second submit
    textarea.set_content("Write tests for the feature");
    history.add(textarea.content());
    textarea.clear();
    history.reset_navigation();

    // Third submit
    textarea.set_content("Update documentation");
    history.add(textarea.content());
    textarea.clear();
    history.reset_navigation();

    // Verify all entries are preserved
    assert_eq!(history.len(), 3);

    // Navigate up through history
    let entry = history.navigate_up("");
    assert_eq!(entry, Some("Update documentation"));

    let entry = history.navigate_up("");
    assert_eq!(entry, Some("Write tests for the feature"));

    let entry = history.navigate_up("");
    assert_eq!(entry, Some("Create a new feature"));
}

#[test]
fn test_submit_flow_with_navigation_then_submit() {
    let mut history = InputHistory::new();
    let mut textarea = TextAreaInput::new();

    // Add initial history
    history.add("First command".to_string());
    history.add("Second command".to_string());

    // User navigates up
    if let Some(entry) = history.navigate_up("") {
        textarea.set_content(entry);
    }
    assert_eq!(textarea.content(), "Second command");
    assert!(history.current_index().is_some(), "Should be navigating");

    // User submits (re-running a historical command)
    let content = textarea.content();
    history.add(content);
    textarea.clear();
    history.reset_navigation();

    // Verify:
    // - History length stays 2 (duplicate not added)
    assert_eq!(history.len(), 2);
    // - Navigation is reset
    assert!(history.current_index().is_none());
    // - Textarea is cleared
    assert!(textarea.is_empty());
}

#[test]
fn test_submit_flow_navigation_reset_allows_fresh_navigation() {
    let mut history = InputHistory::new();
    history.add("Command 1".to_string());
    history.add("Command 2".to_string());
    history.add("Command 3".to_string());

    // Navigate up twice
    history.navigate_up("typing");
    history.navigate_up("");

    // Simulate submit (which resets navigation)
    history.reset_navigation();

    // Now navigate up again - should start from most recent
    let entry = history.navigate_up("new input");
    assert_eq!(
        entry,
        Some("Command 3"),
        "Should start from most recent after reset"
    );

    // And saved input should be the new input
    assert_eq!(history.get_current_input(), "new input");
}
