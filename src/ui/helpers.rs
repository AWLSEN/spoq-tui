//! Helper functions and constants for UI rendering
//!
//! Contains utility functions for formatting, truncation, and common UI patterns.
//!
//! Note: Layout-related functionality has been moved to the `layout` module.
//! See `LayoutContext` in `super::layout` for responsive sizing calculations.

use ratatui::layout::Rect;
use serde_json::Value;

/// Spinner frames for tool status animation
pub const SPINNER_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Maximum number of inline error banners to display
pub const MAX_VISIBLE_ERRORS: usize = 2;

/// Get inner rect with margin
pub fn inner_rect(area: Rect, margin: u16) -> Rect {
    Rect {
        x: area.x + margin,
        y: area.y + margin,
        width: area.width.saturating_sub(margin * 2),
        height: area.height.saturating_sub(margin * 2),
    }
}

/// Format token count in a human-readable way (e.g., 45000 -> "45k")
pub fn format_tokens(tokens: u32) -> String {
    if tokens >= 1_000_000 {
        format!("{}M", tokens / 1_000_000)
    } else if tokens >= 1_000 {
        format!("{}k", tokens / 1_000)
    } else {
        format!("{}", tokens)
    }
}

/// Extract a short model name from the full model string
/// Examples:
/// - "claude-opus-4-5-20250514" → "opus"
/// - "claude-sonnet-3-5" → "sonnet"
/// - "gpt-4" → "gpt"
pub fn extract_short_model_name(full_name: &str) -> &str {
    if full_name.contains("opus") {
        "opus"
    } else if full_name.contains("sonnet") {
        "sonnet"
    } else {
        full_name.split('-').next().unwrap_or(full_name)
    }
}

/// Truncate a string to approximately max_len bytes, adding "..." if truncated.
/// Safely handles UTF-8 by finding the nearest char boundary.
pub fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let target = max_len.saturating_sub(3);
        let end = find_char_boundary(s, target);
        format!("{}...", &s[..end])
    }
}

/// Find the nearest valid UTF-8 char boundary at or before the given byte index.
pub fn find_char_boundary(s: &str, index: usize) -> usize {
    if index >= s.len() {
        return s.len();
    }
    let mut end = index;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    end
}

/// Format tool arguments into a human-readable display string
///
/// Parses JSON arguments and extracts the most relevant field based on tool name.
/// Returns a concise description suitable for display in the UI.
///
/// # Examples
/// - Read {"file_path": "/src/main.rs"} -> "Reading /src/main.rs"
/// - Bash {"command": "npm install"} -> "Running: npm install"
/// - Grep {"pattern": "TODO", "path": "src/"} -> "Searching 'TODO' in src/"
pub fn format_tool_args(function_name: &str, args_json: &str) -> String {
    // Try to parse JSON, fall back to function name on failure
    let json: Value = match serde_json::from_str(args_json) {
        Ok(v) => v,
        Err(_) => return function_name.to_string(),
    };

    match function_name {
        "Read" => {
            if let Some(path) = json.get("file_path").and_then(|v| v.as_str()) {
                format!("Reading {}", truncate_string(path, 60))
            } else {
                "Read".to_string()
            }
        }
        "Write" => {
            if let Some(path) = json.get("file_path").and_then(|v| v.as_str()) {
                format!("Writing {}", truncate_string(path, 60))
            } else {
                "Write".to_string()
            }
        }
        "Edit" => {
            if let Some(path) = json.get("file_path").and_then(|v| v.as_str()) {
                format!("Editing {}", truncate_string(path, 60))
            } else {
                "Edit".to_string()
            }
        }
        "Grep" => {
            let pattern = json.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
            let path = json.get("path").and_then(|v| v.as_str());
            if let Some(p) = path {
                format!("Searching '{}' in {}", truncate_string(pattern, 30), truncate_string(p, 25))
            } else {
                format!("Searching '{}'", truncate_string(pattern, 40))
            }
        }
        "Glob" => {
            if let Some(pattern) = json.get("pattern").and_then(|v| v.as_str()) {
                format!("Finding {}", truncate_string(pattern, 50))
            } else {
                "Glob".to_string()
            }
        }
        "Bash" => {
            if let Some(cmd) = json.get("command").and_then(|v| v.as_str()) {
                format!("Running: {}", truncate_string(cmd, 50))
            } else {
                "Bash".to_string()
            }
        }
        "Task" => {
            if let Some(desc) = json.get("description").and_then(|v| v.as_str()) {
                format!("Spawning: {}", truncate_string(desc, 50))
            } else {
                "Task".to_string()
            }
        }
        "WebFetch" => {
            if let Some(url) = json.get("url").and_then(|v| v.as_str()) {
                format!("Fetching {}", truncate_string(url, 55))
            } else {
                "WebFetch".to_string()
            }
        }
        "WebSearch" => {
            if let Some(query) = json.get("query").and_then(|v| v.as_str()) {
                format!("Searching: {}", truncate_string(query, 50))
            } else {
                "WebSearch".to_string()
            }
        }
        "TodoWrite" => "Updating todos".to_string(),
        "NotebookEdit" => {
            if let Some(path) = json.get("notebook_path").and_then(|v| v.as_str()) {
                format!("Editing notebook {}", truncate_string(path, 45))
            } else {
                "NotebookEdit".to_string()
            }
        }
        _ => function_name.to_string(),
    }
}

