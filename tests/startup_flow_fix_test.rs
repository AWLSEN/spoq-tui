use spoq::auth::credentials::{Credentials, CredentialsManager};
use tempfile::TempDir;

/// Scenario 1: Fresh install (no credentials.json)
#[test]
fn test_fresh_install_no_credentials() {
    let temp_dir = TempDir::new().unwrap();
    let manager = create_test_manager(&temp_dir);

    let creds = manager.load();
    assert!(!creds.has_token());
    assert!(!creds.has_vps());
}

/// Scenario 2: Valid credentials + VPS (should skip auth/provision)
#[test]
fn test_valid_credentials_with_vps() {
    let temp_dir = TempDir::new().unwrap();
    let manager = create_test_manager(&temp_dir);

    let mut creds = Credentials::default();
    creds.access_token = Some("valid-token".to_string());
    creds.expires_at = Some(chrono::Utc::now().timestamp() + 3600);
    creds.vps_id = Some("vps-123".to_string());
    creds.vps_url = Some("https://vps.example.com".to_string());
    creds.vps_status = Some("ready".to_string());

    assert!(manager.save(&creds));

    let loaded = manager.load();
    assert!(loaded.has_token());
    assert!(!loaded.is_expired());
    assert!(loaded.has_vps());
    assert_eq!(loaded.vps_status, Some("ready".to_string()));
}

/// Scenario 3: Expired token + VPS (should refresh, not re-auth)
#[test]
fn test_expired_token_with_vps() {
    let temp_dir = TempDir::new().unwrap();
    let manager = create_test_manager(&temp_dir);

    let mut creds = Credentials::default();
    creds.access_token = Some("expired-token".to_string());
    creds.refresh_token = Some("valid-refresh".to_string());
    creds.expires_at = Some(0); // Expired
    creds.vps_id = Some("vps-123".to_string());
    creds.vps_url = Some("https://vps.example.com".to_string());
    creds.vps_status = Some("ready".to_string());

    assert!(manager.save(&creds));

    let loaded = manager.load();
    assert!(loaded.has_token());
    assert!(loaded.is_expired()); // Should detect expiration
    assert!(loaded.has_vps());
}

/// Scenario 4: Valid token + no VPS (should provision)
#[test]
fn test_valid_token_no_vps() {
    let temp_dir = TempDir::new().unwrap();
    let manager = create_test_manager(&temp_dir);

    let mut creds = Credentials::default();
    creds.access_token = Some("valid-token".to_string());
    creds.expires_at = Some(chrono::Utc::now().timestamp() + 3600);

    assert!(manager.save(&creds));

    let loaded = manager.load();
    assert!(loaded.has_token());
    assert!(!loaded.is_expired());
    assert!(!loaded.has_vps()); // Should detect no VPS
}

/// Scenario 5: Valid token + stopped VPS (should auto-start)
#[test]
fn test_valid_token_stopped_vps() {
    let temp_dir = TempDir::new().unwrap();
    let manager = create_test_manager(&temp_dir);

    let mut creds = Credentials::default();
    creds.access_token = Some("valid-token".to_string());
    creds.expires_at = Some(chrono::Utc::now().timestamp() + 3600);
    creds.vps_id = Some("vps-123".to_string());
    creds.vps_url = Some("https://vps.example.com".to_string());
    creds.vps_status = Some("stopped".to_string());

    assert!(manager.save(&creds));

    let loaded = manager.load();
    assert_eq!(loaded.vps_status, Some("stopped".to_string()));
}

/// Scenario 6: Valid token + failed VPS (should error, not reprovision)
#[test]
fn test_valid_token_failed_vps() {
    let temp_dir = TempDir::new().unwrap();
    let manager = create_test_manager(&temp_dir);

    let mut creds = Credentials::default();
    creds.access_token = Some("valid-token".to_string());
    creds.expires_at = Some(chrono::Utc::now().timestamp() + 3600);
    creds.vps_id = Some("vps-123".to_string());
    creds.vps_url = Some("https://vps.example.com".to_string());
    creds.vps_status = Some("failed".to_string());

    assert!(manager.save(&creds));

    let loaded = manager.load();
    assert_eq!(loaded.vps_status, Some("failed".to_string()));
    // Startup should detect this and exit with error, not reprovision
}

/// Scenario 7: Credentials missing vps_status field (should fetch, not reprovision)
#[test]
fn test_credentials_missing_vps_status_field() {
    let temp_dir = TempDir::new().unwrap();
    let manager = create_test_manager(&temp_dir);

    let mut creds = Credentials::default();
    creds.access_token = Some("valid-token".to_string());
    creds.expires_at = Some(chrono::Utc::now().timestamp() + 3600);
    creds.vps_id = Some("vps-123".to_string());
    creds.vps_url = Some("https://vps.example.com".to_string());
    creds.vps_status = None; // Missing!

    assert!(manager.save(&creds));

    let loaded = manager.load();
    assert!(loaded.has_vps()); // VPS exists
    assert!(loaded.vps_status.is_none()); // But status missing
    // Startup should fetch status from API, not reprovision
}

// Helper function
fn create_test_manager(temp_dir: &TempDir) -> CredentialsManager {
    // Set HOME to temp directory so CredentialsManager uses it
    std::env::set_var("HOME", temp_dir.path());
    CredentialsManager::new().expect("Failed to create credentials manager")
}
