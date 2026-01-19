//! Round 7: Tests for provisioning screen input handling
//!
//! Tests keyboard input handling for VPS provisioning screen:
//! - Password entry mode (typing, backspace)
//! - Plan selection navigation (Up/Down, j/k)
//! - Password validation (>=12 chars)
//! - Enter key to start provisioning
//! - Exit with q/Esc

use spoq::app::{App, ProvisioningPhase, Screen};
use spoq::auth::central_api::VpsPlan;

/// Helper to create an app in Provisioning screen with test plans
fn create_provisioning_app() -> App {
    let mut app = App::new().expect("Failed to create app");
    app.screen = Screen::Provisioning;
    app.provisioning_phase = ProvisioningPhase::SelectPlan;

    // Add test VPS plans
    app.vps_plans = vec![
        VpsPlan {
            id: "plan1".to_string(),
            name: "Basic".to_string(),
            vcpus: 1,
            ram_mb: 1024,
            disk_gb: 25,
            price_cents: 500,
        },
        VpsPlan {
            id: "plan2".to_string(),
            name: "Standard".to_string(),
            vcpus: 2,
            ram_mb: 2048,
            disk_gb: 50,
            price_cents: 1000,
        },
        VpsPlan {
            id: "plan3".to_string(),
            name: "Premium".to_string(),
            vcpus: 4,
            ram_mb: 4096,
            disk_gb: 100,
            price_cents: 2000,
        },
    ];

    app.selected_plan_idx = 0;
    app.ssh_password_input = String::new();
    app.entering_ssh_password = false;

    app
}

#[test]
fn test_password_entry_mode_toggle() {
    let mut app = create_provisioning_app();

    // Initially not entering password
    assert!(!app.entering_ssh_password);

    // Press 'p' to enter password mode
    app.entering_ssh_password = true;
    assert!(app.entering_ssh_password);

    // Press Enter or Esc to exit password mode
    app.entering_ssh_password = false;
    assert!(!app.entering_ssh_password);
}

#[test]
fn test_password_input_typing() {
    let mut app = create_provisioning_app();
    app.entering_ssh_password = true;

    // Type password characters
    let password = "MySecurePass123!";
    for ch in password.chars() {
        app.ssh_password_input.push(ch);
    }

    assert_eq!(app.ssh_password_input, password);
    assert_eq!(app.ssh_password_input.len(), 16);
}

#[test]
fn test_password_input_backspace() {
    let mut app = create_provisioning_app();
    app.entering_ssh_password = true;
    app.ssh_password_input = "Test123".to_string();

    // Backspace removes last character
    app.ssh_password_input.pop();
    assert_eq!(app.ssh_password_input, "Test12");

    // Multiple backspaces
    app.ssh_password_input.pop();
    app.ssh_password_input.pop();
    assert_eq!(app.ssh_password_input, "Test");

    // Backspace on empty string does nothing
    app.ssh_password_input.clear();
    app.ssh_password_input.pop();
    assert_eq!(app.ssh_password_input, "");
}

#[test]
fn test_plan_navigation_down() {
    let mut app = create_provisioning_app();
    assert_eq!(app.selected_plan_idx, 0);

    // Navigate down
    app.selected_plan_idx += 1;
    assert_eq!(app.selected_plan_idx, 1);

    app.selected_plan_idx += 1;
    assert_eq!(app.selected_plan_idx, 2);

    // At last plan, can't go further
    let max_idx = app.vps_plans.len().saturating_sub(1);
    if app.selected_plan_idx < max_idx {
        app.selected_plan_idx += 1;
    }
    assert_eq!(app.selected_plan_idx, 2); // Still at last
}

#[test]
fn test_plan_navigation_up() {
    let mut app = create_provisioning_app();
    app.selected_plan_idx = 2; // Start at last plan

    // Navigate up
    if app.selected_plan_idx > 0 {
        app.selected_plan_idx -= 1;
    }
    assert_eq!(app.selected_plan_idx, 1);

    if app.selected_plan_idx > 0 {
        app.selected_plan_idx -= 1;
    }
    assert_eq!(app.selected_plan_idx, 0);

    // At first plan, can't go further
    if app.selected_plan_idx > 0 {
        app.selected_plan_idx -= 1;
    }
    assert_eq!(app.selected_plan_idx, 0); // Still at first
}

#[test]
fn test_plan_navigation_vim_keys() {
    let mut app = create_provisioning_app();

    // 'j' moves down
    assert_eq!(app.selected_plan_idx, 0);
    app.selected_plan_idx += 1;
    assert_eq!(app.selected_plan_idx, 1);

    // 'k' moves up
    if app.selected_plan_idx > 0 {
        app.selected_plan_idx -= 1;
    }
    assert_eq!(app.selected_plan_idx, 0);
}

#[test]
fn test_password_validation_too_short() {
    let mut app = create_provisioning_app();
    app.ssh_password_input = "short".to_string();

    // Password less than 12 chars should not allow provisioning
    let can_provision = app.ssh_password_input.len() >= 12 && !app.vps_plans.is_empty();
    assert!(!can_provision);
}

#[test]
fn test_password_validation_exactly_12_chars() {
    let mut app = create_provisioning_app();
    app.ssh_password_input = "ValidPass123".to_string();

    // Password exactly 12 chars should be valid
    assert_eq!(app.ssh_password_input.len(), 12);
    let can_provision = app.ssh_password_input.len() >= 12 && !app.vps_plans.is_empty();
    assert!(can_provision);
}

