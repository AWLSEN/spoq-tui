//! Navigation methods for the App.

use std::sync::Arc;

use super::{App, Focus, Screen};

impl App {
    /// Cycle focus to the next panel
    pub fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Notifications => Focus::Tasks,
            Focus::Tasks => Focus::Threads,
            Focus::Threads => Focus::Input,
            Focus::Input => Focus::Notifications,
        };
    }

    /// Cycle programming mode: PlanMode → BypassPermissions → None → PlanMode
    pub fn cycle_programming_mode(&mut self) {
        use super::ProgrammingMode;
        self.programming_mode = match self.programming_mode {
            ProgrammingMode::PlanMode => ProgrammingMode::BypassPermissions,
            ProgrammingMode::BypassPermissions => ProgrammingMode::None,
            ProgrammingMode::None => ProgrammingMode::PlanMode,
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

    /// Mark the app to quit
    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    /// Navigate back to the CommandDeck screen
    pub fn navigate_to_command_deck(&mut self) {
        self.screen = Screen::CommandDeck;
        self.active_thread_id = None; // Clear so next submit creates new thread
        self.input_box.clear(); // Clear any partial input
    }

    /// Open a specific thread by ID for conversation
    pub fn open_thread(&mut self, thread_id: String) {
        use super::AppMessage;

        // Set active thread and navigate (existing logic)
        self.active_thread_id = Some(thread_id.clone());
        self.screen = Screen::Conversation;
        self.input_box.clear();
        self.conversation_scroll = 0;

        // Check if messages need to be fetched
        if self.cache.get_messages(&thread_id).is_none() {
            // Spawn async fetch task
            let client = Arc::clone(&self.client);
            let message_tx = self.message_tx.clone();
            let tid = thread_id.clone();

            tokio::spawn(async move {
                match client.fetch_thread_with_messages(&tid).await {
                    Ok(response) => {
                        let messages: Vec<crate::models::Message> = response
                            .messages
                            .into_iter()
                            .enumerate()
                            .map(|(i, m)| m.to_client_message(&tid, i as i64 + 1))
                            .collect();
                        let _ = message_tx.send(AppMessage::MessagesLoaded {
                            thread_id: tid,
                            messages,
                        });
                    }
                    Err(e) => {
                        let _ = message_tx.send(AppMessage::MessagesLoadError {
                            thread_id: tid,
                            error: e.to_string(),
                        });
                    }
                }
            });
        }
    }

    /// Open the currently selected thread from the threads panel
    pub fn open_selected_thread(&mut self) {
        let threads = self.cache.threads();

        // Check if selection is beyond thread list (e.g., "New Thread" button)
        if self.threads_index >= threads.len() {
            // No valid thread selected, just focus input
            self.focus = Focus::Input;
            return;
        }

        let thread_id = threads[self.threads_index].id.clone();
        self.open_thread(thread_id);
    }
}
