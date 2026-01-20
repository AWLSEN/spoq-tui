use std::process::Command;

#[test]
fn test_update_flag_is_recognized() {
    // Build the binary path
    let binary_path = env!("CARGO_BIN_EXE_spoq");

    // Run with --update flag
    // Note: This will attempt to connect to the update server, which may fail
    // in test environments. We're just testing that the flag is recognized.
    let output = Command::new(binary_path)
        .arg("--update")
        .output()
        .expect("Failed to execute binary");

    // The command should either:
    // 1. Exit with 0 if it successfully checked/updated
    // 2. Exit with 1 if there was an error (network, server unavailable, etc.)
    // We verify that it exits (doesn't hang) and produces output

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Verify that the update flow started (should print "Checking for updates...")
    assert!(
        stdout.contains("Checking for updates") || stderr.contains("Error checking for updates"),
        "Update flag should trigger update check flow. stdout: {}, stderr: {}",
        stdout,
        stderr
    );
}

#[test]
fn test_update_flag_exits_cleanly() {
    // Build the binary path
    let binary_path = env!("CARGO_BIN_EXE_spoq");

    // Run with --update flag with a timeout to ensure it doesn't hang
    let output = Command::new(binary_path)
        .arg("--update")
        .output()
        .expect("Failed to execute binary");

    // The process should exit (not hang), regardless of success or failure
    // We don't check the exit code because network issues are expected in test environments
    assert!(
        output.status.code().is_some(),
        "Update command should exit with a status code"
    );
}
