//! Color theme constants for SPOQ UI
//!
//! Defines the minimal dark color palette used throughout the UI.

use ratatui::style::Color;

// ============================================================================
// Minimal Dark Color Theme
// ============================================================================

/// Primary border color - dark gray for minimal aesthetic
pub const COLOR_BORDER: Color = Color::DarkGray;

/// Accent color - white for highlights and important elements
pub const COLOR_ACCENT: Color = Color::White;

/// Header text color - white for the logo
pub const COLOR_HEADER: Color = Color::White;

/// Active/running elements - bright green
pub const COLOR_ACTIVE: Color = Color::LightGreen;

/// Queued/pending elements - gray
pub const COLOR_QUEUED: Color = Color::Gray;

/// Dim text for less important info
pub const COLOR_DIM: Color = Color::DarkGray;

/// Background for input areas (used in later phases)
#[allow(dead_code)]
pub const COLOR_INPUT_BG: Color = Color::Rgb(20, 20, 30);

/// Progress bar fill color - white
pub const COLOR_PROGRESS: Color = Color::White;

/// Progress bar background (used in later phases)
#[allow(dead_code)]
pub const COLOR_PROGRESS_BG: Color = Color::DarkGray;

// ============================================================================
// Claude Code Tool Colors
// ============================================================================

/// Tool icon color - Claude Code blue
pub const COLOR_TOOL_ICON: Color = Color::Rgb(0, 122, 204); // blue #007ACC

/// Tool running state - gray
pub const COLOR_TOOL_RUNNING: Color = Color::Rgb(128, 128, 128); // gray for running state

/// Tool success state - Claude Code green
pub const COLOR_TOOL_SUCCESS: Color = Color::Rgb(4, 181, 117); // green #04B575

/// Tool error state - red
pub const COLOR_TOOL_ERROR: Color = Color::Red;

// ============================================================================
// Subagent Colors
// ============================================================================

/// Subagent running state - cyan
pub const COLOR_SUBAGENT_RUNNING: Color = Color::Cyan;

/// Subagent complete state - green (same as tool success for consistency)
pub const COLOR_SUBAGENT_COMPLETE: Color = Color::Rgb(4, 181, 117); // green #04B575

// ============================================================================
// Dialog Colors
// ============================================================================

/// Background color for dialog boxes (permission, skill, AskUserQuestion)
pub const COLOR_DIALOG_BG: Color = Color::Rgb(10, 15, 35);
