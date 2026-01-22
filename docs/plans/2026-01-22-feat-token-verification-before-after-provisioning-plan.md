---
title: Token Verification Before and After VPS Provisioning
type: feat
date: 2026-01-22
---

# Token Verification Before and After VPS Provisioning

## Overview

Implement two-phase token verification to ensure required credentials (Claude Code, GitHub CLI) are present before VPS provisioning starts and actually work on the VPS after migration completes. This prevents users from discovering token issues late in the setup process.

**Key Changes:**
- Add pre-provisioning gate that blocks if required tokens missing locally
- Add post-provisioning verification that tests tokens work on VPS via SSH
- Create new `src/auth/token_verification.rs` module with reusable verification functions
- Integrate at three points in provisioning flow (start + after managed/BYOVPS ready)

## Problem Statement / Motivation

**Current Issues:**

1. **No pre-flight check**: Users can start provisioning without required tokens (Claude Code, GitHub CLI), wasting time only to discover tokens missing after VPS is ready

2. **No post-migration validation**: Migration creates archive and saves path to credentials, but doesn't verify tokens actually work on VPS. User discovers issues much later when trying to use Claude/GitHub remotely.

3. **Silent failures**: If migration succeeds but tokens don't work on VPS, there's no feedback loop. User assumes everything is fine until they SSH in manually.

**Impact:**
- Poor user experience (discover problems late)
- Wasted time provisioning VPS without usable tokens
- No confidence that migration actually worked
- Manual debugging required to understand why tokens don't work on VPS

**From brainstorm decision:** Two-phase approach with pre-provisioning gate (block) and post-provisioning verification (warn).

## Proposed Solution

### Phase 1: Pre-Provisioning Verification (Blocking)

Add `verify_local_tokens()` at **start of provisioning flow** (before VPS type selection):

```rust
// src/auth/provisioning_flow.rs:357
pub fn run_provisioning_flow(...) -> Result<(), CentralApiError> {
    println!("\nPress Ctrl+C to cancel.\n");

    // NEW: Verify required tokens exist locally
    let local_verification = verify_local_tokens()?;
    if !local_verification.all_required_present {
        display_missing_tokens_error(&local_verification);
        return Err(CentralApiError::ServerError {
            status: 0,
            message: "Required tokens missing. Please login first.".to_string(),
        });
    }

    // Existing flow continues...
    let interrupted = setup_interrupt_handler();
    let vps_type = choose_vps_type(&interrupted)?;
    ...
}
```

**Blocking Criteria:**
- ❌ **Block if missing**: Claude Code token OR GitHub CLI token
- ✅ **Optional**: Codex token (continue with warning)

**Error Display:**
```
⚠️  Required tokens missing:
  ✗ Claude Code - not found
  ✗ GitHub CLI - not found

To continue, please login:
  1. Claude Code: Run 'claude login'
  2. GitHub CLI: Run 'gh auth login'

After logging in, run this command again to provision your VPS.
```

### Phase 2: Post-Provisioning Verification (Non-Blocking)

Add `verify_vps_tokens()` **after migration completes** at three integration points:

**Integration Point 1: Managed VPS Ready** (src/auth/provisioning_flow.rs:704)
```rust
// Run token migration after VPS is ready
println!("Running token migration...");
let migration_result = run_token_migration();
if let Some(ref archive_path) = migration_result.archive_path {
    credentials.token_archive_path = Some(archive_path.to_string_lossy().to_string());
    save_credentials(credentials);
}

// NEW: Verify tokens work on VPS
println!("\nVerifying tokens on VPS...");
match verify_vps_tokens(
    credentials.vps_ip.as_ref().unwrap(),
    "spoq", // SSH username for managed VPS
    &ssh_password,
) {
    Ok(verification) => {
        display_vps_verification_results(&verification);
    }
    Err(e) => {
        eprintln!("Warning: Could not verify tokens on VPS: {}", e);
        eprintln!("You may need to manually SSH and login to Claude Code/GitHub.");
    }
}

Ok(())
```

**Integration Point 2 & 3: BYOVPS Ready** (lines 1041, 1108)
- Same pattern as managed VPS
- Use BYOVPS SSH credentials (`byovps_creds.ssh_username`, `byovps_creds.ssh_password`)

**Success Display:**
```
✓ Claude Code verified on VPS
✓ GitHub CLI verified on VPS

Your VPS is ready with working credentials!
```

**Warning Display (graceful failure):**
```
⚠️  Warning: Could not verify tokens on VPS

  ✗ Claude Code - verification failed
  ✓ GitHub CLI - verified successfully

Your VPS is ready, but you may need to manually login to Claude Code:
  1. SSH to VPS: ssh spoq@1.2.3.4
  2. Run: claude login
  3. Verify: echo "test" | claude
```

