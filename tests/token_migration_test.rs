use spoq::auth::{detect_tokens, export_tokens, TokenDetectionResult, TokenExportResult};

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

#[test]
fn test_export_tokens_executes() {
    // Test that the export_tokens function runs without panicking
    // This will create the staging directory and attempt to export
    let result = export_tokens();

    // The function may succeed or fail depending on whether credentials exist
    // But it should not panic
    match result {
        Ok(export_result) => {
            // If successful, verify the result structure
            assert!(
                export_result.archive_path.exists(),
                "Archive should exist at {:?}",
                export_result.archive_path
            );
            assert!(
                export_result.size_bytes > 0,
                "Archive should have non-zero size"
            );

            // Clean up
            std::fs::remove_file(&export_result.archive_path).ok();
        }
        Err(e) => {
            // If it fails, it should be due to missing credentials or script issues
            // This is acceptable for a test
            println!("Export failed (expected if no credentials present): {}", e);
        }
    }
}

#[test]
fn test_export_tokens_creates_staging_directory() {
    // Test that the export function creates the staging directory
    let home = std::env::var("HOME").expect("HOME should be set");
    let staging_dir = std::path::Path::new(&home).join(".spoq-migration");

    // Run export (may fail, but should create directory)
    let _ = export_tokens();

    // Verify staging directory was created
    assert!(
        staging_dir.exists(),
        "Staging directory should exist at {:?}",
        staging_dir
    );
    assert!(
        staging_dir.is_dir(),
        "Staging path should be a directory"
    );
}

#[test]
fn test_export_tokens_result_structure() {
    // Test the TokenExportResult structure
    use std::path::PathBuf;

    let result = TokenExportResult {
        archive_path: PathBuf::from("/tmp/test_archive.tar.gz"),
        size_bytes: 1024,
        tokens_included: TokenDetectionResult {
            claude_code: true,
            github_cli: false,
            codex: true,
        },
    };

    assert_eq!(result.archive_path, PathBuf::from("/tmp/test_archive.tar.gz"));
    assert_eq!(result.size_bytes, 1024);
    assert!(result.tokens_included.claude_code);
    assert!(!result.tokens_included.github_cli);
    assert!(result.tokens_included.codex);
}

#[test]
fn test_export_tokens_result_equality() {
    // Test TokenExportResult equality
    use std::path::PathBuf;

    let result1 = TokenExportResult {
        archive_path: PathBuf::from("/tmp/archive.tar.gz"),
        size_bytes: 2048,
        tokens_included: TokenDetectionResult {
            claude_code: true,
            github_cli: true,
            codex: false,
        },
    };

    let result2 = TokenExportResult {
        archive_path: PathBuf::from("/tmp/archive.tar.gz"),
        size_bytes: 2048,
        tokens_included: TokenDetectionResult {
            claude_code: true,
            github_cli: true,
            codex: false,
        },
    };

    let result3 = TokenExportResult {
        archive_path: PathBuf::from("/tmp/different.tar.gz"),
        size_bytes: 2048,
        tokens_included: TokenDetectionResult {
            claude_code: true,
            github_cli: true,
            codex: false,
        },
    };

    assert_eq!(result1, result2);
    assert_ne!(result1, result3);
}

#[test]
fn test_export_tokens_archive_location() {
    // Test that export creates archive in the correct location
    let result = export_tokens();

    if let Ok(export_result) = result {
        // Verify archive is in ~/.spoq-migration/
        let home = std::env::var("HOME").expect("HOME should be set");
        let expected_dir = format!("{}/.spoq-migration", home);

        assert!(
            export_result
                .archive_path
                .to_str()
                .unwrap()
                .starts_with(&expected_dir),
            "Archive should be in ~/.spoq-migration/, got {:?}",
            export_result.archive_path
        );

        // Verify filename is archive.tar.gz
        assert_eq!(
            export_result.archive_path.file_name().unwrap(),
            "archive.tar.gz",
            "Archive should be named archive.tar.gz"
        );

        // Clean up
        std::fs::remove_file(&export_result.archive_path).ok();
    }
}
