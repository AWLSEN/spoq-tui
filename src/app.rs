use crate::state::{Task, Thread};
use crate::storage;
use crate::widgets::input_box::InputBox;
use color_eyre::Result;

/// Represents which UI component has focus
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Focus {
    Notifications,
    Tasks,
    #[default]
    Threads,
    Input,
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
    /// Current focus panel
    pub focus: Focus,
    /// Selected index in notifications panel
    pub notifications_index: usize,
    /// Selected index in tasks panel
    pub tasks_index: usize,
    /// Selected index in threads panel
    pub threads_index: usize,
    /// Input box state
    pub input_box: InputBox,
    /// Migration/indexing progress (0-100), None when complete
    pub migration_progress: Option<u8>,
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
            focus: Focus::default(),
            notifications_index: 0,
            tasks_index: 0,
            threads_index: 0,
            input_box: InputBox::new(),
            migration_progress: Some(0),
        })
    }

    /// Cycle focus to the next panel
    pub fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Notifications => Focus::Tasks,
            Focus::Tasks => Focus::Threads,
            Focus::Threads => Focus::Input,
            Focus::Input => Focus::Notifications,
        };
    }

    /// Move selection up in the current focused panel
    pub fn move_up(&mut self) {
        match self.focus {
            Focus::Notifications => {
                if self.notifications_index > 0 {
                    self.notifications_index -= 1;
                }
            }
            Focus::Tasks => {
                if self.tasks_index > 0 {
                    self.tasks_index -= 1;
                }
            }
            Focus::Threads => {
                if self.threads_index > 0 {
                    self.threads_index -= 1;
                }
            }
            Focus::Input => {}
        }
    }

    /// Move selection down in the current focused panel
    pub fn move_down(&mut self, max_notifications: usize, max_tasks: usize, max_threads: usize) {
        match self.focus {
            Focus::Notifications => {
                if max_notifications > 0 && self.notifications_index < max_notifications - 1 {
                    self.notifications_index += 1;
                }
            }
            Focus::Tasks => {
                if max_tasks > 0 && self.tasks_index < max_tasks - 1 {
                    self.tasks_index += 1;
                }
            }
            Focus::Threads => {
                if max_threads > 0 && self.threads_index < max_threads - 1 {
                    self.threads_index += 1;
                }
            }
            Focus::Input => {}
        }
    }

    /// Create a new thread placeholder
    pub fn create_new_thread(&mut self) {
        use crate::state::Thread;
        use chrono::Utc;

        let new_thread = Thread {
            id: format!("thread-{}", self.threads.len() + 1),
            title: "New Thread".to_string(),
            preview: "No messages yet...".to_string(),
            created_at: Utc::now(),
        };
        self.threads.insert(0, new_thread);
        self.threads_index = 0;
        self.focus = Focus::Threads;
    }

    /// Submit the current input (for now just clear it)
    pub fn submit_input(&mut self) {
        if !self.input_box.is_empty() {
            // For now, just clear the input
            // In future phases, this will send the message
            self.input_box.clear();
        }
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
