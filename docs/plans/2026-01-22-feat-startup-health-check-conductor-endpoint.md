---
title: Startup Health Check via Conductor Endpoint
type: feat
date: 2026-01-22
status: revised
---

# Startup Health Check via Conductor Endpoint

## Overview

Add comprehensive health checks on CLI startup by having Conductor verify tokens locally on the VPS. No SSH needed!

## Revised Approach

Instead of SSH from CLI → VPS to verify tokens, have Conductor expose an endpoint that performs the verification locally and returns results.

### Benefits
- ✅ No SSH password storage needed
- ✅ No SSH connection overhead
- ✅ Conductor already has local access to VPS
- ✅ Simpler and more secure
- ✅ Single HTTPS request instead of multiple SSH commands

## Startup Sequence Flow

```
Starting SPOQ...
✓ Authentication verified
✓ VPS found (192.168.1.100)

Running VPS health checks...

⠋ Checking conductor health...
✓ Conductor responding (123ms)

⠋ Verifying VPS tokens...
✓ Claude Code verified on VPS
✓ GitHub CLI verified on VPS

✓ All systems ready! Starting TUI...
```

## Technical Implementation

### Part 1: Conductor Endpoint (conductor repo)

Add new endpoint: `GET /v1/tokens/verify`

**Request:**
- No authentication needed (Conductor trusts local CLI)
- No parameters

**Response (200 OK):**
```json
{
  "claude_code": {
    "installed": true,
    "authenticated": true,
    "version": "2.1.7",
    "checked_at": "2026-01-22T10:30:00Z"
  },
  "github_cli": {
    "installed": true,
    "authenticated": true,
    "user": "username",
    "checked_at": "2026-01-22T10:30:00Z"
  }
}
```

**Implementation (conductor/src/routes/tokens.rs):**
```rust
use axum::{Json, response::IntoResponse};
use serde::Serialize;
use std::process::Command;
use chrono::Utc;

#[derive(Serialize)]
pub struct TokenStatus {
    installed: bool,
    authenticated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<String>,
    checked_at: String,
}

#[derive(Serialize)]
pub struct TokensVerifyResponse {
    claude_code: TokenStatus,
    github_cli: TokenStatus,
}

pub async fn verify_tokens() -> impl IntoResponse {
    let now = Utc::now().to_rfc3339();

    // Check Claude Code
    let claude_status = check_claude_code();

    // Check GitHub CLI
    let gh_status = check_github_cli();

    Json(TokensVerifyResponse {
        claude_code: TokenStatus {
            installed: claude_status.0,
            authenticated: claude_status.1,
            version: claude_status.2,
            user: None,
            checked_at: now.clone(),
        },
        github_cli: TokenStatus {
            installed: gh_status.0,
            authenticated: gh_status.1,
            version: None,
            user: gh_status.2,
            checked_at: now,
        },
    })
}

fn check_claude_code() -> (bool, bool, Option<String>) {
    // Check if installed
    let version_check = Command::new("claude")
        .arg("--version")
        .output();

    let installed = version_check.as_ref().map(|o| o.status.success()).unwrap_or(false);
    if !installed {
        return (false, false, None);
    }

    let version = version_check
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string());

    // Check if authenticated by running a test command
    let auth_check = Command::new("claude")
        .arg("-p")
        .arg("testing verification")
        .output();

    let authenticated = auth_check
        .map(|o| o.status.success())
        .unwrap_or(false);

    (installed, authenticated, version)
}

fn check_github_cli() -> (bool, bool, Option<String>) {
    // Check if installed
    let version_check = Command::new("gh")
        .arg("--version")
        .output();

    let installed = version_check.as_ref().map(|o| o.status.success()).unwrap_or(false);
    if !installed {
        return (false, false, None);
    }

    // Check if authenticated
    let auth_check = Command::new("gh")
        .arg("auth")
        .arg("status")
        .output();

    let (authenticated, user) = auth_check
        .ok()
        .and_then(|o| {
            let stdout = String::from_utf8(o.stdout).ok()?;
            let authenticated = o.status.success() && (stdout.contains("Logged in") || stdout.contains("✓"));
            let user = if authenticated {
                // Parse username from output if available
                None // TODO: parse from gh auth status output
            } else {
                None
            };
            Some((authenticated, user))
        })
        .unwrap_or((false, None));

    (installed, authenticated, user)
}
```

