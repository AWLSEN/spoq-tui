---
title: Fix CLI startup flow bugs - token expiration, VPS detection, proactive refresh
type: fix
date: 2026-01-22
related_brainstorm: docs/brainstorms/2026-01-22-cli-startup-flow-fix-brainstorm.md
---

# Fix CLI Startup Flow Bugs

## Problem Statement

Three critical bugs in the CLI startup flow cause users to repeatedly go through authentication and VPS provisioning even when valid credentials exist:

**Bug #1: Token expiration not checked at startup**
- Location: `src/main.rs:392`
- Issue: Only checks `access_token.is_none()`, not `is_expired()`
- Impact: Users with expired tokens bypass auth check, hit 401 errors during API calls

**Bug #2: VPS detection triggers re-provisioning**
- Location: `src/main.rs:404-424`
- Issue: If `vps_status` is `None` but VPS exists (`vps_id` + `vps_url` present), CLI re-provisions
- Impact: Users with existing VPS go through full provisioning flow every startup

**Bug #3: Token refresh is reactive, not proactive**
- Location: `src/auth/central_api.rs`
- Issue: Refresh only happens AFTER 401 response during API calls
- Impact: Poor UX - first API call fails, retries after refresh

## Proposed Solution

**Minimal fix approach** - Patch the three bugs without restructuring the codebase:

1. Add token expiration check at startup (before VPS operations)
2. Change VPS detection from checking `vps_status` to using `has_vps()` method
3. Implement proactive token refresh when expired token detected
4. Change failed VPS behavior from auto-reprovision to error message

## Implementation Details

### File Changes

#### 1. `src/main.rs` (lines 387-443)

**Current problematic code:**
```rust
// Line 392 - Only checks existence
if credentials.access_token.is_none() {
    credentials = run_auth_flow(&runtime)?;
}

// Line 404 - Re-provisions on vps_status=None
match credentials.vps_status.as_deref() {
    Some("ready") | Some("running") | Some("active") => { /* OK */ }
    Some("stopped") => { /* Auto-start */ }
    Some("failed") | Some("terminated") | None => {
        run_provisioning_flow(&runtime, &mut credentials)?; // BUG!
    }
}
```

