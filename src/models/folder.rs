use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Folder {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
}
