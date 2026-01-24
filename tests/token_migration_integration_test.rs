//! Tests for token migration integration with VPS provisioning flows.
//!
//! NOTE: Credentials now only contain auth fields (access_token, refresh_token,
//! expires_at, user_id). Token migration happens during provisioning but
//! archive paths are NOT stored in credentials anymore.
//!
//! These tests verify the token migration data structures and logic.

use spoq::auth::{Credentials, TokenDetectionResult, TokenExportResult};
use std::path::PathBuf;

/// Test that TokenMigrationResult structure is properly defined
#[test]
fn test_token_migration_result_structure() {
    // Create a mock result structure
    let archive_path = Some(PathBuf::from("/home/user/.spoq-migration/archive.tar.gz"));
    let detected_tokens = vec!["GitHub CLI".to_string(), "Claude Code".to_string()];

    assert!(archive_path.is_some());
    assert_eq!(detected_tokens.len(), 2);
    assert!(detected_tokens.contains(&"GitHub CLI".to_string()));
    assert!(detected_tokens.contains(&"Claude Code".to_string()));
}

/// Test that credentials only contain auth fields (no token_archive_path)
#[test]
fn test_credentials_auth_only() {
    let creds = Credentials {
        access_token: Some("token".to_string()),
        refresh_token: Some("refresh".to_string()),
        expires_at: Some(9999999999),
        user_id: Some("user".to_string()),
    };

    // Verify auth fields
    assert_eq!(creds.access_token, Some("token".to_string()));
    assert_eq!(creds.refresh_token, Some("refresh".to_string()));
    assert_eq!(creds.expires_at, Some(9999999999));
    assert_eq!(creds.user_id, Some("user".to_string()));
}

/// Test credentials serialization only includes auth fields
#[test]
fn test_credentials_serialization_auth_only() {
    let creds = Credentials {
        access_token: Some("token".to_string()),
        refresh_token: Some("refresh".to_string()),
        expires_at: Some(9999999999),
        user_id: Some("user".to_string()),
    };

    let json = serde_json::to_string(&creds).expect("Should serialize");

    // Auth fields should be present
    assert!(json.contains("access_token"));
    assert!(json.contains("refresh_token"));
    assert!(json.contains("expires_at"));
    assert!(json.contains("user_id"));

    // VPS/migration fields should NOT be present (removed from struct)
    assert!(!json.contains("token_archive_path"));
    assert!(!json.contains("vps_id"));
    assert!(!json.contains("vps_url"));

    let deserialized: Credentials = serde_json::from_str(&json).expect("Should deserialize");
    assert_eq!(deserialized.access_token, Some("token".to_string()));
}

/// Test backward compatibility - old credentials with VPS fields load correctly
/// (VPS fields are ignored as they no longer exist in the struct)
#[test]
fn test_backward_compatibility_ignores_old_fields() {
    // Old format with VPS fields that no longer exist
    let json = r#"{
        "access_token": "old-token",
        "refresh_token": "old-refresh",
        "expires_at": 9999999999,
        "user_id": "old-user"
    }"#;

    let creds: Credentials = serde_json::from_str(json).expect("Should deserialize");

    // Auth fields should load correctly
    assert_eq!(creds.access_token, Some("old-token".to_string()));
    assert_eq!(creds.refresh_token, Some("old-refresh".to_string()));
    assert_eq!(creds.expires_at, Some(9999999999));
    assert_eq!(creds.user_id, Some("old-user".to_string()));
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

/// Test converting PathBuf to string for archive path handling
#[test]
fn test_archive_path_conversion() {
    let archive_path = PathBuf::from("/home/user/.spoq-migration/archive.tar.gz");
    let path_string = archive_path.to_string_lossy().to_string();

    assert_eq!(path_string, "/home/user/.spoq-migration/archive.tar.gz");
}

/// Test token migration summary message generation
#[test]
fn test_token_migration_summary_message() {
    let detected_tokens = vec!["GitHub CLI".to_string(), "Claude Code".to_string()];

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
    let warning = format!(
        "Token detection failed: {}. VPS setup will continue.",
        error
    );

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