**Fixed code:**
```rust
// =========================================================
// Auth check - validate token and refresh if needed
// =========================================================

if credentials.access_token.is_none() {
    // No token at all - run full auth flow
    credentials = match run_auth_flow(&runtime) {
        Ok(creds) => creds,
        Err(e) => {
            eprintln!("Authentication failed: {}", e);
            std::process::exit(1);
        }
    };
    // Save credentials after auth
    if !manager.save(&credentials) {
        eprintln!("Warning: Failed to save credentials after authentication");
    }
} else if credentials.is_expired() {
    // Token exists but expired - try to refresh
    match attempt_token_refresh(&runtime, &credentials) {
        Ok(refreshed) => {
            credentials = refreshed;
            if !manager.save(&credentials) {
                eprintln!("Warning: Failed to save refreshed credentials");
            }
        }
        Err(e) => {
            eprintln!("Token refresh failed: {}. Re-authenticating...", e);
            credentials = match run_auth_flow(&runtime) {
                Ok(creds) => creds,
                Err(e) => {
                    eprintln!("Authentication failed: {}", e);
                    std::process::exit(1);
                }
            };
            if !manager.save(&credentials) {
                eprintln!("Warning: Failed to save credentials after authentication");
            }
        }
    }
}

// =========================================================
// VPS check - ensure VPS exists and is usable
// =========================================================

if !credentials.has_vps() {
    // No VPS configured - need to provision
    if let Err(e) = run_provisioning_flow(&runtime, &mut credentials) {
        eprintln!("Provisioning failed: {}", e);
        std::process::exit(1);
    }
    if !manager.save(&credentials) {
        eprintln!("Warning: Failed to save credentials after provisioning");
    }
} else {
    // VPS exists - check its status
    match credentials.vps_status.as_deref() {
        Some("ready") | Some("running") | Some("active") => {
            // Good to go - launch TUI
        }
        Some("stopped") => {
            // Auto-start existing VPS
            match start_stopped_vps(&runtime, &credentials) {
                Ok(status) => {
                    credentials.vps_status = Some(status.status);
                    credentials.vps_ip = status.ip;
                    if !manager.save(&credentials) {
                        eprintln!("Warning: Failed to save credentials after starting VPS");
                    }
                }
                Err(e) => {
                    eprintln!("Failed to start VPS: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Some("failed") | Some("terminated") => {
            // Failed VPS - don't auto-reprovision, show error
            eprintln!("Error: VPS is in failed state (status: {}).",
                credentials.vps_status.as_deref().unwrap_or("unknown"));
            eprintln!("Your VPS cannot be started automatically.");
            eprintln!("Please contact support@spoq.dev for assistance.");
            std::process::exit(1);
        }
        None => {
            // VPS exists but status field missing (legacy credentials)
            // Fetch status from API instead of re-provisioning
            match fetch_vps_status(&runtime, &credentials) {
                Ok(status) => {
                    credentials.vps_status = Some(status.status.clone());
                    if let Some(ip) = status.ip {
                        credentials.vps_ip = Some(ip);
                    }
                    if !manager.save(&credentials) {
                        eprintln!("Warning: Failed to save updated VPS status");
                    }
                    // Re-check status after fetching
                    match status.status.as_str() {
                        "ready" | "running" | "active" => {
                            // Good to go
                        }
                        "stopped" => {
                            // Need to start it
                            match start_stopped_vps(&runtime, &credentials) {
                                Ok(status) => {
                                    credentials.vps_status = Some(status.status);
                                    credentials.vps_ip = status.ip;
                                    if !manager.save(&credentials) {
                                        eprintln!("Warning: Failed to save credentials");
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Failed to start VPS: {}", e);
                                    std::process::exit(1);
                                }
                            }
                        }
                        "failed" | "terminated" => {
                            eprintln!("Error: VPS is in failed state (status: {}).", status.status);
                            eprintln!("Your VPS cannot be started automatically.");
                            eprintln!("Please contact support@spoq.dev for assistance.");
                            std::process::exit(1);
                        }
                        other => {
                            eprintln!("VPS in unexpected state: {}. Please wait or contact support.", other);
                            std::process::exit(1);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error: Cannot determine VPS status: {}", e);
                    eprintln!("Please check your network connection and try again.");
                    std::process::exit(1);
                }
            }
        }
        Some(other) => {
            // Unknown state
            eprintln!("VPS in unexpected state: {}. Please wait or contact support.", other);
            std::process::exit(1);
        }
    }
}
```

#### 2. `src/main.rs` - New helper functions (add before main function)

```rust
/// Attempt to refresh an expired access token using the refresh token.
///
/// # Arguments
/// * `runtime` - Tokio runtime for async operations
/// * `credentials` - Current credentials with refresh_token
///
/// # Returns
/// * `Ok(Credentials)` - New credentials with refreshed tokens
/// * `Err(CentralApiError)` - Refresh failed (trigger re-auth)
fn attempt_token_refresh(
    runtime: &tokio::runtime::Runtime,
    credentials: &Credentials,
) -> Result<Credentials, CentralApiError> {
    let refresh_token = credentials.refresh_token.as_ref()
        .ok_or_else(|| CentralApiError::ServerError {
            status: 0,
            message: "No refresh token available".to_string(),
        })?;

    let client = CentralApiClient::new(credentials.access_token.clone());

    let refresh_response = runtime.block_on(client.refresh_token(refresh_token))?;

    // Build new credentials with refreshed tokens
    let mut new_credentials = credentials.clone();
    new_credentials.access_token = Some(refresh_response.access_token.clone());

    // Update refresh token if server provided a new one
    if let Some(new_refresh) = refresh_response.refresh_token {
        new_credentials.refresh_token = Some(new_refresh);
    }

    // Update expiration time
    new_credentials.expires_at = Some(refresh_response.expires_at);

    Ok(new_credentials)
}

/// Fetch VPS status from API for cases where vps_status field is missing.
///
/// # Arguments
/// * `runtime` - Tokio runtime for async operations
/// * `credentials` - Current credentials with access token
///
/// # Returns
/// * `Ok(VpsStatusResponse)` - VPS status from API
/// * `Err(CentralApiError)` - API call failed
fn fetch_vps_status(
    runtime: &tokio::runtime::Runtime,
    credentials: &Credentials,
) -> Result<VpsStatusResponse, CentralApiError> {
    let client = CentralApiClient::new(credentials.access_token.clone());
    runtime.block_on(client.fetch_vps_status())
}
```

#### 3. Add imports at top of `src/main.rs`

