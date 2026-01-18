//! Input height calculation utilities.
//!
//! Provides functions for calculating dynamic input box heights.

// ============================================================================
// Input Height Constants
// ============================================================================

/// Maximum number of visible lines in the input area
pub const MAX_INPUT_LINES: u16 = 5;

// ============================================================================
// Input Height Calculation
// ============================================================================

/// Calculate the dynamic input box height based on line count.
///
/// Returns height in rows (including borders):
/// - Min: 3 rows (border + 1 line + border)
/// - Max: 7 rows (border + 5 lines + border)
pub fn calculate_input_box_height(line_count: usize) -> u16 {
    let content_lines = (line_count as u16).clamp(1, MAX_INPUT_LINES);
    content_lines + 2 // +2 for top/bottom borders
}

/// Calculate the total input area height (input box + keybinds + padding).
pub fn calculate_input_area_height(line_count: usize) -> u16 {
    calculate_input_box_height(line_count) + 1 + 2 // +1 keybinds, +2 for top/bottom padding
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_input_box_height_single_line() {
        assert_eq!(
            calculate_input_box_height(1),
            3,
            "Single line: 1 + 2 borders = 3"
        );
    }

    #[test]
    fn test_calculate_input_box_height_multiple_lines() {
        assert_eq!(
            calculate_input_box_height(2),
            4,
            "2 lines: 2 + 2 borders = 4"
        );
        assert_eq!(
            calculate_input_box_height(3),
            5,
            "3 lines: 3 + 2 borders = 5"
        );
        assert_eq!(
            calculate_input_box_height(4),
            6,
            "4 lines: 4 + 2 borders = 6"
        );
        assert_eq!(
            calculate_input_box_height(5),
            7,
            "5 lines: 5 + 2 borders = 7"
        );
    }

    #[test]
    fn test_calculate_input_box_height_clamped_max() {
        assert_eq!(
            calculate_input_box_height(6),
            7,
            "Max 5 lines + 2 borders = 7"
        );
        assert_eq!(
            calculate_input_box_height(10),
            7,
            "Max 5 lines + 2 borders = 7"
        );
        assert_eq!(
            calculate_input_box_height(100),
            7,
            "Max 5 lines + 2 borders = 7"
        );
    }

    #[test]
    fn test_calculate_input_box_height_clamped_min() {
        assert_eq!(
            calculate_input_box_height(0),
            3,
            "Min 1 line + 2 borders = 3"
        );
    }

    #[test]
    fn test_calculate_input_area_height_includes_keybinds_and_padding() {
        assert_eq!(
            calculate_input_area_height(1),
            6,
            "Box (3) + keybinds (1) + padding (2) = 6"
        );
        assert_eq!(
            calculate_input_area_height(5),
            10,
            "Box (7) + keybinds (1) + padding (2) = 10"
        );
    }
}
