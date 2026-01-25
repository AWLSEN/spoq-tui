//! GitHub repository models for conductor API responses.

use serde::{Deserialize, Serialize};

/// GitHub repository from conductor API
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GitHubRepo {
    /// Full repository name (e.g., "owner/repo-name")
    pub name_with_owner: String,

    /// Repository description (may be null)
    pub description: Option<String>,

    /// Whether the repository is private
    pub is_private: bool,

    /// Last push timestamp (ISO 8601 format)
    pub pushed_at: String,

    /// Primary language of the repository
    pub primary_language: Option<PrimaryLanguage>,

    /// Whether this is a fork
    pub is_fork: bool,

    /// GitHub URL to the repository
    pub url: String,
}

/// Primary language information
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct PrimaryLanguage {
    /// Language name (e.g., "Rust", "TypeScript")
    pub name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_github_repo_deserialization() {
        let json = r#"{
            "nameWithOwner": "owner/my-repo",
            "description": "A cool project",
            "isPrivate": false,
            "pushedAt": "2024-01-15T10:30:00Z",
            "primaryLanguage": {"name": "Rust"},
            "isFork": false,
            "url": "https://github.com/owner/my-repo"
        }"#;

        let repo: GitHubRepo = serde_json::from_str(json).unwrap();
        assert_eq!(repo.name_with_owner, "owner/my-repo");
        assert_eq!(repo.description, Some("A cool project".to_string()));
        assert_eq!(repo.is_private, false);
        assert_eq!(repo.primary_language.as_ref().unwrap().name, "Rust");
    }

    #[test]
    fn test_github_repo_deserialization_null_fields() {
        let json = r#"{
            "nameWithOwner": "owner/my-repo",
            "description": null,
            "isPrivate": true,
            "pushedAt": "2024-01-15T10:30:00Z",
            "primaryLanguage": null,
            "isFork": true,
            "url": "https://github.com/owner/my-repo"
        }"#;

        let repo: GitHubRepo = serde_json::from_str(json).unwrap();
        assert_eq!(repo.name_with_owner, "owner/my-repo");
        assert!(repo.description.is_none());
        assert!(repo.is_private);
        assert!(repo.primary_language.is_none());
        assert!(repo.is_fork);
    }
}
