use spoq::app::{App, AppMessage, Focus, Screen};
use spoq::debug::{create_debug_channel, start_debug_server};
use spoq::models;
use spoq::ui;

use color_eyre::Result;
use crossterm::{
    cursor::Show,
    event::{
        DisableMouseCapture, EnableMouseCapture, Event, EventStream, KeyCode, KeyEventKind,
        KeyModifiers, MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::StreamExt;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    // Setup panic hook to ensure terminal cleanup on panic
    setup_panic_hook();

    // Create debug channel and start debug server (optional - continues without if fails)
    let (debug_tx, debug_server_handle) = start_debug_system().await;

    // Open debug dashboard in browser (fire and forget)
    if debug_tx.is_some() {
        let _ = open::that("http://localhost:3030");
    }

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Clear the terminal
    terminal.clear()?;

    // Initialize application state with debug sender
    let mut app = App::with_debug(debug_tx)?;

    // Load threads from backend (async initialization)
    app.initialize().await;

    // Main event loop
    let result = run_app(&mut terminal, &mut app).await;

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
async fn start_debug_system() -> (Option<spoq::debug::DebugEventSender>, Option<JoinHandle<()>>) {
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
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        // Try to restore terminal state
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
        let _ = execute!(io::stdout(), Show);

        // Call the original panic hook
        original_hook(panic_info);
    }));
}

/// Restore terminal to normal mode
fn restore_terminal<B: ratatui::backend::Backend + std::io::Write>(terminal: &mut Terminal<B>) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        DisableMouseCapture,
        LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;
    Ok(())
}

async fn run_app<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()> {
    // Mock data counts for navigation bounds
    const MOCK_NOTIFICATIONS_COUNT: usize = 4;
    const MOCK_THREADS_COUNT: usize = 3;

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
            } else {
                // Calculate progress percentage (0-100)
                let progress = ((elapsed_ms * 100) / MIGRATION_DURATION_MS) as u8;
                app.migration_progress = Some(progress);
            }
        }

        // Draw the UI
        terminal.draw(|f| {
            ui::render(f, app);
        })?;

        // Poll both keyboard events and message channel using tokio::select!
        let timeout = tokio::time::sleep(std::time::Duration::from_millis(100));

        tokio::select! {
            // Handle timeout for UI updates (migration progress, animations, etc.)
            _ = timeout => {
                // Increment tick counter for animations (spinner, cursor blink)
                app.tick();
            }

            // Handle keyboard events
            event_result = event_stream.next() => {
                if let Some(Ok(event)) = event_result {
                    match event {
                        Event::Resize(_width, _height) => {
                            // Terminal was resized, redraw will happen on next loop iteration
                            continue;
                        }
                        Event::Key(key) if key.kind == KeyEventKind::Press => {
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
                                // Ctrl+P to submit as Programming thread (from CommandDeck)
                                KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                    if app.screen == Screen::CommandDeck && !app.input_box.is_empty() {
                                        app.submit_input(models::ThreadType::Programming);
                                    }
                                    continue;
                                }
                                _ => {}
                            }

                            // Handle permission prompt keys (y/a/n) when a permission is pending
                            // This takes priority over all other key handling
                            if app.session_state.has_pending_permission() {
                                if let KeyCode::Char(c) = key.code {
                                    if app.handle_permission_key(c) {
                                        continue;
                                    }
                                }
                                // When permission is pending, ignore all other keys except Ctrl+C
                                continue;
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

                                match key.code {
                                    KeyCode::Char(c) => {
                                        app.input_box.insert_char(c);
                                        continue;
                                    }
                                    KeyCode::Backspace => {
                                        app.input_box.backspace();
                                        continue;
                                    }
                                    KeyCode::Delete => {
                                        app.input_box.delete_char();
                                        continue;
                                    }
                                    KeyCode::Left => {
                                        app.input_box.move_cursor_left();
                                        continue;
                                    }
                                    KeyCode::Right => {
                                        app.input_box.move_cursor_right();
                                        continue;
                                    }
                                    KeyCode::Home => {
                                        app.input_box.move_cursor_home();
                                        continue;
                                    }
                                    KeyCode::End => {
                                        app.input_box.move_cursor_end();
                                        continue;
                                    }
                                    KeyCode::Enter => {
                                        // Plain Enter = Conversation thread (Shift+Enter handled above)
                                        app.submit_input(models::ThreadType::Conversation);
                                        continue;
                                    }
                                    KeyCode::Esc => {
                                        // Plain Escape (no Shift) - depends on input state and screen
                                        if app.screen == Screen::Conversation {
                                            if app.input_box.is_empty() {
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
                                    app.cycle_focus();
                                }
                                KeyCode::BackTab => {
                                    // Shift+Tab in Conversation screen with Programming thread: cycle mode
                                    if app.screen == Screen::Conversation && app.is_active_thread_programming() {
                                        app.cycle_programming_mode();
                                    } else {
                                        // Otherwise: cycle focus backwards
                                        app.focus = match app.focus {
                                            Focus::Notifications => Focus::Input,
                                            Focus::Tasks => Focus::Notifications,
                                            Focus::Threads => Focus::Tasks,
                                            Focus::Input => Focus::Threads,
                                        };
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
                                KeyCode::Up => {
                                    app.move_up();
                                }
                                KeyCode::Down => {
                                    let max_tasks = app.tasks.len().max(5); // Mock minimum of 5
                                    app.move_down(MOCK_NOTIFICATIONS_COUNT, max_tasks, MOCK_THREADS_COUNT.max(app.threads.len()));
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
                                _ => {}
                            }
                        }
                        Event::Mouse(mouse_event) => {
                            match mouse_event.kind {
                                // Natural scrolling: scroll down = see newer content, scroll up = see older content
                                MouseEventKind::ScrollDown => {
                                    if app.screen == Screen::Conversation {
                                        // Scroll down to see newer content (decrease scroll offset)
                                        // Minimum is 0 (showing latest at bottom)
                                        app.conversation_scroll =
                                            app.conversation_scroll.saturating_sub(1);
                                    }
                                }
                                MouseEventKind::ScrollUp => {
                                    if app.screen == Screen::Conversation {
                                        // Scroll up to see older content (increase scroll offset)
                                        app.conversation_scroll =
                                            app.conversation_scroll.saturating_add(1);
                                    }
                                }
                                _ => {}
                            }
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
