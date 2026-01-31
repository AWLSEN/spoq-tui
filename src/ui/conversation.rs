//! Conversation screen rendering
//!
//! Implements the conversation view with header, messages, and streaming indicator.
//! Uses the `LayoutContext` responsive layout system for all dimension calculations.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::App;
use crate::models::{MessageSegment, PermissionMode, ToolEventStatus};

use super::file_picker::render_file_picker;
use super::helpers::{inner_rect, truncate_string, SPINNER_FRAMES};
use super::layout::LayoutContext;
use super::messages::render_messages_area;
use super::slash_autocomplete::{render_slash_autocomplete_anchored, AnchorMode};
use super::theme::{COLOR_BORDER, COLOR_DIM, COLOR_HEADER};

// ============================================================================
// Mode Indicator
// ============================================================================

pub fn create_mode_indicator_line(mode: PermissionMode) -> Option<Line<'static>> {
    match mode {
        PermissionMode::Default => None,
        PermissionMode::Plan => Some(Line::from(vec![Span::styled(
            " [PLAN]",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        )])),
        PermissionMode::Execution => Some(Line::from(vec![Span::styled(
            " [EXECUTE]",
            Style::default()
                .fg(Color::Rgb(255, 140, 0))
                .add_modifier(Modifier::BOLD),
        )])),
    }
}

// ============================================================================
// Conversation Screen
// ============================================================================

