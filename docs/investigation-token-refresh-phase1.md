# Token Refresh Flow Investigation - Phase 1

**Date:** 2026-01-22
**Investigator:** Claude Sonnet 4.5
**Scope:** Deep analysis of startup token refresh logic and failure points

## Executive Summary

This investigation identifies critical gaps in the token refresh implementation that cause inconsistent behavior during CLI startup. The primary issue is **inadequate error handling and logging** around token expiration detection and refresh attempts, combined with **race conditions** between health check token sync and startup token refresh.

## 1. Code Path Analysis

### 1.1 Startup Token Validation Flow (src/main.rs:509-553)

**Entry Point:** `main()` function at line 509

```rust
// Line 509-511: Load credentials
let manager = CredentialsManager::new().expect("Failed to initialize credentials manager");
let mut credentials = manager.load();
```

**Decision Tree:**

1. **No token at all** (line 517-529):
   - Trigger: `credentials.access_token.is_none()`
   - Action: Run full auth flow via `run_auth_flow()`
   - Success: Save credentials
   - Failure: Exit with error code 1

2. **Token exists but expired** (line 530-553):
   - Trigger: `credentials.is_expired()` returns `true`
   - Action: Call `attempt_token_refresh()`
   - Success path: Save refreshed credentials
   - **Failure path (line 539-551):**
     - Print: `"Token refresh failed: {}. Re-authenticating..."`
     - Fallback to `run_auth_flow()`
     - If re-auth fails: Exit with error code 1

3. **Valid token** (implicit else):
   - Continue to VPS checks

### 1.2 Token Expiration Detection (src/auth/credentials.rs:66-74)

```rust
pub fn is_expired(&self) -> bool {
    match self.expires_at {
        Some(expires_at) => {
            let now = chrono::Utc::now().timestamp();
            now >= expires_at  // Returns true if current time >= expiration time
        }
        None => true, // No expiration means we should consider it expired
    }
}
```

**Critical Issue #1:** Missing `expires_at` field is treated as expired
- **Impact:** Any credentials without expiration timestamp trigger refresh
- **Occurs When:** Legacy credentials or incomplete token responses
- **Logging Gap:** No differentiation between "truly expired" vs "missing expiration"

### 1.3 Token Refresh Implementation (src/main.rs:368-401)

```rust
fn attempt_token_refresh(
    runtime: &tokio::runtime::Runtime,
    credentials: &Credentials,
) -> Result<Credentials, spoq::auth::central_api::CentralApiError> {
    let refresh_token = credentials.refresh_token.as_ref()
        .ok_or_else(|| CentralApiError::ServerError {
            status: 0,
            message: "No refresh token available".to_string(),
        })?;

    let client = CentralApiClient::new();
    let refresh_response = runtime.block_on(client.refresh_token(refresh_token))?;

    // Build new credentials with refreshed tokens
    let mut new_credentials = credentials.clone();
    new_credentials.access_token = Some(refresh_response.access_token.clone());

    // Update refresh token if server provided a new one
    if let Some(new_refresh) = refresh_response.refresh_token {
        new_credentials.refresh_token = Some(new_refresh);
    }

    // Calculate expiration from response or JWT
    let expires_in = refresh_response
        .expires_in
        .or_else(|| get_jwt_expires_in(&refresh_response.access_token))
        .unwrap_or(900); // Default 15 minutes
    new_credentials.expires_at = Some(chrono::Utc::now().timestamp() + expires_in as i64);

    Ok(new_credentials)
}
```

**Findings:**

1. **No logging inside refresh function**
   - No log when refresh starts
   - No log of expiration calculation
   - No log of success before returning

2. **Silent fallback to 900 seconds**
   - If both `expires_in` and JWT parsing fail → 15-minute expiration
   - No warning emitted
   - Could cause premature re-expiration

3. **Error propagation is binary**
   - Either `Ok(Credentials)` or `Err(CentralApiError)`
   - No differentiation between:
     - Network failure
     - Invalid refresh token
     - Server-side token revocation
     - Expired refresh token

### 1.4 Central API Refresh Endpoint (src/auth/central_api.rs:511-540)

