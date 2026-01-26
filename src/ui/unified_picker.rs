//! Unified @ Picker Overlay rendering
//!
//! Implements the unified @ picker overlay for quick selection of:
//! - GitHub repositories (local and remote)
//! - Threads (conversation history)
//! - Folders (local directories)
//!
//! Shows filtered items organized by section with keyboard navigation.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

use crate::models::picker::{PickerItem, PickerSection};
use crate::state::UnifiedPickerState;

use super::theme::{COLOR_ACCENT, COLOR_BORDER, COLOR_DIALOG_BG, COLOR_DIM, COLOR_HEADER};

/// Maximum visible rows in the picker (across all sections)
const MAX_VISIBLE_ROWS: usize = 10;

/// Section header style
const SECTION_HEADER_STYLE: Style = Style::new();

/// Calculate dialog height based on content
fn calculate_dialog_height(total_lines: usize, area_height: u16) -> u16 {
    // Height: 2 (borders) + total_lines + 1 (hint line)
    let content_height = (total_lines as u16) + 3;

    // Cap at reasonable max based on terminal height
    let max_height = area_height.saturating_sub(6);
    content_height.min(max_height).max(5)
}

/// Get icon for a picker item type
fn item_icon(item: &PickerItem) -> &'static str {
    match item {
        PickerItem::Repo { local_path: Some(_), .. } => " ",  // Local repo
        PickerItem::Repo { local_path: None, .. } => "↓ ",     // Remote repo (needs clone)
        PickerItem::Thread { .. } => " ",                      // Thread/conversation
        PickerItem::Folder { .. } => " ",                      // Folder
    }
}

/// Get section header text
fn section_header(section: PickerSection) -> &'static str {
    match section {
        PickerSection::Repos => "REPOS",
        PickerSection::Threads => "THREADS",
        PickerSection::Folders => "FOLDERS",
    }
}

/// Render the unified picker dialog as a bottom-anchored overlay
///
/// # Arguments
/// * `frame` - The ratatui frame to render into
/// * `state` - The unified picker state
/// * `input_area` - The input field area to anchor the picker to
pub fn render_unified_picker(frame: &mut Frame, state: &UnifiedPickerState, input_area: Rect) {
    if !state.visible {
        return;
    }

    let area = frame.area();

    // Build all display lines
    let (all_lines, _total_items) = build_picker_lines(state, input_area.width as usize);

    if all_lines.is_empty() && !state.is_loading() {
        // Don't show empty picker if not loading
        return;
    }

    // Apply scroll offset - show only visible portion
    let scroll_offset = state.scroll_offset.min(all_lines.len().saturating_sub(1));
    let visible_lines: Vec<Line> = all_lines
        .into_iter()
        .skip(scroll_offset)
        .take(MAX_VISIBLE_ROWS)
        .collect();

    // Calculate dimensions based on visible lines
    let dialog_width = input_area.width;
    let line_count = visible_lines.len().min(MAX_VISIBLE_ROWS);
    let dialog_height = calculate_dialog_height(line_count.max(1), area.height);

    // Position: horizontally aligned with input area, bottom-anchored (above input area)
    let x = input_area.x;
    let y = input_area.y.saturating_sub(dialog_height);

    let dialog_area = Rect {
        x,
        y,
        width: dialog_width,
        height: dialog_height,
    };

    // Clear the background behind the dialog
    frame.render_widget(Clear, dialog_area);

    // Create the dialog border with solid background
    let title = if state.query.is_empty() {
        " Select Project ".to_string()
    } else {
        format!(" @{} ", state.query)
    };

    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default()
                .fg(COLOR_HEADER)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(COLOR_BORDER))
        .style(Style::default().bg(COLOR_DIALOG_BG));

    frame.render_widget(block, dialog_area);

    // Inner area for content
    let inner = Rect {
        x: dialog_area.x + 2,
        y: dialog_area.y + 1,
        width: dialog_area.width.saturating_sub(4),
        height: dialog_area.height.saturating_sub(2),
    };

    // Render visible lines
    let content = Paragraph::new(visible_lines).style(Style::default().bg(COLOR_DIALOG_BG));
    frame.render_widget(content, inner);
}