/// Render the conversation screen with header, messages area, and input
///
/// Layout adapts to terminal dimensions using `LayoutContext`:
/// - Header height adjusts for compact terminals
/// - Input area height adapts to available space
/// - All sections use the full available width
/// - Mode indicator is rendered within the input section (build_input_section)
pub fn render_conversation_screen(frame: &mut Frame, app: &mut App) {
    let size = frame.area();

    // Create layout context for responsive calculations
    let ctx = LayoutContext::new(app.terminal_width, app.terminal_height);

    // Determine if we should show the streaming indicator
    let show_streaming_indicator = app.is_streaming();

    // Create main layout sections
    let inner = inner_rect(size, 0);

    // Calculate responsive layout heights
    // Input is now part of unified scroll in render_messages_area
    // Mode indicator is rendered within the input section (build_input_section)
    let header_height = if ctx.is_short() { 2 } else { 3 };

    if show_streaming_indicator {
        // Layout with streaming indicator (3 sections)
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(header_height), // Thread header (responsive)
                Constraint::Min(10),               // Unified content (messages + input)
                Constraint::Length(1),             // Streaming indicator
            ])
            .split(inner);

        render_conversation_header(frame, main_chunks[0], app, &ctx);
        render_messages_area(frame, main_chunks[1], app, &ctx);
        render_streaming_indicator(frame, main_chunks[2], app, &ctx);

        // Render slash autocomplete overlay (if visible) - must be last for proper layering
        if app.slash_autocomplete_visible {
            // Calculate where input section starts in the viewport
            // With unified scroll anchored to bottom, input is near the bottom
            let viewport_height = main_chunks[1].height as usize;
            let content_top = app.total_content_lines.saturating_sub(viewport_height + app.unified_scroll as usize);
            let input_y_in_viewport = app.input_section_start.saturating_sub(content_top);

            // Position dropdown ABOVE the input (grows upward from input top)
            let input_anchor_area = Rect {
                x: main_chunks[1].x + 2,
                y: main_chunks[1].y + input_y_in_viewport as u16,
                width: main_chunks[1].width.saturating_sub(4),
                height: 1,
            };
            render_slash_autocomplete_anchored(frame, app, input_anchor_area, AnchorMode::Above);
        }

        // Render file picker overlay (if visible)
        if app.file_picker.visible {
            let viewport_height = main_chunks[1].height as usize;
            let content_top = app.total_content_lines.saturating_sub(viewport_height + app.unified_scroll as usize);
            let input_y_in_viewport = app.input_section_start.saturating_sub(content_top);

            // Calculate input section height (from messages/mod.rs tracking)
            let input_section_height = app.total_content_lines.saturating_sub(app.input_section_start);

            // Position anchor at BOTTOM of input (for below-positioning)
            let input_bottom_y = (main_chunks[1].y as usize + input_y_in_viewport + input_section_height)
                .min(main_chunks[1].bottom() as usize);

            let input_anchor_area = Rect {
                x: main_chunks[1].x + 2,
                y: input_bottom_y as u16,
                width: main_chunks[1].width.saturating_sub(4),
                height: 1,
            };
            render_file_picker(frame, &app.file_picker, input_anchor_area);
        }
    } else {
        // Layout without streaming indicator (2 sections)
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(header_height), // Thread header (responsive)
                Constraint::Min(10),               // Unified content (messages + input)
            ])
            .split(inner);

        render_conversation_header(frame, main_chunks[0], app, &ctx);
        render_messages_area(frame, main_chunks[1], app, &ctx);

        // Render slash autocomplete overlay (if visible) - must be last for proper layering
        if app.slash_autocomplete_visible {
            // Calculate where input section starts in the viewport
            // With unified scroll anchored to bottom, input is near the bottom
            let viewport_height = main_chunks[1].height as usize;
            let content_top = app.total_content_lines.saturating_sub(viewport_height + app.unified_scroll as usize);
            let input_y_in_viewport = app.input_section_start.saturating_sub(content_top);

            // Position dropdown ABOVE the input (grows upward from input top)
            let input_anchor_area = Rect {
                x: main_chunks[1].x + 2,
                y: main_chunks[1].y + input_y_in_viewport as u16,
                width: main_chunks[1].width.saturating_sub(4),
                height: 1,
            };
            render_slash_autocomplete_anchored(frame, app, input_anchor_area, AnchorMode::Above);
        }

        // Render file picker overlay (if visible)
        if app.file_picker.visible {
            let viewport_height = main_chunks[1].height as usize;
            let content_top = app.total_content_lines.saturating_sub(viewport_height + app.unified_scroll as usize);
            let input_y_in_viewport = app.input_section_start.saturating_sub(content_top);

            // Calculate input section height (from messages/mod.rs tracking)
            let input_section_height = app.total_content_lines.saturating_sub(app.input_section_start);

            // Position anchor at BOTTOM of input (for below-positioning)
            let input_bottom_y = (main_chunks[1].y as usize + input_y_in_viewport + input_section_height)
                .min(main_chunks[1].bottom() as usize);

            let input_anchor_area = Rect {
                x: main_chunks[1].x + 2,
                y: input_bottom_y as u16,
                width: main_chunks[1].width.saturating_sub(4),
                height: 1,
            };
            render_file_picker(frame, &app.file_picker, input_anchor_area);
        }
    }

    // Render ClaudeAccounts overlay (screen-agnostic floating modal)
    if let Some(crate::view_state::dashboard_view::OverlayState::ClaudeAccounts {
        ref accounts,
        selected_index,
        adding,
        ref status_message,
        ..
    }) = app.dashboard.overlay()
    {
        let card_width = 50u16.min(size.width.saturating_sub(4));
        let card_height = super::dashboard::accounts_card::calculate_height(accounts.len(), status_message.is_some())
            .saturating_add(2) // borders
            .min(size.height.saturating_sub(4));

        // Center the card
        let x = size.x + (size.width.saturating_sub(card_width)) / 2;
        let y = size.y + (size.height.saturating_sub(card_height)) / 2;
        let card_area = Rect::new(x, y, card_width, card_height);

        // Clear background and draw border
        frame.render_widget(Clear, card_area);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Cyan))
            .style(Style::default().bg(Color::Black));
        let inner_area = block.inner(card_area);
        frame.render_widget(block, card_area);

        // Render card content
        super::dashboard::accounts_card::render(frame, inner_area, accounts, *selected_index, *adding, status_message.as_deref());
    }

    // Render rate limit modal (screen-agnostic floating modal)
    if let Some(ref modal_state) = app.rate_limit_modal {
        let card_width = 60u16.min(size.width.saturating_sub(4));
        let card_height = 10u16.min(size.height.saturating_sub(4));

        // Center the card
        let x = size.x + (size.width.saturating_sub(card_width)) / 2;
        let y = size.y + (size.height.saturating_sub(card_height)) / 2;
        let card_area = Rect::new(x, y, card_width, card_height);

        // Clear background and draw border
        frame.render_widget(Clear, card_area);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Yellow))
            .title(" Rate Limit ")
            .style(Style::default().bg(Color::Black));
        let inner_area = block.inner(card_area);
        frame.render_widget(block, card_area);

        // Render modal content
        use ratatui::text::{Line, Span};
        use ratatui::widgets::Paragraph;
        use ratatui::style::{Color, Style, Modifier};

        let mut lines = vec![];
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            " ⚠️  Account rate-limited",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));

        if modal_state.next_account_id.is_some() {
            lines.push(Line::from(vec![
                Span::raw("  Continue with "),
                Span::styled("next account", Style::default().fg(Color::Cyan)),
                Span::raw("?"),
            ]));
        } else {
            lines.push(Line::from(Span::styled(
                "  No more accounts available",
                Style::default().fg(Color::Red),
            )));
        }

        lines.push(Line::from(""));
        if modal_state.next_account_id.is_some() {
            lines.push(Line::from(vec![
                Span::styled("  [Y] ", Style::default().fg(Color::Green)),
                Span::raw("Continue  "),
                Span::styled("  [N] ", Style::default().fg(Color::Red)),
                Span::raw("Cancel"),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled("  [Esc] ", Style::default().fg(Color::Cyan)),
                Span::raw("Close"),
            ]));
        }

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner_area);
    }
}

