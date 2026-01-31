//! Input Field Component
//!
//! A text input field with focus handling, password masking, and inline error display.
//! Matches the thread_switcher visual style with rounded borders.

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::ui::layout::LayoutContext;
use crate::ui::theme::{COLOR_BORDER, COLOR_DIM, COLOR_INPUT_BG};

/// Configuration for rendering an input field
#[derive(Debug, Clone)]
pub struct InputFieldConfig<'a> {
    /// Label displayed above the input
    pub label: &'a str,
    /// Current value of the input
    pub value: &'a str,
    /// Whether the input is currently focused
    pub focused: bool,
    /// Whether to mask the value (for passwords)
    pub is_password: bool,
    /// Optional error message to display below the input
    pub error: Option<&'a str>,
    /// Optional placeholder text when empty
    pub placeholder: Option<&'a str>,
}

impl<'a> InputFieldConfig<'a> {
    /// Create a new input field configuration
    pub fn new(label: &'a str, value: &'a str) -> Self {
        Self {
            label,
            value,
            focused: false,
            is_password: false,
            error: None,
            placeholder: None,
        }
    }

    /// Set whether the input is focused
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Set whether to mask the value (for passwords)
    pub fn password(mut self, is_password: bool) -> Self {
        self.is_password = is_password;
        self
    }

    /// Set an error message to display
    pub fn error(mut self, error: Option<&'a str>) -> Self {
        self.error = error;
        self
    }

    /// Set placeholder text
    pub fn placeholder(mut self, placeholder: &'a str) -> Self {
        self.placeholder = Some(placeholder);
        self
    }
}

/// Calculate the height needed for an input field
///
/// Returns the number of rows needed:
/// - 1 for label
/// - 3 for input box (border + content + border)
/// - 1 for error (if present)
pub fn calculate_input_field_height(config: &InputFieldConfig) -> u16 {
    let mut height = 4; // Label (1) + input box (3)
    if config.error.is_some() {
        height += 1; // Error message
    }
    height
}

/// Render an input field with label, input box, and optional error
///
/// # Arguments
/// * `frame` - The frame to render to
/// * `area` - The area to render in (should be tall enough for all elements)
/// * `config` - Configuration for the input field
/// * `ctx` - Layout context for responsive sizing
///
/// # Returns
/// The height consumed by this input field
pub fn render_input_field(
    frame: &mut Frame,
    area: Rect,
    config: &InputFieldConfig,
    _ctx: &LayoutContext,
) -> u16 {
    let mut y_offset = 0;

    // Render label
    let label_style = if config.focused {
        Style::default().fg(Color::White)
    } else {
        Style::default().fg(COLOR_DIM)
    };

    let label_area = Rect {
        x: area.x + 2,
        y: area.y + y_offset,
        width: area.width.saturating_sub(4),
        height: 1,
    };
    let label = Paragraph::new(Line::from(Span::styled(config.label, label_style)));
    frame.render_widget(label, label_area);
    y_offset += 1;

    // Render input box
    let input_area = Rect {
        x: area.x + 2,
        y: area.y + y_offset,
        width: area.width.saturating_sub(4),
        height: 3,
    };

    let border_color = if config.focused {
        Color::White
    } else {
        COLOR_BORDER
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(COLOR_INPUT_BG));

    // Prepare display value
    let display_value = if config.is_password {
        "\u{2022}".repeat(config.value.len()) // Bullet character
    } else if config.value.is_empty() && config.placeholder.is_some() {
        config.placeholder.unwrap().to_string()
    } else {
        config.value.to_string()
    };

    let text_style = if config.value.is_empty() && config.placeholder.is_some() {
        Style::default().fg(COLOR_DIM)
    } else if config.focused {
        Style::default().fg(Color::White)
    } else {
        Style::default().fg(COLOR_DIM)
    };

    // Add cursor if focused
    let mut content = display_value.clone();
    if config.focused {
        content.push('\u{2588}'); // Block cursor
    }

    let input_text = Paragraph::new(Line::from(Span::styled(content, text_style))).block(block);

    frame.render_widget(input_text, input_area);
    y_offset += 3;

    // Render error if present
    if let Some(error) = config.error {
        let error_area = Rect {
            x: area.x + 2,
            y: area.y + y_offset,
            width: area.width.saturating_sub(4),
            height: 1,
        };

        let error_text = Paragraph::new(Line::from(vec![
            Span::styled("\u{2717} ", Style::default().fg(Color::Red)), // X mark
            Span::styled(error, Style::default().fg(Color::Red)),
        ]));

        frame.render_widget(error_text, error_area);
        y_offset += 1;
    }

    y_offset
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_field_config_new() {
        let config = InputFieldConfig::new("Label", "Value");
        assert_eq!(config.label, "Label");
        assert_eq!(config.value, "Value");
        assert!(!config.focused);
        assert!(!config.is_password);
        assert!(config.error.is_none());
        assert!(config.placeholder.is_none());
    }

    #[test]
    fn test_input_field_config_builder() {
        let config = InputFieldConfig::new("Password", "secret")
            .focused(true)
            .password(true)
            .error(Some("Invalid"))
            .placeholder("Enter password");

        assert!(config.focused);
        assert!(config.is_password);
        assert_eq!(config.error, Some("Invalid"));
        assert_eq!(config.placeholder, Some("Enter password"));
    }

    #[test]
    fn test_calculate_height_without_error() {
        let config = InputFieldConfig::new("Label", "Value");
        assert_eq!(calculate_input_field_height(&config), 4);
    }

    #[test]
    fn test_calculate_height_with_error() {
        let config = InputFieldConfig::new("Label", "Value").error(Some("Error message"));
        assert_eq!(calculate_input_field_height(&config), 5);
    }
}
