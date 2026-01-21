//! Keybind hints rendering for input area.
//!
//! Provides responsive keybind hints that adapt to terminal dimensions.

use ratatui::{
    style::Style,
    text::{Line, Span},
};

use crate::app::{App, Screen};

use super::super::layout::LayoutContext;
use super::super::theme::{COLOR_ACCENT, COLOR_DIM};

// ============================================================================
// Keybind Hints
// ============================================================================

/// Build contextual keybind hints based on application state.
///
/// This is the legacy function for backwards compatibility. For responsive keybinds,
/// use `build_responsive_keybinds` instead.
pub fn build_contextual_keybinds(app: &App) -> Line<'static> {
    build_responsive_keybinds(app, &LayoutContext::default())
}

/// Build responsive keybind hints based on application state and terminal dimensions.
///
/// On narrow terminals (< 80 columns), keybind hints are abbreviated:
/// - "[Shift+Tab]" becomes "[S+Tab]"
/// - "[Alt+Enter]" becomes "[A+Ent]"
/// - "[Tab Tab]" becomes "[Tab]"
/// - "cycle mode" becomes "mode"
/// - "switch thread" becomes "switch"
/// - "dismiss error" becomes "dismiss"
///
/// On extra small terminals (< 60 columns), only essential keybinds are shown.
pub fn build_responsive_keybinds(app: &App, ctx: &LayoutContext) -> Line<'static> {
    let mut spans = vec![Span::raw(" ")];

    // Check for visible elements that need special keybinds
    let has_error = app.stream_error.is_some();
    let has_links = app.has_visible_links;
    let is_narrow = ctx.is_narrow();
    let is_extra_small = ctx.is_extra_small();

    // Always show basic navigation
    if app.screen == Screen::Conversation {
        // Show mode cycling hint on all threads (skip on extra small)
        if !is_extra_small {
            if is_narrow {
                spans.push(Span::styled("[S+Tab]", Style::default().fg(COLOR_ACCENT)));
                spans.push(Span::raw(" mode | "));
            } else {
                spans.push(Span::styled(
                    "[Shift+Tab]",
                    Style::default().fg(COLOR_ACCENT),
                ));
                spans.push(Span::raw(" cycle mode | "));
            }
        }

        if has_error && !is_extra_small {
            // Error visible: show dismiss hint (skip on extra small)
            spans.push(Span::styled("d", Style::default().fg(COLOR_ACCENT)));
            if is_narrow {
                spans.push(Span::raw(": dismiss | "));
            } else {
                spans.push(Span::raw(": dismiss error | "));
            }
        }

        // Newline hint (skip on extra small)
        if !is_extra_small {
            if is_narrow {
                spans.push(Span::styled("[A+Ent]", Style::default().fg(COLOR_ACCENT)));
                spans.push(Span::raw(" newline | "));
            } else {
                spans.push(Span::styled(
                    "[Alt+Enter]",
                    Style::default().fg(COLOR_ACCENT),
                ));
                spans.push(Span::raw(" newline | "));
            }
        }

        spans.push(Span::styled("[Enter]", Style::default().fg(COLOR_ACCENT)));
        spans.push(Span::raw(" send | "));

        spans.push(Span::styled("[Esc]", Style::default().fg(COLOR_ACCENT)));
        spans.push(Span::raw(" back"));

        // Link hint (when links are visible) - dimmed to not distract
        if has_links && !is_extra_small {
            spans.push(Span::raw(" | "));
            if is_narrow {
                spans.push(Span::styled("[Cmd] links", Style::default().fg(COLOR_DIM)));
            } else {
                spans.push(Span::styled(
                    "[Cmd+click] open links",
                    Style::default().fg(COLOR_DIM),
                ));
            }
        }
    } else {
        // CommandDeck screen
        // Show mode cycling hint (skip on extra small)
        if !is_extra_small {
            if is_narrow {
                spans.push(Span::styled("[S+Tab]", Style::default().fg(COLOR_ACCENT)));
                spans.push(Span::raw(" mode | "));
            } else {
                spans.push(Span::styled(
                    "[Shift+Tab]",
                    Style::default().fg(COLOR_ACCENT),
                ));
                spans.push(Span::raw(" cycle mode | "));
            }
        }

        if !is_extra_small {
            if is_narrow {
                spans.push(Span::styled("[Tab]", Style::default().fg(COLOR_ACCENT)));
                spans.push(Span::raw(" switch | "));
            } else {
                spans.push(Span::styled("[Tab Tab]", Style::default().fg(COLOR_ACCENT)));
                spans.push(Span::raw(" switch thread | "));
            }
        }

        // Newline hint (skip on extra small)
        if !is_extra_small {
            if is_narrow {
                spans.push(Span::styled("[A+Ent]", Style::default().fg(COLOR_ACCENT)));
                spans.push(Span::raw(" newline | "));
            } else {
                spans.push(Span::styled(
                    "[Alt+Enter]",
                    Style::default().fg(COLOR_ACCENT),
                ));
                spans.push(Span::raw(" newline | "));
            }
        }

        spans.push(Span::styled("[Enter]", Style::default().fg(COLOR_ACCENT)));
        spans.push(Span::raw(" send | "));

        spans.push(Span::styled("[Esc]", Style::default().fg(COLOR_ACCENT)));
        spans.push(Span::raw(" back"));
    }

    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_app() -> App {
        App::default()
    }

    // ========================================================================
    // Responsive Keybinds Tests
    // ========================================================================

    #[test]
    fn test_responsive_keybinds_normal_width() {
        let app = create_test_app();
        let ctx = LayoutContext::new(120, 40);

        let keybinds = build_responsive_keybinds(&app, &ctx);
        let content: String = keybinds
            .spans
            .iter()
            .map(|s| s.content.to_string())
            .collect();

        // Should show full keybinds on normal width
        assert!(content.contains("[Tab Tab]"), "Should show full Tab Tab");
        assert!(
            content.contains("switch thread"),
            "Should show full 'switch thread'"
        );
    }

    #[test]
    fn test_responsive_keybinds_narrow_width() {
        let app = create_test_app();
        let ctx = LayoutContext::new(70, 24); // Narrow (< 80)

        let keybinds = build_responsive_keybinds(&app, &ctx);
        let content: String = keybinds
            .spans
            .iter()
            .map(|s| s.content.to_string())
            .collect();

        // Should show abbreviated keybinds on narrow width
        assert!(content.contains("[Tab]"), "Should show abbreviated Tab");
        assert!(
            content.contains("switch"),
            "Should show abbreviated 'switch'"
        );
        assert!(
            !content.contains("switch thread"),
            "Should NOT show full 'switch thread'"
        );
    }

    #[test]
    fn test_responsive_keybinds_extra_small_width() {
        let app = create_test_app();
        let ctx = LayoutContext::new(50, 24); // Extra small (< 60)

        let keybinds = build_responsive_keybinds(&app, &ctx);
        let content: String = keybinds
            .spans
            .iter()
            .map(|s| s.content.to_string())
            .collect();

        // On extra small, Tab switch hint should be hidden
        assert!(
            !content.contains("[Tab Tab]"),
            "Should NOT show Tab Tab on extra small"
        );
        assert!(
            !content.contains("switch"),
            "Should NOT show switch on extra small"
        );
        // But essential keybinds should remain
        assert!(content.contains("[Enter]"), "Should show Enter");
        assert!(content.contains("[Esc]"), "Should show Esc");
    }

    #[test]
    fn test_responsive_keybinds_conversation_programming_thread_narrow() {
        let mut app = create_test_app();
        app.screen = Screen::Conversation;
        app.terminal_width = 70;

        // Create a programming thread
        app.cache.upsert_thread(crate::models::Thread {
            id: "prog-thread".to_string(),
            title: "Programming".to_string(),
            description: None,
            preview: "Code".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: crate::models::ThreadType::Programming,
            mode: crate::models::ThreadMode::default(),
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: chrono::Utc::now(),
            working_directory: None,
            status: None,
            verified: None,
            verified_at: None,
        });
        app.active_thread_id = Some("prog-thread".to_string());

        let ctx = LayoutContext::new(70, 24);
        let keybinds = build_responsive_keybinds(&app, &ctx);
        let content: String = keybinds
            .spans
            .iter()
            .map(|s| s.content.to_string())
            .collect();

        // Should show abbreviated Shift+Tab on narrow
        assert!(content.contains("[S+Tab]"), "Should show abbreviated S+Tab");
        assert!(content.contains("mode"), "Should show abbreviated 'mode'");
        assert!(
            !content.contains("cycle mode"),
            "Should NOT show full 'cycle mode'"
        );
    }

    #[test]
    fn test_responsive_keybinds_with_error_narrow() {
        let mut app = create_test_app();
        app.screen = Screen::Conversation;
        app.stream_error = Some("Test error".to_string());

        let ctx = LayoutContext::new(70, 24);
        let keybinds = build_responsive_keybinds(&app, &ctx);
        let content: String = keybinds
            .spans
            .iter()
            .map(|s| s.content.to_string())
            .collect();

        // Should show abbreviated dismiss hint
        assert!(content.contains("dismiss"), "Should show 'dismiss'");
        assert!(
            !content.contains("dismiss error"),
            "Should NOT show full 'dismiss error'"
        );
    }

    // ========================================================================
    // Legacy Compatibility Tests
    // ========================================================================

    #[test]
    fn test_build_contextual_keybinds_uses_default_context() {
        let app = create_test_app();

        // build_contextual_keybinds should produce same result as build_responsive_keybinds
        // with default context (80x24)
        let legacy = build_contextual_keybinds(&app);
        let responsive = build_responsive_keybinds(&app, &LayoutContext::default());

        let legacy_content: String = legacy.spans.iter().map(|s| s.content.to_string()).collect();
        let responsive_content: String = responsive
            .spans
            .iter()
            .map(|s| s.content.to_string())
            .collect();

        assert_eq!(legacy_content, responsive_content);
    }

    // ========================================================================
    // Link Hint Tests
    // ========================================================================

    #[test]
    fn test_link_hint_appears_when_links_visible() {
        let mut app = create_test_app();
        app.screen = Screen::Conversation;
        app.has_visible_links = true; // Links are visible

        let ctx = LayoutContext::new(120, 40); // Normal width
        let keybinds = build_responsive_keybinds(&app, &ctx);
        let content: String = keybinds
            .spans
            .iter()
            .map(|s| s.content.to_string())
            .collect();

        // Should show full link hint on normal width
        assert!(content.contains("[Cmd+click]"), "Should show [Cmd+click]");
        assert!(content.contains("open links"), "Should show 'open links'");
    }

    #[test]
    fn test_link_hint_hidden_when_no_links() {
        let mut app = create_test_app();
        app.screen = Screen::Conversation;
        app.has_visible_links = false; // No links visible

        let ctx = LayoutContext::new(120, 40);
        let keybinds = build_responsive_keybinds(&app, &ctx);
        let content: String = keybinds
            .spans
            .iter()
            .map(|s| s.content.to_string())
            .collect();

        // Should NOT show link hint when no links present
        assert!(
            !content.contains("[Cmd+click]"),
            "Should NOT show [Cmd+click]"
        );
        assert!(
            !content.contains("open links"),
            "Should NOT show 'open links'"
        );
        assert!(!content.contains("[Cmd]"), "Should NOT show [Cmd]");
        assert!(!content.contains("links"), "Should NOT show 'links'");
    }

    #[test]
    fn test_link_hint_abbreviated_on_narrow() {
        let mut app = create_test_app();
        app.screen = Screen::Conversation;
        app.has_visible_links = true;

        let ctx = LayoutContext::new(70, 24); // Narrow (< 80)
        let keybinds = build_responsive_keybinds(&app, &ctx);
        let content: String = keybinds
            .spans
            .iter()
            .map(|s| s.content.to_string())
            .collect();

        // Should show abbreviated link hint on narrow width
        assert!(content.contains("[Cmd]"), "Should show abbreviated [Cmd]");
        assert!(content.contains("links"), "Should show abbreviated 'links'");
        assert!(
            !content.contains("[Cmd+click]"),
            "Should NOT show full [Cmd+click]"
        );
        assert!(
            !content.contains("open links"),
            "Should NOT show full 'open links'"
        );
    }

    #[test]
    fn test_link_hint_hidden_on_extra_small() {
        let mut app = create_test_app();
        app.screen = Screen::Conversation;
        app.has_visible_links = true;

        let ctx = LayoutContext::new(50, 24); // Extra small (< 60)
        let keybinds = build_responsive_keybinds(&app, &ctx);
        let content: String = keybinds
            .spans
            .iter()
            .map(|s| s.content.to_string())
            .collect();

        // Link hint should be hidden on extra small terminals
        assert!(
            !content.contains("[Cmd+click]"),
            "Should NOT show [Cmd+click]"
        );
        assert!(!content.contains("[Cmd]"), "Should NOT show [Cmd]");
        assert!(
            !content.contains("open links"),
            "Should NOT show 'open links'"
        );
    }

    #[test]
    fn test_link_hint_only_on_conversation_screen() {
        let mut app = create_test_app();
        app.screen = Screen::CommandDeck; // Not on conversation screen
        app.has_visible_links = true;

        let ctx = LayoutContext::new(120, 40);
        let keybinds = build_responsive_keybinds(&app, &ctx);
        let content: String = keybinds
            .spans
            .iter()
            .map(|s| s.content.to_string())
            .collect();

        // Link hint should only appear on conversation screen
        assert!(
            !content.contains("[Cmd+click]"),
            "Should NOT show link hint on CommandDeck"
        );
        assert!(
            !content.contains("open links"),
            "Should NOT show link hint on CommandDeck"
        );
    }

    #[test]
    fn test_link_hint_with_other_hints() {
        let mut app = create_test_app();
        app.screen = Screen::Conversation;
        app.has_visible_links = true;
        app.stream_error = Some("Test error".to_string());

        let ctx = LayoutContext::new(120, 40);
        let keybinds = build_responsive_keybinds(&app, &ctx);
        let content: String = keybinds
            .spans
            .iter()
            .map(|s| s.content.to_string())
            .collect();

        // All hints should coexist
        assert!(
            content.contains("dismiss error"),
            "Should show error dismiss hint"
        );
        assert!(content.contains("[Cmd+click]"), "Should show link hint");
        assert!(content.contains("[Enter]"), "Should show send hint");
        assert!(content.contains("[Esc]"), "Should show back hint");
    }

    #[test]
    fn test_link_hint_position_at_end() {
        let mut app = create_test_app();
        app.screen = Screen::Conversation;
        app.has_visible_links = true;

        let ctx = LayoutContext::new(120, 40);
        let keybinds = build_responsive_keybinds(&app, &ctx);
        let content: String = keybinds
            .spans
            .iter()
            .map(|s| s.content.to_string())
            .collect();

        // Link hint should appear after the "back" hint
        let back_pos = content.find("back").unwrap();
        let link_pos = content.find("[Cmd+click]").unwrap();
        assert!(
            link_pos > back_pos,
            "Link hint should appear after 'back' hint"
        );
    }
}
