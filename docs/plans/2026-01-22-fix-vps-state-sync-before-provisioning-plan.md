---
title: Fix VPS state sync - prevent duplicate provisioning attempts
type: fix
date: 2026-01-22
related: docs/brainstorms/2026-01-22-cli-startup-flow-fix-brainstorm.md
---

# Fix VPS State Sync Before Provisioning

## Problem Statement

**Issue:** CLI allows BYOVPS provisioning even when user already has an active VPS on the server, causing API error:

```
[DEBUG] HTTP 400 Response body: {"error":"User already has an active VPS. Terminate it first before adding a new one."}
```

**Root Cause:** State mismatch between local credentials and server:
- **Local:** `~/.spoq/credentials.json` has no `vps_id`/`vps_url` (so `has_vps()` returns `false`)
- **Server:** User's account already has an active VPS

**Impact:** Users see confusing error after entering all VPS details, poor UX.

## Current Flow (Broken)

```
Startup → Load credentials → has_vps() == false → Run provisioning flow
  → User selects BYOVPS → Enters IP/password → API call fails
  → Error: "User already has an active VPS"
```

## Proposed Solution

**Check server VPS state BEFORE entering provisioning flow** when local credentials show no VPS.

### Approach: Proactive Server State Check

1. Before `run_provisioning_flow()`, check if server has a VPS for this user
2. If server has VPS but local doesn't → sync local credentials with server VPS
3. Only enter provisioning flow if both local AND server confirm no VPS

## Implementation Details

### 1. Add VPS List Endpoint Check

**File:** `src/auth/central_api.rs`

Add new method to check user's VPS on server:

```rust
/// Check if user has any VPS configured on the server.
///
/// Returns the VPS details if one exists, None otherwise.
pub async fn fetch_user_vps(&mut self) -> Result<Option<VpsStatusResponse>, CentralApiError> {
    let url = format!("{}/api/vps/status", self.base_url);

    let response = self
        .client
        .get(&url)
        .header("Authorization", format!("Bearer {}", self.auth_token.as_ref().unwrap_or(&String::new())))
        .send()
        .await?;

    // Handle different status codes
    match response.status().as_u16() {
        200 => {
            let vps = response.json::<VpsStatusResponse>().await?;
            Ok(Some(vps))
        }
        404 => {
            // No VPS found - this is expected for new users
            Ok(None)
        }
        401 => {
            // Token expired - trigger refresh and retry
            if let Some(refresh_token) = &self.refresh_token {
                let token_response = self.refresh_token(refresh_token).await?;
                self.auth_token = Some(token_response.access_token);
                // Retry the request
                return Box::pin(self.fetch_user_vps()).await;
            }
            Err(CentralApiError::ServerError {
                status: 401,
                message: "Unauthorized - please re-authenticate".to_string(),
            })
        }
        status => {
            let error_body = response.text().await.unwrap_or_default();
            Err(CentralApiError::ServerError {
                status,
                message: format!("Failed to fetch VPS: {}", error_body),
            })
        }
    }
}
```

### 2. Update Startup Flow in main.rs

**File:** `src/main.rs` (around line 504)

**Current code:**
```rust
if !credentials.has_vps() {
    // No VPS configured - need to provision
    if let Err(e) = run_provisioning_flow(&runtime, &mut credentials) {
        eprintln!("Provisioning failed: {}", e);
        std::process::exit(1);
    }
    if !manager.save(&credentials) {
        eprintln!("Warning: Failed to save credentials after provisioning");
    }
}
```

