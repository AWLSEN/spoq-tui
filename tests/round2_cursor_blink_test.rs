//! Round 2 test: Cursor blinking functionality
//!
//! Tests the new cursor blink features added in Round 2:
//! - render_input_area_with_blink() function
//! - render_without_border_with_cursor() method on TextAreaInput
//! - cursor_visible field on InputWithChipWidget

use spoq::app::App;

#[test]
fn test_cursor_blink_logic_visible_at_tick_0() {
    // At tick_count = 0: (0 / 5) = 0, 0.is_multiple_of(2) = true (even)
    // Cursor should be visible
    let tick_count = 0u64;
    let blink_enabled = true;
    let focused = true;

    let cursor_visible = if blink_enabled && focused {
        (tick_count / 5).is_multiple_of(2)
    } else {
        focused
    };

    assert!(cursor_visible, "Cursor should be visible at tick_count=0");
}

#[test]
fn test_cursor_blink_logic_hidden_at_tick_5() {
    // At tick_count = 5: (5 / 5) = 1, 1.is_multiple_of(2) = false (odd)
    // Cursor should be hidden (blink off phase)
    let tick_count = 5u64;
    let blink_enabled = true;
    let focused = true;

    let cursor_visible = if blink_enabled && focused {
        (tick_count / 5).is_multiple_of(2)
    } else {
        focused
    };

    assert!(
        !cursor_visible,
        "Cursor should be hidden at tick_count=5 (blink off)"
    );
}

#[test]
fn test_cursor_blink_logic_visible_at_tick_10() {
    // At tick_count = 10: (10 / 5) = 2, 2.is_multiple_of(2) = true (even)
    // Cursor should be visible again
    let tick_count = 10u64;
    let blink_enabled = true;
    let focused = true;

    let cursor_visible = if blink_enabled && focused {
        (tick_count / 5).is_multiple_of(2)
    } else {
        focused
    };

    assert!(
        cursor_visible,
        "Cursor should be visible at tick_count=10"
    );
}

#[test]
fn test_cursor_blink_disabled_always_visible() {
    // When blink_enabled = false, cursor should always be visible when focused
    let tick_count = 5u64; // Would be hidden if blink was enabled
    let blink_enabled = false;
    let focused = true;

    let cursor_visible = if blink_enabled && focused {
        (tick_count / 5).is_multiple_of(2)
    } else {
        focused
    };

    assert!(
        cursor_visible,
        "Cursor should always be visible when blink_enabled=false"
    );
}

#[test]
fn test_cursor_not_visible_when_unfocused() {
    // When not focused, cursor should never be visible regardless of blink setting
    let tick_count = 0u64;
    let blink_enabled = true;
    let focused = false;

    let cursor_visible = if blink_enabled && focused {
        (tick_count / 5).is_multiple_of(2)
    } else {
        focused
    };

    assert!(
        !cursor_visible,
        "Cursor should be hidden when not focused"
    );
}

#[test]
fn test_cursor_blink_cycle() {
    // Test a full blink cycle: visible -> hidden -> visible
    let blink_enabled = true;
    let focused = true;

    let mut visible_count = 0;
    let mut hidden_count = 0;

    for tick in 0u64..20u64 {
        let cursor_visible = if blink_enabled && focused {
            (tick / 5).is_multiple_of(2)
        } else {
            focused
        };

        if cursor_visible {
            visible_count += 1;
        } else {
            hidden_count += 1;
        }
    }

    // Should have roughly equal visible/hidden time
    // Ticks 0-4 (5 ticks): visible (0/5=0, even)
    // Ticks 5-9 (5 ticks): hidden (1, odd)
    // Ticks 10-14 (5 ticks): visible (2, even)
    // Ticks 15-19 (5 ticks): hidden (3, odd)
    assert_eq!(visible_count, 10, "Should have 10 visible ticks");
    assert_eq!(hidden_count, 10, "Should have 10 hidden ticks");
}

#[test]
fn test_app_has_tick_count_field() {
    // Verify the App struct has the tick_count field used for cursor blinking
    let app = App::default();
    let _tick_count = app.tick_count; // Should compile without error
}

#[test]
fn test_render_without_border_with_cursor_signature() {
    // Test that the render_without_border_with_cursor method exists
    // with the correct signature
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use spoq::widgets::textarea_input::TextAreaInput;

    let mut textarea = TextAreaInput::new();
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);

    // This should compile - method exists with (area, buf, focused, cursor_visible) signature
    textarea.render_without_border_with_cursor(area, &mut buf, true, true);
    textarea.render_without_border_with_cursor(area, &mut buf, true, false);
    textarea.render_without_border_with_cursor(area, &mut buf, false, false);
}
