//! Click action handler for the touch interaction system.
//!
//! This module processes click actions dispatched from the hit area registry,
//! translating them into App state mutations.

use super::hit_area::ClickAction;
use crate::app::App;
use crate::ui::dashboard::FilterState;

/// Handle a click action by updating App state.
///
/// This function is called from the event loop when a mouse click lands on
/// a registered hit area. It dispatches to the appropriate App methods based
/// on the action type.
pub fn handle_click_action(app: &mut App, action: ClickAction) {
    // Mark the app as dirty since any click action likely changes state
    app.mark_dirty();

    match action {
        // =====================================================================
        // Filter Actions (CommandDeck dashboard filters)
        // =====================================================================
        ClickAction::FilterWorking => {
            app.dashboard.toggle_filter(FilterState::Working);
            tracing::debug!("Click: FilterWorking - toggled filter");
        }
        ClickAction::FilterReadyToTest => {
            app.dashboard.toggle_filter(FilterState::ReadyToTest);
            tracing::debug!("Click: FilterReadyToTest - toggled filter");
        }
        ClickAction::FilterIdle => {
            app.dashboard.toggle_filter(FilterState::Idle);
            tracing::debug!("Click: FilterIdle - toggled filter");
        }
        ClickAction::ClearFilter => {
            app.dashboard.clear_filter();
            tracing::debug!("Click: ClearFilter - filter cleared");
        }

        // =====================================================================
        // Overlay Actions (Expanded thread overlay)
        // =====================================================================
        ClickAction::ExpandThread {
            thread_id,
            anchor_y,
        } => {
            app.dashboard.expand_thread(&thread_id, anchor_y);
            tracing::debug!(
                "Click: ExpandThread(thread_id={}, anchor_y={})",
                thread_id,
                anchor_y
            );
        }
        ClickAction::CollapseOverlay => {
            app.dashboard.collapse_overlay();
            tracing::debug!("Click: CollapseOverlay - overlay closed");
        }

        // =====================================================================
        // Thread Action Buttons
        // =====================================================================
        ClickAction::ApproveThread(thread_id) => {
            // Handle thread approval (permission or plan) via WebSocket
            let sent = app.handle_thread_approval(&thread_id);
            if sent {
                // Collapse overlay after successful approval
                app.dashboard.collapse_overlay();
                tracing::info!("Click: ApproveThread(thread_id={}) - response sent", thread_id);
            } else {
                tracing::debug!(
                    "Click: ApproveThread(thread_id={}) - no action needed",
                    thread_id
                );
            }
        }
        ClickAction::RejectThread(thread_id) => {
            // Handle thread rejection (permission or plan) via WebSocket
            let sent = app.handle_thread_rejection(&thread_id);
            if sent {
                // Collapse overlay after successful rejection
                app.dashboard.collapse_overlay();
                tracing::info!("Click: RejectThread(thread_id={}) - response sent", thread_id);
            } else {
                tracing::debug!(
                    "Click: RejectThread(thread_id={}) - no action needed",
                    thread_id
                );
            }
        }
        ClickAction::VerifyThread(thread_id) => {
            // Handle verification via REST endpoint with local fallback
            app.handle_thread_verification(&thread_id);
            tracing::info!("Click: VerifyThread(thread_id={}) - verification initiated", thread_id);
        }
        ClickAction::ArchiveThread(thread_id) => {
            // TODO: Implement when thread archiving is added
            tracing::debug!("Click: ArchiveThread(thread_id={}) (stub)", thread_id);
        }
        ClickAction::ResumeThread(thread_id) => {
            // TODO: Implement when thread resume functionality is added
            tracing::debug!("Click: ResumeThread(thread_id={}) (stub)", thread_id);
        }
        ClickAction::DeleteThread(thread_id) => {
            // TODO: Implement when thread deletion is added (with confirmation)
            tracing::debug!("Click: DeleteThread(thread_id={}) (stub)", thread_id);
        }
        ClickAction::ReportIssue(thread_id) => {
            // TODO: Implement when issue reporting is added
            tracing::debug!("Click: ReportIssue(thread_id={}) (stub)", thread_id);
        }

        // =====================================================================
        // Question Prompt Interactions
        // =====================================================================
        ClickAction::SelectOption { thread_id, index } => {
            // Select a specific option in the current question prompt
            // This updates the question state to toggle/select the option
            tracing::debug!(
                "Click: SelectOption(thread_id={}, index={}) (stub)",
                thread_id,
                index
            );
            // TODO: Implement when clickable question options are added
            // For now, question navigation is keyboard-only
        }
        ClickAction::ShowFreeFormInput(thread_id) => {
            // Show the free-form text input for a question
            app.dashboard.show_free_form(&thread_id);
            tracing::debug!("Click: ShowFreeFormInput(thread_id={})", thread_id);
        }
        ClickAction::SubmitFreeForm(thread_id) => {
            // Submit the free-form text response
            tracing::debug!("Click: SubmitFreeForm(thread_id={}) (stub)", thread_id);
            // TODO: Implement when clickable submit button for free-form is added
        }
        ClickAction::BackToOptions(thread_id) => {
            // Go back from free-form input to option selection
            app.dashboard.back_to_options(&thread_id);
            tracing::debug!("Click: BackToOptions(thread_id={})", thread_id);
        }

        // =====================================================================
        // Navigation
        // =====================================================================
        ClickAction::ViewFullPlan(thread_id) => {
            // Open the full plan view for a thread by expanding with a default anchor
            app.dashboard.expand_thread(&thread_id, 5);
            tracing::debug!("Click: ViewFullPlan(thread_id={})", thread_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests verify the click handler doesn't panic and marks dirty.
    // More comprehensive tests will be added when the actual implementations
    // are completed in later phases.

    fn create_test_app() -> App {
        App::new().expect("Failed to create test app")
    }

    #[test]
    fn test_handle_click_marks_dirty() {
        let mut app = create_test_app();

        // Clear the dirty flag
        app.needs_redraw = false;

        // Any click action should mark the app as dirty
        handle_click_action(&mut app, ClickAction::FilterWorking);
        assert!(app.needs_redraw);
    }

    #[test]
    fn test_handle_filter_actions_no_panic() {
        let mut app = create_test_app();

        // These should not panic even though they're stubs
        handle_click_action(&mut app, ClickAction::FilterWorking);
        handle_click_action(&mut app, ClickAction::FilterReadyToTest);
        handle_click_action(&mut app, ClickAction::FilterIdle);
        handle_click_action(&mut app, ClickAction::ClearFilter);
    }

    #[test]
    fn test_handle_overlay_actions_no_panic() {
        let mut app = create_test_app();

        handle_click_action(
            &mut app,
            ClickAction::ExpandThread {
                thread_id: "test-123".to_string(),
                anchor_y: 10,
            },
        );
        handle_click_action(&mut app, ClickAction::CollapseOverlay);
    }

    #[tokio::test]
    async fn test_handle_thread_actions_no_panic() {
        let mut app = create_test_app();
        let thread_id = "test-thread".to_string();

        handle_click_action(&mut app, ClickAction::ApproveThread(thread_id.clone()));
        handle_click_action(&mut app, ClickAction::RejectThread(thread_id.clone()));
        handle_click_action(&mut app, ClickAction::VerifyThread(thread_id.clone()));
        handle_click_action(&mut app, ClickAction::ArchiveThread(thread_id.clone()));
        handle_click_action(&mut app, ClickAction::ResumeThread(thread_id.clone()));
        handle_click_action(&mut app, ClickAction::DeleteThread(thread_id.clone()));
        handle_click_action(&mut app, ClickAction::ReportIssue(thread_id));
    }

    #[test]
    fn test_handle_question_actions_no_panic() {
        let mut app = create_test_app();
        let thread_id = "test-thread".to_string();

        handle_click_action(
            &mut app,
            ClickAction::SelectOption {
                thread_id: thread_id.clone(),
                index: 0,
            },
        );
        handle_click_action(&mut app, ClickAction::ShowFreeFormInput(thread_id.clone()));
        handle_click_action(&mut app, ClickAction::SubmitFreeForm(thread_id.clone()));
        handle_click_action(&mut app, ClickAction::BackToOptions(thread_id));
    }

    #[test]
    fn test_handle_navigation_actions_no_panic() {
        let mut app = create_test_app();

        handle_click_action(
            &mut app,
            ClickAction::ViewFullPlan("test-thread".to_string()),
        );
    }
}
