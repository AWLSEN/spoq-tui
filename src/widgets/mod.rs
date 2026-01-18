pub mod textarea;

// Re-export for backwards compatibility
pub use textarea::{TextAreaInput, TextAreaInputWidget};

// Also re-export under the old module name for full backwards compatibility
pub mod textarea_input {
    pub use super::textarea::{TextAreaInput, TextAreaInputWidget};
}