**Fixed code:**
```rust
if !credentials.has_vps() {
    // Local credentials show no VPS - check server state first
    println!("Checking VPS status...");

    let mut client = CentralApiClient::new();
    if let Some(ref token) = credentials.access_token {
        client = client.with_auth(token);
    }

    match runtime.block_on(client.fetch_user_vps()) {
        Ok(Some(server_vps)) => {
            // Server has VPS but local doesn't - sync local credentials
            println!("Found existing VPS on server. Syncing local credentials...");
            credentials.vps_id = Some(server_vps.vps_id.clone());
            credentials.vps_url = server_vps.url.clone();
            credentials.vps_hostname = server_vps.hostname.clone();
            credentials.vps_ip = server_vps.ip.clone();
            credentials.vps_status = Some(server_vps.status.clone());

            if !manager.save(&credentials) {
                eprintln!("Warning: Failed to save synced VPS credentials");
            }

            println!("VPS credentials synced successfully.");

            // Continue with VPS status checks (may need to start if stopped, etc.)
            // Fall through to the "VPS exists" branch below
        }
        Ok(None) => {
            // Server confirms no VPS - safe to provision
            if let Err(e) = run_provisioning_flow(&runtime, &mut credentials) {
                eprintln!("Provisioning failed: {}", e);
                std::process::exit(1);
            }
            if !manager.save(&credentials) {
                eprintln!("Warning: Failed to save credentials after provisioning");
            }
        }
        Err(e) => {
            eprintln!("Error: Cannot verify VPS status: {}", e);
            eprintln!("Please check your network connection and try again.");
            std::process::exit(1);
        }
    }
} else {
    // VPS exists in local credentials - continue as before
    // ...existing status check logic...
}
```

### 3. Extract to Helper Function (Cleaner)

For better code organization, extract this to a helper function:

```rust
/// Sync VPS state between local credentials and server.
///
/// Checks if server has a VPS when local credentials don't, and syncs if needed.
/// Returns true if provisioning is needed, false if VPS already exists.
fn sync_vps_state(
    runtime: &tokio::runtime::Runtime,
    credentials: &mut Credentials,
    manager: &CredentialsManager,
) -> Result<bool, spoq::auth::central_api::CentralApiError> {
    println!("Checking VPS status...");

    let mut client = CentralApiClient::new();
    if let Some(ref token) = credentials.access_token {
        client = client.with_auth(token);
    }

    match runtime.block_on(client.fetch_user_vps())? {
        Some(server_vps) => {
            // Server has VPS - sync local credentials
            println!("Found existing VPS on server. Syncing local credentials...");
            credentials.vps_id = Some(server_vps.vps_id.clone());
            credentials.vps_url = server_vps.url.clone();
            credentials.vps_hostname = server_vps.hostname.clone();
            credentials.vps_ip = server_vps.ip.clone();
            credentials.vps_status = Some(server_vps.status.clone());

            if !manager.save(credentials) {
                eprintln!("Warning: Failed to save synced VPS credentials");
            }

            println!("VPS credentials synced successfully.");
            Ok(false) // No provisioning needed
        }
        None => {
            // Server confirms no VPS
            Ok(true) // Provisioning needed
        }
    }
}
```

Then in main.rs:

```rust
if !credentials.has_vps() {
    match sync_vps_state(&runtime, &mut credentials, &manager) {
        Ok(true) => {
            // Provisioning needed
            if let Err(e) = run_provisioning_flow(&runtime, &mut credentials) {
                eprintln!("Provisioning failed: {}", e);
                std::process::exit(1);
            }
            if !manager.save(&credentials) {
                eprintln!("Warning: Failed to save credentials after provisioning");
            }
        }
        Ok(false) => {
            // VPS synced - continue to status checks below
        }
        Err(e) => {
            eprintln!("Error: Cannot verify VPS status: {}", e);
            std::process::exit(1);
        }
    }
}
```

## Acceptance Criteria

**Functional:**
- [ ] When local has no VPS but server does, CLI syncs credentials from server
- [ ] When both local and server have no VPS, provisioning proceeds normally
- [ ] When sync succeeds, user sees success message
- [ ] When sync fails due to network, user sees clear error message
- [ ] After sync, VPS status check logic runs (start if stopped, error if failed, etc.)

**Testing:**
- [ ] Test scenario: Delete `vps_id` from credentials.json manually → startup syncs from server
- [ ] Test scenario: Fresh user (no VPS anywhere) → provisioning proceeds
- [ ] Test scenario: Server VPS is "stopped" → after sync, auto-starts
- [ ] Test scenario: Network failure during sync → clear error message
- [ ] All existing 7 startup flow tests still pass

**Code Quality:**
- [ ] No breaking changes to existing flows
- [ ] Clear console messages during sync
- [ ] Proper error handling for all API failure cases
- [ ] Helper function extracted for maintainability

## Implementation Checklist