**Add to router (conductor/src/main.rs):**
```rust
let app = Router::new()
    .route("/health", get(health_check))
    .route("/v1/health", get(health_check))
    .route("/v1/tokens/verify", get(tokens::verify_tokens)) // NEW
    // ... other routes
```

### Part 2: CLI Implementation (spoq-cli)

#### 2.1 Update Conductor Client

**File:** `src/conductor.rs`

```rust
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct TokenStatus {
    pub installed: bool,
    pub authenticated: bool,
    pub version: Option<String>,
    pub user: Option<String>,
    pub checked_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TokensVerifyResponse {
    pub claude_code: TokenStatus,
    pub github_cli: TokenStatus,
}

impl ConductorClient {
    /// Verify tokens on VPS via Conductor
    ///
    /// GET /v1/tokens/verify
    pub async fn verify_tokens(&self) -> Result<TokensVerifyResponse, ConductorError> {
        let url = format!("{}/v1/tokens/verify", self.base_url);

        let builder = self.client.get(&url);
        let response = self.add_auth_header(builder).send().await?;

        if response.status().is_success() {
            let result = response.json::<TokensVerifyResponse>().await?;
            Ok(result)
        } else {
            Err(ConductorError::HttpError(format!(
                "Token verification failed: {}",
                response.status()
            )))
        }
    }
}
```

#### 2.2 Add Health Check Module

**File:** `src/health_check.rs` (NEW)

```rust
use crate::auth::credentials::Credentials;
use crate::conductor::ConductorClient;

pub struct HealthCheckResult {
    pub conductor_healthy: bool,
    pub conductor_response_time_ms: Option<u64>,
    pub claude_code_works: bool,
    pub github_cli_works: bool,
}

/// Run comprehensive health checks on VPS via Conductor
pub async fn run_health_checks(credentials: &Credentials) -> HealthCheckResult {
    let mut result = HealthCheckResult {
        conductor_healthy: false,
        conductor_response_time_ms: None,
        claude_code_works: false,
        github_cli_works: false,
    };

    // Create conductor client
    let conductor = match &credentials.vps_url {
        Some(url) => ConductorClient::with_url(url),
        None => return result, // No VPS URL
    };

    // Step 1: Check conductor health
    let start = std::time::Instant::now();
    match conductor.health_check().await {
        Ok(healthy) => {
            result.conductor_healthy = healthy;
            result.conductor_response_time_ms = Some(start.elapsed().as_millis() as u64);
        }
        Err(_) => {
            result.conductor_healthy = false;
            return result; // If conductor is down, skip token check
        }
    }

    // Step 2: Verify tokens via conductor
    match conductor.verify_tokens().await {
        Ok(tokens) => {
            result.claude_code_works = tokens.claude_code.installed && tokens.claude_code.authenticated;
            result.github_cli_works = tokens.github_cli.installed && tokens.github_cli.authenticated;
        }
        Err(e) => {
            tracing::warn!("Token verification failed: {}", e);
        }
    }

    result
}

/// Display health check results to user
pub fn display_health_check_results(result: &HealthCheckResult) {
    println!();

    // Conductor health
    if result.conductor_healthy {
        if let Some(ms) = result.conductor_response_time_ms {
            println!("✓ Conductor responding ({}ms)", ms);
        } else {
            println!("✓ Conductor healthy");
        }
    } else {
        println!("⚠️  Conductor not responding");
        println!("   Your VPS may be offline or starting up.");
        return; // Don't show token status if conductor is down
    }

    // Token verification
    if result.claude_code_works && result.github_cli_works {
        println!("✓ Claude Code verified on VPS");
        println!("✓ GitHub CLI verified on VPS");
        println!("\n✓ All systems ready!\n");
    } else {
        if result.claude_code_works {
            println!("✓ Claude Code verified on VPS");
        } else {
            println!("⚠️  Claude Code not authenticated on VPS");
            println!("   Run: ssh spoq@[VPS_IP] → claude login");
        }

        if result.github_cli_works {
            println!("✓ GitHub CLI verified on VPS");
        } else {
            println!("⚠️  GitHub CLI not authenticated on VPS");
            println!("   Run: ssh spoq@[VPS_IP] → gh auth login");
        }

        println!("\n⚠️  Some checks failed. You may experience issues.\n");
    }
}
```

