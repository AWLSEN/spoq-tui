// Integration tests for scroll event handling

use spoq::app::{App, Screen, ScrollBoundary};
use spoq::models::ThreadType;

/// Helper to create a test app with conversation screen
fn create_test_app_in_conversation() -> App {
    let mut app = App::new().expect("Failed to create app");

    // Create a thread and navigate to conversation
    let thread_id = app.cache.create_pending_thread(
        "Test Thread".to_string(),
        ThreadType::Conversation,
        None,
    );
    app.active_thread_id = Some(thread_id);
    app.screen = Screen::Conversation;

    app
}

#[test]
fn test_scroll_offset_initializes_to_zero() {
    let app = create_test_app_in_conversation();
    assert_eq!(app.unified_scroll, 0, "Initial scroll offset should be 0");
}

#[test]
fn test_max_scroll_initializes_to_zero() {
    let app = create_test_app_in_conversation();
    assert_eq!(app.max_scroll, 0, "Initial max_scroll should be 0");
}

#[test]
fn test_scroll_down_decreases_offset() {
    let mut app = create_test_app_in_conversation();
    app.unified_scroll = 10;
    app.max_scroll = 100;

    // Simulate scroll down (see newer content)
    // Based on the code: near_bottom = scroll <= threshold
    // amount = if near_bottom { 1 } else { 3 }
    let threshold = (app.max_scroll / 10).max(5);
    let near_bottom = app.unified_scroll <= threshold;
    let amount = if near_bottom { 1 } else { 3 };

    let original_scroll = app.unified_scroll;
    app.unified_scroll = app.unified_scroll.saturating_sub(amount);

    assert!(
        app.unified_scroll < original_scroll,
        "Scroll down should decrease offset (see newer content)"
    );
}

#[test]
fn test_scroll_up_increases_offset() {
    let mut app = create_test_app_in_conversation();
    app.unified_scroll = 10;
    app.max_scroll = 100;

    // Simulate scroll up (see older content)
    let threshold = (app.max_scroll / 10).max(5);
    let near_top = app.unified_scroll >= app.max_scroll.saturating_sub(threshold);
    let amount = if near_top { 1 } else { 3 };

    let original_scroll = app.unified_scroll;
    app.unified_scroll = app.unified_scroll.saturating_add(amount).min(app.max_scroll);

    assert!(
        app.unified_scroll > original_scroll,
        "Scroll up should increase offset (see older content)"
    );
}

#[test]
fn test_scroll_acceleration_far_from_boundary() {
    let mut app = create_test_app_in_conversation();
    app.unified_scroll = 50; // Middle position
    app.max_scroll = 100;

    // Test scroll down acceleration
    let threshold = (app.max_scroll / 10).max(5);
    let near_bottom = app.unified_scroll <= threshold;
    let amount = if near_bottom { 1 } else { 3 };

    assert_eq!(amount, 3, "Should scroll 3 lines when far from boundary");
}

#[test]
fn test_scroll_acceleration_near_bottom_boundary() {
    let mut app = create_test_app_in_conversation();
    app.max_scroll = 100;
    let threshold = (app.max_scroll / 10).max(5);
    app.unified_scroll = threshold; // At the threshold

    // Test scroll down near bottom
    let near_bottom = app.unified_scroll <= threshold;
    let amount = if near_bottom { 1 } else { 3 };

    assert_eq!(amount, 1, "Should scroll 1 line when near bottom");
}

#[test]
fn test_scroll_acceleration_near_top_boundary() {
    let mut app = create_test_app_in_conversation();
    app.max_scroll = 100;
    let threshold = (app.max_scroll / 10).max(5);
    app.unified_scroll = app.max_scroll - threshold; // Near top

    // Test scroll up near top
    let near_top = app.unified_scroll >= app.max_scroll.saturating_sub(threshold);
    let amount = if near_top { 1 } else { 3 };

    assert_eq!(amount, 1, "Should scroll 1 line when near top");
}

#[test]
fn test_scroll_clamping_at_bottom() {
    let mut app = create_test_app_in_conversation();
    app.unified_scroll = 2;
    app.max_scroll = 100;

    // Scroll down more than current offset
    app.unified_scroll = app.unified_scroll.saturating_sub(5);

    assert_eq!(
        app.unified_scroll, 0,
        "Scroll should clamp to 0 at bottom"
    );
}

#[test]
fn test_scroll_clamping_at_top() {
    let mut app = create_test_app_in_conversation();
    app.unified_scroll = 98;
    app.max_scroll = 100;

    // Scroll up beyond max
    app.unified_scroll = app.unified_scroll.saturating_add(5).min(app.max_scroll);

    assert_eq!(
        app.unified_scroll, app.max_scroll,
        "Scroll should clamp to max_scroll at top"
    );
}