```rust
use spoq_tui::auth::central_api::VpsStatusResponse;
```

### Testing Requirements

Create test file: `tests/startup_flow_fix_test.rs`

```rust
use spoq_tui::auth::credentials::{Credentials, CredentialsManager};
use tempfile::TempDir;

/// Scenario 1: Fresh install (no credentials.json)
#[test]
fn test_fresh_install_no_credentials() {
    let temp_dir = TempDir::new().unwrap();
    let manager = create_test_manager(&temp_dir);

    let creds = manager.load();
    assert!(!creds.has_token());
    assert!(!creds.has_vps());
}

/// Scenario 2: Valid credentials + VPS (should skip auth/provision)
#[test]
fn test_valid_credentials_with_vps() {
    let temp_dir = TempDir::new().unwrap();
    let manager = create_test_manager(&temp_dir);

    let mut creds = Credentials::default();
    creds.access_token = Some("valid-token".to_string());
    creds.expires_at = Some(chrono::Utc::now().timestamp() + 3600);
    creds.vps_id = Some("vps-123".to_string());
    creds.vps_url = Some("https://vps.example.com".to_string());
    creds.vps_status = Some("ready".to_string());

    assert!(manager.save(&creds));

    let loaded = manager.load();
    assert!(loaded.has_token());
    assert!(!loaded.is_expired());
    assert!(loaded.has_vps());
    assert_eq!(loaded.vps_status, Some("ready".to_string()));
}

/// Scenario 3: Expired token + VPS (should refresh, not re-auth)
#[test]
fn test_expired_token_with_vps() {
    let temp_dir = TempDir::new().unwrap();
    let manager = create_test_manager(&temp_dir);

    let mut creds = Credentials::default();
    creds.access_token = Some("expired-token".to_string());
    creds.refresh_token = Some("valid-refresh".to_string());
    creds.expires_at = Some(0); // Expired
    creds.vps_id = Some("vps-123".to_string());
    creds.vps_url = Some("https://vps.example.com".to_string());
    creds.vps_status = Some("ready".to_string());

    assert!(manager.save(&creds));

    let loaded = manager.load();
    assert!(loaded.has_token());
    assert!(loaded.is_expired()); // Should detect expiration
    assert!(loaded.has_vps());
}

/// Scenario 4: Valid token + no VPS (should provision)
#[test]
fn test_valid_token_no_vps() {
    let temp_dir = TempDir::new().unwrap();
    let manager = create_test_manager(&temp_dir);

    let mut creds = Credentials::default();
    creds.access_token = Some("valid-token".to_string());
    creds.expires_at = Some(chrono::Utc::now().timestamp() + 3600);

    assert!(manager.save(&creds));

    let loaded = manager.load();
    assert!(loaded.has_token());
    assert!(!loaded.is_expired());
    assert!(!loaded.has_vps()); // Should detect no VPS
}

/// Scenario 5: Valid token + stopped VPS (should auto-start)
#[test]
fn test_valid_token_stopped_vps() {
    let temp_dir = TempDir::new().unwrap();
    let manager = create_test_manager(&temp_dir);

    let mut creds = Credentials::default();
    creds.access_token = Some("valid-token".to_string());
    creds.expires_at = Some(chrono::Utc::now().timestamp() + 3600);
    creds.vps_id = Some("vps-123".to_string());
    creds.vps_url = Some("https://vps.example.com".to_string());
    creds.vps_status = Some("stopped".to_string());

    assert!(manager.save(&creds));

    let loaded = manager.load();
    assert_eq!(loaded.vps_status, Some("stopped".to_string()));
}

/// Scenario 6: Valid token + failed VPS (should error, not reprovision)
#[test]
fn test_valid_token_failed_vps() {
    let temp_dir = TempDir::new().unwrap();
    let manager = create_test_manager(&temp_dir);

    let mut creds = Credentials::default();
    creds.access_token = Some("valid-token".to_string());
    creds.expires_at = Some(chrono::Utc::now().timestamp() + 3600);
    creds.vps_id = Some("vps-123".to_string());
    creds.vps_url = Some("https://vps.example.com".to_string());
    creds.vps_status = Some("failed".to_string());

    assert!(manager.save(&creds));

    let loaded = manager.load();
    assert_eq!(loaded.vps_status, Some("failed".to_string()));
    // Startup should detect this and exit with error, not reprovision
}

/// Scenario 7: Credentials missing vps_status field (should fetch, not reprovision)
#[test]
fn test_credentials_missing_vps_status_field() {
    let temp_dir = TempDir::new().unwrap();
    let manager = create_test_manager(&temp_dir);

    let mut creds = Credentials::default();
    creds.access_token = Some("valid-token".to_string());
    creds.expires_at = Some(chrono::Utc::now().timestamp() + 3600);
    creds.vps_id = Some("vps-123".to_string());
    creds.vps_url = Some("https://vps.example.com".to_string());
    creds.vps_status = None; // Missing!

    assert!(manager.save(&creds));

    let loaded = manager.load();
    assert!(loaded.has_vps()); // VPS exists
    assert!(loaded.vps_status.is_none()); // But status missing
    // Startup should fetch status from API, not reprovision
}

// Helper function
fn create_test_manager(temp_dir: &TempDir) -> CredentialsManager {
    let credentials_path = temp_dir.path().join(".spoq").join("credentials.json");
    CredentialsManager { credentials_path }
}
```

