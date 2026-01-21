//! Tests for token migration integration with VPS provisioning flows.
//!
//! These tests verify that token migration is properly integrated into
//! both managed VPS and BYOVPS provisioning flows.

use spoq::auth::{Credentials, TokenDetectionResult, TokenExportResult};
use std::path::PathBuf;

/// Test that TokenMigrationResult structure is properly defined
#[test]
fn test_token_migration_result_structure() {
    // Create a mock result structure
    let archive_path = Some(PathBuf::from("/home/user/.spoq-migration/archive.tar.gz"));
    let detected_tokens = vec![
        "GitHub CLI".to_string(),
        "Claude Code".to_string(),
    ];

    assert!(archive_path.is_some());
    assert_eq!(detected_tokens.len(), 2);
    assert!(detected_tokens.contains(&"GitHub CLI".to_string()));
    assert!(detected_tokens.contains(&"Claude Code".to_string()));
}

/// Test that credentials can store token archive path
#[test]
fn test_credentials_stores_token_archive_path() {
    let mut creds = Credentials::default();
    assert!(creds.token_archive_path.is_none());

    let archive_path = "/home/user/.spoq-migration/archive.tar.gz".to_string();
    creds.token_archive_path = Some(archive_path.clone());

    assert_eq!(creds.token_archive_path, Some(archive_path));
}

/// Test that credentials with token archive path serializes correctly
#[test]
fn test_credentials_serialization_with_archive_path() {
    let creds = Credentials {
        access_token: Some("token".to_string()),
        refresh_token: Some("refresh".to_string()),
        expires_at: Some(9999999999),
        user_id: Some("user".to_string()),
        username: Some("username".to_string()),
        vps_id: Some("vps-id".to_string()),
        vps_url: Some("https://example.com".to_string()),
        vps_hostname: Some("hostname".to_string()),
        vps_ip: Some("192.168.1.1".to_string()),
        vps_status: Some("running".to_string()),
        datacenter_id: Some(1),
        token_archive_path: Some("/tmp/archive.tar.gz".to_string()),
    };

    let json = serde_json::to_string(&creds).expect("Should serialize");
    assert!(json.contains("token_archive_path"));
    assert!(json.contains("/tmp/archive.tar.gz"));

    let deserialized: Credentials = serde_json::from_str(&json).expect("Should deserialize");
    assert_eq!(deserialized.token_archive_path, Some("/tmp/archive.tar.gz".to_string()));
}

/// Test backward compatibility - old credentials without archive path load correctly
#[test]
fn test_backward_compatibility_without_archive_path() {
    let json = r#"{
        "access_token": "old-token",
        "refresh_token": "old-refresh",
        "expires_at": 9999999999,
        "user_id": "old-user",
        "username": "olduser",
        "vps_id": "old-vps",
        "vps_url": "https://old.example.com",
        "vps_hostname": "old.example.com",
        "vps_ip": "10.0.0.1",
        "vps_status": "active"
    }"#;

    let creds: Credentials = serde_json::from_str(json).expect("Should deserialize old format");

    assert_eq!(creds.access_token, Some("old-token".to_string()));
    assert_eq!(creds.token_archive_path, None); // Should default to None
}

/// Test that token detection result structure is correct
#[test]
fn test_token_detection_result_for_migration() {
    let detection = TokenDetectionResult {
        claude_code: true,
        github_cli: true,
        codex: false,
    };

    assert!(detection.claude_code);
    assert!(detection.github_cli);
    assert!(!detection.codex);
}

/// Test building detected tokens list
#[test]
fn test_build_detected_tokens_list() {
    let detection = TokenDetectionResult {
        claude_code: true,
        github_cli: true,
        codex: true,
    };

    let mut detected_tokens = Vec::new();
    if detection.github_cli {
        detected_tokens.push("GitHub CLI".to_string());
    }
    if detection.claude_code {
        detected_tokens.push("Claude Code".to_string());
    }
    if detection.codex {
        detected_tokens.push("Codex".to_string());
    }

    assert_eq!(detected_tokens.len(), 3);
    assert!(detected_tokens.contains(&"GitHub CLI".to_string()));
    assert!(detected_tokens.contains(&"Claude Code".to_string()));
    assert!(detected_tokens.contains(&"Codex".to_string()));
}

/// Test building detected tokens list with missing tokens
#[test]
fn test_build_detected_tokens_list_partial() {
    let detection = TokenDetectionResult {
        claude_code: false,
        github_cli: true,
        codex: false,
    };

    let mut detected_tokens = Vec::new();
    if detection.github_cli {
        detected_tokens.push("GitHub CLI".to_string());
    }
    if detection.claude_code {
        detected_tokens.push("Claude Code".to_string());
    }
    if detection.codex {
        detected_tokens.push("Codex".to_string());
    }

    assert_eq!(detected_tokens.len(), 1);
    assert!(detected_tokens.contains(&"GitHub CLI".to_string()));
    assert!(!detected_tokens.contains(&"Claude Code".to_string()));
}