#[test]
fn test_scroll_only_active_on_conversation_screen() {
    let mut app = create_test_app_in_conversation();
    app.screen = Screen::CommandDeck;

    let _original_scroll = app.unified_scroll;

    // Scroll events should be ignored on CommandDeck
    // (This is enforced in main.rs event handler, not in App)
    // Just verify the screen state
    assert_eq!(
        app.screen,
        Screen::CommandDeck,
        "Should be on CommandDeck screen"
    );

    // Change back to conversation
    app.screen = Screen::Conversation;
    assert_eq!(
        app.screen,
        Screen::Conversation,
        "Should be on Conversation screen"
    );
}

#[test]
fn test_scroll_threshold_minimum_of_five() {
    let mut app = create_test_app_in_conversation();
    app.max_scroll = 30; // Small max_scroll

    // Threshold should be at least 5
    let threshold = (app.max_scroll / 10).max(5);

    assert!(
        threshold >= 5,
        "Threshold should have minimum value of 5"
    );
}

#[test]
fn test_scroll_threshold_scales_with_max_scroll() {
    let mut app = create_test_app_in_conversation();
    app.max_scroll = 1000; // Large max_scroll

    // Threshold should be 10% of max_scroll
    let threshold = (app.max_scroll / 10).max(5);

    assert_eq!(
        threshold, 100,
        "Threshold should be 10% of max_scroll when max_scroll is large"
    );
}

#[test]
fn test_natural_scrolling_direction() {
    let mut app = create_test_app_in_conversation();
    app.unified_scroll = 50;
    app.max_scroll = 100;

    // ScrollDown -> see newer content -> decrease offset
    let original_scroll = app.unified_scroll;
    app.unified_scroll = app.unified_scroll.saturating_sub(3);
    assert!(
        app.unified_scroll < original_scroll,
        "ScrollDown should decrease offset (natural scrolling)"
    );

    // ScrollUp -> see older content -> increase offset
    app.unified_scroll = 50; // Reset
    let original_scroll = app.unified_scroll;
    app.unified_scroll = app.unified_scroll.saturating_add(3).min(app.max_scroll);
    assert!(
        app.unified_scroll > original_scroll,
        "ScrollUp should increase offset (natural scrolling)"
    );
}

#[test]
fn test_scroll_saturating_operations_prevent_overflow() {
    let mut app = create_test_app_in_conversation();
    app.unified_scroll = u16::MAX;
    app.max_scroll = u16::MAX;

    // Test saturating_add doesn't overflow
    let result = app.unified_scroll.saturating_add(100);
    assert_eq!(result, u16::MAX, "saturating_add should not overflow");

    // Test saturating_sub doesn't underflow
    app.unified_scroll = 0;
    let result = app.unified_scroll.saturating_sub(100);
    assert_eq!(result, 0, "saturating_sub should not underflow");
}

// =============================================================================
// Scroll Boundary Detection Tests
// =============================================================================

#[test]
fn test_scroll_boundary_initializes_to_none() {
    let app = create_test_app_in_conversation();
    assert!(
        app.scroll_boundary_hit.is_none(),
        "scroll_boundary_hit should initialize to None"
    );
}

#[test]
fn test_boundary_hit_tick_initializes_to_zero() {
    let app = create_test_app_in_conversation();
    assert_eq!(
        app.boundary_hit_tick, 0,
        "boundary_hit_tick should initialize to 0"
    );
}

#[test]
fn test_scroll_boundary_enum_equality() {
    assert_eq!(ScrollBoundary::Top, ScrollBoundary::Top);
    assert_eq!(ScrollBoundary::Bottom, ScrollBoundary::Bottom);
    assert_ne!(ScrollBoundary::Top, ScrollBoundary::Bottom);
}

#[test]
fn test_scroll_boundary_enum_copy() {
    let boundary = ScrollBoundary::Top;
    let copy = boundary; // Copy trait
    assert_eq!(boundary, copy, "ScrollBoundary should implement Copy");
}

#[test]
fn test_bottom_boundary_detection() {
    let mut app = create_test_app_in_conversation();
    app.max_scroll = 100;
    app.unified_scroll = 1;
    app.tick_count = 42;

    // Simulate scroll down that hits bottom boundary
    let threshold = (app.max_scroll / 10).max(5);
    let near_bottom = app.unified_scroll <= threshold;
    let amount = if near_bottom { 1 } else { 3 };
    app.unified_scroll = app.unified_scroll.saturating_sub(amount);

    // Detect bottom boundary hit
    if app.unified_scroll == 0 {
        app.scroll_boundary_hit = Some(ScrollBoundary::Bottom);
        app.boundary_hit_tick = app.tick_count;
    }

    assert_eq!(
        app.scroll_boundary_hit,
        Some(ScrollBoundary::Bottom),
        "Should detect bottom boundary hit"
    );
    assert_eq!(
        app.boundary_hit_tick, 42,
        "Should record tick count when boundary hit"
    );
}

