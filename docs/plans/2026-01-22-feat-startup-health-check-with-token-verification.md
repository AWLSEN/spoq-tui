---
title: Startup Health Check with Token Verification
type: feat
date: 2026-01-22
---

# Startup Health Check with Token Verification

## Overview

Add a comprehensive health check sequence on every startup when a VPS exists:
1. Check conductor health endpoint (`/v1/health`)
2. Fetch SSH credentials from server
3. SSH to VPS and verify Claude Code and GitHub CLI tokens work
4. Display results before starting TUI

## Problem Statement

Currently, we verify tokens only during provisioning. Users have no visibility into:
- Whether conductor is reachable
- Whether tokens are still working on VPS
- Whether VPS is actually online

This can lead to confusing errors when trying to use the TUI.

## Proposed Solution

### Startup Sequence Flow

```
Starting SPOQ...
✓ Authentication verified
✓ VPS found (192.168.1.100)

Running VPS health checks...

[1/2] Checking conductor health...
  ⠋ Connecting to https://user.spoq.dev:8000
  ✓ Conductor responding (123ms)

[2/2] Verifying VPS tokens...
  ⠋ Fetching SSH credentials...
  ✓ Credentials retrieved
  ⠋ Checking Claude Code on VPS...
  ✓ Claude Code verified
  ⠋ Checking GitHub CLI on VPS...
  ✓ GitHub CLI verified

✓ All systems ready! Starting TUI...
```

## Technical Approach

### Part 1: API Endpoint (spoq-web-apis)

Create new endpoint: `GET /api/vps/ssh-credentials`

**Request:**
- Requires authentication (Bearer token)
- No parameters

**Response (200 OK):**
```json
{
  "ip": "192.168.1.100",
  "username": "spoq",
  "password": "encrypted_password_here"
}
```

**Error responses:**
- 401 Unauthorized - Invalid/missing token
- 404 Not Found - User has no VPS
- 500 Server Error - Database error

**Security considerations:**
- Only return credentials for authenticated user's VPS
- Rate limit: 10 requests per minute
- Log access attempts

**Implementation location:**
- File: `spoq-web-apis/src/handlers/vps.rs` (or new file `ssh_credentials.rs`)
- Route: Add to router in `src/main.rs`
- Database: Query VPS table for ssh_username and ssh_password

### Part 2: CLI Implementation (spoq-cli)

#### 2.1 Add API Client Method

**File:** `src/auth/central_api.rs`

```rust
/// SSH credentials response from GET /api/vps/ssh-credentials
#[derive(Debug, Clone, Deserialize)]
pub struct SshCredentialsResponse {
    pub ip: String,
    pub username: String,
    pub password: String,
}

impl CentralApiClient {
    /// Fetch SSH credentials for user's VPS.
    ///
    /// GET /api/vps/ssh-credentials
    ///
    /// Requires authentication.
    pub async fn fetch_ssh_credentials(&mut self) -> Result<SshCredentialsResponse, CentralApiError> {
        let url = format!("{}/api/vps/ssh-credentials", self.base_url);

        // With automatic token refresh on 401
        let builder = self.client.get(&url);
        let response = self.add_auth_header(builder).send().await?;

        if response.status().is_success() {
            let creds = response.json::<SshCredentialsResponse>().await?;
            Ok(creds)
        } else if response.status() == 404 {
            Err(CentralApiError::NotFound)
        } else {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            Err(CentralApiError::ServerError {
                status,
                message: text,
            })
        }
    }
}
```

#### 2.2 Add Health Check Module

**File:** `src/health_check.rs` (NEW)

