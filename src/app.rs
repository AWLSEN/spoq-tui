use crate::state::{Task, Thread};
use crate::storage;
use color_eyre::Result;

/// Represents which UI component has focus
/// Note: Planned for Phase 3 (UI/Display)
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub enum Focus {
    Threads,
}

/// Main application state
#[derive(Debug)]
pub struct App {
    /// List of conversation threads
    pub threads: Vec<Thread>,
    /// List of tasks
    pub tasks: Vec<Task>,
    /// Flag to track if the app should quit
    pub should_quit: bool,
}

impl App {
    /// Create a new App instance
    pub fn new() -> Result<Self> {
        // Initialize storage directories
        storage::init_storage()?;

        // Load existing data
        let threads = storage::load_threads().unwrap_or_default();
        let tasks = storage::load_tasks().unwrap_or_default();

        Ok(Self {
            threads,
            tasks,
            should_quit: false,
        })
    }

    /// Save all data to storage
    pub fn save(&self) -> Result<()> {
        storage::save_threads(&self.threads)?;
        storage::save_tasks(&self.tasks)?;
        Ok(())
    }

    /// Mark the app to quit
    pub fn quit(&mut self) {
        self.should_quit = true;
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new().expect("Failed to create default App")
    }
}
