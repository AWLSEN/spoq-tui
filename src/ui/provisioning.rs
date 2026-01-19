//! VPS Provisioning Screen UI
//!
//! Implements the VPS provisioning screen with plan selection, password input,
//! and provisioning progress display. Uses the state machine pattern with
//! `ProvisioningPhase` to track the current stage of the provisioning flow.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, Paragraph},
    Frame,
};

use crate::app::App;
use crate::auth::central_api::{VpsPlan, VpsStatusResponse};

use super::command_deck::SPOQ_LOGO;
use super::helpers::{inner_rect, SPINNER_FRAMES};
use super::layout::LayoutContext;
use super::theme::{COLOR_ACTIVE, COLOR_BORDER, COLOR_DIM, COLOR_HEADER};

// ============================================================================
// Provisioning Phase State Machine
// ============================================================================

/// Current phase of the VPS provisioning flow
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProvisioningPhase {
    /// Loading available VPS plans from API
    LoadingPlans,
    /// User selecting a plan and entering password
    SelectPlan,
    /// Provisioning in progress (VPS being created)
    Provisioning,
    /// Waiting for VPS to become ready
    WaitingReady,
    /// VPS is ready with connection info
    Ready,
    /// Error occurred during any phase
    Error(String),
}

impl Default for ProvisioningPhase {
    fn default() -> Self {
        Self::LoadingPlans
    }
}

// ============================================================================
// Provisioning UI State
// ============================================================================

/// UI state for the provisioning screen
#[derive(Debug, Clone, Default)]
pub struct ProvisioningState {
    /// Current phase of provisioning
    pub phase: ProvisioningPhase,
    /// Available VPS plans (populated after LoadingPlans)
    pub plans: Vec<VpsPlan>,
    /// Currently selected plan index
    pub selected_plan_index: usize,
    /// Password input (masked in display)
    pub password: String,
    /// Status message for progress display
    pub status_message: Option<String>,
    /// VPS info when ready
    pub vps_info: Option<VpsStatusResponse>,
    /// Animation tick for spinners
    pub tick: u64,
}

impl ProvisioningState {
    /// Create new provisioning state in loading phase
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if password is valid (>= 12 characters)
    pub fn is_password_valid(&self) -> bool {
        self.password.len() >= 12
    }

    /// Get the currently selected plan, if any
    pub fn selected_plan(&self) -> Option<&VpsPlan> {
        self.plans.get(self.selected_plan_index)
    }

    /// Move selection up in plan list
    pub fn select_previous_plan(&mut self) {
        if self.selected_plan_index > 0 {
            self.selected_plan_index -= 1;
        }
    }

    /// Move selection down in plan list
    pub fn select_next_plan(&mut self) {
        if self.selected_plan_index + 1 < self.plans.len() {
            self.selected_plan_index += 1;
        }
    }

    /// Add character to password
    pub fn password_push(&mut self, c: char) {
        self.password.push(c);
    }

    /// Remove last character from password
    pub fn password_pop(&mut self) {
        self.password.pop();
    }

    /// Increment tick for animations
    pub fn tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
    }
}

// ============================================================================
// Main Render Function
// ============================================================================

/// Render the VPS provisioning screen
pub fn render_provisioning_screen(frame: &mut Frame, app: &App) {
    let size = frame.area();
    let ctx = LayoutContext::new(app.terminal_width, app.terminal_height);

    // Outer double border with title
    let outer_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(COLOR_BORDER))
        .title(Span::styled(
            " VPS Setup ",
            Style::default().fg(COLOR_HEADER).add_modifier(Modifier::BOLD),
        ));
    frame.render_widget(outer_block, size);

    let inner = inner_rect(size, 1);

    // Layout: Logo area | Content area
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(SPOQ_LOGO.len() as u16 + 1), // Logo
            Constraint::Min(10),                            // Content
        ])
        .split(inner);

    render_logo(frame, main_chunks[0]);

    // Render content based on current phase
    match &app.provisioning.phase {
        ProvisioningPhase::LoadingPlans => {
            render_loading_plans(frame, main_chunks[1], &app.provisioning);
        }
        ProvisioningPhase::SelectPlan => {
            render_plan_selection(frame, main_chunks[1], &app.provisioning, &ctx);
        }
        ProvisioningPhase::Provisioning => {
            render_provisioning_progress(frame, main_chunks[1], &app.provisioning, "Provisioning VPS...");
        }
        ProvisioningPhase::WaitingReady => {
            let msg = app.provisioning.status_message.as_deref().unwrap_or("Waiting for VPS to become ready...");
            render_provisioning_progress(frame, main_chunks[1], &app.provisioning, msg);
        }
        ProvisioningPhase::Ready => {
            render_vps_ready(frame, main_chunks[1], &app.provisioning);
        }
        ProvisioningPhase::Error(msg) => {
            render_error(frame, main_chunks[1], msg);
        }
    }
}

