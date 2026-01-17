//! Helper functions and constants for UI rendering
//!
//! Contains utility functions for formatting, truncation, and common UI patterns.

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

/// Returns the appropriate Unicode icon for a given tool function name
pub fn get_tool_icon(function_name: &str) -> &'static str {
    match function_name {
        "Read" => "📄",
        "Write" => "📝",
        "Edit" => "✏️",
        "Bash" => "$",
        "Grep" => "🔍",
        "Glob" => "🔍",
        "Task" => "🤖",
        "WebFetch" => "🌐",
        "WebSearch" => "🌐",
        "TodoWrite" => "📋",
        "AskUserQuestion" => "❓",
        "NotebookEdit" => "📓",
        _ => "⚙️"
    }
}

/// Returns the appropriate icon for a subagent based on its type
///
/// # Arguments
/// * `subagent_type` - The type of subagent (e.g., "Explore", "Bash", "general-purpose")
///
/// # Returns
/// A static string with the icon character
pub fn get_subagent_icon(subagent_type: &str) -> &'static str {
    match subagent_type {
        "Explore" => "🔍",
        "Bash" => "$",
        "Plan" => "📋",
        "general-purpose" => "🤖",
        _ => "●"
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

// ============================================================================
// Responsive Layout Helpers
// ============================================================================

/// Layout context holding terminal dimensions for responsive calculations
#[derive(Debug, Clone, Copy)]
pub struct LayoutContext {
    /// Terminal width in columns
    pub width: u16,
    /// Terminal height in rows
    pub height: u16,
}

impl LayoutContext {
    /// Create a new layout context with the given dimensions
    pub fn new(width: u16, height: u16) -> Self {
        Self { width, height }
    }

    /// Create a layout context from a Rect
    pub fn from_rect(rect: Rect) -> Self {
        Self {
            width: rect.width,
            height: rect.height,
        }
    }

    /// Calculate a width as a percentage of terminal width
    ///
    /// # Arguments
    /// * `percentage` - Value between 0 and 100
    ///
    /// # Returns
    /// The calculated width in columns, minimum 1
    pub fn percent_width(&self, percentage: u16) -> u16 {
        ((self.width as u32 * percentage as u32) / 100).max(1) as u16
    }

    /// Calculate a height as a percentage of terminal height
    ///
    /// # Arguments
    /// * `percentage` - Value between 0 and 100
    ///
    /// # Returns
    /// The calculated height in rows, minimum 1
    pub fn percent_height(&self, percentage: u16) -> u16 {
        ((self.height as u32 * percentage as u32) / 100).max(1) as u16
    }

    /// Calculate proportional width with min/max bounds
    ///
    /// # Arguments
    /// * `percentage` - Base percentage (0-100)
    /// * `min` - Minimum width
    /// * `max` - Maximum width
    pub fn bounded_width(&self, percentage: u16, min: u16, max: u16) -> u16 {
        self.percent_width(percentage).clamp(min, max)
    }

    /// Calculate proportional height with min/max bounds
    ///
    /// # Arguments
    /// * `percentage` - Base percentage (0-100)
    /// * `min` - Minimum height
    /// * `max` - Maximum height
    pub fn bounded_height(&self, percentage: u16, min: u16, max: u16) -> u16 {
        self.percent_height(percentage).clamp(min, max)
    }

    /// Check if the terminal is in a "narrow" state (less than 80 columns)
    pub fn is_narrow(&self) -> bool {
        self.width < 80
    }

    /// Check if the terminal is in a "short" state (less than 24 rows)
    pub fn is_short(&self) -> bool {
        self.height < 24
    }

    /// Check if the terminal is in a "compact" state (narrow or short)
    pub fn is_compact(&self) -> bool {
        self.is_narrow() || self.is_short()
    }

    /// Get available content width after accounting for borders
    ///
    /// # Arguments
    /// * `border_width` - Total horizontal border width (default: 4 for left+right borders with padding)
    pub fn content_width(&self, border_width: u16) -> u16 {
        self.width.saturating_sub(border_width)
    }

    /// Get available content height after accounting for header/footer
    ///
    /// # Arguments
    /// * `chrome_height` - Total vertical space used by header/footer/borders
    pub fn content_height(&self, chrome_height: u16) -> u16 {
        self.height.saturating_sub(chrome_height)
    }
}

impl Default for LayoutContext {
    fn default() -> Self {
        Self {
            width: 80,
            height: 24,
        }
    }
}

/// Calculate responsive panel widths for a two-column layout
///
/// Returns (left_width, right_width) based on terminal width.
/// The left panel gets more space on wider terminals.
pub fn calculate_two_column_widths(total_width: u16) -> (u16, u16) {
    if total_width < 60 {
        // Very narrow: equal split
        let half = total_width / 2;
        (half, total_width - half)
    } else if total_width < 120 {
        // Medium: 40/60 split
        let left = (total_width * 40) / 100;
        (left, total_width - left)
    } else {
        // Wide: 35/65 split, with max left width
        let left = ((total_width * 35) / 100).min(60);
        (left, total_width - left)
    }
}

/// Calculate responsive panel heights for a stacked layout
///
/// Returns (top_height, bottom_height) based on terminal height.
pub fn calculate_stacked_heights(total_height: u16, input_rows: u16) -> (u16, u16) {
    let bottom = input_rows.min(total_height / 3); // Input area max 1/3 of height
    let top = total_height.saturating_sub(bottom);
    (top, bottom)
}
