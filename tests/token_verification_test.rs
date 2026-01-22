use spoq::auth::token_verification::{
    LocalTokenVerification, TokenVerificationError, VpsTokenVerification,
};

/// Test LocalTokenVerification structure
#[test]
fn test_local_token_verification_structure() {
    let verification = LocalTokenVerification {
        claude_code_present: true,
        github_cli_present: false,
        codex_present: true,
        all_required_present: false, // Because github_cli is missing
    };

    assert!(verification.claude_code_present);
    assert!(!verification.github_cli_present);
    assert!(verification.codex_present);
    assert!(!verification.all_required_present); // Should be false when required token missing
}

/// Test all_required_present flag when both required tokens present
#[test]
fn test_local_verification_all_required_present() {
    let verification = LocalTokenVerification {
        claude_code_present: true,
        github_cli_present: true,
        codex_present: false, // Optional token, doesn't affect required
        all_required_present: true,
    };

    assert!(verification.all_required_present);
}

/// Test all_required_present flag when Claude Code missing
#[test]
fn test_local_verification_missing_claude_code() {
    let verification = LocalTokenVerification {
        claude_code_present: false,
        github_cli_present: true,
        codex_present: true,
        all_required_present: false,
    };

    assert!(!verification.all_required_present);
    assert!(!verification.claude_code_present);
}

/// Test all_required_present flag when GitHub CLI missing
#[test]
fn test_local_verification_missing_github_cli() {
    let verification = LocalTokenVerification {
        claude_code_present: true,
        github_cli_present: false,
        codex_present: true,
        all_required_present: false,
    };

    assert!(!verification.all_required_present);
    assert!(!verification.github_cli_present);
}

/// Test VpsTokenVerification structure
#[test]
fn test_vps_token_verification_structure() {
    let verification = VpsTokenVerification {
        claude_code_works: true,
        github_cli_works: false,
        ssh_error: None,
    };

    assert!(verification.claude_code_works);
    assert!(!verification.github_cli_works);
    assert!(verification.ssh_error.is_none());
}

/// Test VpsTokenVerification with SSH error
#[test]
fn test_vps_verification_with_ssh_error() {
    let verification = VpsTokenVerification {
        claude_code_works: false,
        github_cli_works: false,
        ssh_error: Some("Connection refused".to_string()),
    };

    assert!(!verification.claude_code_works);
    assert!(!verification.github_cli_works);
    assert!(verification.ssh_error.is_some());
    assert_eq!(
        verification.ssh_error.unwrap(),
        "Connection refused".to_string()
    );
}

/// Test TokenVerificationError enum variants
#[test]
fn test_verification_error_detection_failed() {
    let error = TokenVerificationError::DetectionFailed("Script failed".to_string());
    let error_string = format!("{}", error);
    assert!(error_string.contains("Token detection failed"));
    assert!(error_string.contains("Script failed"));
}

#[test]
fn test_verification_error_ssh_connection_failed() {
    let error = TokenVerificationError::SshConnectionFailed("Connection refused".to_string());
    let error_string = format!("{}", error);
    assert!(error_string.contains("SSH connection failed"));
    assert!(error_string.contains("Connection refused"));
}

#[test]
fn test_verification_error_ssh_timeout() {
    let error = TokenVerificationError::SshCommandTimeout("Timeout after 30s".to_string());
    let error_string = format!("{}", error);
    assert!(error_string.contains("SSH command timed out"));
    assert!(error_string.contains("Timeout after 30s"));
}

#[test]
fn test_verification_error_sshpass_not_installed() {
    let error = TokenVerificationError::SshpassNotInstalled("Install sshpass".to_string());
    let error_string = format!("{}", error);
    assert!(error_string.contains("sshpass not installed"));
    assert!(error_string.contains("Install sshpass"));
}

/// Test Clone trait for LocalTokenVerification
#[test]
fn test_local_verification_clone() {
    let original = LocalTokenVerification {
        claude_code_present: true,
        github_cli_present: true,
        codex_present: false,
        all_required_present: true,
    };

    let cloned = original.clone();
    assert_eq!(cloned.claude_code_present, original.claude_code_present);
    assert_eq!(cloned.github_cli_present, original.github_cli_present);
    assert_eq!(cloned.codex_present, original.codex_present);
    assert_eq!(
        cloned.all_required_present,
        original.all_required_present
    );
}

/// Test Clone trait for VpsTokenVerification
#[test]
fn test_vps_verification_clone() {
    let original = VpsTokenVerification {
        claude_code_works: true,
        github_cli_works: false,
        ssh_error: Some("Test error".to_string()),
    };

    let cloned = original.clone();
    assert_eq!(cloned.claude_code_works, original.claude_code_works);
    assert_eq!(cloned.github_cli_works, original.github_cli_works);
    assert_eq!(cloned.ssh_error, original.ssh_error);
}

/// Test Clone trait for TokenVerificationError
#[test]
fn test_verification_error_clone() {
    let original = TokenVerificationError::DetectionFailed("Test".to_string());
    let cloned = original.clone();

    let original_str = format!("{}", original);
    let cloned_str = format!("{}", cloned);
    assert_eq!(original_str, cloned_str);
}

/// Test Debug trait for LocalTokenVerification
#[test]
fn test_local_verification_debug() {
    let verification = LocalTokenVerification {
        claude_code_present: true,
        github_cli_present: true,
        codex_present: false,
        all_required_present: true,
    };

    let debug_output = format!("{:?}", verification);
    assert!(debug_output.contains("LocalTokenVerification"));
    assert!(debug_output.contains("claude_code_present"));
}

/// Test Debug trait for VpsTokenVerification
#[test]
fn test_vps_verification_debug() {
    let verification = VpsTokenVerification {
        claude_code_works: true,
        github_cli_works: true,
        ssh_error: None,
    };

    let debug_output = format!("{:?}", verification);
    assert!(debug_output.contains("VpsTokenVerification"));
    assert!(debug_output.contains("claude_code_works"));
}

/// Test Debug trait for TokenVerificationError
#[test]
fn test_verification_error_debug() {
    let error = TokenVerificationError::DetectionFailed("Test".to_string());
    let debug_output = format!("{:?}", error);
    assert!(debug_output.contains("DetectionFailed"));
}

/// Test error implements std::error::Error trait
#[test]
fn test_verification_error_is_error_trait() {
    let error: Box<dyn std::error::Error> =
        Box::new(TokenVerificationError::DetectionFailed("Test".to_string()));
    let _error_ref: &dyn std::error::Error = error.as_ref();
    // If this compiles, the trait is implemented correctly
}