/// Build the display lines for the picker
///
/// Returns (lines, total_item_count)
fn build_picker_lines(state: &UnifiedPickerState, available_width: usize) -> (Vec<Line<'static>>, usize) {
    let mut lines: Vec<Line> = Vec::new();

    // Show clone progress if cloning
    if state.cloning {
        if let Some(ref msg) = state.clone_message {
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    msg.clone(),
                    Style::default().fg(COLOR_ACCENT).add_modifier(Modifier::ITALIC),
                ),
            ]));
            return (lines, 0);
        }
    }

    // Sections in display order
    let sections = [
        (PickerSection::Repos, &state.repos),
        (PickerSection::Threads, &state.threads),
        (PickerSection::Folders, &state.folders),
    ];

    for (section, section_state) in sections {
        // Skip empty sections (unless loading)
        if section_state.items.is_empty() && !section_state.loading {
            continue;
        }

        // Section header
        let header_style = SECTION_HEADER_STYLE
            .fg(COLOR_DIM)
            .add_modifier(Modifier::BOLD);

        let mut header_spans = vec![
            Span::styled("  ", Style::default()),
            Span::styled(section_header(section), header_style),
        ];

        // Show loading indicator
        if section_state.loading {
            header_spans.push(Span::styled(" ...", Style::default().fg(COLOR_DIM)));
        }

        lines.push(Line::from(header_spans));

        // Section items
        for (item_idx, item) in section_state.items.iter().enumerate() {
            let is_selected = state.selected_section == section && state.selected_index == item_idx;

            let line = render_item_line(item, is_selected, available_width);
            lines.push(line);
        }

        // Show error if any
        if let Some(ref error) = section_state.error {
            lines.push(Line::from(vec![
                Span::styled("    ", Style::default()),
                Span::styled(
                    format!("Error: {}", error),
                    Style::default().fg(ratatui::style::Color::Red),
                ),
            ]));
        }
    }

    // Show empty state
    if lines.is_empty() {
        if state.is_loading() {
            lines.push(Line::from(vec![Span::styled(
                "  Loading...",
                Style::default().fg(COLOR_DIM),
            )]));
        } else if !state.query.is_empty() {
            lines.push(Line::from(vec![Span::styled(
                format!("  No results for \"{}\"", state.query),
                Style::default().fg(COLOR_DIM),
            )]));
        } else {
            lines.push(Line::from(vec![Span::styled(
                "  No items available",
                Style::default().fg(COLOR_DIM),
            )]));
        }
    }

    // Hint line
    lines.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled("↑↓", Style::default().fg(COLOR_ACCENT)),
        Span::styled(": navigate  ", Style::default().fg(COLOR_DIM)),
        Span::styled("Enter", Style::default().fg(COLOR_ACCENT)),
        Span::styled(": select  ", Style::default().fg(COLOR_DIM)),
        Span::styled("Esc", Style::default().fg(COLOR_ACCENT)),
        Span::styled(": cancel", Style::default().fg(COLOR_DIM)),
    ]));

    let total_items = state.total_items();
    (lines, total_items)
}

/// Render a single item line
fn render_item_line(item: &PickerItem, is_selected: bool, available_width: usize) -> Line<'static> {
    let icon = item_icon(item);
    let name = item.display_name().to_string();

    // Selection marker
    let marker = if is_selected { "" } else { "  " };
    let marker_style = if is_selected {
        Style::default()
            .fg(COLOR_ACCENT)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(COLOR_DIM)
    };

    // Name style
    let name_style = if is_selected {
        Style::default()
            .fg(COLOR_HEADER)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(COLOR_ACCENT)
    };

    // Calculate space for secondary info (path for folders/repos, working_dir for threads)
    let marker_len = 2;
    let icon_len = 2;
    let name_len = name.chars().count();
    let separator_len = 2;
    let remaining = available_width.saturating_sub(marker_len + icon_len + name_len + separator_len + 4);

    let secondary_info = match item {
        PickerItem::Folder { path, .. } => Some(truncate_path(path, remaining)),
        PickerItem::Repo { local_path: Some(path), .. } => Some(truncate_path(path, remaining)),
        PickerItem::Repo { local_path: None, .. } => Some("(remote)".to_string()),
        PickerItem::Thread { working_directory: Some(dir), .. } => Some(truncate_path(dir, remaining)),
        PickerItem::Thread { working_directory: None, .. } => None,
    };

    let mut spans = vec![
        Span::styled(marker.to_string(), marker_style),
        Span::styled(icon.to_string(), Style::default().fg(COLOR_DIM)),
        Span::styled(name, name_style),
    ];

    if let Some(info) = secondary_info {
        spans.push(Span::styled("  ", Style::default()));
        spans.push(Span::styled(info, Style::default().fg(COLOR_DIM)));
    }

    Line::from(spans)
}

