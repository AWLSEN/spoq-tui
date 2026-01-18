//! UI rendering for SPOQ Command Deck
//!
//! Implements the full cyberpunk-styled terminal interface with:
//! - Header with ASCII logo and migration progress
//! - Left panel: Notifications + Saved/Active task columns
//! - Right panel: Thread cards
//! - Bottom: Input box and keybind hints
//!
//! ## Responsive Layout System
//!
//! The UI uses a responsive layout system based on `LayoutContext`. This struct
//! encapsulates terminal dimensions and provides methods for proportional sizing:
//!
//! - `percent_width()` / `percent_height()` - Calculate proportional dimensions
//! - `should_stack_panels()` - Determine if panels should stack vertically
//! - `available_content_width()` - Get usable width after borders/margins
//! - `is_compact()` / `is_narrow()` / `is_short()` - Query terminal size state
//!
//! All render functions receive a `LayoutContext` parameter to enable responsive
//! sizing decisions throughout the UI hierarchy.

mod command_deck;
mod conversation;
mod folder_picker;
mod helpers;
pub mod input;
mod layout;
mod messages;
mod panels;
mod theme;
mod thread_switcher;

// Re-export theme colors for external use
pub use theme::{
    COLOR_ACCENT, COLOR_ACTIVE, COLOR_BORDER, COLOR_DIM, COLOR_HEADER,
    COLOR_INPUT_BG, COLOR_PROGRESS, COLOR_PROGRESS_BG, COLOR_QUEUED,
    COLOR_TOOL_ERROR, COLOR_TOOL_ICON, COLOR_TOOL_RUNNING, COLOR_TOOL_SUCCESS,
};

// Re-export layout system for external use
pub use layout::{
    calculate_stacked_heights, calculate_two_column_widths, LayoutContext, SizeCategory,
    breakpoints,
};

// Re-export helper functions for external use
pub use helpers::{format_tool_args, is_terminal_too_small, MIN_TERMINAL_HEIGHT, MIN_TERMINAL_WIDTH};

// Re-export rendering functions for external use
pub use messages::{estimate_wrapped_line_count, truncate_preview};

use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Style},
    widgets::Paragraph,
};

use crate::app::{App, Screen};
use command_deck::render_command_deck;
use conversation::render_conversation_screen;
use thread_switcher::render_thread_switcher;

// ============================================================================
// Main UI Rendering
// ============================================================================

/// Render the UI based on current screen
pub fn render(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    // Check if terminal is too small
    if helpers::is_terminal_too_small(area.width, area.height) {
        render_terminal_too_small(frame, area);
        return;
    }

    match app.screen {
        Screen::CommandDeck => render_command_deck(frame, app),
        Screen::Conversation => render_conversation_screen(frame, app),
    }

    // Render thread switcher overlay (if visible)
    render_thread_switcher(frame, app);
}