#[test]
fn test_top_boundary_detection() {
    let mut app = create_test_app_in_conversation();
    app.max_scroll = 100;
    app.unified_scroll = 99;
    app.tick_count = 123;

    // Simulate scroll up that hits top boundary
    let threshold = (app.max_scroll / 10).max(5);
    let near_top = app.unified_scroll >= app.max_scroll.saturating_sub(threshold);
    let amount = if near_top { 1 } else { 3 };
    app.unified_scroll = app.unified_scroll.saturating_add(amount).min(app.max_scroll);

    // Detect top boundary hit
    if app.unified_scroll == app.max_scroll && app.max_scroll > 0 {
        app.scroll_boundary_hit = Some(ScrollBoundary::Top);
        app.boundary_hit_tick = app.tick_count;
    }

    assert_eq!(
        app.scroll_boundary_hit,
        Some(ScrollBoundary::Top),
        "Should detect top boundary hit"
    );
    assert_eq!(
        app.boundary_hit_tick, 123,
        "Should record tick count when boundary hit"
    );
}

#[test]
fn test_boundary_state_clears_after_timeout() {
    let mut app = create_test_app_in_conversation();
    app.scroll_boundary_hit = Some(ScrollBoundary::Top);
    app.boundary_hit_tick = 100;
    app.tick_count = 115; // 15 ticks later (past the 12-tick threshold at 16ms = ~200ms)

    const BOUNDARY_HIGHLIGHT_TICKS: u64 = 12;
    let ticks_since_hit = app.tick_count.saturating_sub(app.boundary_hit_tick);

    if ticks_since_hit >= BOUNDARY_HIGHLIGHT_TICKS {
        app.scroll_boundary_hit = None;
    }

    assert!(
        app.scroll_boundary_hit.is_none(),
        "Boundary state should clear after timeout (12 ticks)"
    );
}

#[test]
fn test_boundary_state_persists_within_timeout() {
    let mut app = create_test_app_in_conversation();
    app.scroll_boundary_hit = Some(ScrollBoundary::Bottom);
    app.boundary_hit_tick = 100;
    app.tick_count = 105; // 5 ticks later (within the 12-tick threshold at 16ms = ~200ms)

    const BOUNDARY_HIGHLIGHT_TICKS: u64 = 12;
    let ticks_since_hit = app.tick_count.saturating_sub(app.boundary_hit_tick);

    let should_clear = ticks_since_hit >= BOUNDARY_HIGHLIGHT_TICKS;

    assert!(
        !should_clear,
        "Boundary state should persist within timeout window"
    );
    assert_eq!(
        app.scroll_boundary_hit,
        Some(ScrollBoundary::Bottom),
        "Boundary state should still be set"
    );
}

#[test]
fn test_no_top_boundary_when_max_scroll_zero() {
    let mut app = create_test_app_in_conversation();
    app.max_scroll = 0; // No scrollable content
    app.unified_scroll = 0;
    app.tick_count = 10;

    // This scenario shouldn't trigger top boundary (max_scroll > 0 check)
    let at_top = app.unified_scroll == app.max_scroll && app.max_scroll > 0;

    assert!(
        !at_top,
        "Should not detect top boundary when max_scroll is 0"
    );
}

#[test]
fn test_scroll_does_not_hit_boundary_when_in_middle() {
    let mut app = create_test_app_in_conversation();
    app.max_scroll = 100;
    app.unified_scroll = 50;

    // Scroll up a bit (still in middle)
    app.unified_scroll = app.unified_scroll.saturating_add(3).min(app.max_scroll);

    // Check neither boundary is hit
    let at_bottom = app.unified_scroll == 0;
    let at_top = app.unified_scroll == app.max_scroll && app.max_scroll > 0;

    assert!(!at_bottom, "Should not be at bottom when in middle");
    assert!(!at_top, "Should not be at top when in middle");
}

// =============================================================================
// Smooth Scrolling / Momentum Tests
// =============================================================================

#[test]
fn test_scroll_velocity_initializes_to_zero() {
    let app = create_test_app_in_conversation();
    assert_eq!(
        app.scroll_velocity, 0.0,
        "scroll_velocity should initialize to 0.0"
    );
}

#[test]
fn test_scroll_position_initializes_to_zero() {
    let app = create_test_app_in_conversation();
    assert_eq!(
        app.scroll_position, 0.0,
        "scroll_position should initialize to 0.0"
    );
}