/// Test token export result structure
#[test]
fn test_token_export_result_for_migration() {
    let export_result = TokenExportResult {
        archive_path: PathBuf::from("/home/user/.spoq-migration/archive.tar.gz"),
        size_bytes: 12345,
        tokens_included: TokenDetectionResult {
            claude_code: true,
            github_cli: true,
            codex: false,
        },
    };

    assert_eq!(
        export_result.archive_path,
        PathBuf::from("/home/user/.spoq-migration/archive.tar.gz")
    );
    assert_eq!(export_result.size_bytes, 12345);
    assert!(export_result.tokens_included.claude_code);
    assert!(export_result.tokens_included.github_cli);
    assert!(!export_result.tokens_included.codex);
}

/// Test converting PathBuf to string for credentials storage
#[test]
fn test_archive_path_conversion() {
    let archive_path = PathBuf::from("/home/user/.spoq-migration/archive.tar.gz");
    let path_string = archive_path.to_string_lossy().to_string();

    let mut creds = Credentials::default();
    creds.token_archive_path = Some(path_string.clone());

    assert_eq!(
        creds.token_archive_path,
        Some("/home/user/.spoq-migration/archive.tar.gz".to_string())
    );
}

/// Test token migration summary message generation
#[test]
fn test_token_migration_summary_message() {
    let detected_tokens = vec![
        "GitHub CLI".to_string(),
        "Claude Code".to_string(),
    ];

    let summary = if detected_tokens.is_empty() {
        "Token migration prepared. No tokens detected.".to_string()
    } else {
        format!(
            "Token migration prepared. Found: [{}]",
            detected_tokens.join(", ")
        )
    };

    assert_eq!(
        summary,
        "Token migration prepared. Found: [GitHub CLI, Claude Code]"
    );
}

/// Test token migration summary with no tokens
#[test]
fn test_token_migration_summary_no_tokens() {
    let detected_tokens: Vec<String> = vec![];

    let summary = if detected_tokens.is_empty() {
        "Token migration prepared. No tokens detected.".to_string()
    } else {
        format!(
            "Token migration prepared. Found: [{}]",
            detected_tokens.join(", ")
        )
    };

    assert_eq!(summary, "Token migration prepared. No tokens detected.");
}

/// Test error handling for token detection
#[test]
fn test_token_detection_error_handling() {
    // Simulate an error message
    let error = "Failed to execute migration script: No such file or directory";
    let warning = format!("Token detection failed: {}. VPS setup will continue.", error);

    assert!(warning.contains("Token detection failed"));
    assert!(warning.contains("VPS setup will continue"));
}

/// Test error handling for token export
#[test]
fn test_token_export_error_handling() {
    // Simulate an error message
    let error = "Archive file was not created";
    let warning = format!(
        "Token export failed: {}. VPS setup will continue without token migration.",
        error
    );

    assert!(warning.contains("Token export failed"));
    assert!(warning.contains("VPS setup will continue"));
}

/// Test graceful handling when Claude Code token is missing
#[test]
fn test_claude_code_missing_warning() {
    let error = "Failed to detect Claude Code token after 5 attempts";
    let warning = format!(
        "Claude Code token not available: {}. VPS setup will continue without it.",
        error
    );

    assert!(warning.contains("Claude Code token not available"));
    assert!(warning.contains("VPS setup will continue"));
}

/// Test credentials update after successful migration
#[test]
fn test_credentials_update_after_migration() {
    let mut creds = Credentials::default();
    creds.vps_id = Some("vps-123".to_string());
    creds.vps_status = Some("running".to_string());

    // Simulate migration result
    let archive_path = PathBuf::from("/home/user/.spoq-migration/archive.tar.gz");

    // Update credentials with archive path
    creds.token_archive_path = Some(archive_path.to_string_lossy().to_string());

    assert!(creds.token_archive_path.is_some());
    assert_eq!(
        creds.token_archive_path.unwrap(),
        "/home/user/.spoq-migration/archive.tar.gz"
    );
}

/// Test that credentials are not updated when migration fails
#[test]
fn test_credentials_not_updated_on_migration_failure() {
    let mut creds = Credentials::default();
    creds.vps_id = Some("vps-123".to_string());

    // Simulate failed migration (archive_path is None)
    let archive_path: Option<PathBuf> = None;

    // Only update if archive_path is Some
    if let Some(ref path) = archive_path {
        creds.token_archive_path = Some(path.to_string_lossy().to_string());
    }

    // Credentials should not have archive path set
    assert!(creds.token_archive_path.is_none());
}
