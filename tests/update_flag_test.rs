use std::process::Command;
use std::path::PathBuf;
use serial_test::serial;

// NOTE: These tests are intentionally minimal because running `spoq --update`
// can modify the binary being tested if it connects to the production update server.
// We test only that the flag is recognized and doesn't cause a crash.

// IMPORTANT: These tests can modify the binary under test by installing production updates.
// To prevent test pollution, we back up and restore the binary after testing.

struct BinaryBackup {
    binary_path: String,
    backup_path: PathBuf,
}

impl BinaryBackup {
    fn new(binary_path: &str) -> Option<Self> {
        let backup_path = PathBuf::from(format!("{}.test_backup", binary_path));
        match std::fs::copy(binary_path, &backup_path) {
            Ok(_) => Some(Self {
                binary_path: binary_path.to_string(),
                backup_path,
            }),
            Err(e) => {
                eprintln!("Warning: Failed to create binary backup: {}", e);
                None
            }
        }
    }
}

impl Drop for BinaryBackup {
    fn drop(&mut self) {
        use std::io::Write;

        // Restore from backup if it exists
        if !self.backup_path.exists() {
            // Write diagnostic to stderr which is captured by test framework
            let _ = writeln!(
                std::io::stderr(),
                "BINARY_BACKUP_DROP: Backup does not exist at {}",
                self.backup_path.display()
            );
            return;
        }

        let _ = writeln!(
            std::io::stderr(),
            "BINARY_BACKUP_DROP: Restoring from {}",
            self.backup_path.display()
        );

        // On macOS, we might need to clear the quarantine attribute
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("xattr")
                .args(["-d", "com.apple.quarantine", &self.binary_path])
                .output();
        }

        // Restore the original binary
        match std::fs::copy(&self.backup_path, &self.binary_path) {
            Ok(bytes) => {
                let _ = writeln!(
                    std::io::stderr(),
                    "BINARY_BACKUP_DROP: Restored {} bytes to {}",
                    bytes,
                    self.binary_path
                );
            }
            Err(e) => {
                let _ = writeln!(
                    std::io::stderr(),
                    "BINARY_BACKUP_DROP: ERROR restoring: {}",
                    e
                );
            }
        }

        // Clean up the backup file
        let _ = std::fs::remove_file(&self.backup_path);
    }
}

#[test]
#[serial]
fn test_update_flag_is_recognized() {
    // Build the binary path
    let binary_path = env!("CARGO_BIN_EXE_spoq");

    // Create backup before running update (which might modify the binary)
    // The Drop impl will restore the binary automatically
    let _backup = BinaryBackup::new(binary_path);

    // Run with --update flag
    // WARNING: This can modify the binary if an update is available on the server!
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

    // Binary will be restored by the Drop impl when _backup goes out of scope
}

#[test]
#[serial]
fn test_update_flag_exits_cleanly() {
    // Build the binary path
    let binary_path = env!("CARGO_BIN_EXE_spoq");

    // Create backup before running update
    let _backup = BinaryBackup::new(binary_path);

    // Verify backup was created
    if _backup.is_none() {
        eprintln!("Warning: Could not create backup for test_update_flag_exits_cleanly");
    }

    // Run with --update flag to ensure it doesn't hang
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

    // Binary will be restored by the Drop impl when _backup goes out of scope
}
