//! Input history management for the Spoq TUI.
//!
//! This module provides functionality for storing submitted inputs,
//! navigating through history, and persisting to `~/.spoq_history`.

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

/// The default history file path.
const HISTORY_FILE: &str = ".spoq_history";

/// Maximum number of history entries to keep.
const MAX_HISTORY_SIZE: usize = 1000;

/// Manages input history for the application.
#[derive(Debug, Clone)]
pub struct InputHistory {
    /// Stored history entries (oldest first, newest last).
    entries: Vec<String>,
    /// Current position when navigating (None when at bottom/new input).
    index: Option<usize>,
    /// Saves what user was typing before navigating history.
    current_input: String,
}

impl Default for InputHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl InputHistory {
    /// Create a new empty history.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: None,
            current_input: String::new(),
        }
    }

    /// Get the path to the history file.
    fn history_path() -> Option<PathBuf> {
        dirs::home_dir().map(|home| home.join(HISTORY_FILE))
    }

    /// Load history from `~/.spoq_history` file (create if doesn't exist).
    pub fn load() -> Self {
        let Some(path) = Self::history_path() else {
            // No home directory, return empty history
            return Self::new();
        };

        // Create file if it doesn't exist
        if !path.exists() {
            if let Ok(mut file) = File::create(&path) {
                // Write empty file
                let _ = file.flush();
            }
            return Self::new();
        }

        // Read entries from file
        let file = match File::open(&path) {
            Ok(f) => f,
            Err(_) => return Self::new(),
        };

        let reader = BufReader::new(file);
        let entries: Vec<String> = reader
            .lines()
            .map_while(Result::ok)
            .filter(|line| !line.is_empty())
            .collect();

        Self {
            entries,
            index: None,
            current_input: String::new(),
        }
    }

    /// Persist history to file.
    pub fn save(&self) {
        let Some(path) = Self::history_path() else {
            return;
        };

        let file = match OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
        {
            Ok(f) => f,
            Err(_) => return,
        };

        let mut writer = std::io::BufWriter::new(file);
        for entry in &self.entries {
            // Replace newlines with a special marker for multi-line entries
            let escaped = entry.replace('\n', "\\n");
            let _ = writeln!(writer, "{}", escaped);
        }
        let _ = writer.flush();
    }

    /// Add a new entry to history (at the end).
    ///
    /// Skips empty entries and duplicates of the last entry.
    /// Trims to MAX_HISTORY_SIZE if necessary.
    pub fn add(&mut self, entry: String) {
        // Skip empty entries
        if entry.trim().is_empty() {
            return;
        }

        // Unescape if needed (for entries loaded from file)
        let unescaped = entry.replace("\\n", "\n");

        // Skip if it's a duplicate of the last entry
        if self.entries.last() == Some(&unescaped) {
            return;
        }

        self.entries.push(unescaped);

        // Trim to max size (remove oldest entries)
        while self.entries.len() > MAX_HISTORY_SIZE {
            self.entries.remove(0);
        }

        // Reset navigation state after adding
        self.reset_navigation();
    }

    /// Navigate to older entry (up arrow).
    ///
    /// On first call, saves current input and moves to most recent history entry.
    /// On subsequent calls, moves to older entries.
    /// Returns the entry to display, or None if at oldest entry.
    pub fn navigate_up(&mut self, current: &str) -> Option<&str> {
        if self.entries.is_empty() {
            return None;
        }

        match self.index {
            None => {
                // First navigation: save current input and go to most recent
                self.current_input = current.to_string();
                let last_idx = self.entries.len() - 1;
                self.index = Some(last_idx);
                Some(&self.entries[last_idx])
            }
            Some(idx) => {
                if idx > 0 {
                    // Move to older entry
                    self.index = Some(idx - 1);
                    Some(&self.entries[idx - 1])
                } else {
                    // Already at oldest entry
                    Some(&self.entries[0])
                }
            }
        }
    }

    /// Navigate to newer entry (down arrow).
    ///
    /// Returns the entry to display, or None if at bottom (current input).
    pub fn navigate_down(&mut self) -> Option<&str> {
        match self.index {
            None => {
                // Already at bottom
                None
            }
            Some(idx) => {
                if idx + 1 < self.entries.len() {
                    // Move to newer entry
                    self.index = Some(idx + 1);
                    Some(&self.entries[idx + 1])
                } else {
                    // At most recent entry, go back to current input
                    self.index = None;
                    None
                }
            }
        }
    }

    /// Reset navigation index to None (called after submit).
    pub fn reset_navigation(&mut self) {
        self.index = None;
        self.current_input.clear();
    }

    /// Get the saved current input (what user was typing before navigating).
    pub fn get_current_input(&self) -> &str {
        &self.current_input
    }

    /// Get the number of entries in history.
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if history is empty.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get current navigation index (for testing).
    #[allow(dead_code)]
    pub fn current_index(&self) -> Option<usize> {
        self.index
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_creates_empty_history() {
        let history = InputHistory::new();
        assert!(history.is_empty());
        assert_eq!(history.len(), 0);
        assert!(history.index.is_none());
        assert!(history.current_input.is_empty());
    }

    #[test]
    fn test_add_entry() {
        let mut history = InputHistory::new();
        history.add("first entry".to_string());
        assert_eq!(history.len(), 1);
        history.add("second entry".to_string());
        assert_eq!(history.len(), 2);
    }

    #[test]
    fn test_add_skips_empty() {
        let mut history = InputHistory::new();
        history.add("".to_string());
        assert!(history.is_empty());
        history.add("   ".to_string());
        assert!(history.is_empty());
    }

    #[test]
    fn test_add_skips_duplicates() {
        let mut history = InputHistory::new();
        history.add("same".to_string());
        history.add("same".to_string());
        assert_eq!(history.len(), 1);
    }

    #[test]
    fn test_navigate_up_from_empty() {
        let mut history = InputHistory::new();
        assert!(history.navigate_up("current").is_none());
    }

    #[test]
    fn test_navigate_up_saves_current() {
        let mut history = InputHistory::new();
        history.add("old entry".to_string());

        let _ = history.navigate_up("my current text");
        assert_eq!(history.get_current_input(), "my current text");
    }

    #[test]
    fn test_navigate_up_returns_most_recent_first() {
        let mut history = InputHistory::new();
        history.add("oldest".to_string());
        history.add("middle".to_string());
        history.add("newest".to_string());

        let result = history.navigate_up("current");
        assert_eq!(result, Some("newest"));
    }

    #[test]
    fn test_navigate_up_through_history() {
        let mut history = InputHistory::new();
        history.add("first".to_string());
        history.add("second".to_string());
        history.add("third".to_string());

        assert_eq!(history.navigate_up("current"), Some("third"));
        assert_eq!(history.navigate_up("current"), Some("second"));
        assert_eq!(history.navigate_up("current"), Some("first"));
        // Stay at oldest
        assert_eq!(history.navigate_up("current"), Some("first"));
    }

    #[test]
    fn test_navigate_down_from_bottom() {
        let mut history = InputHistory::new();
        history.add("entry".to_string());

        // When at bottom (index is None), navigate_down returns None
        assert!(history.navigate_down().is_none());
    }

    #[test]
    fn test_navigate_down_returns_to_current() {
        let mut history = InputHistory::new();
        history.add("first".to_string());
        history.add("second".to_string());

        // Navigate up twice
        history.navigate_up("my text");
        history.navigate_up("my text");

        // Navigate down
        assert_eq!(history.navigate_down(), Some("second"));

        // Navigate down again returns to current input (None)
        assert!(history.navigate_down().is_none());
    }

    #[test]
    fn test_reset_navigation() {
        let mut history = InputHistory::new();
        history.add("entry".to_string());

        history.navigate_up("current text");
        assert!(history.index.is_some());
        assert!(!history.current_input.is_empty());

        history.reset_navigation();
        assert!(history.index.is_none());
        assert!(history.current_input.is_empty());
    }

    #[test]
    fn test_navigate_up_down_cycle() {
        let mut history = InputHistory::new();
        history.add("a".to_string());
        history.add("b".to_string());
        history.add("c".to_string());

        // Navigate up through all
        assert_eq!(history.navigate_up("z"), Some("c"));
        assert_eq!(history.navigate_up("z"), Some("b"));
        assert_eq!(history.navigate_up("z"), Some("a"));

        // Navigate back down
        assert_eq!(history.navigate_down(), Some("b"));
        assert_eq!(history.navigate_down(), Some("c"));

        // Back to current input
        assert!(history.navigate_down().is_none());

        // Current input should be preserved
        assert_eq!(history.get_current_input(), "z");
    }

    #[test]
    fn test_multiline_entry() {
        let mut history = InputHistory::new();
        let multiline = "line 1\nline 2\nline 3".to_string();
        history.add(multiline.clone());

        let result = history.navigate_up("").unwrap();
        assert_eq!(result, &multiline);
    }

    #[test]
    fn test_max_history_size() {
        let mut history = InputHistory::new();

        // Add more than MAX_HISTORY_SIZE entries
        for i in 0..MAX_HISTORY_SIZE + 100 {
            history.add(format!("entry {}", i));
        }

        assert_eq!(history.len(), MAX_HISTORY_SIZE);

        // Oldest entries should be removed
        assert_eq!(history.entries[0], "entry 100");
    }

    #[test]
    fn test_default_trait() {
        let history: InputHistory = Default::default();
        assert!(history.is_empty());
    }
}
