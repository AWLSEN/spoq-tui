mod app;
mod cache;
mod conductor;
mod events;
mod models;
pub mod sse;
mod state;
mod storage;
mod ui;
mod widgets;

use app::{App, AppMessage, Focus, Screen};
use color_eyre::Result;
use crossterm::{
    cursor::Show,
    event::{Event, EventStream, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::StreamExt;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    // Setup panic hook to ensure terminal cleanup on panic
    setup_panic_hook();

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Clear the terminal
    terminal.clear()?;

    // Initialize application state
    let mut app = App::new()?;

    // Load threads from backend (async initialization)
    app.initialize().await;

    // Main event loop
    let result = run_app(&mut terminal, &mut app).await;

    // Save app state before exit
    if let Err(e) = app.save() {
        eprintln!("Failed to save app state: {}", e);
    }

    // Restore terminal
    restore_terminal(&mut terminal)?;

    result
}

/// Setup panic hook to restore terminal on panic
fn setup_panic_hook() {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        // Try to restore terminal state
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
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
            // Handle timeout for UI updates (migration progress, etc.)
            _ = timeout => {
                // Just continue to redraw
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
                                _ => {}
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
                                        app.submit_input();
                                        continue;
                                    }
                                    KeyCode::Esc => {
                                        // Escape behavior depends on input state and screen
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
                                    // Shift+Tab to go backwards
                                    app.focus = match app.focus {
                                        Focus::Notifications => Focus::Input,
                                        Focus::Tasks => Focus::Notifications,
                                        Focus::Threads => Focus::Tasks,
                                        Focus::Input => Focus::Threads,
                                    };
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
                                _ => {}
                            }
                        }
                        _ => {
                            // Ignore other events (mouse, focus, etc.)
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