### Phase 1: Add Server VPS Check
- [x] Add `fetch_user_vps()` method to CentralApiClient
- [x] Handle 200 (VPS exists), 404 (no VPS), 401 (auth error) responses
- [x] Add auto-refresh on 401 (existing pattern)
- [x] Return `Option<VpsStatusResponse>`

### Phase 2: Add Sync Logic
- [x] Create `sync_vps_state()` helper function in main.rs
- [x] Check server VPS when `has_vps() == false`
- [x] Sync all VPS fields to local credentials
- [x] Save credentials after sync
- [x] Return bool indicating if provisioning needed

### Phase 3: Update Startup Flow
- [x] Replace direct `run_provisioning_flow()` call with `sync_vps_state()` check
- [x] Only provision if sync confirms no VPS on server
- [x] Add console messages for user feedback
- [x] Handle sync errors gracefully

### Phase 4: Testing
- [ ] Test with manually deleted VPS fields in credentials.json
- [ ] Test with fresh credentials (no VPS anywhere)
- [ ] Test with network errors during sync
- [x] Run all existing tests to verify no regressions
- [ ] Manual test the exact scenario user encountered

## Alternative Approaches Considered

### Option 1: Server-Side Fix
**Approach:** API returns existing VPS info instead of error when duplicate provisioning attempted.

**Pros:** No CLI changes needed.

**Cons:** API design is correct - shouldn't create duplicate resources. CLI should prevent the attempt.

### Option 2: Warning Only
**Approach:** Let user attempt provisioning, catch 400 error, show warning with existing VPS info.

**Pros:** Simpler implementation.

**Cons:** Poor UX - user wastes time entering VPS details only to be told they can't provision.

### Option 3: Periodic Sync
**Approach:** Periodically sync credentials with server in background.

**Pros:** Always in sync.

**Cons:** Unnecessary API calls, complexity.

**Decision:** **Proactive server check at startup** (our proposed solution) - catches mismatch early, one-time API call, clean UX.

## Testing Strategy

### Unit Tests

Test `fetch_user_vps()` responses:

```rust
#[tokio::test]
async fn test_fetch_user_vps_exists() {
    // Mock 200 response with VPS data
    // Assert Ok(Some(vps))
}

#[tokio::test]
async fn test_fetch_user_vps_not_found() {
    // Mock 404 response
    // Assert Ok(None)
}

#[tokio::test]
async fn test_fetch_user_vps_unauthorized() {
    // Mock 401 response
    // Assert error or auto-refresh attempt
}
```

### Integration Tests

Add to `tests/startup_flow_fix_test.rs`:

```rust
/// Scenario 8: Local has no VPS but server does (sync required)
#[test]
fn test_local_missing_vps_server_has_vps() {
    // Setup credentials with no VPS fields
    // Mock server response with VPS data
    // Assert credentials are synced
    // Assert no provisioning attempted
}

/// Scenario 9: Both local and server have no VPS
#[test]
fn test_no_vps_anywhere() {
    // Setup credentials with no VPS
    // Mock 404 from server
    // Assert provisioning flow is entered
}
```

### Manual Testing

1. **Reproduce user's scenario:**
   - Delete `vps_id`, `vps_url`, `vps_status` from `~/.spoq/credentials.json`
   - Keep valid `access_token`
   - Run `./run.sh --debug`
   - **Expected:** VPS synced from server, no provisioning menu

2. **Fresh user:**
   - Delete entire `~/.spoq/credentials.json`
   - Run CLI
   - **Expected:** Auth flow → server check → provisioning menu

## Success Metrics

**Before Fix:**
- User with server VPS but cleared local credentials sees provisioning menu
- User wastes time entering VPS details
- API rejects with confusing "already has VPS" error

**After Fix:**
- CLI checks server state before provisioning menu
- Syncs VPS from server if mismatch detected
- User sees "VPS credentials synced" message
- Goes straight to TUI (or VPS status handling)

## References

- **Brainstorm:** `docs/brainstorms/2026-01-22-cli-startup-flow-fix-brainstorm.md`
- **Related Plan:** `docs/plans/2026-01-22-fix-cli-startup-flow-bugs-plan.md`
- **Key files:**
  - `src/main.rs:504` - VPS check logic
  - `src/auth/central_api.rs` - API client methods
  - `src/auth/credentials.rs` - has_vps() method
