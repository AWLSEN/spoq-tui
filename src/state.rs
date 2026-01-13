use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Represents a conversation thread
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thread {
    pub id: String,
    pub title: String,
    pub preview: String,
    pub created_at: DateTime<Utc>,
}

impl Thread {
    pub fn new(id: String, title: String, preview: String) -> Self {
        Self {
            id,
            title,
            preview,
            created_at: Utc::now(),
        }
    }
}

/// Task status enum
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
}

/// Represents a task in the system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub name: String,
    pub status: TaskStatus,
    pub progress: f32, // 0.0 to 1.0
}

impl Task {
    pub fn new(id: String, name: String) -> Self {
        Self {
            id,
            name,
            status: TaskStatus::Pending,
            progress: 0.0,
        }
    }

    pub fn with_status(mut self, status: TaskStatus) -> Self {
        self.status = status;
        self
    }

    pub fn with_progress(mut self, progress: f32) -> Self {
        self.progress = progress.clamp(0.0, 1.0);
        self
    }
}

/// Represents a system notification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub timestamp: DateTime<Utc>,
    pub message: String,
}

impl Notification {
    pub fn new(message: String) -> Self {
        Self {
            timestamp: Utc::now(),
            message,
        }
    }
}