/// Truncate a path string to fit within max_len characters
fn truncate_path(path: &str, max_len: usize) -> String {
    if path.chars().count() <= max_len {
        return path.to_string();
    }

    if max_len < 5 {
        return "...".to_string();
    }

    // Show end of path with ellipsis
    let end_len = max_len.saturating_sub(3);
    let char_count = path.chars().count();
    let skip = char_count.saturating_sub(end_len);

    format!("...{}", path.chars().skip(skip).collect::<String>())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_item_icon_local_repo() {
        let item = PickerItem::Repo {
            name: "owner/repo".to_string(),
            local_path: Some("/path/to/repo".to_string()),
            url: "https://github.com/owner/repo".to_string(),
        };
        assert_eq!(item_icon(&item), " ");
    }

    #[test]
    fn test_item_icon_remote_repo() {
        let item = PickerItem::Repo {
            name: "owner/repo".to_string(),
            local_path: None,
            url: "https://github.com/owner/repo".to_string(),
        };
        assert_eq!(item_icon(&item), "↓ ");
    }

    #[test]
    fn test_item_icon_thread() {
        let item = PickerItem::Thread {
            id: "thread-1".to_string(),
            title: "Thread Title".to_string(),
            working_directory: None,
        };
        assert_eq!(item_icon(&item), " ");
    }

    #[test]
    fn test_item_icon_folder() {
        let item = PickerItem::Folder {
            name: "my-folder".to_string(),
            path: "/home/user/my-folder".to_string(),
        };
        assert_eq!(item_icon(&item), " ");
    }

    #[test]
    fn test_section_header() {
        assert_eq!(section_header(PickerSection::Repos), "REPOS");
        assert_eq!(section_header(PickerSection::Threads), "THREADS");
        assert_eq!(section_header(PickerSection::Folders), "FOLDERS");
    }

    #[test]
    fn test_truncate_path_short() {
        let path = "/short";
        assert_eq!(truncate_path(path, 20), "/short");
    }

    #[test]
    fn test_truncate_path_long() {
        let path = "/very/long/path/to/some/directory";
        let truncated = truncate_path(path, 20);
        assert!(truncated.starts_with("..."));
        assert!(truncated.chars().count() <= 20);
    }

    #[test]
    fn test_truncate_path_tiny() {
        let path = "/long/path";
        let truncated = truncate_path(path, 3);
        assert_eq!(truncated, "...");
    }

    #[test]
    fn test_calculate_dialog_height() {
        // Normal case
        let height = calculate_dialog_height(5, 40);
        assert_eq!(height, 8); // 5 + 3 (borders + hint)

        // Capped case
        let height = calculate_dialog_height(50, 20);
        assert!(height <= 14); // area - 6
    }

    #[test]
    fn test_build_picker_lines_empty() {
        let state = UnifiedPickerState::new();
        let (lines, count) = build_picker_lines(&state, 80);

        // Should have at least empty state message and hint
        assert!(!lines.is_empty());
        assert_eq!(count, 0);
    }

    #[test]
    fn test_build_picker_lines_with_items() {
        let mut state = UnifiedPickerState::new();
        state.visible = true;
        state.repos.items.push(PickerItem::Repo {
            name: "owner/repo".to_string(),
            local_path: None,
            url: "https://github.com/owner/repo".to_string(),
        });
        state.folders.items.push(PickerItem::Folder {
            name: "my-folder".to_string(),
            path: "/home/user/my-folder".to_string(),
        });

        let (lines, count) = build_picker_lines(&state, 80);

        // Should have section headers + items + hint
        assert!(lines.len() >= 4);
        assert_eq!(count, 2);
    }

    #[test]
    fn test_build_picker_lines_cloning() {
        let mut state = UnifiedPickerState::new();
        state.visible = true;
        state.cloning = true;
        state.clone_message = Some("Cloning owner/repo...".to_string());

        let (lines, _) = build_picker_lines(&state, 80);

        // Should only show clone message
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn test_render_item_line_selected() {
        let item = PickerItem::Folder {
            name: "my-project".to_string(),
            path: "/home/user/my-project".to_string(),
        };

        let line = render_item_line(&item, true, 80);

        // Should contain the item name
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("my-project"));
    }

    #[test]
    fn test_render_item_line_not_selected() {
        let item = PickerItem::Folder {
            name: "my-project".to_string(),
            path: "/home/user/my-project".to_string(),
        };

        let line = render_item_line(&item, false, 80);

        // Should contain the item name
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("my-project"));
    }
}