### New Module: `src/auth/token_verification.rs`

```rust
/// Result of local token verification (before provisioning)
#[derive(Debug, Clone)]
pub struct LocalTokenVerification {
    pub claude_code_present: bool,
    pub github_cli_present: bool,
    pub codex_present: bool,
    pub all_required_present: bool,
}

/// Result of VPS token verification (after migration)
#[derive(Debug, Clone)]
pub struct VpsTokenVerification {
    pub claude_code_works: bool,
    pub github_cli_works: bool,
    pub ssh_error: Option<String>,
}

/// Error types for token verification
#[derive(Debug, Clone)]
pub enum TokenVerificationError {
    DetectionFailed(String),
    SshConnectionFailed(String),
    SshCommandTimeout(String),
    VerificationScriptFailed(String),
    SshpassNotInstalled(String),
}

impl std::fmt::Display for TokenVerificationError { ... }
impl std::error::Error for TokenVerificationError {}

/// Verify required tokens exist locally before provisioning
pub fn verify_local_tokens() -> Result<LocalTokenVerification, TokenVerificationError>

/// SSH to VPS and verify tokens work by running commands
pub fn verify_vps_tokens(
    vps_ip: &str,
    ssh_username: &str,
    ssh_password: &str,
) -> Result<VpsTokenVerification, TokenVerificationError>
```

**Implementation Details:**

**Local Verification:**
- Reuse existing `detect_tokens()` from `token_migration.rs`
- Check both required tokens present
- Return detailed result with all token statuses

**VPS Verification:**
- SSH to VPS and run verification commands
- **Claude Code test**: `echo "testing 123" | claude --non-interactive`
  - Success: Any response (exit code 0 or valid output)
  - Failure: Exit code non-zero, "command not found", or timeout
- **GitHub CLI test**: `gh auth status`
  - Success: Exit code 0 or output contains "Logged in"
  - Failure: Exit code non-zero, "not logged in", or timeout
- Use separate SSH connections per command (simplicity over optimization)
- 30-second timeout per SSH connection

**SSH Command Pattern** (following existing pattern from `token_migration.rs:462-619`):

```rust
use std::process::{Command, Stdio};
use std::time::Duration;

fn run_ssh_command(
    vps_ip: &str,
    ssh_username: &str,
    ssh_password: &str,
    command: &str,
) -> Result<String, TokenVerificationError> {
    // Check sshpass available
    if Command::new("which").arg("sshpass").output().is_err() {
        return Err(TokenVerificationError::SshpassNotInstalled(
            "sshpass not found. Install it to enable SSH verification.".to_string()
        ));
    }

    // Build SSH command
    let remote_host = format!("{}@{}", ssh_username, vps_ip);
    let escaped_password = ssh_password.replace("'", "'\\''");

    let output = Command::new("sshpass")
        .arg("-p").arg(&escaped_password)
        .arg("ssh")
        .arg("-o").arg("StrictHostKeyChecking=no")
        .arg("-o").arg("UserKnownHostsFile=/dev/null")
        .arg("-o").arg("ConnectTimeout=30")
        .arg(&remote_host)
        .arg(command)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| TokenVerificationError::SshConnectionFailed(e.to_string()))?;

    // Parse result
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(TokenVerificationError::VerificationScriptFailed(
            String::from_utf8_lossy(&output.stderr).to_string()
        ))
    }
}
```

## Technical Considerations

### SSH Security
- **Existing pattern**: Reuse `sshpass` with `-o StrictHostKeyChecking=no` (already used in codebase)
- **Password escaping**: Single quotes escaped as `'\\''` (matches existing `token_migration.rs` pattern)
- **Timeout**: 30 seconds per command prevents indefinite hangs
- **Credentials**: Use existing SSH password from provisioning flow (not persisted long-term)

### Error Handling
- **Local verification failure**: Block with clear error message and recovery instructions
- **VPS verification failure**: Warn but don't block VPS ready status (graceful degradation)
- **SSH connection errors**: Provide specific troubleshooting (IP, firewall, SSH enabled)
- **Command timeout**: Distinguish timeout from command failure in error messages

### Performance
- **Local check**: < 1 second (reuses existing `detect_tokens()`)
- **VPS check**: ~5-10 seconds (2 SSH connections × 2-5 seconds each)
- **Non-blocking**: VPS verification failures don't prevent TUI launch
- **No retry logic**: Single attempt per verification (user can manually retry via `/sync` in future)