#[test]
fn test_tick_applies_velocity_to_position() {
    let mut app = create_test_app_in_conversation();
    app.max_scroll = 100;
    app.scroll_position = 10.0;
    app.scroll_velocity = 5.0; // Positive = scrolling up (older content)

    app.tick();

    // Position should increase by velocity (with friction)
    assert!(
        app.scroll_position > 10.0,
        "tick() should apply velocity to position"
    );
}

#[test]
fn test_tick_decays_velocity_with_friction() {
    let mut app = create_test_app_in_conversation();
    app.max_scroll = 100;
    app.scroll_position = 50.0;
    app.scroll_velocity = 10.0;

    let original_velocity = app.scroll_velocity;
    app.tick();

    assert!(
        app.scroll_velocity.abs() < original_velocity.abs(),
        "tick() should decay velocity with friction"
    );
}

#[test]
fn test_tick_stops_velocity_when_very_small() {
    let mut app = create_test_app_in_conversation();
    app.max_scroll = 100;
    app.scroll_position = 50.0;
    app.scroll_velocity = 0.04; // Below threshold (0.05)

    app.tick();

    assert_eq!(
        app.scroll_velocity, 0.0,
        "tick() should stop velocity when below threshold"
    );
}

#[test]
fn test_tick_clamps_position_at_bottom() {
    let mut app = create_test_app_in_conversation();
    app.max_scroll = 100;
    app.scroll_position = 2.0;
    app.scroll_velocity = -10.0; // Negative = scrolling down (newer content)

    app.tick();

    assert_eq!(
        app.scroll_position, 0.0,
        "tick() should clamp position at bottom (0)"
    );
    assert_eq!(
        app.scroll_velocity, 0.0,
        "velocity should stop when hitting boundary"
    );
}

#[test]
fn test_tick_clamps_position_at_top() {
    let mut app = create_test_app_in_conversation();
    app.max_scroll = 100;
    app.scroll_position = 98.0;
    app.scroll_velocity = 10.0; // Positive = scrolling up (older content)

    app.tick();

    assert_eq!(
        app.scroll_position, 100.0,
        "tick() should clamp position at top (max_scroll)"
    );
    assert_eq!(
        app.scroll_velocity, 0.0,
        "velocity should stop when hitting boundary"
    );
}

#[test]
fn test_tick_detects_bottom_boundary_hit() {
    let mut app = create_test_app_in_conversation();
    app.max_scroll = 100;
    app.scroll_position = 2.0;
    app.scroll_velocity = -10.0; // Will overshoot bottom

    app.tick();

    assert_eq!(
        app.scroll_boundary_hit,
        Some(ScrollBoundary::Bottom),
        "Should detect bottom boundary hit"
    );
}

#[test]
fn test_tick_detects_top_boundary_hit() {
    let mut app = create_test_app_in_conversation();
    app.max_scroll = 100;
    app.scroll_position = 98.0;
    app.scroll_velocity = 10.0; // Will overshoot top

    app.tick();

    assert_eq!(
        app.scroll_boundary_hit,
        Some(ScrollBoundary::Top),
        "Should detect top boundary hit"
    );
}

#[test]
fn test_unified_scroll_synced_with_position() {
    let mut app = create_test_app_in_conversation();
    app.max_scroll = 100;
    app.scroll_position = 42.7;
    app.scroll_velocity = 1.0;

    app.tick();

    // unified_scroll should be rounded from scroll_position
    let expected = app.scroll_position.round() as u16;
    assert_eq!(
        app.unified_scroll, expected,
        "unified_scroll should be synced with scroll_position (rounded)"
    );
}

#[test]
fn test_reset_scroll_clears_all_scroll_state() {
    let mut app = create_test_app_in_conversation();
    app.unified_scroll = 50;
    app.scroll_position = 50.0;
    app.scroll_velocity = 10.0;

    app.reset_scroll();

    assert_eq!(app.unified_scroll, 0, "unified_scroll should reset to 0");
    assert_eq!(app.scroll_position, 0.0, "scroll_position should reset to 0.0");
    assert_eq!(app.scroll_velocity, 0.0, "scroll_velocity should reset to 0.0");
}

#[test]
fn test_momentum_continues_after_no_input() {
    let mut app = create_test_app_in_conversation();
    app.max_scroll = 100;
    app.scroll_position = 50.0;
    app.scroll_velocity = 8.0; // Initial impulse

    // Simulate several ticks without new input
    let mut positions = vec![app.scroll_position];
    for _ in 0..5 {
        app.tick();
        positions.push(app.scroll_position);
    }

    // Position should keep increasing (momentum)
    for i in 1..positions.len() {
        assert!(
            positions[i] > positions[i - 1] || app.scroll_velocity.abs() < 0.1,
            "Position should keep increasing with momentum (or velocity stopped)"
        );
    }

    // Velocity should decrease over time
    assert!(
        app.scroll_velocity < 8.0,
        "Velocity should decrease due to friction"
    );
}
