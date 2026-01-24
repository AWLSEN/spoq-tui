//! Navigation command handlers.
//!
//! Handles commands related to focus navigation, screen transitions,
//! and scrolling within the application.

use crate::app::{App, Focus, Screen, ScrollBoundary};
use crate::input::Command;

/// Handles navigation-related commands.
///
/// Returns `true` if the command was handled successfully.
pub fn handle_navigation_command(app: &mut App, cmd: &Command) -> bool {
    match cmd {
        Command::NavigateToCommandDeck => {
            if app.screen == Screen::Conversation {
                app.navigate_to_command_deck();
                true
            } else {
                false
            }
        }

        Command::MoveUp => {
            app.move_up();
            true
        }

        Command::MoveDown => {
            let max_threads = app.cache.threads().len();
            app.move_down(max_threads);
            true
        }

        Command::OpenSelectedThread => {
            if app.focus == Focus::Threads {
                app.open_selected_thread();
                true
            } else {
                false
            }
        }

        Command::CycleFocus => {
            app.handle_tab_press();
            true
        }

        Command::HandleTabPress => {
            app.handle_tab_press();
            true
        }

        Command::CyclePermissionMode => {
            if app.screen == Screen::Conversation || app.screen == Screen::CommandDeck {
                app.cycle_permission_mode();
                true
            } else {
                false
            }
        }

        Command::ScrollPageUp => {
            if app.screen == Screen::Conversation {
                handle_scroll_page_up(app);
                true
            } else {
                false
            }
        }

        Command::ScrollPageDown => {
            if app.screen == Screen::Conversation {
                handle_scroll_page_down(app);
                true
            } else {
                false
            }
        }

        Command::ScrollUp(lines) => {
            if app.screen == Screen::Conversation {
                handle_scroll_up(app, *lines);
                true
            } else {
                false
            }
        }

        Command::ScrollDown(lines) => {
            if app.screen == Screen::Conversation {
                handle_scroll_down(app, *lines);
                true
            } else {
                false
            }
        }

        Command::UnfocusInput => {
            if app.focus == Focus::Input {
                app.focus = Focus::Threads;
                true
            } else {
                false
            }
        }

        _ => false,
    }
}

/// Handles page up scrolling in conversation view.
fn handle_scroll_page_up(app: &mut App) {
    app.scroll_velocity = 0.0;
    app.user_has_scrolled = true;
    let new_scroll = (app.unified_scroll + 10).min(app.max_scroll);
    if new_scroll != app.unified_scroll {
        app.unified_scroll = new_scroll;
        app.scroll_position = app.unified_scroll as f32;
        app.mark_dirty();
    } else if app.max_scroll > 0 {
        app.scroll_boundary_hit = Some(ScrollBoundary::Top);
        app.boundary_hit_tick = app.tick_count;
        app.mark_dirty();
    }
}

/// Handles page down scrolling in conversation view.
fn handle_scroll_page_down(app: &mut App) {
    app.scroll_velocity = 0.0;
    let new_scroll = app.unified_scroll.saturating_sub(10);
    if new_scroll != app.unified_scroll {
        app.unified_scroll = new_scroll;
        app.scroll_position = app.unified_scroll as f32;
        if app.unified_scroll == 0 {
            app.user_has_scrolled = false;
        }
        app.mark_dirty();
    } else {
        app.scroll_boundary_hit = Some(ScrollBoundary::Bottom);
        app.boundary_hit_tick = app.tick_count;
        app.mark_dirty();
    }
}

/// Handles mouse scroll up (see older content).
fn handle_scroll_up(app: &mut App, lines: usize) {
    app.scroll_velocity = 0.0;
    app.user_has_scrolled = true;
    let lines_u16 = lines as u16;
    let new_scroll = (app.unified_scroll + lines_u16).min(app.max_scroll);
    if new_scroll != app.unified_scroll {
        app.unified_scroll = new_scroll;
        app.scroll_position = app.unified_scroll as f32;
        app.mark_dirty();
    } else if app.max_scroll > 0 {
        app.scroll_boundary_hit = Some(ScrollBoundary::Top);
        app.boundary_hit_tick = app.tick_count;
        app.mark_dirty();
    }
}

/// Handles mouse scroll down (see newer content).
fn handle_scroll_down(app: &mut App, lines: usize) {
    app.scroll_velocity = 0.0;
    let lines_u16 = lines as u16;
    if app.unified_scroll >= lines_u16 {
        app.unified_scroll -= lines_u16;
        app.scroll_position = app.unified_scroll as f32;
        app.mark_dirty();
    } else if app.unified_scroll > 0 {
        app.unified_scroll = 0;
        app.user_has_scrolled = false;
        app.scroll_position = 0.0;
        app.mark_dirty();
    } else {
        app.scroll_boundary_hit = Some(ScrollBoundary::Bottom);
        app.boundary_hit_tick = app.tick_count;
        app.mark_dirty();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_app() -> App {
        App::default()
    }

    #[test]
    fn test_handle_move_up() {
        let mut app = create_test_app();

        let handled = handle_navigation_command(&mut app, &Command::MoveUp);
        assert!(handled);
    }

    #[test]
    fn test_handle_move_down() {
        let mut app = create_test_app();

        let handled = handle_navigation_command(&mut app, &Command::MoveDown);
        assert!(handled);
    }

    #[test]
    fn test_handle_navigate_to_command_deck_from_conversation() {
        let mut app = create_test_app();
        app.screen = Screen::Conversation;

        let handled = handle_navigation_command(&mut app, &Command::NavigateToCommandDeck);
        assert!(handled);
        assert_eq!(app.screen, Screen::CommandDeck);
    }

    #[test]
    fn test_handle_navigate_to_command_deck_from_command_deck() {
        let mut app = create_test_app();
        app.screen = Screen::CommandDeck;

        let handled = handle_navigation_command(&mut app, &Command::NavigateToCommandDeck);
        assert!(!handled);
    }

    #[test]
    fn test_handle_cycle_permission_mode_on_conversation() {
        let mut app = create_test_app();
        app.screen = Screen::Conversation;

        let handled = handle_navigation_command(&mut app, &Command::CyclePermissionMode);
        assert!(handled);
    }

    #[test]
    fn test_handle_unfocus_input() {
        let mut app = create_test_app();
        app.focus = Focus::Input;

        let handled = handle_navigation_command(&mut app, &Command::UnfocusInput);
        assert!(handled);
        assert_eq!(app.focus, Focus::Threads);
    }

    #[test]
    fn test_handle_scroll_page_up() {
        let mut app = create_test_app();
        app.screen = Screen::Conversation;
        app.max_scroll = 100;
        app.unified_scroll = 0;

        let handled = handle_navigation_command(&mut app, &Command::ScrollPageUp);
        assert!(handled);
        assert!(app.unified_scroll > 0);
        assert!(app.user_has_scrolled);
    }

    #[test]
    fn test_handle_scroll_page_down() {
        let mut app = create_test_app();
        app.screen = Screen::Conversation;
        app.max_scroll = 100;
        app.unified_scroll = 50;

        let handled = handle_navigation_command(&mut app, &Command::ScrollPageDown);
        assert!(handled);
        assert!(app.unified_scroll < 50);
    }
}
