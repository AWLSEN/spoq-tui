//! Provisioning Screen rendering
//!
//! Implements the provisioning/setup screen for initial application configuration.
//! Shows SPOQ logo, plan list, password field, and status information.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, Paragraph},
    Frame,
};

use crate::app::App;

use super::command_deck::SPOQ_LOGO;
use super::theme::{COLOR_ACCENT, COLOR_BORDER, COLOR_DIM, COLOR_HEADER};

// ============================================================================
// Provisioning Screen Rendering
// ============================================================================

/// Render the provisioning screen
pub fn render_provisioning_screen(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // Main outer border
    let outer_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(COLOR_BORDER))
        .title(Span::styled(
            " SPOQ Setup ",
            Style::default()
                .fg(COLOR_HEADER)
                .add_modifier(Modifier::BOLD),
        ));
    frame.render_widget(outer_block, area);

    // Inner area for content
    let inner = Rect {
        x: area.x + 2,
        y: area.y + 1,
        width: area.width.saturating_sub(4),
        height: area.height.saturating_sub(2),
    };

    // Main layout: Logo | Content
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),  // Logo section
            Constraint::Length(1),  // Spacer
            Constraint::Min(10),    // Main content
            Constraint::Length(3),  // Status bar
        ])
        .split(inner);

    render_logo_section(frame, main_chunks[0]);
    render_content_section(frame, main_chunks[2], app);
    render_status_bar(frame, main_chunks[3], app);
}

// ============================================================================
// Logo Section
// ============================================================================

fn render_logo_section(frame: &mut Frame, area: Rect) {
    // Center the logo horizontally
    let logo_width = 35u16; // Approximate width of SPOQ logo
    let x_offset = area.width.saturating_sub(logo_width) / 2;

    let logo_area = Rect {
        x: area.x + x_offset,
        y: area.y + 1,
        width: logo_width.min(area.width),
        height: 6,
    };

    let logo_lines: Vec<Line> = SPOQ_LOGO
        .iter()
        .map(|line| Line::from(Span::styled(*line, Style::default().fg(COLOR_HEADER))))
        .collect();

    let logo = Paragraph::new(logo_lines);
    frame.render_widget(logo, logo_area);
}

// ============================================================================
// Content Section
// ============================================================================

fn render_content_section(frame: &mut Frame, area: Rect, app: &App) {
    // Split into two columns: Plan list | Password/Settings
    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(area);

    render_plan_list(frame, content_chunks[0], app);
    render_settings_panel(frame, content_chunks[1], app);
}

