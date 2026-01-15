//! Permission handling methods for the App.

use std::sync::Arc;

use super::App;

impl App {
    /// Approve the current pending permission (user pressed 'y')
    pub fn approve_permission(&mut self, permission_id: &str) {
        // Send approval to backend (spawns async task if runtime available)
        // This check allows unit tests to run without a Tokio runtime
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let client = Arc::clone(&self.client);
            let perm_id = permission_id.to_string();
            handle.spawn(async move {
                let _ = client.respond_to_permission(&perm_id, true).await;
            });
        }

        // Clear the pending permission
        self.session_state.clear_pending_permission();
    }

    /// Deny the current pending permission (user pressed 'n')
    pub fn deny_permission(&mut self, permission_id: &str) {
        // Send denial to backend (spawns async task if runtime available)
        // This check allows unit tests to run without a Tokio runtime
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let client = Arc::clone(&self.client);
            let perm_id = permission_id.to_string();
            handle.spawn(async move {
                let _ = client.respond_to_permission(&perm_id, false).await;
            });
        }

        // Clear the pending permission
        self.session_state.clear_pending_permission();
    }

    /// Allow the tool always for this session and approve (user pressed 'a')
    pub fn allow_tool_always(&mut self, tool_name: &str, permission_id: &str) {
        // Add tool to allowed list
        self.session_state.allow_tool(tool_name.to_string());

        // Approve the current permission
        self.approve_permission(permission_id);
    }

    /// Handle a permission response key press ('y', 'a', or 'n')
    /// Returns true if a permission was handled, false if no pending permission
    pub fn handle_permission_key(&mut self, key: char) -> bool {
        if let Some(ref perm) = self.session_state.pending_permission.clone() {
            match key {
                'y' | 'Y' => {
                    // Allow once
                    self.approve_permission(&perm.permission_id);
                    true
                }
                'a' | 'A' => {
                    // Allow always
                    self.allow_tool_always(&perm.tool_name, &perm.permission_id);
                    true
                }
                'n' | 'N' => {
                    // Deny
                    self.deny_permission(&perm.permission_id);
                    true
                }
                _ => false,
            }
        } else {
            false
        }
    }
}
