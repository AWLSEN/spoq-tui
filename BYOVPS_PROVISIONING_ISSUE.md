# BYOVPS Provisioning Issue - Root Cause Analysis

## Problem

When BYOVPS provisioning fails, the user sees:
```
Provisioning failed!

Error: BYOVPS provisioning failed. Check the script output for details.
```

But **no script output is shown**, leaving the user without the actual error details needed to debug.

---

## Root Cause

The CLI is **not displaying the `install_script.output` field** from the server response.

### Server Response Structure

When provisioning fails, the server returns (from `spoq-web-apis/src/handlers/byovps.rs:453-468`):

```rust
{
    "status": "failed",
    "message": "BYOVPS provisioning failed. Check the script output for details.",
    "install_script": {
        "status": "failed",
        "output": "... ACTUAL ERROR DETAILS HERE ..."  // ⚠️ This is not displayed!
    }
}
```

The `install_script.output` field contains the stdout/stderr from the installation script execution, including:
- SSH connection errors
- Script execution errors
- Package installation failures
- Service startup issues
- Network connectivity problems
- Any other errors from the bash script

### CLI Error Handling (Current)

File: `src/auth/provisioning_flow.rs:720-728`

```rust
match provision_response.status.to_lowercase().as_str() {
    "failed" | "error" => {
        let msg = provision_response
            .message
            .unwrap_or_else(|| "BYOVPS provisioning failed".to_string());
        return Err(CentralApiError::ServerError {
            status: 500,
            message: msg,  // ⚠️ Only uses the generic message!
        });
    }
    // ...
}
```

**The problem:** The code only uses `provision_response.message` (which says "Check the script output") but never accesses `provision_response.install_script.output` (which contains the actual error details).

---

## Impact

Users cannot debug BYOVPS provisioning failures because they don't see:
- Why SSH connection failed
- What script command failed
- Which package couldn't be installed
- What service failed to start
- Network errors during downloads
- Any other specific error from the installation process

---

## Solution

Display the `install_script.output` when available and provisioning fails.

### Proposed Fix

File: `src/auth/provisioning_flow.rs:720-728`

```rust
match provision_response.status.to_lowercase().as_str() {
    "failed" | "error" => {
        let mut msg = provision_response
            .message
            .unwrap_or_else(|| "BYOVPS provisioning failed".to_string());

        // Include install script output if available
        if let Some(ref install_script) = provision_response.install_script {
            if let Some(ref output) = install_script.output {
                msg = format!("{}\n\nScript output:\n{}", msg, output);
            }
        }

        return Err(CentralApiError::ServerError {
            status: 500,
            message: msg,
        });
    }
    // ...
}
```

### Alternative: Display in Error Handler

File: `src/auth/provisioning_flow.rs:458-477`

Could also modify `display_byovps_error()` to accept the full response and extract install_script there, but that would require passing more context through the error chain.

---

## Additional Finding: Password Length Validation Mismatch

### Secondary Issue

**Client validation** (`src/auth/provisioning_flow.rs:81-175`):
- Minimum password length: **1 character**

**Server validation** (assumed based on server code):
- Minimum password length: **8 characters** (standard for SSH passwords)

If a user enters a password shorter than 8 characters:
1. Client accepts it (only requires 1 char)
2. Server likely rejects it during validation
3. User sees generic error without knowing password is too short

### Recommended Fix

Update client-side validation to match server requirements:

```rust
// In collect_byovps_credentials() function
let ssh_password = loop {
    let pass = rpassword::prompt_password("Enter SSH password (min 8 characters): ")?;
    let pass = pass.trim();
    if pass.len() >= 8 {  // Changed from 1 to 8
        break pass.to_string();
    }
    println!("SSH password must be at least 8 characters. Please try again.");
};
```

---

## How Server Provisioning Works (Context)

The server (`spoq-web-apis`) performs these steps:

1. **Validates request** (IP format, password length, username)
2. **Creates DNS record** (username.spoq.dev → vps_ip via Cloudflare)
3. **Creates database record** (status: "provisioning")
4. **Generates registration code** (6 chars, 15-min expiration)
5. **Generates bash script** with:
   - System updates (apt-get)
   - Dependency installation (curl, jq, ca-certificates)
   - Hostname setup
   - User creation (spoq user with sudo)
   - Conductor installation and configuration
   - CLI installation
   - Firewall configuration (UFW: ports 22, 80, 443)
   - Caddy reverse proxy setup
6. **SSH connects to VPS** (60s connection timeout, 600s execution timeout)
7. **Executes script via "bash -s"** (writes to stdin)
8. **Captures stdout/stderr** (truncated to 2000 chars)
9. **Fallback**: Reads `/var/log/spoq-setup.log` if no output
10. **Updates database** (status: "ready" or "failed")
11. **Returns response** with install_script.status and install_script.output

**Key point:** All actual provisioning work happens on the server. The CLI just makes API calls and polls for status. No scripts are executed locally.

---

## Differences Between ttest CLI and spoq-cli

| Aspect | ttest (Shell Script) | spoq-cli (Rust TUI) |
|--------|---------------------|---------------------|
| Implementation | Pure bash script test harness | Interactive TUI application |
| Script execution | None (just API calls) | None (just API calls) |
| Token refresh | Manual with 401 check | Proactive + auto-refresh on 401 |
| Password validation | User's responsibility | Min 1 char (should be 8) |
| Error display | Shows JSON response directly | Formatted, but missing install_script.output |
| Retry logic | None | 3 attempts with user prompts |
| Polling | Basic with spinner | Separate status endpoint with progress |
| Output handling | Direct from API response | Structured display functions |

**Note:** ttest likely works because it's showing the raw JSON response which includes the install_script.output field.

---

## Summary

**Primary Issue:** CLI not displaying `install_script.output` when provisioning fails
**Secondary Issue:** Password validation mismatch (1 char vs 8 char minimum)
**Impact:** Users cannot debug failures because error details are hidden
**Fix Priority:** HIGH - This is a critical usability issue

---

## Testing Recommendations

After implementing the fix:

1. **Test with intentional SSH failure**: Wrong IP/password to see SSH error output
2. **Test with network issue**: Unreachable VPS to see connection timeout output
3. **Test with script failure**: Mock server response with failed install_script
4. **Test with short password**: Verify validation catches passwords < 8 chars
5. **Test with successful provision**: Ensure success case still works

---

## Related Files

**CLI (spoq-cli):**
- `src/auth/provisioning_flow.rs` - Main provisioning logic (line 720-728 needs fix)
- `src/auth/central_api.rs` - API client with response structures (line 241-246 has InstallScriptStatus)
- `tests/byovps_test.rs` - Test coverage

**Server (spoq-web-apis):**
- `src/handlers/byovps.rs` - BYOVPS endpoint handler (line 453 sets message, 465-468 sets install_script)
- `src/services/hostinger.rs` - Post-install script generation (line 497-672)
- `src/services/ssh_installer.rs` - SSH connection and script execution
