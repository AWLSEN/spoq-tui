mod app;
mod state;
mod storage;
mod ui;
mod widgets;

use app::{App, Focus};
use color_eyre::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};
use std::io;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

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

    // Main event loop
    let result = run_app(&mut terminal, &mut app).await;

    // Save app state before exit
    if let Err(e) = app.save() {
        eprintln!("Failed to save app state: {}", e);
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;

    result
}

async fn run_app<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()> {
    // Mock data counts for navigation bounds
    const MOCK_NOTIFICATIONS_COUNT: usize = 4;
    const MOCK_THREADS_COUNT: usize = 3;

    loop {
        // Draw the UI
        terminal.draw(|f| {
            ui::render(f, app);
        })?;

        // Handle events
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    // Global keybinds (always active)
                    match key.code {
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            app.quit();
                            return Ok(());
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
                                // Escape from input to go back to threads
                                app.focus = Focus::Threads;
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
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}
