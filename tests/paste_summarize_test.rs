//! Tests for paste text summarization threshold check

use spoq::app::App;
use spoq::cache::ThreadCache;
use spoq::conductor::ConductorClient;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Helper to create a minimal App instance for testing
fn create_test_app() -> App {
    let (message_tx, message_rx) = mpsc::unbounded_channel();
    let cache = ThreadCache::new();
    let client = Arc::new(ConductorClient::new("http://localhost:8080"));

    App::new_with_config(
        cache,
        client,
        message_tx,
        Some(message_rx),
        None, // no debug tx for tests
    )
}

#[test]
fn test_should_summarize_paste_short_text() {
    let app = create_test_app();

    // Short text, 1 line, < 150 chars
    let text = "Hello world";
    assert!(!app.should_summarize_paste(text), "Short text should not be summarized");
}

#[test]
fn test_should_summarize_paste_exactly_150_chars() {
    let app = create_test_app();

    // Exactly 150 chars, 1 line
    let text = "a".repeat(150);
    assert!(!app.should_summarize_paste(&text), "Exactly 150 chars should not be summarized");
}

#[test]
fn test_should_summarize_paste_151_chars() {
    let app = create_test_app();

    // 151 chars, 1 line - should trigger summarization
    let text = "a".repeat(151);
    assert!(app.should_summarize_paste(&text), "151 chars should be summarized");
}

#[test]
fn test_should_summarize_paste_exactly_3_lines() {
    let app = create_test_app();

    // Exactly 3 lines - should not trigger
    let text = "line1\nline2\nline3";
    assert!(!app.should_summarize_paste(text), "Exactly 3 lines should not be summarized");
}

#[test]
fn test_should_summarize_paste_4_lines() {
    let app = create_test_app();

    // 4 lines - should trigger
    let text = "line1\nline2\nline3\nline4";
    assert!(app.should_summarize_paste(text), "4 lines should be summarized");
}

#[test]
fn test_should_summarize_paste_long_multiline() {
    let app = create_test_app();

    // Both conditions met: > 3 lines AND > 150 chars
    let text = "a".repeat(40) + "\n" + &"b".repeat(40) + "\n" + &"c".repeat(40) + "\n" + &"d".repeat(40);
    assert!(app.should_summarize_paste(&text), "Long multiline text should be summarized");
}

#[test]
fn test_should_summarize_paste_empty_text() {
    let app = create_test_app();

    // Empty text
    let text = "";
    assert!(!app.should_summarize_paste(text), "Empty text should not be summarized");
}

#[test]
fn test_should_summarize_paste_single_long_line() {
    let app = create_test_app();

    // Single line but very long (200 chars)
    let text = "a".repeat(200);
    assert!(app.should_summarize_paste(&text), "Single long line (200 chars) should be summarized");
}

#[test]
fn test_should_summarize_paste_many_short_lines() {
    let app = create_test_app();

    // Many lines but total < 150 chars
    let text = "a\nb\nc\nd\ne";
    assert!(app.should_summarize_paste(text), "5 short lines should be summarized (> 3 lines)");
}
