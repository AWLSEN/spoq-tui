//! File model for the file picker in conversation threads.
//!
//! Represents files and directories from the /v1/files endpoint.

use serde::Deserialize;

/// A file or directory entry from the file listing API
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct FileEntry {
    /// File or directory name (e.g., "main.rs")
    pub name: String,
    /// Full path (e.g., "/Users/sam/project/src/main.rs")
    pub path: String,
    /// True for directories, false for files
    pub is_dir: bool,
    /// File size in bytes (None for directories)
    #[serde(default)]
    pub size: Option<u64>,
    /// Last modified timestamp as ISO 8601 string
    #[serde(default)]
    pub modified_at: Option<String>,
}

impl FileEntry {
    /// Get display name for the file
    pub fn display_name(&self) -> &str {
        &self.name
    }

    /// Format file size for display (e.g., "1.2 KB", "3.4 MB")
    pub fn format_size(&self) -> Option<String> {
        self.size.map(|bytes| {
            if bytes < 1024 {
                format!("{} B", bytes)
            } else if bytes < 1024 * 1024 {
                format!("{:.1} KB", bytes as f64 / 1024.0)
            } else if bytes < 1024 * 1024 * 1024 {
                format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
            } else {
                format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
            }
        })
    }

    /// Get relative path from a base directory
    pub fn relative_path(&self, base: &str) -> String {
        if self.path.starts_with(base) {
            let relative = self.path.trim_start_matches(base).trim_start_matches('/');
            if relative.is_empty() {
                self.name.clone()
            } else {
                relative.to_string()
            }
        } else {
            self.path.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_entry_deserialize() {
        let json = r#"{
            "name": "main.rs",
            "path": "/home/user/project/src/main.rs",
            "is_dir": false,
            "size": 1234,
            "modified_at": "2024-01-15T10:30:00+00:00"
        }"#;

        let entry: FileEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.name, "main.rs");
        assert_eq!(entry.path, "/home/user/project/src/main.rs");
        assert!(!entry.is_dir);
        assert_eq!(entry.size, Some(1234));
    }

    #[test]
    fn test_file_entry_deserialize_directory() {
        let json = r#"{
            "name": "src",
            "path": "/home/user/project/src",
            "is_dir": true
        }"#;

        let entry: FileEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.name, "src");
        assert!(entry.is_dir);
        assert!(entry.size.is_none());
    }

    #[test]
    fn test_format_size_bytes() {
        let entry = FileEntry {
            name: "small.txt".to_string(),
            path: "/small.txt".to_string(),
            is_dir: false,
            size: Some(512),
            modified_at: None,
        };
        assert_eq!(entry.format_size(), Some("512 B".to_string()));
    }

    #[test]
    fn test_format_size_kilobytes() {
        let entry = FileEntry {
            name: "medium.txt".to_string(),
            path: "/medium.txt".to_string(),
            is_dir: false,
            size: Some(2048),
            modified_at: None,
        };
        assert_eq!(entry.format_size(), Some("2.0 KB".to_string()));
    }

    #[test]
    fn test_format_size_megabytes() {
        let entry = FileEntry {
            name: "large.zip".to_string(),
            path: "/large.zip".to_string(),
            is_dir: false,
            size: Some(5 * 1024 * 1024),
            modified_at: None,
        };
        assert_eq!(entry.format_size(), Some("5.0 MB".to_string()));
    }

    #[test]
    fn test_format_size_none_for_directory() {
        let entry = FileEntry {
            name: "dir".to_string(),
            path: "/dir".to_string(),
            is_dir: true,
            size: None,
            modified_at: None,
        };
        assert!(entry.format_size().is_none());
    }

    #[test]
    fn test_relative_path() {
        let entry = FileEntry {
            name: "main.rs".to_string(),
            path: "/home/user/project/src/main.rs".to_string(),
            is_dir: false,
            size: Some(1000),
            modified_at: None,
        };

        assert_eq!(entry.relative_path("/home/user/project"), "src/main.rs");
        assert_eq!(entry.relative_path("/home/user/project/src"), "main.rs");
        assert_eq!(entry.relative_path("/other"), "/home/user/project/src/main.rs");
    }
}