### Testing Strategy
- **Unit tests**: Structure tests for `LocalTokenVerification`, `VpsTokenVerification`, error types
- **Integration tests**: Test with mock SSH commands (requires test VPS or mocking)
- **Error case tests**: Missing tokens, SSH timeout, command not found, auth failure
- **Manual testing**: Real VPS required to test SSH verification end-to-end

### Open Questions (Require Hands-On Testing)

1. **Claude CLI non-interactive mode**:
   - Does `--non-interactive` flag exist on VPS Claude CLI?
   - Alternative: `echo "test" | claude --stdin` or just `echo "test" | claude`?
   - Need to test actual command format on provisioned VPS

2. **GitHub CLI verification command**:
   - Confirm `gh auth status` exits with code 0 when authenticated
   - Alternative: `gh api user` (tests API access)?
   - Verify output format for parsing success/failure

3. **Timeout tuning**:
   - Is 30 seconds sufficient for slow VPS responses?
   - Should SSH connection timeout be separate from command execution timeout?
   - May need adjustment based on real-world VPS performance

4. **VPS username**:
   - Managed VPS uses "spoq" username - confirmed?
   - BYOVPS uses user-provided username - stored where?
   - Verify username availability at verification time

## Acceptance Criteria

### Must Have
- [x] Local verification blocks provisioning if Claude Code token missing
- [x] Local verification blocks provisioning if GitHub CLI token missing
- [x] Local verification allows provisioning with optional tokens (Codex) missing
- [x] VPS verification runs after managed VPS provisioning completes
- [x] VPS verification runs after BYOVPS provisioning completes (both code paths)
- [x] VPS verification tests Claude Code via SSH command execution
- [x] VPS verification tests GitHub CLI via SSH command execution
- [x] VPS verification failures show warnings but don't block VPS ready status
- [x] Clear error messages for missing tokens with recovery instructions
- [x] SSH timeout prevents indefinite hangs (30 seconds per command)

### Testing
- [x] Unit tests for `LocalTokenVerification` structure
- [x] Unit tests for `VpsTokenVerification` structure
- [x] Unit tests for `TokenVerificationError` enum and Display impl
- [ ] Integration test for local verification with mock token detection
- [ ] Integration test for VPS verification with mock SSH responses
- [ ] Error case tests for SSH timeout, connection failure, auth failure
- [ ] Manual test on real VPS to verify end-to-end flow

### User Experience
- [x] Clear console output showing verification in progress
- [x] Success messages with checkmarks (✓) for verified tokens
- [x] Warning messages with icons (⚠️) for failed verifications
- [x] Specific troubleshooting steps in error messages
- [x] No confusing technical jargon in user-facing messages

## Success Metrics

**User-Facing:**
- Users discover token issues before provisioning (not after)
- Reduced support requests about "tokens not working on VPS"
- Clear feedback when migration succeeds vs. fails

**Technical:**
- 100% of provisioning attempts verify local tokens first
- 100% of completed provisioning runs VPS verification
- < 10 seconds added to provisioning flow for VPS checks
- Graceful degradation when VPS verification unavailable

## Dependencies & Risks

### Dependencies
- **External**: `sshpass` command must be installed on user's machine
  - Mitigation: Check with `which sshpass` and show install instructions if missing
- **Internal**: Existing `detect_tokens()` function from `token_migration.rs`
- **VPS**: SSH access must work for verification to succeed

### Risks

**Risk 1: SSH Connection Failures**
- **Likelihood**: Medium (network issues, firewall, VPS not fully ready)
- **Impact**: High (VPS verification fails, user sees warnings)
- **Mitigation**:
  - Graceful failure with clear error messages
  - Provide troubleshooting steps (check IP, SSH port, firewall)
  - Don't block VPS ready status (user can manually verify)

**Risk 2: Claude CLI Command Unknown**
- **Likelihood**: Medium (command format may differ on VPS)
- **Impact**: High (Claude verification always fails)
- **Mitigation**:
  - Research exact command during implementation
  - Test on real VPS before finalizing
  - Fall back to warning if command format wrong

**Risk 3: Password with Special Characters**
- **Likelihood**: Low (already handled in existing SSH code)
- **Impact**: Medium (SSH auth fails)
- **Mitigation**: Reuse existing password escaping pattern (`'\\''`)

**Risk 4: VPS Not Ready for SSH**
- **Likelihood**: Low (verification runs after VPS marked ready)
- **Impact**: Medium (verification fails despite tokens being fine)
- **Mitigation**:
  - Add retry logic? (out of scope for MVP)
  - Clear warning message distinguishing SSH failure from token failure

## References & Research

### Internal References