/// Render the mode indicator bar
pub fn render_mode_indicator(frame: &mut Frame, area: Rect, mode_line: Line<'static>) {
    let indicator = Paragraph::new(mode_line);
    frame.render_widget(indicator, area);
}

/// Render the streaming indicator bar
///
/// Adapts to terminal width using `LayoutContext`:
/// - On narrow terminals, tool names are truncated
/// - Uses available width for status text
pub fn render_streaming_indicator(frame: &mut Frame, area: Rect, app: &App, ctx: &LayoutContext) {
    // Get messages from cache if we have an active thread
    let cached_messages = app
        .active_thread_id
        .as_ref()
        .and_then(|id| app.cache.get_messages(id));

    // Check if any message is currently streaming
    let streaming_message = cached_messages
        .as_ref()
        .and_then(|msgs| msgs.iter().find(|m| m.is_streaming));

    if let Some(streaming_msg) = streaming_message {
        // Check if cancel is in progress
        if app.cancel_in_progress {
            // Show "Cancelling..." state
            let indicator_line = Line::from(vec![Span::styled(
                "  ⏹ Cancelling...".to_string(),
                Style::default().fg(Color::Yellow),
            )]);
            let indicator = Paragraph::new(indicator_line);
            frame.render_widget(indicator, area);
            return;
        }

        // Use dots spinner
        let spinner_index = (app.tick_count % 10) as usize;
        let spinner = SPINNER_FRAMES[spinner_index];

        // Find the last running tool event in the message
        let running_tool_name = streaming_msg.segments.iter().rev().find_map(|seg| {
            if let MessageSegment::ToolEvent(event) = seg {
                if event.status == ToolEventStatus::Running {
                    return Some(event.function_name.clone());
                }
            }
            None
        });

        // Calculate max tool name length based on LayoutContext
        let max_tool_name_len = if ctx.is_extra_small() {
            15 // Very short for extra small
        } else if ctx.is_narrow() {
            20 // Short for narrow
        } else {
            40 // Full length for normal terminals
        };

        let status_text = if let Some(tool_name) = running_tool_name {
            let truncated_name = truncate_string(&tool_name, max_tool_name_len);
            format!("Using {}...", truncated_name)
        } else {
            "Responding...".to_string()
        };

        // Add cancel hint for normal streaming
        let cancel_hint = if ctx.is_extra_small() {
            "" // No hint on very small terminals
        } else if ctx.is_narrow() {
            " [^C]"
        } else {
            " [Ctrl+C: Cancel]"
        };

        let indicator_line = Line::from(vec![
            Span::styled(
                format!("  {} {}", spinner, status_text),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(cancel_hint, Style::default().fg(Color::DarkGray)),
        ]);

        let indicator = Paragraph::new(indicator_line);
        frame.render_widget(indicator, area);
    }
}

/// Render the thread title header with connection status and badges
///
/// Adapts to terminal dimensions using `LayoutContext`:
/// - On narrow terminals, badges are abbreviated
/// - Title and description are truncated to fit based on available width
/// - On compact terminals (short height), description may be hidden
pub fn render_conversation_header(frame: &mut Frame, area: Rect, app: &App, ctx: &LayoutContext) {
    let is_narrow = ctx.is_narrow();
    let is_extra_small = ctx.is_extra_small();
    let is_compact = ctx.is_short();

    // Get thread title and description from cache or default
    let thread_info = app
        .active_thread_id
        .as_ref()
        .and_then(|id| app.cache.get_thread(id));

    let thread_title = thread_info
        .map(|t| t.title.as_str())
        .unwrap_or("New Conversation");

    let thread_description = thread_info.and_then(|t| t.description.clone());

    // Get model name from thread if available
    let model_name = thread_info.and_then(|t| t.model.clone());

    let header_block = Block::default()
        .borders(Borders::BOTTOM)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(COLOR_BORDER));

    // Build badges - adapt to terminal width
    let mut badges: Vec<Span> = Vec::new();

    // Skills badge [skills: N] - abbreviated on narrow terminals, hidden on extra small
    let skills_count = app.session_state.skills.len();
    if skills_count > 0 && !is_extra_small {
        let skills_badge = if is_narrow {
            format!("[s:{}] ", skills_count)
        } else {
            format!("[skills: {}] ", skills_count)
        };
        badges.push(Span::styled(skills_badge, Style::default().fg(Color::Cyan)));
    }

    // Context progress bar - abbreviated on narrow terminals
    let ctx_badge = match (
        app.session_state.context_tokens_used,
        app.session_state.context_token_limit,
    ) {
        (Some(used), Some(limit)) if limit > 0 => {
            let percentage = (used as f64 / limit as f64 * 100.0).round() as u32;
            if is_extra_small {
                // Minimal: just percentage
                format!("{}% ", percentage)
            } else if is_narrow {
                // Short bar with 5 blocks
                let filled_blocks = (percentage / 20).min(5) as usize;
                let empty_blocks = 5 - filled_blocks;
                format!(
                    "[{}{}] {}% ",
                    "\u{2588}".repeat(filled_blocks),
                    "\u{2591}".repeat(empty_blocks),
                    percentage
                )
            } else {
                // Full bar with 10 blocks
                let filled_blocks = (percentage / 10).min(10) as usize;
                let empty_blocks = 10 - filled_blocks;
                format!(
                    "[{}{}] {}% ",
                    "\u{2588}".repeat(filled_blocks),
                    "\u{2591}".repeat(empty_blocks),
                    percentage
                )
            }
        }
        _ => {
            if is_extra_small {
                "-- ".to_string()
            } else if is_narrow {
                "[\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}] -- ".to_string()
            } else {
                "[\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}] -- ".to_string()
            }
        }
    };
    badges.push(Span::styled(ctx_badge, Style::default().fg(COLOR_DIM)));

    // Model badge [sonnet] if available - abbreviated on narrow terminals
    if let Some(model) = model_name {
        if !is_extra_small {
            let model_badge = if is_narrow {
                // Truncate model name on narrow terminals
                let short_model = truncate_string(&model, 8);
                format!("[{}] ", short_model)
            } else {
                format!("[{}] ", model)
            };
            badges.push(Span::styled(
                model_badge,
                Style::default().fg(Color::Magenta),
            ));
        }
    }

    // Connection status badge (always shown)
    let (status_icon, status_color) = if app.connection_status {
        ("\u{25CF}", Color::LightGreen)
    } else {
        ("\u{25CB}", Color::Red)
    };
    badges.push(Span::styled(status_icon, Style::default().fg(status_color)));

    // Split header area to show title on left and badges on right
    let badges_width = badges.iter().map(|s| s.content.len()).sum::<usize>() + 2;
    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(20),                     // Thread title (flexible)
            Constraint::Length(badges_width as u16), // Badges (dynamic)
        ])
        .split(area);

    // Calculate max title length using LayoutContext
    let max_title_len = ctx.max_title_length();

    // Truncate title if needed
    let display_title = truncate_string(thread_title, max_title_len);

    // Thread title and description (left side)
    let mut title_lines = vec![Line::from(vec![
        Span::styled("  Thread: ", Style::default().fg(COLOR_DIM)),
        Span::styled(
            display_title,
            Style::default()
                .fg(COLOR_HEADER)
                .add_modifier(Modifier::BOLD),
        ),
    ])];

    // Add description line if present, not empty, and we have space (not compact)
    if !is_compact {
        if let Some(description) = thread_description {
            if !description.is_empty() {
                // Truncate description using LayoutContext's preview length
                let max_desc_len = ctx.max_preview_length();
                let display_desc = truncate_string(&description, max_desc_len);
                title_lines.push(Line::from(vec![
                    Span::styled("  ", Style::default()),
                    Span::styled(display_desc, Style::default().fg(COLOR_DIM)),
                ]));
            }
        }
    }

    // Add working directory line if present
    if let Some(thread) = thread_info {
        if let Some(wd) = thread.working_directory.as_ref() {
            title_lines.push(Line::from(Span::styled(
                format!("  📁 {}", wd),
                Style::default().fg(COLOR_DIM),
            )));
        }
    }

    let title_widget = Paragraph::new(title_lines).block(header_block);
    frame.render_widget(title_widget, header_chunks[0]);

    // Badges (right side)
    let badges_widget =
        Paragraph::new(Line::from(badges)).alignment(ratatui::layout::Alignment::Right);
    frame.render_widget(badges_widget, header_chunks[1]);
}
