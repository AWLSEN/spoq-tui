//! Full-screen Browse List rendering
//!
//! Implements the full-screen list view for browsing threads and repos.
//! Accessible via /threads and /repos slash commands.
//!
//! Follows the same minimal aesthetic as the dashboard thread list.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::app::{App, BrowseListMode};

use super::layout::LayoutContext;
use super::theme::{COLOR_ACCENT, COLOR_DIM, COLOR_HEADER};

/// Thread list width as percentage of area width (matches dashboard)
const LIST_WIDTH_PERCENT: f32 = 0.915;

/// Maximum items to load (API doesn't support offset pagination)
pub const MAX_ITEMS: usize = 50;

/// Lines per item (name + path + blank line spacing)
const LINES_PER_ITEM: usize = 3;

/// Debounce delay for search in milliseconds
pub const SEARCH_DEBOUNCE_MS: u64 = 300;

/// Format a relative time string from ISO timestamp
fn format_relative_time(timestamp: &Option<String>) -> String {
    let Some(ts) = timestamp else {
        return String::new();
    };

    let Ok(dt) = chrono::DateTime::parse_from_rfc3339(ts) else {
        return String::new();
    };

    let now = chrono::Utc::now();
    let duration = now.signed_duration_since(dt);

    if duration.num_seconds() < 60 {
        "now".to_string()
    } else if duration.num_minutes() < 60 {
        format!("{}m", duration.num_minutes())
    } else if duration.num_hours() < 24 {
        format!("{}h", duration.num_hours())
    } else if duration.num_days() < 30 {
        format!("{}d", duration.num_days())
    } else {
        format!("{}mo", duration.num_days() / 30)
    }
}

/// Truncate string to fit width, adding "..." if needed
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len > 3 {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    } else {
        s[..max_len].to_string()
    }
}

/// Render the full-screen browse list
pub fn render_browse_list(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    let _ctx = LayoutContext::new(app.terminal_width, app.terminal_height);

    // Layout: header (1 row) + search (1 row) + margin + content
    let margin_rows = ((area.height as f32 * 0.04).round() as u16).max(1);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),            // Header (esc | title | count)
            Constraint::Length(1),            // Spacing
            Constraint::Length(1),            // Search input
            Constraint::Length(margin_rows),  // Margin
            Constraint::Min(5),               // List content
        ])
        .split(area);

    render_header(frame, chunks[0], app);
    render_search_input(frame, chunks[2], app);
    render_list_content(frame, chunks[4], app);
}

/// Render the header: esc (left) | title (center) | count (right)
fn render_header(frame: &mut Frame, area: Rect, app: &App) {
    let centered_area = calculate_centered_area(area);

    // Title in center
    let title = match app.browse_list.mode {
        BrowseListMode::Threads => "threads",
        BrowseListMode::Repos => "repos",
    };

    // Count on right
    let count = match app.browse_list.mode {
        BrowseListMode::Threads => app.browse_list.threads.len(),
        BrowseListMode::Repos => app.browse_list.repos.len(),
    };
    let count_text = count.to_string();

    // Calculate positions
    let esc_text = "esc";
    let esc_width = esc_text.len();
    let count_width = count_text.len();
    let title_width = title.len();

    // Center the title
    let available_center = centered_area.width as usize - esc_width - count_width - 4;
    let center_start = esc_width + 2 + (available_center.saturating_sub(title_width)) / 2;

    // Render esc on left
    let esc_span = Span::styled(esc_text, Style::default().fg(COLOR_DIM));
    frame.render_widget(
        Paragraph::new(Line::from(vec![esc_span])),
        Rect::new(centered_area.x, centered_area.y, esc_width as u16, 1),
    );

    // Render title in center
    let title_span = Span::styled(title, Style::default().fg(COLOR_HEADER).add_modifier(Modifier::BOLD));
    frame.render_widget(
        Paragraph::new(Line::from(vec![title_span])),
        Rect::new(centered_area.x + center_start as u16, centered_area.y, title_width as u16, 1),
    );

    // Render count on right
    let count_span = Span::styled(&count_text, Style::default().fg(COLOR_DIM));
    frame.render_widget(
        Paragraph::new(Line::from(vec![count_span])),
        Rect::new(centered_area.x + centered_area.width - count_width as u16, centered_area.y, count_width as u16, 1),
    );
}

