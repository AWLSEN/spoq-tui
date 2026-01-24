use std::process::Command;

#[test]
fn test_version_flag() {
    // Build the binary path
    let binary_path = env!("CARGO_BIN_EXE_spoq");

    // Run with --version flag
    let output = Command::new(binary_path)
        .arg("--version")
        .output()
        .expect("Failed to execute binary");

    // Verify exit code is 0
    assert!(
        output.status.success(),
        "Version flag should exit with code 0"
    );

    // Verify output format
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.starts_with("spoq "),
        "Version output should start with 'spoq '"
    );

    // Verify it contains a version number (e.g., 0.1.4)
    let version_part = stdout.trim().strip_prefix("spoq ").unwrap_or("");
    assert!(
        !version_part.is_empty(),
        "Version output should include version number"
    );
    assert!(
        version_part.chars().any(|c| c.is_ascii_digit()),
        "Version should contain digits"
    );
}

#[test]
fn test_version_matches_cargo_toml() {
    // Build the binary path
    let binary_path = env!("CARGO_BIN_EXE_spoq");

    // Run with --version flag
    let output = Command::new(binary_path)
        .arg("--version")
        .output()
        .expect("Failed to execute binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let version = stdout.trim().strip_prefix("spoq ").unwrap_or("");

    // Get version from Cargo.toml
    let cargo_version = env!("CARGO_PKG_VERSION");

    assert_eq!(
        version, cargo_version,
        "Binary version should match CARGO_PKG_VERSION"
    );
}
