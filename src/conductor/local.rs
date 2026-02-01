//! Local conductor management: download, start, health check.
//!
//! Handles downloading the conductor binary, starting it as a local process,
//! and waiting for it to become healthy on localhost.

use std::path::PathBuf;

const CONDUCTOR_DOWNLOAD_URL: &str = "https://download.spoq.dev/conductor/download";
const DEFAULT_PORT: u16 = 8000;

/// Path to conductor binary: ~/.spoq/bin/conductor
pub fn conductor_binary_path() -> Result<PathBuf, std::io::Error> {
    let home = dirs::home_dir().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "Home directory not found")
    })?;
    Ok(home.join(".spoq").join("bin").join("conductor"))
}

/// Check if conductor binary exists AND has correct architecture.
/// Returns false if binary is wrong architecture (triggers re-download).
pub fn conductor_exists() -> bool {
    let path = match conductor_binary_path() {
        Ok(p) => p,
        Err(_) => return false,
    };

    if !path.exists() {
        return false;
    }

    // Verify architecture matches current platform
    match verify_binary_architecture(&path) {
        Ok(true) => true,
        Ok(false) => {
            tracing::warn!("Conductor binary has wrong architecture, will re-download");
            // Delete the wrong binary so it gets re-downloaded
            let _ = std::fs::remove_file(&path);
            false
        }
        Err(e) => {
            tracing::warn!("Failed to verify conductor architecture: {}, assuming valid", e);
            true // Assume valid if we can't check
        }
    }
}

/// Verify that a binary matches the current platform's architecture.
/// Returns Ok(true) if matches, Ok(false) if wrong architecture.
fn verify_binary_architecture(path: &std::path::Path) -> Result<bool, std::io::Error> {
    use std::io::Read;

    let mut file = std::fs::File::open(path)?;
    let mut magic = [0u8; 4];
    file.read_exact(&mut magic)?;

    // Detect binary format from magic bytes
    let is_correct = match &magic {
        // Mach-O (macOS)
        [0xCF, 0xFA, 0xED, 0xFE] | [0xFE, 0xED, 0xFA, 0xCF] => {
            // Mach-O 64-bit - correct for macOS
            cfg!(target_os = "macos")
        }
        [0xCA, 0xFE, 0xBA, 0xBE] => {
            // Mach-O universal binary - correct for macOS
            cfg!(target_os = "macos")
        }
        // ELF (Linux)
        [0x7F, b'E', b'L', b'F'] => {
            // ELF binary - correct for Linux
            cfg!(target_os = "linux")
        }
        _ => {
            // Unknown format, assume incorrect
            false
        }
    };

    Ok(is_correct)
}

/// Download conductor binary for current platform.
/// Uses same platform detection as CLI self-updater.
pub async fn download_conductor() -> Result<PathBuf, String> {
    use crate::update::detect_platform;

    let platform = detect_platform().map_err(|e| format!("Unsupported platform: {}", e))?;
    let url = format!("{}/{}", CONDUCTOR_DOWNLOAD_URL, platform.as_str());
    let path = conductor_binary_path().map_err(|e| e.to_string())?;

    // Create ~/.spoq/bin/
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    tracing::info!("Downloading conductor from {}", url);

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Download failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Download failed: HTTP {}", response.status()));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;

    if bytes.len() < 100 * 1024 {
        return Err("Downloaded file too small — possibly corrupted".to_string());
    }

    // Write to temp file, then rename for atomicity
    let temp_path = path.with_extension("tmp");
    std::fs::write(&temp_path, &bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&temp_path, &path).map_err(|e| e.to_string())?;

    // chmod +x
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
    }

    tracing::info!("Conductor downloaded to {:?} ({} bytes)", path, bytes.len());
    Ok(path)
}

/// Generate a random JWT secret (>= 32 chars) using two UUID v4s.
fn generate_jwt_secret() -> String {
    let a = uuid::Uuid::new_v4().simple().to_string();
    let b = uuid::Uuid::new_v4().simple().to_string();
    format!("{}{}", a, b) // 64 hex chars
}

/// Start conductor process on localhost with required env vars.
///
/// The conductor runs as a background daemon — it is NOT killed when the
/// returned `Child` handle is dropped. This means:
/// - Preflight can start it and return without killing it
/// - On next startup, `is_running()` detects it and skips re-start
/// - User can stop it manually or by switching to Remote mode
pub async fn start_conductor(
    port: u16,
    owner_id: &str,
) -> Result<tokio::process::Child, String> {
    let binary = conductor_binary_path().map_err(|e| e.to_string())?;

    if !binary.exists() {
        return Err("Conductor binary not found. Download it first.".to_string());
    }

    let jwt_secret = generate_jwt_secret();

    let child = tokio::process::Command::new(&binary)
        .env("CONDUCTOR_SERVER__HOST", "127.0.0.1")
        .env("CONDUCTOR_SERVER__PORT", port.to_string())
        .env("CONDUCTOR_AUTH__JWT_SECRET", &jwt_secret)
        .env("CONDUCTOR_AUTH__OWNER_ID", owner_id)
        .env("CONDUCTOR_SKIP_REGISTRATION", "1")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(false)
        .spawn()
        .map_err(|e| format!("Failed to start conductor: {}", e))?;

    tracing::info!(
        "Conductor started on port {} (pid: {:?})",
        port,
        child.id()
    );
    Ok(child)
}

/// Wait for conductor /health endpoint to return 200.
pub async fn wait_for_health(port: u16, timeout_secs: u64) -> Result<(), String> {
    let url = format!("http://127.0.0.1:{}/health", port);
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(timeout_secs);

    loop {
        if start.elapsed() > timeout {
            return Err(format!(
                "Timeout ({}s) waiting for conductor at {}",
                timeout_secs, url
            ));
        }
        if let Ok(resp) = reqwest::get(&url).await {
            if resp.status().is_success() {
                tracing::info!("Conductor healthy at {}", url);
                return Ok(());
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
}

/// Check if conductor is already running on the given port.
pub async fn is_running(port: u16) -> bool {
    let url = format!("http://127.0.0.1:{}/health", port);
    reqwest::get(&url)
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

/// Default port for local conductor.
pub fn default_port() -> u16 {
    DEFAULT_PORT
}
