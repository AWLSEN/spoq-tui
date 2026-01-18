//! Input area rendering
//!
//! Implements the input box, keybind hints, and permission prompt UI.
//! This module provides responsive rendering that adapts to terminal dimensions.

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Widget},
    Frame,
};

use crate::app::{App, Screen};
use crate::models::PermissionMode;
use crate::state::session::{AskUserQuestionData, AskUserQuestionState, PermissionRequest};

use super::helpers::render_dialog_background;
use super::layout::LayoutContext;
use super::theme::{COLOR_ACCENT, COLOR_DIM};

// ============================================================================
// Folder Chip Constants
// ============================================================================

/// Maximum display length for folder name in chip (truncate if longer)
const MAX_CHIP_FOLDER_NAME_LEN: usize = 20;

/// Background color for folder chip - subtle dark blue
const COLOR_CHIP_BG: Color = Color::Rgb(40, 44, 52);

/// Text color for folder chip
const COLOR_CHIP_TEXT: Color = Color::White;

// ============================================================================
// Folder Chip Rendering
// ============================================================================

/// Format the folder name for display in the chip.
///
/// Truncates to MAX_CHIP_FOLDER_NAME_LEN characters and adds "..." if truncated.
fn format_chip_folder_name(name: &str) -> String {
    if name.len() > MAX_CHIP_FOLDER_NAME_LEN {
        format!("{}...", &name[..MAX_CHIP_FOLDER_NAME_LEN.saturating_sub(3)])
    } else {
        name.to_string()
    }
}

/// Calculate the width of the folder chip in columns.
///
/// Returns the width including the brackets and emoji: `[📁 folder-name]`
fn calculate_chip_width(folder_name: &str) -> u16 {
    let display_name = format_chip_folder_name(folder_name);
    // Format: "[📁 " (4 chars) + name + "]" (1 char)
    // Note: emoji 📁 is typically 2 columns wide
    (3 + display_name.len() + 1) as u16
}

/// Render the folder chip directly to the buffer.
///
/// The chip is rendered at the specified position with the format: `[📁 folder-name]`
fn render_folder_chip(buf: &mut Buffer, x: u16, y: u16, folder_name: &str) {
    let display_name = format_chip_folder_name(folder_name);
    let chip_text = format!("[📁 {}]", display_name);

    let style = Style::default()
        .fg(COLOR_CHIP_TEXT)
        .bg(COLOR_CHIP_BG);

    buf.set_string(x, y, &chip_text, style);
}

// ============================================================================
// AskUserQuestion Parsing
// ============================================================================

/// Parse AskUserQuestion tool input into structured data.
///
/// Attempts to deserialize the tool_input JSON value into an `AskUserQuestionData`.
/// Returns `None` if the input doesn't match the expected structure.
pub fn parse_ask_user_question(tool_input: &serde_json::Value) -> Option<AskUserQuestionData> {
    serde_json::from_value(tool_input.clone()).ok()
}

// ============================================================================
// Input Height Calculation
// ============================================================================

/// Maximum number of visible lines in the input area
const MAX_INPUT_LINES: u16 = 5;

/// Calculate the dynamic input box height based on line count.
///
/// Returns height in rows (including borders):
/// - Min: 3 rows (border + 1 line + border)
/// - Max: 7 rows (border + 5 lines + border)
pub fn calculate_input_box_height(line_count: usize) -> u16 {
    let content_lines = (line_count as u16).clamp(1, MAX_INPUT_LINES);
    content_lines + 2 // +2 for top/bottom borders
}

/// Calculate the total input area height (input box + keybinds + padding).
pub fn calculate_input_area_height(line_count: usize) -> u16 {
    calculate_input_box_height(line_count) + 1 + 2 // +1 keybinds, +2 for top/bottom padding
}

// ============================================================================
// Input Area
// ============================================================================

pub fn render_input_area(frame: &mut Frame, area: Rect, app: &mut App) {
    // Input is always "focused" since we removed panel focus cycling
    let input_focused = true;

    // No border - use spacing at top instead for visual separation
    let inner = Rect {
        x: area.x + 1,
        y: area.y + 1, // 1 row spacing at top
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };

    // Calculate chip width if a folder is selected
    let chip_width = app
        .selected_folder
        .as_ref()
        .map(|f| calculate_chip_width(&f.name) + 1) // +1 for space after chip
        .unwrap_or(0);

    // Calculate content width (accounting for input box borders and chip)
    let content_width = inner.width.saturating_sub(2).saturating_sub(chip_width);

    // Set hard wrap width so auto-newlines are inserted during typing
    app.textarea.set_wrap_width(Some(content_width));

    // Calculate dynamic input box height based on line count
    let line_count = app.textarea.line_count();
    let input_box_height = calculate_input_box_height(line_count);

    let input_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(input_box_height), // Input box (dynamic height)
            Constraint::Length(1),                // Keybinds
        ])
        .split(inner);

    // Render the folder chip + input widget using our custom composite widget
    let input_with_chip = InputWithChipWidget {
        textarea_input: &mut app.textarea,
        focused: input_focused,
        selected_folder: app.selected_folder.as_ref(),
    };
    frame.render_widget(input_with_chip, input_chunks[0]);

    // Build responsive keybind hints based on terminal dimensions
    let ctx = LayoutContext::new(app.terminal_width, app.terminal_height);
    let keybinds = build_responsive_keybinds(app, &ctx);

    let keybinds_widget = Paragraph::new(keybinds);
    frame.render_widget(keybinds_widget, input_chunks[1]);
}

/// Widget that renders a folder chip followed by the TextArea input.
///
/// This composite widget handles:
/// - Rendering the folder chip at the start (if selected)
/// - Rendering the TextArea input in the remaining space
struct InputWithChipWidget<'a, 'b> {
    textarea_input: &'b mut crate::widgets::textarea_input::TextAreaInput<'a>,
    focused: bool,
    selected_folder: Option<&'b crate::models::Folder>,
}

