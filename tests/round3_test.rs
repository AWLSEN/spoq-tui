//! Integration tests for Round 3 features
//! Tests reasoning/thinking blocks, subagent tracking, session state extensions

use spoq::app::App;
use spoq::cache::ThreadCache;
use spoq::models::{Message, MessageRole, ThreadType};
use spoq::state::{SubagentTracker, SessionState};

// ============================================================================
// Reasoning/Thinking Block Tests
// ============================================================================

#[test]
fn test_reasoning_content_accumulation() {
    let mut cache = ThreadCache::new();
    let thread_id = cache.create_streaming_thread("What is Rust?".to_string());

    // Simulate streaming reasoning tokens
    cache.append_reasoning_to_message(&thread_id, "Let me think about this. ");
    cache.append_reasoning_to_message(&thread_id, "Rust is a systems programming language. ");
    cache.append_reasoning_to_message(&thread_id, "It focuses on safety and performance.");

    let messages = cache.get_messages(&thread_id).unwrap();
    let assistant_msg = &messages[1];

    assert_eq!(
        assistant_msg.reasoning_content,
        "Let me think about this. Rust is a systems programming language. It focuses on safety and performance."
    );
    assert!(!assistant_msg.reasoning_collapsed); // Should be expanded while streaming
    assert!(assistant_msg.is_streaming);
}

#[test]
fn test_reasoning_collapses_on_finalize() {
    let mut cache = ThreadCache::new();
    let thread_id = cache.create_streaming_thread("Question".to_string());

    cache.append_reasoning_to_message(&thread_id, "Thinking...");
    cache.append_to_message(&thread_id, "Answer");

    // Before finalize - reasoning should be visible
    let messages = cache.get_messages(&thread_id).unwrap();
    assert!(!messages[1].reasoning_collapsed);

    cache.finalize_message(&thread_id, 100);

    // After finalize - reasoning should be collapsed
    let messages = cache.get_messages(&thread_id).unwrap();
    assert!(messages[1].reasoning_collapsed);
    assert_eq!(messages[1].content, "Answer");
    assert_eq!(messages[1].reasoning_content, "Thinking...");
}

#[test]
fn test_reasoning_toggle() {
    let mut cache = ThreadCache::new();
    let thread_id = cache.create_streaming_thread("Test".to_string());

    cache.append_reasoning_to_message(&thread_id, "Some reasoning");
    cache.finalize_message(&thread_id, 100);

    // After finalize, reasoning is collapsed
    let messages = cache.get_messages(&thread_id).unwrap();
    assert!(messages[1].reasoning_collapsed);

    // Toggle to expand
    cache.toggle_message_reasoning(&thread_id, 1);
    let messages = cache.get_messages(&thread_id).unwrap();
    assert!(!messages[1].reasoning_collapsed);

    // Toggle to collapse again
    cache.toggle_message_reasoning(&thread_id, 1);
    let messages = cache.get_messages(&thread_id).unwrap();
    assert!(messages[1].reasoning_collapsed);
}

#[test]
fn test_reasoning_token_count() {
    let message = Message {
        id: 1,
        thread_id: "thread-1".to_string(),
        role: MessageRole::Assistant,
        content: "Response".to_string(),
        created_at: chrono::Utc::now(),
        is_streaming: false,
        partial_content: String::new(),
        reasoning_content: "Let me think about this step by step carefully".to_string(),
        reasoning_collapsed: false,
    };

    // "Let me think about this step by step carefully" = 9 words
    assert_eq!(message.reasoning_token_count(), 9);
}

#[test]
fn test_find_last_reasoning_message() {
    let mut cache = ThreadCache::new();
    let thread_id = cache.create_streaming_thread("First".to_string());

    // First message with reasoning
    cache.append_reasoning_to_message(&thread_id, "Reasoning 1");
    cache.finalize_message(&thread_id, 100);

    // Second message with reasoning
    cache.add_streaming_message(&thread_id, "Second question".to_string());
    cache.append_reasoning_to_message(&thread_id, "Reasoning 2");
    cache.finalize_message(&thread_id, 101);

    // Should find the last assistant message with reasoning (index 3)
    let idx = cache.find_last_reasoning_message_index(&thread_id);
    assert!(idx.is_some());
    assert_eq!(idx.unwrap(), 3);
}

// ============================================================================
// Subagent Tracking Tests
// ============================================================================