/// Render the search input: "search: " + query/placeholder + searching indicator
fn render_search_input(frame: &mut Frame, area: Rect, app: &App) {
    let centered_area = calculate_centered_area(area);

    let prefix = "search: ";

    let mut spans = vec![
        Span::styled(prefix, Style::default().fg(COLOR_DIM)),
    ];

    if app.browse_list.search_query.is_empty() {
        // Show placeholder
        spans.push(Span::styled("type to search", Style::default().fg(COLOR_DIM)));
    } else {
        // Show query with cursor
        spans.push(Span::styled(&app.browse_list.search_query, Style::default().fg(Color::White)));
        spans.push(Span::styled("_", Style::default().fg(COLOR_ACCENT).add_modifier(Modifier::SLOW_BLINK)));
    }

    let search_line = Line::from(spans);
    frame.render_widget(Paragraph::new(search_line), Rect::new(centered_area.x, centered_area.y, centered_area.width, 1));

    // Show "searching..." on the right if searching
    if app.browse_list.searching {
        let indicator = "searching...";
        let indicator_width = indicator.len() as u16;
        let indicator_span = Span::styled(indicator, Style::default().fg(COLOR_DIM));
        frame.render_widget(
            Paragraph::new(Line::from(vec![indicator_span])),
            Rect::new(centered_area.x + centered_area.width - indicator_width, centered_area.y, indicator_width, 1),
        );
    }
}

/// Render the main list content
fn render_list_content(frame: &mut Frame, area: Rect, app: &App) {
    let centered_area = calculate_centered_area(area);

    // Handle cloning state
    if app.browse_list.cloning {
        if let Some(ref msg) = app.browse_list.clone_message {
            let clone_line = Line::from(vec![
                Span::styled(msg, Style::default().fg(COLOR_ACCENT)),
            ]);
            frame.render_widget(Paragraph::new(clone_line), centered_area);
            return;
        }
    }

    // Handle loading state (initial load, not search)
    if app.browse_list.loading && !app.browse_list.searching {
        let loading_line = Line::from(vec![
            Span::styled("loading...", Style::default().fg(COLOR_DIM)),
        ]);
        frame.render_widget(Paragraph::new(loading_line), centered_area);
        return;
    }

    // Handle empty state
    let items_count = match app.browse_list.mode {
        BrowseListMode::Threads => app.browse_list.threads.len(),
        BrowseListMode::Repos => app.browse_list.repos.len(),
    };

    if items_count == 0 {
        let empty_msg = if !app.browse_list.search_query.is_empty() {
            "no results"
        } else {
            match app.browse_list.mode {
                BrowseListMode::Threads => "no threads",
                BrowseListMode::Repos => "no repos",
            }
        };
        let empty_line = Line::from(vec![
            Span::styled(empty_msg, Style::default().fg(COLOR_DIM)),
        ]);
        frame.render_widget(Paragraph::new(empty_line), centered_area);
        return;
    }

    // Calculate how many items fit (each item takes LINES_PER_ITEM rows)
    let visible_items = (centered_area.height as usize) / LINES_PER_ITEM;
    let scroll_offset = app.browse_list.scroll_offset;
    let selected_index = app.browse_list.selected_index;

    match app.browse_list.mode {
        BrowseListMode::Threads => {
            for (display_idx, (i, thread)) in app.browse_list.threads.iter().enumerate().skip(scroll_offset).take(visible_items).enumerate() {
                let is_selected = i == selected_index;
                let row_y = centered_area.y + (display_idx * LINES_PER_ITEM) as u16;

                render_thread_item(frame, centered_area.x, row_y, centered_area.width, thread, is_selected);
            }
        }
        BrowseListMode::Repos => {
            for (display_idx, (i, repo)) in app.browse_list.repos.iter().enumerate().skip(scroll_offset).take(visible_items).enumerate() {
                let is_selected = i == selected_index;
                let row_y = centered_area.y + (display_idx * LINES_PER_ITEM) as u16;

                render_repo_item(frame, centered_area.x, row_y, centered_area.width, repo, is_selected);
            }
        }
    }
}