impl Widget for InputWithChipWidget<'_, '_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Create the outer border block
        let border_style = Style::default().fg(COLOR_DIM);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(border_style);

        // Render the border
        let inner_area = block.inner(area);
        block.render(area, buf);

        // If a folder is selected, render the chip at the start of the input
        let textarea_area = if let Some(folder) = self.selected_folder {
            let chip_width = calculate_chip_width(&folder.name);
            let spacing = 1u16; // Space after chip

            // Render the chip at the start of the inner area (top-left)
            render_folder_chip(buf, inner_area.x, inner_area.y, &folder.name);

            // Calculate remaining area for textarea
            let chip_total_width = chip_width + spacing;
            let textarea_x = inner_area.x + chip_total_width;
            let textarea_width = inner_area.width.saturating_sub(chip_total_width);

            Rect {
                x: textarea_x,
                y: inner_area.y,
                width: textarea_width,
                height: inner_area.height,
            }
        } else {
            inner_area
        };

        // Render textarea without border (we handle the border ourselves)
        self.textarea_input
            .render_without_border(textarea_area, buf, self.focused);
    }
}


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
                spans.push(Span::styled("[Shift+Tab]", Style::default().fg(COLOR_ACCENT)));
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
                spans.push(Span::styled("[Alt+Enter]", Style::default().fg(COLOR_ACCENT)));
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
                spans.push(Span::styled(
                    "[Cmd] links",
                    Style::default().fg(COLOR_DIM),
                ));
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
                spans.push(Span::styled("[Shift+Tab]", Style::default().fg(COLOR_ACCENT)));
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
                spans.push(Span::styled("[Alt+Enter]", Style::default().fg(COLOR_ACCENT)));
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

// ============================================================================
// Inline Permission Prompt
// ============================================================================

/// Minimum width for the permission box (must fit keyboard options)
const MIN_PERMISSION_BOX_WIDTH: u16 = 30;
/// Default/maximum width for the permission box
const DEFAULT_PERMISSION_BOX_WIDTH: u16 = 60;
/// Default height for the permission box
const DEFAULT_PERMISSION_BOX_HEIGHT: u16 = 10;
/// Minimum height for a compact permission box (skips preview)
const MIN_PERMISSION_BOX_HEIGHT: u16 = 6;

/// Render an inline permission prompt in the message flow.
///
/// Shows a Claude Code-style permission box with:
/// - Tool name and description
/// - Preview of the action (file path, command, etc.)
/// - Keyboard options: [y] Yes, [a] Always, [n] No
///
/// For AskUserQuestion tools, renders a special tabbed question UI instead.
pub fn render_permission_prompt(frame: &mut Frame, area: Rect, app: &App) {
    if let Some(ref perm) = app.session_state.pending_permission {
        let ctx = LayoutContext::new(app.terminal_width, app.terminal_height);
        render_permission_box(frame, area, perm, &app.question_state, &ctx);
    }
}

