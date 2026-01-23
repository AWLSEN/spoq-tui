//! Integration tests for SSH transfer functionality.
//!
//! These tests verify the SSH transfer mechanism for token migration to VPS.

use spoq::auth::{
    transfer_tokens_to_vps, transfer_tokens_with_credentials, SshTransferError,
    TokenTransferResult, VpsConnectionInfo,
};

// ===========================================
// VpsConnectionInfo Tests
// ===========================================

#[test]
fn test_vps_connection_info_new_default_username() {
    let conn = VpsConnectionInfo::new("10.0.0.1".to_string(), "secret123".to_string());

    assert_eq!(conn.vps_ip, "10.0.0.1");
    assert_eq!(conn.ssh_username, "root"); // Default username should be "root"
    assert_eq!(conn.ssh_password, "secret123");
}

#[test]
fn test_vps_connection_info_with_custom_username() {
    let conn = VpsConnectionInfo::with_username(
        "192.168.1.100".to_string(),
        "admin".to_string(),
        "password456".to_string(),
    );

    assert_eq!(conn.vps_ip, "192.168.1.100");
    assert_eq!(conn.ssh_username, "admin");
    assert_eq!(conn.ssh_password, "password456");
}

#[test]
fn test_vps_connection_info_clone() {
    let conn = VpsConnectionInfo::new("1.2.3.4".to_string(), "pass".to_string());
    let cloned = conn.clone();

    assert_eq!(cloned.vps_ip, conn.vps_ip);
    assert_eq!(cloned.ssh_username, conn.ssh_username);
    assert_eq!(cloned.ssh_password, conn.ssh_password);
}

// ===========================================
// SshTransferError Tests
// ===========================================

#[test]
fn test_ssh_transfer_error_connection_refused_display() {
    let error = SshTransferError::ConnectionRefused("port 22 refused".to_string());
    let display = format!("{}", error);

    assert!(display.contains("connection refused"));
    assert!(display.contains("port 22 refused"));
}

#[test]
fn test_ssh_transfer_error_authentication_failed_display() {
    let error = SshTransferError::AuthenticationFailed("bad password".to_string());
    let display = format!("{}", error);

    assert!(display.contains("authentication failed"));
    assert!(display.contains("bad password"));
}

#[test]
fn test_ssh_transfer_error_network_timeout_display() {
    let error = SshTransferError::NetworkTimeout("host unreachable".to_string());
    let display = format!("{}", error);

    assert!(display.contains("timeout"));
    assert!(display.contains("host unreachable"));
}

#[test]
fn test_ssh_transfer_error_sshpass_not_installed_display() {
    let error = SshTransferError::SshpassNotInstalled("install with brew".to_string());
    let display = format!("{}", error);

    assert!(display.contains("sshpass"));
    assert!(display.contains("install with brew"));
}

#[test]
fn test_ssh_transfer_error_transfer_failed_display() {
    let error = SshTransferError::TransferFailed("tar error".to_string());
    let display = format!("{}", error);

    assert!(display.contains("failed"));
    assert!(display.contains("tar error"));
}

#[test]
fn test_ssh_transfer_error_import_failed_display() {
    let error = SshTransferError::ImportFailed("script error".to_string());
    let display = format!("{}", error);

    assert!(display.contains("Import failed"));
    assert!(display.contains("script error"));
}

#[test]
fn test_ssh_transfer_error_missing_credentials_display() {
    let error = SshTransferError::MissingCredentials("no HOME".to_string());
    let display = format!("{}", error);

    assert!(display.contains("Missing credentials"));
    assert!(display.contains("no HOME"));
}

#[test]
fn test_ssh_transfer_error_staging_not_found_display() {
    let error = SshTransferError::StagingNotFound("directory missing".to_string());
    let display = format!("{}", error);

    assert!(display.contains("not found"));
    assert!(display.contains("directory missing"));
}

#[test]
fn test_ssh_transfer_error_is_std_error() {
    let error = SshTransferError::TransferFailed("test".to_string());

    // Verify it implements std::error::Error
    let _: &dyn std::error::Error = &error;
}

// ===========================================
// TokenTransferResult Tests
// ===========================================

#[test]
fn test_token_transfer_result_structure() {
    let result = TokenTransferResult {
        vps_ip: "10.0.0.1".to_string(),
        ssh_username: "root".to_string(),
        import_successful: true,
        import_message: Some("Imported 3 tokens".to_string()),
    };

    assert_eq!(result.vps_ip, "10.0.0.1");
    assert_eq!(result.ssh_username, "root");
    assert!(result.import_successful);
    assert_eq!(result.import_message, Some("Imported 3 tokens".to_string()));
}

#[test]
fn test_token_transfer_result_without_message() {
    let result = TokenTransferResult {
        vps_ip: "192.168.1.1".to_string(),
        ssh_username: "admin".to_string(),
        import_successful: false,
        import_message: None,
    };

    assert_eq!(result.vps_ip, "192.168.1.1");
    assert_eq!(result.ssh_username, "admin");
    assert!(!result.import_successful);
    assert!(result.import_message.is_none());
}

#[test]
fn test_token_transfer_result_clone() {
    let result = TokenTransferResult {
        vps_ip: "1.2.3.4".to_string(),
        ssh_username: "user".to_string(),
        import_successful: true,
        import_message: Some("done".to_string()),
    };

    let cloned = result.clone();
    assert_eq!(cloned, result);
}