fn render_plan_list(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(Span::styled(
            " Available Plans ",
            Style::default()
                .fg(COLOR_HEADER)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(COLOR_BORDER));

    // Create plan list items
    let items: Vec<ListItem> = app
        .provisioning
        .plans
        .iter()
        .enumerate()
        .map(|(idx, plan)| {
            let is_selected = idx == app.provisioning.selected_plan_index;
            let marker = if is_selected { "▶ " } else { "  " };

            let style = if is_selected {
                Style::default()
                    .fg(COLOR_ACCENT)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(COLOR_DIM)
            };

            ListItem::new(Line::from(vec![
                Span::styled(marker, style),
                Span::styled(&plan.name, style),
            ]))
        })
        .collect();

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

fn render_settings_panel(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(Span::styled(
            " Configuration ",
            Style::default()
                .fg(COLOR_HEADER)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(COLOR_BORDER));

    frame.render_widget(block, area);

    // Inner area for settings content
    let inner = Rect {
        x: area.x + 2,
        y: area.y + 2,
        width: area.width.saturating_sub(4),
        height: area.height.saturating_sub(3),
    };

    let mut lines: Vec<Line> = Vec::new();

    // Password field
    lines.push(Line::from(vec![
        Span::styled("Password: ", Style::default().fg(COLOR_DIM)),
    ]));

    let password_display = if app.provisioning.password.is_empty() {
        Span::styled(
            "[Enter password]",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        )
    } else {
        Span::styled(
            "*".repeat(app.provisioning.password.len()),
            Style::default().fg(COLOR_ACCENT),
        )
    };
    lines.push(Line::from(vec![
        Span::raw("  "),
        password_display,
    ]));

    lines.push(Line::from(""));

    // Show selected plan info
    if let Some(plan) = app.provisioning.plans.get(app.provisioning.selected_plan_index) {
        lines.push(Line::from(vec![
            Span::styled("Selected: ", Style::default().fg(COLOR_DIM)),
            Span::styled(&plan.name, Style::default().fg(COLOR_ACCENT)),
        ]));

        if let Some(desc) = &plan.description {
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(desc, Style::default().fg(COLOR_DIM)),
            ]));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(""));

    // Key hints
    lines.push(Line::from(vec![
        Span::styled("↑/↓", Style::default().fg(COLOR_ACCENT)),
        Span::styled(" Select plan  ", Style::default().fg(COLOR_DIM)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Tab", Style::default().fg(COLOR_ACCENT)),
        Span::styled(" Switch focus  ", Style::default().fg(COLOR_DIM)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Enter", Style::default().fg(COLOR_ACCENT)),
        Span::styled(" Confirm setup", Style::default().fg(COLOR_DIM)),
    ]));

    let content = Paragraph::new(lines);
    frame.render_widget(content, inner);
}

// ============================================================================
// Status Bar
// ============================================================================

fn render_status_bar(frame: &mut Frame, area: Rect, app: &App) {
    let status_text = match &app.provisioning.status {
        ProvisioningStatus::Idle => "Ready to configure",
        ProvisioningStatus::Loading => "Loading plans...",
        ProvisioningStatus::Error(msg) => msg.as_str(),
        ProvisioningStatus::Success => "Configuration complete!",
    };

    let status_color = match &app.provisioning.status {
        ProvisioningStatus::Idle => COLOR_DIM,
        ProvisioningStatus::Loading => Color::Yellow,
        ProvisioningStatus::Error(_) => Color::Red,
        ProvisioningStatus::Success => Color::Green,
    };

    let status_line = Line::from(vec![
        Span::styled(" Status: ", Style::default().fg(COLOR_DIM)),
        Span::styled(status_text, Style::default().fg(status_color)),
    ]);

    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(COLOR_BORDER));

    let paragraph = Paragraph::new(status_line).block(block);
    frame.render_widget(paragraph, area);
}

// ============================================================================
// Provisioning State Types
// ============================================================================

/// Status of the provisioning process
#[derive(Debug, Clone, Default)]
pub enum ProvisioningStatus {
    #[default]
    Idle,
    Loading,
    Error(String),
    Success,
}

/// A plan available for provisioning
#[derive(Debug, Clone)]
pub struct ProvisioningPlan {
    pub name: String,
    pub description: Option<String>,
    pub id: String,
}

/// State for the provisioning screen
#[derive(Debug, Clone, Default)]
pub struct ProvisioningState {
    pub plans: Vec<ProvisioningPlan>,
    pub selected_plan_index: usize,
    pub password: String,
    pub status: ProvisioningStatus,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provisioning_status_default() {
        let status = ProvisioningStatus::default();
        assert!(matches!(status, ProvisioningStatus::Idle));
    }

    #[test]
    fn test_provisioning_state_default() {
        let state = ProvisioningState::default();
        assert!(state.plans.is_empty());
        assert_eq!(state.selected_plan_index, 0);
        assert!(state.password.is_empty());
        assert!(matches!(state.status, ProvisioningStatus::Idle));
    }

    #[test]
    fn test_provisioning_plan_creation() {
        let plan = ProvisioningPlan {
            name: "Basic Plan".to_string(),
            description: Some("A basic plan for testing".to_string()),
            id: "basic-001".to_string(),
        };
        assert_eq!(plan.name, "Basic Plan");
        assert_eq!(plan.id, "basic-001");
        assert!(plan.description.is_some());
    }
}
