//! Touch interaction system for SPOQ TUI.
//!
//! This module provides a registry-based system for handling clickable regions
//! in the terminal UI. It enables touch-first interactions by:
//!
//! 1. **Hit Area Registration**: Components register clickable regions during
//!    rendering with their associated actions.
//!
//! 2. **Hit Testing**: When a mouse click occurs, the registry is queried to
//!    find which area (if any) was clicked.
//!
//! 3. **Action Dispatch**: The clicked area's action is passed to the click
//!    handler which updates the App state accordingly.
//!
//! 4. **Hover Tracking**: Mouse movement updates hover state for visual feedback.
//!
//! ## Usage
//!
//! ```ignore
//! // During render (in a UI component):
//! app.hit_registry.register(
//!     button_rect,
//!     ClickAction::FilterWorking,
//!     Some(hover_style),
//! );
//!
//! // In the event loop (main.rs):
//! Event::Mouse(mouse) => {
//!     match mouse.kind {
//!         MouseEventKind::Down(MouseButton::Left) => {
//!             if let Some(action) = app.hit_registry.hit_test(mouse.column, mouse.row) {
//!                 handle_click_action(&mut app, action);
//!                 app.mark_dirty();
//!             }
//!         }
//!         MouseEventKind::Moved => {
//!             if app.hit_registry.update_hover(mouse.column, mouse.row) {
//!                 app.mark_dirty();
//!             }
//!         }
//!         _ => {}
//!     }
//! }
//! ```
//!
//! ## Architecture
//!
//! - [`HitArea`]: A single clickable region with rect, action, and optional hover style
//! - [`ClickAction`]: Enum of all possible click actions
//! - [`HitAreaRegistry`]: Manages registered areas, hit testing, and hover state
//! - [`handle_click_action`]: Dispatches click actions to App methods

mod click_handler;
mod hit_area;

pub use click_handler::handle_click_action;
pub use hit_area::{ClickAction, HitArea, HitAreaRegistry};
