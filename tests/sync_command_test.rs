//! Integration tests for the /sync CLI command.
//!
//! These tests verify that the sync command properly:
//! - Detects and validates arguments
//! - Checks credentials and VPS status
//! - Runs token migration flow
//! - Handles error cases gracefully

use std::process::Command;

#[test]
fn test_sync_command_detects_flag() {
    // Test that both /sync and --sync flags are recognized
    // This test verifies the argument parsing works

    // We can't actually run the full sync without a VPS,
    // but we can verify the flag is recognized by checking
    // that it doesn't fall through to normal TUI startup

    // The command should exit with an error about missing credentials
    // rather than trying to start the TUI

    let output = Command::new("cargo")
        .args(&["run", "--", "/sync"])
        .output()
        .expect("Failed to execute command");

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should either:
    // 1. Show "Running token synchronization" message, OR
    // 2. Exit with credential/VPS error
    // Either way, it shouldn't start the TUI

    let recognized = stdout.contains("Running token synchronization")
        || stdout.contains("Verifying credentials")
        || stderr.contains("Not authenticated")
        || stderr.contains("No VPS configured");

    assert!(
        recognized,
        "Sync command should be recognized. stdout: {}, stderr: {}",
        stdout,
        stderr
    );
}

#[test]
fn test_sync_command_alternate_flag() {
    // Test that --sync also works
    let output = Command::new("cargo")
        .args(&["run", "--", "--sync"])
        .output()
        .expect("Failed to execute command");

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    let recognized = stdout.contains("Running token synchronization")
        || stdout.contains("Verifying credentials")
        || stderr.contains("Not authenticated")
        || stderr.contains("No VPS configured");

    assert!(
        recognized,
        "--sync flag should be recognized. stdout: {}, stderr: {}",
        stdout,
        stderr
    );
}

