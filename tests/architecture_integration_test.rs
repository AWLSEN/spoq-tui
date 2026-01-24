//! Integration tests for the new architecture patterns.
//!
//! These tests verify that the dependency injection patterns work correctly
//! and that the new architectural modules (input, startup, terminal, cli)
//! integrate properly with the rest of the application.

mod common;

use common::{test_app, test_app_with_thread, test_credentials, MockHttpConfig};
use spoq::app::{Focus, Screen};
use spoq::cli::{parse_args, run_cli_command, CliCommand};
use spoq::input::{Command, CommandRegistry, InputContext, ModalType};
use spoq::startup::{StartupConfig, StartupResult};

// =============================================================================
// CLI Module Integration Tests
// =============================================================================

#[test]
fn test_cli_parse_args_version() {
    let args = vec!["spoq".to_string(), "--version".to_string()];
    let cmd = parse_args(args.into_iter());
    assert_eq!(cmd, CliCommand::Version);
}

#[test]
fn test_cli_parse_args_update() {
    let args = vec!["spoq".to_string(), "--update".to_string()];
    let cmd = parse_args(args.into_iter());
    assert_eq!(cmd, CliCommand::Update);
}

#[test]
fn test_cli_parse_args_sync() {
    let args = vec!["spoq".to_string(), "--sync".to_string()];
    let cmd = parse_args(args.into_iter());
    assert_eq!(cmd, CliCommand::Sync);
}

#[test]
fn test_cli_parse_args_default() {
    let args = vec!["spoq".to_string()];
    let cmd = parse_args(args.into_iter());
    assert_eq!(cmd, CliCommand::RunTui);
}

#[test]
fn test_cli_run_tui_returns_none() {
    let result = run_cli_command(CliCommand::RunTui);
    assert!(result.is_none(), "RunTui should return None to continue to TUI");
}

// =============================================================================
// Startup Module Integration Tests
// =============================================================================

#[test]
fn test_startup_config_default_values() {
    let config = StartupConfig::default();

    assert!(config.skip_update_check);
    assert!(!config.skip_vps_check);
    assert!(!config.skip_health_check);
    assert!(config.enable_debug);
    assert_eq!(config.debug_port, 3030);
}

#[test]
fn test_startup_config_builder_pattern() {
    let config = StartupConfig::new()
        .with_skip_update_check(false)
        .with_skip_vps_check(true)
        .with_skip_health_check(true)
        .with_enable_debug(false)
        .with_debug_port(4040);

    assert!(!config.skip_update_check);
    assert!(config.skip_vps_check);
    assert!(config.skip_health_check);
    assert!(!config.enable_debug);
    assert_eq!(config.debug_port, 4040);
}

#[test]
fn test_startup_result_creation() {
    let creds = test_credentials();
    let result = StartupResult::new(creds.clone());

    assert_eq!(result.credentials.access_token, creds.access_token);
    assert!(result.vps_state.is_none());
    assert!(result.vps_url.is_none());
    assert!(result.debug_tx.is_none());
}

// =============================================================================
// Input Module Integration Tests
// =============================================================================

#[test]
fn test_command_registry_creation() {
    let registry = CommandRegistry::new();
    let config = registry.config();

    // Verify essential bindings exist
    assert!(config.global.len() > 0, "Should have global bindings");
    assert!(config.input_editing.len() > 0, "Should have input editing bindings");
}

#[test]
fn test_input_context_default() {
    let ctx = InputContext::default();

    assert_eq!(ctx.screen, Screen::CommandDeck);
    assert_eq!(ctx.focus, Focus::Threads);
    assert_eq!(ctx.modal, ModalType::None);
    assert!(ctx.input_is_empty);
}

#[test]
fn test_input_context_builder() {
    let ctx = InputContext::new()
        .with_screen(Screen::Conversation)
        .with_focus(Focus::Input)
        .with_modal(ModalType::FolderPicker)
        .with_input_empty(false);

    assert_eq!(ctx.screen, Screen::Conversation);
    assert_eq!(ctx.focus, Focus::Input);
    assert_eq!(ctx.modal, ModalType::FolderPicker);
    assert!(!ctx.input_is_empty);
}

#[test]
fn test_command_dispatch_quit() {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    let registry = CommandRegistry::new();
    let context = InputContext::default();

    let key = KeyEvent {
        code: KeyCode::Char('c'),
        modifiers: KeyModifiers::CONTROL,
        kind: KeyEventKind::Press,
        state: crossterm::event::KeyEventState::NONE,
    };

    let cmd = registry.dispatch(key, &context);
    assert!(matches!(cmd, Some(Command::Quit)));
}

#[test]
fn test_command_dispatch_character_input() {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    let registry = CommandRegistry::new();
    let context = InputContext::new().with_focus(Focus::Input);

    let key = KeyEvent {
        code: KeyCode::Char('a'),
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: crossterm::event::KeyEventState::NONE,
    };

    let cmd = registry.dispatch(key, &context);
    assert!(matches!(cmd, Some(Command::InsertChar('a'))));
}

#[test]
fn test_command_dispatch_modal_escape() {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    let registry = CommandRegistry::new();
    let context = InputContext::new().with_modal(ModalType::FolderPicker);

    let key = KeyEvent {
        code: KeyCode::Esc,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: crossterm::event::KeyEventState::NONE,
    };

    let cmd = registry.dispatch(key, &context);
    assert!(matches!(cmd, Some(Command::CloseFolderPicker)));
}