// ===========================================
// Transfer Function Tests
// ===========================================

#[test]
fn test_transfer_tokens_to_vps_fails_without_staging_dir() {
    // Ensure staging directory doesn't exist
    let home = std::env::var("HOME").expect("HOME should be set");
    let staging_dir = std::path::Path::new(&home).join(".spoq-migration");
    let _ = std::fs::remove_dir_all(&staging_dir);

    let conn = VpsConnectionInfo::new("192.168.1.1".to_string(), "password".to_string());

    let result = transfer_tokens_to_vps(&conn);

    assert!(result.is_err());
    match result.unwrap_err() {
        SshTransferError::StagingNotFound(msg) => {
            assert!(msg.contains("does not exist") || msg.contains("Staging"));
        }
        other => panic!("Expected StagingNotFound error, got {:?}", other),
    }
}

#[test]
fn test_transfer_tokens_to_vps_fails_with_empty_staging() {
    // Create an empty staging directory
    let home = std::env::var("HOME").expect("HOME should be set");
    let staging_dir = std::path::Path::new(&home).join(".spoq-migration");

    // Remove if exists and recreate as empty
    let _ = std::fs::remove_dir_all(&staging_dir);
    std::fs::create_dir_all(&staging_dir).expect("Failed to create staging directory");

    let conn = VpsConnectionInfo::new("10.0.0.1".to_string(), "pass".to_string());

    let result = transfer_tokens_to_vps(&conn);

    // Clean up
    let _ = std::fs::remove_dir_all(&staging_dir);

    assert!(result.is_err());
    match result.unwrap_err() {
        SshTransferError::StagingNotFound(msg) => {
            assert!(msg.contains("empty") || msg.contains("No tokens"));
        }
        other => panic!("Expected StagingNotFound error, got {:?}", other),
    }
}

#[test]
fn test_transfer_tokens_with_credentials_uses_default_username() {
    // Ensure staging directory doesn't exist
    let home = std::env::var("HOME").expect("HOME should be set");
    let staging_dir = std::path::Path::new(&home).join(".spoq-migration");
    let _ = std::fs::remove_dir_all(&staging_dir);

    // Using None for username should use default "root"
    let result = transfer_tokens_with_credentials("192.168.1.1", "password", None);

    // Should fail at staging check, not at username parsing
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), SshTransferError::StagingNotFound(_)));
}

#[test]
fn test_transfer_tokens_with_credentials_uses_custom_username() {
    // Ensure staging directory doesn't exist
    let home = std::env::var("HOME").expect("HOME should be set");
    let staging_dir = std::path::Path::new(&home).join(".spoq-migration");
    let _ = std::fs::remove_dir_all(&staging_dir);

    // Using custom username
    let result = transfer_tokens_with_credentials("192.168.1.1", "password", Some("root"));

    // Should fail at staging check
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), SshTransferError::StagingNotFound(_)));
}

// ===========================================
// IPv6 Address Tests
// ===========================================

#[test]
fn test_vps_connection_info_ipv6() {
    let conn = VpsConnectionInfo::new(
        "2001:db8::1".to_string(),
        "password".to_string(),
    );

    assert_eq!(conn.vps_ip, "2001:db8::1");
    assert_eq!(conn.ssh_username, "root");
}

#[test]
fn test_vps_connection_info_ipv6_full() {
    let conn = VpsConnectionInfo::new(
        "2001:0db8:85a3:0000:0000:8a2e:0370:7334".to_string(),
        "pass".to_string(),
    );

    assert_eq!(conn.vps_ip, "2001:0db8:85a3:0000:0000:8a2e:0370:7334");
}

// ===========================================
// Password Edge Case Tests
// ===========================================

#[test]
fn test_vps_connection_info_special_chars_in_password() {
    // Test that special characters in password are accepted
    let passwords = vec![
        "p@ssw0rd!",
        "pass'word",
        "pass\"word",
        "pass$word",
        "pass`word",
        "pass\\word",
        "pass word",
        "!@#$%^&*()",
    ];

    for password in passwords {
        let conn = VpsConnectionInfo::new("1.2.3.4".to_string(), password.to_string());
        assert_eq!(conn.ssh_password, password);
    }
}

// ===========================================
// Error Equality Tests
// ===========================================

#[test]
fn test_ssh_transfer_error_equality() {
    let err1 = SshTransferError::ConnectionRefused("same message".to_string());
    let err2 = SshTransferError::ConnectionRefused("same message".to_string());
    let err3 = SshTransferError::ConnectionRefused("different message".to_string());
    let err4 = SshTransferError::AuthenticationFailed("same message".to_string());

    assert_eq!(err1, err2);
    assert_ne!(err1, err3); // Different message
    assert_ne!(err1, err4); // Different variant
}

#[test]
fn test_token_transfer_result_equality() {
    let result1 = TokenTransferResult {
        vps_ip: "1.2.3.4".to_string(),
        ssh_username: "root".to_string(),
        import_successful: true,
        import_message: None,
    };

    let result2 = TokenTransferResult {
        vps_ip: "1.2.3.4".to_string(),
        ssh_username: "root".to_string(),
        import_successful: true,
        import_message: None,
    };

    let result3 = TokenTransferResult {
        vps_ip: "5.6.7.8".to_string(),
        ssh_username: "root".to_string(),
        import_successful: true,
        import_message: None,
    };

    assert_eq!(result1, result2);
    assert_ne!(result1, result3);
}