```rust
pub async fn refresh_token(
    &self,
    refresh_token: &str,
) -> Result<TokenResponse, CentralApiError> {
    let url = format!("{}/auth/refresh", self.base_url);

    let body = serde_json::json!({
        "refresh_token": refresh_token,
    });

    let response = self
        .client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(parse_error_response(status, &body));
    }

    let data: TokenResponse = response.json().await?;
    Ok(data)
}
```

**Critical Issue #2:** No automatic retry or token refresh in this path
- Unlike `provision_vps()`, `fetch_vps_status()`, etc., this function does NOT have auto-refresh logic
- **Why:** Refresh is the recovery mechanism itself—cannot refresh during refresh
- **Impact:** If access token expires mid-startup, refresh call fails immediately

## 2. Health Check Flow (src/main.rs:703-780, src/health_check.rs:26-96)

### 2.1 Health Check Timing

```rust
// Line 703-780: VPS health check
if credentials.has_vps() {
    println!("\nRunning VPS health checks...\n");

    let mut first_attempt = true;

    // Keep checking until VPS is ready
    loop {
        // Run health checks
        let health_result = runtime.block_on(
            spoq::health_check::run_health_checks(&credentials)
        );

        // If tokens are missing on first attempt, try to auto-sync
        if first_attempt && health_result.should_block {
            first_attempt = false;

            println!("⚙️  Attempting to sync credentials to VPS...\n");

            // Attempt sync via conductor
            match &credentials.vps_url {
                Some(url) => {
                    let mut conductor = spoq::conductor::ConductorClient::with_url(url);
                    if let Some(ref token) = credentials.access_token {
                        conductor = conductor.with_auth(token);
                    }
                    if let Some(ref refresh) = credentials.refresh_token {
                        conductor = conductor.with_refresh_token(refresh);
                    }

                    match runtime.block_on(conductor.sync_tokens("all")) {
                        Ok(_) => {
                            println!("✓ Sync initiated, verifying...\n");
                            std::thread::sleep(std::time::Duration::from_secs(2));
                            continue; // Recheck immediately
                        }
                        Err(e) => {
                            println!("⚠️  Auto-sync failed: {}\n", e);
                        }
                    }
                }
                None => {}
            }
        }
```

**Critical Issue #3:** Race condition between token refresh and health check
- Token refresh happens at line 530-553
- Health check happens at line 703-780
- **Gap:** 150+ lines of VPS status checks (586-698)
- **Risk:** Token could expire between refresh and health check if:
  - Server returns very short-lived token (< 2 minutes)
  - VPS checks take long time (slow network, multiple retries)
  - System clock drift

### 2.2 Conductor Token Sync (src/conductor.rs:560-605)

```rust
pub async fn sync_tokens(&mut self, sync_type: &str) -> Result<bool, ConductorError> {
    let url = format!("{}/v1/tokens/sync", self.base_url);

    // Read local token files based on sync_type
    let data = read_local_tokens(sync_type)?;

    let body = serde_json::json!({
        "sync_type": sync_type,
        "data": data
    });

    let builder = self.client.post(&url).json(&body);
    let response = self.add_auth_header(builder).send().await?;

    // Check for 401 and try to refresh
    if response.status().as_u16() == 401 && self.refresh_token.is_some() {
        // Try to refresh the token
        self.refresh_access_token().await?;

        // Retry the request with new token
        let builder = self.client.post(&url).json(&body);
        let response = self.add_auth_header(builder).send().await?;
        // ... handle response
    }
```

**Finding:** Conductor has auto-refresh capability
- Line 575-593: Detects 401, refreshes token, retries request
- **Contrast:** This is BETTER than startup refresh flow
- **Issue:** If startup token refresh fails, health check auto-sync will also fail (same credentials)

## 3. Identified Failure Scenarios

### 3.1 Missing Expiration Timestamp

**Trigger:**
```json
{
  "access_token": "valid_jwt",
  "refresh_token": "valid_refresh",
  "expires_at": null
}
```

**Flow:**
1. `is_expired()` returns `true` (line 72: `None => true`)
2. Triggers refresh attempt
3. Refresh succeeds, sets new `expires_at`
4. But: unnecessary refresh, wastes API call

