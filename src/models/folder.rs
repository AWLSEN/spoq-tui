use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Folder {
    pub name: String,
    pub path: String,
}

/// Response wrapper for folder list endpoint
#[derive(Debug, Clone, Deserialize)]
pub struct FolderListResponse {
    pub folders: Vec<Folder>,
}
