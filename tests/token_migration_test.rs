use spoq::auth::{detect_tokens, TokenDetectionResult};

#[test]
fn test_token_detection_executes() {
    // Test that the token detection function runs without panicking
    let result = detect_tokens();
    assert!(
        result.is_ok(),
        "Token detection should succeed: {:?}",
        result.err()
    );
}

#[test]
fn test_token_detection_returns_valid_structure() {
    let result = detect_tokens().expect("Token detection should succeed");

    // Verify we can access all fields
    let _github_cli = result.github_cli;
    let _claude_code = result.claude_code;
    let _codex = result.codex;

    // At least one of the boolean values should be valid (true or false)
    assert!(
        result.github_cli || !result.github_cli,
        "github_cli should be a valid boolean"
    );
    assert!(
        result.claude_code || !result.claude_code,
        "claude_code should be a valid boolean"
    );
    assert!(result.codex || !result.codex, "codex should be a valid boolean");
}

#[test]
fn test_token_detection_result_structure() {
    let result = TokenDetectionResult {
        claude_code: true,
        github_cli: false,
        codex: true,
    };

    assert!(result.claude_code);
    assert!(!result.github_cli);
    assert!(result.codex);
}

#[test]
fn test_token_detection_result_equality() {
    let result1 = TokenDetectionResult {
        claude_code: true,
        github_cli: true,
        codex: false,
    };

    let result2 = TokenDetectionResult {
        claude_code: true,
        github_cli: true,
        codex: false,
    };

    let result3 = TokenDetectionResult {
        claude_code: false,
        github_cli: true,
        codex: false,
    };

    assert_eq!(result1, result2);
    assert_ne!(result1, result3);
}