**Logging Gap:** No log distinguishing "missing expiration" from "actually expired"

### 3.2 JWT Parsing Failure

**Trigger:** Server returns token without `expires_in` field, JWT is malformed

**Flow:**
1. Line 394-397: `expires_in` is `None`
2. `get_jwt_expires_in()` fails (returns `None`)
3. Fallback to 900 seconds
4. Token might actually have longer lifetime → premature re-expiration

**Logging Gap:** No warning when JWT parsing fails

### 3.3 Expired Refresh Token

**Trigger:** User hasn't launched CLI in > 30 days, refresh token expired

**Flow:**
1. Line 530: `is_expired()` returns `true`
2. Line 532: `attempt_token_refresh()` called
3. Central API returns 401 or similar error
4. Line 540: Fallback to `run_auth_flow()`
5. **Problem:** User sees "Token refresh failed", doesn't know WHY

**User Experience Issue:** Error message doesn't explain:
- Is it network issue?
- Is refresh token expired?
- Should user just re-authenticate?

### 3.4 Race Condition Scenario

**Trigger:** Server returns token with 90-second expiration

**Timeline:**
```
00:00 - Load credentials (token expires at 00:01:30)
00:00 - is_expired() = false (still valid)
00:00 - Skip refresh, continue
00:00-00:45 - VPS sync_state() takes 45 seconds (slow network)
00:45 - VPS status checks take 30 seconds
01:15 - Health check starts
01:15 - Conductor.verify_tokens() called
01:15 - Token expired 15 seconds ago
01:15 - Request fails with 401
01:15 - Conductor tries auto-refresh
01:15 - Auto-refresh uses SAME expired refresh token
01:15 - Auto-refresh fails
01:15 - Health check blocks user
```

**Logging Gap:** No timestamp logging to identify timing issues

## 4. Logging Deficiencies

### 4.1 Missing Logs in Token Refresh

**Current State:**
- No log when `attempt_token_refresh()` starts
- No log of expiration calculation
- No log of success

**Should Log:**
```rust
println!("Refreshing expired access token...");
// After success:
println!("✓ Token refreshed successfully (expires in {} seconds)", expires_in);
```

### 4.2 Missing Logs in Expiration Check

**Current State:**
```rust
pub fn is_expired(&self) -> bool {
    match self.expires_at {
        Some(expires_at) => {
            let now = chrono::Utc::now().timestamp();
            now >= expires_at
        }
        None => true,
    }
}
```

**Should Log:**
```rust
pub fn is_expired(&self) -> bool {
    match self.expires_at {
        Some(expires_at) => {
            let now = chrono::Utc::now().timestamp();
            let is_expired = now >= expires_at;
            if is_expired {
                let seconds_ago = now - expires_at;
                eprintln!("Token expired {} seconds ago", seconds_ago);
            } else {
                let seconds_remaining = expires_at - now;
                eprintln!("Token valid for {} more seconds", seconds_remaining);
            }
            is_expired
        }
        None => {
            eprintln!("No expiration timestamp set - treating as expired");
            true
        }
    }
}
```

### 4.3 Missing Error Context

**Current Error Message (line 540):**
```rust
eprintln!("Token refresh failed: {}. Re-authenticating...", e);
```

**Should Include:**
- Refresh token presence: `has_refresh_token={}`
- Error type classification: `error_type=NetworkFailure|InvalidToken|Expired`
- Recommendation: `action=please_wait|please_reauth|check_network`

## 5. Error Handling Issues

### 5.1 Binary Success/Failure

**Problem:** `attempt_token_refresh()` returns `Result<Credentials, CentralApiError>`

**Missing:** Error categorization:
- `NetworkError` - transient, should retry
- `InvalidRefreshToken` - refresh token revoked, must re-auth
- `ExpiredRefreshToken` - refresh token expired, must re-auth
- `ServerError` - temporary server issue, should retry

### 5.2 No Retry Logic

**Current:** Single attempt, immediate fallback to auth flow

**Should Implement:**
- Network errors: 3 retries with exponential backoff
- Server errors (5xx): 2 retries
- Client errors (4xx): No retry, immediate fallback

