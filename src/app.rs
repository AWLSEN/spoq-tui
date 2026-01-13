use crate::state::{Notification, Task, Thread};
use crate::storage;
use color_eyre::Result;

/// Represents which UI component has focus
#[derive(Debug, Clone, PartialEq)]
pub enum Focus {
    Threads,
    Tasks,
    Input,
}

/// Main application state
#[derive(Debug)]
pub struct App {
    /// List of conversation threads
    pub threads: Vec<Thread>,
    /// List of tasks
    pub tasks: Vec<Task>,
    /// System notifications
    pub notifications: Vec<Notification>,
    /// Currently focused UI component
    pub focus: Focus,
    /// Input buffer for user text entry
    pub input: String,
    /// Migration progress (0.0 to 1.0)
    pub migration_progress: f32,
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
            notifications: Vec::new(),
            focus: Focus::Threads,
            input: String::new(),
            migration_progress: 0.0,
            should_quit: false,
        })
    }

    /// Add a new notification
    pub fn add_notification(&mut self, message: String) {
        self.notifications.push(Notification::new(message));
    }

    /// Add a new thread
    pub fn add_thread(&mut self, thread: Thread) {
        self.threads.push(thread);
    }

    /// Add a new task
    pub fn add_task(&mut self, task: Task) {
        self.tasks.push(task);
    }

    /// Save all data to storage
    pub fn save(&self) -> Result<()> {
        storage::save_threads(&self.threads)?;
        storage::save_tasks(&self.tasks)?;
        Ok(())
    }

    /// Set the focus to a specific component
    pub fn set_focus(&mut self, focus: Focus) {
        self.focus = focus;
    }

    /// Cycle focus to the next component
    pub fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Threads => Focus::Tasks,
            Focus::Tasks => Focus::Input,
            Focus::Input => Focus::Threads,
        };
    }

    /// Update migration progress
    pub fn set_migration_progress(&mut self, progress: f32) {
        self.migration_progress = progress.clamp(0.0, 1.0);
    }

    /// Mark the app to quit
    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    /// Clear the input buffer
    pub fn clear_input(&mut self) {
        self.input.clear();
    }

    /// Append a character to the input buffer
    pub fn input_push(&mut self, c: char) {
        self.input.push(c);
    }

    /// Remove the last character from the input buffer
    pub fn input_pop(&mut self) {
        self.input.pop();
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new().expect("Failed to create default App")
    }
}