#[test]
fn test_subagent_tracker_basic_workflow() {
    let mut tracker = SubagentTracker::new();

    // Register a subagent
    tracker.register_subagent(
        "agent-1".to_string(),
        "Explore".to_string(),
        "Exploring codebase structure".to_string(),
        10,
    );

    assert_eq!(tracker.total_count(), 1);
    assert!(tracker.has_active_subagents());
    assert_eq!(tracker.active_count(), 1);

    // Get the subagent
    let state = tracker.get_subagent("agent-1").unwrap();
    assert_eq!(state.subagent_type, "Explore");
    assert_eq!(state.description, "Exploring codebase structure");
    assert_eq!(state.tool_call_count, 0);

    // Update progress
    tracker.update_progress("agent-1", "Found 5 relevant files".to_string());
    let state = tracker.get_subagent("agent-1").unwrap();
    assert_eq!(state.tool_call_count, 1);

    // Complete the subagent
    tracker.complete_subagent(
        "agent-1",
        true,
        "Complete (5 tool calls)".to_string(),
        100,
    );

    assert!(!tracker.has_active_subagents());
    assert_eq!(tracker.active_count(), 0);
    assert_eq!(tracker.total_count(), 1); // Still tracked, just not active
}

#[test]
fn test_subagent_tracker_multiple_subagents() {
    let mut tracker = SubagentTracker::new();

    // Register multiple subagents
    tracker.register_subagent(
        "agent-1".to_string(),
        "Explore".to_string(),
        "Exploring".to_string(),
        10,
    );
    tracker.register_subagent(
        "agent-2".to_string(),
        "Plan".to_string(),
        "Planning".to_string(),
        20,
    );
    tracker.register_subagent(
        "agent-3".to_string(),
        "Bash".to_string(),
        "Running tests".to_string(),
        30,
    );

    assert_eq!(tracker.total_count(), 3);
    assert_eq!(tracker.active_count(), 3);

    // Complete one
    tracker.complete_subagent("agent-2", true, "Done".to_string(), 50);
    assert_eq!(tracker.active_count(), 2);

    // Complete another with failure
    tracker.complete_subagent("agent-3", false, "Failed".to_string(), 60);
    assert_eq!(tracker.active_count(), 1);

    // Verify the last active one
    let active = tracker.active_subagents();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].1.subagent_id, "agent-1");
}

#[test]
fn test_subagent_render_filtering() {
    let mut tracker = SubagentTracker::new();

    // Add a started subagent
    tracker.register_subagent(
        "agent-1".to_string(),
        "Explore".to_string(),
        "Exploring".to_string(),
        10,
    );

    // Add a completed success (within fade window)
    tracker.register_subagent(
        "agent-2".to_string(),
        "Plan".to_string(),
        "Planning".to_string(),
        20,
    );
    tracker.complete_subagent("agent-2", true, "Done".to_string(), 50);

    // Add a completed failure
    tracker.register_subagent(
        "agent-3".to_string(),
        "Bash".to_string(),
        "Running".to_string(),
        30,
    );
    tracker.complete_subagent("agent-3", false, "Error".to_string(), 40);

    // At tick 60, all three should render (success at tick 50, fades at 80)
    let to_render = tracker.subagents_to_render(60);
    assert_eq!(to_render.len(), 3);

    // At tick 90, only started and failure should render (success faded)
    let to_render = tracker.subagents_to_render(90);
    assert_eq!(to_render.len(), 2);
}

#[test]
fn test_subagent_tracker_clear_on_done() {
    let mut tracker = SubagentTracker::new();

    tracker.register_subagent(
        "agent-1".to_string(),
        "Explore".to_string(),
        "Exploring".to_string(),
        10,
    );
    tracker.register_subagent(
        "agent-2".to_string(),
        "Plan".to_string(),
        "Planning".to_string(),
        20,
    );

    assert_eq!(tracker.total_count(), 2);

    // Clear all (simulates done event)
    tracker.clear();

    assert_eq!(tracker.total_count(), 0);
    assert!(!tracker.has_active_subagents());
}

// ============================================================================
// Session State Tests (Extensions for Round 3)
// ============================================================================

#[test]
fn test_session_state_oauth_with_url() {
    let mut state = SessionState::new();

    // Set OAuth required
    state.set_oauth_required("github".to_string(), "commit-skill".to_string());
    assert!(state.needs_oauth());

    // Set OAuth URL
    state.set_oauth_url("https://github.com/login/oauth/authorize".to_string());
    assert_eq!(
        state.oauth_url,
        Some("https://github.com/login/oauth/authorize".to_string())
    );

    // Clear OAuth URL after opening
    state.clear_oauth_url();
    assert!(state.oauth_url.is_none());
    // OAuth requirement should still be set
    assert!(state.needs_oauth());
}