#### 2.3 Integrate into Main Startup

**File:** `src/main.rs`

Add after VPS check, before TUI initialization:

```rust
// After VPS status check succeeds...
if credentials.has_vps() {
    println!("\nRunning VPS health checks...\n");

    // Run health checks
    let health_result = runtime.block_on(
        spoq::health_check::run_health_checks(&credentials)
    );

    // Display results
    spoq::health_check::display_health_check_results(&health_result);

    // Non-blocking: continue to TUI even if checks failed
}
```

## Implementation Phases

### Phase 1: Conductor Endpoint
- [ ] Add `/v1/tokens/verify` endpoint to Conductor
- [ ] Implement `check_claude_code()` function
- [ ] Implement `check_github_cli()` function
- [ ] Test locally on VPS
- [ ] Deploy Conductor update

### Phase 2: CLI Implementation
- [ ] Add `TokensVerifyResponse` to conductor.rs
- [ ] Implement `verify_tokens()` method in ConductorClient
- [ ] Create `src/health_check.rs` module
- [ ] Implement `run_health_checks()` function
- [ ] Implement `display_health_check_results()` function
- [ ] Integrate into main.rs startup sequence
- [ ] Export module in lib.rs

### Phase 3: Testing
- [ ] Test with healthy VPS and working tokens
- [ ] Test with VPS offline
- [ ] Test with missing Claude Code token
- [ ] Test with missing GitHub CLI token
- [ ] Test graceful failure handling

### Phase 4: Polish
- [ ] Add loading spinner during checks
- [ ] Improve error messages
- [ ] Add `--skip-health-check` flag (optional)
- [ ] Update documentation

## Files to Create/Modify

### Conductor (separate repo)
- [ ] Create: `src/routes/tokens.rs` (new file)
- [ ] Modify: `src/main.rs` (add route)
- [ ] Test: Add integration test

### spoq-cli
- [ ] Modify: `src/conductor.rs` (add verify_tokens method)
- [ ] Create: `src/health_check.rs` (new module)
- [ ] Modify: `src/lib.rs` (export module)
- [ ] Modify: `src/main.rs` (integrate health checks)
- [ ] Create: `tests/health_check_test.rs` (unit tests)

## Success Criteria

- ✓ Health checks run on every startup (when VPS exists)
- ✓ Checks complete in < 3 seconds on healthy VPS
- ✓ Clear visual feedback during checks
- ✓ Graceful handling of failures (non-blocking)
- ✓ No SSH password storage needed
- ✓ Users can identify issues before entering TUI

## Security Benefits

- ✅ No SSH password transmission
- ✅ No SSH password storage
- ✅ Single HTTPS request instead of SSH
- ✅ Conductor runs checks locally (no external access)
- ✅ Simpler attack surface

## Future Enhancements

- Add `--skip-health-check` flag for faster startup
- Cache health check results for 5 minutes
- Add health check to status bar in TUI
- Periodic health checks while TUI is running
- Return more detailed token info (email, expiry, etc.)
