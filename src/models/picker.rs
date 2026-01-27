use serde::Deserialize;

/// Represents a section in the unified @ picker
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerSection {
    Repos,
    Threads,
    Folders,
}

/// Unified picker item - can be a folder, repo, or thread
#[derive(Debug, Clone, PartialEq)]
pub enum PickerItem {
    Folder {
        name: String,
        path: String,
    },
    Repo {
        name: String,           // e.g., "owner/repo-name"
        local_path: Option<String>, // Some if cloned locally
        url: String,            // GitHub URL
    },
    Thread {
        id: String,
        title: String,
        working_directory: Option<String>,
    },
}

/// Response from /v1/search/folders
#[derive(Debug, Clone, Deserialize)]
pub struct SearchFoldersResponse {
    pub folders: Vec<FolderEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FolderEntry {
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub is_dir: bool,
}

/// Response from /v1/search/repos
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchReposResponse {
    pub repos: Vec<RepoEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoEntry {
    pub name_with_owner: String,
    pub url: String,
    /// Local path if repo is cloned (detected by conductor via .git check)
    #[serde(default)]
    pub local_path: Option<String>,
    // These fields are returned by conductor but not needed for picker
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub is_private: Option<bool>,
    #[serde(default)]
    pub pushed_at: Option<String>,
    #[serde(default)]
    pub is_fork: Option<bool>,
}

/// Response from /v1/search/threads
#[derive(Debug, Clone, Deserialize)]
pub struct SearchThreadsResponse {
    pub threads: Vec<ThreadEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThreadEntry {
    pub id: String,
    pub title: Option<String>,
    pub working_directory: Option<String>,
    // Extra fields returned by conductor
    #[serde(default, rename = "type")]
    pub thread_type: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub last_activity: Option<String>,
    #[serde(default)]
    pub message_count: Option<i64>,
}

/// Response from /v1/clone
#[derive(Debug, Clone, Deserialize)]
pub struct CloneResponse {
    pub path: String,
}

impl PickerItem {
    /// Get display name for the picker
    pub fn display_name(&self) -> &str {
        match self {
            PickerItem::Folder { name, .. } => name,
            PickerItem::Repo { name, .. } => name,
            PickerItem::Thread { title, id, .. } => {
                if title.is_empty() { id } else { title }
            }
        }
    }

    /// Check if this is a remote repo (not cloned locally)
    pub fn is_remote_repo(&self) -> bool {
        matches!(self, PickerItem::Repo { local_path: None, .. })
    }

    /// Get the working directory path (for folders and local repos)
    pub fn working_directory(&self) -> Option<&str> {
        match self {
            PickerItem::Folder { path, .. } => Some(path),
            PickerItem::Repo { local_path: Some(path), .. } => Some(path),
            PickerItem::Repo { local_path: None, .. } => None,
            PickerItem::Thread { working_directory, .. } => working_directory.as_deref(),
        }
    }

    /// Get the section this item belongs to
    pub fn section(&self) -> PickerSection {
        match self {
            PickerItem::Folder { .. } => PickerSection::Folders,
            PickerItem::Repo { .. } => PickerSection::Repos,
            PickerItem::Thread { .. } => PickerSection::Threads,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_picker_section_equality() {
        assert_eq!(PickerSection::Repos, PickerSection::Repos);
        assert_eq!(PickerSection::Threads, PickerSection::Threads);
        assert_eq!(PickerSection::Folders, PickerSection::Folders);

        assert_ne!(PickerSection::Repos, PickerSection::Threads);
        assert_ne!(PickerSection::Threads, PickerSection::Folders);
        assert_ne!(PickerSection::Folders, PickerSection::Repos);
    }

    #[test]
    fn test_picker_item_folder_display_name() {
        let folder = PickerItem::Folder {
            name: "my-project".to_string(),
            path: "/home/user/my-project".to_string(),
        };

        assert_eq!(folder.display_name(), "my-project");
    }

    #[test]
    fn test_picker_item_repo_display_name() {
        let repo = PickerItem::Repo {
            name: "owner/repo-name".to_string(),
            local_path: Some("/home/user/repos/repo-name".to_string()),
            url: "https://github.com/owner/repo-name".to_string(),
        };

        assert_eq!(repo.display_name(), "owner/repo-name");
    }

    #[test]
    fn test_picker_item_thread_display_name() {
        let thread_with_title = PickerItem::Thread {
            id: "thread-123".to_string(),
            title: "My Conversation".to_string(),
            working_directory: None,
        };

        assert_eq!(thread_with_title.display_name(), "My Conversation");

        let thread_without_title = PickerItem::Thread {
            id: "thread-456".to_string(),
            title: "".to_string(),
            working_directory: None,
        };

        assert_eq!(thread_without_title.display_name(), "thread-456");
    }

    #[test]
    fn test_picker_item_is_remote_repo() {
        let local_repo = PickerItem::Repo {
            name: "owner/local-repo".to_string(),
            local_path: Some("/home/user/local-repo".to_string()),
            url: "https://github.com/owner/local-repo".to_string(),
        };

        assert!(!local_repo.is_remote_repo());

        let remote_repo = PickerItem::Repo {
            name: "owner/remote-repo".to_string(),
            local_path: None,
            url: "https://github.com/owner/remote-repo".to_string(),
        };

        assert!(remote_repo.is_remote_repo());

        // Folders and threads are not repos
        let folder = PickerItem::Folder {
            name: "my-folder".to_string(),
            path: "/home/user/my-folder".to_string(),
        };

        assert!(!folder.is_remote_repo());

        let thread = PickerItem::Thread {
            id: "thread-789".to_string(),
            title: "Thread".to_string(),
            working_directory: None,
        };

        assert!(!thread.is_remote_repo());
    }

    #[test]
    fn test_picker_item_working_directory() {
        let folder = PickerItem::Folder {
            name: "my-project".to_string(),
            path: "/home/user/my-project".to_string(),
        };

        assert_eq!(folder.working_directory(), Some("/home/user/my-project"));

        let local_repo = PickerItem::Repo {
            name: "owner/local-repo".to_string(),
            local_path: Some("/home/user/repos/local-repo".to_string()),
            url: "https://github.com/owner/local-repo".to_string(),
        };

        assert_eq!(local_repo.working_directory(), Some("/home/user/repos/local-repo"));

        let remote_repo = PickerItem::Repo {
            name: "owner/remote-repo".to_string(),
            local_path: None,
            url: "https://github.com/owner/remote-repo".to_string(),
        };

        assert_eq!(remote_repo.working_directory(), None);

        let thread_with_dir = PickerItem::Thread {
            id: "thread-123".to_string(),
            title: "Thread".to_string(),
            working_directory: Some("/home/user/project".to_string()),
        };

        assert_eq!(thread_with_dir.working_directory(), Some("/home/user/project"));

        let thread_without_dir = PickerItem::Thread {
            id: "thread-456".to_string(),
            title: "Thread".to_string(),
            working_directory: None,
        };

        assert_eq!(thread_without_dir.working_directory(), None);
    }

    #[test]
    fn test_picker_item_section() {
        let folder = PickerItem::Folder {
            name: "my-folder".to_string(),
            path: "/home/user/my-folder".to_string(),
        };

        assert_eq!(folder.section(), PickerSection::Folders);

        let repo = PickerItem::Repo {
            name: "owner/repo".to_string(),
            local_path: None,
            url: "https://github.com/owner/repo".to_string(),
        };

        assert_eq!(repo.section(), PickerSection::Repos);

        let thread = PickerItem::Thread {
            id: "thread-123".to_string(),
            title: "Thread".to_string(),
            working_directory: None,
        };

        assert_eq!(thread.section(), PickerSection::Threads);
    }

    #[test]
    fn test_picker_item_clone() {
        let folder = PickerItem::Folder {
            name: "my-folder".to_string(),
            path: "/home/user/my-folder".to_string(),
        };

        let cloned = folder.clone();

        assert_eq!(folder, cloned);
    }

    #[test]
    fn test_search_folders_response_deserialize() {
        let json = r#"{
            "folders": [
                {"name": "project1", "path": "/home/user/project1"},
                {"name": "project2", "path": "/home/user/project2"}
            ]
        }"#;

        let response: SearchFoldersResponse = serde_json::from_str(json).expect("Failed to deserialize");

        assert_eq!(response.folders.len(), 2);
        assert_eq!(response.folders[0].name, "project1");
        assert_eq!(response.folders[0].path, "/home/user/project1");
        assert_eq!(response.folders[1].name, "project2");
        assert_eq!(response.folders[1].path, "/home/user/project2");
    }

    #[test]
    fn test_search_repos_response_deserialize() {
        // Test JSON format matching actual conductor response
        let json = r#"{
            "repos": [
                {
                    "nameWithOwner": "owner/repo1",
                    "url": "https://github.com/owner/repo1",
                    "description": "A great repo",
                    "isPrivate": false,
                    "pushedAt": "2025-01-15T10:30:00Z",
                    "isFork": false
                },
                {
                    "nameWithOwner": "owner/repo2",
                    "url": "https://github.com/owner/repo2"
                }
            ]
        }"#;

        let response: SearchReposResponse = serde_json::from_str(json).expect("Failed to deserialize");

        assert_eq!(response.repos.len(), 2);
        assert_eq!(response.repos[0].name_with_owner, "owner/repo1");
        assert_eq!(response.repos[0].url, "https://github.com/owner/repo1");
        assert_eq!(response.repos[0].description, Some("A great repo".to_string()));
        assert_eq!(response.repos[0].is_private, Some(false));
        assert_eq!(response.repos[1].name_with_owner, "owner/repo2");
        assert_eq!(response.repos[1].url, "https://github.com/owner/repo2");
        // Optional fields should be None when not present
        assert_eq!(response.repos[1].description, None);
    }

    #[test]
    fn test_search_threads_response_deserialize() {
        let json = r#"{
            "threads": [
                {
                    "id": "thread-123",
                    "title": "My Thread",
                    "working_directory": "/home/user/project"
                },
                {
                    "id": "thread-456",
                    "title": null,
                    "working_directory": null
                }
            ]
        }"#;

        let response: SearchThreadsResponse = serde_json::from_str(json).expect("Failed to deserialize");

        assert_eq!(response.threads.len(), 2);
        assert_eq!(response.threads[0].id, "thread-123");
        assert_eq!(response.threads[0].title, Some("My Thread".to_string()));
        assert_eq!(response.threads[0].working_directory, Some("/home/user/project".to_string()));
        assert_eq!(response.threads[1].id, "thread-456");
        assert_eq!(response.threads[1].title, None);
        assert_eq!(response.threads[1].working_directory, None);
    }

    #[test]
    fn test_clone_response_deserialize() {
        let json = r#"{"path": "/home/user/repos/new-repo"}"#;

        let response: CloneResponse = serde_json::from_str(json).expect("Failed to deserialize");

        assert_eq!(response.path, "/home/user/repos/new-repo");
    }
}