## 6. Race Conditions

### 6.1 Time-of-Check to Time-of-Use (TOCTOU)

**Location:** Lines 530-553 (token refresh) vs 703-780 (health check)

**Problem:**
1. Check: `is_expired()` at line 530
2. Decision: Skip refresh or refresh
3. Use: Conductor API calls 150+ lines later

**Risk:** Token expires between check and use

**Mitigation Needed:**
- Move health check earlier (before VPS status checks)
- OR: Check token freshness before each API call
- OR: Implement token refresh buffer (refresh if < 2 minutes remaining)

### 6.2 Concurrent Token Updates

**Scenario:** If health check auto-sync triggers token refresh while credentials are in use

**Current Protection:** None identified

**Needed:**
- Mutex/lock around credential updates
- OR: Immutable credentials with atomic swap
- OR: Ensure sequential execution (already true in current code)

## 7. Key Findings Summary

| Issue | Location | Severity | Impact |
|-------|----------|----------|--------|
| No differentiation between missing vs expired timestamp | credentials.rs:66-74 | **HIGH** | Unnecessary refresh attempts |
| No logging in refresh function | main.rs:368-401 | **HIGH** | Cannot debug failures |
| Silent fallback to 900s expiration | main.rs:394-397 | **MEDIUM** | Premature re-expiration |
| Generic error messages | main.rs:540 | **HIGH** | Poor user experience |
| No retry logic | main.rs:532-538 | **MEDIUM** | Transient failures cause re-auth |
| TOCTOU race condition | main.rs:530-780 | **MEDIUM** | Token expires between check and use |
| No error categorization | central_api.rs:511-540 | **HIGH** | Cannot differentiate failure types |

## 8. Exact Failure Point Hypothesis

Based on code analysis, the most likely failure point is:

**Primary:** Line 532-538 in `main.rs`
```rust
match attempt_token_refresh(&runtime, &credentials) {
    Ok(refreshed) => {
        credentials = refreshed;
        if !manager.save(&credentials) {
            eprintln!("Warning: Failed to save refreshed credentials");
        }
    }
    Err(e) => {
        eprintln!("Token refresh failed: {}. Re-authenticating...", e);
```

**Why Refresh Fails:**
1. Network timeout (no retry logic)
2. Expired refresh token (not distinguished from other errors)
3. Server-side token revocation (same error as network)
4. Malformed response (JSON parsing fails)

**Why Auth Flow is Triggered Instead:**
- Line 541: Automatic fallback after ANY refresh error
- No attempt to categorize or retry

**Missing Context:**
- Was it network issue or token issue?
- Should user wait or re-authenticate?
- How long until retry?

## 9. Recommendations for Next Phase

### 9.1 Immediate Logging Improvements

1. Add detailed logging in `attempt_token_refresh()`
2. Add timestamp logging in `is_expired()`
3. Add error categorization in failure messages
4. Add timing logs between token check and use

### 9.2 Error Handling Improvements

1. Categorize `CentralApiError` into retryable vs permanent
2. Implement retry logic for network errors
3. Add exponential backoff
4. Improve user-facing error messages

### 9.3 Race Condition Mitigations

1. Add token freshness buffer (refresh if < 2 minutes remaining)
2. Move health check earlier in startup flow
3. Add token validity check before Conductor API calls
4. Consider implementing token auto-refresh middleware

### 9.4 Testing Improvements

1. Add test for missing `expires_at` field
2. Add test for JWT parsing failure
3. Add test for expired refresh token
4. Add test for network timeout during refresh
5. Add test for race condition scenario

## 10. Next Steps

**Phase 2 should focus on:**
1. Implementing comprehensive logging
2. Adding error categorization
3. Implementing retry logic
4. Adding token freshness buffer
5. Moving health check earlier or adding pre-check validation

**Success Criteria:**
- User can see WHY refresh failed
- Transient errors automatically retry
- Permanent errors show clear action
- Race conditions are eliminated
- All code paths have logging

---

**Investigation Complete**
**Date:** 2026-01-22
**Next Phase:** Implementation of logging and error handling improvements