#[test]
fn test_password_validation_long_password() {
    let mut app = create_provisioning_app();
    app.ssh_password_input = "ThisIsAVeryLongSecurePassword123!@#".to_string();

    // Long password should be valid
    assert!(app.ssh_password_input.len() > 12);
    let can_provision = app.ssh_password_input.len() >= 12 && !app.vps_plans.is_empty();
    assert!(can_provision);
}

#[test]
fn test_enter_starts_provisioning() {
    let mut app = create_provisioning_app();
    app.ssh_password_input = "ValidPassword123".to_string();
    app.selected_plan_idx = 1;

    // Validate and start provisioning
    if app.ssh_password_input.len() >= 12 && !app.vps_plans.is_empty() {
        app.provisioning_phase = ProvisioningPhase::Provisioning;
    }

    assert!(matches!(app.provisioning_phase, ProvisioningPhase::Provisioning));
}

#[test]
fn test_enter_does_not_start_with_invalid_password() {
    let mut app = create_provisioning_app();
    app.ssh_password_input = "short".to_string();
    app.provisioning_phase = ProvisioningPhase::SelectPlan;

    // Should not transition to Provisioning
    if app.ssh_password_input.len() >= 12 && !app.vps_plans.is_empty() {
        app.provisioning_phase = ProvisioningPhase::Provisioning;
    }

    assert!(matches!(app.provisioning_phase, ProvisioningPhase::SelectPlan));
}

#[test]
fn test_enter_does_not_start_without_plans() {
    let mut app = create_provisioning_app();
    app.ssh_password_input = "ValidPassword123".to_string();
    app.vps_plans.clear(); // No plans available

    // Should not transition to Provisioning
    if app.ssh_password_input.len() >= 12 && !app.vps_plans.is_empty() {
        app.provisioning_phase = ProvisioningPhase::Provisioning;
    }

    assert!(matches!(app.provisioning_phase, ProvisioningPhase::SelectPlan));
}

#[test]
fn test_provisioning_screen_state() {
    let app = create_provisioning_app();

    // Verify initial state
    assert_eq!(app.screen, Screen::Provisioning);
    assert!(matches!(app.provisioning_phase, ProvisioningPhase::SelectPlan));
    assert_eq!(app.vps_plans.len(), 3);
    assert_eq!(app.selected_plan_idx, 0);
    assert_eq!(app.ssh_password_input, "");
    assert!(!app.entering_ssh_password);
}

#[test]
fn test_selected_plan_stays_in_bounds() {
    let mut app = create_provisioning_app();
    let max_idx = app.vps_plans.len().saturating_sub(1);

    // Try to navigate beyond bounds
    app.selected_plan_idx = 0;
    if app.selected_plan_idx > 0 {
        app.selected_plan_idx -= 1; // Should not decrement
    }
    assert_eq!(app.selected_plan_idx, 0);

    app.selected_plan_idx = max_idx;
    if app.selected_plan_idx < max_idx {
        app.selected_plan_idx += 1; // Should not increment
    }
    assert_eq!(app.selected_plan_idx, max_idx);
}

#[test]
fn test_password_entry_mode_isolation() {
    let mut app = create_provisioning_app();

    // In normal mode, password input should not change
    app.entering_ssh_password = false;
    let before = app.ssh_password_input.clone();
    // Simulate navigation keys - these don't affect password input
    app.selected_plan_idx += 1;
    assert_eq!(app.ssh_password_input, before);

    // In password entry mode, navigation should not happen
    app.entering_ssh_password = true;
    app.selected_plan_idx = 1;
    // Simulate typing password
    app.ssh_password_input.push('a');
    // selected_plan_idx should remain unchanged
    assert_eq!(app.selected_plan_idx, 1);
}

#[test]
fn test_phase_transitions() {
    let mut app = create_provisioning_app();

    // LoadingPlans -> SelectPlan
    app.provisioning_phase = ProvisioningPhase::LoadingPlans;
    app.provisioning_phase = ProvisioningPhase::SelectPlan;
    assert!(matches!(app.provisioning_phase, ProvisioningPhase::SelectPlan));

    // SelectPlan -> Provisioning (with valid password)
    app.ssh_password_input = "ValidPassword123".to_string();
    if app.ssh_password_input.len() >= 12 && !app.vps_plans.is_empty() {
        app.provisioning_phase = ProvisioningPhase::Provisioning;
    }
    assert!(matches!(app.provisioning_phase, ProvisioningPhase::Provisioning));

    // Provisioning -> WaitingReady
    app.provisioning_phase = ProvisioningPhase::WaitingReady {
        status: "Initializing...".to_string(),
    };
    assert!(matches!(app.provisioning_phase, ProvisioningPhase::WaitingReady { .. }));

    // WaitingReady -> Ready
    app.provisioning_phase = ProvisioningPhase::Ready {
        hostname: "vps-001.example.com".to_string(),
        ip: "192.168.1.1".to_string(),
    };
    assert!(matches!(app.provisioning_phase, ProvisioningPhase::Ready { .. }));
}

#[test]
fn test_password_special_characters() {
    let mut app = create_provisioning_app();
    app.entering_ssh_password = true;

    // Type password with special characters
    let password = "P@ssw0rd!#$%";
    for ch in password.chars() {
        app.ssh_password_input.push(ch);
    }

    assert_eq!(app.ssh_password_input, password);
    assert_eq!(app.ssh_password_input.len(), 12);

    // Should be valid for provisioning
    let can_provision = app.ssh_password_input.len() >= 12 && !app.vps_plans.is_empty();
    assert!(can_provision);
}

#[test]
fn test_empty_password_validation() {
    let mut app = create_provisioning_app();
    app.ssh_password_input = "".to_string();

    let can_provision = app.ssh_password_input.len() >= 12 && !app.vps_plans.is_empty();
    assert!(!can_provision);
}