#[test]
fn test_session_state_context_token_limit() {
    let mut state = SessionState::new();

    // Set context tokens and limit
    state.set_context_tokens(45_000);
    state.set_context_token_limit(100_000);

    assert_eq!(state.context_tokens_used, Some(45_000));
    assert_eq!(state.context_token_limit, Some(100_000));

    // Update tokens after compaction
    state.set_context_tokens(30_000);
    assert_eq!(state.context_tokens_used, Some(30_000));
    assert_eq!(state.context_token_limit, Some(100_000)); // Limit unchanged
}

#[test]
fn test_session_state_skills_injected() {
    let mut state = SessionState::new();

    // Add skills
    state.add_skill("commit".to_string());
    state.add_skill("review".to_string());
    state.add_skill("lint".to_string());

    assert_eq!(state.skills.len(), 3);
    assert!(state.has_skill("commit"));
    assert!(state.has_skill("review"));
    assert!(state.has_skill("lint"));

    // Try to add duplicate
    state.add_skill("commit".to_string());
    assert_eq!(state.skills.len(), 3); // No duplicate
}

// ============================================================================
// App Integration Tests for Round 3
// ============================================================================

#[tokio::test]
async fn test_app_initializes_with_subagent_tracker() {
    let app = App::new().expect("Failed to create app");

    assert_eq!(app.subagent_tracker.total_count(), 0);
    assert!(!app.subagent_tracker.has_active_subagents());
}

#[tokio::test]
async fn test_app_initializes_with_session_state_extensions() {
    let app = App::new().expect("Failed to create app");

    assert!(app.session_state.oauth_url.is_none());
    assert!(app.session_state.context_token_limit.is_none());
    assert!(app.session_state.context_tokens_used.is_none());
}

#[test]
fn test_message_reasoning_fields_initialization() {
    let mut cache = ThreadCache::new();
    let thread_id = cache.create_streaming_thread("Test".to_string());

    let messages = cache.get_messages(&thread_id).unwrap();

    // User message should have empty reasoning
    assert_eq!(messages[0].reasoning_content, "");
    assert!(messages[0].reasoning_collapsed);

    // Assistant streaming message should have empty reasoning, not collapsed (yet)
    assert_eq!(messages[1].reasoning_content, "");
    assert!(!messages[1].reasoning_collapsed); // Not collapsed during streaming
}

#[test]
fn test_reasoning_persistence_through_reconciliation() {
    let mut cache = ThreadCache::new();
    let pending_id = cache.create_pending_thread("Hello".to_string(), ThreadType::Normal);

    // Add reasoning to pending thread
    cache.append_reasoning_to_message(&pending_id, "Initial reasoning");

    // Reconcile thread
    cache.reconcile_thread_id(&pending_id, "real-id-123", Some("Thread Title".to_string()));

    // Reasoning should still be accessible via new ID
    let messages = cache.get_messages("real-id-123").unwrap();
    let assistant_msg = &messages[1];
    assert_eq!(assistant_msg.reasoning_content, "Initial reasoning");
}

#[test]
fn test_multiple_messages_with_reasoning() {
    let mut cache = ThreadCache::new();
    let thread_id = cache.create_streaming_thread("First question".to_string());

    // First exchange with reasoning
    cache.append_reasoning_to_message(&thread_id, "Thinking about first question");
    cache.append_to_message(&thread_id, "First answer");
    cache.finalize_message(&thread_id, 100);

    // Second exchange with reasoning
    cache.add_streaming_message(&thread_id, "Second question".to_string());
    cache.append_reasoning_to_message(&thread_id, "Analyzing second question");
    cache.append_to_message(&thread_id, "Second answer");
    cache.finalize_message(&thread_id, 101);

    let messages = cache.get_messages(&thread_id).unwrap();
    assert_eq!(messages.len(), 4); // 2 user + 2 assistant

    // Check first assistant message
    assert_eq!(messages[1].reasoning_content, "Thinking about first question");
    assert!(messages[1].reasoning_collapsed);
    assert_eq!(messages[1].content, "First answer");

    // Check second assistant message
    assert_eq!(messages[3].reasoning_content, "Analyzing second question");
    assert!(messages[3].reasoning_collapsed);
    assert_eq!(messages[3].content, "Second answer");
}