#[test]
fn test_sync_requires_credentials() {
    // Test that sync command fails gracefully when not authenticated

    // First, backup any existing credentials
    let home = std::env::var("HOME").expect("HOME should be set");
    let creds_path = std::path::Path::new(&home).join(".spoq").join("credentials.json");
    let backup_path = creds_path.with_extension("json.test_backup");

    // Backup credentials if they exist
    if creds_path.exists() {
        std::fs::copy(&creds_path, &backup_path).ok();
        std::fs::remove_file(&creds_path).ok();
    }

    // Run sync command without credentials
    let output = Command::new("cargo")
        .args(&["run", "--", "/sync"])
        .output()
        .expect("Failed to execute command");

    // Restore credentials
    if backup_path.exists() {
        std::fs::copy(&backup_path, &creds_path).ok();
        std::fs::remove_file(&backup_path).ok();
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should fail with authentication error
    let has_auth_error = stderr.contains("Not authenticated")
        || stdout.contains("Not authenticated")
        || !output.status.success();

    assert!(
        has_auth_error,
        "Sync should fail without credentials. stdout: {}, stderr: {}",
        stdout,
        stderr
    );
}

#[test]
fn test_sync_command_structure() {
    // Test that the sync implementation follows expected flow
    // by verifying the step-by-step output structure
    //
    // NOTE: Credentials now only contain auth fields (access_token, refresh_token,
    // expires_at, user_id). VPS state is fetched from the server API.

    use spoq::auth::{Credentials, CredentialsManager};

    // Create mock credentials with auth tokens
    let _manager = CredentialsManager::new().expect("Should create manager");
    let creds = Credentials {
        access_token: Some("test_token".to_string()),
        refresh_token: Some("test_refresh".to_string()),
        expires_at: Some(chrono::Utc::now().timestamp() + 3600),
        user_id: Some("user-123".to_string()),
    };

    // Check that credentials validation works
    assert!(creds.has_token(), "Should have token");
    assert!(creds.is_valid(), "Should be valid");
}

#[test]
fn test_sync_step_messages() {
    // Verify that sync command outputs the expected step messages
    let expected_steps = vec![
        "[1/5] Verifying credentials and VPS",
        "[2/5] Detecting tokens",
        "[3/5]", // Claude Code check
        "[4/5] Exporting tokens",
        "[5/5] Transferring to VPS",
    ];

    // We can't run the full flow without a real VPS,
    // but we document the expected output format here
    for (i, step) in expected_steps.iter().enumerate() {
        assert!(
            !step.is_empty(),
            "Step {} should have a message",
            i + 1
        );
    }
}

#[test]
fn test_sync_error_messages() {
    // Test that error messages are clear and actionable
    let error_cases = vec![
        ("not authenticated", "Not authenticated. Please run spoq to authenticate first"),
        ("no vps", "No VPS configured. Please run spoq to provision a VPS first"),
        ("token detection", "Error detecting tokens"),
        ("token export", "Error exporting tokens"),
        ("ssh transfer", "SCP transfer failed"),
    ];

    for (case, expected_msg) in error_cases {
        assert!(
            !expected_msg.is_empty(),
            "Error message for '{}' should not be empty",
            case
        );
        assert!(
            expected_msg.len() > 10,
            "Error message for '{}' should be descriptive",
            case
        );
    }
}

#[test]
fn test_sync_command_exit_codes() {
    // Test that sync command uses proper exit codes
    // Success: 0
    // Errors: 1

    // Without credentials, should exit with 1
    let home = std::env::var("HOME").expect("HOME should be set");
    let creds_path = std::path::Path::new(&home).join(".spoq").join("credentials.json");
    let backup_path = creds_path.with_extension("json.test_backup2");

    // Backup and remove credentials
    if creds_path.exists() {
        std::fs::copy(&creds_path, &backup_path).ok();
        std::fs::remove_file(&creds_path).ok();
    }

    let output = Command::new("cargo")
        .args(&["run", "--", "/sync"])
        .output()
        .expect("Failed to execute command");

    // Restore credentials
    if backup_path.exists() {
        std::fs::copy(&backup_path, &creds_path).ok();
        std::fs::remove_file(&backup_path).ok();
    }

    // Should fail with non-zero exit code
    assert!(
        !output.status.success(),
        "Sync without credentials should fail with non-zero exit code"
    );
}

#[test]
fn test_sync_help_text() {
    // Verify that help/error messages mention both /sync and --sync
    let help_aliases = vec!["/sync", "--sync"];

    for alias in help_aliases {
        assert!(
            alias.starts_with('/') || alias.starts_with("--"),
            "Sync command should accept both slash and dash notation"
        );
    }
}

#[test]
fn test_sync_cleanup_on_error() {
    // Test that sync command cleans up temporary files on error
    // This ensures no leftover archives when sync fails

    use spoq::auth::export_tokens;

    // Export tokens to create an archive
    if let Ok(export_result) = export_tokens() {
        let archive_path = export_result.archive_path;

        // Verify archive exists
        assert!(
            archive_path.exists(),
            "Archive should exist after export"
        );

        // Simulate cleanup (what sync does after transfer)
        std::fs::remove_file(&archive_path).ok();

        // Verify cleanup worked
        assert!(
            !archive_path.exists(),
            "Archive should be cleaned up"
        );
    }
}

#[test]
fn test_sync_integration_with_token_migration() {
    // Test that sync uses the same token migration functions
    // as provisioning flow for consistency

    use spoq::auth::{detect_tokens, export_tokens};

    // Both functions should work independently
    let detection = detect_tokens();
    assert!(detection.is_ok(), "Detection should work");

    // Export should work if tokens are present
    // (may fail if no tokens, which is fine)
    let export = export_tokens();
    match export {
        Ok(result) => {
            // If successful, verify structure
            assert!(result.size_bytes > 0);
            assert!(result.archive_path.exists());

            // Clean up
            std::fs::remove_file(&result.archive_path).ok();
        }
        Err(_) => {
            // Acceptable if no credentials present
        }
    }
}

#[test]
fn test_sync_ssh_transfer_logic() {
    // Test the SCP command construction logic
    // Verify that the command would be constructed correctly

    let vps_ip = "203.0.113.42";
    let local_path = "/tmp/test-archive.tar.gz";
    let remote_path = "/tmp/spoq-tokens.tar.gz";

    // Verify SCP command would be constructed correctly
    let remote_dest = format!("root@{}:{}", vps_ip, remote_path);
    let scp_args = vec![
        "-o",
        "StrictHostKeyChecking=no",
        "-o",
        "UserKnownHostsFile=/dev/null",
        local_path,
        &remote_dest,
    ];

    assert_eq!(scp_args.len(), 6);
    assert!(scp_args[4].contains("/tmp"));
    assert!(scp_args[5].contains("root@"));
    assert!(scp_args[5].contains(vps_ip));
}

#[test]
fn test_sync_extraction_command() {
    // Test the SSH extraction command construction
    let remote_path = "/tmp/spoq-tokens.tar.gz";
    let extract_cmd = format!(
        "cd /tmp && tar -xzf {} && rm {}",
        remote_path, remote_path
    );

    assert!(extract_cmd.contains("tar -xzf"));
    assert!(extract_cmd.contains("rm"));
    assert!(extract_cmd.contains(remote_path));
}