/// Render the permission request box.
///
/// For AskUserQuestion tools, delegates to `render_ask_user_question_box`.
/// For other tools, renders the standard responsive permission prompt.
pub fn render_permission_box(
    frame: &mut Frame,
    area: Rect,
    perm: &PermissionRequest,
    question_state: &AskUserQuestionState,
    ctx: &LayoutContext,
) {
    // Check if this is an AskUserQuestion tool - render special UI
    if perm.tool_name == "AskUserQuestion" {
        if let Some(ref tool_input) = perm.tool_input {
            if let Some(data) = parse_ask_user_question(tool_input) {
                render_ask_user_question_box(frame, area, perm, &data, question_state);
                return;
            }
        }
    }

    // Standard responsive permission box for other tools
    // Calculate responsive box dimensions
    let available_width = area.width.saturating_sub(4);
    let box_width = if ctx.is_extra_small() {
        // Extra small: use minimum width or available space
        MIN_PERMISSION_BOX_WIDTH.min(available_width)
    } else if ctx.is_narrow() {
        // Narrow: scale down from default
        ctx.bounded_width(70, MIN_PERMISSION_BOX_WIDTH, DEFAULT_PERMISSION_BOX_WIDTH)
            .min(available_width)
    } else {
        // Normal: use default width
        DEFAULT_PERMISSION_BOX_WIDTH.min(available_width)
    };

    // Calculate responsive height
    let available_height = area.height.saturating_sub(2);
    let show_preview = !ctx.is_short() && available_height >= DEFAULT_PERMISSION_BOX_HEIGHT;
    let box_height = if show_preview {
        DEFAULT_PERMISSION_BOX_HEIGHT.min(available_height)
    } else {
        MIN_PERMISSION_BOX_HEIGHT.min(available_height)
    };

    // Center the box
    let x = area.x + (area.width.saturating_sub(box_width)) / 2;
    let y = area.y + (area.height.saturating_sub(box_height)) / 2;

    let box_area = Rect {
        x,
        y,
        width: box_width,
        height: box_height,
    };

    // Choose title based on available width
    let title = if ctx.is_narrow() {
        " Permission "
    } else {
        " Permission Required "
    };

    // Render solid background
    render_dialog_background(frame, box_area);

    // Create the permission box with border
    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(COLOR_DIM));

    // Render the block
    frame.render_widget(block, box_area);

    // Inner area for content
    let inner = Rect {
        x: box_area.x + 2,
        y: box_area.y + 1,
        width: box_area.width.saturating_sub(4),
        height: box_area.height.saturating_sub(2),
    };

    // Build content lines
    let mut lines: Vec<Line> = Vec::new();

    // Tool name line - truncate description on narrow terminals
    let max_desc_width = (inner.width as usize).saturating_sub(perm.tool_name.len() + 2);
    let description = if perm.description.len() > max_desc_width && max_desc_width > 3 {
        format!("{}...", &perm.description[..max_desc_width.saturating_sub(3)])
    } else {
        perm.description.clone()
    };

    lines.push(Line::from(vec![
        Span::styled(
            format!("{}: ", perm.tool_name),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(description, Style::default().fg(Color::White)),
    ]));

    lines.push(Line::from("")); // Empty line

    // Preview box - only show if we have space
    if show_preview {
        let preview_content = get_permission_preview(perm);
        if !preview_content.is_empty() {
            // Preview border top
            lines.push(Line::from(vec![Span::styled(
                format!(
                    "┌{}┐",
                    "─".repeat((inner.width as usize).saturating_sub(2))
                ),
                Style::default().fg(COLOR_DIM),
            )]));

            // Preview content (truncated if needed)
            let max_preview_width = (inner.width as usize).saturating_sub(4);
            let max_preview_lines = if ctx.is_short() { 1 } else { 3 };
            for line in preview_content.lines().take(max_preview_lines) {
                let truncated = if line.len() > max_preview_width && max_preview_width > 3 {
                    format!("{}...", &line[..max_preview_width.saturating_sub(3)])
                } else {
                    line.to_string()
                };
                lines.push(Line::from(vec![
                    Span::styled("| ", Style::default().fg(COLOR_DIM)),
                    Span::styled(truncated, Style::default().fg(COLOR_DIM)),
                    Span::raw(" "),
                ]));
            }

            // Preview border bottom
            lines.push(Line::from(vec![Span::styled(
                format!(
                    "└{}┘",
                    "─".repeat((inner.width as usize).saturating_sub(2))
                ),
                Style::default().fg(COLOR_DIM),
            )]));
        }

        lines.push(Line::from("")); // Empty line
    }

    lines.push(Line::from("")); // Empty line

    // Calculate timeout countdown (server times out at 55s)
    let elapsed_secs = perm.received_at.elapsed().as_secs();
    let remaining_secs = 55u64.saturating_sub(elapsed_secs);

    // Style countdown: white when urgent (<5s), dim otherwise
    let countdown_color = if remaining_secs <= 5 {
        Color::White
    } else {
        COLOR_DIM
    };

    let countdown_text = if remaining_secs == 0 {
        " (expired)".to_string()
    } else {
        format!(" ({}s)", remaining_secs)
    };

    // Keyboard options - compact on narrow terminals, with countdown
    if ctx.is_extra_small() {
        // Extra compact: [y]/[a]/[n] (countdown)
        lines.push(Line::from(vec![
            Span::styled(
                "[y]",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("/"),
            Span::styled(
                "[a]",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("/"),
            Span::styled(
                "[n]",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(countdown_text, Style::default().fg(countdown_color)),
        ]));
    } else if ctx.is_narrow() {
        // Compact: [y] Y  [a] A  [n] N (countdown)
        lines.push(Line::from(vec![
            Span::styled(
                "[y]",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Y "),
            Span::styled(
                "[a]",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" A "),
            Span::styled(
                "[n]",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" N"),
            Span::styled(countdown_text, Style::default().fg(countdown_color)),
        ]));
    } else {
        // Full: [y] Yes  [a] Always  [n] No (countdown)
        lines.push(Line::from(vec![
            Span::styled(
                "[y]",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Yes  "),
            Span::styled(
                "[a]",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Always  "),
            Span::styled(
                "[n]",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" No"),
            Span::styled(countdown_text, Style::default().fg(countdown_color)),
        ]));
    }

    let content = Paragraph::new(lines);
    frame.render_widget(content, inner);
}

/// Extract preview content from a PermissionRequest.
pub fn get_permission_preview(perm: &PermissionRequest) -> String {
    // First try context (human-readable description)
    if let Some(ref ctx) = perm.context {
        return ctx.clone();
    }

    // Fall back to tool_input if available
    if let Some(ref input) = perm.tool_input {
        // Try to extract common fields
        if let Some(path) = input.get("file_path").and_then(|v| v.as_str()) {
            return path.to_string();
        }
        if let Some(cmd) = input.get("command").and_then(|v| v.as_str()) {
            return cmd.to_string();
        }
        if let Some(content) = input.get("content").and_then(|v| v.as_str()) {
            // Truncate long content (respecting UTF-8 boundaries)
            if content.len() > 100 {
                return super::helpers::truncate_string(content, 100);
            }
            return content.to_string();
        }
        // Fallback: pretty print JSON
        if let Ok(pretty) = serde_json::to_string_pretty(input) {
            return pretty;
        }
    }

    String::new()
}

// ============================================================================
// Input Section Builder (Unified Scroll)
// ============================================================================

/// Build the input section as content lines for unified scroll.
///
/// Returns lines for: separator, label, input content with border, keybinds.
pub fn build_input_section(app: &App, viewport_width: u16) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let content_width = viewport_width.saturating_sub(4) as usize; // margins

    // 1. Blank line separator
    lines.push(Line::from(""));

    // 2. Mode indicator (only shown when Plan or Execute mode is active)
    let mode_line = match app.permission_mode {
        PermissionMode::Default => Line::from(""),
        PermissionMode::Plan => Line::from(vec![Span::styled(
            "  [PLAN]",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        )]),
        PermissionMode::BypassPermissions => Line::from(vec![Span::styled(
            "  [EXECUTE]",
            Style::default()
                .fg(Color::Rgb(255, 140, 0))
                .add_modifier(Modifier::BOLD),
        )]),
    };
    lines.push(mode_line);

    // 3. Input top border (full-width horizontal line)
    let full_width = content_width + 6; // Account for removed indent and borders
    lines.push(Line::from(Span::styled(
        "─".repeat(full_width),
        Style::default().fg(COLOR_ACCENT),
    )));

    // 4. Input content lines (from tui-textarea)
    for input_line in app.textarea.to_content_lines() {
        let mut styled_spans = vec![Span::styled("  ", Style::default())];
        styled_spans.extend(input_line.spans);
        lines.push(Line::from(styled_spans));
    }

    // 5. Input bottom border (full-width horizontal line)
    lines.push(Line::from(Span::styled(
        "─".repeat(full_width),
        Style::default().fg(COLOR_ACCENT),
    )));

    // 6. Keybind hints
    lines.push(Line::from(vec![
        Span::styled("    Enter", Style::default().fg(COLOR_DIM)),
        Span::styled(" send ", Style::default().fg(COLOR_DIM)),
        Span::styled("|", Style::default().fg(COLOR_DIM)),
        Span::styled(" Shift+Enter", Style::default().fg(COLOR_DIM)),
        Span::styled(" newline ", Style::default().fg(COLOR_DIM)),
        Span::styled("|", Style::default().fg(COLOR_DIM)),
        Span::styled(" Esc", Style::default().fg(COLOR_DIM)),
        Span::styled(" menu", Style::default().fg(COLOR_DIM)),
    ]));

    // 7. Bottom padding
    lines.push(Line::from(""));

    lines
}

// ============================================================================
// AskUserQuestion UI Renderer
// ============================================================================

/// Render the AskUserQuestion prompt as a tabbed question UI.
///
/// Displays:
/// - Tab bar for multiple questions (only if > 1 question)
/// - Current question text
/// - Options with selection highlight
/// - "Other..." option at bottom
/// - Keybind help and countdown timer
fn render_ask_user_question_box(
    frame: &mut Frame,
    area: Rect,
    perm: &PermissionRequest,
    data: &AskUserQuestionData,
    state: &AskUserQuestionState,
) {
    if data.questions.is_empty() {
        return;
    }

    // Calculate box dimensions - larger than standard permission box for questions
    let box_width = 65u16.min(area.width.saturating_sub(4));
    // Height: border(2) + tabs?(1) + question(2) + options(varies) + help(1) + padding
    let num_options = data
        .questions
        .get(state.tab_index)
        .map(|q| q.options.len())
        .unwrap_or(0);
    // Each option takes 2-3 lines (label + description)
    let options_height = ((num_options + 1) * 3) as u16; // +1 for "Other"
    let base_height = 8u16; // borders + question + help + padding
    let box_height = (base_height + options_height).min(area.height.saturating_sub(2));

    // Center the box
    let x = area.x + (area.width.saturating_sub(box_width)) / 2;
    let y = area.y + (area.height.saturating_sub(box_height)) / 2;

    let box_area = Rect {
        x,
        y,
        width: box_width,
        height: box_height,
    };

    // Render solid background
    render_dialog_background(frame, box_area);

    // Create the question box with border
    let block = Block::default()
        .title(Span::styled(
            " Question ",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(COLOR_DIM));

    frame.render_widget(block, box_area);

    // Inner area for content
    let inner = Rect {
        x: box_area.x + 2,
        y: box_area.y + 1,
        width: box_area.width.saturating_sub(4),
        height: box_area.height.saturating_sub(2),
    };

    let mut lines: Vec<Line> = Vec::new();
    let current_question = &data.questions[state.tab_index.min(data.questions.len() - 1)];

    // Tab bar (only for multiple questions)
    if data.questions.len() > 1 {
        let mut tab_spans: Vec<Span> = Vec::new();
        for (i, q) in data.questions.iter().enumerate() {
            if i > 0 {
                tab_spans.push(Span::raw("  "));
            }
            if i == state.tab_index {
                // Active tab: white with brackets
                tab_spans.push(Span::styled(
                    format!("[{}]", q.header),
                    Style::default().fg(Color::White),
                ));
            } else {
                // Inactive tab: dim gray
                tab_spans.push(Span::styled(
                    q.header.clone(),
                    Style::default().fg(COLOR_DIM),
                ));
            }
        }
        lines.push(Line::from(tab_spans));
        lines.push(Line::from("")); // Empty line after tabs
    }

    // Question text
    lines.push(Line::from(Span::styled(
        current_question.question.clone(),
        Style::default().fg(Color::White),
    )));
    lines.push(Line::from("")); // Empty line after question

    // Options
    let current_selection = state.current_selection();

    for (i, opt) in current_question.options.iter().enumerate() {
        let is_selected = current_selection == Some(i);

        if current_question.multi_select {
            // Multi-select mode: show checkboxes
            let checkbox = if state.is_multi_selected(i) {
                "[×]"
            } else {
                "[ ]"
            };
            let marker = if is_selected { "› " } else { "  " };

            lines.push(Line::from(vec![
                Span::styled(
                    format!("{}{} ", marker, checkbox),
                    if is_selected {
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(COLOR_DIM)
                    },
                ),
                Span::styled(
                    opt.label.clone(),
                    if is_selected {
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(COLOR_DIM)
                    },
                ),
            ]));
        } else {
            // Single-select mode: show arrow marker
            let marker = if is_selected { "› " } else { "  " };

            lines.push(Line::from(vec![
                Span::styled(
                    marker,
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    opt.label.clone(),
                    if is_selected {
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(COLOR_DIM)
                    },
                ),
            ]));
        }

        // Description (indented, dim)
        if !opt.description.is_empty() {
            lines.push(Line::from(Span::styled(
                format!("    {}", opt.description),
                Style::default().fg(COLOR_DIM),
            )));
        }
        lines.push(Line::from("")); // Spacing between options
    }

    // "Other..." option
    let is_other_selected = current_selection.is_none();
    let other_marker = if is_other_selected { "› " } else { "  " };
    lines.push(Line::from(vec![
        Span::styled(
            other_marker,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "Other...",
            if is_other_selected {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(COLOR_DIM)
            },
        ),
    ]));

    // "Other" text input (if active)
    if state.other_active && is_other_selected {
        let other_text = state.current_other_text();
        let input_width = inner.width.saturating_sub(8) as usize;
        let display_text = if other_text.len() > input_width {
            &other_text[other_text.len() - input_width..]
        } else {
            other_text
        };

        // Input box top border
        lines.push(Line::from(Span::styled(
            format!("    ┌{}┐", "─".repeat(input_width + 2)),
            Style::default().fg(COLOR_DIM),
        )));
        // Input content with cursor
        lines.push(Line::from(vec![
            Span::styled("    │ ", Style::default().fg(COLOR_DIM)),
            Span::styled(display_text, Style::default().fg(Color::White)),
            Span::styled("█", Style::default().fg(Color::White)),
            Span::raw(" ".repeat(input_width.saturating_sub(display_text.len()))),
            Span::styled("│", Style::default().fg(COLOR_DIM)),
        ]));
        // Input box bottom border
        lines.push(Line::from(Span::styled(
            format!("    └{}┘", "─".repeat(input_width + 2)),
            Style::default().fg(COLOR_DIM),
        )));
    }

    lines.push(Line::from("")); // Empty line before help

    // Calculate timeout countdown
    let elapsed_secs = perm.received_at.elapsed().as_secs();
    let remaining_secs = 55u64.saturating_sub(elapsed_secs);

    let countdown_color = if remaining_secs <= 5 {
        Color::Red
    } else {
        COLOR_DIM
    };

    let countdown_text = if remaining_secs == 0 {
        "(expired)".to_string()
    } else {
        format!("({}s)", remaining_secs)
    };

    // Help line - varies based on mode
    let mut help_spans: Vec<Span> = Vec::new();

    if state.other_active {
        // "Other" text input mode
        help_spans.push(Span::styled("esc", Style::default().fg(COLOR_DIM)));
        help_spans.push(Span::styled(" cancel  ", Style::default().fg(COLOR_DIM)));
        help_spans.push(Span::styled("enter", Style::default().fg(COLOR_DIM)));
        help_spans.push(Span::styled(" submit", Style::default().fg(COLOR_DIM)));
    } else {
        // Tab switching (only if multiple questions)
        if data.questions.len() > 1 {
            help_spans.push(Span::styled("tab", Style::default().fg(COLOR_DIM)));
            help_spans.push(Span::styled(" switch  ", Style::default().fg(COLOR_DIM)));
        }

        if current_question.multi_select {
            help_spans.push(Span::styled("space", Style::default().fg(COLOR_DIM)));
            help_spans.push(Span::styled(" toggle  ", Style::default().fg(COLOR_DIM)));
        }

        help_spans.push(Span::styled("↑↓", Style::default().fg(COLOR_DIM)));
        help_spans.push(Span::styled(" navigate  ", Style::default().fg(COLOR_DIM)));
        help_spans.push(Span::styled("enter", Style::default().fg(COLOR_DIM)));
        help_spans.push(Span::styled(
            if current_question.multi_select {
                " submit"
            } else {
                " confirm"
            },
            Style::default().fg(COLOR_DIM),
        ));
    }

    // Add countdown at the end with spacing
    let help_text_len: usize = help_spans.iter().map(|s| s.content.len()).sum();
    let countdown_len = countdown_text.len();
    let padding_needed = (inner.width as usize)
        .saturating_sub(help_text_len)
        .saturating_sub(countdown_len);

    help_spans.push(Span::raw(" ".repeat(padding_needed)));
    help_spans.push(Span::styled(countdown_text, Style::default().fg(countdown_color)));

    lines.push(Line::from(help_spans));

    let content = Paragraph::new(lines);
    frame.render_widget(content, inner);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_ask_user_question_valid() {
        let input = json!({
            "questions": [
                {
                    "question": "Which library should we use?",
                    "header": "Auth method",
                    "options": [
                        {"label": "Option A", "description": "Description of A"},
                        {"label": "Option B", "description": "Description of B"}
                    ],
                    "multiSelect": false
                }
            ],
            "answers": {}
        });

        let result = parse_ask_user_question(&input);
        assert!(result.is_some());

        let data = result.unwrap();
        assert_eq!(data.questions.len(), 1);
        assert_eq!(data.questions[0].question, "Which library should we use?");
        assert_eq!(data.questions[0].header, "Auth method");
        assert_eq!(data.questions[0].options.len(), 2);
        assert!(!data.questions[0].multi_select);
    }

    #[test]
    fn test_parse_ask_user_question_multi_select() {
        let input = json!({
            "questions": [
                {
                    "question": "Select features",
                    "header": "Features",
                    "options": [
                        {"label": "A", "description": "Feature A"},
                        {"label": "B", "description": "Feature B"}
                    ],
                    "multiSelect": true
                }
            ],
            "answers": {}
        });

        let result = parse_ask_user_question(&input);
        assert!(result.is_some());
        assert!(result.unwrap().questions[0].multi_select);
    }

    #[test]
    fn test_parse_ask_user_question_missing_multi_select_defaults() {
        let input = json!({
            "questions": [
                {
                    "question": "Test?",
                    "header": "Test",
                    "options": []
                }
            ]
        });

        let result = parse_ask_user_question(&input);
        assert!(result.is_some());
        assert!(!result.unwrap().questions[0].multi_select);
    }

    #[test]
    fn test_parse_ask_user_question_with_answers() {
        let input = json!({
            "questions": [
                {
                    "question": "Test?",
                    "header": "Test",
                    "options": [{"label": "A", "description": "a"}],
                    "multiSelect": false
                }
            ],
            "answers": {"q1": "answer1"}
        });

        let result = parse_ask_user_question(&input);
        assert!(result.is_some());
        let data = result.unwrap();
        assert_eq!(data.answers.get("q1"), Some(&"answer1".to_string()));
    }

    #[test]
    fn test_parse_ask_user_question_multiple_questions() {
        let input = json!({
            "questions": [
                {
                    "question": "First?",
                    "header": "Q1",
                    "options": [{"label": "A", "description": "a"}],
                    "multiSelect": false
                },
                {
                    "question": "Second?",
                    "header": "Q2",
                    "options": [{"label": "B", "description": "b"}],
                    "multiSelect": true
                }
            ],
            "answers": {}
        });

        let result = parse_ask_user_question(&input);
        assert!(result.is_some());
        let data = result.unwrap();
        assert_eq!(data.questions.len(), 2);
        assert_eq!(data.questions[0].header, "Q1");
        assert_eq!(data.questions[1].header, "Q2");
    }

    #[test]
    fn test_parse_ask_user_question_invalid_missing_questions() {
        let input = json!({
            "answers": {}
        });

        let result = parse_ask_user_question(&input);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_ask_user_question_invalid_wrong_type() {
        let input = json!({
            "questions": "not an array",
            "answers": {}
        });

        let result = parse_ask_user_question(&input);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_ask_user_question_invalid_empty_object() {
        let input = json!({});

        let result = parse_ask_user_question(&input);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_ask_user_question_invalid_null() {
        let input = json!(null);

        let result = parse_ask_user_question(&input);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_ask_user_question_invalid_missing_required_question_fields() {
        let input = json!({
            "questions": [
                {
                    "header": "Test"
                    // missing "question" and "options"
                }
            ],
            "answers": {}
        });

        let result = parse_ask_user_question(&input);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_ask_user_question_completely_unrelated_json() {
        let input = json!({
            "command": "npm install",
            "file_path": "/some/path"
        });

        let result = parse_ask_user_question(&input);
        assert!(result.is_none());
    }

    // ========================================================================
    // Test Helpers
    // ========================================================================

    fn create_test_app() -> App {
        App::default()
    }

    // ========================================================================
    // Input Height Calculation Tests
    // ========================================================================

    #[test]
    fn test_calculate_input_box_height_single_line() {
        assert_eq!(calculate_input_box_height(1), 3, "Single line: 1 + 2 borders = 3");
    }

    #[test]
    fn test_calculate_input_box_height_multiple_lines() {
        assert_eq!(calculate_input_box_height(2), 4, "2 lines: 2 + 2 borders = 4");
        assert_eq!(calculate_input_box_height(3), 5, "3 lines: 3 + 2 borders = 5");
        assert_eq!(calculate_input_box_height(4), 6, "4 lines: 4 + 2 borders = 6");
        assert_eq!(calculate_input_box_height(5), 7, "5 lines: 5 + 2 borders = 7");
    }

    #[test]
    fn test_calculate_input_box_height_clamped_max() {
        assert_eq!(calculate_input_box_height(6), 7, "Max 5 lines + 2 borders = 7");
        assert_eq!(calculate_input_box_height(10), 7, "Max 5 lines + 2 borders = 7");
        assert_eq!(calculate_input_box_height(100), 7, "Max 5 lines + 2 borders = 7");
    }

    #[test]
    fn test_calculate_input_box_height_clamped_min() {
        assert_eq!(calculate_input_box_height(0), 3, "Min 1 line + 2 borders = 3");
    }

    #[test]
    fn test_calculate_input_area_height_includes_keybinds_and_padding() {
        assert_eq!(calculate_input_area_height(1), 6, "Box (3) + keybinds (1) + padding (2) = 6");
        assert_eq!(calculate_input_area_height(5), 10, "Box (7) + keybinds (1) + padding (2) = 10");
    }

    // ========================================================================
    // Responsive Keybinds Tests
    // ========================================================================

    #[test]
    fn test_responsive_keybinds_normal_width() {
        let app = create_test_app();
        let ctx = LayoutContext::new(120, 40);

        let keybinds = build_responsive_keybinds(&app, &ctx);
        let content: String = keybinds.spans.iter().map(|s| s.content.to_string()).collect();

        // Should show full keybinds on normal width
        assert!(content.contains("[Tab Tab]"), "Should show full Tab Tab");
        assert!(content.contains("switch thread"), "Should show full 'switch thread'");
    }

    #[test]
    fn test_responsive_keybinds_narrow_width() {
        let app = create_test_app();
        let ctx = LayoutContext::new(70, 24); // Narrow (< 80)

        let keybinds = build_responsive_keybinds(&app, &ctx);
        let content: String = keybinds.spans.iter().map(|s| s.content.to_string()).collect();

        // Should show abbreviated keybinds on narrow width
        assert!(content.contains("[Tab]"), "Should show abbreviated Tab");
        assert!(content.contains("switch"), "Should show abbreviated 'switch'");
        assert!(!content.contains("switch thread"), "Should NOT show full 'switch thread'");
    }

    #[test]
    fn test_responsive_keybinds_extra_small_width() {
        let app = create_test_app();
        let ctx = LayoutContext::new(50, 24); // Extra small (< 60)

        let keybinds = build_responsive_keybinds(&app, &ctx);
        let content: String = keybinds.spans.iter().map(|s| s.content.to_string()).collect();

        // On extra small, Tab switch hint should be hidden
        assert!(!content.contains("[Tab Tab]"), "Should NOT show Tab Tab on extra small");
        assert!(!content.contains("switch"), "Should NOT show switch on extra small");
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
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: chrono::Utc::now(),
            working_directory: None,
        });
        app.active_thread_id = Some("prog-thread".to_string());

        let ctx = LayoutContext::new(70, 24);
        let keybinds = build_responsive_keybinds(&app, &ctx);
        let content: String = keybinds.spans.iter().map(|s| s.content.to_string()).collect();

        // Should show abbreviated Shift+Tab on narrow
        assert!(content.contains("[S+Tab]"), "Should show abbreviated S+Tab");
        assert!(content.contains("mode"), "Should show abbreviated 'mode'");
        assert!(!content.contains("cycle mode"), "Should NOT show full 'cycle mode'");
    }

    #[test]
    fn test_responsive_keybinds_with_error_narrow() {
        let mut app = create_test_app();
        app.screen = Screen::Conversation;
        app.stream_error = Some("Test error".to_string());

        let ctx = LayoutContext::new(70, 24);
        let keybinds = build_responsive_keybinds(&app, &ctx);
        let content: String = keybinds.spans.iter().map(|s| s.content.to_string()).collect();

        // Should show abbreviated dismiss hint
        assert!(content.contains("dismiss"), "Should show 'dismiss'");
        assert!(!content.contains("dismiss error"), "Should NOT show full 'dismiss error'");
    }

    // ========================================================================
    // Permission Box Responsive Tests
    // ========================================================================

    #[test]
    fn test_permission_box_width_normal() {
        let ctx = LayoutContext::new(120, 40);
        let area = Rect::new(0, 0, 120, 40);

        // For normal width, box should be 60 chars (DEFAULT_PERMISSION_BOX_WIDTH)
        let available = area.width.saturating_sub(4);
        let expected_width = DEFAULT_PERMISSION_BOX_WIDTH.min(available);

        assert_eq!(expected_width, 60);
        // Verify ctx is not narrow or extra small
        assert!(!ctx.is_narrow());
        assert!(!ctx.is_extra_small());
    }

    #[test]
    fn test_permission_box_width_narrow() {
        let ctx = LayoutContext::new(70, 24);

        // Narrow terminals should scale down
        let scaled = ctx.bounded_width(70, MIN_PERMISSION_BOX_WIDTH, DEFAULT_PERMISSION_BOX_WIDTH);

        // 70% of 70 = 49, clamped between 30 and 60
        assert!(scaled >= MIN_PERMISSION_BOX_WIDTH);
        assert!(scaled <= DEFAULT_PERMISSION_BOX_WIDTH);
    }

    #[test]
    fn test_permission_box_width_extra_small() {
        let ctx = LayoutContext::new(50, 24);

        // Extra small should use minimum width
        let available = 50u16.saturating_sub(4);
        let expected_width = MIN_PERMISSION_BOX_WIDTH.min(available);

        assert_eq!(expected_width, MIN_PERMISSION_BOX_WIDTH);
        assert!(ctx.is_extra_small());
    }

    #[test]
    fn test_permission_box_title_changes_on_narrow() {
        // On normal width, title is " Permission Required "
        // On narrow width, title is " Permission "
        let normal_ctx = LayoutContext::new(120, 40);
        let narrow_ctx = LayoutContext::new(70, 24);

        assert!(!normal_ctx.is_narrow());
        assert!(narrow_ctx.is_narrow());
    }

    #[test]
    fn test_permission_box_preview_hidden_on_short() {
        let ctx = LayoutContext::new(80, 20);

        // SM_HEIGHT is 24, so height < 24 means is_short() returns true
        assert!(ctx.is_short()); // 20 < 24

        // Preview should be hidden on short terminals
        let show_preview = !ctx.is_short();
        assert!(!show_preview, "Preview should be hidden on short terminals");
    }

    #[test]
    fn test_permission_box_keyboard_options_normal() {
        // Normal: [y] Yes  [a] Always  [n] No
        let ctx = LayoutContext::new(120, 40);
        assert!(!ctx.is_narrow());
        assert!(!ctx.is_extra_small());
    }

    #[test]
    fn test_permission_box_keyboard_options_narrow() {
        // Narrow: [y] Y  [a] A  [n] N
        let ctx = LayoutContext::new(70, 24);
        assert!(ctx.is_narrow());
        assert!(!ctx.is_extra_small());
    }

    #[test]
    fn test_permission_box_keyboard_options_extra_small() {
        // Extra small: [y]/[a]/[n]
        let ctx = LayoutContext::new(50, 24);
        assert!(ctx.is_extra_small());
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
        let responsive_content: String = responsive.spans.iter().map(|s| s.content.to_string()).collect();

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
        let content: String = keybinds.spans.iter().map(|s| s.content.to_string()).collect();

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
        let content: String = keybinds.spans.iter().map(|s| s.content.to_string()).collect();

        // Should NOT show link hint when no links present
        assert!(!content.contains("[Cmd+click]"), "Should NOT show [Cmd+click]");
        assert!(!content.contains("open links"), "Should NOT show 'open links'");
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
        let content: String = keybinds.spans.iter().map(|s| s.content.to_string()).collect();

        // Should show abbreviated link hint on narrow width
        assert!(content.contains("[Cmd]"), "Should show abbreviated [Cmd]");
        assert!(content.contains("links"), "Should show abbreviated 'links'");
        assert!(!content.contains("[Cmd+click]"), "Should NOT show full [Cmd+click]");
        assert!(!content.contains("open links"), "Should NOT show full 'open links'");
    }

    #[test]
    fn test_link_hint_hidden_on_extra_small() {
        let mut app = create_test_app();
        app.screen = Screen::Conversation;
        app.has_visible_links = true;

        let ctx = LayoutContext::new(50, 24); // Extra small (< 60)
        let keybinds = build_responsive_keybinds(&app, &ctx);
        let content: String = keybinds.spans.iter().map(|s| s.content.to_string()).collect();

        // Link hint should be hidden on extra small terminals
        assert!(!content.contains("[Cmd+click]"), "Should NOT show [Cmd+click]");
        assert!(!content.contains("[Cmd]"), "Should NOT show [Cmd]");
        assert!(!content.contains("open links"), "Should NOT show 'open links'");
    }

    #[test]
    fn test_link_hint_only_on_conversation_screen() {
        let mut app = create_test_app();
        app.screen = Screen::CommandDeck; // Not on conversation screen
        app.has_visible_links = true;

        let ctx = LayoutContext::new(120, 40);
        let keybinds = build_responsive_keybinds(&app, &ctx);
        let content: String = keybinds.spans.iter().map(|s| s.content.to_string()).collect();

        // Link hint should only appear on conversation screen
        assert!(!content.contains("[Cmd+click]"), "Should NOT show link hint on CommandDeck");
        assert!(!content.contains("open links"), "Should NOT show link hint on CommandDeck");
    }

    #[test]
    fn test_link_hint_with_other_hints() {
        let mut app = create_test_app();
        app.screen = Screen::Conversation;
        app.has_visible_links = true;
        app.stream_error = Some("Test error".to_string());

        let ctx = LayoutContext::new(120, 40);
        let keybinds = build_responsive_keybinds(&app, &ctx);
        let content: String = keybinds.spans.iter().map(|s| s.content.to_string()).collect();

        // All hints should coexist
        assert!(content.contains("dismiss error"), "Should show error dismiss hint");
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
        let content: String = keybinds.spans.iter().map(|s| s.content.to_string()).collect();

        // Link hint should appear after the "back" hint
        let back_pos = content.find("back").unwrap();
        let link_pos = content.find("[Cmd+click]").unwrap();
        assert!(link_pos > back_pos, "Link hint should appear after 'back' hint");
    }

    // ========================================================================
    // Folder Chip Tests
    // ========================================================================

    #[test]
    fn test_format_chip_folder_name_short() {
        let name = "project";
        let formatted = format_chip_folder_name(name);
        assert_eq!(formatted, "project");
    }

    #[test]
    fn test_format_chip_folder_name_exact_max() {
        // MAX_CHIP_FOLDER_NAME_LEN is 20
        let name = "12345678901234567890"; // Exactly 20 chars
        let formatted = format_chip_folder_name(name);
        assert_eq!(formatted, "12345678901234567890");
    }

    #[test]
    fn test_format_chip_folder_name_truncated() {
        // MAX_CHIP_FOLDER_NAME_LEN is 20
        let name = "very-long-project-name-that-exceeds-limit";
        let formatted = format_chip_folder_name(name);
        // Should truncate to 17 chars + "..." = 20 chars total
        assert!(formatted.ends_with("..."));
        assert!(formatted.len() <= MAX_CHIP_FOLDER_NAME_LEN);
    }

    #[test]
    fn test_calculate_chip_width() {
        // Format: "[📁 " + name + "]"
        // "[📁 " is 3 chars ([ + emoji(counts as 1 in len) + space)
        // "]" is 1 char
        let width = calculate_chip_width("project");
        // "[📁 project]" = 4 + 7 = 11 characters
        // But emoji 📁 is 2 columns wide, so actual display is 12
        // The function returns: (3 + name.len() + 1) = 3 + 7 + 1 = 11
        assert_eq!(width, 11);
    }

    #[test]
    fn test_calculate_chip_width_long_name_truncated() {
        let name = "very-long-project-name-that-exceeds-limit";
        let width = calculate_chip_width(name);
        // Name gets truncated to 17 + "..." = 20 chars max
        // Width = 3 + 20 + 1 = 24
        assert!(width <= 24);
    }

    #[test]
    fn test_build_input_section_basic_structure() {
        let app = App::default();
        let lines = build_input_section(&app, 80);

        // Should have: blank, label, top border, content lines, bottom border, keybinds, blank
        // Minimum is 7 lines (for empty input with 1 content line)
        assert!(lines.len() >= 7);

        // First line should be blank separator
        assert_eq!(lines[0].spans.len(), 0);

        // Last line should be blank padding
        assert_eq!(lines[lines.len() - 1].spans.len(), 0);
    }

    #[test]
    fn test_build_input_section_default_mode_empty_line() {
        let app = App::default();
        let lines = build_input_section(&app, 80);

        // Second line should be empty for Default mode (no mode indicator)
        let mode_line = &lines[1];
        assert_eq!(mode_line.spans.len(), 0, "Default mode should show empty line");
    }

    #[test]
    fn test_build_input_section_plan_mode_indicator() {
        let mut app = App::default();
        app.permission_mode = PermissionMode::Plan;

        let lines = build_input_section(&app, 80);

        // Second line should show [PLAN] indicator
        let mode_line = &lines[1];
        assert!(mode_line.spans.len() > 0, "Plan mode should show mode indicator");

        let mode_text: String = mode_line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(mode_text.contains("[PLAN]"), "Plan mode should display '[PLAN]'");

        // Verify magenta color styling
        if let Some(span) = mode_line.spans.first() {
            assert_eq!(span.style.fg, Some(Color::Magenta), "Plan mode indicator should be magenta");
        }
    }

    #[test]
    fn test_build_input_section_execute_mode_indicator() {
        let mut app = App::default();
        app.permission_mode = PermissionMode::BypassPermissions;

        let lines = build_input_section(&app, 80);

        // Second line should show [EXECUTE] indicator
        let mode_line = &lines[1];
        assert!(mode_line.spans.len() > 0, "BypassPermissions mode should show mode indicator");

        let mode_text: String = mode_line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(mode_text.contains("[EXECUTE]"), "BypassPermissions mode should display '[EXECUTE]'");

        // Verify orange color styling (RGB 255, 140, 0)
        if let Some(span) = mode_line.spans.first() {
            assert_eq!(span.style.fg, Some(Color::Rgb(255, 140, 0)), "Execute mode indicator should be orange");
        }
    }

    #[test]
    fn test_build_input_section_folder_does_not_affect_mode_line() {
        use crate::models::Folder;

        let mut app = App::default();
        app.selected_folder = Some(Folder {
            name: "my-project".to_string(),
            path: "/path/to/project".to_string(),
        });

        let lines = build_input_section(&app, 80);

        // Mode line should still be empty for Default mode (folder chip is no longer shown in mode line)
        let mode_line = &lines[1];
        assert_eq!(mode_line.spans.len(), 0, "Default mode should show empty line even with folder selected");
    }

    #[test]
    fn test_build_input_section_plan_mode_with_folder() {
        use crate::models::Folder;

        let mut app = App::default();
        app.permission_mode = PermissionMode::Plan;
        app.selected_folder = Some(Folder {
            name: "my-project".to_string(),
            path: "/path/to/project".to_string(),
        });

        let lines = build_input_section(&app, 80);

        // Mode line should show [PLAN] regardless of folder
        let mode_line = &lines[1];
        let mode_text: String = mode_line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(mode_text.contains("[PLAN]"), "Plan mode should display '[PLAN]' even with folder selected");
        // Folder chip should NOT appear in mode line
        assert!(!mode_text.contains("my-project"), "Folder chip should not appear in mode line");
    }

    #[test]
    fn test_build_input_section_multiline_input() {
        let mut app = App::default();
        app.textarea.set_content("First line\nSecond line\nThird line");

        let lines = build_input_section(&app, 80);

        // Should have more content lines for multiline input
        // Structure: blank, label, top border, 3 content lines, bottom border, keybinds, blank
        assert_eq!(lines.len(), 9);
    }

    #[test]
    fn test_build_input_section_borders_present() {
        let app = App::default();
        let lines = build_input_section(&app, 80);

        // Line 2 should be top border (full-width horizontal line, no corners)
        let top_border = &lines[2];
        let top_text: String = top_border.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(top_text.contains("─"), "Top border should contain horizontal line");
        assert!(!top_text.contains("┌"), "Top border should not have corner");
        assert!(!top_text.contains("┐"), "Top border should not have corner");

        // Find bottom border (full-width horizontal line, no corners)
        let bottom_border_idx = lines.len() - 3; // Before keybinds and blank
        let bottom_border = &lines[bottom_border_idx];
        let bottom_text: String = bottom_border.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(bottom_text.contains("─"), "Bottom border should contain horizontal line");
        assert!(!bottom_text.contains("└"), "Bottom border should not have corner");
        assert!(!bottom_text.contains("┘"), "Bottom border should not have corner");
    }

    #[test]
    fn test_build_input_section_keybind_hints() {
        let app = App::default();
        let lines = build_input_section(&app, 80);

        // Second to last line should be keybind hints
        let keybinds_line = &lines[lines.len() - 2];
        let keybinds_text: String = keybinds_line.spans.iter().map(|s| s.content.as_ref()).collect();

        // Should contain key keybind hints
        assert!(keybinds_text.contains("Enter"), "Should contain 'Enter'");
        assert!(keybinds_text.contains("send"), "Should contain 'send'");
        assert!(keybinds_text.contains("Shift+Enter"), "Should contain 'Shift+Enter'");
        assert!(keybinds_text.contains("newline"), "Should contain 'newline'");
        assert!(keybinds_text.contains("Esc"), "Should contain 'Esc'");
        assert!(keybinds_text.contains("menu"), "Should contain 'menu'");

        // Should use pipe separators between keybind hints
        assert!(keybinds_text.contains("|"), "Keybind hints should use '|' as separator");

        // Verify there are multiple pipe separators (at least 2 for 3 keybinds)
        let pipe_count = keybinds_text.matches('|').count();
        assert!(pipe_count >= 2, "Should have at least 2 pipe separators, found {}", pipe_count);
    }

    #[test]
    fn test_build_input_section_keybind_styling() {
        let app = App::default();
        let lines = build_input_section(&app, 80);

        // Second to last line should be keybind hints
        let keybinds_line = &lines[lines.len() - 2];

        // All spans in keybind hints should use COLOR_DIM
        for span in &keybinds_line.spans {
            if !span.content.trim().is_empty() {
                assert_eq!(
                    span.style.fg,
                    Some(COLOR_DIM),
                    "Keybind span '{}' should use COLOR_DIM",
                    span.content
                );
            }
        }
    }

    #[test]
    fn test_build_input_section_narrow_viewport() {
        let app = App::default();
        let lines = build_input_section(&app, 40);

        // Should still render without panicking on narrow width
        assert!(lines.len() >= 7);
    }
}