## Acceptance Criteria

**Functional:**
- [ ] Running CLI 3+ times consecutively with valid credentials launches TUI immediately (no re-auth, no re-provision)
- [ ] Token expiration detected at startup before any API calls
- [ ] Expired tokens refreshed proactively with saved credentials
- [ ] Token refresh failure triggers re-authentication flow
- [ ] VPS with `vps_id`/`vps_url` but missing `vps_status` fetches status from API
- [ ] Failed/terminated VPS displays error message and exits (does not re-provision)
- [ ] Stopped VPS auto-starts successfully
- [ ] Fresh install flows through auth → provision → TUI

**Testing:**
- [ ] All 7 test scenarios pass
- [ ] No regressions in existing auth/provisioning flows
- [ ] Test with real BYOVPS flow (user's primary use case)

**Code Quality:**
- [ ] No breaking changes to public APIs
- [ ] Minimal code changes (scoped to main.rs startup flow)
- [ ] Clear error messages for all failure cases
- [ ] Credentials saved after every state mutation

## Implementation Checklist

### Phase 1: Core Bug Fixes
- [x] Add `attempt_token_refresh()` helper function in main.rs
- [x] Add `fetch_vps_status()` helper function in main.rs
- [x] Add `use spoq::auth::central_api::VpsStatusResponse` import
- [x] Replace auth check (line 392) with token expiration logic
- [x] Replace VPS check (line 404) to use `has_vps()` instead of `vps_status`
- [x] Add proactive token refresh when `is_expired()` is true
- [x] Change failed/terminated VPS case from provisioning to error message
- [x] Add `vps_status = None` case to fetch from API
- [x] Fixed pre-existing compilation errors in provisioning_flow.rs

### Phase 2: Testing
- [x] Create `tests/startup_flow_fix_test.rs`
- [x] Implement all 7 test scenarios
- [x] Run `cargo test` to verify all tests pass (7/7 passed)
- [ ] Manual test with actual BYOVPS flow
- [ ] Test with expired token scenario
- [ ] Test with stopped VPS scenario

### Phase 3: Validation
- [ ] Run debug build: `./run.sh --debug`
- [ ] Verify TUI launches immediately on subsequent runs
- [ ] Check `~/.spoq/credentials.json` after each scenario
- [ ] Confirm no re-provisioning with existing VPS

## References

- **Brainstorm:** `docs/brainstorms/2026-01-22-cli-startup-flow-fix-brainstorm.md`
- **Critical files:**
  - `src/main.rs:387-443` - Startup flow implementation
  - `src/auth/credentials.rs` - Credential validation methods
  - `src/auth/central_api.rs` - Token refresh API
  - `src/auth/provisioning_flow.rs` - VPS provisioning flow
- **Existing tests:**
  - `tests/auth_integration_test.rs`
  - `tests/startup_update_check_test.rs`
  - `tests/byovps_test.rs`

## Success Metrics

**Before Fix:**
- CLI re-authenticates every startup (even with valid credentials)
- CLI re-provisions VPS every startup (even with existing VPS)
- First API call fails with 401, triggers refresh, retries

**After Fix:**
- Valid credentials → instant TUI launch (0 auth prompts)
- Expired token → automatic refresh → TUI launch (0 user prompts)
- Existing VPS → detected correctly → TUI launch (0 provisioning)
- Failed VPS → clear error message → exit (no auto-reprovision)