```rust
use crate::auth::central_api::CentralApiClient;
use crate::auth::credentials::Credentials;
use crate::auth::token_verification::{verify_vps_tokens, VpsTokenVerification};
use crate::conductor::ConductorClient;

pub struct HealthCheckResult {
    pub conductor_healthy: bool,
    pub conductor_response_time_ms: Option<u64>,
    pub tokens_verified: Option<VpsTokenVerification>,
    pub ssh_error: Option<String>,
}

/// Run comprehensive health checks on VPS
pub async fn run_health_checks(
    credentials: &Credentials,
    central_api: &mut CentralApiClient,
) -> HealthCheckResult {
    let mut result = HealthCheckResult {
        conductor_healthy: false,
        conductor_response_time_ms: None,
        tokens_verified: None,
        ssh_error: None,
    };

    // Step 1: Check conductor health
    if let Some(ref vps_url) = credentials.vps_url {
        let conductor = ConductorClient::with_url(vps_url);
        let start = std::time::Instant::now();

        match conductor.health_check().await {
            Ok(healthy) => {
                result.conductor_healthy = healthy;
                result.conductor_response_time_ms = Some(start.elapsed().as_millis() as u64);
            }
            Err(_) => {
                result.conductor_healthy = false;
            }
        }
    }

    // Step 2: Fetch SSH credentials and verify tokens
    match central_api.fetch_ssh_credentials().await {
        Ok(ssh_creds) => {
            // Run token verification (blocking call in tokio context)
            match tokio::task::spawn_blocking(move || {
                verify_vps_tokens(&ssh_creds.ip, &ssh_creds.username, &ssh_creds.password)
            }).await {
                Ok(Ok(verification)) => {
                    result.tokens_verified = Some(verification);
                }
                Ok(Err(e)) => {
                    result.ssh_error = Some(format!("{}", e));
                }
                Err(e) => {
                    result.ssh_error = Some(format!("Task error: {}", e));
                }
            }
        }
        Err(e) => {
            result.ssh_error = Some(format!("Could not fetch SSH credentials: {}", e));
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
    }

    // Token verification
    if let Some(ref verification) = result.tokens_verified {
        if verification.claude_code_works && verification.github_cli_works {
            println!("✓ Claude Code verified on VPS");
            println!("✓ GitHub CLI verified on VPS");
        } else {
            if verification.claude_code_works {
                println!("✓ Claude Code verified on VPS");
            } else {
                println!("⚠️  Claude Code verification failed");
            }
            if verification.github_cli_works {
                println!("✓ GitHub CLI verified on VPS");
            } else {
                println!("⚠️  GitHub CLI verification failed");
            }
        }
    } else if let Some(ref error) = result.ssh_error {
        println!("⚠️  Could not verify tokens: {}", error);
    }

    // Summary
    let all_healthy = result.conductor_healthy
        && result.tokens_verified.as_ref().map(|v| v.claude_code_works && v.github_cli_works).unwrap_or(false);

    if all_healthy {
        println!("\n✓ All systems ready!\n");
    } else {
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

    // Create central API client
    let mut central_client = CentralApiClient::with_base_url(
        std::env::var("CENTRAL_API_URL")
            .unwrap_or_else(|_| "https://api.spoq.dev".to_string())
    );
    if let Some(ref token) = credentials.access_token {
        central_client = central_client.with_auth(token);
    }
    if let Some(ref refresh) = credentials.refresh_token {
        central_client = central_client.with_refresh_token(refresh);
    }

    // Run health checks
    let health_result = runtime.block_on(
        spoq::health_check::run_health_checks(&credentials, &mut central_client)
    );

    // Display results
    spoq::health_check::display_health_check_results(&health_result);

    // Non-blocking: continue to TUI even if checks failed
}
```

#### 2.4 Export New Module

**File:** `src/lib.rs`

```rust
pub mod health_check;
```

## Implementation Phases

### Phase 1: API Endpoint (spoq-web-apis)
- [ ] Create `GET /api/vps/ssh-credentials` endpoint
- [ ] Add route to router
- [ ] Test with authenticated requests
- [ ] Add rate limiting
- [ ] Deploy to staging

### Phase 2: CLI Implementation (spoq-cli)
- [ ] Add `SshCredentialsResponse` struct to central_api.rs
- [ ] Implement `fetch_ssh_credentials()` method
- [ ] Create `src/health_check.rs` module
- [ ] Implement `run_health_checks()` function
- [ ] Implement `display_health_check_results()` function
- [ ] Integrate into main.rs startup sequence
- [ ] Export module in lib.rs

### Phase 3: Testing
- [ ] Test endpoint with valid credentials
- [ ] Test endpoint without VPS
- [ ] Test CLI health check with healthy VPS
- [ ] Test CLI health check with offline VPS
- [ ] Test CLI health check with missing tokens
- [ ] Test graceful failure handling

### Phase 4: Polish
- [ ] Add loading spinner during checks
- [ ] Improve error messages
- [ ] Add `--skip-health-check` flag (optional)
- [ ] Update documentation

## Files to Modify/Create

### spoq-web-apis
- [ ] Create or modify: `src/handlers/vps.rs` or `src/handlers/ssh_credentials.rs`
- [ ] Modify: `src/main.rs` (add route)
- [ ] Test: Add integration test

### spoq-cli
- [ ] Modify: `src/auth/central_api.rs` (add endpoint method)
- [ ] Create: `src/health_check.rs` (new module)
- [ ] Modify: `src/lib.rs` (export module)
- [ ] Modify: `src/main.rs` (integrate health checks)
- [ ] Create: `tests/health_check_test.rs` (unit tests)

## Success Metrics

- ✓ Health checks run on every startup (when VPS exists)
- ✓ Checks complete in < 5 seconds on healthy VPS
- ✓ Clear visual feedback during checks
- ✓ Graceful handling of failures (non-blocking)
- ✓ Users can identify VPS issues before entering TUI

## Security Considerations

- SSH password transmitted over HTTPS only
- Endpoint requires authentication
- Rate limiting to prevent abuse
- Credentials never logged or displayed
- No caching of SSH passwords in CLI

## Future Enhancements

- Add `--skip-health-check` flag for faster startup
- Cache health check results for 5 minutes
- Add health check to status bar in TUI
- Periodic health checks while TUI is running
- SSH key support instead of password
