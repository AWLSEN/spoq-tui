use spoq::app::{start_websocket_with_config, App, AppMessage, Focus, Screen, ScrollBoundary};
use spoq::websocket::WsClientConfig;
use spoq::auth::{run_auth_flow, run_provisioning_flow, start_stopped_vps, CredentialsManager};
use spoq::debug::{
    create_debug_channel, start_debug_server, DebugEvent, DebugEventKind, StateChangeData, StateType,
};
use spoq::models;
use spoq::ui;

use color_eyre::Result;
use crossterm::{
    cursor::Show,
    event::{
        DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        Event, EventStream, KeyCode, KeyEventKind, KeyModifiers, KeyboardEnhancementFlags,
        MouseButton, MouseEventKind, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::StreamExt;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> Result<()> {
    // Handle --version flag before any initialization
    if std::env::args().any(|arg| arg == "--version") {
        println!("spoq {}", VERSION);
        std::process::exit(0);
    }

    color_eyre::install()?;

    // Setup panic hook to ensure terminal cleanup on panic
    setup_panic_hook();

    // Create Tokio runtime for the entire application
    // This runtime will be used for auth flows and then for TUI async operations
    let runtime = tokio::runtime::Runtime::new()?;

    // =========================================================
    // Pre-flight auth checks - run BEFORE TUI starts
    // =========================================================

    // Load or create credentials
    let manager = CredentialsManager::new().expect("Failed to initialize credentials manager");
    let mut credentials = manager.load();

    // Auth check - run interactive flow if not authenticated
    if credentials.access_token.is_none() {
        credentials = match run_auth_flow(&runtime) {
            Ok(creds) => creds,
            Err(e) => {
                eprintln!("Authentication failed: {}", e);
                std::process::exit(1);
            }
        };
    }

    // VPS check - ensure VPS is in a usable state before launching TUI
    // State matrix handles all possible VPS states
    match credentials.vps_status.as_deref() {
        Some("ready") | Some("running") | Some("active") => {
            // Good to go - launch TUI
        }
        Some("stopped") => {
            // Auto-start existing VPS (don't re-provision!)
            match start_stopped_vps(&runtime, &credentials) {
                Ok(status) => {
                    credentials.vps_status = Some(status.status);
                    credentials.vps_ip = status.ip;
                    if !manager.save(&credentials) {
                        eprintln!("Warning: Failed to save credentials after starting VPS");
                    }
                }
                Err(e) => {
                    eprintln!("Failed to start VPS: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Some("failed") | Some("terminated") | None => {
            // Need to provision
            if let Err(e) = run_provisioning_flow(&runtime, &mut credentials) {
                eprintln!("Provisioning failed: {}", e);
                std::process::exit(1);
            }
            // Save updated credentials after provisioning
            if !manager.save(&credentials) {
                eprintln!("Warning: Failed to save credentials after provisioning");
            }
        }
        Some(other) => {
            // Unknown state - probably still provisioning
            eprintln!(
                "VPS in unexpected state: {}. Please wait or contact support.",
                other
            );
            std::process::exit(1);
        }
    }

    // At this point, user is authenticated AND has a ready VPS
    println!("Starting SPOQ...\n");

    // =========================================================
    // TUI initialization - user is now authenticated
    // =========================================================

    // Run async initialization using the runtime
    let (debug_tx, debug_server_handle) = runtime.block_on(start_debug_system());

    // Open debug dashboard in browser (fire and forget)
    if debug_tx.is_some() {
        let _ = open::that("http://localhost:3030");
    }

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();

    // Enable keyboard enhancement for modern terminals (Kitty protocol)
    // This allows Ctrl+Enter and Shift+Enter to work properly
    // Silently fails on unsupported terminals (Terminal.app, Warp, etc.)
    let _ = execute!(
        stdout,
        PushKeyboardEnhancementFlags(
            KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
        )
    );

    // Enter alternate screen, enable bracketed paste, and mouse capture for scroll events
    // Note: Mouse capture is enabled but click events are ignored in the handler,
    // allowing scroll wheel to work while terminal handles text selection natively
    execute!(
        stdout,
        EnterAlternateScreen,
        EnableBracketedPaste,
        EnableMouseCapture
    )?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Clear the terminal
    terminal.clear()?;

    // Initialize application state with debug sender
    let mut app = App::with_debug(debug_tx)?;

    // Log initial auth state for debugging
    app.log_initial_auth_state();

    // Capture initial terminal dimensions
    let size = terminal.size()?;
    app.update_terminal_dimensions(size.width, size.height);

    // Initialize server connection - user is already authenticated with ready VPS
    // Login and Provisioning screens are handled by pre-flight checks above
    runtime.block_on(async {
        // Load threads from backend (async initialization)
        app.initialize().await;

        // Load folders for the folder picker (async, non-blocking)
        app.load_folders();

        // Connect WebSocket for real-time communication
        // Build config with token from credentials (not just env var)
        let ws_config = if let Some(ref token) = app.credentials.access_token {
            WsClientConfig::default().with_auth(token)
        } else {
            WsClientConfig::default()
        };

        // Emit debug event showing connection attempt
        if let Some(ref tx) = app.debug_tx {
            let _ = tx.send(DebugEvent::new(DebugEventKind::StateChange(
                StateChangeData::new(
                    StateType::WebSocket,
                    "WS_CONNECTING",
                    format!("Connecting to {} (has_token={})", ws_config.host, ws_config.auth_token.is_some()),
                ),
            )));
        }

        // If connection fails, app continues in SSE-only mode
        match start_websocket_with_config(app.message_tx.clone(), ws_config).await {
            Ok(sender) => {
                if let Some(ref tx) = app.debug_tx {
                    let _ = tx.send(DebugEvent::new(DebugEventKind::StateChange(
                        StateChangeData::new(StateType::WebSocket, "WS_INIT", "WebSocket connected successfully"),
                    )));
                }
                app.ws_sender = Some(sender);
            }
            Err(e) => {
                if let Some(ref tx) = app.debug_tx {
                    let _ = tx.send(DebugEvent::new(DebugEventKind::StateChange(
                        StateChangeData::new(StateType::WebSocket, "WS_INIT_FAILED", format!("WebSocket connection failed: {}", e)),
                    )));
                }
                app.ws_sender = None;
            }
        }
    });

    // Main event loop
    let result = runtime.block_on(run_app(&mut terminal, &mut app));

    // Before exiting, save input history
    app.input_history.save();

    // Restore terminal
    restore_terminal(&mut terminal)?;

    // Cleanup debug server if it was started
    if let Some(handle) = debug_server_handle {
        handle.abort();
    }

    result
}

/// Start the debug system (channel + server).
///
/// Returns the debug event sender and server handle if successful.
/// If the debug server fails to start, returns None for both - the app continues without debug.
async fn start_debug_system() -> (
    Option<spoq::debug::DebugEventSender>,
    Option<JoinHandle<()>>,
) {
    // Create debug channel with capacity for 1000 events
    let (debug_tx, _) = create_debug_channel(1000);

    // Try to start the debug server
    match start_debug_server(debug_tx.clone()).await {
        Ok((handle, _)) => {
            // Server started successfully
            (Some(debug_tx), Some(handle))
        }
        Err(_e) => {
            // Server failed to start - continue without debug
            // (e.g., port 3030 already in use)
            (None, None)
        }
    }
}

/// Setup panic hook to restore terminal on panic
fn setup_panic_hook() {
    use std::io::Write;
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        // Try to restore terminal state
        // Pop keyboard enhancement flags BEFORE disabling raw mode
        let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);

        let _ = disable_raw_mode();
        let _ = execute!(
            io::stdout(),
            DisableMouseCapture,
            DisableBracketedPaste,
            LeaveAlternateScreen
        );
        let _ = execute!(io::stdout(), Show);

        // CRITICAL: Hard reset Kitty keyboard protocol AFTER leaving alternate screen
        // Ghostty (and potentially other terminals) need this sent after leaving alternate screen
        // CSI = 0 u sets all keyboard enhancement flags to zero (non-stack based reset)
        let _ = write!(io::stdout(), "\x1b[=0u");
        let _ = io::stdout().flush();

        // Call the original panic hook
        original_hook(panic_info);
    }));
}

/// Restore terminal to normal mode
fn restore_terminal<B: ratatui::backend::Backend + std::io::Write>(
    terminal: &mut Terminal<B>,
) -> Result<()>
where
    B::Error: Send + Sync + 'static,
{
    // Pop keyboard enhancement flags (crossterm's standard approach)
    let _ = execute!(terminal.backend_mut(), PopKeyboardEnhancementFlags);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        DisableMouseCapture,
        DisableBracketedPaste,
        LeaveAlternateScreen
    )?;

    // CRITICAL: Hard reset Kitty keyboard protocol AFTER leaving alternate screen
    // Some terminals (Ghostty) need this sent after leaving alternate screen
    // CSI = 0 u sets all keyboard enhancement flags to zero (non-stack based reset)
    let _ = write!(terminal.backend_mut(), "\x1b[=0u");
    let _ = io::Write::flush(terminal.backend_mut());

    terminal.show_cursor()?;
    Ok(())
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<()>
where
    B::Error: Send + Sync + 'static,
{
    // Track migration progress animation
    let migration_start = tokio::time::Instant::now();
    const MIGRATION_DURATION_MS: u64 = 5000; // 5 seconds

    // Create async event stream for keyboard input
    let mut event_stream = EventStream::new();

    // Take the message receiver from the app (we need ownership for select!)
    let mut message_rx: Option<mpsc::UnboundedReceiver<AppMessage>> = app.message_rx.take();

    loop {
        // Update migration progress if it's running
        if app.migration_progress.is_some() {
            let elapsed_ms = migration_start.elapsed().as_millis() as u64;
            if elapsed_ms >= MIGRATION_DURATION_MS {
                // Migration complete, hide progress bar
                app.migration_progress = None;
                app.mark_dirty();
            } else {
                // Calculate progress percentage (0-100)
                let progress = ((elapsed_ms * 100) / MIGRATION_DURATION_MS) as u8;
                if app.migration_progress != Some(progress) {
                    app.migration_progress = Some(progress);
                    app.mark_dirty();
                }
            }
        }

        // Draw the UI only when needed (dirty flag or streaming)
        if app.needs_redraw || app.is_streaming() {
            terminal.draw(|f| {
                ui::render(f, &mut *app);
            })?;
            app.needs_redraw = false;
        }

        // Poll both keyboard events and message channel using tokio::select!
        // 16ms tick for smooth 60fps-like scrolling animation
        let timeout = tokio::time::sleep(std::time::Duration::from_millis(16));

        tokio::select! {
            // Handle timeout for UI updates (migration progress, animations, etc.)
            _ = timeout => {
                // Increment tick counter for animations (spinner, cursor blink)
                app.tick();

                // Check for thread switcher auto-confirm (Tab release simulation)
                app.check_switcher_timeout();
            }

            // Handle keyboard events
            event_result = event_stream.next() => {
                if let Some(Ok(event)) = event_result {
                    match event {
                        Event::Resize(width, height) => {
                            // Update app state with new terminal dimensions
                            app.update_terminal_dimensions(width, height);
                            // Redraw will happen on next loop iteration
                            continue;
                        }
                        Event::Key(key) if key.kind == KeyEventKind::Press => {
                            // Any key press likely changes state (input, navigation, etc.)
                            app.mark_dirty();

                            // DEBUG: Log ALL key events
                            app.emit_debug_state_change(
                                "KeyEvent",
                                &format!(
                                    "code={:?} mods={:?}",
                                    key.code, key.modifiers
                                ),
                                "",
                            );

                            // Global keybinds (always active)
                            match key.code {
                                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                    app.quit();
                                    return Ok(());
                                }
                                // Shift+Escape to return to CommandDeck from Conversation
                                // (kept for terminals that support it)
                                KeyCode::Esc if key.modifiers.contains(KeyModifiers::SHIFT) => {
                                    if app.screen == Screen::Conversation {
                                        app.navigate_to_command_deck();
                                    }
                                    continue;
                                }
                                // Ctrl+W to return to CommandDeck (explicit close/back binding)
                                KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                    if app.screen == Screen::Conversation {
                                        app.navigate_to_command_deck();
                                    }
                                    continue;
                                }
                                // Shift+N to create new thread
                                KeyCode::Char('N') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                                    app.create_new_thread();
                                    continue;
                                }
                                // CapsLock is tricky - use Ctrl+N as alternative
                                KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                    app.create_new_thread();
                                    continue;
                                }
                                // Alt+P to submit as Programming thread (from CommandDeck)
                                KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::ALT) => {
                                    if app.screen == Screen::CommandDeck && !app.textarea.is_empty() {
                                        app.submit_input(models::ThreadType::Programming);
                                    }
                                    continue;
                                }
                                _ => {}
                            }

                            // Handle permission prompt keys when a permission is pending
                            // This takes priority over all other key handling
                            if app.session_state.has_pending_permission() {
                                // Check if this is an AskUserQuestion prompt
                                // State is already initialized when permission is received
                                if app.is_ask_user_question_pending() {

                                    // Handle "Other" text input mode
                                    if app.question_state.other_active {
                                        match key.code {
                                            KeyCode::Esc => {
                                                app.question_cancel_other();
                                                continue;
                                            }
                                            KeyCode::Enter => {
                                                if app.question_confirm() {
                                                    continue;
                                                }
                                                continue;
                                            }
                                            KeyCode::Backspace => {
                                                app.question_backspace();
                                                continue;
                                            }
                                            KeyCode::Char(c) => {
                                                app.question_type_char(c);
                                                continue;
                                            }
                                            _ => continue,
                                        }
                                    }

                                    // Handle question navigation keys
                                    match key.code {
                                        KeyCode::Tab => {
                                            app.question_next_tab();
                                            continue;
                                        }
                                        KeyCode::Up => {
                                            app.question_prev_option();
                                            continue;
                                        }
                                        KeyCode::Down => {
                                            app.question_next_option();
                                            continue;
                                        }
                                        KeyCode::Char(' ') => {
                                            app.question_toggle_option();
                                            continue;
                                        }
                                        KeyCode::Enter => {
                                            app.question_confirm();
                                            continue;
                                        }
                                        KeyCode::Char('n') | KeyCode::Char('N') => {
                                            // Allow 'n' to deny/cancel
                                            if let Some(ref perm) = app.session_state.pending_permission.clone() {
                                                app.deny_permission(&perm.permission_id);
                                            }
                                            continue;
                                        }
                                        _ => continue,
                                    }
                                }

                                // Standard permission prompt (y/a/n)
                                if let KeyCode::Char(c) = key.code {
                                    // Debug: emit key press to debug system
                                    app.emit_debug_state_change(
                                        "permission_key",
                                        "Key pressed during permission",
                                        &format!("key: '{}', pending: true", c),
                                    );
                                    if app.handle_permission_key(c) {
                                        app.emit_debug_state_change(
                                            "permission_key",
                                            "Permission handled",
                                            &format!("key: '{}' -> handled", c),
                                        );
                                        continue;
                                    }
                                    app.emit_debug_state_change(
                                        "permission_key",
                                        "Key not handled",
                                        &format!("key: '{}' -> not Y/N/A", c),
                                    );
                                }
                                // When permission is pending, ignore all other keys except Ctrl+C
                                continue;
                            }

                            // =========================================================
                            // Folder Picker Key Handling (HIGHEST PRIORITY when visible)
                            // Must come BEFORE thread switcher to capture typed characters
                            // =========================================================
                            if app.folder_picker_visible {
                                match key.code {
                                    KeyCode::Esc => {
                                        // Close picker, remove @ + filter from input
                                        app.remove_at_and_filter_from_input();
                                        app.close_folder_picker();
                                        continue;
                                    }
                                    KeyCode::Enter => {
                                        // Select folder, close picker, clear @ + filter
                                        // The @ and filter text should be removed since we're selecting
                                        app.remove_at_and_filter_from_input();
                                        app.folder_picker_select();
                                        continue;
                                    }
                                    KeyCode::Backspace => {
                                        if app.folder_picker_backspace() {
                                            // Filter was empty, close picker and remove @
                                            app.textarea.backspace(); // Remove the @
                                            app.close_folder_picker();
                                        }
                                        continue;
                                    }
                                    KeyCode::Up => {
                                        app.folder_picker_cursor_up();
                                        continue;
                                    }
                                    KeyCode::Down => {
                                        app.folder_picker_cursor_down();
                                        continue;
                                    }
                                    KeyCode::Char(c) if !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER) => {
                                        // Append character to filter
                                        app.folder_picker_type_char(c);
                                        continue;
                                    }
                                    _ => {
                                        // Other keys are ignored while picker is open
                                        continue;
                                    }
                                }
                            }

                            // Thread switcher handling (takes priority when visible)
                            if app.thread_switcher.visible {
                                match key.code {
                                    KeyCode::Tab | KeyCode::Down => {
                                        app.cycle_switcher_forward();
                                        continue;
                                    }
                                    KeyCode::Up => {
                                        app.cycle_switcher_backward();
                                        continue;
                                    }
                                    KeyCode::Esc => {
                                        app.close_switcher();
                                        continue;
                                    }
                                    KeyCode::Enter => {
                                        app.confirm_switcher_selection();
                                        continue;
                                    }
                                    _ => {
                                        // Any other key closes and confirms
                                        app.confirm_switcher_selection();
                                        continue;
                                    }
                                }
                            }

                            // Handle OAuth consent 'o' key to open URL in browser
                            if let KeyCode::Char('o') = key.code {
                                if let Some(url) = &app.session_state.oauth_url {
                                    // Open URL in browser using the 'open' crate
                                    if let Err(_e) = open::that(url) {
                                        // Silently ignore errors - user can manually copy URL from UI
                                    }
                                    // Don't clear the URL yet - leave it until OAuth is completed
                                    continue;
                                }
                            }

                            // Auto-focus to Input when user starts typing
                            // (printable characters only, not Ctrl combinations)
                            if let KeyCode::Char(_) = key.code {
                                if !key.modifiers.contains(KeyModifiers::CONTROL) && app.focus != Focus::Input {
                                    app.focus = Focus::Input;
                                    // Character will be processed by input handling below
                                }
                            }

                            // Handle input-specific keys when Input is focused
                            if app.focus == Focus::Input {
                                // Check for Shift+Escape FIRST (before plain Escape)
                                // This ensures Shift+Escape goes back to CommandDeck even when typing
                                if key.code == KeyCode::Esc && key.modifiers.contains(KeyModifiers::SHIFT) {
                                    if app.screen == Screen::Conversation {
                                        app.navigate_to_command_deck();
                                    }
                                    continue;
                                }

                                // Shift+Tab cycles permission mode (works while typing, all threads)
                                if key.code == KeyCode::BackTab {
                                    if app.screen == Screen::Conversation || app.screen == Screen::CommandDeck {
                                        app.cycle_permission_mode();
                                    }
                                    continue;
                                }

                                // macOS-style text navigation shortcuts (modifier + key)
                                // Check these BEFORE plain key handlers
                                match key.code {
                                    // Alt+Backspace: Delete word backward
                                    KeyCode::Backspace if key.modifiers.contains(KeyModifiers::ALT) => {
                                        app.textarea.delete_word_backward();
                                        continue;
                                    }
                                    // Super+Backspace (Cmd+Backspace): Delete to line start
                                    // Note: Most terminals intercept this, so Ctrl+U is the reliable alternative
                                    KeyCode::Backspace if key.modifiers.contains(KeyModifiers::SUPER) => {
                                        app.textarea.delete_to_line_start();
                                        continue;
                                    }
                                    // Alt+Left: Move cursor word left
                                    KeyCode::Left if key.modifiers.contains(KeyModifiers::ALT) => {
                                        app.textarea.move_cursor_word_left();
                                        continue;
                                    }
                                    // Super+Left (Cmd+Left): Move cursor to line start
                                    KeyCode::Left if key.modifiers.contains(KeyModifiers::SUPER) => {
                                        app.textarea.move_cursor_home();
                                        continue;
                                    }
                                    // Alt+Right: Move cursor word right
                                    KeyCode::Right if key.modifiers.contains(KeyModifiers::ALT) => {
                                        app.textarea.move_cursor_word_right();
                                        continue;
                                    }
                                    // Super+Right (Cmd+Right): Move cursor to line end
                                    KeyCode::Right if key.modifiers.contains(KeyModifiers::SUPER) => {
                                        app.textarea.move_cursor_end();
                                        continue;
                                    }
                                    _ => {}
                                }

                                // Plain key handlers (without modifiers)
                                match key.code {
                                    // Ctrl+U = Unix "kill line" - delete to line start
                                    // Works in ALL terminals (unlike Cmd+Backspace which terminals intercept)
                                    KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                        app.textarea.delete_to_line_start();
                                        continue;
                                    }
                                    // Ctrl+J = ASCII LF (newline) - works in ALL terminals
                                    // MUST come before plain Char(c) handler
                                    KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                        app.textarea.insert_newline();
                                        continue;
                                    }
                                    // Plain characters (no modifiers or only SHIFT)
                                    KeyCode::Char(c) if !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER) => {
                                        // Reset scroll to show input when typing (unified scroll)
                                        if app.screen == Screen::Conversation {
                                            app.user_has_scrolled = false;
                                            app.unified_scroll = 0;
                                        }
                                        // Check for @ trigger for folder picker (only on CommandDeck)
                                        if c == '@' && app.screen == Screen::CommandDeck {
                                            // Get current line content and cursor position
                                            let (row, col) = app.textarea.cursor();
                                            let lines = app.textarea.lines();
                                            let line_content = lines.get(row).map(|s| s.as_str()).unwrap_or("");

                                            if app.is_folder_picker_trigger(line_content, col) {
                                                // Insert the @ character first
                                                app.textarea.insert_char('@');
                                                // Then open the folder picker
                                                app.open_folder_picker();
                                                continue;
                                            }
                                        }
                                        // Normal character insertion
                                        app.textarea.insert_char(c);
                                        continue;
                                    }
                                    KeyCode::Backspace => {
                                        // Check if we should clear the folder chip instead of backspace
                                        if app.should_clear_folder_on_backspace() {
                                            app.clear_folder();
                                        } else {
                                            app.textarea.backspace();
                                        }
                                        continue;
                                    }
                                    KeyCode::Delete => {
                                        app.textarea.delete_char();
                                        continue;
                                    }
                                    KeyCode::Left => {
                                        app.textarea.move_cursor_left();
                                        continue;
                                    }
                                    KeyCode::Right => {
                                        app.textarea.move_cursor_right();
                                        continue;
                                    }
                                    KeyCode::Up => {
                                        // If cursor is on first line, try to navigate history up
                                        if app.textarea.is_cursor_on_first_line() {
                                            let current_content = app.textarea.content();
                                            if let Some(history_entry) = app.input_history.navigate_up(&current_content) {
                                                let entry = history_entry.to_string();
                                                app.textarea.set_content(&entry);
                                            }
                                        } else {
                                            // Normal cursor movement
                                            app.textarea.move_cursor_up();
                                        }
                                        continue;
                                    }
                                    KeyCode::Down => {
                                        // If cursor is on last line and navigating history, go forward
                                        if app.textarea.is_cursor_on_last_line() {
                                            // Only handle history navigation if we're currently navigating
                                            if app.input_history.is_navigating() {
                                                if let Some(history_entry) = app.input_history.navigate_down() {
                                                    let entry = history_entry.to_string();
                                                    app.textarea.set_content(&entry);
                                                } else {
                                                    // At bottom of history, restore original input
                                                    let original = app.input_history.get_current_input().to_string();
                                                    app.textarea.set_content(&original);
                                                }
                                            }
                                            // If not navigating, Down on last line does nothing
                                        } else {
                                            // Normal cursor movement in multi-line input
                                            app.textarea.move_cursor_down();
                                        }
                                        continue;
                                    }
                                    KeyCode::Home => {
                                        app.textarea.move_cursor_home();
                                        continue;
                                    }
                                    KeyCode::End => {
                                        app.textarea.move_cursor_end();
                                        continue;
                                    }
                                    KeyCode::Enter if key.modifiers.contains(KeyModifiers::SHIFT) => {
                                        // Shift+Enter inserts a newline (works in Kitty protocol terminals)
                                        app.textarea.insert_newline();
                                        continue;
                                    }
                                    KeyCode::Enter if key.modifiers.contains(KeyModifiers::ALT) => {
                                        // Alt+Enter inserts a newline
                                        app.textarea.insert_newline();
                                        continue;
                                    }
                                    KeyCode::Enter if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                        // Ctrl+Enter inserts a newline (fallback - may not work in all terminals)
                                        app.textarea.insert_newline();
                                        continue;
                                    }
                                    KeyCode::Enter => {
                                        // Plain Enter = Conversation thread
                                        app.submit_input(models::ThreadType::Conversation);
                                        continue;
                                    }
                                    KeyCode::Esc => {
                                        // Plain Escape (no Shift) - depends on input state and screen
                                        if app.screen == Screen::Conversation {
                                            if app.textarea.is_empty() {
                                                // Empty input: go back to CommandDeck
                                                app.navigate_to_command_deck();
                                            } else {
                                                // Has content: just unfocus to allow navigation
                                                app.focus = Focus::Threads;
                                            }
                                        } else {
                                            // On CommandDeck: unfocus input
                                            app.focus = Focus::Threads;
                                        }
                                        continue;
                                    }
                                    _ => {}
                                }
                            }

                            // Panel navigation (when not typing in input)
                            match key.code {
                                KeyCode::Tab => {
                                    // Double-tap Tab opens thread switcher
                                    app.handle_tab_press();
                                }
                                KeyCode::BackTab => {
                                    // Shift+Tab in Conversation/CommandDeck screens: cycle permission mode (all threads)
                                    if app.screen == Screen::Conversation || app.screen == Screen::CommandDeck {
                                        app.cycle_permission_mode();
                                    }
                                }
                                KeyCode::Esc if app.focus != Focus::Input => {
                                    // Escape when not in input: go back to CommandDeck
                                    if app.screen == Screen::Conversation {
                                        app.navigate_to_command_deck();
                                    }
                                }
                                KeyCode::Enter if app.focus == Focus::Threads => {
                                    // Open selected thread when pressing Enter on Threads panel
                                    app.open_selected_thread();
                                }
                                // Page scroll keys for conversation (unified scroll)
                                KeyCode::PageUp if app.screen == Screen::Conversation => {
                                    // Page up = scroll up to see older content
                                    app.scroll_velocity = 0.0; // Reset momentum on user scroll
                                    app.user_has_scrolled = true;
                                    let new_scroll = (app.unified_scroll + 10).min(app.max_scroll);
                                    let needs_redraw = if new_scroll != app.unified_scroll {
                                        app.unified_scroll = new_scroll;
                                        app.scroll_position = app.unified_scroll as f32;
                                        true
                                    } else if app.max_scroll > 0 {
                                        app.scroll_boundary_hit = Some(ScrollBoundary::Top);
                                        app.boundary_hit_tick = app.tick_count;
                                        true
                                    } else {
                                        false
                                    };
                                    if needs_redraw {
                                        app.mark_dirty();
                                    }
                                }
                                KeyCode::PageDown if app.screen == Screen::Conversation => {
                                    // Page down = scroll down to see newer content / input
                                    app.scroll_velocity = 0.0; // Reset momentum on user scroll
                                    let new_scroll = app.unified_scroll.saturating_sub(10);
                                    let needs_redraw = if new_scroll != app.unified_scroll {
                                        app.unified_scroll = new_scroll;
                                        app.scroll_position = app.unified_scroll as f32;
                                        if app.unified_scroll == 0 {
                                            app.user_has_scrolled = false; // Back at bottom
                                        }
                                        true
                                    } else {
                                        app.scroll_boundary_hit = Some(ScrollBoundary::Bottom);
                                        app.boundary_hit_tick = app.tick_count;
                                        true
                                    };
                                    if needs_redraw {
                                        app.mark_dirty();
                                    }
                                }
                                KeyCode::Up => {
                                    app.move_up();
                                }
                                KeyCode::Down => {
                                    let max_threads = app.cache.threads().len();
                                    app.move_down(max_threads);
                                }
                                KeyCode::Char('q') if app.focus != Focus::Input => {
                                    app.quit();
                                    return Ok(());
                                }
                                // 'd' to dismiss focused error in Conversation screen
                                KeyCode::Char('d') if app.focus != Focus::Input && app.screen == Screen::Conversation => {
                                    if app.has_errors() {
                                        app.dismiss_focused_error();
                                    }
                                }
                                // 't' to toggle thinking/reasoning block in Conversation screen
                                KeyCode::Char('t') if app.focus != Focus::Input && app.screen == Screen::Conversation => {
                                    app.toggle_reasoning();
                                }
                                // Note: Custom mouse selection removed - native terminal selection now handles copy
                                _ => {}
                            }
                        }
                        Event::Mouse(mouse_event) => {
                            // Handle mouse events for scroll, click, and hover
                            match mouse_event.kind {
                                // Left click: check hit areas for interactive elements
                                MouseEventKind::Down(MouseButton::Left) => {
                                    if let Some(action) = app.hit_registry.hit_test(
                                        mouse_event.column,
                                        mouse_event.row,
                                    ) {
                                        ui::handle_click_action(app, action);
                                        app.mark_dirty();
                                    }
                                    // If no hit area was clicked, let terminal handle text selection
                                }
                                // Mouse move: update hover state for visual feedback
                                MouseEventKind::Moved => {
                                    if app.hit_registry.update_hover(
                                        mouse_event.column,
                                        mouse_event.row,
                                    ) {
                                        app.mark_dirty();
                                    }
                                }
                                // Simple line-based scrolling (like native terminal apps)
                                // Each scroll event moves 3 lines (unified scroll)
                                MouseEventKind::ScrollDown => {
                                    if app.screen == Screen::Conversation {
                                        // Scroll down = see newer content / input
                                        app.scroll_velocity = 0.0; // Reset momentum on user scroll
                                        let needs_redraw = if app.unified_scroll >= 3 {
                                            app.unified_scroll -= 3;
                                            app.scroll_position = app.unified_scroll as f32;
                                            true
                                        } else if app.unified_scroll > 0 {
                                            app.unified_scroll = 0;
                                            app.user_has_scrolled = false; // Back at bottom
                                            app.scroll_position = 0.0;
                                            true
                                        } else {
                                            app.scroll_boundary_hit = Some(ScrollBoundary::Bottom);
                                            app.boundary_hit_tick = app.tick_count;
                                            true
                                        };
                                        if needs_redraw {
                                            app.mark_dirty();
                                        }
                                    }
                                }
                                MouseEventKind::ScrollUp => {
                                    if app.screen == Screen::Conversation {
                                        // Scroll up = see older content
                                        app.scroll_velocity = 0.0; // Reset momentum on user scroll
                                        app.user_has_scrolled = true;
                                        let new_scroll = (app.unified_scroll + 3).min(app.max_scroll);
                                        let needs_redraw = if new_scroll != app.unified_scroll {
                                            app.unified_scroll = new_scroll;
                                            app.scroll_position = app.unified_scroll as f32;
                                            true
                                        } else if app.max_scroll > 0 {
                                            app.scroll_boundary_hit = Some(ScrollBoundary::Top);
                                            app.boundary_hit_tick = app.tick_count;
                                            true
                                        } else {
                                            false
                                        };
                                        if needs_redraw {
                                            app.mark_dirty();
                                        }
                                    }
                                }
                                // Ignore other mouse events (right click, drag, etc.)
                                // Terminal handles text selection natively
                                _ => {}
                            }
                            continue;
                        }
                        Event::Paste(text) => {
                            // Handle paste events from bracketed paste mode
                            // Auto-focus to input if not already focused
                            if app.focus != Focus::Input {
                                app.focus = Focus::Input;
                            }

                            if app.should_summarize_paste(&text) {
                                // Insert as atomic token
                                app.textarea.insert_paste_token(text);
                            } else {
                                // Insert normally character by character
                                for ch in text.chars() {
                                    app.textarea.insert_char(ch);
                                }
                            }

                            app.mark_dirty();
                            continue;
                        }
                        _ => {
                            // Ignore other events (focus, etc.)
                        }
                    }
                }
            }

            // Handle async messages from streaming/connection
            msg = async {
                match &mut message_rx {
                    Some(rx) => rx.recv().await,
                    None => std::future::pending().await,
                }
            } => {
                if let Some(msg) = msg {
                    app.handle_message(msg);
                }
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}
