//! Reusable UI Components
//!
//! This module provides reusable UI components that can be shared across
//! different dialogs and overlays. All components follow the thread_switcher
//! styling patterns with responsive layout support.
//!
//! ## Components
//!
//! - `TabSelector` - Horizontal tab/mode selector with arrow markers
//! - `InputField` - Text input with focus handling, password masking, and errors
//! - `StatusIndicator` - Spinner, success, and error indicators
//! - `DialogFrame` - Centered dialog overlay with rounded borders

mod dialog_frame;
mod input_field;
mod status_indicator;
mod tab_selector;

pub use dialog_frame::{render_dialog_frame, DialogFrameConfig};
pub use input_field::{render_input_field, InputFieldConfig};
pub use status_indicator::{render_status_indicator, StatusIndicatorType};
pub use tab_selector::{render_tab_selector, TabItem};