/// Render a single thread item (2 lines + spacing)
/// Line 1: > Title                                    2h
/// Line 2:   ~/path/to/directory
fn render_thread_item(frame: &mut Frame, x: u16, y: u16, width: u16, thread: &crate::models::picker::ThreadEntry, is_selected: bool) {
    let content_width = (width as usize).saturating_sub(2); // Account for "> " prefix

    // Title
    let title = thread.title.clone().unwrap_or_else(|| {
        if thread.id.len() > 8 {
            format!("thread {}", &thread.id[..8])
        } else {
            format!("thread {}", thread.id)
        }
    });

    // Time
    let time_text = format_relative_time(&thread.last_activity);
    let time_width = time_text.len();

    // Truncate title to fit with time
    let title_max = content_width.saturating_sub(time_width + 2);
    let title_text = truncate(&title, title_max);

    // Build title line
    let title_style = if is_selected {
        Style::default().fg(COLOR_HEADER).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    let time_style = Style::default().fg(COLOR_DIM);

    let prefix = if is_selected { "> " } else { "  " };
    let prefix_style = if is_selected {
        Style::default().fg(COLOR_ACCENT)
    } else {
        Style::default()
    };

    // Calculate padding between title and time
    let padding_len = content_width.saturating_sub(title_text.len() + time_width);
    let padding = " ".repeat(padding_len);

    let title_line = Line::from(vec![
        Span::styled(prefix, prefix_style),
        Span::styled(title_text, title_style),
        Span::raw(padding),
        Span::styled(time_text, time_style),
    ]);
    frame.render_widget(Paragraph::new(title_line), Rect::new(x, y, width, 1));

    // Directory line
    let dir = thread.working_directory.as_deref().unwrap_or("");
    let dir_display = if dir.starts_with("/Users/") || dir.starts_with("/home/") {
        format!("~/{}", dir.split('/').skip(3).collect::<Vec<_>>().join("/"))
    } else {
        dir.to_string()
    };
    let dir_text = truncate(&dir_display, content_width);
    let dir_style = Style::default().fg(COLOR_DIM);

    let dir_line = Line::from(vec![
        Span::raw("  "), // Same indent as prefix
        Span::styled(dir_text, dir_style),
    ]);
    frame.render_widget(Paragraph::new(dir_line), Rect::new(x, y + 1, width, 1));

    // Line 3 is blank (spacing) - no need to render
}

/// Render a single repo item (2 lines + spacing)
/// Line 1: > owner/repo                              local
/// Line 2:   ~/path/to/repo
fn render_repo_item(frame: &mut Frame, x: u16, y: u16, width: u16, repo: &crate::models::picker::RepoEntry, is_selected: bool) {
    let content_width = (width as usize).saturating_sub(2); // Account for "> " prefix

    // Status
    let status_text = if repo.local_path.is_some() { "local" } else { "remote" };
    let status_width = status_text.len();

    // Truncate name to fit with status
    let name_max = content_width.saturating_sub(status_width + 2);
    let name_text = truncate(&repo.name_with_owner, name_max);

    // Build name line
    let name_style = if is_selected {
        Style::default().fg(COLOR_HEADER).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    let status_style = if repo.local_path.is_some() {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(COLOR_DIM)
    };

    let prefix = if is_selected { "> " } else { "  " };
    let prefix_style = if is_selected {
        Style::default().fg(COLOR_ACCENT)
    } else {
        Style::default()
    };

    // Calculate padding between name and status
    let padding_len = content_width.saturating_sub(name_text.len() + status_width);
    let padding = " ".repeat(padding_len);

    let name_line = Line::from(vec![
        Span::styled(prefix, prefix_style),
        Span::styled(name_text, name_style),
        Span::raw(padding),
        Span::styled(status_text, status_style),
    ]);
    frame.render_widget(Paragraph::new(name_line), Rect::new(x, y, width, 1));

    // Path line
    let path = repo.local_path.as_ref().map(|p| {
        if p.starts_with("/Users/") || p.starts_with("/home/") {
            format!("~/{}", p.split('/').skip(3).collect::<Vec<_>>().join("/"))
        } else {
            p.clone()
        }
    }).unwrap_or_else(|| repo.url.clone());
    let path_text = truncate(&path, content_width);
    let path_style = Style::default().fg(COLOR_DIM);

    let path_line = Line::from(vec![
        Span::raw("  "), // Same indent as prefix
        Span::styled(path_text, path_style),
    ]);
    frame.render_widget(Paragraph::new(path_line), Rect::new(x, y + 1, width, 1));

    // Line 3 is blank (spacing) - no need to render
}

/// Calculate a horizontally centered area (matches dashboard thread_list)
fn calculate_centered_area(area: Rect) -> Rect {
    let card_width = (area.width as f32 * LIST_WIDTH_PERCENT).round() as u16;
    let left_padding = (area.width - card_width) / 2;

    Rect::new(area.x + left_padding, area.y, card_width, area.height)
}