#[test]
fn test_command_marks_dirty() {
    assert!(Command::InsertChar('a').marks_dirty());
    assert!(Command::Quit.marks_dirty());
    assert!(!Command::Noop.marks_dirty());
    assert!(!Command::Tick.marks_dirty());
}

#[test]
fn test_command_is_quit() {
    assert!(Command::Quit.is_quit());
    assert!(Command::ForceQuit.is_quit());
    assert!(!Command::InsertChar('a').is_quit());
    assert!(!Command::Noop.is_quit());
}

// =============================================================================
// App + Input Integration Tests
// =============================================================================

#[test]
fn test_app_build_input_context() {
    let app = test_app();
    let ctx = app.build_input_context();

    assert_eq!(ctx.screen, Screen::CommandDeck);
    assert_eq!(ctx.focus, Focus::Threads);
    assert_eq!(ctx.modal, ModalType::None);
}

#[test]
fn test_app_build_input_context_with_folder_picker() {
    let mut app = test_app();
    app.folder_picker_visible = true;

    let ctx = app.build_input_context();
    assert_eq!(ctx.modal, ModalType::FolderPicker);
}

#[test]
fn test_app_build_input_context_with_thread_switcher() {
    let mut app = test_app();
    app.thread_switcher.visible = true;

    let ctx = app.build_input_context();
    assert_eq!(ctx.modal, ModalType::ThreadSwitcher);
}

#[test]
fn test_app_execute_command_quit() {
    let mut app = test_app();
    assert!(!app.should_quit);

    let handled = app.execute_command(Command::Quit);

    assert!(handled);
    assert!(app.should_quit);
}

#[test]
fn test_app_execute_command_insert_char() {
    let mut app = test_app();

    let handled = app.execute_command(Command::InsertChar('x'));

    assert!(handled);
    assert!(app.textarea.content().contains('x'));
}

#[test]
fn test_app_execute_command_noop() {
    let mut app = test_app();

    let handled = app.execute_command(Command::Noop);

    assert!(handled);
}

#[test]
fn test_full_dispatch_execute_flow() {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    let mut app = test_app();
    let registry = CommandRegistry::new();
    let context = app.build_input_context();

    // Simulate pressing 'h' key
    let key = KeyEvent {
        code: KeyCode::Char('h'),
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: crossterm::event::KeyEventState::NONE,
    };

    if let Some(cmd) = registry.dispatch(key, &context) {
        let handled = app.execute_command(cmd);
        assert!(handled);
        assert!(app.textarea.content().contains('h'));
    } else {
        panic!("Expected command to be dispatched");
    }
}

// =============================================================================
// Mock Adapters Integration Tests
// =============================================================================

#[tokio::test]
async fn test_mock_http_client_integration() {
    use spoq::traits::{Headers, HttpClient};

    let client = MockHttpConfig::new()
        .with_json_response(
            "https://api.test.com/status",
            200,
            r#"{"status": "ok"}"#,
        )
        .build();

    let response = client
        .get("https://api.test.com/status", &Headers::new())
        .await
        .unwrap();

    assert_eq!(response.status, 200);
    assert!(String::from_utf8_lossy(&response.body).contains("ok"));
}

#[tokio::test]
async fn test_mock_credentials_integration() {
    use spoq::adapters::mock::InMemoryCredentials;
    use spoq::traits::CredentialsProvider;

    let provider = InMemoryCredentials::new();

    // Initially empty
    let loaded = provider.load().await.unwrap();
    assert!(loaded.is_none());

    // Save credentials
    let creds = test_credentials();
    provider.save(&creds).await.unwrap();

    // Load them back
    let loaded = provider.load().await.unwrap().unwrap();
    assert_eq!(loaded.access_token, creds.access_token);

    // Clear
    provider.clear().await.unwrap();
    let loaded = provider.load().await.unwrap();
    assert!(loaded.is_none());
}

#[tokio::test]
async fn test_mock_websocket_integration() {
    use spoq::adapters::mock::MockWebSocket;
    use spoq::traits::WebSocketConnection;
    use spoq::websocket::messages::{WsConnected, WsIncomingMessage};

    let mock = MockWebSocket::new();
    let mut rx = mock.subscribe();

    // Inject a message
    mock.inject_message(WsIncomingMessage::Connected(WsConnected {
        session_id: "test-session".to_string(),
        timestamp: 1234567890,
    }));

    // Receive the message
    let msg = rx.recv().await.unwrap();
    match msg {
        WsIncomingMessage::Connected(connected) => {
            assert_eq!(connected.session_id, "test-session");
        }
        _ => panic!("Expected Connected message"),
    }
}

// =============================================================================
// Test Utilities Integration Tests
// =============================================================================

#[test]
fn test_test_app_helper() {
    let app = test_app();
    assert!(app.active_thread_id.is_none());
    assert_eq!(app.cache.thread_count(), 0);
}

#[tokio::test]
async fn test_test_app_with_thread_helper() {
    let app = test_app_with_thread("Test Thread");
    assert!(app.active_thread_id.is_some());
    assert_eq!(app.cache.thread_count(), 1);
    assert_eq!(app.screen, Screen::Conversation);
}

#[test]
fn test_credentials_helpers() {
    use crate::common::{empty_credentials, expired_credentials};

    let valid = test_credentials();
    assert!(valid.access_token.is_some());
    assert_eq!(valid.expires_at, Some(i64::MAX));

    let expired = expired_credentials();
    assert_eq!(expired.expires_at, Some(0));

    let empty = empty_credentials();
    assert!(empty.access_token.is_none());
}