// ============================================================================
// Component Renderers
// ============================================================================

/// Render the SPOQ logo at top-left
fn render_logo(frame: &mut Frame, area: Rect) {
    let logo_lines: Vec<Line> = SPOQ_LOGO
        .iter()
        .map(|line| Line::from(Span::styled(*line, Style::default().fg(COLOR_HEADER))))
        .collect();

    let logo = Paragraph::new(logo_lines);
    frame.render_widget(logo, area);
}

/// Render loading spinner for plans
fn render_loading_plans(frame: &mut Frame, area: Rect, state: &ProvisioningState) {
    let spinner_idx = (state.tick as usize) % SPINNER_FRAMES.len();
    let spinner = SPINNER_FRAMES[spinner_idx];

    let text = Line::from(vec![
        Span::styled(spinner, Style::default().fg(COLOR_ACTIVE)),
        Span::raw(" Loading available plans..."),
    ]);

    let paragraph = Paragraph::new(text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(COLOR_DIM)),
    );
    frame.render_widget(paragraph, area);
}

/// Render plan selection with password field
fn render_plan_selection(frame: &mut Frame, area: Rect, state: &ProvisioningState, _ctx: &LayoutContext) {
    // Split into plan list and password section
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(6),     // Plan list
            Constraint::Length(5),  // Password section
        ])
        .split(area);

    render_plan_list(frame, chunks[0], state);
    render_password_field(frame, chunks[1], state);
}

/// Render the list of available plans
fn render_plan_list(frame: &mut Frame, area: Rect, state: &ProvisioningState) {
    let items: Vec<ListItem> = state
        .plans
        .iter()
        .enumerate()
        .map(|(idx, plan)| {
            let is_selected = idx == state.selected_plan_index;
            let marker = if is_selected { "▶ " } else { "  " };

            // Format: name | vcpus | ram | disk | price
            let ram_gb = plan.ram_mb / 1024;
            let price_dollars = plan.price_cents as f64 / 100.0;

            let line = format!(
                "{}{} | {}vCPU | {}GB RAM | {}GB Disk | ${:.2}/mo",
                marker, plan.name, plan.vcpus, ram_gb, plan.disk_gb, price_dollars
            );

            let style = if is_selected {
                Style::default().fg(COLOR_ACTIVE).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(COLOR_DIM)
            };

            ListItem::new(Line::from(Span::styled(line, style)))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(COLOR_BORDER))
            .title(Span::styled(
                " Select Plan ",
                Style::default().fg(COLOR_HEADER),
            )),
    );

    frame.render_widget(list, area);
}

/// Render password input field with validation indicator
fn render_password_field(frame: &mut Frame, area: Rect, state: &ProvisioningState) {
    let masked: String = "●".repeat(state.password.len());
    let is_valid = state.is_password_valid();

    let validation_indicator = if is_valid {
        Span::styled(" ✓", Style::default().fg(Color::Green))
    } else {
        Span::styled(" ✗", Style::default().fg(Color::Red))
    };

    let hint = if is_valid {
        Span::styled(" (valid)", Style::default().fg(Color::Green))
    } else {
        Span::styled(
            format!(" ({}/12 chars)", state.password.len()),
            Style::default().fg(COLOR_DIM),
        )
    };

    let lines = vec![
        Line::from(vec![
            Span::styled("Password: ", Style::default().fg(COLOR_HEADER)),
            Span::raw(&masked),
            Span::styled("█", Style::default().fg(COLOR_ACTIVE)), // Cursor
            validation_indicator,
        ]),
        Line::from(hint),
    ];

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(COLOR_BORDER))
            .title(Span::styled(
                " Root Password ",
                Style::default().fg(COLOR_HEADER),
            )),
    );

    frame.render_widget(paragraph, area);
}

/// Render provisioning progress with spinner
fn render_provisioning_progress(frame: &mut Frame, area: Rect, state: &ProvisioningState, message: &str) {
    let spinner_idx = (state.tick as usize) % SPINNER_FRAMES.len();
    let spinner = SPINNER_FRAMES[spinner_idx];

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(spinner, Style::default().fg(COLOR_ACTIVE)),
            Span::raw(" "),
            Span::raw(message),
        ]),
        Line::from(""),
    ];

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(COLOR_BORDER))
            .title(Span::styled(
                " Provisioning ",
                Style::default().fg(COLOR_HEADER),
            )),
    );

    frame.render_widget(paragraph, area);
}