/// Render a message when the terminal is too small
fn render_terminal_too_small(frame: &mut Frame, area: Rect) {
    let message = [
        "Terminal Too Small".to_string(),
        String::new(),
        format!("Current size: {}x{}", area.width, area.height),
        format!("Minimum required: {}x{}", helpers::MIN_TERMINAL_WIDTH, helpers::MIN_TERMINAL_HEIGHT),
        String::new(),
        "Please resize your terminal".to_string(),
    ];

    let text = message.join("\n");

    let paragraph = Paragraph::new(text)
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::Yellow));

    frame.render_widget(paragraph, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::PermissionMode;
    use conversation::create_mode_indicator_line;
    use helpers::{extract_short_model_name, format_tokens, get_tool_icon, truncate_string, is_terminal_too_small, MIN_TERMINAL_WIDTH, MIN_TERMINAL_HEIGHT};
    use input::{build_contextual_keybinds, get_permission_preview};
    use messages::{render_tool_event, truncate_preview};
    use ratatui::{backend::TestBackend, Terminal};

    fn create_test_app() -> App {
        App::default()
    }

    #[test]
    fn test_screen_enum_default() {
        let screen = Screen::default();
        assert_eq!(screen, Screen::CommandDeck);
    }

    #[test]
    fn test_screen_enum_variants() {
        let command_deck = Screen::CommandDeck;
        let conversation = Screen::Conversation;
        assert_ne!(command_deck, conversation);
    }

    #[test]
    fn test_render_command_deck_screen() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        // Check that the terminal rendered without panic
        let buffer = terminal.backend().buffer();
        // Verify the buffer contains some content (not all spaces)
        let has_content = buffer
            .content()
            .iter()
            .any(|cell| cell.symbol() != " ");
        assert!(has_content, "CommandDeck screen should render content");
    }

    #[test]
    fn test_render_conversation_screen() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        // Check that the terminal rendered without panic
        let buffer = terminal.backend().buffer();
        // Verify the buffer contains some content
        let has_content = buffer
            .content()
            .iter()
            .any(|cell| cell.symbol() != " ");
        assert!(has_content, "Conversation screen should render content");
    }

    #[test]
    fn test_conversation_screen_shows_thread_title() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;
        // Add thread to cache instead of legacy threads vec
        app.cache.upsert_thread(crate::models::Thread {
            id: "test-thread".to_string(),
            title: "Test Thread".to_string(),
            description: None,
            preview: "Test preview".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::default(),
            model: None,
            permission_mode: None,
            message_count: 0,
            working_directory: None,
            created_at: chrono::Utc::now(),
        });
        app.active_thread_id = Some("test-thread".to_string());

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        // Check that the buffer contains the thread title
        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(
            buffer_str.contains("Test Thread"),
            "Conversation screen should show thread title"
        );
    }

    #[test]
    fn test_conversation_screen_default_title() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;
        app.active_thread_id = None;

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        // Check that the buffer contains the default title
        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(
            buffer_str.contains("New Conversation"),
            "Conversation screen should show default title when no active thread"
        );
    }

    #[test]
    fn test_conversation_screen_renders_with_user_input() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;
        app.textarea.insert_char('H');
        app.textarea.insert_char('e');
        app.textarea.insert_char('l');
        app.textarea.insert_char('l');
        app.textarea.insert_char('o');

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        // Check that the screen renders without panic when there's input
        let buffer = terminal.backend().buffer();
        let has_content = buffer
            .content()
            .iter()
            .any(|cell| cell.symbol() != " ");
        assert!(has_content, "Conversation screen should render content with user input");
    }

    #[test]
    fn test_conversation_screen_shows_placeholder() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        // Check that the buffer shows placeholder response with vertical bar
        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        // Messages now use vertical bar prefix instead of role labels
        assert!(
            buffer_str.contains("│"),
            "Conversation screen should show vertical bar for messages"
        );
        assert!(
            buffer_str.contains("Waiting for your message"),
            "Conversation screen should show placeholder text"
        );
    }

    #[test]
    fn test_command_deck_shows_disconnected_status() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.connection_status = false;

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(
            buffer_str.contains("Disconnected"),
            "CommandDeck should show Disconnected status when connection_status is false"
        );
        assert!(
            buffer_str.contains("○"),
            "CommandDeck should show empty circle icon when disconnected"
        );
    }

    #[test]
    fn test_command_deck_shows_connected_status() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.connection_status = true;

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(
            buffer_str.contains("Connected"),
            "CommandDeck should show Connected status when connection_status is true"
        );
        assert!(
            buffer_str.contains("●"),
            "CommandDeck should show filled circle icon when connected"
        );
    }

    #[test]
    fn test_conversation_screen_shows_disconnected_status() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;
        app.connection_status = false;

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(
            buffer_str.contains("○"),
            "Conversation screen should show disconnected status icon (○)"
        );
    }

    #[test]
    fn test_conversation_screen_shows_connected_status() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;
        app.connection_status = true;

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(
            buffer_str.contains("●"),
            "Conversation screen should show connected status icon (●)"
        );
    }

    #[test]
    fn test_conversation_screen_shows_error_banner() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;
        app.stream_error = Some("Connection timed out".to_string());

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(
            buffer_str.contains("ERROR"),
            "Conversation screen should show ERROR label when stream_error is set"
        );
        assert!(
            buffer_str.contains("Connection timed out"),
            "Conversation screen should show the error message"
        );
    }

    #[test]
    fn test_conversation_screen_no_error_banner_when_no_error() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;
        app.stream_error = None;

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(
            !buffer_str.contains("ERROR"),
            "Conversation screen should not show ERROR label when stream_error is None"
        );
    }

    #[test]
    fn test_conversation_screen_shows_streaming_indicator() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;

        // Create a streaming thread with a streaming message
        let thread_id = app.cache.create_streaming_thread("Test message".to_string());
        app.active_thread_id = Some(thread_id);

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(
            buffer_str.contains("Responding"),
            "Conversation screen should show spinner with 'Responding...' when a message is streaming"
        );
    }

    #[test]
    fn test_conversation_screen_shows_partial_content_during_streaming() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;

        // Create a streaming thread and append some tokens
        let thread_id = app.cache.create_streaming_thread("Test message".to_string());
        app.cache.append_to_message(&thread_id, "Hello from ");
        app.cache.append_to_message(&thread_id, "the AI");
        app.active_thread_id = Some(thread_id);

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(
            buffer_str.contains("Hello from the AI"),
            "Conversation screen should show partial_content during streaming"
        );
    }

    #[test]
    fn test_conversation_screen_shows_cursor_during_streaming() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;
        app.tick_count = 0; // Ensure cursor is visible (tick_count / 5) % 2 == 0

        // Create a streaming thread
        let thread_id = app.cache.create_streaming_thread("Test".to_string());
        app.cache.append_to_message(&thread_id, "Response");
        app.active_thread_id = Some(thread_id);

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        // The cursor character █ should be present when tick_count makes it visible
        assert!(
            buffer_str.contains("█"),
            "Conversation screen should show blinking cursor during streaming"
        );
    }

    #[test]
    fn test_conversation_screen_cursor_blinks() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;

        // Create a streaming thread
        let thread_id = app.cache.create_streaming_thread("Test".to_string());
        app.cache.append_to_message(&thread_id, "Response");
        app.active_thread_id = Some(thread_id.clone());

        // Test cursor visible (tick_count = 0, 0/5 % 2 == 0)
        app.tick_count = 0;
        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str_visible: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // Test cursor hidden (tick_count = 5, 5/5 % 2 == 1)
        app.tick_count = 5;
        let backend2 = TestBackend::new(100, 30);
        let mut terminal2 = Terminal::new(backend2).unwrap();
        terminal2
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer2 = terminal2.backend().buffer();
        let buffer_str_hidden: String = buffer2
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // When visible, should have █; when hidden, the cursor position should have space
        assert!(
            buffer_str_visible.contains("█"),
            "Cursor should be visible at tick_count=0"
        );
        // Note: The hidden cursor shows a space, so we check that █ is not present
        // or that the behavior differs
        assert!(
            !buffer_str_hidden.contains("█"),
            "Cursor should be hidden at tick_count=5"
        );
    }

    #[test]
    fn test_conversation_screen_no_streaming_indicator_for_completed_messages() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;

        // Create a streaming thread and finalize it
        let thread_id = app.cache.create_streaming_thread("Test message".to_string());
        app.cache.append_to_message(&thread_id, "Completed response");
        app.cache.finalize_message(&thread_id, 123);
        app.active_thread_id = Some(thread_id);

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(
            !buffer_str.contains("Responding..."),
            "Conversation screen should NOT show 'Responding...' spinner for completed messages"
        );
    }

    #[test]
    fn test_conversation_screen_shows_completed_message_content() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;

        // Create a streaming thread and finalize it
        let thread_id = app.cache.create_streaming_thread("User question".to_string());
        app.cache.append_to_message(&thread_id, "Final answer from AI");
        app.cache.finalize_message(&thread_id, 456);
        app.active_thread_id = Some(thread_id);

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(
            buffer_str.contains("Final answer from AI"),
            "Conversation screen should show completed message content"
        );
        // Should NOT have the blinking cursor for completed messages
        assert!(
            !buffer_str.contains("█"),
            "Conversation screen should NOT show cursor for completed messages"
        );
    }

    #[test]
    fn test_conversation_screen_shows_user_message() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;

        // Create a streaming thread (which includes a user message)
        let thread_id = app.cache.create_streaming_thread("Hello from user".to_string());
        app.active_thread_id = Some(thread_id);

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(
            buffer_str.contains("Hello from user"),
            "Conversation screen should show user message content"
        );
        // Messages now use vertical bar prefix instead of role labels
        assert!(
            buffer_str.contains("│"),
            "Conversation screen should show vertical bar for user messages"
        );
    }

    // ============= Mode Indicator Tests =============

    #[test]
    fn test_create_mode_indicator_line_plan() {
        let line = create_mode_indicator_line(PermissionMode::Plan);
        assert!(line.is_some());
        let line = line.unwrap();
        // Check that the line contains "[PLAN]"
        let content: String = line.spans.iter().map(|s| s.content.to_string()).collect();
        assert!(content.contains("[PLAN]"));
    }

    #[test]
    fn test_create_mode_indicator_line_bypass() {
        let line = create_mode_indicator_line(PermissionMode::BypassPermissions);
        assert!(line.is_some());
        let line = line.unwrap();
        // Check that the line contains "[EXECUTE]"
        let content: String = line.spans.iter().map(|s| s.content.to_string()).collect();
        assert!(content.contains("[EXECUTE]"));
    }

    #[test]
    fn test_create_mode_indicator_line_none() {
        let line = create_mode_indicator_line(PermissionMode::Default);
        assert!(line.is_none());
    }

    #[test]
    fn test_mode_indicator_shown_for_all_threads() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;

        // Create a Normal thread (not Programming)
        app.cache.upsert_thread(crate::models::Thread {
            id: "conv-thread".to_string(),
            title: "Normal Thread".to_string(),
            description: None,
            preview: "Just chatting".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Conversation,
            model: None,
            permission_mode: None,
            message_count: 0,
            working_directory: None,
            created_at: chrono::Utc::now(),
        });
        app.active_thread_id = Some("conv-thread".to_string());
        app.permission_mode = PermissionMode::Plan; // Set mode - should show on all threads now

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // Mode indicator should now be shown for all threads
        assert!(
            buffer_str.contains("[PLAN]"),
            "Mode indicator should be shown for all threads"
        );
    }

    #[test]
    fn test_mode_indicator_shown_for_plan_mode() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;

        // Create a Programming thread
        app.cache.upsert_thread(crate::models::Thread {
            id: "prog-thread".to_string(),
            title: "Programming Thread".to_string(),
            description: None,
            preview: "Code review".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Programming,
            model: None,
            permission_mode: None,
            message_count: 0,
            working_directory: None,
            created_at: chrono::Utc::now(),
        });
        app.active_thread_id = Some("prog-thread".to_string());
        app.permission_mode = PermissionMode::Plan;

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        assert!(
            buffer_str.contains("[PLAN]"),
            "Mode indicator should show '[PLAN]' in Plan mode"
        );
    }

    #[test]
    fn test_mode_indicator_shown_for_programming_thread_bypass_mode() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;

        // Create a Programming thread
        app.cache.upsert_thread(crate::models::Thread {
            id: "prog-thread".to_string(),
            title: "Programming Thread".to_string(),
            description: None,
            preview: "Code review".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Programming,
            model: None,
            permission_mode: None,
            message_count: 0,
            working_directory: None,
            created_at: chrono::Utc::now(),
        });
        app.active_thread_id = Some("prog-thread".to_string());
        app.permission_mode = PermissionMode::BypassPermissions;

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        assert!(
            buffer_str.contains("[EXECUTE]"),
            "Mode indicator should show '[EXECUTE]' in BypassPermissions mode"
        );
    }

    #[test]
    fn test_mode_indicator_not_shown_for_programming_thread_none_mode() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;

        // Create a Programming thread
        app.cache.upsert_thread(crate::models::Thread {
            id: "prog-thread".to_string(),
            title: "Programming Thread".to_string(),
            description: None,
            preview: "Code review".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Programming,
            model: None,
            permission_mode: None,
            message_count: 0,
            working_directory: None,
            created_at: chrono::Utc::now(),
        });
        app.active_thread_id = Some("prog-thread".to_string());
        app.permission_mode = PermissionMode::Default;

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // When mode is Default, no indicator should be shown
        assert!(
            !buffer_str.contains("[PLAN]"),
            "Mode indicator should not show '[PLAN]' when mode is Default"
        );
        assert!(
            !buffer_str.contains("[EXECUTE]"),
            "Mode indicator should not show '[EXECUTE]' when mode is Default"
        );
    }

    #[test]
    fn test_mode_indicator_not_shown_on_command_deck() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::CommandDeck; // Not on Conversation screen
        app.permission_mode = PermissionMode::Plan;

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // Mode indicator should not be shown on CommandDeck
        assert!(
            !buffer_str.contains("[PLAN MODE]"),
            "Mode indicator should not be shown on CommandDeck screen"
        );
    }

    #[test]
    fn test_mode_indicator_not_shown_when_no_active_thread() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;
        app.active_thread_id = None; // No active thread
        app.permission_mode = PermissionMode::Plan;

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // Mode indicator should not be shown when there's no active thread
        assert!(
            !buffer_str.contains("[PLAN MODE]"),
            "Mode indicator should not be shown when there's no active thread"
        );
    }

    // ========================================================================
    // Tests for Phase 6: Thread Type Indicators and Model Names
    // ========================================================================

    #[test]
    fn test_extract_short_model_name_opus() {
        assert_eq!(extract_short_model_name("claude-opus-4-5-20250514"), "opus");
        assert_eq!(extract_short_model_name("claude-opus-3-5"), "opus");
        assert_eq!(extract_short_model_name("opus-anything"), "opus");
    }

    #[test]
    fn test_extract_short_model_name_sonnet() {
        assert_eq!(extract_short_model_name("claude-sonnet-4-5-20250514"), "sonnet");
        assert_eq!(extract_short_model_name("claude-sonnet-3-5"), "sonnet");
        assert_eq!(extract_short_model_name("sonnet-anything"), "sonnet");
    }

    #[test]
    fn test_extract_short_model_name_other_models() {
        assert_eq!(extract_short_model_name("gpt-4"), "gpt");
        assert_eq!(extract_short_model_name("gpt-3.5-turbo"), "gpt");
        assert_eq!(extract_short_model_name("llama-2-70b"), "llama");
    }

    #[test]
    fn test_extract_short_model_name_simple_model() {
        assert_eq!(extract_short_model_name("simple"), "simple");
        assert_eq!(extract_short_model_name("model"), "model");
    }

    #[test]
    fn test_thread_type_indicator_shown_for_normal_thread() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::CommandDeck;

        // Add a normal thread to the cache
        app.cache.upsert_thread(crate::models::Thread {
            id: "thread-1".to_string(),
            title: "Normal Thread".to_string(),
            description: None,
            preview: "A normal conversation".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Conversation,
            model: Some("claude-sonnet-4-5".to_string()),
            permission_mode: None,
            message_count: 0,
            working_directory: None,
            created_at: chrono::Utc::now(),
        });

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // Should show [C] indicator for Conversation thread
        assert!(
            buffer_str.contains("[C]"),
            "Thread type indicator [C] should be shown for Conversation threads"
        );
    }

    #[test]
    fn test_thread_type_indicator_shown_for_programming_thread() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::CommandDeck;

        // Add a programming thread to the cache
        app.cache.upsert_thread(crate::models::Thread {
            id: "thread-1".to_string(),
            title: "Programming Thread".to_string(),
            description: None,
            preview: "A programming conversation".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Programming,
            model: Some("claude-opus-4-5".to_string()),
            permission_mode: None,
            message_count: 0,
            working_directory: None,
            created_at: chrono::Utc::now(),
        });

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // Should show [P] indicator for Programming thread
        assert!(
            buffer_str.contains("[P]"),
            "Thread type indicator [P] should be shown for Programming threads"
        );
    }

    #[test]
    fn test_model_name_shown_with_thread_type_indicator() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::CommandDeck;

        // Add a thread with model information
        app.cache.upsert_thread(crate::models::Thread {
            id: "thread-1".to_string(),
            title: "Thread with Model".to_string(),
            description: None,
            preview: "Testing model display".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Conversation,
            model: Some("claude-sonnet-4-5-20250514".to_string()),
            permission_mode: None,
            message_count: 0,
            working_directory: None,
            created_at: chrono::Utc::now(),
        });

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // Should show [C] and "sonnet" model name
        assert!(
            buffer_str.contains("[C]"),
            "Thread type indicator should be shown"
        );
        assert!(
            buffer_str.contains("sonnet"),
            "Short model name should be shown next to type indicator"
        );
    }

    #[test]
    fn test_thread_type_indicator_without_model() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::CommandDeck;

        // Add a thread without model information
        app.cache.upsert_thread(crate::models::Thread {
            id: "thread-1".to_string(),
            title: "Thread without Model".to_string(),
            description: None,
            preview: "No model info".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Programming,
            model: None,
            permission_mode: None,
            message_count: 0,
            working_directory: None,
            created_at: chrono::Utc::now(),
        });

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // Should show [P] indicator even without model
        assert!(
            buffer_str.contains("[P]"),
            "Thread type indicator should be shown even without model information"
        );
    }

    #[test]
    fn test_multiple_threads_show_different_type_indicators() {
        let backend = TestBackend::new(120, 50);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::CommandDeck;

        // Add a normal thread
        app.cache.upsert_thread(crate::models::Thread {
            id: "thread-1".to_string(),
            title: "Normal".to_string(),
            description: None,
            preview: "Normal thread".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Conversation,
            model: Some("claude-sonnet-4-5".to_string()),
            permission_mode: None,
            message_count: 0,
            working_directory: None,
            created_at: chrono::Utc::now(),
        });

        // Add a programming thread
        app.cache.upsert_thread(crate::models::Thread {
            id: "thread-2".to_string(),
            title: "Programming".to_string(),
            description: None,
            preview: "Programming thread".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Programming,
            model: Some("claude-opus-4-5".to_string()),
            permission_mode: None,
            message_count: 0,
            working_directory: None,
            created_at: chrono::Utc::now(),
        });

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // Both indicators should be present
        assert!(
            buffer_str.contains("[C]"),
            "Should show [C] indicator for conversation thread"
        );
        assert!(
            buffer_str.contains("[P]"),
            "Should show [P] indicator for programming thread"
        );
        assert!(
            buffer_str.contains("sonnet"),
            "Should show sonnet model name"
        );
        assert!(
            buffer_str.contains("opus"),
            "Should show opus model name"
        );
    }

    // ============= Phase 10: Contextual Keybinds Tests =============

    #[test]
    fn test_contextual_keybinds_command_deck() {
        let app = create_test_app();
        // app.screen defaults to CommandDeck

        let keybinds = build_contextual_keybinds(&app);
        let content: String = keybinds.spans.iter().map(|s| s.content.to_string()).collect();

        // Should show basic CommandDeck hints
        assert!(content.contains("Tab"));
        assert!(content.contains("switch thread"));
        assert!(content.contains("Enter"));
        assert!(content.contains("send"));
    }

    #[test]
    fn test_contextual_keybinds_conversation_with_error() {
        let mut app = create_test_app();
        app.screen = Screen::Conversation;
        app.stream_error = Some("Test error".to_string());

        let keybinds = build_contextual_keybinds(&app);
        let content: String = keybinds.spans.iter().map(|s| s.content.to_string()).collect();

        // Should show dismiss error hint
        assert!(content.contains("d"));
        assert!(content.contains("dismiss error"));
    }

    #[test]
    fn test_contextual_keybinds_programming_thread_shows_mode_cycling() {
        let mut app = create_test_app();
        app.screen = Screen::Conversation;

        // Create a programming thread
        app.cache.upsert_thread(crate::models::Thread {
            id: "prog-thread".to_string(),
            title: "Programming".to_string(),
            description: None,
            preview: "Code".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Programming,
            model: None,
            permission_mode: None,
            message_count: 0,
            working_directory: None,
            created_at: chrono::Utc::now(),
        });
        app.active_thread_id = Some("prog-thread".to_string());

        let keybinds = build_contextual_keybinds(&app);
        let content: String = keybinds.spans.iter().map(|s| s.content.to_string()).collect();

        // Should show mode cycling hint for programming thread
        assert!(content.contains("Shift+Tab"));
        assert!(content.contains("cycle mode"));
    }

    #[test]
    fn test_contextual_keybinds_normal_thread_no_mode_cycling() {
        let mut app = create_test_app();
        app.screen = Screen::Conversation;

        // Create a normal thread
        app.cache.upsert_thread(crate::models::Thread {
            id: "conv-thread".to_string(),
            title: "Normal".to_string(),
            description: None,
            preview: "Chat".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Conversation,
            model: None,
            permission_mode: None,
            message_count: 0,
            working_directory: None,
            created_at: chrono::Utc::now(),
        });
        app.active_thread_id = Some("conv-thread".to_string());

        let keybinds = build_contextual_keybinds(&app);
        let content: String = keybinds.spans.iter().map(|s| s.content.to_string()).collect();

        // Round 2: Mode cycling is now shown for ALL threads (Conversation and Programming)
        assert!(content.contains("Shift+Tab") || content.contains("S+Tab"));
        assert!(content.contains("cycle mode") || content.contains("mode"));
    }

    // ============= Phase 10: Input Border Tests =============
    // Updated in Round 1: Removed dashed border streaming mode.
    // Input box now always uses solid borders regardless of streaming state.

    #[test]
    fn test_conversation_input_uses_solid_border_when_streaming() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;

        // Create a streaming thread
        let thread_id = app.cache.create_streaming_thread("Test".to_string());
        app.active_thread_id = Some(thread_id);

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // Should have solid border characters (─)
        assert!(
            buffer_str.contains("─"),
            "Input should use solid border (streaming mode removed in Round 1)"
        );
        // Should NOT have dashed border characters
        assert!(
            !buffer_str.contains("┄"),
            "Input should not use dashed border (feature removed)"
        );
    }

    #[test]
    fn test_conversation_input_uses_solid_border_when_not_streaming() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;

        // Create a thread with completed message
        let thread_id = app.cache.create_streaming_thread("Test".to_string());
        app.cache.append_to_message(&thread_id, "Response");
        app.cache.finalize_message(&thread_id, 1);
        app.active_thread_id = Some(thread_id);

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // Should have solid border characters (─), not dashed
        assert!(
            buffer_str.contains("─"),
            "Input should use solid border when not streaming"
        );
    }

    // ============= Permission Prompt Tests =============

    #[test]
    fn test_get_permission_preview_returns_context() {
        use crate::state::session::PermissionRequest;

        let perm = PermissionRequest {
            permission_id: "perm-test".to_string(),
            tool_name: "Write".to_string(),
            description: "Write file".to_string(),
            context: Some("/home/user/test.rs".to_string()),
            tool_input: None,
            received_at: std::time::Instant::now(),
        };

        let preview = get_permission_preview(&perm);
        assert_eq!(preview, "/home/user/test.rs");
    }

    #[test]
    fn test_get_permission_preview_extracts_file_path() {
        use crate::state::session::PermissionRequest;

        let perm = PermissionRequest {
            permission_id: "perm-test".to_string(),
            tool_name: "Read".to_string(),
            description: "Read file".to_string(),
            context: None,
            tool_input: Some(serde_json::json!({"file_path": "/var/log/test.log"})),
            received_at: std::time::Instant::now(),
        };

        let preview = get_permission_preview(&perm);
        assert_eq!(preview, "/var/log/test.log");
    }

    #[test]
    fn test_get_permission_preview_extracts_command() {
        use crate::state::session::PermissionRequest;

        let perm = PermissionRequest {
            permission_id: "perm-test".to_string(),
            tool_name: "Bash".to_string(),
            description: "Run command".to_string(),
            context: None,
            tool_input: Some(serde_json::json!({"command": "npm install"})),
            received_at: std::time::Instant::now(),
        };

        let preview = get_permission_preview(&perm);
        assert_eq!(preview, "npm install");
    }

    #[test]
    fn test_get_permission_preview_truncates_long_content() {
        use crate::state::session::PermissionRequest;

        let long_content = "a".repeat(150);
        let perm = PermissionRequest {
            permission_id: "perm-test".to_string(),
            tool_name: "Write".to_string(),
            description: "Write file".to_string(),
            context: None,
            tool_input: Some(serde_json::json!({"content": long_content})),
            received_at: std::time::Instant::now(),
        };

        let preview = get_permission_preview(&perm);
        assert!(preview.len() < 110); // Should be truncated
        assert!(preview.ends_with("..."));
    }

    #[test]
    fn test_get_permission_preview_empty_when_no_info() {
        use crate::state::session::PermissionRequest;

        let perm = PermissionRequest {
            permission_id: "perm-test".to_string(),
            tool_name: "Custom".to_string(),
            description: "Custom action".to_string(),
            context: None,
            tool_input: None,
            received_at: std::time::Instant::now(),
        };

        let preview = get_permission_preview(&perm);
        assert!(preview.is_empty());
    }

    #[test]
    fn test_permission_prompt_renders_with_pending_permission() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;

        // Set up a pending permission
        use crate::state::session::PermissionRequest;
        app.session_state.set_pending_permission(PermissionRequest {
            permission_id: "perm-render".to_string(),
            tool_name: "Bash".to_string(),
            description: "Run npm install".to_string(),
            context: Some("npm install".to_string()),
            tool_input: None,
            received_at: std::time::Instant::now(),
        });

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // Check that permission prompt elements are rendered
        assert!(
            buffer_str.contains("Permission Required"),
            "Should show 'Permission Required' title"
        );
        assert!(
            buffer_str.contains("Bash"),
            "Should show tool name"
        );
    }

    #[test]
    fn test_format_tokens_small() {
        assert_eq!(format_tokens(0), "0");
        assert_eq!(format_tokens(500), "500");
        assert_eq!(format_tokens(999), "999");
    }

    #[test]
    fn test_format_tokens_thousands() {
        assert_eq!(format_tokens(1_000), "1k");
        assert_eq!(format_tokens(5_000), "5k");
        assert_eq!(format_tokens(45_000), "45k");
        assert_eq!(format_tokens(100_000), "100k");
        assert_eq!(format_tokens(999_999), "999k");
    }

    #[test]
    fn test_format_tokens_millions() {
        assert_eq!(format_tokens(1_000_000), "1M");
        assert_eq!(format_tokens(5_000_000), "5M");
        assert_eq!(format_tokens(10_000_000), "10M");
    }

    #[test]
    fn test_truncate_string_no_truncation() {
        assert_eq!(truncate_string("hello", 10), "hello");
        assert_eq!(truncate_string("test", 4), "test");
    }

    #[test]
    fn test_truncate_string_with_truncation() {
        assert_eq!(truncate_string("hello world", 8), "hello...");
        assert_eq!(truncate_string("very long string that needs truncation", 15), "very long st...");
    }

    #[test]
    fn test_truncate_string_edge_cases() {
        assert_eq!(truncate_string("", 10), "");
        assert_eq!(truncate_string("abc", 3), "abc");
        assert_eq!(truncate_string("abcd", 3), "...");
    }

    #[test]
    fn test_get_tool_icon_known_tools() {
        // Icons are disabled for cleaner display, all tools return empty string
        assert_eq!(get_tool_icon("Read"), "");
        assert_eq!(get_tool_icon("Write"), "");
        assert_eq!(get_tool_icon("Edit"), "");
        assert_eq!(get_tool_icon("Bash"), "");
        assert_eq!(get_tool_icon("Grep"), "");
        assert_eq!(get_tool_icon("Glob"), "");
        assert_eq!(get_tool_icon("Task"), "");
        assert_eq!(get_tool_icon("WebFetch"), "");
        assert_eq!(get_tool_icon("WebSearch"), "");
        assert_eq!(get_tool_icon("TodoWrite"), "");
        assert_eq!(get_tool_icon("AskUserQuestion"), "");
        assert_eq!(get_tool_icon("NotebookEdit"), "");
    }

    #[test]
    fn test_get_tool_icon_unknown_tool() {
        // Icons are disabled for cleaner display
        assert_eq!(get_tool_icon("UnknownTool"), "");
        assert_eq!(get_tool_icon("CustomFunction"), "");
    }

    #[test]
    fn test_format_tool_args_read() {
        let args = r#"{"file_path": "/src/main.rs"}"#;
        assert_eq!(helpers::format_tool_args("Read", args), "Reading /src/main.rs");
    }

    #[test]
    fn test_format_tool_args_write() {
        let args = r#"{"file_path": "/src/models.rs"}"#;
        assert_eq!(helpers::format_tool_args("Write", args), "Writing /src/models.rs");
    }

    #[test]
    fn test_format_tool_args_edit() {
        let args = r#"{"file_path": "/tests/integration.rs"}"#;
        assert_eq!(helpers::format_tool_args("Edit", args), "Editing /tests/integration.rs");
    }

    #[test]
    fn test_format_tool_args_bash() {
        let args = r#"{"command": "npm install"}"#;
        assert_eq!(helpers::format_tool_args("Bash", args), "Running: npm install");
    }

    #[test]
    fn test_format_tool_args_grep_with_path() {
        let args = r#"{"pattern": "TODO", "path": "src/"}"#;
        assert_eq!(helpers::format_tool_args("Grep", args), "Searching 'TODO' in src/");
    }

    #[test]
    fn test_format_tool_args_grep_without_path() {
        let args = r#"{"pattern": "FIXME"}"#;
        assert_eq!(helpers::format_tool_args("Grep", args), "Searching 'FIXME'");
    }

    #[test]
    fn test_format_tool_args_glob() {
        let args = r#"{"pattern": "**/*.rs"}"#;
        assert_eq!(helpers::format_tool_args("Glob", args), "Finding **/*.rs");
    }

    #[test]
    fn test_format_tool_args_task() {
        let args = r#"{"description": "Run all tests"}"#;
        assert_eq!(helpers::format_tool_args("Task", args), "Spawning: Run all tests");
    }

    #[test]
    fn test_format_tool_args_webfetch() {
        let args = r#"{"url": "https://example.com"}"#;
        assert_eq!(helpers::format_tool_args("WebFetch", args), "Fetching https://example.com");
    }

    #[test]
    fn test_format_tool_args_websearch() {
        let args = r#"{"query": "rust async"}"#;
        assert_eq!(helpers::format_tool_args("WebSearch", args), "Searching: rust async");
    }

    #[test]
    fn test_format_tool_args_todowrite() {
        let args = r#"{}"#;
        assert_eq!(helpers::format_tool_args("TodoWrite", args), "Updating todos");
    }

    #[test]
    fn test_format_tool_args_notebookedit() {
        let args = r#"{"notebook_path": "/notebooks/analysis.ipynb"}"#;
        assert_eq!(helpers::format_tool_args("NotebookEdit", args), "Editing notebook /notebooks/analysis.ipynb");
    }

    #[test]
    fn test_format_tool_args_unknown_tool() {
        let args = r#"{"some": "data"}"#;
        assert_eq!(helpers::format_tool_args("CustomTool", args), "CustomTool");
    }

    #[test]
    fn test_format_tool_args_invalid_json() {
        assert_eq!(helpers::format_tool_args("Read", "{invalid json"), "Read");
    }

    #[test]
    fn test_format_tool_args_truncates_long_paths() {
        let long_path = "/very/long/path/that/should/be/truncated/because/it/exceeds/the/maximum/length/allowed/for/display.rs";
        let args = format!(r#"{{"file_path": "{}"}}"#, long_path);
        let result = helpers::format_tool_args("Read", &args);
        assert!(result.starts_with("Reading "));
        assert!(result.ends_with("..."));
        assert!(result.len() < long_path.len() + 20);
    }

    #[test]
    fn test_format_tool_args_missing_expected_fields() {
        // Read without file_path
        assert_eq!(helpers::format_tool_args("Read", r#"{}"#), "Read");
        // Bash without command
        assert_eq!(helpers::format_tool_args("Bash", r#"{}"#), "Bash");
        // Grep without pattern
        assert_eq!(helpers::format_tool_args("Grep", r#"{}"#), "Searching ''");
    }

    #[test]
    fn test_truncate_preview_no_truncation_needed() {
        let text = "Short text";
        let result = truncate_preview(text, 150, 2);
        assert_eq!(result, "Short text");
        assert!(!result.ends_with("..."));
    }

    #[test]
    fn test_truncate_preview_truncates_at_max_chars() {
        let text = "a".repeat(200);
        let result = truncate_preview(&text, 150, 2);
        assert_eq!(result.len(), 153); // 150 chars + "..."
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_truncate_preview_truncates_at_max_lines() {
        let text = "line1\nline2\nline3\nline4";
        let result = truncate_preview(text, 150, 2);
        // Should stop at 2nd newline
        assert_eq!(result, "line1 line2...");
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_truncate_preview_replaces_newlines_with_spaces() {
        let text = "line1\nline2";
        let result = truncate_preview(text, 150, 10);
        assert_eq!(result, "line1 line2");
        assert!(!result.contains('\n'));
    }

    #[test]
    fn test_truncate_preview_trims_trailing_whitespace() {
        let text = "line1\nline2\n";
        let result = truncate_preview(text, 150, 1);
        assert_eq!(result, "line1...");
        assert!(!result.ends_with(" ..."));
    }

    #[test]
    fn test_truncate_preview_empty_text() {
        let text = "";
        let result = truncate_preview(text, 150, 2);
        assert_eq!(result, "");
    }

    #[test]
    fn test_truncate_preview_only_newlines() {
        let text = "\n\n\n";
        let result = truncate_preview(text, 150, 2);
        assert_eq!(result, "...");
    }

    #[test]
    fn test_format_tool_args_empty_json() {
        // Empty JSON object should return just the tool name
        assert_eq!(helpers::format_tool_args("Read", "{}"), "Read");
        assert_eq!(helpers::format_tool_args("Bash", "{}"), "Bash");
        assert_eq!(helpers::format_tool_args("Write", "{}"), "Write");
    }

    #[test]
    fn test_render_tool_event_complete_with_all_fields() {
        let mut tool = crate::models::ToolEvent::new("tool_456".to_string(), "Read".to_string());

        // Set args
        tool.args_json = r#"{"file_path": "/path/to/file.rs"}"#.to_string();
        tool.args_display = Some("Reading /path/to/file.rs".to_string());

        // Set result
        tool.set_result("File contents here", false);

        // Mark as done
        tool.complete();

        let ctx = LayoutContext::new(120, 40);
        let line = render_tool_event(&tool, 0, &ctx);

        // Verify the line contains expected elements
        let line_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();

        // Should contain formatted args
        assert!(line_text.contains("Reading /path/to/file.rs"));

        // Should show completed status (checkmark)
        assert!(line_text.contains("✓"));
    }

    #[test]
    fn test_render_tool_event_with_error_result() {
        let mut tool = crate::models::ToolEvent::new("tool_789".to_string(), "Bash".to_string());

        // Set args
        tool.args_json = r#"{"command": "invalid_command"}"#.to_string();
        tool.args_display = Some("invalid_command".to_string());

        // Set error result
        tool.set_result("Command not found", true);

        // Mark as done
        tool.fail();

        let ctx = LayoutContext::new(120, 40);
        let line = render_tool_event(&tool, 0, &ctx);

        // Verify the line contains expected elements
        let line_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();

        // Should contain command
        assert!(line_text.contains("invalid_command"));

        // Should show error status
        assert!(line_text.contains("✗"));
    }

    #[test]
    fn test_render_tool_event_streaming_state() {
        let mut tool = crate::models::ToolEvent::new("tool_streaming".to_string(), "Grep".to_string());

        // Set args but don't finish
        tool.args_json = r#"{"pattern": "test"}"#.to_string();
        tool.args_display = Some("Searching 'test'".to_string());

        // Tool is still running (not finished)
        let ctx = LayoutContext::new(120, 40);
        let line = render_tool_event(&tool, 0, &ctx);

        let line_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();

        // Should contain search pattern
        assert!(line_text.contains("Searching 'test'"));

        // Should show spinner or running indicator (not checkmark or X)
        assert!(!line_text.contains("✓"));
        assert!(!line_text.contains("✗"));
    }

    #[test]
    fn test_render_tool_event_full_lifecycle() {
        // Test the full flow: tool starts, args stream in, result comes back

        // Step 1: Tool starts with no args yet
        let mut tool = crate::models::ToolEvent::new("tool_lifecycle".to_string(), "Write".to_string());
        let ctx = LayoutContext::new(120, 40);
        let line1 = render_tool_event(&tool, 0, &ctx);
        let text1: String = line1.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text1.contains("Write")); // Default to tool name

        // Step 2: Args stream in
        tool.args_json = r#"{"file_path": "/tmp/test.txt", "content": "Hello"}"#.to_string();
        tool.args_display = Some("Writing /tmp/test.txt".to_string());
        let line2 = render_tool_event(&tool, 0, &ctx);
        let text2: String = line2.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text2.contains("Writing /tmp/test.txt"));

        // Step 3: Result comes back
        tool.set_result("File written successfully", false);
        tool.complete();
        let line3 = render_tool_event(&tool, 0, &ctx);
        let text3: String = line3.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text3.contains("✓")); // Success indicator
        assert!(text3.contains("Writing /tmp/test.txt"));
    }

    // ============= Phase 8: Subagent Rendering Tests =============

    #[test]
    fn test_get_subagent_icon() {
        use helpers::get_subagent_icon;

        // Icons are disabled for cleaner display, all subagents return empty string
        assert_eq!(get_subagent_icon("Explore"), "");
        assert_eq!(get_subagent_icon("Bash"), "");
        assert_eq!(get_subagent_icon("Plan"), "");
        assert_eq!(get_subagent_icon("general-purpose"), "");
        assert_eq!(get_subagent_icon("unknown"), "");
        assert_eq!(get_subagent_icon("CustomAgent"), "");
    }

    #[test]
    fn test_tree_connector_as_str() {
        use messages::TreeConnector;

        assert_eq!(TreeConnector::Single.as_str(), "● ");
        assert_eq!(TreeConnector::Branch.as_str(), "├── ");
        assert_eq!(TreeConnector::LastBranch.as_str(), "└── ");
    }

    #[test]
    fn test_render_subagent_event_running() {
        use crate::models::SubagentEvent;
        use messages::{render_subagent_event, TreeConnector};

        let event = SubagentEvent::new(
            "task-123".to_string(),
            "Exploring codebase".to_string(),
            "Explore".to_string(),
        );

        let ctx = LayoutContext::new(120, 40);
        let lines = render_subagent_event(&event, 0, TreeConnector::Single, &ctx);

        assert!(!lines.is_empty());
        let line_text: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();

        // Should contain the bullet point and Task
        assert!(line_text.contains("●"));
        assert!(line_text.contains("Task("));
        assert!(line_text.contains("Exploring codebase"));
        assert!(line_text.contains(")"));
    }

    #[test]
    fn test_render_subagent_event_running_with_progress() {
        use crate::models::SubagentEvent;
        use messages::{render_subagent_event, TreeConnector};

        let mut event = SubagentEvent::new(
            "task-123".to_string(),
            "Analyzing files".to_string(),
            "Explore".to_string(),
        );
        event.update_progress(Some("Reading src/main.rs".to_string()), true);

        let ctx = LayoutContext::new(120, 40);
        let lines = render_subagent_event(&event, 0, TreeConnector::Single, &ctx);

        assert_eq!(lines.len(), 2); // Main line + progress line
        let progress_text: String = lines[1].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(progress_text.contains("Reading src/main.rs"));
    }

    #[test]
    fn test_render_subagent_event_complete() {
        use crate::models::SubagentEvent;
        use messages::{render_subagent_event, TreeConnector};

        let mut event = SubagentEvent::new(
            "task-456".to_string(),
            "Task completed".to_string(),
            "general-purpose".to_string(),
        );
        event.tool_call_count = 5;
        event.complete(Some("Found 10 files".to_string()));

        let ctx = LayoutContext::new(120, 40);
        let lines = render_subagent_event(&event, 0, TreeConnector::Single, &ctx);

        assert_eq!(lines.len(), 1);
        let line_text: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();

        assert!(line_text.contains("Done"));
        assert!(line_text.contains("5 tool uses"));
        assert!(line_text.contains("Found 10 files"));
    }

    #[test]
    fn test_render_subagent_event_complete_single_tool_use() {
        use crate::models::SubagentEvent;
        use messages::{render_subagent_event, TreeConnector};

        let mut event = SubagentEvent::new(
            "task-789".to_string(),
            "Quick task".to_string(),
            "Bash".to_string(),
        );
        event.tool_call_count = 1;
        event.complete(None);

        let ctx = LayoutContext::new(120, 40);
        let lines = render_subagent_event(&event, 0, TreeConnector::Single, &ctx);

        let line_text: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(line_text.contains("1 tool use")); // Singular
        assert!(!line_text.contains("1 tool uses")); // Not plural
    }

    #[test]
    fn test_render_subagent_event_with_branch_connector() {
        use crate::models::SubagentEvent;
        use messages::{render_subagent_event, TreeConnector};

        let event = SubagentEvent::new(
            "task-branch".to_string(),
            "Branch task".to_string(),
            "Explore".to_string(),
        );

        let ctx = LayoutContext::new(120, 40);
        let lines = render_subagent_event(&event, 0, TreeConnector::Branch, &ctx);

        let line_text: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(line_text.contains("├──"));
    }

    #[test]
    fn test_render_subagent_event_with_last_branch_connector() {
        use crate::models::SubagentEvent;
        use messages::{render_subagent_event, TreeConnector};

        let event = SubagentEvent::new(
            "task-last".to_string(),
            "Last task".to_string(),
            "Explore".to_string(),
        );

        let ctx = LayoutContext::new(120, 40);
        let lines = render_subagent_event(&event, 0, TreeConnector::LastBranch, &ctx);

        let line_text: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(line_text.contains("└──"));
    }

    #[test]
    fn test_render_subagent_events_block_single() {
        use crate::models::SubagentEvent;
        use messages::render_subagent_events_block;

        let event = SubagentEvent::new(
            "task-single".to_string(),
            "Single task".to_string(),
            "Explore".to_string(),
        );

        let events: Vec<&SubagentEvent> = vec![&event];
        let ctx = LayoutContext::new(120, 40);
        let lines = render_subagent_events_block(&events, 0, &ctx);

        // Should use Single connector (●)
        let line_text: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(line_text.contains("●"));
        assert!(!line_text.contains("├──"));
        assert!(!line_text.contains("└──"));
    }

    #[test]
    fn test_render_subagent_events_block_multiple() {
        use crate::models::SubagentEvent;
        use messages::render_subagent_events_block;

        let event1 = SubagentEvent::new(
            "task-1".to_string(),
            "First task".to_string(),
            "Explore".to_string(),
        );
        let event2 = SubagentEvent::new(
            "task-2".to_string(),
            "Second task".to_string(),
            "Bash".to_string(),
        );
        let event3 = SubagentEvent::new(
            "task-3".to_string(),
            "Third task".to_string(),
            "general-purpose".to_string(),
        );

        let events: Vec<&SubagentEvent> = vec![&event1, &event2, &event3];
        let ctx = LayoutContext::new(120, 40);
        let lines = render_subagent_events_block(&events, 0, &ctx);

        // Should have lines for each event (running events have 1 line each)
        assert!(lines.len() >= 3);

        // Collect all text
        let all_text: String = lines.iter()
            .flat_map(|line| line.spans.iter())
            .map(|s| s.content.as_ref())
            .collect();

        // First and second should use Branch (├──), last should use LastBranch (└──)
        assert!(all_text.contains("├──"));
        assert!(all_text.contains("└──"));
        // Should not use Single (●) for multiple items
        assert!(!all_text.contains("● "));
    }

    #[test]
    fn test_render_subagent_events_block_empty() {
        use crate::models::SubagentEvent;
        use messages::render_subagent_events_block;

        let events: Vec<&SubagentEvent> = vec![];
        let ctx = LayoutContext::new(120, 40);
        let lines = render_subagent_events_block(&events, 0, &ctx);

        assert!(lines.is_empty());
    }

    #[test]
    fn test_render_subagent_event_summary_truncation() {
        use crate::models::SubagentEvent;
        use messages::{render_subagent_event, TreeConnector};

        let mut event = SubagentEvent::new(
            "task-long".to_string(),
            "Task with long summary".to_string(),
            "Explore".to_string(),
        );
        event.tool_call_count = 3;
        let long_summary = "This is a very long summary that should be truncated because it exceeds the maximum allowed length for display purposes";
        event.complete(Some(long_summary.to_string()));

        let ctx = LayoutContext::new(120, 40);
        let lines = render_subagent_event(&event, 0, TreeConnector::Single, &ctx);

        let line_text: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        // Should be truncated with ellipsis
        assert!(line_text.contains("..."));
        // Should not contain the full summary
        assert!(!line_text.contains("for display purposes"));
    }

    #[test]
    fn test_render_subagent_event_spinner_animation() {
        use crate::models::SubagentEvent;
        use messages::{render_subagent_event, TreeConnector};

        let event = SubagentEvent::new(
            "task-spinner".to_string(),
            "Spinner test".to_string(),
            "Explore".to_string(),
        );

        let ctx = LayoutContext::new(120, 40);
        // Test at different tick counts to verify spinner changes
        let lines_tick_0 = render_subagent_event(&event, 0, TreeConnector::Single, &ctx);
        let lines_tick_5 = render_subagent_event(&event, 5, TreeConnector::Single, &ctx);

        let text_0: String = lines_tick_0[0].spans.iter().map(|s| s.content.as_ref()).collect();
        let text_5: String = lines_tick_5[0].spans.iter().map(|s| s.content.as_ref()).collect();

        // Spinner frame should change between tick 0 and tick 5
        // (frames are at indices 0 and 5 in SPINNER_FRAMES)
        assert_ne!(text_0, text_5);
    }

    #[test]
    fn test_conversation_screen_renders_subagent_events() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;

        // Create a streaming thread with a subagent event
        let thread_id = app.cache.create_streaming_thread("Test message".to_string());

        // Get the streaming message and add a subagent event
        if let Some(messages) = app.cache.get_messages_mut(&thread_id) {
            if let Some(msg) = messages.iter_mut().find(|m| m.is_streaming) {
                msg.start_subagent_event(
                    "task-render".to_string(),
                    "Exploring codebase".to_string(),
                    "Explore".to_string(),
                );
            }
        }

        app.active_thread_id = Some(thread_id);

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // Should show the subagent event
        assert!(
            buffer_str.contains("Task("),
            "Should show Task( in subagent event"
        );
        assert!(
            buffer_str.contains("Exploring codebase"),
            "Should show subagent description"
        );
    }

    #[test]
    fn test_conversation_screen_renders_completed_subagent() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;

        // Create a thread with completed subagent
        let thread_id = app.cache.create_streaming_thread("Test message".to_string());

        if let Some(messages) = app.cache.get_messages_mut(&thread_id) {
            if let Some(msg) = messages.iter_mut().find(|m| m.is_streaming) {
                msg.start_subagent_event(
                    "task-complete".to_string(),
                    "Analysis task".to_string(),
                    "general-purpose".to_string(),
                );
                msg.complete_subagent_event(
                    "task-complete",
                    Some("Found 5 issues".to_string()),
                    3,
                );
            }
        }

        app.cache.finalize_message(&thread_id, 1);
        app.active_thread_id = Some(thread_id);

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // Should show completed status
        assert!(
            buffer_str.contains("Done"),
            "Should show 'Done' for completed subagent"
        );
        assert!(
            buffer_str.contains("3 tool uses"),
            "Should show tool count"
        );
        assert!(
            buffer_str.contains("Found 5 issues"),
            "Should show summary"
        );
    }

    #[test]
    fn test_conversation_screen_renders_parallel_subagents() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;

        // Create a thread with multiple parallel subagents
        let thread_id = app.cache.create_streaming_thread("Test message".to_string());

        if let Some(messages) = app.cache.get_messages_mut(&thread_id) {
            if let Some(msg) = messages.iter_mut().find(|m| m.is_streaming) {
                // Add multiple subagent events consecutively (simulating parallel execution)
                msg.start_subagent_event(
                    "task-1".to_string(),
                    "First parallel task".to_string(),
                    "Explore".to_string(),
                );
                msg.start_subagent_event(
                    "task-2".to_string(),
                    "Second parallel task".to_string(),
                    "Bash".to_string(),
                );
            }
        }

        app.active_thread_id = Some(thread_id);

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // Should show tree connectors for parallel tasks
        assert!(
            buffer_str.contains("├──") || buffer_str.contains("└──"),
            "Should show tree connectors for parallel subagents"
        );
        assert!(
            buffer_str.contains("First parallel task"),
            "Should show first task description"
        );
        assert!(
            buffer_str.contains("Second parallel task"),
            "Should show second task description"
        );
    }

    #[test]
    fn test_is_terminal_too_small_below_width_threshold() {
        assert!(is_terminal_too_small(25, 15));
    }

    #[test]
    fn test_is_terminal_too_small_below_height_threshold() {
        assert!(is_terminal_too_small(50, 8));
    }

    #[test]
    fn test_is_terminal_too_small_at_minimum() {
        assert!(!is_terminal_too_small(MIN_TERMINAL_WIDTH, MIN_TERMINAL_HEIGHT));
    }

    #[test]
    fn test_is_terminal_too_small_above_minimum() {
        assert!(!is_terminal_too_small(80, 24));
    }

    #[test]
    fn test_render_terminal_too_small_message() {
        let backend = TestBackend::new(20, 8);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();

        terminal.draw(|f| render(f, &mut app)).unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer.content().iter().map(|cell| cell.symbol()).collect();

        assert!(buffer_str.contains("Terminal Too Small"), "Should show 'Terminal Too Small' message");
        assert!(buffer_str.contains("20") && buffer_str.contains("8"), "Should show current terminal size");
        assert!(buffer_str.contains("30"), "Should show minimum required width");
    }

    #[test]
    fn test_render_normal_when_terminal_large_enough() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();

        terminal.draw(|f| render(f, &mut app)).unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer.content().iter().map(|cell| cell.symbol()).collect();

        assert!(!buffer_str.contains("Terminal Too Small"), "Should not show 'Terminal Too Small' message");
    }

    // ========================================================================
    // Responsive Layout Integration Tests
    // ========================================================================
    // These tests verify that the UI renders correctly at various terminal sizes
    // without panics and with appropriate layout adaptations.

    /// Helper function to verify basic UI rendering succeeds without panic
    fn verify_render_succeeds(width: u16, height: u16, screen: Screen) {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = screen;
        // IMPORTANT: Set app's terminal dimensions to match the test backend
        // This ensures LayoutContext calculations use the correct size
        app.terminal_width = width;
        app.terminal_height = height;

        // Add some test data to make rendering more comprehensive
        app.cache.upsert_thread(crate::models::Thread {
            id: "test-thread-1".to_string(),
            title: "Test Thread with a longer title".to_string(),
            description: Some("A description".to_string()),
            preview: "This is a preview message that might be truncated".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Conversation,
            model: Some("claude-opus".to_string()),
            permission_mode: None,
            message_count: 5,
            working_directory: None,
            created_at: chrono::Utc::now(),
        });

        if screen == Screen::Conversation {
            app.active_thread_id = Some("test-thread-1".to_string());
            app.cache.add_message_simple(
                "test-thread-1",
                crate::models::MessageRole::User,
                "Hello, this is a test message".to_string(),
            );
            app.cache.add_message_simple(
                "test-thread-1",
                crate::models::MessageRole::Assistant,
                "This is a response from the assistant".to_string(),
            );
        }

        let result = terminal.draw(|f| render(f, &mut app));
        assert!(result.is_ok(), "Render should succeed at {}x{}", width, height);

        let buffer = terminal.backend().buffer();
        let has_content = buffer.content().iter().any(|cell| cell.symbol() != " ");
        assert!(has_content, "Should render some content at {}x{}", width, height);
    }

    // ========================================================================
    // 40x20 - Narrow and Short Terminal (Edge case)
    // ========================================================================

    #[test]
    fn test_responsive_40x20_command_deck() {
        verify_render_succeeds(40, 20, Screen::CommandDeck);
    }

    #[test]
    fn test_responsive_40x20_conversation() {
        verify_render_succeeds(40, 20, Screen::Conversation);
    }

    #[test]
    fn test_responsive_40x20_layout_context() {
        let ctx = LayoutContext::new(40, 20);

        // Verify layout decisions
        assert!(ctx.is_narrow(), "40 cols should be narrow");
        assert!(ctx.is_short(), "20 rows should be short");
        assert!(ctx.is_compact(), "40x20 should be compact");
        assert!(ctx.should_stack_panels(), "40 cols should trigger panel stacking");
        assert!(ctx.should_collapse_sidebar(), "40 cols should collapse sidebar");

        // Verify panel widths are reasonable (equal split for narrow)
        let (left, right) = ctx.two_column_widths();
        assert_eq!(left, 20, "Left panel should be half width at 40 cols");
        assert_eq!(right, 20, "Right panel should be half width at 40 cols");

        // Verify header/input heights are reduced
        assert_eq!(ctx.header_height(), 3, "Header should be compact at 40x20");
        assert_eq!(ctx.input_area_height(), 4, "Input area should be compact at 40x20");
    }

    // ========================================================================
    // 80x24 - Standard Terminal (Default case)
    // ========================================================================

    #[test]
    fn test_responsive_80x24_command_deck() {
        verify_render_succeeds(80, 24, Screen::CommandDeck);
    }

    #[test]
    fn test_responsive_80x24_conversation() {
        verify_render_succeeds(80, 24, Screen::Conversation);
    }

    #[test]
    fn test_responsive_80x24_layout_context() {
        let ctx = LayoutContext::new(80, 24);

        // Verify layout decisions
        assert!(!ctx.is_narrow(), "80 cols should not be narrow");
        assert!(!ctx.is_short(), "24 rows should not be short");
        assert!(!ctx.is_compact(), "80x24 should not be compact");
        assert!(!ctx.should_stack_panels(), "80 cols should not stack panels");
        assert!(!ctx.should_collapse_sidebar(), "80 cols should not collapse sidebar");

        // Verify panel widths (40/60 split for medium)
        let (left, right) = ctx.two_column_widths();
        assert_eq!(left, 32, "Left panel should be 40% at 80 cols");
        assert_eq!(right, 48, "Right panel should be 60% at 80 cols");

        // Verify header/input heights are normal
        assert_eq!(ctx.header_height(), 9, "Header should be normal at 80x24");
        assert_eq!(ctx.input_area_height(), 6, "Input area should be normal at 80x24");
    }

    #[test]
    fn test_responsive_80x24_with_messages() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;
        app.active_thread_id = Some("test-thread".to_string());

        // Add thread with multiple messages
        app.cache.upsert_thread(crate::models::Thread {
            id: "test-thread".to_string(),
            title: "Multi-message Thread".to_string(),
            description: None,
            preview: "Preview".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Conversation,
            model: None,
            permission_mode: None,
            message_count: 4,
            working_directory: None,
            created_at: chrono::Utc::now(),
        });

        for i in 0..4 {
            let role = if i % 2 == 0 { crate::models::MessageRole::User } else { crate::models::MessageRole::Assistant };
            app.cache.add_message_simple("test-thread", role, format!("Message {}", i));
        }

        terminal.draw(|f| render(f, &mut app)).unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer.content().iter().map(|cell| cell.symbol()).collect();

        // Verify messages are rendered
        assert!(buffer_str.contains("Message"), "Messages should be visible at 80x24");
    }

    // ========================================================================
    // 120x40 - Medium-Large Terminal
    // ========================================================================

    #[test]
    fn test_responsive_120x40_command_deck() {
        verify_render_succeeds(120, 40, Screen::CommandDeck);
    }

    #[test]
    fn test_responsive_120x40_conversation() {
        verify_render_succeeds(120, 40, Screen::Conversation);
    }

    #[test]
    fn test_responsive_120x40_layout_context() {
        let ctx = LayoutContext::new(120, 40);

        // Verify layout decisions
        assert!(!ctx.is_narrow(), "120 cols should not be narrow");
        assert!(!ctx.is_short(), "40 rows should not be short");
        assert!(!ctx.is_compact(), "120x40 should not be compact");
        assert!(!ctx.should_stack_panels(), "120 cols should not stack panels");

        // At 120 cols, we're at the boundary for wide layout
        let (left, right) = ctx.two_column_widths();
        assert_eq!(left, 42, "Left panel should be 35% at 120 cols");
        assert_eq!(right, 78, "Right panel should be 65% at 120 cols");

        // Verify size category
        // Note: 120 is >= MD_WIDTH (120), so it's Large (not Medium)
        assert_eq!(ctx.width_category(), layout::SizeCategory::Large);
        assert_eq!(ctx.height_category(), layout::SizeCategory::Large);
    }

    #[test]
    fn test_responsive_120x40_with_programming_thread() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;
        app.active_thread_id = Some("prog-thread".to_string());

        // Add a programming thread
        app.cache.upsert_thread(crate::models::Thread {
            id: "prog-thread".to_string(),
            title: "Programming Task".to_string(),
            description: Some("A coding task".to_string()),
            preview: "Code preview".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Programming,
            model: Some("claude-opus".to_string()),
            permission_mode: None,
            message_count: 2,
            working_directory: None,
            created_at: chrono::Utc::now(),
        });

        app.cache.add_message_simple("prog-thread", crate::models::MessageRole::User, "Write some code".to_string());
        app.cache.add_message_simple("prog-thread", crate::models::MessageRole::Assistant, "Here is the code...".to_string());

        terminal.draw(|f| render(f, &mut app)).unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer.content().iter().map(|cell| cell.symbol()).collect();

        // Verify programming indicator is visible
        assert!(buffer_str.contains("Programming") || buffer_str.contains("PROGRAMMING"),
            "Programming indicator should be visible at 120x40");
    }

    // ========================================================================
    // 200x50 - Wide Terminal
    // ========================================================================

    #[test]
    fn test_responsive_200x50_command_deck() {
        verify_render_succeeds(200, 50, Screen::CommandDeck);
    }

    #[test]
    fn test_responsive_200x50_conversation() {
        verify_render_succeeds(200, 50, Screen::Conversation);
    }

    #[test]
    fn test_responsive_200x50_layout_context() {
        let ctx = LayoutContext::new(200, 50);

        // Verify layout decisions
        assert!(!ctx.is_narrow(), "200 cols should not be narrow");
        assert!(!ctx.is_short(), "50 rows should not be short");
        assert!(!ctx.is_compact(), "200x50 should not be compact");
        assert!(!ctx.should_stack_panels(), "200 cols should not stack panels");

        // At 200 cols, left panel should be capped at 60
        let (left, right) = ctx.two_column_widths();
        assert_eq!(left, 60, "Left panel should be capped at 60 for wide terminals");
        assert_eq!(right, 140, "Right panel gets remaining space");

        // Verify size category
        assert_eq!(ctx.width_category(), layout::SizeCategory::Large);
        assert_eq!(ctx.height_category(), layout::SizeCategory::Large);

        // Verify full features are enabled
        assert!(ctx.should_show_scrollbar(), "Scrollbar should be shown at 200x50");
        assert!(ctx.should_show_full_badges(), "Full badges should be shown at 200x50");
        assert!(ctx.should_show_tool_previews(), "Tool previews should be shown at 200x50");
    }

    #[test]
    fn test_responsive_200x50_with_long_content() {
        let backend = TestBackend::new(200, 50);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::Conversation;
        app.active_thread_id = Some("long-thread".to_string());

        // Add thread with long content
        app.cache.upsert_thread(crate::models::Thread {
            id: "long-thread".to_string(),
            title: "A thread with a very long title that should be fully visible on wide terminals".to_string(),
            description: Some("A detailed description that provides more context".to_string()),
            preview: "Preview text".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Conversation,
            model: Some("claude-opus-4-5".to_string()),
            permission_mode: None,
            message_count: 10,
            working_directory: None,
            created_at: chrono::Utc::now(),
        });

        // Add a long message
        let long_message = "This is a very long message that would typically wrap on smaller terminals. ".repeat(10);
        app.cache.add_message_simple("long-thread", crate::models::MessageRole::User, long_message.clone());
        app.cache.add_message_simple("long-thread", crate::models::MessageRole::Assistant, "Response to the long message".to_string());

        terminal.draw(|f| render(f, &mut app)).unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer.content().iter().map(|cell| cell.symbol()).collect();

        // Verify thread title is visible
        assert!(buffer_str.contains("long title") || buffer_str.contains("long-thread"),
            "Thread title should be visible at 200x50");
    }

    // ========================================================================
    // Edge Case Tests
    // ========================================================================

    #[test]
    fn test_responsive_boundary_60x24() {
        // Test at the XS_WIDTH boundary
        verify_render_succeeds(60, 24, Screen::CommandDeck);
        verify_render_succeeds(60, 24, Screen::Conversation);

        let ctx = LayoutContext::new(60, 24);
        // At 60, should NOT be extra small but should still be small
        assert!(!ctx.is_extra_small(), "60 cols should not be extra small");
        assert!(ctx.is_narrow(), "60 cols should be narrow (< 80)");
    }

    #[test]
    fn test_responsive_boundary_80x24() {
        // Test at the SM_WIDTH boundary
        let ctx = LayoutContext::new(80, 24);
        assert!(!ctx.is_narrow(), "80 cols should not be narrow");
        assert_eq!(ctx.width_category(), layout::SizeCategory::Medium);
    }

    #[test]
    fn test_responsive_boundary_120x40() {
        // Test at the MD_WIDTH and MD_HEIGHT boundary
        // 120 is >= MD_WIDTH (120), so it's Large
        // 40 is >= MD_HEIGHT (40), so it's Large
        let ctx = LayoutContext::new(120, 40);
        assert_eq!(ctx.width_category(), layout::SizeCategory::Large);
        assert_eq!(ctx.height_category(), layout::SizeCategory::Large);
    }

    #[test]
    fn test_responsive_extreme_narrow_30x24() {
        // Terminal at minimum width (30) - should render normally
        let backend = TestBackend::new(30, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        // Set app dimensions to match test backend
        app.terminal_width = 30;
        app.terminal_height = 24;

        terminal.draw(|f| render(f, &mut app)).unwrap();

        let buffer = terminal.backend().buffer();

        // At exactly minimum width, it should work (MIN_TERMINAL_WIDTH = 30)
        let has_content = buffer.content().iter().any(|cell| cell.symbol() != " ");
        assert!(has_content, "Should render at minimum width");
    }

    #[test]
    fn test_responsive_extreme_short_80x9() {
        // Very short terminal - BELOW minimum height threshold (MIN_TERMINAL_HEIGHT = 10)
        let backend = TestBackend::new(80, 9);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.terminal_width = 80;
        app.terminal_height = 9;

        terminal.draw(|f| render(f, &mut app)).unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer.content().iter().map(|cell| cell.symbol()).collect();

        // Should show "too small" message for height below MIN_TERMINAL_HEIGHT (10)
        assert!(buffer_str.contains("Terminal Too Small"),
            "Terminal below minimum height should show 'Terminal Too Small' message");
    }

    #[test]
    fn test_responsive_at_minimum_80x10() {
        // Terminal exactly at minimum height - should work normally
        let backend = TestBackend::new(80, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.terminal_width = 80;
        app.terminal_height = 10;

        terminal.draw(|f| render(f, &mut app)).unwrap();

        let buffer = terminal.backend().buffer();
        let has_content = buffer.content().iter().any(|cell| cell.symbol() != " ");

        // At minimum size, it should render normally (not show "too small")
        assert!(has_content, "Terminal at minimum size should render content");
    }

    #[test]
    fn test_responsive_various_sizes_no_panic() {
        // Test a variety of sizes to ensure no panics
        // Note: All sizes must be >= minimum (30x10)
        let sizes = [
            (40, 20),
            (50, 20),
            (60, 16),
            (70, 20),
            (80, 24),
            (100, 30),
            (120, 40),
            (150, 45),
            (200, 50),
            (250, 60),
        ];

        for (width, height) in sizes.iter() {
            let backend = TestBackend::new(*width, *height);
            let mut terminal = Terminal::new(backend).unwrap();
            let mut app = create_test_app();
            // Set app dimensions to match test backend
            app.terminal_width = *width;
            app.terminal_height = *height;

            // Test both screens
            for screen in [Screen::CommandDeck, Screen::Conversation] {
                app.screen = screen;
                let result = terminal.draw(|f| render(f, &mut app));
                assert!(result.is_ok(), "Render should not panic at {}x{} on {:?}", width, height, screen);
            }
        }
    }

    #[test]
    fn test_responsive_layout_consistency() {
        // Verify that layout calculations are consistent
        let sizes = [
            (40, 20, true, true),    // (width, height, expect_narrow, expect_short)
            (60, 16, true, true),
            (80, 24, false, false),
            (120, 40, false, false),
            (200, 50, false, false),
        ];

        for (width, height, expect_narrow, expect_short) in sizes.iter() {
            let ctx = LayoutContext::new(*width, *height);
            assert_eq!(ctx.is_narrow(), *expect_narrow,
                "is_narrow() mismatch at {}x{}", width, height);
            assert_eq!(ctx.is_short(), *expect_short,
                "is_short() mismatch at {}x{}", width, height);

            // Verify panel widths sum to total width
            let (left, right) = ctx.two_column_widths();
            assert_eq!(left + right, *width,
                "Panel widths should sum to total width at {}x{}", width, height);
        }
    }
}
