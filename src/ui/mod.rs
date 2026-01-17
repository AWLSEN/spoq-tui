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
pub use helpers::format_tool_args;

// Re-export rendering functions for external use
pub use messages::{estimate_wrapped_line_count, render_tool_result_preview, truncate_preview};

use ratatui::Frame;

use crate::app::{App, Screen};
use command_deck::render_command_deck;
use conversation::render_conversation_screen;
use thread_switcher::render_thread_switcher;

// ============================================================================
// Main UI Rendering
// ============================================================================

/// Render the UI based on current screen
pub fn render(frame: &mut Frame, app: &mut App) {
    match app.screen {
        Screen::CommandDeck => render_command_deck(frame, app),
        Screen::Conversation => render_conversation_screen(frame, app),
    }

    // Render thread switcher overlay (if visible)
    render_thread_switcher(frame, app);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::ProgrammingMode;
    use conversation::create_mode_indicator_line;
    use helpers::{extract_short_model_name, format_tokens, get_tool_icon, truncate_string};
    use input::{build_contextual_keybinds, get_permission_preview};
    use messages::{render_tool_event, render_tool_result_preview, truncate_preview};
    use ratatui::{backend::TestBackend, Terminal};
    use theme::COLOR_TOOL_ERROR;

    fn create_test_app() -> App {
        let (message_tx, message_rx) = tokio::sync::mpsc::unbounded_channel();
        App {
            threads: vec![],
            tasks: vec![],
            todos: vec![],
            should_quit: false,
            screen: Screen::CommandDeck,
            active_thread_id: None,
            focus: crate::app::Focus::default(),
            notifications_index: 0,
            tasks_index: 0,
            threads_index: 0,
            input_box: crate::widgets::input_box::InputBox::new(),
            migration_progress: None,
            cache: crate::cache::ThreadCache::new(),
            message_rx: Some(message_rx),
            message_tx,
            connection_status: false,
            stream_error: None,
            client: std::sync::Arc::new(crate::conductor::ConductorClient::new()),
            tick_count: 0,
            conversation_scroll: 0,
            max_scroll: 0,
            programming_mode: ProgrammingMode::default(),
            session_state: crate::state::SessionState::new(),
            tool_tracker: crate::state::ToolTracker::new(),
            subagent_tracker: crate::state::SubagentTracker::new(),
            debug_tx: None,
            stream_start_time: None,
            last_event_time: None,
            cumulative_token_count: 0,
            thread_switcher: crate::app::ThreadSwitcher::default(),
            last_tab_press: None,
            ws_sender: None,
            ws_connection_state: crate::websocket::WsConnectionState::Disconnected,
            question_state: crate::state::AskUserQuestionState::default(),
            scroll_boundary_hit: None,
            boundary_hit_tick: 0,
            scroll_velocity: 0.0,
            scroll_position: 0.0,
            terminal_width: 80,
            terminal_height: 24,
            active_panel: crate::app::ActivePanel::default(),
        }
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
        app.input_box.insert_char('H');
        app.input_box.insert_char('e');
        app.input_box.insert_char('l');
        app.input_box.insert_char('l');
        app.input_box.insert_char('o');

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
    fn test_create_mode_indicator_line_plan_mode() {
        let line = create_mode_indicator_line(ProgrammingMode::PlanMode);
        assert!(line.is_some());
        let line = line.unwrap();
        // Check that the line contains "[PLAN MODE]"
        let content: String = line.spans.iter().map(|s| s.content.to_string()).collect();
        assert!(content.contains("[PLAN MODE]"));
    }

    #[test]
    fn test_create_mode_indicator_line_bypass() {
        let line = create_mode_indicator_line(ProgrammingMode::BypassPermissions);
        assert!(line.is_some());
        let line = line.unwrap();
        // Check that the line contains "[BYPASS]"
        let content: String = line.spans.iter().map(|s| s.content.to_string()).collect();
        assert!(content.contains("[BYPASS]"));
    }

    #[test]
    fn test_create_mode_indicator_line_none() {
        let line = create_mode_indicator_line(ProgrammingMode::None);
        assert!(line.is_none());
    }

    #[test]
    fn test_mode_indicator_not_shown_for_conversation_thread() {
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
            created_at: chrono::Utc::now(),
        });
        app.active_thread_id = Some("conv-thread".to_string());
        app.programming_mode = ProgrammingMode::PlanMode; // Set mode, but shouldn't show

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

        // Mode indicator should NOT be shown for Conversation threads
        assert!(
            !buffer_str.contains("[PLAN MODE]"),
            "Mode indicator should not be shown for Conversation threads"
        );
        assert!(
            !buffer_str.contains("[BYPASS]"),
            "Mode indicator should not be shown for Conversation threads"
        );
    }

    #[test]
    fn test_mode_indicator_shown_for_programming_thread_plan_mode() {
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
            created_at: chrono::Utc::now(),
        });
        app.active_thread_id = Some("prog-thread".to_string());
        app.programming_mode = ProgrammingMode::PlanMode;

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
            buffer_str.contains("[PLAN MODE]"),
            "Mode indicator should show '[PLAN MODE]' for Programming thread in PlanMode"
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
            created_at: chrono::Utc::now(),
        });
        app.active_thread_id = Some("prog-thread".to_string());
        app.programming_mode = ProgrammingMode::BypassPermissions;

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
            buffer_str.contains("[BYPASS]"),
            "Mode indicator should show '[BYPASS]' for Programming thread in BypassPermissions"
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
            created_at: chrono::Utc::now(),
        });
        app.active_thread_id = Some("prog-thread".to_string());
        app.programming_mode = ProgrammingMode::None;

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

        // When mode is None, no indicator should be shown
        assert!(
            !buffer_str.contains("[PLAN MODE]"),
            "Mode indicator should not show '[PLAN MODE]' when mode is None"
        );
        assert!(
            !buffer_str.contains("[BYPASS]"),
            "Mode indicator should not show '[BYPASS]' when mode is None"
        );
    }

    #[test]
    fn test_mode_indicator_not_shown_on_command_deck() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = Screen::CommandDeck; // Not on Conversation screen
        app.programming_mode = ProgrammingMode::PlanMode;

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
        app.programming_mode = ProgrammingMode::PlanMode;

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
            created_at: chrono::Utc::now(),
        });
        app.active_thread_id = Some("conv-thread".to_string());

        let keybinds = build_contextual_keybinds(&app);
        let content: String = keybinds.spans.iter().map(|s| s.content.to_string()).collect();

        // Should NOT show mode cycling hint for normal thread
        assert!(!content.contains("Shift+Tab"));
        assert!(!content.contains("cycle mode"));
    }

    // ============= Phase 10: Streaming Input Border Tests =============

    #[test]
    fn test_conversation_input_uses_dashed_border_when_streaming() {
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

        // Should have dashed border characters (┄)
        assert!(
            buffer_str.contains("┄"),
            "Input should use dashed border when streaming"
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
        assert_eq!(get_tool_icon("Read"), "📄");
        assert_eq!(get_tool_icon("Write"), "📝");
        assert_eq!(get_tool_icon("Edit"), "✏️");
        assert_eq!(get_tool_icon("Bash"), "$");
        assert_eq!(get_tool_icon("Grep"), "🔍");
        assert_eq!(get_tool_icon("Glob"), "🔍");
        assert_eq!(get_tool_icon("Task"), "🤖");
        assert_eq!(get_tool_icon("WebFetch"), "🌐");
        assert_eq!(get_tool_icon("WebSearch"), "🌐");
        assert_eq!(get_tool_icon("TodoWrite"), "📋");
        assert_eq!(get_tool_icon("AskUserQuestion"), "❓");
        assert_eq!(get_tool_icon("NotebookEdit"), "📓");
    }

    #[test]
    fn test_get_tool_icon_unknown_tool() {
        assert_eq!(get_tool_icon("UnknownTool"), "⚙️");
        assert_eq!(get_tool_icon("CustomFunction"), "⚙️");
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
    fn test_render_tool_result_preview_none_when_no_preview() {
        let mut tool = crate::models::ToolEvent::new("tool_123".to_string(), "Bash".to_string());
        tool.result_preview = None;

        let result = render_tool_result_preview(&tool);
        assert!(result.is_none());
    }

    #[test]
    fn test_render_tool_result_preview_none_when_empty_preview() {
        let mut tool = crate::models::ToolEvent::new("tool_123".to_string(), "Bash".to_string());
        tool.result_preview = Some("   ".to_string());

        let result = render_tool_result_preview(&tool);
        assert!(result.is_none());
    }

    #[test]
    fn test_render_tool_result_preview_success_result() {
        let mut tool = crate::models::ToolEvent::new("tool_123".to_string(), "Read".to_string());
        tool.set_result("File contents here", false);

        let result = render_tool_result_preview(&tool);
        assert!(result.is_some());

        let line = result.unwrap();
        assert_eq!(line.spans.len(), 2);
        // First span is indentation
        assert_eq!(line.spans[0].content, "    ");
        // Second span contains the preview
        assert_eq!(line.spans[1].content, "File contents here");
        // Success results use dim gray color
        assert_eq!(line.spans[1].style.fg, Some(ratatui::style::Color::Rgb(100, 100, 100)));
    }

    #[test]
    fn test_render_tool_result_preview_error_result() {
        let mut tool = crate::models::ToolEvent::new("tool_123".to_string(), "Read".to_string());
        tool.set_result("File not found", true);

        let result = render_tool_result_preview(&tool);
        assert!(result.is_some());

        let line = result.unwrap();
        assert_eq!(line.spans.len(), 2);
        // Second span contains the preview
        assert_eq!(line.spans[1].content, "File not found");
        // Error results use red color
        assert_eq!(line.spans[1].style.fg, Some(COLOR_TOOL_ERROR));
    }

    #[test]
    fn test_render_tool_result_preview_truncates_long_content() {
        let mut tool = crate::models::ToolEvent::new("tool_123".to_string(), "Bash".to_string());
        let long_content = "a".repeat(200);
        tool.set_result(&long_content, false);

        let result = render_tool_result_preview(&tool);
        assert!(result.is_some());

        let line = result.unwrap();
        let preview = &line.spans[1].content;
        assert!(preview.len() <= 153); // 150 chars + "..."
        assert!(preview.ends_with("..."));
    }

    #[test]
    fn test_render_tool_result_preview_multiline_truncation() {
        let mut tool = crate::models::ToolEvent::new("tool_123".to_string(), "Bash".to_string());
        let multiline = "line1\nline2\nline3\nline4\nline5";
        tool.set_result(multiline, false);

        let result = render_tool_result_preview(&tool);
        assert!(result.is_some());

        let line = result.unwrap();
        let preview = &line.spans[1].content;
        // Should truncate at 2 lines
        assert!(preview.ends_with("..."));
        // Newlines should be replaced with spaces
        assert!(!preview.contains('\n'));
        assert!(preview.contains("line1 line2"));
    }

    #[test]
    fn test_format_tool_args_empty_json() {
        // Empty JSON object should return just the tool name
        assert_eq!(helpers::format_tool_args("Read", "{}"), "Read");
        assert_eq!(helpers::format_tool_args("Bash", "{}"), "Bash");
        assert_eq!(helpers::format_tool_args("Write", "{}"), "Write");
    }

    #[test]
    fn test_render_tool_result_preview_exactly_at_boundary() {
        let mut tool = crate::models::ToolEvent::new("tool_123".to_string(), "Bash".to_string());
        // Create text exactly at the 150 character boundary
        let exactly_150 = "a".repeat(150);
        tool.set_result(&exactly_150, false);

        let result = render_tool_result_preview(&tool);
        assert!(result.is_some());

        let line = result.unwrap();
        let preview = &line.spans[1].content;
        // At exactly 150 chars, truncate_preview will still truncate (>= condition)
        assert_eq!(preview.len(), 153); // 150 chars + "..."
        assert!(preview.ends_with("..."));
    }

    #[test]
    fn test_render_tool_result_preview_one_char_over_boundary() {
        let mut tool = crate::models::ToolEvent::new("tool_123".to_string(), "Bash".to_string());
        // Create text one character over the boundary
        let one_over = "a".repeat(151);
        tool.set_result(&one_over, false);

        let result = render_tool_result_preview(&tool);
        assert!(result.is_some());

        let line = result.unwrap();
        let preview = &line.spans[1].content;
        // Should truncate
        assert_eq!(preview.len(), 153); // 150 chars + "..."
        assert!(preview.ends_with("..."));
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

        let line = render_tool_event(&tool, 0);

        // Verify the line contains expected elements
        let line_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();

        // Should contain icon
        assert!(line_text.contains("📄"));

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

        let line = render_tool_event(&tool, 0);

        // Verify the line contains expected elements
        let line_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();

        // Should contain Bash icon ($ not ⚙️)
        assert!(line_text.contains("$"));

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
        let line = render_tool_event(&tool, 0);

        let line_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();

        // Should contain icon
        assert!(line_text.contains("🔍"));

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
        let line1 = render_tool_event(&tool, 0);
        let text1: String = line1.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text1.contains("📝"));  // Write icon is 📝 not ✍
        assert!(text1.contains("Write")); // Default to tool name

        // Step 2: Args stream in
        tool.args_json = r#"{"file_path": "/tmp/test.txt", "content": "Hello"}"#.to_string();
        tool.args_display = Some("Writing /tmp/test.txt".to_string());
        let line2 = render_tool_event(&tool, 0);
        let text2: String = line2.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text2.contains("Writing /tmp/test.txt"));

        // Step 3: Result comes back
        tool.set_result("File written successfully", false);
        tool.complete();
        let line3 = render_tool_event(&tool, 0);
        let text3: String = line3.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text3.contains("✓")); // Success indicator
        assert!(text3.contains("Writing /tmp/test.txt"));
    }

    // ============= Phase 8: Subagent Rendering Tests =============

    #[test]
    fn test_get_subagent_icon() {
        use helpers::get_subagent_icon;

        assert_eq!(get_subagent_icon("Explore"), "🔍");
        assert_eq!(get_subagent_icon("Bash"), "$");
        assert_eq!(get_subagent_icon("Plan"), "📋");
        assert_eq!(get_subagent_icon("general-purpose"), "🤖");
        assert_eq!(get_subagent_icon("unknown"), "●");
        assert_eq!(get_subagent_icon("CustomAgent"), "●");
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

        let lines = render_subagent_event(&event, 0, TreeConnector::Single);

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

        let lines = render_subagent_event(&event, 0, TreeConnector::Single);

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

        let lines = render_subagent_event(&event, 0, TreeConnector::Single);

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

        let lines = render_subagent_event(&event, 0, TreeConnector::Single);

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

        let lines = render_subagent_event(&event, 0, TreeConnector::Branch);

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

        let lines = render_subagent_event(&event, 0, TreeConnector::LastBranch);

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
        let lines = render_subagent_events_block(&events, 0);

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
        let lines = render_subagent_events_block(&events, 0);

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
        let lines = render_subagent_events_block(&events, 0);

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

        let lines = render_subagent_event(&event, 0, TreeConnector::Single);

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

        // Test at different tick counts to verify spinner changes
        let lines_tick_0 = render_subagent_event(&event, 0, TreeConnector::Single);
        let lines_tick_5 = render_subagent_event(&event, 5, TreeConnector::Single);

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
}