/// Render VPS ready screen with connection info
fn render_vps_ready(frame: &mut Frame, area: Rect, state: &ProvisioningState) {
    let mut lines = vec![
        Line::from(Span::styled(
            "✓ VPS Ready!",
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    if let Some(vps_info) = &state.vps_info {
        lines.push(Line::from(vec![
            Span::styled("VPS ID: ", Style::default().fg(COLOR_HEADER)),
            Span::raw(&vps_info.vps_id),
        ]));

        if let Some(hostname) = &vps_info.hostname {
            lines.push(Line::from(vec![
                Span::styled("Hostname: ", Style::default().fg(COLOR_HEADER)),
                Span::raw(hostname),
            ]));
        }

        if let Some(ip) = &vps_info.ip {
            lines.push(Line::from(vec![
                Span::styled("IP: ", Style::default().fg(COLOR_HEADER)),
                Span::raw(ip),
            ]));
        }

        if let Some(url) = &vps_info.url {
            lines.push(Line::from(vec![
                Span::styled("URL: ", Style::default().fg(COLOR_HEADER)),
                Span::raw(url),
            ]));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Press Enter to continue...",
        Style::default().fg(COLOR_DIM),
    )));

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green))
            .title(Span::styled(
                " Complete ",
                Style::default().fg(Color::Green),
            )),
    );

    frame.render_widget(paragraph, area);
}

/// Render error message
fn render_error(frame: &mut Frame, area: Rect, message: &str) {
    let lines = vec![
        Line::from(Span::styled(
            "✗ Error",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(message, Style::default().fg(Color::Red))),
        Line::from(""),
        Line::from(Span::styled(
            "Press Esc to go back",
            Style::default().fg(COLOR_DIM),
        )),
    ];

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Red))
            .title(Span::styled(" Error ", Style::default().fg(Color::Red))),
    );

    frame.render_widget(paragraph, area);
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provisioning_phase_default() {
        let phase = ProvisioningPhase::default();
        assert_eq!(phase, ProvisioningPhase::LoadingPlans);
    }

    #[test]
    fn test_provisioning_state_new() {
        let state = ProvisioningState::new();
        assert_eq!(state.phase, ProvisioningPhase::LoadingPlans);
        assert!(state.plans.is_empty());
        assert_eq!(state.selected_plan_index, 0);
        assert!(state.password.is_empty());
    }

    #[test]
    fn test_password_validation() {
        let mut state = ProvisioningState::new();

        // Less than 12 chars is invalid
        state.password = "short".to_string();
        assert!(!state.is_password_valid());

        // Exactly 12 chars is valid
        state.password = "123456789012".to_string();
        assert!(state.is_password_valid());

        // More than 12 chars is valid
        state.password = "this_is_a_long_password".to_string();
        assert!(state.is_password_valid());
    }

    #[test]
    fn test_password_push_pop() {
        let mut state = ProvisioningState::new();

        state.password_push('a');
        state.password_push('b');
        state.password_push('c');
        assert_eq!(state.password, "abc");

        state.password_pop();
        assert_eq!(state.password, "ab");

        state.password_pop();
        state.password_pop();
        state.password_pop(); // Should handle empty gracefully
        assert!(state.password.is_empty());
    }

    #[test]
    fn test_plan_selection() {
        let mut state = ProvisioningState::new();
        state.plans = vec![
            VpsPlan {
                id: "1".to_string(),
                name: "Small".to_string(),
                vcpus: 1,
                ram_mb: 1024,
                disk_gb: 25,
                price_cents: 500,
                bandwidth_tb: None,
                first_month_price_cents: None,
            },
            VpsPlan {
                id: "2".to_string(),
                name: "Medium".to_string(),
                vcpus: 2,
                ram_mb: 2048,
                disk_gb: 50,
                price_cents: 1000,
                bandwidth_tb: None,
                first_month_price_cents: None,
            },
            VpsPlan {
                id: "3".to_string(),
                name: "Large".to_string(),
                vcpus: 4,
                ram_mb: 4096,
                disk_gb: 100,
                price_cents: 2000,
                bandwidth_tb: None,
                first_month_price_cents: None,
            },
        ];

        assert_eq!(state.selected_plan_index, 0);
        assert_eq!(state.selected_plan().unwrap().name, "Small");

        state.select_next_plan();
        assert_eq!(state.selected_plan_index, 1);
        assert_eq!(state.selected_plan().unwrap().name, "Medium");

        state.select_next_plan();
        assert_eq!(state.selected_plan_index, 2);
        assert_eq!(state.selected_plan().unwrap().name, "Large");

        // Should not go past the end
        state.select_next_plan();
        assert_eq!(state.selected_plan_index, 2);

        state.select_previous_plan();
        assert_eq!(state.selected_plan_index, 1);

        state.select_previous_plan();
        state.select_previous_plan();
        // Should not go below 0
        state.select_previous_plan();
        assert_eq!(state.selected_plan_index, 0);
    }

    #[test]
    fn test_tick_increment() {
        let mut state = ProvisioningState::new();
        assert_eq!(state.tick, 0);

        state.tick();
        assert_eq!(state.tick, 1);

        state.tick();
        state.tick();
        assert_eq!(state.tick, 3);
    }
}