**Token Migration Implementation:**
- `src/auth/token_migration.rs:42-91` - `detect_tokens()` function pattern
- `src/auth/token_migration.rs:183-281` - `export_tokens()` and error handling
- `src/auth/token_migration.rs:462-619` - SSH command execution pattern with `sshpass`
- `src/auth/token_migration.rs:284-336` - Error enum pattern (`SshTransferError`)

**Provisioning Flow Integration Points:**
- `src/auth/provisioning_flow.rs:357-384` - `run_provisioning_flow()` entry point
- `src/auth/provisioning_flow.rs:700-710` - Managed VPS ready, migration runs
- `src/auth/provisioning_flow.rs:1040-1048` - BYOVPS ready (early return path)
- `src/auth/provisioning_flow.rs:1106-1112` - BYOVPS ready (polling path)

**Error Handling Patterns:**
- `src/auth/provisioning_flow.rs:745-765` - `display_byovps_error()` user messaging
- `src/auth/token_migration.rs:393-422` - `parse_ssh_error()` stderr parsing

**Test Patterns:**
- `tests/token_migration_test.rs:10-40` - Integration test pattern
- `tests/token_migration_test.rs:376-419` - Error case test pattern
- `tests/ssh_transfer_test.rs` - SSH command tests with special characters

**Shell Script:**
- `scripts/migration/creds-migrate.sh` - Token detection and export script
  - `list` command outputs `[OK]` markers for detected tokens
  - Could potentially be extended for VPS-side verification

### Brainstorm & Design Documents

- `docs/brainstorms/2026-01-22-token-verification-brainstorm.md` - Complete feature design
  - Key decisions: Two-phase approach, required vs optional tokens, SSH method
  - Open questions: Claude CLI command format, GitHub CLI verification
- `docs/plans/2026-01-22-fix-vps-state-sync-before-provisioning-plan.md` - Similar pattern for server state checks
- `docs/brainstorms/2026-01-22-cli-startup-flow-fix-brainstorm.md` - Startup validation patterns

### Module Structure

**New File:** `src/auth/token_verification.rs`
- Export from `src/auth/mod.rs`:
  ```rust
  pub use token_verification::{
      verify_local_tokens, verify_vps_tokens,
      LocalTokenVerification, VpsTokenVerification,
      TokenVerificationError,
  };
  ```

**New Test File:** `tests/token_verification_test.rs`
- Structure tests (result types)
- Integration tests (with mocks)
- Error case tests

---

## Implementation Checklist

### Phase 1: Create Verification Module
- [x] Create `src/auth/token_verification.rs` file
- [x] Define `LocalTokenVerification` struct
- [x] Define `VpsTokenVerification` struct
- [x] Define `TokenVerificationError` enum with Display impl
- [x] Implement `verify_local_tokens()` using existing `detect_tokens()`
- [x] Implement `verify_vps_tokens()` with SSH command execution
- [x] Add helper function `run_ssh_command()` for reusable SSH pattern
- [x] Export public API from `src/auth/mod.rs`

### Phase 2: Integrate Local Verification (Pre-Provisioning Gate)
- [x] Add `verify_local_tokens()` call at `run_provisioning_flow()` start
- [x] Implement `display_missing_tokens_error()` function
- [x] Return error if required tokens missing
- [x] Show login instructions for missing tokens

### Phase 3: Integrate VPS Verification (Post-Provisioning)
- [x] Add verification after managed VPS ready (line ~704)
- [x] Add verification after BYOVPS ready - early return (line ~1041)
- [x] Add verification after BYOVPS ready - polling path (line ~1108)
- [x] Implement `display_vps_verification_results()` function
- [x] Handle verification errors gracefully with warnings

### Phase 4: Testing
- [x] Create `tests/token_verification_test.rs`
- [x] Add structure tests for result types
- [x] Add unit tests for error enum
- [ ] Add integration tests with mocked SSH
- [ ] Add error case tests (timeout, connection failure, auth failure)
- [ ] Manual test on real VPS with actual provisioning

### Phase 5: Documentation & Polish
- [ ] Update `README.md` if needed (mention token requirements)
- [ ] Add inline code comments for complex SSH logic
- [ ] Verify all error messages are user-friendly
- [ ] Test with various edge cases (special chars in password, slow VPS)

---

## Future Enhancements (Out of Scope for MVP)

1. **`/sync` command**: Re-run verification manually anytime to check token health
2. **Verification retry**: Auto-retry VPS verification if first attempt fails
3. **Persist verification status**: Track last verification time in credentials.json
4. **Periodic health check**: Background task to verify tokens still work on VPS
5. **Token refresh flow**: Detect expired VPS tokens and trigger re-migration
6. **SSH library**: Migrate from `sshpass` shell command to Rust SSH library (e.g., `ssh2` crate)
